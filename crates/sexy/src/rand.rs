//! `MTRand`: SexyFramework's Mersenne Twister (MT19937), with its quirks
//! (31-bit output, seed 0 → 4357, `%` range reduction), plus TodLib's
//! `RandRangeInt`/`RandRangeFloat`.
//!
//! Behaviour per ruslan831 decomp, `SexyAppFramework/MTRand.cpp` and
//! `TodCommon.cpp`.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

#[derive(Clone)]
pub struct MtRand {
    mt: [u32; N],
    mti: usize,
}

impl Default for MtRand {
    fn default() -> Self {
        Self::new(4357)
    }
}

impl std::fmt::Debug for MtRand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MtRand").field("mti", &self.mti).finish_non_exhaustive()
    }
}

impl MtRand {
    pub fn new(seed: u32) -> Self {
        let mut r = Self { mt: [0; N], mti: N };
        r.seed(seed);
        r
    }

    pub fn seed(&mut self, seed: u32) {
        let seed = if seed == 0 { 4357 } else { seed };
        self.mt[0] = seed;
        for i in 1..N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32.wrapping_mul(prev ^ (prev >> 30)).wrapping_add(i as u32);
        }
        self.mti = N;
    }

    /// Next value in 0..2³¹.
    pub fn next_u31(&mut self) -> u32 {
        let mag01 = |y: u32| if y & 1 == 0 { 0 } else { MATRIX_A };
        if self.mti >= N {
            for kk in 0..N - M {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M] ^ (y >> 1) ^ mag01(y);
            }
            for kk in N - M..N - 1 {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M - N] ^ (y >> 1) ^ mag01(y);
            }
            let y = (self.mt[N - 1] & UPPER_MASK) | (self.mt[0] & LOWER_MASK);
            self.mt[N - 1] = self.mt[M - 1] ^ (y >> 1) ^ mag01(y);
            self.mti = 0;
        }
        let mut y = self.mt[self.mti];
        self.mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y & 0x7fff_ffff
    }

    /// `Rand(int range)`: 0..range.
    pub fn next_range(&mut self, range: u32) -> u32 {
        self.next_u31() % range
    }

    /// `Rand(float range)`: 0..=range.
    pub fn next_f32(&mut self, range: f32) -> f32 {
        (self.next_u31() as f64 / 0x7fff_ffff as f64 * range as f64) as f32
    }

    /// `RandRangeInt`: inclusive on both ends.
    pub fn range_int(&mut self, min: i32, max: i32) -> i32 {
        debug_assert!(min <= max);
        self.next_range((max - min + 1) as u32) as i32 + min
    }

    /// `RandRangeFloat`.
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        debug_assert!(min <= max);
        self.next_f32(max - min) + min
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_reference_mt19937_masked_to_31_bits() {
        // MT19937 seeded with 5489 starts 3499211612, 581869302, 3890346734.
        let mut r = MtRand::new(5489);
        assert_eq!([r.next_u31(), r.next_u31(), r.next_u31()], [3499211612 & 0x7fff_ffff, 581869302, 3890346734 & 0x7fff_ffff]);
    }

    #[test]
    fn ranges_stay_in_bounds() {
        let mut r = MtRand::default();
        for _ in 0..1000 {
            let i = r.range_int(-6000, 2000);
            assert!((-6000..=2000).contains(&i));
            let f = r.range_f32(3.0, 12.0);
            assert!((3.0..=12.0).contains(&f));
        }
    }
}
