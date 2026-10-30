mod engine;

use engine::{FastBoard, FastMove, FastMoveKind, MAX_MOVES, NO_PIECE};
use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

/// Convertit l'état du SDK en structure de simulation rapide FastBoard
fn build_fast_board(turn: &Turn) -> FastBoard {
    let mut board = FastBoard::new();
    let state = &turn.state;

    // Relief
    for (i, &h) in state.heights.iter().enumerate().take(100) {
        board.heights[i] = h;
    }

    // Pièces
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
    board
}

/// Évalue statiquement l'état du simulateur
fn evaluate_board(board: &FastBoard, my_side: u8) -> i32 {
    let opp_side = if my_side == 1 { 2 } else { 1 };

    if board.sentinel_exited[my_side as usize] {
        return 1_000_000;
    }
    if board.sentinel_exited[opp_side as usize] {
        return -1_000_000;
    }

    let mut score: i32 = 0;

    for i in 0..board.piece_count {
        let p = &board.pieces[i];
        if !p.alive {
            continue;
        }

        let y = (p.sq / 10) as i32;
        let progress = if p.owner == 1 { y } else { 9 - y };
        let h = board.heights[p.sq as usize] as i32;

        let piece_val = match p.kind {
            2 => 1000 + progress * 150 + h * 30, // Sentinel
            1 => 300 + progress * 40 + h * 20,   // Vanguard
            _ => 100 + progress * 20 + h * 10,   // Explorer
        };

        if p.owner == my_side {
            score += piece_val;
        } else {
            score -= piece_val;
        }
    }

    score
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let my_side = turn.state.to_move;

    // 1. Détection prioritaire : Sortie immédiate de notre Sentinelle
    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p_idx = m.piece as usize;
            let pieces = turn.state.pieces();
            if p_idx < pieces.len() && pieces[p_idx].kind as u8 == 2 {
                return i;
            }
        }
    }

    let mut board = build_fast_board(turn);
    let mut best_idx = 0;
    let mut best_score = i32::MIN;

    // 2. Exploration avec simulation du coup racine
    for (i, m) in turn.moves.iter().enumerate() {
        let fast_kind = match m.kind {
            MoveKind::Exit => FastMoveKind::Exit,
            MoveKind::Attack => FastMoveKind::Attack,
            _ => FastMoveKind::Move,
        };

        let cap_idx = if m.to != 255 {
            board.grid[m.to as usize]
        } else {
            NO_PIECE
        };

        let fast_move = FastMove {
            piece_idx: m.piece,
            from: turn.state.pieces()[m.piece as usize].sq,
            to: m.to,
            kind: fast_kind,
            cost: m.cost,
            captured_piece_idx: cap_idx,
        };

        // Appliquer le coup dans le simulateur
        let undo = board.apply_move(fast_move);

        // Simulation de la meilleure riposte adverse (profondeur 2)
        let mut opp_moves = [FastMove {
            piece_idx: 0,
            from: 0,
            to: 0,
            kind: FastMoveKind::Move,
            cost: 0,
            captured_piece_idx: NO_PIECE,
        }; MAX_MOVES];

        let opp_move_count = board.generate_moves(&mut opp_moves);
        let mut min_opp_score = i32::MAX;

        if opp_move_count == 0 {
            min_opp_score = evaluate_board(&board, my_side);
        } else {
            for j in 0..opp_move_count {
                let opp_undo = board.apply_move(opp_moves[j]);
                let score = evaluate_board(&board, my_side);
                board.unmake_move(opp_undo);

                if score < min_opp_score {
                    min_opp_score = score;
                }
            }
        }

        // Annuler le coup racine
        board.unmake_move(undo);

        if min_opp_score > best_score {
            best_score = min_opp_score;
            best_idx = i;
        }
    }

    best_idx
}

bot!(choose);
