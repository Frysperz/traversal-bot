pub mod engine;
pub mod nnue;

#[allow(dead_code)]
#[path = "nnue_weights.rs"]
mod nnue_weights;

use engine::{FastBoard, FastMove, FastMoveKind, MAX_MOVES, NO_PIECE};
use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};

const SAFETY_FUEL: u64 = 75_000_000;
const INF: i32 = 2_000_000;
const SENTINEL_WIN_SCORE: i32 = 1_000_000;
const MAX_DEPTH: usize = 12;

/// Injection des poids entraînés dans le moteur au démarrage
fn setup_trained_weights() {
    static mut DONE: bool = false;
    unsafe {
        if DONE {
            return;
        }
        engine::FEATURE_WEIGHTS = nnue_weights::TRAINED_FEATURE_WEIGHTS;
        engine::OUTPUT_WEIGHTS = nnue_weights::TRAINED_OUTPUT_WEIGHTS;
        engine::OUTPUT_BIAS = nnue_weights::TRAINED_OUTPUT_BIAS;
        DONE = true;
    }
}

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

/// Évaluation statique combinée (NNUE + bonus tactiques de relief et progression)
#[inline(always)]
fn evaluate(board: &FastBoard, perspective_side: u8) -> i32 {
    let opp_side = if perspective_side == 1 { 2 } else { 1 };

    if board.sentinel_exited[perspective_side as usize] {
        return SENTINEL_WIN_SCORE;
    }
    if board.sentinel_exited[opp_side as usize] {
        return -SENTINEL_WIN_SCORE;
    }

    let nnue_val = unsafe {
        board.acc.evaluate_output(
            &engine::OUTPUT_WEIGHTS,
            engine::OUTPUT_BIAS,
            perspective_side == 1,
        )
    };

    // Bonus de crête et progression de Sentinelle
    let mut positional = 0;
    for i in 0..board.piece_count {
        let p = &board.pieces[i];
        if !p.alive || p.sq == 255 {
            continue;
        }

        let sq = p.sq as usize;
        let y = (sq / 10) as i32;
        let progress = if p.owner == 1 { y } else { 9 - y };
        let h = board.heights[sq] as i32;

        let piece_score = match p.kind {
            2 => progress * 150 + h * 25,
            1 => progress * 50 + h * 15,
            _ => progress * 20 + h * 8,
        };

        if p.owner == perspective_side {
            positional += piece_score;
        } else {
            positional -= piece_score;
        }
    }

    nnue_val + positional
}

/// Ordonnancement des coups (Move Ordering) pour maximiser les coupures bêta
fn sort_moves(moves: &mut [FastMove], count: usize, killer: Option<FastMove>) {
    let slice = &mut moves[..count];
    slice.sort_unstable_by_key(|m| {
        let mut priority = 0;

        if m.kind == FastMoveKind::Exit {
            priority += 200_000;
        } else if m.kind == FastMoveKind::Attack {
            priority += 50_000;
        }

        if let Some(k) = killer {
            if m.from == k.from && m.to == k.to && m.kind == k.kind {
                priority += 30_000;
            }
        }

        -priority
    });
}

/// Moteur Alpha-Bêta récursif (Negamax)
fn alpha_beta(
    board: &mut FastBoard,
    depth: usize,
    mut alpha: i32,
    beta: i32,
    my_side: u8,
    killers: &mut [Option<FastMove>; MAX_DEPTH],
    aborted: &mut bool,
) -> i32 {
    if fuel_left() < SAFETY_FUEL {
        *aborted = true;
        return 0;
    }

    if board.sentinel_exited[1] || board.sentinel_exited[2] || depth == 0 {
        return evaluate(board, board.to_move);
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
        return -SENTINEL_WIN_SCORE + 100;
    }

    sort_moves(&mut move_buffer, count, killers[depth]);

    let mut max_eval = -INF;

    for i in 0..count {
        let m = move_buffer[i];
        let undo = board.apply_move(m);

        let eval = -alpha_beta(board, depth - 1, -beta, -alpha, my_side, killers, aborted);

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

        // Coupure Bêta
        if alpha >= beta {
            if m.kind != FastMoveKind::Exit {
                killers[depth] = Some(m);
            }
            break;
        }
    }

    max_eval
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    setup_trained_weights();

    let my_side = turn.state.to_move;

    // Détection de victoire instantanée (racine)
    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p_idx = m.piece as usize;
            let pieces = turn.state.pieces();
            if p_idx < pieces.len() && pieces[p_idx].kind as u8 == 2 {
                return i;
            }
        }
    }

    let mut root_board = build_fast_board(turn);

    // Conversion des coups SDK vers FastMove pour la racine
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

    let mut best_overall_idx = 0;
    let mut killers: [Option<FastMove>; MAX_DEPTH] = [None; MAX_DEPTH];

    // Approfondissement itératif (Iterative Deepening) : d = 1 jusqu'à MAX_DEPTH
    for current_depth in 1..=MAX_DEPTH {
        if fuel_left() < SAFETY_FUEL {
            break;
        }

        let mut depth_best_idx = root_moves[0].0;
        let mut alpha = -INF;
        let beta = INF;
        let mut depth_aborted = false;

        for &(orig_idx, fast_m) in &root_moves {
            if fuel_left() < SAFETY_FUEL {
                depth_aborted = true;
                break;
            }

            let undo = root_board.apply_move(fast_m);

            let score = -alpha_beta(
                &mut root_board,
                current_depth - 1,
                -beta,
                -alpha,
                my_side,
                &mut killers,
                &mut depth_aborted,
            );

            root_board.unmake_move(undo);

            if depth_aborted {
                break;
            }

            if score > alpha {
                alpha = score;
                depth_best_idx = orig_idx;
            }
        }

        // On ne valide le coup que si toute la profondeur a été explorée sans épuiser le fuel
        if !depth_aborted {
            best_overall_idx = depth_best_idx;
        } else {
            break;
        }
    }

    best_overall_idx
}

bot!(choose);
