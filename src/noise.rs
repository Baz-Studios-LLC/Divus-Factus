//! Value noise and fractal combinators used by terrain and scatter generation.
//!
//! Hand-rolled rather than pulled from a crate: the whole procedural pipeline needs
//! to be bit-reproducible from a seed, and this is a few dozen lines we fully control.

use crate::rng::hash_2d_f32;

/// Cubic smoothstep. Cheaper than a quintic and the difference is invisible at
/// the frequencies terrain uses.
#[inline]
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// 2D value noise in `[0, 1]`.
pub fn value_2d(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let xf = x - xi;
    let yf = y - yi;
    let (xi, yi) = (xi as i32, yi as i32);

    let v00 = hash_2d_f32(xi, yi, seed);
    let v10 = hash_2d_f32(xi + 1, yi, seed);
    let v01 = hash_2d_f32(xi, yi + 1, seed);
    let v11 = hash_2d_f32(xi + 1, yi + 1, seed);

    let u = smooth(xf);
    let v = smooth(yf);
    lerp(lerp(v00, v10, u), lerp(v01, v11, u), v)
}

/// Fractal Brownian motion: octaves of value noise at doubling frequency.
/// Returns roughly `[0, 1]`.
pub fn fbm_2d(x: f32, y: f32, seed: u32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;

    for octave in 0..octaves {
        let octave_seed = seed.wrapping_add(octave.wrapping_mul(0x9e37_79b9));
        sum += value_2d(x * freq, y * freq, octave_seed) * amp;
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }

    if norm > 0.0 { sum / norm } else { 0.0 }
}

/// Ridged fractal noise — folds each octave around its midpoint to produce creases.
/// Good for rocky spines; too sharp for general ground.
pub fn ridged_2d(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;

    for octave in 0..octaves {
        let octave_seed = seed.wrapping_add(octave.wrapping_mul(0x85eb_ca6b));
        let n = value_2d(x * freq, y * freq, octave_seed);
        let ridge = 1.0 - (n * 2.0 - 1.0).abs();
        sum += ridge * ridge * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }

    if norm > 0.0 { sum / norm } else { 0.0 }
}

/// Offsets the sample point by another noise field before sampling.
///
/// Turns the soap-bubble look of plain fbm into something with the swirls and
/// overhangs of eroded ground, for one extra noise lookup.
pub fn warped_fbm_2d(x: f32, y: f32, seed: u32, octaves: u32, warp: f32) -> f32 {
    let wx = fbm_2d(x + 5.2, y + 1.3, seed ^ 0x1234_5678, 3, 2.0, 0.5) - 0.5;
    let wy = fbm_2d(x + 9.7, y + 7.1, seed ^ 0x8765_4321, 3, 2.0, 0.5) - 0.5;
    fbm_2d(x + wx * warp, y + wy * warp, seed, octaves, 2.0, 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_noise_stays_in_range() {
        for i in 0..2_000 {
            let x = i as f32 * 0.37;
            let y = i as f32 * 0.71;
            let n = value_2d(x, y, 1234);
            assert!((0.0..=1.0).contains(&n), "{n} out of range");
        }
    }

    #[test]
    fn value_noise_is_continuous() {
        // Neighbouring samples should not jump: interpolation, not white noise.
        let mut prev = value_2d(0.0, 0.0, 7);
        for i in 1..500 {
            let n = value_2d(i as f32 * 0.01, 0.0, 7);
            assert!((n - prev).abs() < 0.1, "discontinuity at {i}");
            prev = n;
        }
    }

    #[test]
    fn fbm_stays_in_range() {
        for i in 0..2_000 {
            let n = fbm_2d(i as f32 * 0.13, i as f32 * 0.29, 99, 5, 2.0, 0.5);
            assert!((0.0..=1.0).contains(&n), "{n} out of range");
        }
    }

    #[test]
    fn noise_is_deterministic() {
        let a = warped_fbm_2d(3.5, -2.25, 4242, 4, 0.5);
        let b = warped_fbm_2d(3.5, -2.25, 4242, 4, 0.5);
        assert_eq!(a, b);
    }

    #[test]
    fn different_seeds_give_different_fields() {
        let a = fbm_2d(1.5, 2.5, 1, 4, 2.0, 0.5);
        let b = fbm_2d(1.5, 2.5, 2, 4, 2.0, 0.5);
        assert_ne!(a, b);
    }
}
