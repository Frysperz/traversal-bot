mod nnue_weights;

use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};
use nnue_weights::{ACCUMULATOR_SIZE, FEATURE_WEIGHTS, OUTPUT_BIAS, OUTPUT_WEIGHTS};

const SAFETY_FUEL: u64 = 60_000_000;
const INF: i32 = 2_000_000;
const SENTINEL_EXIT_WIN: i32 = 1_000_000;

#[derive(Clone, Copy)]
pub struct Accumulator {
    pub vals: [i16; ACCUMULATOR_SIZE],
}

impl Accumulator {
    pub fn new() -> Self {
        Self { vals: [0i16; ACCUMULATOR_SIZE] }
    }

    /// Mise à jour incrémentale NNUE (Ajout / Retrait de caractéristique en O(1))
    #[inline(always)]
    pub fn add_feature(&mut self, feature_idx: usize) {
        if feature_idx < nnue_weights::FEATURE_COUNT {
            let weights = &FEATURE_WEIGHTS[feature_idx];
            for i in 0..ACCUMULATOR_SIZE {
                self.vals[i] = self.vals[i].saturating_add(weights[i]);
            }
        }
    }

    #[inline(always)]
    pub fn sub_feature(&mut self, feature_idx: usize) {
        if feature_idx < nnue_weights::FEATURE_COUNT {
            let weights = &FEATURE_WEIGHTS[feature_idx];
            for i in 0..ACCUMULATOR_SIZE {
                self.vals[i] = self.vals[i].saturating_sub(weights[i]);
            }
        }
    }

    /// Évaluation rapide via la couche de sortie quantifiée + activation Clipped ReLU
    #[inline(always)]
    pub fn evaluate_output(&self, perspective_is_side1: bool) -> i32 {
        let mut sum: i32 = OUTPUT_BIAS;
        for i in 0..ACCUMULATOR_SIZE {
            // Clipped ReLU (0..127)
            let val = self.vals[i].clamp(0, 127) as i32;
            sum += val * (OUTPUT_WEIGHTS[i] as i32);
        }
        if perspective_is_side1 { sum } else { -sum }
    }
}

/// Convertit une pièce sur une case donnée en index de feature binaire
#[inline(always)]
fn get_feature_index(sq: u8, kind: u8, owner: u8) -> usize {
    let side_offset = if owner == 1 { 0 } else { 1 };
    let piece_offset = (kind as usize).min(2) * 2 + side_offset;
    (sq as usize) * 6 + piece_offset
}

/// Évaluation statique combinée (NNUE + Heuristique de course et blocage)
fn evaluate_position(turn: &Turn, acc: &Accumulator, my_side: u8) -> i32 {
    let nnue_score = acc.evaluate_output(my_side == 1);
    let state = &turn.state;
    let pieces = state.pieces();
    let opp_side = if my_side == 1 { 2 } else { 1 };

    let mut tactical_bonus: i32 = 0;

    if let Some(my_sent) = pieces.iter().find(|p| p.owner == my_side && p.kind as u8 == 2) {
        let y = (my_sent.sq / 10) as i32;
        let progress = if my_side == 1 { y } else { 9 - y };
        tactical_bonus += progress * 400;
    }

    if let Some(opp_sent) = pieces.iter().find(|p| p.owner == opp_side && p.kind as u8 == 2) {
        let ey = (opp_sent.sq / 10) as i32;
        let opp_progress = if opp_side == 1 { ey } else { 9 - ey };
        // Malus sévère si la Sentinelle adverse approche de la ligne de sortie
        if opp_progress >= 5 {
            tactical_bonus -= opp_progress * 500;
        }
    }

    nnue_score + tactical_bonus
}

/// Ordonnancement des coups pour provoquer des coupures Alpha-Bêta immédiates
fn sort_moves(turn: &Turn, move_order: &mut [usize]) {
    move_order.sort_unstable_by_key(|&idx| {
        let m = &turn.moves[idx];
        let mut priority: i32 = 0;
        if m.kind == MoveKind::Exit {
            priority += 500_000;
        } else if m.kind == MoveKind::Attack {
            priority += 50_000 + (m.attack as i32 - m.defense as i32) * 1_000;
        } else {
            priority -= (m.cost as i32) * 20;
        }
        -priority
    });
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let state = &turn.state;
    let pieces = state.pieces();
    let my_side = state.to_move;

    // 1. Victoire immédiate par sortie de Sentinelle
    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p_idx = m.piece as usize;
            if p_idx < pieces.len() && pieces[p_idx].kind as u8 == 2 {
                return i;
            }
        }
    }

    // 2. Initialisation de l'accumulateur NNUE pour la position racine
    let mut root_acc = Accumulator::new();
    for p in pieces.iter() {
        if p.sq != 255 {
            let f_idx = get_feature_index(p.sq, p.kind as u8, p.owner);
            root_acc.add_feature(f_idx);
        }
    }

    let mut move_order: Vec<usize> = (0..turn.moves.len()).collect();
    sort_moves(turn, &mut move_order);

    let mut best_idx = move_order[0];
    let mut best_score = -INF;
    let mut alpha = -INF;
    let beta = INF;

    // 3. Recherche Alpha-Bêta à profondeur 2 avec mises à jour incrémentales
    for &idx in &move_order {
        if fuel_left() < SAFETY_FUEL {
            break;
        }

        let m = &turn.moves[idx];
        let mut move_acc = root_acc;
        let p_idx = m.piece as usize;

        // Mise à jour incrémentale NNUE du coup allié
        if p_idx < pieces.len() {
            let old_sq = pieces[p_idx].sq;
            let kind = pieces[p_idx].kind as u8;
            let owner = pieces[p_idx].owner;

            if old_sq != 255 {
                move_acc.sub_feature(get_feature_index(old_sq, kind, owner));
            }

            if m.to != 255 {
                // Si une pièce adverse occupait la case d'arrivée, on la retire de l'accumulateur
                for opp_p in pieces.iter().filter(|p| p.owner != my_side && p.sq == m.to) {
                    move_acc.sub_feature(get_feature_index(opp_p.sq, opp_p.kind as u8, opp_p.owner));
                    break;
                }
                move_acc.add_feature(get_feature_index(m.to, kind, owner));
            }
        }

        let mut node_score = evaluate_position(turn, &move_acc, my_side);

        if m.kind == MoveKind::Exit {
            node_score += 20_000;
        }

        // Élagage Alpha-Bêta
        if node_score > best_score {
            best_score = node_score;
            best_idx = idx;
        }

        if node_score > alpha {
            alpha = node_score;
        }
        if alpha >= beta {
            break; // Coupure Beta
        }
    }

    best_idx
}

bot!(choose);
