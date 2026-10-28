use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

// --- SYSTÈME DE POIDS STRATÉGIQUES ---
const SCORE_SENTINEL_WIN: i32 = 1_000_000; // Priorité absolue : sortie de plateau
const WEIGHT_ATTACK: i32      = 2_000;     // Priorité aux éliminations de pièces adverses
const WEIGHT_DOWNWARD_CHARGE: i32 = 500;   // Bonus quand notre attaque surclasse nettement la défense
const WEIGHT_EFFICIENT_MOVE: i32  = 20;    // Privilégier les déplacements économes en points/relief

fn evaluate_move(turn: &Turn, move_idx: usize) -> i32 {
    let m = &turn.moves[move_idx];
    let mut score: i32 = 0;

    // 1. Sortie de plateau
    // Le premier joueur à franchir l'arrivée avec une Sentinelle gagne sur-le-champ,
    // et toute sortie rapporte des points décisifs.
    if m.kind == MoveKind::Exit {
        return SCORE_SENTINEL_WIN;
    }

    // 2. Attaques et menaces (Vanguards / Sentinels)
    if m.kind == MoveKind::Attack {
        score += WEIGHT_ATTACK;

        // Si notre force d'attaque dépasse la défense adverse (ex: charge ou avantage de hauteur)
        let net_power = (m.attack as i32) - (m.defense as i32);
        score += net_power * WEIGHT_DOWNWARD_CHARGE;
    }

    // 3. Efficacité du déplacement & contrôle du terrain
    // Plus le coup franchit de distance pour un coût faible, plus il est valorisé
    score -= (m.cost as i32) * WEIGHT_EFFICIENT_MOVE;

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
