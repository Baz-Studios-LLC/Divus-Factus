//! Rivers that flow downhill.
//!
//! The first rivers were the zero-crossings of a noise field. They meandered
//! beautifully and were physically absurd — perched on hillsides, sagging across
//! valleys, going nowhere. Water obeys one law: it seeks level. A river that
//! ignores it reads as wrong to anyone watching, instantly, even if they cannot
//! say why.
//!
//! These rivers are *traced*, not drawn. A spring is chosen deterministically in
//! high country, and the course walks the terrain's steepest descent — with a
//! little momentum, the way moving water cuts its own bank — until it reaches the
//! sea or a basin it cannot leave. The water level along the course is clamped to
//! never rise, so a river cannot flow uphill *by construction*. No fluid is
//! simulated; the one law fluids obey here is enforced at generation time instead
//! of run time, which costs nothing per frame.
//!
//! Purity survives through determinism rather than statelessness. Traces are
//! memoised behind a lock, but every trace is a pure function of the seed — the
//! cache changes when rivers are computed, never what they are.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use super::{Terrain, WATER_LEVEL};
use crate::rng::hash_2d_f32;

/// Half-width of a river channel, in world units.
pub const CHANNEL_HALF_WIDTH: f32 = 6.0;
/// How far the bed dips below the water level at the channel centre.
pub const CHANNEL_DEPTH: f32 = 3.2;

/// One spring candidate per cell of this size.
const SPRING_CELL: f32 = 400.0;
/// Fraction of candidate cells that actually hold a spring.
const SPRING_KEEP: f32 = 0.30;
/// Springs only rise this far above sea level or higher.
const SPRING_MIN_ALTITUDE: f32 = 14.0;

/// Distance covered per trace step.
const STEP: f32 = 5.0;
/// Longest a river can run, in steps.
const MAX_STEPS: usize = 600;
/// How much of the previous heading carries into the next step. Raw gradient
/// descent zigzags; momentum is what gives a course its swept bends.
const MOMENTUM: f32 = 0.55;
/// Steps without finding lower ground before the course is declared trapped.
const PIT_PATIENCE: usize = 40;

/// Regions are the unit of generation: all springs in a region trace together.
const REGION: f32 = 2048.0;
/// Regions built around a query, so traces from neighbours are always present.
/// A trace runs at most `STEP * MAX_STEPS` = 3km, under two regions.
const REGION_REACH: i32 = 2;

/// Spatial-hash bin size for segment lookup.
const BIN: f32 = 32.0;

/// A piece of river course, carrying the water level and maturity at each end.
#[derive(Clone, Copy, Debug)]
struct Segment {
    a: Vec2,
    b: Vec2,
    level_a: f32,
    level_b: f32,
    width_a: f32,
    width_b: f32,
}

#[derive(Default)]
struct Inner {
    /// Regions whose own springs have been traced.
    built: HashSet<IVec2>,
    /// Regions whose full neighbourhood is built, i.e. safe to query.
    covered: HashSet<IVec2>,
    /// Segments by spatial bin. Insertion inflates by the channel width, so a
    /// point only ever needs to look in its own bin.
    bins: HashMap<IVec2, Vec<Segment>>,
}

/// The memoised river network.
#[derive(Default)]
pub struct RiverIndex {
    inner: RwLock<Inner>,
}

impl RiverIndex {
    /// Makes sure every trace that could influence `(x, z)` exists.
    ///
    /// Fast path is one set lookup. The slow path traces a neighbourhood of
    /// springs, which happens once per region per session.
    pub fn ensure_near(&self, terrain: &Terrain, x: f32, z: f32) {
        let region = IVec2::new((x / REGION).floor() as i32, (z / REGION).floor() as i32);
        if self.inner.read().unwrap().covered.contains(&region) {
            return;
        }

        let mut inner = self.inner.write().unwrap();
        if inner.covered.contains(&region) {
            return;
        }
        for dz in -REGION_REACH..=REGION_REACH {
            for dx in -REGION_REACH..=REGION_REACH {
                let build = region + IVec2::new(dx, dz);
                if inner.built.insert(build) {
                    build_region(&mut inner, terrain, build);
                }
            }
        }
        inner.covered.insert(region);
    }

