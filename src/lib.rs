mod weights;

use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};
use weights::{B1, B2, HIDDEN_SIZE, INPUT_SIZE, W1, W2};

fn neural_network_eval(features: &[f32; INPUT_SIZE]) -> f32 {
    let mut hidden = [0.0f32; HIDDEN_SIZE];

    // Passe couche cachée + ReLU
    for j in 0..HIDDEN_SIZE {
        let mut sum = B1[j];
        for i in 0..INPUT_SIZE {
            sum += features[i] * W1[i][j];
        }
        hidden[j] = if sum > 0.0 { sum } else { 0.0 };
    }

    // Sortie
    let mut output = B2;
    for j in 0..HIDDEN_SIZE {
        output += hidden[j] * W2[j];
    }
    output
}

fn evaluate_move(turn: &Turn, move_idx: usize) -> f32 {
    let m = &turn.moves[move_idx];
    let state = &turn.state;

    // Priorité absolue : sortie du plateau
    if m.kind == MoveKind::Exit {
        let p_idx = m.piece as usize;
        let is_sentinel = p_idx < state.pieces.len() && (state.pieces[p_idx].kind as u8 == 2);
        return if is_sentinel { 50_000.0 } else { 10_000.0 };
    }

    let mut features = [0.0f32; INPUT_SIZE];

    // 1. Encodage des pièces sur le plateau (0..99)
    for p in state.pieces.iter() {
        let sq = p.square as usize;
        if sq < 100 {
            features[sq] = if p.side == state.to_move { 1.0 } else { -1.0 };
        }
    }

    // 2. Encodage du coup testé
    let mut target_y = 0.0f32;
    let mut height_val = 0.0f32;

    if m.to != 255 {
        let sq = m.to as usize;
        let y = (sq / 10) as f32;
        target_y = if state.to_move == 1 { y / 9.0 } else { (9.0 - y) / 9.0 };

        if sq < state.heights.len() {
            height_val = (state.heights[sq] as f32) / 4.0;
        }
    }

    let piece_kind = if (m.piece as usize) < state.pieces.len() {
        state.pieces[m.piece as usize].kind as u8
    } else {
        0
    };

    features[100] = target_y;
    features[101] = (piece_kind as f32) / 2.0;
    features[102] = if m.kind == MoveKind::Attack { 1.0 } else { 0.0 };
    features[103] = 0.0;
    features[104] = (m.attack as f32) / 5.0;
    features[105] = (m.defense as f32) / 5.0;
    features[106] = (m.cost as f32) / 5.0;
    features[107] = height_val;
    features[108] = if piece_kind == 2 { 1.0 } else { 0.0 };
    features[109] = ((m.attack as i32 - m.defense as i32) as f32) / 5.0;

    neural_network_eval(&features)
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let mut best_idx = 0;
    let mut best_score = f32::NEG_INFINITY;

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
