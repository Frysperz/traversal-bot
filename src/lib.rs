pub mod engine;
pub mod nnue;

use engine::{FastBoard, FastMove, FastMoveKind, MAX_MOVES, NO_PIECE};
use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};

const SAFETY_FUEL: u64 = 75_000_000;
const INF: i32 = 2_000_000;
const SENTINEL_WIN: i32 = 1_000_000;
const MAX_DEPTH: usize = 8;

fn build_fast_board(turn: &Turn) -> FastBoard {
    let mut board = FastBoard::new();
    let state = &turn.state;

    for (i, &h) in state.heights.iter().enumerate().take(100) {
        board.heights[i] = h;
    }

    let pieces = state.pieces();
    board.piece_count = pieces.len().min(engine::MAX_PIECES);

    for (i, p) in pieces.iter().enumerate().take(board.piece_count) {
        board.pieces[i] = engine::FastPiece {
            sq: p.sq,
            kind: p.kind as u8,
            owner: p.owner,
            alive: p.sq != 255,
        };

        if p.sq != 255 && (p.sq as usize) < 100 {
            board.grid[p.sq as usize] = i as u8;
        }
    }

    board.to_move = state.to_move;
    board.init_accumulator();
    board
}

/// Évaluation géométrique grand-maître conçue pour écraser Sommet
#[inline(always)]
fn evaluate_expert(board: &FastBoard, perspective_side: u8) -> i32 {
    let opp_side = if perspective_side == 1 { 2 } else { 1 };

    if board.sentinel_exited[perspective_side as usize] {
        return SENTINEL_WIN;
    }
    if board.sentinel_exited[opp_side as usize] {
        return -SENTINEL_WIN;
    }

    let mut score = 0;
    let mut my_sent_sq = 255;
    let mut opp_sent_sq = 255;

    for i in 0..board.piece_count {
        let p = &board.pieces[i];
        if !p.alive || p.sq == 255 {
            continue;
        }

        let sq = p.sq as usize;
        let x = (sq % 10) as i32;
        let y = (sq / 10) as i32;
        let forward = if p.owner == 1 { y } else { 9 - y };
        let h = board.heights[sq] as i32;

        if p.kind == 2 {
            // Sentinelle : objectif course absolue
            if p.owner == perspective_side {
                my_sent_sq = p.sq;
                score += forward * 600 + h * 40;
                // Si la ligne d'arrivée est proche
                if forward >= 7 {
                    score += forward * 800;
                }
            } else {
                opp_sent_sq = p.sq;
                score -= forward * 600 + h * 40;
                if forward >= 7 {
                    score -= forward * 800;
                }
            }
        } else if p.kind == 1 {
            // Avant-garde : soutien et prise de hauteur
            let val = forward * 40 + h * 25 + 200;
            if p.owner == perspective_side {
                score += val;
            } else {
                score -= val;
            }
        } else {
            // Explorateur
            let val = forward * 15 + h * 10 + 80;
            if p.owner == perspective_side {
                score += val;
            } else {
                score -= val;
            }
        }
    }

    // Blocus de la Sentinelle adverse par nos Avant-gardes / Explorateurs
    if opp_sent_sq != 255 {
        let ox = (opp_sent_sq % 10) as i32;
        let oy = (opp_sent_sq / 10) as i32;

        for i in 0..board.piece_count {
            let p = &board.pieces[i];
            if p.alive && p.owner == perspective_side && p.sq != 255 {
                let px = (p.sq % 10) as i32;
                let py = (p.sq / 10) as i32;

                // Si notre pièce se place sur la même colonne et bloque la course ennemie
                if px == ox {
                    let is_in_front = if opp_side == 1 { py > oy } else { py < oy };
                    if is_in_front {
                        score += 350;
                    }
                }
            }
        }
    }

    score
}

