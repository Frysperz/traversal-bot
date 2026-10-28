use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

// Constantes calibrées par Self-Play
const SCORE_SENTINEL_WIN: i32       = 1_000_000;
const WEIGHT_ATTACK: i32            = 2_340;
const WEIGHT_DOWNWARD_CHARGE: i32   = 620;
const WEIGHT_EFFICIENT_MOVE: i32    = 15;
const WEIGHT_SENTINEL_ADVANCE: i32  = 145;
const WEIGHT_VANGUARD_ADVANCE: i32  = 75;

fn evaluate_move(turn: &Turn, move_idx: usize) -> i32 {
    let m = &turn.moves[move_idx];
    let state = &turn.state;

    // 1. Victoire immédiate
    if m.kind == MoveKind::Exit {
        return SCORE_SENTINEL_WIN;
    }

    let mut score: i32 = 0;

    // 2. Attaques et combat
    if m.kind == MoveKind::Attack {
        score += WEIGHT_ATTACK;
        let net_power = (m.attack as i32) - (m.defense as i32);
        score += net_power * WEIGHT_DOWNWARD_CHARGE;
    }

    // 3. Déplacement et poussée de pions
    if m.target != 255 {
        let target_case = m.target as usize;
        let target_y = (target_case / 10) as i32;

        let forward_progress = if state.side_to_move == 1 {
            target_y
        } else {
            9 - target_y
        };

        let piece_idx = m.piece as usize;
        if piece_idx < state.pieces.len() {
            let piece_kind = state.pieces[piece_idx].kind as u8;
            match piece_kind {
                2 => score += forward_progress * WEIGHT_SENTINEL_ADVANCE,
                1 => score += forward_progress * WEIGHT_VANGUARD_ADVANCE,
                _ => score += forward_progress * 25,
            }
        }

        // Pénalité sur le coût en déplacement/dénivelé
        score -= (m.cost as i32) * WEIGHT_EFFICIENT_MOVE;
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
        let score = evaluate_move(turn, i);
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    best_idx
}

bot!(choose);
