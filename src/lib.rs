use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};

const SAFETY_FUEL: u64 = 150_000_000;
const INF: i32 = 2_000_000;
const SENTINEL_WIN: i32 = 1_000_000;
const MAX_DEPTH: usize = 14;

// Table de transposition calibrée sous 300 Ko
const TT_SIZE: usize = 1 << 14;
const TT_MASK: usize = TT_SIZE - 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TTFlag {
    None,
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy)]
struct TTEntry {
    key: u64,
    depth: u8,
    score: i32,
    flag: TTFlag,
    best_move_sq: u8,
}

static mut TT: [TTEntry; TT_SIZE] = [TTEntry {
    key: 0,
    depth: 0,
    score: 0,
    flag: TTFlag::None,
    best_move_sq: 255,
}; TT_SIZE];

fn zobrist_hash(grid: &[u8; 100], to_move: u8) -> u64 {
    let mut h: u64 = if to_move == 1 { 0x9e3779b97f4a7c15 } else { 0xc6a4a7935bd1e995 };
    for (sq, &p_val) in grid.iter().enumerate() {
        if p_val != 255 {
            let mut val = (sq as u64) ^ ((p_val as u64) << 8);
            val = val.wrapping_mul(0xbf58476d1ce4e5b9);
            val ^= val >> 30;
            val = val.wrapping_mul(0x94d049bb133111eb);
            val ^= val >> 27;
            h ^= val;
        }
    }
    h
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SimMoveKind {
    Move,
    Attack,
    Exit,
}

#[derive(Clone, Copy, Debug)]
pub struct SimMove {
    pub piece_idx: u8,
    pub from: u8,
    pub to: u8,
    pub kind: SimMoveKind,
    pub cost: u8,
    pub captured_idx: u8,
}

#[derive(Clone, Copy)]
pub struct BoardState {
    pub pieces_sq: [u8; 16],
    pub pieces_kind: [u8; 16],
    pub pieces_owner: [u8; 16],
    pub pieces_alive: [bool; 16],
    pub piece_count: usize,
    pub grid: [u8; 100],
    pub heights: [u8; 100],
    pub to_move: u8,
    pub sentinel_exited: [bool; 3],
}

fn quiescence(
    board: &mut BoardState,
    mut alpha: i32,
    beta: i32,
    aborted: &mut bool,
) -> i32 {
    if fuel_left() < SAFETY_FUEL {
        *aborted = true;
        return 0;
    }

    let stand_pat = board.evaluate(board.to_move);
    if stand_pat >= beta {
        return beta;
    }
    alpha = alpha.max(stand_pat);

    // Génère seulement les captures
    let mut captures = [SimMove::default(); 64];
    let cap_count = board.generate_captures(&mut captures);

    // Trie les captures (MVV/LVA: Most Valuable Victim / Least Valuable Attacker)
    order_captures(&mut captures, cap_count);

    for i in 0..cap_count {
        let m = captures[i];
        let (prev_sq, captured, exited) = board.apply(m);

        let score = -quiescence(board, -beta, -alpha, aborted);

        board.unmake(m, prev_sq, captured, exited);

        if *aborted {
            return 0;
        }

        if score >= beta {
            return beta;
        }
        alpha = alpha.max(score);
    }

    alpha
}

impl BoardState {
    #[inline(always)]
    fn apply(&mut self, m: SimMove) -> (u8, u8, bool) {
        let p = m.piece_idx as usize;
        let prev_sq = self.pieces_sq[p];
        let mut captured = 255;
        let mut exited = false;

        match m.kind {
            SimMoveKind::Exit => {
                self.grid[m.from as usize] = 255;
                self.pieces_sq[p] = 255;
                self.pieces_alive[p] = false;
                exited = true;
                if self.pieces_kind[p] == 2 {
                    self.sentinel_exited[self.to_move as usize] = true;
                }
            }
            SimMoveKind::Attack => {
                let target = m.to as usize;
                captured = self.grid[target];
                if captured != 255 {
                    self.pieces_alive[captured as usize] = false;
                    self.pieces_sq[captured as usize] = 255;
                }
                self.grid[m.from as usize] = 255;
                self.grid[target] = m.piece_idx;
                self.pieces_sq[p] = m.to;
            }
            SimMoveKind::Move => {
                self.grid[m.from as usize] = 255;
                self.grid[m.to as usize] = m.piece_idx;
                self.pieces_sq[p] = m.to;
            }
        }

        self.to_move = if self.to_move == 1 { 2 } else { 1 };
        (prev_sq, captured, exited)
    }

    #[inline(always)]
    fn unmake(&mut self, m: SimMove, prev_sq: u8, captured: u8, exited: bool) {
        self.to_move = if self.to_move == 1 { 2 } else { 1 };
        let p = m.piece_idx as usize;

        if exited {
            self.pieces_alive[p] = true;
            self.pieces_sq[p] = prev_sq;
            self.grid[prev_sq as usize] = m.piece_idx;
            if self.pieces_kind[p] == 2 {
                self.sentinel_exited[self.to_move as usize] = false;
            }
        } else {
            self.grid[m.to as usize] = 255;
            self.grid[prev_sq as usize] = m.piece_idx;
            self.pieces_sq[p] = prev_sq;

            if captured != 255 {
                let cap = captured as usize;
                self.pieces_alive[cap] = true;
                self.pieces_sq[cap] = m.to;
                self.grid[m.to as usize] = captured;
            }
        }
    }

    fn generate_moves(&self, buffer: &mut [SimMove; 64]) -> usize {
        let mut count = 0;
        let opp = if self.to_move == 1 { 2 } else { 1 };
        let exit_row = if self.to_move == 1 { 9 } else { 0 };

        for i in 0..self.piece_count {
            if !self.pieces_alive[i] || self.pieces_owner[i] != self.to_move {
                continue;
            }

            let sq = self.pieces_sq[i] as usize;
            let x = (sq % 10) as i32;
            let y = (sq / 10) as i32;
            let ch = self.heights[sq] as i32;

            if y == exit_row && count < 64 {
                buffer[count] = SimMove {
                    piece_idx: i as u8,
                    from: sq as u8,
                    to: 255,
                    kind: SimMoveKind::Exit,
                    cost: 1,
                    captured_idx: 255,
                };
                count += 1;
            }

            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x + dx;
                let ny = y + dy;

                if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                    let nsq = (ny * 10 + nx) as usize;
                    let occ = self.grid[nsq];
                    let nh = self.heights[nsq] as i32;

                    if occ == 255 {
                        let cost = (1 + (nh - ch).max(0)) as u8;
                        if count < 64 {
                            buffer[count] = SimMove {
                                piece_idx: i as u8,
                                from: sq as u8,
                                to: nsq as u8,
                                kind: SimMoveKind::Move,
                                cost,
                                captured_idx: 255,
                            };
                            count += 1;
                        }
                    } else if self.pieces_owner[occ as usize] == opp {
                        let att = match self.pieces_kind[i] { 2 => 3, 1 => 2, _ => 1 } + if ch > nh { 1 } else { 0 };
                        let def = match self.pieces_kind[occ as usize] { 2 => 3, 1 => 2, _ => 1 } + if nh > ch { 1 } else { 0 };

                        if att >= def && count < 64 {
                            buffer[count] = SimMove {
                                piece_idx: i as u8,
                                from: sq as u8,
                                to: nsq as u8,
                                kind: SimMoveKind::Attack,
                                cost: 2,
                                captured_idx: occ,
                            };
                            count += 1;
                        }
                    }
                }
            }
        }
        count
    }

    #[inline(always)]
    // Dans BoardState::evaluate()
    fn evaluate(&self, side: u8) -> i32 {
        let opp = 3 - side;
    
        // 1. Vérifie victoire/défaite instantanée Sentinel
        if self.sentinel_exited[side as usize] {
            return 1_000_000; // Victoire
        }
        if self.sentinel_exited[opp as usize] {
            return -1_000_000; // Défaite
        }
    
        // 2. Vérifie si un Sentinel adverse est en position de victoire
        let mut score = 0;
        if self.is_sentinel_threatened(opp) {
            score -= 50_000; // URGENT: bloquer !
        }
    
        // 3. Évalue chaque pièce
        for i in 0..self.piece_count {
            if !self.pieces_alive[i] { continue; }
    
            let p = &self.pieces[i];
            let sq = p.sq as usize;
            let x = sq % 10;
            let y = sq / 10;
            let h = self.heights[sq] as i32;
    
            // Distance à la sortie
            let dist_to_exit = if p.owner == 1 {
                9 - y // Rouge → ligne 9 (index 90-99)
            } else {
                y // Bleu → ligne 0 (index 0-9)
            };
    
            // Valeur de base (selon le type de pièce)
            let base_value = match p.kind {
                2 => 3_000, // Sentinel = 3 pts
                1 => 2_000, // Vanguard = 2 pts
                _ => 1_000, // Explorer = 1 pt
            };
    
            // Bonus de progression (plus important pour les pièces lentes)
            let progress_bonus = dist_to_exit * match p.kind {
                2 => 300, // Sentinel lent → bonus élevé
                1 => 200,
                _ => 100,
            };
    
            // Bonus de hauteur (contrôle du terrain)
            let height_bonus = h * 100;
    
            // Bonus si la pièce bloque un Sentinel adverse
            let blocking_bonus = self.count_blocked_sentinels(p) * 1_000;
    
            // Bonus de mobilité (nombre de moves possibles)
            let mobility = self.count_legal_moves_for_piece(i) as i32 * 50;
    
            let piece_score = base_value + progress_bonus + height_bonus + blocking_bonus + mobility;
    
            if p.owner == side {
                score += piece_score;
            } else {
                score -= piece_score;
            }
        }
    
        // 4. Bonus de contrôle global des hauteurs
        score += self.evaluate_height_control(side) * 30;
    
        // 5. Pénalité si nos pièces sont isolées
        score -= self.evaluate_isolation(side) * 200;
    
        score
    }
    
    // Ajoute ces méthodes à BoardState:
    fn is_sentinel_threatened(&self, side: u8) -> bool {
        for i in 0..self.piece_count {
            if !self.pieces_alive[i] || self.pieces_owner[i] != side {
                continue;
            }
            let p = &self.pieces[i];
            if p.kind != 2 { continue; } // Seuls les Sentinels comptent
    
            let sq = p.sq as usize;
            let y = sq / 10;
            let exit_row = if side == 1 { 9 } else { 0 };
    
            // Si le Sentinel est à 1 case de la sortie
            if (y as i32).abs_diff(exit_row as i32) == 1 {
                // Vérifie si un ennemi peut le bloquer ou le capturer
                return true;
            }
        }
        false
    }
    
    fn count_blocked_sentinels(&self, p: &Piece) -> i32 {
        // Compte combien de Sentinels adverses sont bloqués par cette pièce
        let mut count = 0;
        let opp = 3 - p.owner;
        for i in 0..self.piece_count {
            if !self.pieces_alive[i] || self.pieces_owner[i] != opp || self.pieces_kind[i] != 2 {
                continue;
            }
            let sent_sq = self.pieces_sq[i] as usize;
            let p_sq = p.sq as usize;
            // Si la pièce est adjacente au Sentinel adverse
            if (p_sq % 10).abs_diff(sent_sq % 10) <= 1 && (p_sq / 10).abs_diff(sent_sq / 10) <= 1 {
                count += 1;
            }
        }
        count
    }

    fn generate_captures(&self, buffer: &mut [SimMove; 64]) -> usize {
        let mut count = 0;
        let opp = if self.to_move == 1 { 2 } else { 1 };
    
        for i in 0..self.piece_count {
            if !self.pieces_alive[i] || self.pieces_owner[i] != self.to_move {
                continue;
            }
    
            let sq = self.pieces_sq[i] as usize;
            let x = (sq % 10) as i32;
            let y = (sq / 10) as i32;
            let ch = self.heights[sq] as i32;
    
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x + dx;
                let ny = y + dy;
    
                if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                    let nsq = (ny * 10 + nx) as usize;
                    let target = self.grid[nsq];
                    if target != 255 && self.pieces_owner[target as usize] == opp {
                        let att_str = match self.pieces_kind[i] {
                            2 => 3,
                            1 => 2,
                            _ => 1,
                        } + if ch > self.heights[nsq] as i32 { 1 } else { 0 };
    
                        let def_str = match self.pieces_kind[target as usize] {
                            2 => 3,
                            1 => 2,
                            _ => 1,
                        } + if self.heights[nsq] as i32 > ch { 1 } else { 0 };
    
                        if att_str >= def_str && count < 64 {
                            buffer[count] = SimMove {
                                piece_idx: i as u8,
                                from: sq as u8,
                                to: nsq as u8,
                                kind: SimMoveKind::Attack,
                                cost: 2,
                                captured_idx: target,
                            };
                            count += 1;
                        }
                    }
                }
            }
        }
        count
    }
    
    fn order_captures(moves: &mut [SimMove; 64], count: usize) {
        moves[..count].sort_unstable_by(|a, b| {
            // MVV/LVA: Priorité aux captures de pièces fortes avec des pièces faibles
            let a_val = match a.captured_idx {
                255 => 0,
                idx => match moves[idx as usize].kind {
                    2 => 3,
                    1 => 2,
                    _ => 1,
                },
            };
            let b_val = match b.captured_idx {
                255 => 0,
                idx => match moves[idx as usize].kind {
                    2 => 3,
                    1 => 2,
                    _ => 1,
                },
            };
            b_val.cmp(&a_val)
        });
    }
}