fn sort_moves(moves: &mut [FastMove], count: usize) {
    let slice = &mut moves[..count];
    slice.sort_unstable_by_key(|m| {
        let mut priority = 0;
        if m.kind == FastMoveKind::Exit {
            priority += 500_000;
        } else if m.kind == FastMoveKind::Attack {
            priority += 80_000;
        } else {
            priority -= (m.cost as i32) * 20;
        }
        -priority
    });
}

fn alpha_beta(
    board: &mut FastBoard,
    depth: usize,
    mut alpha: i32,
    beta: i32,
    aborted: &mut bool,
) -> i32 {
    if fuel_left() < SAFETY_FUEL {
        *aborted = true;
        return 0;
    }

    if board.sentinel_exited[1] || board.sentinel_exited[2] || depth == 0 {
        return evaluate_expert(board, board.to_move);
    }

    let mut move_buffer = [FastMove {
        piece_idx: 0,
        from: 0,
        to: 0,
        kind: FastMoveKind::Move,
        cost: 0,
        captured_piece_idx: NO_PIECE,
    }; MAX_MOVES];

    let count = board.generate_moves(&mut move_buffer);
    if count == 0 {
        return -SENTINEL_WIN + 50;
    }

    sort_moves(&mut move_buffer, count);

    let mut max_eval = -INF;

    for i in 0..count {
        let m = move_buffer[i];
        let undo = board.apply_move(m);

        let eval = -alpha_beta(board, depth - 1, -beta, -alpha, aborted);

        board.unmake_move(undo);

        if *aborted {
            return 0;
        }

        if eval > max_eval {
            max_eval = eval;
        }
        if eval > alpha {
            alpha = eval;
        }
        if alpha >= beta {
            break;
        }
    }

    max_eval
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let my_side = turn.state.to_move;
    let pieces = turn.state.pieces();

    // 1. Priorité absolue : notre Sentinelle sort ce tour-ci
    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p_idx = m.piece as usize;
            if p_idx < pieces.len() && pieces[p_idx].kind as u8 == 2 {
                return i;
            }
        }
    }

    let mut root_board = build_fast_board(turn);

    let mut root_moves: Vec<(usize, FastMove)> = turn
        .moves
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let kind = match m.kind {
                MoveKind::Exit => FastMoveKind::Exit,
                MoveKind::Attack => FastMoveKind::Attack,
                _ => FastMoveKind::Move,
            };
            let cap = if m.to != 255 {
                root_board.grid[m.to as usize]
            } else {
                NO_PIECE
            };
            (
                i,
                FastMove {
                    piece_idx: m.piece,
                    from: turn.state.pieces()[m.piece as usize].sq,
                    to: m.to,
                    kind,
                    cost: m.cost,
                    captured_piece_idx: cap,
                },
            )
        })
        .collect();

    // Tri préliminaire des coups à la racine
    root_moves.sort_by_key(|(_, m)| {
        let mut p = 0;
        if m.kind == FastMoveKind::Exit {
            p += 100_000;
        } else if m.kind == FastMoveKind::Attack {
            p += 20_000;
        }
        -p
    });

    let mut best_idx = root_moves[0].0;

    // Iterative Deepening de d=1 à d=MAX_DEPTH
    for current_depth in 1..=MAX_DEPTH {
        if fuel_left() < SAFETY_FUEL {
            break;
        }

        let mut depth_best = root_moves[0].0;
        let mut alpha = -INF;
        let beta = INF;
        let mut aborted = false;

        for &(orig_idx, fast_m) in &root_moves {
            if fuel_left() < SAFETY_FUEL {
                aborted = true;
                break;
            }

            let undo = root_board.apply_move(fast_m);

            let score = -alpha_beta(
                &mut root_board,
                current_depth - 1,
                -beta,
                -alpha,
                &mut aborted,
            );

            root_board.unmake_move(undo);

            if aborted {
                break;
            }

            if score > alpha {
                alpha = score;
                depth_best = orig_idx;
            }
        }

        if !aborted {
            best_idx = depth_best;
        } else {
            break;
        }
    }

    best_idx
}

bot!(choose);
