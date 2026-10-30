use core::fmt;

pub const FEATURE_COUNT: usize = 600; // 100 cases * 3 types de pièces * 2 camps
pub const ACCUMULATOR_SIZE: usize = 128; // 128 neurones cachés en i16
pub const OUTPUT_SCALE: i32 = 16;

#[derive(Clone, Copy)]
pub struct Accumulator {
    pub vals: [i16; ACCUMULATOR_SIZE],
}

impl fmt::Debug for Accumulator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Accumulator({:?})", &self.vals[..8])
    }
}

impl Accumulator {
    pub const fn empty() -> Self {
        Self {
            vals: [0i16; ACCUMULATOR_SIZE],
        }
    }

    #[inline(always)]
    pub fn add_feature(&mut self, feature_idx: usize, weights: &[[i16; ACCUMULATOR_SIZE]; FEATURE_COUNT]) {
        if feature_idx < FEATURE_COUNT {
            let row = &weights[feature_idx];
            for i in 0..ACCUMULATOR_SIZE {
                self.vals[i] = self.vals[i].saturating_add(row[i]);
            }
        }
    }

    #[inline(always)]
    pub fn sub_feature(&mut self, feature_idx: usize, weights: &[[i16; ACCUMULATOR_SIZE]; FEATURE_COUNT]) {
        if feature_idx < FEATURE_COUNT {
            let row = &weights[feature_idx];
            for i in 0..ACCUMULATOR_SIZE {
                self.vals[i] = self.vals[i].saturating_sub(row[i]);
            }
        }
    }

    /// Inférence de la couche de sortie avec activation Clipped ReLU (0..127)
    #[inline(always)]
    pub fn evaluate_output(
        &self,
        output_weights: &[i16; ACCUMULATOR_SIZE],
        output_bias: i32,
        perspective_is_side1: bool,
    ) -> i32 {
        let mut sum: i32 = output_bias;
        for i in 0..ACCUMULATOR_SIZE {
            let activated = self.vals[i].clamp(0, 127) as i32;
            sum += activated * (output_weights[i] as i32);
        }

        let score = sum / OUTPUT_SCALE;
        if perspective_is_side1 {
            score
        } else {
            -score
        }
    }
}

/// Convertit les coordonnées d'une pièce en index de caractéristique dense
#[inline(always)]
pub fn get_feature_index(sq: u8, kind: u8, owner: u8) -> usize {
    let side_offset = if owner == 1 { 0 } else { 1 };
    let piece_offset = (kind as usize).min(2) * 2 + side_offset;
    (sq as usize) * 6 + piece_offset
}
