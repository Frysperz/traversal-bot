use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

const WIN_INSTANT: i32         = 5_000_000;
const PRIORITY_OUR_SPRINT: i32  = 25_000;
const PRIORITY_BLOCK_ENEMY: i32 = 40_000;
const PRIORITY_KILL_SENTINEL: i32 = 100_000;

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let state = &turn.state;
    let pieces = state.pieces();
    let my_side = state.to_move;
    let opp_side = if my_side == 1 { 2 } else { 1 };

    // 1. VICTOIRE IMMÉDIATE : Si notre Sentinelle peut sortir ce tour-ci, on gagne immédiatement
    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p_idx = m.piece as usize;
            if p_idx < pieces.len() && pieces[p_idx].kind as u8 == 2 {
                return i;
            }
        }
    }

    // Repérage des positions clés
    let our_sentinel = pieces.iter().find(|p| p.owner == my_side && p.kind as u8 == 2);
    let opp_sentinel = pieces.iter().find(|p| p.owner == opp_side && p.kind as u8 == 2);

    let mut best_idx = 0;
    let mut best_score = i32::MIN;

    for (i, m) in turn.moves.iter().enumerate() {
        let mut score: i32 = 0;
        let p_idx = m.piece as usize;
        let piece_kind = if p_idx < pieces.len() { pieces[p_idx].kind as u8 } else { 0 };

        // A. Sortie d'une autre pièce (Vanguard / Explorer)
        if m.kind == MoveKind::Exit {
            score += 10_000;
        }

        // B. Attaque directe
        if m.kind == MoveKind::Attack {
            score += 5_000 + (m.attack as i32 - m.defense as i32) * 200;

            // BONUS MAJEUR : Si on détruit la Sentinelle ennemie, son rush est anéanti !
            if let Some(e_sent) = opp_sentinel {
                if m.to == e_sent.sq {
                    score += PRIORITY_KILL_SENTINEL;
                }
            }
        }

        // C. Déplacements et positionnement
        if m.to != 255 {
            let target_sq = m.to as usize;
            let target_x = (target_sq % 10) as i32;
            let target_y = (target_sq / 10) as i32;

            // Avancée vers le camp opposé (Rouge va vers 9, Bleu va vers 0)
            let forward_progress = if my_side == 1 { target_y } else { 9 - target_y };

            // 1) NOTRE SENTINELLE DOIT FONCER TOUT DROIT
            if piece_kind == 2 {
                // Énorme incitation à sprinter vers la ligne de sortie
                score += forward_progress * PRIORITY_OUR_SPRINT;

                // Éviter les cases où un ennemi surélevé peut la décapiter
                for enemy in pieces.iter().filter(|p| p.owner == opp_side) {
                    let ex = (enemy.sq % 10) as i32;
                    let ey = (enemy.sq / 10) as i32;
                    if (target_x - ex).abs() + (target_y - ey).abs() == 1 {
                        let eh = state.heights[enemy.sq as usize] as i32;
                        let my_h = state.heights[target_sq] as i32;
                        if eh > my_h {
                            score -= 30_000; // Frein d'urgence si case piégée
                        }
                    }
                }
            } else {
                // Pour les autres pièces : progression normale
                score += forward_progress * 80;
            }

            // 2) BLOCUS DE LA SENTINELLE DE SOMMET
            if let Some(e_sent) = opp_sentinel {
                let e_x = (e_sent.sq % 10) as i32;
                let e_y = (e_sent.sq / 10) as i32;
                let e_advancement = if opp_side == 1 { e_y } else { 9 - e_y };

                // La Sentinelle ennemie se rapproche dangereusement (à 4 cases ou moins de la sortie)
                if e_advancement >= 5 {
                    // Calcul de la case que Sommet vise pour sortir (colonne e_x, rangée d'arrivée)
                    let exit_goal_y = if opp_side == 1 { 9 } else { 0 };
                    
                    // Si notre pièce se place sur la colonne de sa Sentinelle entre elle et son but
                    if target_x == e_x {
                        let is_between = if opp_side == 1 {
                            target_y >= e_y && target_y <= exit_goal_y
                        } else {
                            target_y <= e_y && target_y >= exit_goal_y
                        };

                        if is_between {
                            score += PRIORITY_BLOCK_ENEMY;
                        }
                    }

                    // Ou si on se colle à elle pour lui couper la route
                    let dist_to_enemy_sent = (target_x - e_x).abs() + (target_y - e_y).abs();
                    if dist_to_enemy_sent == 1 {
                        score += PRIORITY_BLOCK_ENEMY / 2;
                    }
                }
            }

            // Économiser le coût de mouvement
            score -= (m.cost as i32) * 10;
        }

        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    best_idx
}

bot!(choose);
