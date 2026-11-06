use traversal_bot::core::MoveKind;
use traversal_bot::{bot, fuel_left, Turn};
use rand::SeedableRng;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

const SAFETY_FUEL: u64 = 50_000_000;
const MAX_ITERATIONS: usize = 1500;

#[derive(Clone, Copy, Debug)]
pub struct SimMove {
    pub piece_idx: u8,
    pub from: u8,
    pub to: u8,
    pub kind: MoveKind,
    pub cost: u8,
    pub captured_idx: u8,
}

impl Default for SimMove {
    fn default() -> Self {
        SimMove {
            piece_idx: 0,
            from: 0,
            to: 0,
            kind: MoveKind::Move,
            cost: 0,
            captured_idx: 255,
        }
    }
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

impl BoardState {
    fn from_turn(turn: &Turn) -> Self {
        let state = &turn.state;
        let pieces = state.pieces();
        let my_side = state.to_move;

        let mut board = Self {
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

        board
    }

    fn apply(&mut self, m: SimMove) -> (u8, u8, bool) {
        let p = m.piece_idx as usize;
        let prev_sq = self.pieces_sq[p];
        let mut captured = 255;
        let mut exited = false;

        match m.kind {
            MoveKind::Exit => {
                self.grid[m.from as usize] = 255;
                self.pieces_sq[p] = 255;
                self.pieces_alive[p] = false;
                exited = true;
                if self.pieces_kind[p] == 2 {
                    self.sentinel_exited[self.to_move as usize] = true;
                }
            }
            MoveKind::Attack => {
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
            MoveKind::Move | MoveKind::Pass => {
                if m.to != 255 {
                    self.grid[m.from as usize] = 255;
                    self.grid[m.to as usize] = m.piece_idx;
                    self.pieces_sq[p] = m.to;
                }
            }
        }

        self.to_move = if self.to_move == 1 { 2 } else { 1 };
        (prev_sq, captured, exited)
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
            let kind = self.pieces_kind[i];

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
                                kind: MoveKind::Move,
                                cost,
                                captured_idx: 255,
                            };
                            count += 1;
                        }
                    } else if self.pieces_owner[occ as usize] == opp {
                        let att = match kind { 2 => 3, 1 => 2, _ => 1 } + if ch > nh { 1 } else { 0 };
                        let def = match self.pieces_kind[occ as usize] { 2 => 3, 1 => 2, _ => 1 } + if nh > ch { 1 } else { 0 };

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

            // Explorer: diagonal et leap
            if kind == 0 {
                for (dx, dy) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
                    let nx = x + dx;
                    let ny = y + dy;
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

                for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                    let nx = x + dx;
                    let ny = y + dy;
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
        count
    }

    fn is_terminal(&self) -> (bool, f32) {
        if self.sentinel_exited[1] {
            return (true, 1.0);
        }
        if self.sentinel_exited[2] {
            return (true, -1.0);
        }

        let mut moves = [SimMove::default(); 64];
        if self.generate_moves(&mut moves) == 0 {
            return (true, 0.0);
        }

        (false, 0.0)
    }

    fn blocks_sentinel(&self, to_sq: u8) -> bool {
        let to_sq = to_sq as usize;
        let to_x = to_sq % 10;
        let to_y = to_sq / 10;
        let opp = 3 - self.to_move;

        for i in 0..self.piece_count {
            if !self.pieces_alive[i] || self.pieces_owner[i] != opp || self.pieces_kind[i] != 2 {
                continue;
            }
            let sent_sq = self.pieces_sq[i] as usize;
            let sent_x = sent_sq % 10;
            let sent_y = sent_sq / 10;
            let exit_row = if opp == 1 { 9 } else { 0 };

            if (sent_y as i32).abs_diff(exit_row as i32) == 1 {
                if (to_y as i32).abs_diff(sent_y as i32) <= 1 && (to_x as i32).abs_diff(sent_x as i32) <= 1 {
                    return true;
                }
                if to_y == exit_row && to_x == sent_x {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Clone)]
struct MCTSNode {
    visits: u32,
    total_value: f32,
    children: Vec<MCTSNode>,
    untried_moves: Vec<usize>,
    move_index: usize,
}

impl MCTSNode {
    fn new() -> Self {
        Self {
            visits: 0,
            total_value: 0.0,
            children: Vec::new(),
            untried_moves: Vec::new(),
            move_index: 0,
        }
    }

    fn uct_value(&self, exploration_weight: f32, parent_visits: u32) -> f32 {
        if self.visits == 0 {
            return f32::INFINITY;
        }
        let exploitation = self.total_value / self.visits as f32;
        let exploration = exploration_weight * (2.0 * (parent_visits as f32).ln() / self.visits as f32).sqrt();
        exploitation + exploration
    }

    fn best_child(&self) -> usize {
        self.children.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.visits.cmp(&b.visits))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

fn simulate(board: &mut BoardState, rng: &mut ChaCha8Rng) -> f32 {
    let mut ply = 0;

    loop {
        ply += 1;
        if ply > 150 {
            return 0.0;
        }

        let (terminal, result) = board.is_terminal();
        if terminal {
            return result;
        }

        let mut moves = [SimMove::default(); 64];
        let count = board.generate_moves(&mut moves);
        if count == 0 {
            return 0.0;
        }

        // Priorité absolue aux Sentinels
        let mut scores = vec![0.0; count];
        for i in 0..count {
            let m = &moves[i];
            if m.kind == MoveKind::Exit {
                scores[i] = 10_000.0;
                if board.pieces_kind[m.piece_idx as usize] == 2 {
                    scores[i] += 100_000.0;
                }
            } else if m.kind == MoveKind::Attack {
                scores[i] = 1000.0;
                if m.captured_idx != 255 && board.pieces_kind[m.captured_idx as usize] == 2 {
                    scores[i] += 50_000.0;
                }
            } else if m.to != 255 {
                let y = (m.to / 10) as i32;
                let target_y = if board.to_move == 1 { 9 } else { 0 };
                scores[i] = (target_y - y).abs() as f32 * 100.0;
                if board.blocks_sentinel(m.to) {
                    scores[i] += 50_000.0;
                }
            }
        }

        // Sélection pondérée
        let total: f32 = scores.iter().sum();
        if total > 0.0 {
            let r = rng.gen_range(0.0..total);
            let mut sum = 0.0;
            for (i, &s) in scores.iter().enumerate() {
                sum += s;
                if sum >= r {
                    board.apply(moves[i]);
                    break;
                }
            }
        } else {
            let i = rng.gen_range(0..count);
            board.apply(moves[i]);
        }
    }
}

fn choose(turn: &Turn) -> usize {
    if turn.moves.is_empty() {
        return 0;
    }

    // Mode rapide si fuel bas
    if fuel_left() < SAFETY_FUEL {
        let state = &turn.state;
        let pieces = state.pieces();
        for (i, m) in turn.moves.iter().enumerate() {
            if m.kind == MoveKind::Exit {
                if (m.piece as usize) < pieces.len() && pieces[m.piece as usize].kind as u8 == 2 {
                    return i;
                }
            }
        }
        for (i, m) in turn.moves.iter().enumerate() {
            if m.kind == MoveKind::Exit {
                return i;
            }
        }
        return 0;
    }

    let board = BoardState::from_turn(turn);
    let mut root_moves = [SimMove::default(); 64];
    let root_count = board.generate_moves(&mut root_moves);

    if root_count == 0 {
        return 0;
    }

    // MCTS avec gestion d'arbre par indices
    let mut root = MCTSNode::new();
    root.untried_moves = (0..root_count).collect();

    let mut rng = ChaCha8Rng::seed_from_u64(turn.seed);
    let mut iterations = 0;

    while fuel_left() > SAFETY_FUEL && iterations < MAX_ITERATIONS {
        let mut nodes = vec![root.clone()];
        let mut board_states = vec![board.clone()];
        let mut path = Vec::new();
        let mut current_node_idx = 0;

        // Sélection
        loop {
            let node = nodes[current_node_idx].clone();
            if node.untried_moves.is_empty() && node.children.is_empty() {
                break;
            }
            if !node.untried_moves.is_empty() {
                break;
            }

            let best_idx = node.children.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    let uct_a = a.uct_value(1.414, node.visits);
                    let uct_b = b.uct_value(1.414, node.visits);
                    uct_a.partial_cmp(&uct_b).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0);

            path.push(current_node_idx);
            nodes.push(node.children[best_idx].clone());
            current_node_idx = nodes.len() - 1;

            let m = root_moves[nodes[current_node_idx].move_index];
            board_states.last_mut().unwrap().apply(m);
        }

        // Expansion
        let node = nodes[current_node_idx].clone();
        if !node.untried_moves.is_empty() {
            let move_idx = node.untried_moves[rng.gen_range(0..node.untried_moves.len())];
            let m = root_moves[move_idx];

            let mut child = MCTSNode::new();
            child.move_index = move_idx;
            nodes[current_node_idx].untried_moves.remove(
                nodes[current_node_idx].untried_moves.iter().position(|&x| x == move_idx).unwrap()
            );
            nodes[current_node_idx].children.push(child.clone());
            nodes.push(child);
            current_node_idx = nodes.len() - 1;
            path.push(current_node_idx);

            board_states.last_mut().unwrap().apply(m);
        }

        // Simulation
        let mut sim_board = board_states.last().unwrap().clone();
        let result = simulate(&mut sim_board, &mut rng);

        // Backpropagation
        for &node_idx in &path {
            nodes[node_idx].visits += 1;
            nodes[node_idx].total_value += result;
        }

        root = nodes[0].clone();
        iterations += 1;
    }

    // Retourne le move avec le plus de visites
    if !root.children.is_empty() {
        let best_idx = root.best_child();
        root.children[best_idx].move_index
    } else {
        for (i, m) in root_moves.iter().enumerate() {
            if m.kind == MoveKind::Exit {
                return i;
            }
        }
        0
    }
}

bot!(choose);
