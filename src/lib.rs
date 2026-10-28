use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

// Poids optimisés issus de la session d'apprentissage
const SCORE_EXIT: i32               = 1_000_000;
const WEIGHT_ATTACK: i32            = 2_340;
const WEIGHT_DOWNWARD_CHARGE: i32   = 620;
const WEIGHT_EFFICIENT_MOVE: i32    = 15;
const WEIGHT_FORWARD_PROGRESS: i32  = 90;

fn evaluate_move(turn: &Turn, move_idx: usize) -> i32 {
    let m = &turn.moves[move_idx];
    let state = &turn.state;

    // 1. Sortie de plateau (priorité absolue aux points et à la victoire)
    if m.kind == MoveKind::Exit {
        return SCORE_EXIT;
    }

    let mut score: i32 = 0;

    // 2. Attaques et combat
    if m.kind == MoveKind::Attack {
        score += WEIGHT_ATTACK;
        let net_power = (m.attack as i32) - (m.defense as i32);
        score += net_power * WEIGHT_DOWNWARD_CHARGE;
    }

    // 3. Avancée vers le camp adverse
    if m.target != 255 {
        let target_case = m.target as usize;
        let target_y = (target_case / 10) as i32;

        // Rouge (1) progresse vers le bas (y=9), Bleu (2) vers le haut (y=0)
        let forward_progress = if state.side_to_move == 1 {
            target_y
        } else {
            9 - target_y
        };

        score += forward_progress * WEIGHT_FORWARD_PROGRESS;

        // Pénalité sur le coût en points de mouvement (dénivelé / détours inutiles)
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
