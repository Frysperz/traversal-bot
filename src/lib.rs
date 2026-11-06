use traversal_bot::core::{Move, MoveKind, PieceKind, Player};
use traversal_bot::{bot, fuel_left, Turn};

const SAFETY_FUEL: u64 = 150_000_000;
const MAX_DEPTH: u32 = 12;
const INF: i32 = 10_000_000;

// Scores pour l'évaluation
const SENTINEL_EXIT_WIN: i32 = 1_000_000;
const SENTINEL_THREAT: i32 = 50_000;
const SENTINEL_BLOCK: i32 = 40_000;
const SENTINEL_CAPTURE: i32 = 80_000;
const VANGUARD_VALUE: i32 = 2_000;
const EXPLORER_VALUE: i32 = 1_500;
const HEIGHT_BONUS: i32 = 100;
const PROGRESS_BONUS: i32 = 300;

#[derive(Clone, Copy, PartialEq, Eq)]
struct SimMove {
    piece_idx: u8,
    from: u8,
    to: u8,
    kind: MoveKind,
    cost: u8,
    captured_idx: u8,
}

impl Default for SimMove {
    fn default() -> Self {
        SimMove {
            piece_idx: 255,
            from: 255,
            to: 255,
            kind: MoveKind::Move,
            cost: 0,
            captured_idx: 255,
        }
    }
}

struct BoardState {
    pieces: [Option<Piece>; 16],
    grid: [u8; 100],
    heights: [u8; 100],
    to_move: Player,
    sentinel_exited: [bool; 2],
}

#[derive(Clone, Copy)]
struct Piece {
    idx: u8,
    kind: PieceKind,
    owner: Player,
    sq: u8,
}

impl BoardState {
    fn from_turn(turn: &Turn) -> Self {
        let mut pieces = [None; 16];
        let mut grid = [255; 100];
        let mut heights = [0; 100];

        for (i, piece) in turn.pieces.iter().enumerate() {
            if piece.alive {
                let p = Piece {
                    idx: i as u8,
                    kind: piece.kind,
                    owner: piece.owner,
                    sq: piece.square,
                };
                pieces[i] = Some(p);
                grid[piece.square as usize] = i as u8;
                heights[piece.square as usize] = piece.height;
            }
        }

        BoardState {
            pieces,
            grid,
            heights,
            to_move: turn.player,
            sentinel_exited: [false, false],
        }
    }

    fn apply(&mut self, m: SimMove) -> (u8, u8, bool) {
        let piece = self.pieces[m.piece_idx as usize].unwrap();
        let prev_sq = piece.sq;
        let mut captured = 255u8;
        let mut exited = false;

        match m.kind {
            MoveKind::Exit => {
                self.grid[prev_sq as usize] = 255;
                if let Some(p) = &mut self.pieces[m.piece_idx as usize] {
                    p.sq = 255;
                }
                exited = true;
                if piece.kind == PieceKind::Sentinel {
                    self.sentinel_exited[self.to_move as usize] = true;
                }
            }
            MoveKind::Attack => {
                let target_idx = self.grid[m.to as usize];
                if target_idx != 255 {
                    captured = target_idx;
                    if let Some(target_piece) = &mut self.pieces[target_idx as usize] {
                        target_piece.sq = 255;
                    }
                }
                self.grid[prev_sq as usize] = 255;
                self.grid[m.to as usize] = m.piece_idx;
                if let Some(p) = &mut self.pieces[m.piece_idx as usize] {
                    p.sq = m.to;
                }
            }
            MoveKind::Move | MoveKind::Pass => {
                self.grid[prev_sq as usize] = 255;
                self.grid[m.to as usize] = m.piece_idx;
                if let Some(p) = &mut self.pieces[m.piece_idx as usize] {
                    p.sq = m.to;
                }
            }
        }

        self.to_move = self.to_move.opponent();
        (prev_sq, captured, exited)
    }

    fn unmake(&mut self, m: SimMove, prev_sq: u8, captured: u8, exited: bool) {
        self.to_move = self.to_move.opponent();

        if exited {
            if let Some(p) = &mut self.pieces[m.piece_idx as usize] {
                p.sq = prev_sq;
            }
            self.grid[prev_sq as usize] = m.piece_idx;
            if let Some(piece) = self.pieces[m.piece_idx as usize] {
                if piece.kind == PieceKind::Sentinel {
                    self.sentinel_exited[self.to_move as usize] = false;
                }
            }
        } else {
            self.grid[m.to as usize] = 255;
            self.grid[prev_sq as usize] = m.piece_idx;
            if let Some(p) = &mut self.pieces[m.piece_idx as usize] {
                p.sq = prev_sq;
            }

            if captured != 255 {
                if let Some(cap_piece) = &mut self.pieces[captured as usize] {
                    cap_piece.sq = m.to;
                }
                self.grid[m.to as usize] = captured;
            }
        }
    }

