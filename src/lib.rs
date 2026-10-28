mod weights;

use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};
use weights::{B1, B2, HIDDEN_SIZE, INPUT_SIZE, W1, W2};

// Inférence du réseau de neurones en f32 natif
fn neural_network_eval(features: &[f32; INPUT_SIZE]) -> f32 {
    let mut hidden = [0.0f32; HIDDEN_SIZE];

    // Couche cachée 1 + ReLU
    for j in 0..HIDDEN_SIZE {
        let mut sum = B1[j];
        for i in 0..INPUT_SIZE {
            sum += features[i] * W1[i][j];
        }
        hidden[j] = if sum > 0.0 { sum } else { 0.0 }; // ReLU
    }

    // Couche de sortie
    let mut output = B2;
    for j in 0..HIDDEN_SIZE {
        output += hidden[j] * W2[j];
    }

    output
}

fn extract_features_and_eval(turn: &Turn, move_idx: usize) -> f32 {
    let m = &turn.moves[move_idx];
    let state = &turn.state;

    // Condition absolue : sortie du plateau
    if m.kind == MoveKind::Exit {
        return 10_000.0;
    }

    let mut features = [0.0f32; INPUT_SIZE];

    // Encodage du coup joué dans les inputs
    if m.to != 255 {
        let target_idx = m.to as usize;
        if target_idx < 100 {
            let y = (target_idx / 10) as f32;
            let progress = if state.to_move == 1 { y / 9.0 } else { (9.0 - y) / 9.0 };
            features[target_idx] = progress;
        }
    }

    // Caractéristiques globales
    features[100] = (m.attack as f32) / 5.0;
    features[101] = (m.defense as f32) / 5.0;
    features[102] = (m.cost as f32) / 10.0;
    features[103] = if m.kind == MoveKind::Attack { 1.0 } else { 0.0 };

    neural_network_eval(&features)
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let mut best_idx = 0;
    let mut best_score = f32::NEG_INFINITY;

    for i in 0..turn.moves.len() {
        let score = extract_features_and_eval(turn, i);
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    best_idx
}

bot!(choose);