    /// Water level, lateral distance and maturity of the nearest course.
    pub fn nearest(&self, x: f32, z: f32) -> Option<(f32, f32, f32)> {
        let bin = IVec2::new((x / BIN).floor() as i32, (z / BIN).floor() as i32);
        let inner = self.inner.read().unwrap();
        // The NINE bins around the point, not the one it stands in.
        //
        // A segment is filed in the bins it passes through, so a point near a
        // bin's edge could not see a course a few units away on the other side of
        // that edge — and the answer changed the instant the point crossed the
        // line. That is a river surface that steps at every bin boundary in the
        // world, and it is why "level across the channel" kept failing at
        // arbitrary places with no confluence anywhere near them.
        let point = Vec2::new(x, z);
        // The nearest segment, for the banks — and separately the HIGHEST water
        // among the segments whose channel actually holds this point, for the
        // water itself.
        //
        // Nearest alone is what made a river's surface step. Where two channels
        // overlap — a confluence, or a course doubling back on itself — the
        // nearest segment flips from one to the other over a few units and the
        // surface jumped with it, three units of water in three units of ground.
        // Taking the higher level instead is both continuous (the greater of two
        // continuous fields is continuous) and what water does: where two
        // channels meet, the lower one backs up to the higher.
        let mut best: Option<(f32, f32, f32)> = None;
        let mut holding: Option<(f32, f32, f32)> = None;

        for segment in (-1..=1)
            .flat_map(|dz| (-1..=1).map(move |dx| IVec2::new(dx, dz)))
            .filter_map(|offset| inner.bins.get(&(bin + offset)))
            .flatten()
        {
            let ab = segment.b - segment.a;
            let t = ((point - segment.a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let distance = point.distance(segment.a + ab * t);
            let level = segment.level_a + (segment.level_b - segment.level_a) * t;
            let width = segment.width_a + (segment.width_b - segment.width_a) * t;

            if distance < CHANNEL_HALF_WIDTH * width
                && holding.is_none_or(|(highest, _, _)| level > highest)
            {
                holding = Some((level, distance, width));
            }
            if best.is_none_or(|(_, d, _)| distance < d) {
                best = Some((level, distance, width));
            }
        }
        holding.or(best)
    }
}

/// The spring in a candidate cell, if that cell holds one.
pub(super) fn spring_in_cell(seed: u32, cell: IVec2) -> Option<Vec2> {
    if hash_2d_f32(cell.x, cell.y, seed ^ 0x5217) > SPRING_KEEP {
        return None;
    }
    let jx = hash_2d_f32(cell.x, cell.y, seed ^ 0x91aa) - 0.5;
    let jz = hash_2d_f32(cell.x, cell.y, seed ^ 0x37cd) - 0.5;
    Some((Vec2::new(cell.x as f32, cell.y as f32) + 0.5 + Vec2::new(jx, jz) * 0.7) * SPRING_CELL)
}

/// Steepest-descent direction of the un-carved terrain.
fn downhill(terrain: &Terrain, p: Vec2) -> Vec2 {
    let e = 5.0;
    let dx = terrain.base_height_at(p.x + e, p.y) - terrain.base_height_at(p.x - e, p.y);
    let dz = terrain.base_height_at(p.x, p.y + e) - terrain.base_height_at(p.x, p.y - e);
    Vec2::new(-dx, -dz)
}

/// Walks a course downhill from a spring.
///
/// Each point carries the water level there, clamped to never exceed any level
/// upstream of it — the invariant everything else rests on.
pub(super) fn trace(terrain: &Terrain, spring: Vec2) -> Vec<(Vec2, f32, f32)> {
    // A river grows from a trickle: width and depth ramp in over the first stretch
    // of the course. Without this a spring starts as a full-width channel from its
    // first metre — an abrupt brown scar with no water worth seeing in it.
    let maturity = |index: usize| -> f32 {
        let t = (index as f32 / 45.0).min(1.0);
        (t * t * (3.0 - 2.0 * t)).max(0.18)
    };

    let mut points = Vec::with_capacity(64);
    let mut position = spring;
    let mut level = terrain.base_height_at(spring.x, spring.y);
    let mut heading = Vec2::ZERO;
    let mut lowest_at = 0usize;

    points.push((position, level, maturity(0)));

    for _ in 0..MAX_STEPS {
        let fall = downhill(terrain, position);
        if fall.length_squared() > 1e-8 {
            let fall = fall.normalize();
            heading = if heading == Vec2::ZERO {
                fall
            } else {
                (heading * MOMENTUM + fall * (1.0 - MOMENTUM)).normalize_or(fall)
            };
        }
        if heading == Vec2::ZERO {
            break;
        }

        position += heading * STEP;
        let ground = terrain.base_height_at(position.x, position.y);

        // Water seeks level: the course's level may only fall.
        if ground < level - 1e-3 {
            level = ground;
            lowest_at = points.len();
        }
        points.push((position, level, maturity(points.len())));

        if level <= WATER_LEVEL + 0.5 {
            // Reached the sea; the ocean takes it from here.
            break;
        }
        if points.len() - lowest_at > PIT_PATIENCE {
            // Trapped in a basin with no lower ground. Cut back to the lowest
            // point reached — the river ends in a pond there, rather than drawing
            // a level canal through whatever wall it was climbing.
            points.truncate(lowest_at + 1);
            break;
        }
    }

    points
}

/// Traces every spring owned by `region` and files the segments into bins.
fn build_region(inner: &mut Inner, terrain: &Terrain, region: IVec2) {
    let cells = (REGION / SPRING_CELL).ceil() as i32 + 1;
    let first = IVec2::new(
        (region.x as f32 * REGION / SPRING_CELL).floor() as i32,
        (region.y as f32 * REGION / SPRING_CELL).floor() as i32,
    );

    for cz in first.y..=first.y + cells {
        for cx in first.x..=first.x + cells {
            let cell = IVec2::new(cx, cz);

            // A cell belongs to exactly one region, whichever contains its corner.
            let owner = IVec2::new(
                (cx as f32 * SPRING_CELL / REGION).floor() as i32,
                (cz as f32 * SPRING_CELL / REGION).floor() as i32,
            );
            if owner != region {
                continue;
            }

            let Some(spring) = spring_in_cell(terrain.seed(), cell) else {
                continue;
            };
            if terrain.base_height_at(spring.x, spring.y) < WATER_LEVEL + SPRING_MIN_ALTITUDE {
                continue;
            }

            let course = trace(terrain, spring);
            if course.len() < 8 {
                continue;
            }

            for pair in course.windows(2) {
                let segment = Segment {
                    a: pair[0].0,
                    b: pair[1].0,
                    level_a: pair[0].1,
                    level_b: pair[1].1,
                    width_a: pair[0].2,
                    width_b: pair[1].2,
                };
                file_segment(&mut inner.bins, segment);
            }
        }
    }
}

/// Inserts a segment into every bin its widened footprint touches, so queries
/// only ever need their own bin.
fn file_segment(bins: &mut HashMap<IVec2, Vec<Segment>>, segment: Segment) {
    let reach = CHANNEL_HALF_WIDTH * 2.6;
    let min = segment.a.min(segment.b) - Vec2::splat(reach);
    let max = segment.a.max(segment.b) + Vec2::splat(reach);

    let lo = IVec2::new((min.x / BIN).floor() as i32, (min.y / BIN).floor() as i32);
    let hi = IVec2::new((max.x / BIN).floor() as i32, (max.y / BIN).floor() as i32);

    for bz in lo.y..=hi.y {
        for bx in lo.x..=hi.x {
            bins.entry(IVec2::new(bx, bz)).or_default().push(segment);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn springs_near(terrain: &Terrain, span: i32) -> Vec<Vec2> {
        let mut found = Vec::new();
        for cz in -span..=span {
            for cx in -span..=span {
                if let Some(spring) = spring_in_cell(terrain.seed(), IVec2::new(cx, cz))
                    && terrain.base_height_at(spring.x, spring.y)
                        >= WATER_LEVEL + SPRING_MIN_ALTITUDE
                {
                    found.push(spring);
                }
            }
        }
        found
    }

    #[test]
    fn traced_rivers_never_flow_uphill() {
        // The invariant the whole design exists to provide. Every course's water
        // level must be monotonically non-increasing from spring to mouth.
        let terrain = Terrain::new(77);
        let mut checked = 0;

        for spring in springs_near(&terrain, 10) {
            let course = trace(&terrain, spring);
            if course.len() < 8 {
                continue;
            }
            checked += 1;
            for pair in course.windows(2) {
                assert!(
                    pair[1].1 <= pair[0].1 + 1e-4,
                    "river level rose from {} to {}",
                    pair[0].1,
                    pair[1].1,
                );
            }
        }
        assert!(checked >= 3, "only {checked} usable rivers near the origin");
    }

    #[test]
    fn some_rivers_reach_the_sea() {
        // A world where every river dies in a pit would mean the pit handling is
        // eating the coast-bound majority.
        let terrain = Terrain::new(77);
        let mut reached = 0;
        let mut total = 0;

        for spring in springs_near(&terrain, 12) {
            let course = trace(&terrain, spring);
            if course.len() < 8 {
                continue;
            }
            total += 1;
            if course.last().unwrap().1 <= WATER_LEVEL + 0.6 {
                reached += 1;
            }
        }

        assert!(total >= 4, "too few rivers to judge: {total}");
        assert!(reached > 0, "none of {total} rivers reached the sea");
    }

    #[test]
    fn rivers_are_born_small() {
        // Maturity must ramp from a trickle to full width, never exceed 1, and
        // never fall back to nothing mid-course.
        let terrain = Terrain::new(77);
        for spring in springs_near(&terrain, 8).into_iter().take(4) {
            let course = trace(&terrain, spring);
            if course.len() < 12 {
                continue;
            }
            assert!(course[0].2 < 0.4, "a spring starts at {}", course[0].2);
            for pair in course.windows(2) {
                assert!(pair[1].2 >= pair[0].2 - 1e-4, "maturity shrank");
                assert!(pair[1].2 <= 1.0 + 1e-4);
            }
        }
    }

    #[test]
    fn traces_are_deterministic() {
        let a = Terrain::new(2024);
        let b = Terrain::new(2024);
        for spring in springs_near(&a, 6).into_iter().take(3) {
            let ta = trace(&a, spring);
            let tb = trace(&b, spring);
            assert_eq!(ta.len(), tb.len());
            for (pa, pb) in ta.iter().zip(tb.iter()) {
                assert_eq!(pa.0, pb.0);
                assert_eq!(pa.1, pb.1);
            }
        }
    }

    #[test]
    fn courses_bend_rather_than_zigzag() {
        // Momentum is meant to sweep the course. Without it, gradient descent on
        // noisy terrain reverses heading step to step, which reads as a scribble.
        let terrain = Terrain::new(77);
        for spring in springs_near(&terrain, 8).into_iter().take(4) {
            let course = trace(&terrain, spring);
            if course.len() < 12 {
                continue;
            }
            let mut reversals = 0;
            for window in course.windows(3) {
                let first = window[1].0 - window[0].0;
                let second = window[2].0 - window[1].0;
                if first.dot(second) < 0.0 {
                    reversals += 1;
                }
            }
            let rate = reversals as f32 / course.len() as f32;
            assert!(rate < 0.12, "course reverses {rate} of the time");
        }
    }
}