    fn generate_moves(&self, buffer: &mut [SimMove; 64]) -> usize {
        let mut count = 0;
        let opp = self.to_move.opponent();
        let exit_row = if self.to_move == Player::Red { 9 } else { 0 };

        for i in 0..16 {
            if let Some(piece) = self.pieces[i] {
                if piece.owner != self.to_move {
                    continue;
                }

                let sq = piece.sq as usize;
                let x = sq % 10;
                let y = sq / 10;
                let h = self.heights[sq] as i32;

                // Exit move
                if y == exit_row && count < 64 {
                    buffer[count] = SimMove {
                        piece_idx: i as u8,
                        from: sq as u8,
                        to: 255,
                        kind: MoveKind::Exit,
                        cost: 1,
                        captured_idx: 255,
                    };
                    count += 1;
                }

                // Standard moves (orthogonal)
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                        let nsq = (ny * 10 + nx) as usize;
                        let occ = self.grid[nsq];
                        let nh = self.heights[nsq] as i32;

                        if occ == 255 {
                            let cost = (1 + (nh - h).max(0)) as u8;
                            if count < 64 {
                                buffer[count] = SimMove {
                                    piece_idx: i as u8,
                                    from: sq as u8,
                                    to: nsq as u8,
                                    kind: MoveKind::Move,
                                    cost,
                                    captured_idx: 255,
                                };
                                count += 1;
                            }
                        } else if self.pieces[occ as usize].unwrap().owner == opp {
                            let att = self.attack_value(piece.kind, h, nh);
                            let def = self.attack_value(
                                self.pieces[occ as usize].unwrap().kind,
                                nh,
                                h,
                            );
                            if att >= def && count < 64 {
                                buffer[count] = SimMove {
                                    piece_idx: i as u8,
                                    from: sq as u8,
                                    to: nsq as u8,
                                    kind: MoveKind::Attack,
                                    cost: 2,
                                    captured_idx: occ,
                                };
                                count += 1;
                            }
                        }
                    }
                }

                // Explorer special moves
                if piece.kind == PieceKind::Explorer {
                    // Diagonal moves (1 square)
                    for (dx, dy) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                            let nsq = (ny * 10 + nx) as usize;
                            if self.grid[nsq] == 255 && count < 64 {
                                buffer[count] = SimMove {
                                    piece_idx: i as u8,
                                    from: sq as u8,
                                    to: nsq as u8,
                                    kind: MoveKind::Move,
                                    cost: 1,
                                    captured_idx: 255,
                                };
                                count += 1;
                            }
                        }
                    }

                    // Leap moves (2 squares orthogonal)
                    for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                            let nsq = (ny * 10 + nx) as usize;
                            if self.grid[nsq] == 255 && count < 64 {
                                buffer[count] = SimMove {
                                    piece_idx: i as u8,
                                    from: sq as u8,
                                    to: nsq as u8,
                                    kind: MoveKind::Move,
                                    cost: 1,
                                    captured_idx: 255,
                                };
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        count
    }

    fn attack_value(&self, kind: PieceKind, h: i32, target_h: i32) -> i32 {
        let base = match kind {
            PieceKind::Sentinel => 3,
            PieceKind::Vanguard => 2,
            PieceKind::Explorer => 1,
        };
        base + if h > target_h { 1 } else { 0 }
    }

