#[path = "../nnue.rs"]
mod nnue;

#[path = "../engine.rs"]
mod engine;

use engine::{FastBoard, FastMove, FastMoveKind, FastPiece, MAX_MOVES, NO_PIECE};
use nnue::get_feature_index;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

fn setup_initial_board() -> FastBoard {
    let mut board = FastBoard::new();

    // Relief symétrique classique de Traversal
    for sq in 0..100 {
        let x = (sq % 10) as i32;
        let y = (sq / 10) as i32;
        let dist_center = ((x - 4).abs().max((x - 5).abs()) + (y - 4).abs().max((y - 5).abs())) as u8;
        board.heights[sq] = 4u8.saturating_sub(dist_center / 2);
    }

    // Camp 1 (Rouge) : rangée y=0 et y=1
    let mut idx = 0;
    // Sentinelle en (4, 0)
    board.pieces[idx] = FastPiece { sq: 4, kind: 2, owner: 1, alive: true };
    board.grid[4] = idx as u8;
    idx += 1;
    // Avant-gardes en (3, 0) et (5, 0)
    board.pieces[idx] = FastPiece { sq: 3, kind: 1, owner: 1, alive: true };
    board.grid[3] = idx as u8;
    idx += 1;
    board.pieces[idx] = FastPiece { sq: 5, kind: 1, owner: 1, alive: true };
    board.grid[5] = idx as u8;
    idx += 1;
    // Explorateurs
    for &sq in &[12, 14, 16] {
        board.pieces[idx] = FastPiece { sq, kind: 0, owner: 1, alive: true };
        board.grid[sq as usize] = idx as u8;
        idx += 1;
    }

    // Camp 2 (Bleu) : rangée y=9 et y=8
    // Sentinelle en (5, 9)
    board.pieces[idx] = FastPiece { sq: 95, kind: 2, owner: 2, alive: true };
    board.grid[95] = idx as u8;
    idx += 1;
    // Avant-gardes en (4, 9) et (6, 9)
    board.pieces[idx] = FastPiece { sq: 94, kind: 1, owner: 2, alive: true };
    board.grid[94] = idx as u8;
    idx += 1;
    board.pieces[idx] = FastPiece { sq: 96, kind: 1, owner: 2, alive: true };
    board.grid[96] = idx as u8;
    idx += 1;
    // Explorateurs
    for &sq in &[83, 85, 87] {
        board.pieces[idx] = FastPiece { sq, kind: 0, owner: 2, alive: true };
        board.grid[sq as usize] = idx as u8;
        idx += 1;
    }

    board.piece_count = idx;
    board.to_move = 1;
    board.init_accumulator();
    board
}

fn main() {
    let total_games = 5000;
    println!("Démarrage du Self-Play pour {} parties...", total_games);
    let start_time = Instant::now();

    let file = File::create("dataset.jsonl").expect("Impossible de créer dataset.jsonl");
    let mut writer = BufWriter::new(file);

    let mut red_wins = 0;
    let mut blue_wins = 0;

    let mut move_buffer = [FastMove {
        piece_idx: 0,
        from: 0,
        to: 0,
        kind: FastMoveKind::Move,
        cost: 0,
        captured_piece_idx: NO_PIECE,
    }; MAX_MOVES];

    for game_id in 0..total_games {
        let mut board = setup_initial_board();
        let mut history: Vec<(Vec<u16>, u8)> = Vec::with_capacity(80);
        let mut winner = 0u8;

        for _ply in 0..120 {
            if board.sentinel_exited[1] {
                winner = 1;
                break;
            }
            if board.sentinel_exited[2] {
                winner = 2;
                break;
            }

            let move_count = board.generate_moves(&mut move_buffer);
            if move_count == 0 {
                winner = if board.to_move == 1 { 2 } else { 1 };
                break;
            }

            let mut active_features = Vec::new();
            for i in 0..board.piece_count {
                let p = &board.pieces[i];
                if p.alive && p.sq != 255 {
                    active_features.push(get_feature_index(p.sq, p.kind, p.owner) as u16);
                }
            }
            history.push((active_features, board.to_move));

            let chosen_idx = if game_id % 5 == 0 {
                (game_id + _ply) % move_count
            } else {
                let mut best = 0;
                let mut best_val = -9999;
                for m_idx in 0..move_count {
                    let m = &move_buffer[m_idx];
                    let mut val = 0;
                    if m.kind == FastMoveKind::Exit {
                        val += 5000;
                    } else if m.kind == FastMoveKind::Attack {
                        val += 200;
                    } else if m.to != 255 {
                        let y = (m.to / 10) as i32;
                        val += if board.to_move == 1 { y * 10 } else { (9 - y) * 10 };
                    }
                    if val > best_val {
                        best_val = val;
                        best = m_idx;
                    }
                }
                best
            };

            board.apply_move(move_buffer[chosen_idx]);
        }

        if winner == 1 {
            red_wins += 1;
        } else if winner == 2 {
            blue_wins += 1;
        }

        for (feats, side) in history {
            let target_result: f32 = if winner == 0 {
                0.0
            } else if winner == side {
                1.0
            } else {
                -1.0
            };

            let feats_str: Vec<String> = feats.iter().map(|f| f.to_string()).collect();
            let line = format!("{{\"features\":[{}],\"result\":{:.1}}}\n", feats_str.join(","), target_result);
            writer.write_all(line.as_bytes()).unwrap();
        }

        if (game_id + 1) % 1000 == 0 {
            println!(
                "Partie {}/{} terminée (Rouge: {}, Bleu: {})",
                game_id + 1,
                total_games,
                red_wins,
                blue_wins
            );
        }
    }

    println!(
        "Self-Play achevé en {:.2?}s. Dataset écrit dans dataset.jsonl",
        start_time.elapsed()
    );
}
