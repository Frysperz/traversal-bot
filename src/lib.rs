use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};

const WEIGHT_EXIT: i32 = 100_000;
const WEIGHT_SENTINEL_EXIT: i32 = 500_000;
const WEIGHT_ATTACK: i32 = 1_500;
const WEIGHT_PROGRESS: i32 = 40;
const WEIGHT_HEIGHT: i32 = 15;
const SAFETY_FUEL_THRESHOLD: i64 = 100_000_000;

fn evaluate_move(turn: &Turn, move_idx: usize) -> i32 {
    let m = &turn.moves[move_idx];
    let state = &turn.state;
    let mut score: i32 = 0;

    if m.kind == MoveKind::Exit {
        let piece_idx = m.piece as usize;
        let is_sentinel = piece_idx < state.pieces.len() && state.pieces[piece_idx].kind as u8 == 2;
        if is_sentinel {
            return WEIGHT_SENTINEL_EXIT;
        }
        return WEIGHT_EXIT;
    }

    if m.kind == MoveKind::Attack {
        score += WEIGHT_ATTACK + (m.attack as i32 - m.defense as i32) * 50;
    }

    if m.target != 255 {
        let target_case = m.target as usize;
        let target_y = (target_case / 10) as i32;
        let progress = if state.side_to_move == 1 { target_y } else { 9 - target_y };
        score += progress * WEIGHT_PROGRESS;

        if target_case < state.height.len() {
            let height = state.height[target_case] as i32;
            score += height * WEIGHT_HEIGHT;
        }
    }

    score
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let mut best_idx = 0;
    let mut best_score = i32::MIN;

    for i in 0..turn.moves.len() {
        if fuel_left() < SAFETY_FUEL_THRESHOLD {
            break;
        }
        let score = evaluate_move(turn, i);
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    best_idx
}

bot!(choose);