fn order_moves(moves: &mut [SimMove], count: usize, tt_move_sq: u8) {
    let slice = &mut moves[..count];
    slice.sort_unstable_by_key(|m| {
        let mut p = 0;
        if m.to == tt_move_sq && tt_move_sq != 255 {
            p += 1_000_000;
        }
        if m.kind == SimMoveKind::Exit {
            p += 500_000;
        } else if m.kind == SimMoveKind::Attack {
            p += 100_000;
        } else {
            p -= (m.cost as i32) * 50;
        }
        -p
    });
}

fn negamax(
    board: &mut BoardState,
    depth: usize,
    mut alpha: i32,
    beta: i32,
    aborted: &mut bool,
) -> i32 {
    if fuel_left() < SAFETY_FUEL {
        *aborted = true;
        return 0;
    }

    let alpha_orig = alpha;
    let hash = zobrist_hash(&board.grid, board.to_move);
    let tt_idx = (hash as usize) & TT_MASK;
    let mut tt_move_sq = 255;

    unsafe {
        let entry = &TT[tt_idx];
        if entry.flag != TTFlag::None && entry.key == hash && entry.depth >= (depth as u8) {
            match entry.flag {
                TTFlag::Exact => return entry.score,
                TTFlag::LowerBound => alpha = alpha.max(entry.score),
                TTFlag::UpperBound => {
                    if entry.score <= alpha {
                        return entry.score;
                    }
                }
                TTFlag::None => {}
            }
            if alpha >= beta {
                return entry.score;
            }
            tt_move_sq = entry.best_move_sq;
        }
    }

    if board.sentinel_exited[1] || board.sentinel_exited[2] || depth == 0 {
        return board.evaluate(board.to_move);
    }

    let mut moves = [SimMove {
        piece_idx: 0,
        from: 0,
        to: 0,
        kind: SimMoveKind::Move,
        cost: 0,
        captured_idx: 255,
    }; 64];

    let count = board.generate_moves(&mut moves);
    if count == 0 {
        return -SENTINEL_WIN + 50;
    }

    order_moves(&mut moves, count, tt_move_sq);

    let mut max_eval = -INF;
    let mut best_sq = 255;

    for i in 0..count {
        let m = moves[i];
        let (prev_sq, captured, exited) = board.apply(m);

        let eval = -negamax(board, depth - 1, -beta, -alpha, aborted);

        board.unmake(m, prev_sq, captured, exited);

        if *aborted {
            return 0;
        }

        if eval > max_eval {
            max_eval = eval;
            best_sq = m.to;
        }
        if eval > alpha {
            alpha = eval;
        }
        if alpha >= beta {
            break;
        }
    }

    unsafe {
        let flag = if max_eval <= alpha_orig {
            TTFlag::UpperBound
        } else if max_eval >= beta {
            TTFlag::LowerBound
        } else {
            TTFlag::Exact
        };

        TT[tt_idx] = TTEntry {
            key: hash,
            depth: depth as u8,
            score: max_eval,
            flag,
            best_move_sq: best_sq,
        };
    }

    max_eval
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    let state = &turn.state;
    let pieces = state.pieces();
    let my_side = state.to_move;

    for (i, m) in turn.moves.iter().enumerate() {
        if m.kind == MoveKind::Exit {
            let p = m.piece as usize;
            if p < pieces.len() && pieces[p].kind as u8 == 2 {
                return i;
            }
        }
    }

    let mut board = BoardState {
        pieces_sq: [255; 16],
        pieces_kind: [0; 16],
        pieces_owner: [0; 16],
        pieces_alive: [false; 16],
        piece_count: pieces.len().min(16),
        grid: [255; 100],
        heights: [0; 100],
        to_move: my_side,
        sentinel_exited: [false; 3],
    };

    for (i, &h) in state.heights.iter().enumerate().take(100) {
        board.heights[i] = h;
    }

    for (i, p) in pieces.iter().enumerate().take(board.piece_count) {
        board.pieces_sq[i] = p.sq;
        board.pieces_kind[i] = p.kind as u8;
        board.pieces_owner[i] = p.owner;
        board.pieces_alive[i] = p.sq != 255;
        if p.sq != 255 && (p.sq as usize) < 100 {
            board.grid[p.sq as usize] = i as u8;
        }
    }

    let mut root_moves: Vec<(usize, SimMove)> = turn
        .moves
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let k = match m.kind {
                MoveKind::Exit => SimMoveKind::Exit,
                MoveKind::Attack => SimMoveKind::Attack,
                _ => SimMoveKind::Move,
            };
            let cap = if m.to != 255 { board.grid[m.to as usize] } else { 255 };
            (
                i,
                SimMove {
                    piece_idx: m.piece,
                    from: pieces[m.piece as usize].sq,
                    to: m.to,
                    kind: k,
                    cost: m.cost,
                    captured_idx: cap,
                },
            )
        })
        .collect();

    root_moves.sort_by_key(|(_, m)| {
        let mut p = 0;
        if m.kind == SimMoveKind::Exit { p += 100_000; }
        else if m.kind == SimMoveKind::Attack { p += 20_000; }
        -p
    });

    let mut best_move_idx = root_moves[0].0;

    for depth in 1..=MAX_DEPTH {
        if fuel_left() < SAFETY_FUEL {
            break;
        }

        let mut depth_best = root_moves[0].0;
        let mut alpha = -INF;
        let beta = INF;
        let mut aborted = false;

        for &(orig_idx, m) in &root_moves {
            if fuel_left() < SAFETY_FUEL {
                aborted = true;
                break;
            }

            let (prev_sq, captured, exited) = board.apply(m);

            let score = -negamax(&mut board, depth - 1, -beta, -alpha, &mut aborted);

            board.unmake(m, prev_sq, captured, exited);

            if aborted {
                break;
            }

            if score > alpha {
                alpha = score;
                depth_best = orig_idx;
            }
        }

        if !aborted {
            best_move_idx = depth_best;
        } else {
            break;
        }
    }

    best_move_idx
}

bot!(choose);
