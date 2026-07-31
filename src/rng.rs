//! Deterministic pseudo-random number generation.
//!
//! Everything in Divus Factus that is procedurally generated — terrain, creature bodies,
//! names — derives from an explicit seed so that a given world can be reproduced
//! exactly. That rules out `rand`'s thread-local generators, and a PCG32 is small
//! enough that pulling in a dependency for it would not pay for itself.

/// A small, fast, deterministic generator (PCG-XSH-RR 64/32).
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    /// Creates a generator from a seed. Distinct seeds give uncorrelated streams.
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng {
            state: 0,
            inc: (seed << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// Creates a generator for a named sub-stream of `seed`.
    ///
    /// Lets each subsystem draw from its own stream, so adding a die roll in the
    /// terrain generator cannot shift the numbers the creature generator sees.
    pub fn stream(seed: u64, label: &str) -> Self {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in label.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Rng::new(seed ^ h)
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        // 24 bits of mantissa is the most an f32 can represent exactly.
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform in `[lo, hi)`.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.f32() * (hi - lo)
    }

    /// Uniform integer in `[lo, hi]`.
    pub fn range_i(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u32() % ((hi - lo + 1) as u32)) as i32
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// Uniform pick from a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        debug_assert!(!items.is_empty(), "cannot pick from an empty slice");
        &items[(self.next_u32() as usize) % items.len()]
    }

    /// Roughly normal, mean 0, standard deviation 1, clamped to +/- 3.
    ///
    /// Sum-of-uniforms rather than Box-Muller: creature traits want a cheap bell
    /// curve with no tail outliers, not statistical purity.
    pub fn gaussian(&mut self) -> f32 {
        let s = self.f32() + self.f32() + self.f32() + self.f32() + self.f32() + self.f32();
        (s - 3.0) * 1.414
    }

    /// A trait value in `[lo, hi]`, clustered toward the middle.
    pub fn trait_value(&mut self, lo: f32, hi: f32) -> f32 {
        let mid = (lo + hi) * 0.5;
        let half = (hi - lo) * 0.5;
        (mid + self.gaussian() * half * 0.4).clamp(lo, hi)
    }
}

/// Integer hash used by the value noise below. Deterministic across platforms.
pub fn hash_2d(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = seed;
    h ^= (x as u32).wrapping_mul(0x8da6_b343);
    h ^= (y as u32).wrapping_mul(0xd816_3841);
    h = h ^ (h >> 15);
    h = h.wrapping_mul(0x2c1b_3c6d);
    h = h ^ (h >> 12);
    h = h.wrapping_mul(0x2974_5b8d);
    h ^ (h >> 16)
}

/// `hash_2d` mapped to `[0, 1)`.
pub fn hash_2d_f32(x: i32, y: i32, seed: u32) -> f32 {
    (hash_2d(x, y, seed) >> 8) as f32 / (1u32 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..64 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let diverged = (0..16).any(|_| a.next_u32() != b.next_u32());
        assert!(diverged);
    }

    #[test]
    fn named_streams_are_independent() {
        let mut a = Rng::stream(7, "terrain");
        let mut b = Rng::stream(7, "creatures");
        let diverged = (0..16).any(|_| a.next_u32() != b.next_u32());
        assert!(diverged);
    }

    #[test]
    fn floats_stay_in_unit_range() {
        let mut rng = Rng::new(9);
        for _ in 0..10_000 {
            let v = rng.f32();
            assert!((0.0..1.0).contains(&v), "{v} out of range");
        }
    }

    #[test]
    fn range_i_covers_bounds_inclusively() {
        let mut rng = Rng::new(3);
        let mut saw_lo = false;
        let mut saw_hi = false;
        for _ in 0..1_000 {
            let v = rng.range_i(2, 5);
            assert!((2..=5).contains(&v));
            saw_lo |= v == 2;
            saw_hi |= v == 5;
        }
        assert!(saw_lo && saw_hi);
    }

    #[test]
    fn trait_values_respect_bounds() {
        let mut rng = Rng::new(11);
        for _ in 0..10_000 {
            let v = rng.trait_value(0.5, 2.0);
            assert!((0.5..=2.0).contains(&v), "{v} out of range");
        }
    }
}