    fn evaluate(&self) -> i32 {
        let side = self.to_move;
        let opp = side.opponent();

        // 1. Check for instant win/loss via Sentinel exit
        if self.sentinel_exited[side as usize] {
            return -SENTINEL_EXIT_WIN;
        }
        if self.sentinel_exited[opp as usize] {
            return SENTINEL_EXIT_WIN;
        }

        let mut score = 0;

        // 2. Evaluate pieces
        for i in 0..16 {
            if let Some(piece) = self.pieces[i] {
                let value = match piece.kind {
                    PieceKind::Sentinel => 3_000,
                    PieceKind::Vanguard => VANGUARD_VALUE,
                    PieceKind::Explorer => EXPLORER_VALUE,
                };

                let sign = if piece.owner == side { 1 } else { -1 };
                score += value * sign;

                // Progress bonus (distance to exit)
                let y = (piece.sq / 10) as i32;
                let dist_to_exit = if piece.owner == Player::Red {
                    9 - y
                } else {
                    y
                };
                let progress = dist_to_exit * PROGRESS_BONUS;
                score += progress * sign * match piece.kind {
                    PieceKind::Sentinel => 3,
                    PieceKind::Vanguard => 2,
                    PieceKind::Explorer => 1,
                };

                // Height bonus
                let h = self.heights[piece.sq as usize] as i32;
                score += (h * HEIGHT_BONUS) * sign;

                // Bonus for being on exit row
                let exit_row = if piece.owner == Player::Red { 9 } else { 0 };
                if (piece.sq / 10) as i32 == exit_row {
                    score += 5_000 * sign;
                }
            }
        }

        // 3. Sentinel-specific bonuses
        for i in 0..16 {
            if let Some(piece) = self.pieces[i] {
                if piece.kind == PieceKind::Sentinel {
                    let exit_row = if piece.owner == Player::Red { 9 } else { 0 };
                    let y = (piece.sq / 10) as i32;
                    let dist = (exit_row - y).abs();

                    // Urgent: opponent sentinel is about to exit
                    if piece.owner == opp && dist <= 2 {
                        score -= SENTINEL_THREAT * (3 - dist);
                    }

                    // Our sentinel is close to exit
                    if piece.owner == side && dist <= 2 {
                        score += SENTINEL_THREAT * (3 - dist);
                    }
                }
            }
        }

        // 4. Control of center and high ground
        for sq in 0..100 {
            let y = sq / 10;
            let x = sq % 10;
            let h = self.heights[sq] as i32;

            // Center control bonus
            let center_dist = ((x as i32 - 4.5).abs() + (y as i32 - 4.5).abs()) as i32;
            let center_bonus = (5 - center_dist).max(0) * h * 50;

            if self.grid[sq] != 255 {
                let piece_idx = self.grid[sq] as usize;
                if let Some(piece) = self.pieces[piece_idx] {
                    let sign = if piece.owner == side { 1 } else { -1 };
                    score += center_bonus * sign;
                }
            }
        }

        score
    }

    fn is_sentinel_threatened(&self, player: Player) -> bool {
        for i in 0..16 {
            if let Some(piece) = self.pieces[i] {
                if piece.kind == PieceKind::Sentinel && piece.owner == player {
                    let exit_row = if player == Player::Red { 9 } else { 0 };
                    let y = (piece.sq / 10) as i32;
                    return (exit_row - y).abs() <= 1;
                }
            }
        }
        false
    }
}

