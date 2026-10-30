pub mod engine;
pub mod nnue;

use engine::{
    FastBoard, FastMove, FastMoveKind, FEATURE_WEIGHTS, MAX_MOVES, NO_PIECE, OUTPUT_BIAS,
    OUTPUT_WEIGHTS,
};
use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};

const SAFETY_FUEL: u64 = 60_000_000;
const SENTINEL_WIN_SCORE: i32 = 1_000_000;

fn init_weights_if_needed() {
    static mut INITIALIZED: bool = false;
    unsafe {
        if INITIALIZED {
            return;
        }

        for sq in 0..100 {
            let y = (sq / 10) as i32;

            let f_sent_red = (sq * 6) + 4;
            for i in 0..engine::MAX_MOVES.min(nnue::ACCUMULATOR_SIZE) {
                FEATURE_WEIGHTS[f_sent_red][i] = (y * 12) as i16;
            }

            let f_sent_blue = (sq * 6) + 5;
            for i in 0..engine::MAX_MOVES.min(nnue::ACCUMULATOR_SIZE) {
                FEATURE_WEIGHTS[f_sent_blue][i] = ((9 - y) * 12) as i16;
            }
        }

        OUTPUT_BIAS = 0;
        for i in 0..nnue::ACCUMULATOR_SIZE {
            OUTPUT_WEIGHTS[i] = 2;
        }

        INITIALIZED = true;
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

fn evaluate_node(board: &FastBoard, my_side: u8) -> i32 {
    let opp_side = if my_side == 1 { 2 } else { 1 };

    if board.sentinel_exited[my_side as usize] {
        return SENTINEL_WIN_SCORE;
    }
    if board.sentinel_exited[opp_side as usize] {
        return -SENTINEL_WIN_SCORE;
    }

    let nnue_score = unsafe {
        board.acc.evaluate_output(&OUTPUT_WEIGHTS, OUTPUT_BIAS, my_side == 1)
    };

    let mut height_bonus = 0;
    for i in 0..board.piece_count {
        let p = &board.pieces[i];
        if p.alive && p.sq != 255 {
            let h = board.heights[p.sq as usize] as i32;
            if p.owner == my_side {
                height_bonus += h * 20;
            } else {
                height_bonus -= h * 20;
            }
        }
    }

    nnue_score + height_bonus
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    init_weights_if_needed();

    let my_side = turn.state.to_move;

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

    for (i, m) in turn.moves.iter().enumerate() {
        if fuel_left() < SAFETY_FUEL {
            break;
        }

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

        let undo = board.apply_move(fast_move);

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
            min_opp_score = evaluate_node(&board, my_side);
        } else {
            for j in 0..opp_move_count {
                let opp_undo = board.apply_move(opp_moves[j]);
                let score = evaluate_node(&board, my_side);
                board.unmake_move(opp_undo);

                if score < min_opp_score {
                    min_opp_score = score;
                }
            }
        }

        board.unmake_move(undo);

        if min_opp_score > best_score {
            best_score = min_opp_score;
            best_idx = i;
        }
    }

    best_idx
}

bot!(choose);
