//! `Rng`: a small random number generator (xorshift64*).
//!
//! The same seed always gives the same numbers, so a crash chosen from a seed
//! can be repeated exactly.

/// A seeded random number generator.
pub struct Rng(u64);

impl Rng {
    /// A generator started from `seed`. Any seed works, 0 included.
    pub fn new(seed: u64) -> Rng {
        // xorshift gets stuck at 0, and small seeds give similar first numbers.
        // Mixing the seed with an odd constant fixes both.
        let state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
        Rng(if state == 0 { 1 } else { state })
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number from 0 up to, but not including, `n`. `n` must not be 0.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// `len` random bytes.
    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next_u64() as u8).collect()
    }
}
