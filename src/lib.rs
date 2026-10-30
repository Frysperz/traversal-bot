use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

const SENTINEL_WIN: i32        = 10_000_000;
const SENTINEL_STEP: i32       = 500_000;
const BLOCK_ENEMY_SENT: i32    = 800_000;
const KILL_ENEMY_SENT: i32     = 2_000_000;
const NORMAL_ATTACK: i32       = 50_000;
const NORMAL_ADVANCE: i32      = 200;

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let state = &turn.state;
    let pieces = state.pieces();
    let my_side = state.to_move;
    let opp_side = if my_side == 1 { 2 } else { 1 };
    let exit_y = if my_side == 1 { 9 } else { 0 };

    // 1. CONDITION DE VICTOIRE IMMÉDIATE (SORTIE)
    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p_idx = m.piece as usize;
            if p_idx < pieces.len() && pieces[p_idx].kind as u8 == 2 {
                return i;
            }
        }
    }

    // Repérage des Sentinelles
    let my_sentinel = pieces.iter().find(|p| p.owner == my_side && p.kind as u8 == 2 && p.sq != 255);
    let opp_sentinel = pieces.iter().find(|p| p.owner == opp_side && p.kind as u8 == 2 && p.sq != 255);

    let mut best_idx = 0;
    let mut best_score = i32::MIN;

    for (i, m) in turn.moves.iter().enumerate() {
        let mut score: i32 = 0;
        let p_idx = m.piece as usize;
        let is_our_sentinel = if let Some(ms) = my_sentinel {
            p_idx < pieces.len() && pieces[p_idx].sq == ms.sq
        } else {
            false
        };
        let piece_kind = if p_idx < pieces.len() { pieces[p_idx].kind as u8 } else { 0 };

        // A. Sortie d'une pièce secondaire
        if m.kind == MoveKind::Exit {
            score += 20_000;
        }

        // B. Attaque
        if m.kind == MoveKind::Attack {
            let diff = (m.attack as i32) - (m.defense as i32);
            score += NORMAL_ATTACK + diff * 1_000;

            // Décapiter la sentinelle adverse anéantit immédiatement le rush de Sommet
            if let Some(es) = opp_sentinel {
                if m.to == es.sq {
                    score += KILL_ENEMY_SENT;
                }
            }
        }

        // C. Déplacements
        if m.to != 255 {
            let target_sq = m.to as usize;
            let tx = (target_sq % 10) as i32;
            let ty = (target_sq / 10) as i32;
            let target_h = state.heights[target_sq] as i32;

            let forward = if my_side == 1 { ty } else { 9 - ty };
            let dist_to_exit = (exit_y - ty).abs();

            // 1) NOTRE SENTINELLE : RUSH TOTAL ET DIRECT
            if is_our_sentinel {
                // Plus elle est proche de la sortie, plus le score explose
                score += (9 - dist_to_exit) * SENTINEL_STEP;
                score += target_h * 200; // Bonus de crête pour éviter les malus d'assaut

                // Vérification de sécurité : ne pas s'avancer sur une case adjacente à un ennemi qui surplombe
                for opp in pieces.iter().filter(|p| p.owner == opp_side && p.sq != 255) {
                    let ox = (opp.sq % 10) as i32;
                    let oy = (opp.sq / 10) as i32;
                    if (tx - ox).abs() + (ty - oy).abs() == 1 {
                        let opp_h = state.heights[opp.sq as usize] as i32;
                        let opp_kind = opp.kind as u8;
                        let opp_strength = match opp_kind { 2 => 3, 1 => 2, _ => 1 } + if opp_h > target_h { 1 } else { 0 };
                        if opp_strength >= 3 {
                            score -= 2_000_000; // Ne pas se donner bêtement
                        }
                    }
                }
            } else {
                // Pièces secondaires : avancée modérée
                score += forward * NORMAL_ADVANCE + target_h * 50;
            }

            // 2) CONTRE-RUSH : INTERCEPTION PHYSIQUE DE LA SENTINELLE DE SOMMET
            if let Some(es) = opp_sentinel {
                let ox = (es.sq % 10) as i32;
                let oy = (es.sq / 10) as i32;
                let opp_forward = if opp_side == 1 { oy } else { 9 - oy };

                // Si sa sentinelle a commencé sa course
                if opp_forward >= 1 {
                    let next_opp_y = if opp_side == 1 { oy + 1 } else { oy - 1 };

                    // Si notre pièce (Avant-garde ou Explorateur) bloque sa colonne
                    if tx == ox {
                        let is_in_path = if opp_side == 1 { ty > oy } else { ty < oy };
                        if is_in_path {
                            score += BLOCK_ENEMY_SENT;

                            // Bloquer la case immédiatement devant elle
                            if ty == next_opp_y {
                                score += BLOCK_ENEMY_SENT;
                                if target_h >= state.heights[es.sq as usize] as i32 {
                                    score += 400_000; // Mur impénétrable
                                }
                            }
                        }
                    }

                    // Pression au corps-à-corps par nos Vanguards (kind == 1)
                    if piece_kind == 1 && (tx - ox).abs() + (ty - oy).abs() == 1 {
                        score += 300_000;
                    }
                }
            }

            // Moins d'énergie dépensée
            score -= (m.cost as i32) * 40;
        }

        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    best_idx
}

bot!(choose);
