mod weights;

use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};
use weights::{B1, B2, HIDDEN_SIZE, INPUT_SIZE, W1, W2};

const SAFETY_FUEL: u64 = 60_000_000;

fn neural_eval(features: &[f32; INPUT_SIZE]) -> f32 {
    let mut hidden = [0.0f32; HIDDEN_SIZE];

    for j in 0..HIDDEN_SIZE {
        let mut sum = B1[j];
        for i in 0..INPUT_SIZE {
            sum += features[i] * W1[i][j];
        }
        hidden[j] = if sum > 0.0 { sum } else { 0.0 };
    }

    let mut output = B2;
    for j in 0..HIDDEN_SIZE {
        output += hidden[j] * W2[j];
    }
    output
}

fn evaluate_move(turn: &Turn, move_idx: usize) -> f32 {
    let m = &turn.moves[move_idx];
    let state = &turn.state;
    let pieces = state.pieces();

    // 1. Priorité absolue : sortie du plateau
    if m.kind == MoveKind::Exit {
        let p_idx = m.piece as usize;
        let is_sentinel = p_idx < pieces.len() && (pieces[p_idx].kind as u8 == 2);
        return if is_sentinel { 200_000.0 } else { 25_000.0 };
    }

    let mut features = [0.0f32; INPUT_SIZE];

    // 2. Encodage du plateau (0..99)
    for p in pieces.iter() {
        let sq = p.sq as usize;
        if sq < 100 {
            features[sq] = if p.owner == state.to_move { 1.0 } else { -1.0 };
        }
    }

    // 3. Caractéristiques du coup & Topologie relative
    let mut target_y = 0.0f32;
    let mut height_val = 0.0f32;
    let mut relative_height_advantage = 0.0f32;
    let target_sq = m.to as usize;

    if m.to != 255 {
        let tx = (target_sq % 10) as i32;
        let ty = (target_sq / 10) as i32;
        target_y = if state.to_move == 1 { (ty as f32) / 9.0 } else { ((9 - ty) as f32) / 9.0 };

        if target_sq < state.heights.len() {
            let my_h = state.heights[target_sq] as i32;
            height_val = (my_h as f32) / 4.0;

            // Calcul du dénivelé moyen par rapport aux 4 cases adjacentes (crête vs ravin)
            let mut diff_sum = 0;
            let mut neighbors = 0;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = tx + dx;
                let ny = ty + dy;
                if (0..10).contains(&nx) && (0..10).contains(&ny) {
                    let n_sq = (ny * 10 + nx) as usize;
                    diff_sum += my_h - (state.heights[n_sq] as i32);
                    neighbors += 1;
                }
            }
            if neighbors > 0 {
                relative_height_advantage = (diff_sum as f32) / (neighbors as f32 * 4.0);
            }
        }
    }

    let piece_kind = if (m.piece as usize) < pieces.len() {
        pieces[m.piece as usize].kind as u8
    } else {
        0
    };

    features[100] = target_y;
    features[101] = (piece_kind as f32) / 2.0;
    features[102] = if m.kind == MoveKind::Attack { 1.0 } else { 0.0 };
    features[103] = relative_height_advantage;
    features[104] = (m.attack as f32) / 5.0;
    features[105] = (m.defense as f32) / 5.0;
    features[106] = (m.cost as f32) / 5.0;
    features[107] = height_val;
    features[108] = if piece_kind == 2 { 1.0 } else { 0.0 };
    features[109] = ((m.attack as i32 - m.defense as i32) as f32) / 5.0;

    let mut score = neural_eval(&features);

    // 4. Anticipation défensive (Lookahead 1-ply simplifié)
    if m.to != 255 {
        let tx = (target_sq % 10) as i32;
        let ty = (target_sq / 10) as i32;
        let my_h = state.heights[target_sq] as i32;

        for enemy in pieces.iter().filter(|p| p.owner != state.to_move) {
            let ex = (enemy.sq % 10) as i32;
            let ey = (enemy.sq / 10) as i32;
            let dist = (tx - ex).abs() + (ty - ey).abs();

            if dist == 1 {
                let en_h = state.heights[enemy.sq as usize] as i32;
                let en_kind = enemy.kind as u8;

                // L'ennemi bénéficie d'un bonus s'il attaque depuis le haut
                let en_effective_strength = match en_kind {
                    2 => 3, // Sentinel
                    1 => 2, // Vanguard
                    _ => 1, // Explorer
                } + (if en_h > my_h { 1 } else { 0 });

                let my_base_def = match piece_kind {
                    2 => 3,
                    1 => 2,
                    _ => 1,
                };

                // Si l'adversaire peut nous éliminer au demi-coup suivant
                if en_effective_strength >= my_base_def {
                    let penalty = match piece_kind {
                        2 => 300.0, // Perdre une Sentinelle est catastrophique
                        1 => 120.0,
                        _ => 50.0,
                    };
                    score -= penalty;
                }
            }
        }
    }

    score
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let mut best_idx = 0;
    let mut best_score = f32::NEG_INFINITY;

    for i in 0..turn.moves.len() {
        if fuel_left() < SAFETY_FUEL {
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
