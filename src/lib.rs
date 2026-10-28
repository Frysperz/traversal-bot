use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

// --- SYSTÈME DE POIDS STRATÉGIQUES ---
const SCORE_SENTINEL_WIN: i32 = 1_000_000; // Priorité absolue : victoire immédiate
const SCORE_OTHER_EXIT: i32    = -10_000;   // Pénalité : on évite de sortir une autre pièce en premier

const WEIGHT_SENTINEL_ADVANCE: i32 = 120;   // La Sentinelle doit foncer vers l'avant
const WEIGHT_VANGUARD_ATTACK: i32  = 800;   // L'Avant-garde menace et élimine les pièces adverses
const WEIGHT_VANGUARD_ADVANCE: i32 = 50;
const WEIGHT_EXPLORER_ADVANCE: i32 = 30;

const WEIGHT_HEIGHT_ADVANTAGE: i32 = 25;    // Bonus pour le contrôle des crêtes et collines
const WEIGHT_ESCORT_SENTINEL: i32  = 70;    // Bonus pour les Explorateurs proches de la Sentinelle

fn evaluate_move(turn: &Turn, move_idx: usize) -> i32 {
    let m = &turn.moves[move_idx];
    let state = &turn.state;

    // Récupération de la pièce qui joue le coup
    let piece_idx = m.piece as usize;
    if piece_idx >= state.pieces.len() {
        return 0;
    }
    let piece = &state.pieces[piece_idx];
    let piece_kind = piece.kind as u8; // 0: Explorer, 1: Vanguard, 2: Sentinel

    // 1. CONDITION DE SORTIE (PRIORITÉ ABSOLUE SENTINELLE)
    if m.kind == MoveKind::Exit {
        if piece_kind == 2 {
            return SCORE_SENTINEL_WIN; // Une Sentinelle qui sort gagne la partie sur le champ !
        } else {
            // Sortir un Explorateur ou un Avant-garde en premier ferait rater la victoire instantanée
            return SCORE_OTHER_EXIT;
        }
    }

    let mut score: i32 = 0;

    // 2. ATTAQUES & MENACES PAR LES AVANT-GARDES (VANGUARDS)
    if m.kind == MoveKind::Attack {
        if piece_kind == 1 {
            // Rôle offensif principal : bonus massif + avantage aux attaques nettes
            score += WEIGHT_VANGUARD_ATTACK + (m.attack as i32 - m.defense as i32) * 50;
        } else if piece_kind == 2 {
            // La Sentinelle attaque si nécessaire pour se frayer un chemin
            score += 300;
        } else {
            // L'Explorateur n'attaque que s'il est sûr de gagner
            score += 100;
        }
    }

    // 3. DÉPLACEMENT, RELIEF ET POSITIONNEMENT
    if m.target != 255 {
        let target_case = m.target as usize;
        let target_x = (target_case % 10) as i32;
        let target_y = (target_case / 10) as i32;

        // Calcul de l'avancée vers le camp adverse
        // Camp 1 (Rouge) avance vers y=9 ; Camp 2 (Bleu) avance vers y=0
        let forward_progress = if state.side_to_move == 1 {
            target_y
        } else {
            9 - target_y
        };

        match piece_kind {
            2 => {
                // Sentinelle : avancée prioritaire
                score += forward_progress * WEIGHT_SENTINEL_ADVANCE;
            }
            1 => {
                // Avant-garde : progression intermédiaire pour créer des zones de menace
                score += forward_progress * WEIGHT_VANGUARD_ADVANCE;
            }
            0 => {
                // Explorateur : escorte et protection des Sentinelles
                score += forward_progress * WEIGHT_EXPLORER_ADVANCE;

                // On cherche la Sentinelle alliée pour rester à son contact
                for other in state.pieces.iter() {
                    if (other.kind as u8) == 2 && other.side == state.side_to_move {
                        let sent_case = other.square as usize;
                        let sent_x = (sent_case % 10) as i32;
                        let sent_y = (sent_case / 10) as i32;

                        let dist = (target_x - sent_x).abs() + (target_y - sent_y).abs();
                        // Plus l'explorateur est proche (distance de 1 ou 2 cases), plus le score monte
                        if dist <= 2 {
                            score += (3 - dist) * WEIGHT_ESCORT_SENTINEL;
                        }
                    }
                }
            }
            _ => {}
        }

        // 4. CONTRÔLE DE LA HAUTEUR
        // Être en altitude offre +1 d'attaque et protège des charges descendantes
        if target_case < state.height.len() {
            let height = state.height[target_case] as i32;
            score += height * WEIGHT_HEIGHT_ADVANTAGE;
        }
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
