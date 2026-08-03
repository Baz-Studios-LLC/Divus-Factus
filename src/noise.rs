//! Value noise and fractal combinators used by terrain and scatter generation.
//!
//! Hand-rolled rather than pulled from a crate: the whole procedural pipeline needs
//! to be bit-reproducible from a seed, and this is a few dozen lines we fully control.

use crate::rng::{hash_2d_f32, hash_3d_f32};
use bevy::math::Vec3;

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

// ----------------------------------------------------------- in three dimensions
//
// The same construction one axis up, for a world that is a sphere. Terrain there
// is a field over directions rather than over a plane, and sampling a volume and
// keeping only the unit sphere gives it with no seam anywhere and no crowding at
// the poles — which is the one thing every flat projection of a globe gets wrong.

/// 3D value noise in `[0, 1]`.
pub fn value_3d(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let zi = z.floor();
    let xf = x - xi;
    let yf = y - yi;
    let zf = z - zi;
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);

    let u = smooth(xf);
    let v = smooth(yf);
    let w = smooth(zf);

    let corner = |dx: i32, dy: i32, dz: i32| hash_3d_f32(xi + dx, yi + dy, zi + dz, seed);
    let near = lerp(
        lerp(corner(0, 0, 0), corner(1, 0, 0), u),
        lerp(corner(0, 1, 0), corner(1, 1, 0), u),
        v,
    );
    let far = lerp(
        lerp(corner(0, 0, 1), corner(1, 0, 1), u),
        lerp(corner(0, 1, 1), corner(1, 1, 1), u),
        v,
    );
    lerp(near, far, w)
}

/// Fractal Brownian motion over a volume. Returns roughly `[0, 1]`.
pub fn fbm_3d(at: Vec3, seed: u32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;

    for octave in 0..octaves {
        let octave_seed = seed.wrapping_add(octave.wrapping_mul(0x9e37_79b9));
        let p = at * freq;
        sum += value_3d(p.x, p.y, p.z, octave_seed) * amp;
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }

    if norm > 0.0 { sum / norm } else { 0.0 }
}

/// Volume fbm with the sample point pushed about by another field first — the
/// 3D twin of [`warped_fbm_2d`], and what keeps a sphere's ground from reading
/// as soap bubbles.
pub fn warped_fbm_3d(at: Vec3, seed: u32, octaves: u32, warp: f32) -> f32 {
    let wx = fbm_3d(
        at + Vec3::new(5.2, 1.3, 2.7),
        seed ^ 0x1234_5678,
        3,
        2.0,
        0.5,
    ) - 0.5;
    let wy = fbm_3d(
        at + Vec3::new(9.7, 7.1, 4.1),
        seed ^ 0x8765_4321,
        3,
        2.0,
        0.5,
    ) - 0.5;
    let wz = fbm_3d(
        at + Vec3::new(3.3, 8.9, 6.5),
        seed ^ 0xa5a5_5a5a,
        3,
        2.0,
        0.5,
    ) - 0.5;
    fbm_3d(at + Vec3::new(wx, wy, wz) * warp, seed, octaves, 2.0, 0.5)
}