fn quiescence(board: &mut BoardState, mut alpha: i32, beta: i32, aborted: &mut bool) -> i32 {
    if fuel_left() < SAFETY_FUEL {
        *aborted = true;
        return 0;
    }

    let stand_pat = board.evaluate();
    if stand_pat >= beta {
        return beta;
    }
    alpha = alpha.max(stand_pat);

    // Generate only captures
    let mut captures = [SimMove::default(); 64];
    let cap_count = board.generate_captures(&mut captures);

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
    fn generate_captures(&self, buffer: &mut [SimMove; 64]) -> usize {
        let mut count = 0;
        let opp = self.to_move.opponent();

        for i in 0..16 {
            if let Some(piece) = self.pieces[i] {
                if piece.owner != self.to_move {
                    continue;
                }

                let sq = piece.sq as usize;
                let x = sq % 10;
                let y = sq / 10;
                let h = self.heights[sq] as i32;

                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                        let nsq = (ny * 10 + nx) as usize;
                        let occ = self.grid[nsq];
                        if occ != 255 {
                            let target_piece = self.pieces[occ as usize].unwrap();
                            if target_piece.owner == opp {
                                let att = self.attack_value(piece.kind, h, self.heights[nsq] as i32);
                                let def = self.attack_value(
                                    target_piece.kind,
                                    self.heights[nsq] as i32,
                                    h,
                                );
                                if att >= def && count < 64 {
                                    buffer[count] = SimMove {
                                        piece_idx: i as u8,
                                        from: sq as u8,
                                        to: nsq as u8,
                                        kind: MoveKind::Attack,
                                        cost: 2,
                                        captured_idx: occ,
                                    };
                                    count += 1;
                                }
                            }
                        }
                    }
                }

                // Explorer diagonal captures
                if piece.kind == PieceKind::Explorer {
                    for (dx, dy) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                            let nsq = (ny * 10 + nx) as usize;
                            let occ = self.grid[nsq];
                            if occ != 255 {
                                let target_piece = self.pieces[occ as usize].unwrap();
                                if target_piece.owner == opp {
                                    let att = self.attack_value(piece.kind, h, self.heights[nsq] as i32);
                                    let def = self.attack_value(
                                        target_piece.kind,
                                        self.heights[nsq] as i32,
                                        h,
                                    );
                                    if att >= def && count < 64 {
                                        buffer[count] = SimMove {
                                            piece_idx: i as u8,
                                            from: sq as u8,
                                            to: nsq as u8,
                                            kind: MoveKind::Attack,
                                            cost: 2,
                                            captured_idx: occ,
                                        };
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        count
    }
}

fn order_moves(moves: &mut [SimMove; 64], count: usize, board: &BoardState) {
    // Simple move ordering: prioritize Sentinel exits and captures
    for i in 0..count {
        for j in (i + 1)..count {
            let mi = moves[i];
            let mj = moves[j];

            let score_i = move_score(mi, board);
            let score_j = move_score(mj, board);

            if score_j > score_i {
                moves.swap(i, j);
            }
        }
    }
}

fn move_score(m: SimMove, board: &BoardState) -> i32 {
    if m.kind == MoveKind::Exit {
        if board.pieces[m.piece_idx as usize].unwrap().kind == PieceKind::Sentinel {
            return 10_000_000; // Highest priority
        }
        return 1_000_000;
    }

    if m.kind == MoveKind::Attack {
        let captured = board.pieces[m.captured_idx as usize].unwrap();
        if captured.kind == PieceKind::Sentinel {
            return 5_000_000;
        }
        return match captured.kind {
            PieceKind::Vanguard => 2_000_000,
            PieceKind::Explorer => 1_000_000,
            _ => 500_000,
        };
    }

    0
}

fn negamax(
    board: &mut BoardState,
    depth: u32,
    mut alpha: i32,
    beta: i32,
    aborted: &mut bool,
) -> i32 {
    if *aborted || fuel_left() < SAFETY_FUEL {
        *aborted = true;
        return 0;
    }

    // Check for terminal states
    if board.sentinel_exited[Player::Red as usize] {
        return if board.to_move == Player::Red { -SENTINEL_EXIT_WIN } else { SENTINEL_EXIT_WIN };
    }
    if board.sentinel_exited[Player::Blue as usize] {
        return if board.to_move == Player::Blue { -SENTINEL_EXIT_WIN } else { SENTINEL_EXIT_WIN };
    }

    if depth == 0 {
        return quiescence(board, alpha, beta, aborted);
    }

    let mut moves = [SimMove::default(); 64];
    let move_count = board.generate_moves(&mut moves);
    order_moves(&mut moves, move_count, board);

    let mut best_score = -INF;

    for i in 0..move_count {
        let m = moves[i];
        let (prev_sq, captured, exited) = board.apply(m);
        let score = -negamax(board, depth - 1, -beta, -alpha, aborted);
        board.unmake(m, prev_sq, captured, exited);

        if *aborted {
            return 0;
        }

        if score >= beta {
            return beta;
        }

        if score > best_score {
            best_score = score;
            if score > alpha {
                alpha = score;
            }
        }
    }

    if best_score == -INF {
        // No legal moves - should not happen in Traversal
        return -SENTINEL_EXIT_WIN;
    }

    best_score
}

fn choose(turn: Turn) -> Move {
    let mut board = BoardState::from_turn(&turn);
    let mut moves = [SimMove::default(); 64];
    let move_count = board.generate_moves(&mut moves);
    order_moves(&mut moves, move_count, &board);

    if move_count == 0 {
        return Move::pass();
    }

    let mut best_move_idx = 0;
    let mut best_score = -INF;
    let mut aborted = false;

    // Iterative deepening
    for depth in 1..=MAX_DEPTH {
        if fuel_left() < SAFETY_FUEL {
            break;
        }

        let mut depth_best = best_move_idx;
        let mut alpha = -INF;
        let beta = INF;

        for i in 0..move_count {
            if fuel_left() < SAFETY_FUEL {
                aborted = true;
                break;
            }

            let m = moves[i];
            let (prev_sq, captured, exited) = board.apply(m);
            let score = -negamax(&mut board, depth - 1, -beta, -alpha, &mut aborted);
            board.unmake(m, prev_sq, captured, exited);

            if aborted {
                break;
            }

            if score > alpha {
                alpha = score;
                depth_best = i;
            }
        }

        if !aborted {
            best_move_idx = depth_best;
            best_score = alpha;
        } else {
            break;
        }
    }

    // Convert SimMove to Move
    let best_sim_move = moves[best_move_idx];
    let piece = board.pieces[best_sim_move.piece_idx as usize].unwrap();

    if best_sim_move.kind == MoveKind::Exit {
        Move::exit(piece.idx)
    } else if best_sim_move.kind == MoveKind::Attack {
        Move::attack(piece.idx, best_sim_move.to)
    } else {
        Move::move_to(piece.idx, best_sim_move.to)
    }
}

bot!(choose);
