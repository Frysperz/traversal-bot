use crate::nnue::{get_feature_index, Accumulator, ACCUMULATOR_SIZE, FEATURE_COUNT};

pub const NO_PIECE: u8 = 255;
pub const MAX_PIECES: usize = 16;
pub const MAX_MOVES: usize = 64;

// Poids calibrés de l'accumulateur et de sortie
pub static mut FEATURE_WEIGHTS: [[i16; ACCUMULATOR_SIZE]; FEATURE_COUNT] =
    [[0i16; ACCUMULATOR_SIZE]; FEATURE_COUNT];
pub static mut OUTPUT_WEIGHTS: [i16; ACCUMULATOR_SIZE] = [1i16; ACCUMULATOR_SIZE];
pub static mut OUTPUT_BIAS: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FastMoveKind {
    Move,
    Attack,
    Exit,
}

#[derive(Clone, Copy, Debug)]
pub struct FastMove {
    pub piece_idx: u8,
    pub from: u8,
    pub to: u8,
    pub kind: FastMoveKind,
    pub cost: u8,
    pub captured_piece_idx: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct FastPiece {
    pub sq: u8,
    pub kind: u8,  // 0: Explorer, 1: Vanguard, 2: Sentinel
    pub owner: u8, // 1: Rouge, 2: Bleu
    pub alive: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct UndoInfo {
    pub moved_piece_idx: u8,
    pub from: u8,
    pub to: u8,
    pub captured_piece_idx: u8,
    pub was_exit: bool,
    pub prev_to_move: u8,
    pub prev_accumulator: Accumulator,
}

#[derive(Clone, Copy)]
pub struct FastBoard {
    pub pieces: [FastPiece; MAX_PIECES],
    pub piece_count: usize,
    pub grid: [u8; 100],
    pub heights: [u8; 100],
    pub to_move: u8,
    pub sentinel_exited: [bool; 3],
    pub total_exits: u8,
    pub acc: Accumulator,
}

impl FastBoard {
    pub fn new() -> Self {
        Self {
            pieces: [FastPiece { sq: 255, kind: 0, owner: 0, alive: false }; MAX_PIECES],
            piece_count: 0,
            grid: [NO_PIECE; 100],
            heights: [0; 100],
            to_move: 1,
            sentinel_exited: [false; 3],
            total_exits: 0,
            acc: Accumulator::empty(),
        }
    }

    /// Initialise complètement l'accumulateur au début d'un tour
    pub fn init_accumulator(&mut self) {
        self.acc = Accumulator::empty();
        unsafe {
            for i in 0..self.piece_count {
                let p = &self.pieces[i];
                if p.alive && p.sq != 255 {
                    let feat = get_feature_index(p.sq, p.kind, p.owner);
                    self.acc.add_feature(feat, &FEATURE_WEIGHTS);
                }
            }
        }
    }

    /// Applique un coup et met à jour l'accumulateur en O(1)
    #[inline(always)]
    pub fn apply_move(&mut self, m: FastMove) -> UndoInfo {
        let undo = UndoInfo {
            moved_piece_idx: m.piece_idx,
            from: m.from,
            to: m.to,
            captured_piece_idx: m.captured_piece_idx,
            was_exit: m.kind == FastMoveKind::Exit,
            prev_to_move: self.to_move,
            prev_accumulator: self.acc,
        };

        let p_idx = m.piece_idx as usize;
        let kind = self.pieces[p_idx].kind;
        let owner = self.pieces[p_idx].owner;

        unsafe {
            // Retrait de la position d'origine
            let from_feat = get_feature_index(m.from, kind, owner);
            self.acc.sub_feature(from_feat, &FEATURE_WEIGHTS);

            match m.kind {
                FastMoveKind::Exit => {
                    self.grid[m.from as usize] = NO_PIECE;
                    self.pieces[p_idx].sq = 255;
                    self.pieces[p_idx].alive = false;
                    self.total_exits += 1;
                    if kind == 2 && self.total_exits == 1 {
                        self.sentinel_exited[self.to_move as usize] = true;
                    }
                }
                FastMoveKind::Attack => {
                    let target_sq = m.to as usize;
                    let captured = self.grid[target_sq];
                    if captured != NO_PIECE {
                        let cap_p = &mut self.pieces[captured as usize];
                        cap_p.alive = false;
                        cap_p.sq = 255;
                        let cap_feat = get_feature_index(m.to, cap_p.kind, cap_p.owner);
                        self.acc.sub_feature(cap_feat, &FEATURE_WEIGHTS);
                    }
                    self.grid[m.from as usize] = NO_PIECE;
                    self.grid[target_sq] = m.piece_idx;
                    self.pieces[p_idx].sq = m.to;

                    let to_feat = get_feature_index(m.to, kind, owner);
                    self.acc.add_feature(to_feat, &FEATURE_WEIGHTS);
                }
                FastMoveKind::Move => {
                    self.grid[m.from as usize] = NO_PIECE;
                    self.grid[m.to as usize] = m.piece_idx;
                    self.pieces[p_idx].sq = m.to;

                    let to_feat = get_feature_index(m.to, kind, owner);
                    self.acc.add_feature(to_feat, &FEATURE_WEIGHTS);
                }
            }
        }

        self.to_move = if self.to_move == 1 { 2 } else { 1 };
        undo
    }

    /// Restauration instantanée de l'état et de l'accumulateur
    #[inline(always)]
    pub fn unmake_move(&mut self, undo: UndoInfo) {
        self.to_move = undo.prev_to_move;
        self.acc = undo.prev_accumulator;
        let p_idx = undo.moved_piece_idx as usize;

        if undo.was_exit {
            self.pieces[p_idx].alive = true;
            self.pieces[p_idx].sq = undo.from;
            self.grid[undo.from as usize] = undo.moved_piece_idx;
            if self.pieces[p_idx].kind == 2 && self.sentinel_exited[self.to_move as usize] {
                self.sentinel_exited[self.to_move as usize] = false;
            }
            self.total_exits -= 1;
        } else {
            self.grid[undo.to as usize] = NO_PIECE;
            self.grid[undo.from as usize] = undo.moved_piece_idx;
            self.pieces[p_idx].sq = undo.from;

            if undo.captured_piece_idx != NO_PIECE {
                let cap_idx = undo.captured_piece_idx as usize;
                self.pieces[cap_idx].alive = true;
                self.pieces[cap_idx].sq = undo.to;
                self.grid[undo.to as usize] = undo.captured_piece_idx;
            }
        }
    }

    pub fn generate_moves(&self, move_buffer: &mut [FastMove; MAX_MOVES]) -> usize {
        let mut count = 0;
        let opp_side = if self.to_move == 1 { 2 } else { 1 };
        let exit_row = if self.to_move == 1 { 9 } else { 0 };

        for i in 0..self.piece_count {
            let p = &self.pieces[i];
            if !p.alive || p.owner != self.to_move {
                continue;
            }

            let sq = p.sq as usize;
            let x = (sq % 10) as i32;
            let y = (sq / 10) as i32;
            let current_h = self.heights[sq] as i32;

            if y == exit_row && count < MAX_MOVES {
                move_buffer[count] = FastMove {
                    piece_idx: i as u8,
                    from: p.sq,
                    to: 255,
                    kind: FastMoveKind::Exit,
                    cost: 1,
                    captured_piece_idx: NO_PIECE,
                };
                count += 1;
            }

            let dirs = [(-1, 0), (1, 0), (0, -1), (0, 1)];
            for (dx, dy) in dirs {
                let nx = x + dx;
                let ny = y + dy;

                if nx >= 0 && nx < 10 && ny >= 0 && ny < 10 {
                    let next_sq = (ny * 10 + nx) as usize;
                    let target_piece_idx = self.grid[next_sq];
                    let next_h = self.heights[next_sq] as i32;

                    if target_piece_idx == NO_PIECE {
                        let cost = (1 + (next_h - current_h).max(0)) as u8;
                        if count < MAX_MOVES {
                            move_buffer[count] = FastMove {
                                piece_idx: i as u8,
                                from: p.sq,
                                to: next_sq as u8,
                                kind: FastMoveKind::Move,
                                cost,
                                captured_piece_idx: NO_PIECE,
                            };
                            count += 1;
                        }
                    } else {
                        let opp = &self.pieces[target_piece_idx as usize];
                        if opp.owner == opp_side {
                            let att_base = match p.kind { 2 => 3, 1 => 2, _ => 1 };
                            let def_base = match opp.kind { 2 => 3, 1 => 2, _ => 1 };

                            let effective_att = att_base + if current_h > next_h { 1 } else { 0 };
                            let effective_def = def_base + if next_h > current_h { 1 } else { 0 };

                            if effective_att >= effective_def && count < MAX_MOVES {
                                move_buffer[count] = FastMove {
                                    piece_idx: i as u8,
                                    from: p.sq,
                                    to: next_sq as u8,
                                    kind: FastMoveKind::Attack,
                                    cost: 2,
                                    captured_piece_idx: target_piece_idx,
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
}
