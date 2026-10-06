//! Reproducible board sampling, not credential generation.
pub const RNG_VERSION: u16 = 1;
pub trait RandomSource {
    fn next_u64(&mut self) -> u64;
}
pub struct SeededRng {
    state: u64,
}
impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}
impl RandomSource for SeededRng {
    fn next_u64(&mut self) -> u64 {
        // SplitMix64 by Sebastiano Vigna, public domain:
        // https://prng.di.unimi.it/splitmix64.c
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let value = (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        let value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
}
pub(crate) fn shuffle<T>(values: &mut [T], random: &mut impl RandomSource) {
    for index in (1..values.len()).rev() {
        let upper = (index + 1) as u64;
        let threshold = upper.wrapping_neg() % upper;
        let chosen = loop {
            let value = random.next_u64();
            if value >= threshold {
                break (value % upper) as usize;
            }
        };
        values.swap(index, chosen);
    }
}
