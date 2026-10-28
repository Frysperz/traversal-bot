use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

// --- SYSTÈME DE POIDS SPÉCIFIQUE ANTI-RUSH ---
const SCORE_SENTINEL_WIN: i32       = 2_000_000; // Priorité absolue : notre Sentinelle sort
const WEIGHT_BLOCK_ENEMY_SENT: i32  = 5_000;     // Interception d'urgence de la Sentinelle adverse
const WEIGHT_ATTACK_ENEMY_SENT: i32 = 8_000;     // Destruction de la Sentinelle adverse
const WEIGHT_OUR_SENTINEL_RUSH: i32 = 350;       // Poussée agressive de notre Sentinelle
const WEIGHT_ATTACK: i32            = 1_200;
const WEIGHT_ADVANCE: i32           = 40;
const WEIGHT_HEIGHT: i32            = 25;

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let state = &turn.state;
    let pieces = state.pieces();
    let my_side = state.to_move;
    let opp_side = if my_side == 1 { 2 } else { 1 };

    // 1. DÉTECTION IMMÉDIATE D'UNE VICTOIRE (SORTIE SENTINELLE)
    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p_idx = m.piece as usize;
            if p_idx < pieces.len() && pieces[p_idx].kind as u8 == 2 {
                return i;
            }
        }
    }

    // 2. REPÉRER LA SENTINELLE ADVERSE
    let enemy_sentinel = pieces
        .iter()
        .find(|p| p.owner == opp_side && p.kind as u8 == 2);

    let mut best_idx = 0;
    let mut best_score = i32::MIN;

    for (i, m) in turn.moves.iter().enumerate() {
        let mut score: i32 = 0;
        let p_idx = m.piece as usize;
        let piece_kind = if p_idx < pieces.len() {
            pieces[p_idx].kind as u8
        } else {
            0
        };

        // A. Sortie normale
        if m.kind == MoveKind::Exit {
            score += 15_000;
        }

        // B. Combat et élimination
        if m.kind == MoveKind::Attack {
            score += WEIGHT_ATTACK + (m.attack as i32 - m.defense as i32) * 100;

            // Bonus critique si la cible attaquée est la Sentinelle adverse
            if let Some(e_sent) = enemy_sentinel {
                if m.to == e_sent.sq {
                    score += WEIGHT_ATTACK_ENEMY_SENT;
                }
            }
        }

        // C. Positionnement géométrique et blocage
        if m.to != 255 {
            let target_sq = m.to as usize;
            let target_x = (target_sq % 10) as i32;
            let target_y = (target_sq / 10) as i32;

            // Progression vers l'avant (Camp 1 vers y=9, Camp 2 vers y=0)
            let forward_progress = if my_side == 1 { target_y } else { 9 - target_y };

            // Rush de notre Sentinelle
            if piece_kind == 2 {
                score += forward_progress * WEIGHT_OUR_SENTINEL_RUSH;
            } else {
                score += forward_progress * WEIGHT_ADVANCE;
            }

            // Prise de hauteur
            if target_sq < state.heights.len() {
                score += (state.heights[target_sq] as i32) * WEIGHT_HEIGHT;
            }

            // Économie de déplacement
            score -= (m.cost as i32) * 15;

            // LOGIQUE D'INTERCEPTION DE LA SENTINELLE DE SOMMET :
            if let Some(e_sent) = enemy_sentinel {
                let e_x = (e_sent.sq % 10) as i32;
                let e_y = (e_sent.sq / 10) as i32;

                // Calcul du danger : jusqu'où la Sentinelle adverse a-t-elle avancé ?
                let enemy_advancement = if opp_side == 1 { e_y } else { 9 - e_y };

                // Si la Sentinelle adverse s'approche dangereusement (au-delà de la ligne médiane)
                if enemy_advancement >= 4 {
                    let dist = (target_x - e_x).abs() + (target_y - e_y).abs();

                    // Se placer sur son chemin immédiat (distance 1 ou 2)
                    if dist <= 2 {
                        score += (3 - dist) * WEIGHT_BLOCK_ENEMY_SENT;

                        // Si nous occupons une case plus haute que sa Sentinelle, c'est un barrage impénétrable
                        let my_h = state.heights[target_sq] as i32;
                        let enemy_h = state.heights[e_sent.sq as usize] as i32;
                        if my_h > enemy_h {
                            score += 2_000;
                        }
                    }
                }
            }
        }

        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    best_idx
}

bot!(choose);