/// Ridged fractal noise over a volume, for rocky spines on a round world.
pub fn ridged_3d(at: Vec3, seed: u32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;

    for octave in 0..octaves {
        let octave_seed = seed.wrapping_add(octave.wrapping_mul(0x85eb_ca6b));
        let p = at * freq;
        let n = value_3d(p.x, p.y, p.z, octave_seed);
        let ridge = 1.0 - (n * 2.0 - 1.0).abs();
        sum += ridge * ridge * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }

    if norm > 0.0 { sum / norm } else { 0.0 }
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
        let at = Vec3::new(3.5, -2.25, 1.75);
        assert_eq!(
            warped_fbm_3d(at, 4242, 4, 0.5),
            warped_fbm_3d(at, 4242, 4, 0.5)
        );
    }

    #[test]
    fn volume_noise_stays_in_range() {
        for i in 0..2_000 {
            let at = Vec3::new(i as f32 * 0.13, i as f32 * 0.29, i as f32 * 0.07);
            let n = fbm_3d(at, 99, 5, 2.0, 0.5);
            assert!((0.0..=1.0).contains(&n), "{n} out of range");
        }
    }

    /// The whole reason for a volume: a field on the sphere is smooth
    /// everywhere, including over the poles, where a latitude grid crowds to
    /// nothing, and across the meridian, where a flat projection needs a seam.
    ///
    /// Sampled finely enough to mean it. The finest octave of five has features
    /// a sixteenth of a noise unit across, so a stride has to be well under
    /// that or the test is measuring its own undersampling — which is exactly
    /// what the first version of it did, and it read as a discontinuity in a
    /// field that has none.
    #[test]
    fn the_field_is_smooth_all_the_way_round_the_sphere() {
        use std::f32::consts::TAU;
        const SCALE: f32 = 16.5;
        // Three octaves, so the finest feature is a quarter of a noise unit,
        // and six thousand steps, so each stride is a fiftieth of one.
        let steps = 6_000;
        for &lat in &[0.0f32, 0.7, 1.4, -1.4, 1.5707] {
            let mut prev: Option<f32> = None;
            for i in 0..=steps {
                let lon = i as f32 / steps as f32 * TAU;
                let dir = Vec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos());
                let n = fbm_3d(dir * SCALE, 4242, 3, 2.0, 0.5);
                if let Some(p) = prev {
                    // The bound is derived, not picked. Smoothstep's steepest
                    // slope is 1.5 per cell, so each octave can move at most
                    // 1.5 * stride * frequency * amplitude / norm in one step;
                    // over three octaves at this stride that sums to about
                    // 0.044. Anything under it is the field's own gradient,
                    // anything over it would be a cliff - and a guessed
                    // threshold of 0.02 failed on a step of 0.0201, which said
                    // nothing about the field and everything about the guess.
                    assert!((n - p).abs() < 0.05, "cliff at lat {lat}: {p} -> {n}");
                }
                prev = Some(n);
            }
        }
    }

    /// And the circle closes. Going the whole way round arrives back at the
    /// value it left, which is what "no seam" actually means - and it is true
    /// for nothing but a field that never knew about longitude in the first
    /// place.
    #[test]
    fn the_field_has_no_meridian_to_come_apart_on() {
        use std::f32::consts::TAU;
        for &lat in &[0.0f32, 0.9, -1.2] {
            let at = |lon: f32| {
                let dir = Vec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos());
                fbm_3d(dir * 16.5, 4242, 5, 2.0, 0.5)
            };
            assert!((at(0.0) - at(TAU)).abs() < 1e-4, "seam at lat {lat}");
        }
    }

    /// Both poles are ordinary ground. On a latitude/longitude heightmap they
    /// are single rows stretched across the whole width; here they are points
    /// like any other, and the field is continuous through them.
    #[test]
    fn the_poles_are_nothing_special() {
        let north = fbm_3d(Vec3::Y * 16.5, 4242, 5, 2.0, 0.5);
        let south = fbm_3d(Vec3::NEG_Y * 16.5, 4242, 5, 2.0, 0.5);
        for n in [north, south] {
            assert!((0.0..=1.0).contains(&n));
        }
        // Walking over the top: approach the pole and pass beyond it.
        let mut prev: Option<f32> = None;
        for i in 0..=400 {
            let lat = 1.5707 - i as f32 * 0.0002;
            let dir = Vec3::new(0.0, lat.sin(), lat.cos()).normalize();
            let n = fbm_3d(dir * 16.5, 4242, 3, 2.0, 0.5);
            if let Some(p) = prev {
                assert!((n - p).abs() < 0.05, "cliff near the pole: {p} -> {n}");
            }
            prev = Some(n);
        }
    }

    #[test]
    fn different_seeds_give_different_fields() {
        let a = fbm_2d(1.5, 2.5, 1, 4, 2.0, 0.5);
        let b = fbm_2d(1.5, 2.5, 2, 4, 2.0, 0.5);
        assert_ne!(a, b);
    }
}
