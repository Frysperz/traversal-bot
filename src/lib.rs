use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};

// Réserve de sécurité : arrêt immédiat de la recherche s'il reste moins de 80M de fuel
const SAFETY_FUEL: u64 = 80_000_000;

const INF: i32 = 1_000_000;
const WIN_SCORE: i32 = 500_000;

#[derive(Clone, Copy)]
struct SimPiece {
    sq: u8,
    kind: u8,  // 0: Explorer, 1: Vanguard, 2: Sentinel
    owner: u8, // 1: Rouge, 2: Bleu
    alive: bool,
}

#[derive(Clone)]
struct SimBoard {
    pieces: Vec<SimPiece>,
    to_move: u8,
    heights: [u8; 100],
    exited_sentinel: [bool; 3], // index 1: Rouge, 2: Bleu
    total_exits: u8,
    scores: [u8; 3],
}

impl SimBoard {
    fn from_turn(turn: &Turn) -> Self {
        let mut heights = [0u8; 100];
        for (i, &h) in turn.state.heights.iter().enumerate().take(100) {
            heights[i] = h;
        }

        let pieces = turn
            .state
            .pieces()
            .iter()
            .map(|p| SimPiece {
                sq: p.sq,
                kind: p.kind as u8,
                owner: p.owner,
                alive: p.sq != 255,
            })
            .collect();

        Self {
            pieces,
            to_move: turn.state.to_move,
            heights,
            exited_sentinel: [false; 3],
            total_exits: 0,
            scores: [0; 3],
        }
    }

    /// Évaluation heuristique statique de la position
    fn evaluate(&self, my_side: u8) -> i32 {
        let opp_side = if my_side == 1 { 2 } else { 1 };

        // 1. Victoire immédiate par sortie de Sentinelle
        if self.exited_sentinel[my_side as usize] {
            return WIN_SCORE;
        }
        if self.exited_sentinel[opp_side as usize] {
            return -WIN_SCORE;
        }

        let mut score: i32 = 0;

        for p in self.pieces.iter().filter(|p| p.alive) {
            let y = (p.sq / 10) as i32;
            let progress = if p.owner == 1 { y } else { 9 - y };
            let h = self.heights[p.sq as usize] as i32;

            let val = match p.kind {
                2 => progress * 80 + h * 15 + 400, // Sentinel
                1 => progress * 40 + h * 12 + 250, // Vanguard
                _ => progress * 20 + h * 8 + 100,  // Explorer
            };

            if p.owner == my_side {
                score += val;
            } else {
                score -= val;
            }
        }

        score
    }
}

/// Ordonnance les coups pour maximiser les coupures de branches Alpha-Bêta
fn order_moves(turn: &Turn, move_indices: &mut [usize]) {
    move_indices.sort_unstable_by_key(|&idx| {
        let m = &turn.moves[idx];
        let mut priority: i32 = 0;

        if m.kind == MoveKind::Exit {
            priority += 100_000;
        } else if m.kind == MoveKind::Attack {
            priority += 50_000 + (m.attack as i32 - m.defense as i32) * 500;
        } else {
            priority -= (m.cost as i32) * 10;
        }
        -priority // Tri décroissant
    });
}

/// Évaluation simplifiée à profondeur 2 (recherche des contre-attaques immédiates)
fn search_depth2(turn: &Turn, root_move_idx: usize, my_side: u8, alpha: i32, beta: i32) -> i32 {
    let m = &turn.moves[root_move_idx];
    let mut board = SimBoard::from_turn(turn);

    // Simulation du coup racine
    if m.kind == MoveKind::Exit {
        let p_idx = m.piece as usize;
        if p_idx < board.pieces.len() {
            board.pieces[p_idx].alive = false;
            board.total_exits += 1;
            if board.pieces[p_idx].kind == 2 && board.total_exits == 1 {
                board.exited_sentinel[my_side as usize] = true;
                return WIN_SCORE;
            }
        }
    } else if m.to != 255 {
        let p_idx = m.piece as usize;
        let target_sq = m.to;

        // Élimination éventuelle d'une pièce ennemie sur la case
        for enemy in board.pieces.iter_mut().filter(|p| p.alive && p.owner != my_side) {
            if enemy.sq == target_sq {
                enemy.alive = false;
                break;
            }
        }

        if p_idx < board.pieces.len() {
            board.pieces[p_idx].sq = target_sq;
        }
    }

    // Évaluation après notre coup
    let base_eval = board.evaluate(my_side);

    // Vérification de la plus puissante riposte adverse (Min-Player)
    let opp_side = if my_side == 1 { 2 } else { 1 };
    let mut min_eval = base_eval;

    for enemy in board.pieces.iter().filter(|p| p.alive && p.owner == opp_side) {
        if fuel_left() < SAFETY_FUEL {
            break;
        }

        let ex = (enemy.sq % 10) as i32;
        let ey = (enemy.sq / 10) as i32;
        let eh = board.heights[enemy.sq as usize] as i32;
        let e_str = match enemy.kind { 2 => 3, 1 => 2, _ => 1 };

        // Test d'attaque sur nos pièces adjacentes
        for my_p in board.pieces.iter().filter(|p| p.alive && p.owner == my_side) {
            let mx = (my_p.sq % 10) as i32;
            let my = (my_p.sq / 10) as i32;
            let mh = board.heights[my_p.sq as usize] as i32;

            if (ex - mx).abs() + (ey - my).abs() == 1 {
                let eff_att = e_str + if eh > mh { 1 } else { 0 };
                let my_def = match my_p.kind { 2 => 3, 1 => 2, _ => 1 };

                // Si l'adversaire peut capturer
                if eff_att >= my_def {
                    let loss_val = match my_p.kind {
                        2 => 450, // Perte fatale de sentinelle
                        1 => 260,
                        _ => 110,
                    };
                    let countered_eval = base_eval - loss_val;
                    if countered_eval < min_eval {
                        min_eval = countered_eval;
                    }
                    if min_eval <= alpha {
                        return min_eval; // Coupure Alpha
                    }
                }
            }
        }
    }

    min_eval
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let my_side = turn.state.to_move;

    // Détection immédiate d'une victoire par Sentinelle
    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p_idx = m.piece as usize;
            let pieces = turn.state.pieces();
            if p_idx < pieces.len() && pieces[p_idx].kind as u8 == 2 {
                return i;
            }
        }
    }

    let mut move_indices: Vec<usize> = (0..turn.moves.len()).collect();
    order_moves(turn, &mut move_indices);

    let mut best_move = move_indices[0];
    let mut best_score = -INF;
    let mut alpha = -INF;
    let beta = INF;

    // Exploration Alpha-Bêta avec contrôle continu du fuel
    for &idx in &move_indices {
        if fuel_left() < SAFETY_FUEL {
            break;
        }

        let score = search_depth2(turn, idx, my_side, alpha, beta);

        if score > best_score {
            best_score = score;
            best_move = idx;
        }

        if score > alpha {
            alpha = score;
        }
    }

    best_move
}

bot!(choose);
