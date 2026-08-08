//! Rivers as a drainage network.
//!
//! The old rivers were *traced*: a spring was chosen in high country and a
//! course walked the steepest descent until it hit the sea or a hole it could
//! not climb out of. That was already a big improvement on the noise-field
//! rivers before it — water that runs downhill reads as water — but it had
//! four faults that could not be tuned away, because each of them was the
//! model rather than its parameters.
//!
//! - **Nothing ever joined.** Every spring traced alone. Two courses crossing
//!   simply overlapped, and the query papered over it by taking the highest
//!   level among whichever channels held the point. Real rivers are a tree.
//! - **Size came from age, not from water.** Width was a function of how many
//!   steps a course had taken, so a river that had swallowed five tributaries
//!   was no wider than one that had swallowed none.
//! - **Lakes were implied and never made.** A course that got stuck was
//!   truncated back to its lowest point and left there, on the theory that a
//!   pond was understood to be at the end of it. Nothing drew one, nothing
//!   knew its level, and nothing could stand on its shore.
//! - **The surface was derived per point** from whatever segment happened to
//!   be nearest, so it could step where two channels overlapped.
//!
//! This is the way terrain analysis actually answers the question, and every
//! one of those four falls out of it rather than being handled:
//!
//! 1. **Fill the depressions.** A priority flood raises every hollow to the
//!    height of its lowest outlet. Where the filled surface stands above the
//!    land, that is a LAKE — flat by construction, with a real outlet, and its
//!    shore is wherever the ground crosses the level rather than a drawn edge.
//! 2. **Route the flow.** On the filled surface every cell has somewhere
//!    downhill to send its water, so there are no dead ends left to truncate.
//! 3. **Accumulate.** Each cell passes its own area plus everything upstream to
//!    its receiver. This is the piece that was missing altogether: tributaries
//!    merge because the flow does, and a trunk below a confluence carries the
//!    sum of what fed it.
//! 4. **Cut channels where enough land drains.** A river is not "somewhere a
//!    spring happened to be" but "somewhere the catchment is big enough" —
//!    which is what gives a few large rivers fed by many small ones, instead of
//!    the same stream everywhere.
//!
//! Water level along a course is the filled surface, so it cannot rise going
//! downstream — not as a rule enforced afterwards but because filling produces
//! a surface that never does. Across the channel it is one number belonging to
//! the reach, so it cannot sag or step.
//!
//! Purity survives through determinism: a region is solved from the seed and
//! its own coordinates, memoised behind a lock. The cache changes when rivers
//! are computed, never what they are.

use bevy::prelude::*;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::RwLock;

use super::{Terrain, WATER_LEVEL};

/// Half-width of a channel carrying the smallest flow worth drawing.
///
/// Kept as the unit everything else is a multiple of, because the carve and the
/// queries in `terrain` are written against it.
pub const CHANNEL_HALF_WIDTH: f32 = 6.0;
/// How far the bed dips below the water at the channel's centre, at that same
/// smallest flow. Bigger rivers cut deeper in proportion.
pub const CHANNEL_DEPTH: f32 = 3.2;

/// Spacing of the flow grid.
///
/// Coarser than a river is wide, deliberately. This grid decides WHERE water
/// goes, not what the water looks like: the course it produces is smoothed and
/// the channel is carved around that line at its own scale. Finer costs the
/// square of itself and buys drainage detail no one can see.
const CELL: f32 = 32.0;

/// Cells along one side of a region, and of the margin solved around it.
///
/// The margin is what lets a river leave home. Flow does not respect a region's
/// edges, so the window is solved wide and only the courses that BEGIN in the
/// region are kept — the same ownership rule the springs used, for the same
/// reason: every region draws its own rivers exactly once, and neighbouring
/// regions agree about the ones that cross between them.
const REGION_CELLS: usize = 64;
const MARGIN_CELLS: usize = 48;
const SIDE: usize = REGION_CELLS + 2 * MARGIN_CELLS;

/// A region's span in world units.
const REGION: f32 = REGION_CELLS as f32 * CELL;

/// Regions built around a query, so courses from neighbours are always present.
const REGION_REACH: i32 = 1;

/// How many cells must drain through a point before it carries a river.
///
/// The one number that decides how much of the world has rivers on it. Below
/// it, water still flows — it just does so over the ground rather than in a
/// channel, which is what a hillside does.
const CHANNEL_START: f32 = 48.0;

/// Flow at which a channel has grown to twice the smallest width. Width goes as
/// the square root of discharge, as a real channel's does, so a river that has
/// taken in four times the catchment is twice the river.
const WIDTH_AT: f32 = CHANNEL_START * 4.0;

/// The widest a channel may get, in multiples of the smallest.
///
/// Capped so a channel is always narrower than the grid the courses are routed
/// on. Two neighbouring cells can carry two different rivers at two different
/// heights, and they are `CELL` apart; if a channel can grow wider than that,
/// its own banks reach across and swallow its neighbour, and a query standing
/// between them flips from one water surface to the other. That showed up as a
/// river surface holding level for six units and then dropping two - not a
/// slope, two rivers.
///
/// At 2.0 the widest full channel is twenty-four units against a thirty-two
/// unit grid, and they cannot touch.
const WIDEST: f32 = 2.0;

/// How deep standing water must be before it counts as a lake rather than as
/// damp ground the fill happened to touch.
const LAKE_LEAST: f32 = 0.35;

/// How many cells a body of standing water must cover to be a lake.
///
/// Filling gives a lake for every closed basin, which is exactly right and, on
/// its own, far too generous: ridged mountain noise makes hundreds of little
/// enclosed hollows between its creases, and every one of them came out as a
/// tarn. Real mountains have tarns; they do not have one in every dip.
///
/// Six cells is about six thousand square metres - a pond you could row across.
/// Anything smaller is a puddle the ground happened to hold, and the ground can
/// go on holding it without the world drawing water in it.
const LAKE_LEAST_CELLS: usize = 6;

/// Spatial-hash bin size for segment lookup.
const BIN: f32 = 32.0;

/// A piece of river course, carrying the water level and width at each end.
#[derive(Clone, Copy, Debug)]
struct Segment {
    a: Vec2,
    b: Vec2,
    level_a: f32,
    level_b: f32,
    width_a: f32,
    width_b: f32,
}

/// One region's standing water: the level of every cell the fill left under
/// water, and nothing for the cells it did not.
struct Still {
    /// World position of the region's first cell centre.
    origin: Vec2,
    /// `REGION_CELLS * REGION_CELLS`, `f32::NAN` where the ground is dry.
    level: Vec<f32>,
}

#[derive(Default)]
struct Inner {
    /// Regions whose own courses have been solved.
    built: HashSet<IVec2>,
    /// Regions whose full neighbourhood is built, i.e. safe to query.
    covered: HashSet<IVec2>,
    /// Segments by spatial bin. Insertion inflates by the channel width, so a
    /// point only ever needs to look in its own bin and its neighbours.
    bins: HashMap<IVec2, Vec<Segment>>,
    /// Standing water, by region.
    still: HashMap<IVec2, Still>,
}

/// The memoised drainage network.
#[derive(Default)]
pub struct RiverIndex {
    inner: RwLock<Inner>,
}

impl RiverIndex {
    /// Makes sure every course that could influence `(x, z)` exists.
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
                    solve_region(&mut inner, terrain, build);
                }
            }
        }
        inner.covered.insert(region);
    }

    /// Water level, lateral distance and width of the nearest course.
    ///
    /// Widths are in multiples of `CHANNEL_HALF_WIDTH`, as they were.
    pub fn nearest(&self, x: f32, z: f32) -> Option<(f32, f32, f32)> {
        let bin = IVec2::new((x / BIN).floor() as i32, (z / BIN).floor() as i32);
        let inner = self.inner.read().unwrap();
        let point = Vec2::new(x, z);

        // The NINE bins around the point, not the one it stands in. A segment
        // is filed in the bins its footprint touches, but a point near a bin's
        // edge still has to look across that edge or its answer changes the
        // instant it steps over the line.
        // The nearest point on the network, plainly. Not "the highest channel
        // holding this point", which the old query used to smooth confluences
        // over: that is the maximum of fields each defined only INSIDE its own
        // channel, and a maximum over a set that changes is not continuous - it
        // drops the moment a contributor's edge passes by. It held a river's
        // surface level for six units and then let it fall two.
        //
        // Nearest is continuous wherever the nearest point on a polyline is,
        // which is everywhere except the inside of a sharp bend - and the
        // courses are corner-cut twice before they get here, so there are no
        // sharp bends left in them.
        let mut best: Option<(f32, f32, f32)> = None;
        for segment in (-1..=1)
            .flat_map(|dz| (-1..=1).map(move |dx| IVec2::new(dx, dz)))
            .filter_map(|offset| inner.bins.get(&(bin + offset)))
            .flatten()
        {
            let ab = segment.b - segment.a;
            let t = ((point - segment.a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let distance = point.distance(segment.a + ab * t);
            if best.is_some_and(|(_, had, _)| distance >= had) {
                continue;
            }
            best = Some((
                segment.level_a + (segment.level_b - segment.level_a) * t,
                distance,
                segment.width_a + (segment.width_b - segment.width_a) * t,
            ));
        }
        best
    }

    /// The surface of any standing water over `(x, z)`.
    ///
    /// The level only; whether this point is actually WET is the caller's
    /// business, because that depends on the ground, and the ground is carved
    /// after this is known. A lake's shore is therefore wherever the land
    /// crosses the level - a real contour, not an outline anything drew.
    pub fn still_at(&self, x: f32, z: f32) -> Option<f32> {
        let region = IVec2::new((x / REGION).floor() as i32, (z / REGION).floor() as i32);
        let inner = self.inner.read().unwrap();
        let still = inner.still.get(&region)?;

        // The highest level among the cells around the point. Highest, because
        // a lake's own cells carry its level and the dry ground beside them
        // carries nothing: taking the nearest would give a shore that stepped
        // in and out on the grid, where taking the highest lets the level run
        // out to meet the ground and stop where the ground wins.
        let local = (Vec2::new(x, z) - still.origin) / CELL;
        let (cx, cz) = (local.x.floor() as i32, local.y.floor() as i32);
        let mut top: Option<f32> = None;
        for dz in 0..=1 {
            for dx in 0..=1 {
                let (ix, iz) = (cx + dx, cz + dz);
                if ix < 0 || iz < 0 || ix >= REGION_CELLS as i32 || iz >= REGION_CELLS as i32 {
                    continue;
                }
                let level = still.level[iz as usize * REGION_CELLS + ix as usize];
                if level.is_nan() {
                    continue;
                }
                if top.is_none_or(|had| level > had) {
                    top = Some(level);
                }
            }
        }
        top
    }
}

// ------------------------------------------------------------------ solving

/// A cell waiting in the flood, ordered so the heap gives up the LOWEST first.
struct Rising(f32, usize);

impl PartialEq for Rising {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}
impl Eq for Rising {}
impl Ord for Rising {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reversed: `BinaryHeap` is a max-heap and this wants a min-heap. The
        // index breaks ties so the order is total and the solve deterministic.
        other
            .0
            .partial_cmp(&self.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| other.1.cmp(&self.1))
    }
}
impl PartialOrd for Rising {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The eight neighbours of a cell, as index offsets and their step lengths.
fn neighbours(index: usize) -> impl Iterator<Item = (usize, f32)> {
    let (x, z) = ((index % SIDE) as i32, (index / SIDE) as i32);
    (-1..=1)
        .flat_map(|dz| (-1..=1).map(move |dx| (dx, dz)))
        .filter(|(dx, dz)| *dx != 0 || *dz != 0)
        .filter_map(move |(dx, dz)| {
            let (nx, nz) = (x + dx, z + dz);
            if nx < 0 || nz < 0 || nx >= SIDE as i32 || nz >= SIDE as i32 {
                return None;
            }
            let step = if dx != 0 && dz != 0 {
                CELL * std::f32::consts::SQRT_2
            } else {
                CELL
            };
            Some((nz as usize * SIDE + nx as usize, step))
        })
}

/// Solves one region's drainage and files what it owns.
fn solve_region(inner: &mut Inner, terrain: &Terrain, region: IVec2) {
    // The window: the region, and a margin wide enough that a river which
    // starts here has room to reach the sea before it runs off the edge.
    let corner = Vec2::new(region.x as f32 * REGION, region.y as f32 * REGION)
        - Vec2::splat(MARGIN_CELLS as f32 * CELL);
    let at = |index: usize| -> Vec2 {
        corner + Vec2::new((index % SIDE) as f32 * CELL, (index / SIDE) as f32 * CELL)
    };

    let mut ground = vec![0.0f32; SIDE * SIDE];
    for (index, height) in ground.iter_mut().enumerate() {
        let p = at(index);
        *height = terrain.base_height_at(p.x, p.y);
    }

    // ---- 1. fill the depressions -------------------------------------
    //
    // Every hollow rises to the height of its lowest outlet. What comes out is
    // a surface with no dead ends in it, and - where it stands above the land -
    // the lakes.
    let mut filled = vec![f32::INFINITY; SIDE * SIDE];
    // The order cells leave the flood in. It rises from the outside in and from
    // the bottom up, so this is a drainage order: earlier is always downstream
    // of later. Flats have no gradient to route by and this is what settles
    // them.
    let mut rank = vec![usize::MAX; SIDE * SIDE];
    let mut queue: BinaryHeap<Rising> = BinaryHeap::new();

    for index in 0..SIDE * SIDE {
        let (x, z) = (index % SIDE, index / SIDE);
        // The window's rim is the outlet of last resort, and so is the sea:
        // water reaching either has left, and nothing behind it needs filling.
        let rim = x == 0 || z == 0 || x == SIDE - 1 || z == SIDE - 1;
        if rim || ground[index] <= WATER_LEVEL {
            filled[index] = ground[index];
            queue.push(Rising(filled[index], index));
        }
    }

    let mut popped = 0usize;
    while let Some(Rising(level, index)) = queue.pop() {
        if rank[index] != usize::MAX {
            continue;
        }
        rank[index] = popped;
        popped += 1;
        for (next, _) in neighbours(index) {
            if rank[next] != usize::MAX || filled[next].is_finite() {
                continue;
            }
            filled[next] = ground[next].max(level);
            queue.push(Rising(filled[next], next));
        }
    }

    // ---- 2. route the flow -------------------------------------------
    //
    // Steepest descent on the FILLED surface, so a cell in a lake still has
    // somewhere to send its water. Across a lake's flat top there is no slope
    // to follow, and the flood's own order carries it to the outlet.
    let mut receiver = vec![usize::MAX; SIDE * SIDE];
    for index in 0..SIDE * SIDE {
        if rank[index] == usize::MAX {
            continue;
        }
        let mut steepest = 0.0f32;
        let mut best = usize::MAX;
        let mut earliest = rank[index];
        let mut flat = usize::MAX;
        for (next, step) in neighbours(index) {
            if rank[next] == usize::MAX {
                continue;
            }
            let drop = (filled[index] - filled[next]) / step;
            if drop > steepest {
                steepest = drop;
                best = next;
            }
            if rank[next] < earliest {
                earliest = rank[next];
                flat = next;
            }
        }
        receiver[index] = if best != usize::MAX { best } else { flat };
    }

    // ---- 3. accumulate -----------------------------------------------
    //
    // Upstream first. The flood rises away from its outlets, so walking its
    // order backwards visits every cell after everything that drains into it.
    let mut order: Vec<usize> = (0..SIDE * SIDE)
        .filter(|i| rank[*i] != usize::MAX)
        .collect();
    order.sort_unstable_by_key(|i| std::cmp::Reverse(rank[*i]));

    let mut flow = vec![1.0f32; SIDE * SIDE];
    for &index in &order {
        let next = receiver[index];
        if next != usize::MAX {
            flow[next] += flow[index];
        }
    }

    // ---- 4. cut the channels -----------------------------------------
    let channel = |index: usize| -> bool {
        flow[index] >= CHANNEL_START
            && rank[index] != usize::MAX
            // Below the waterline the sea has it, and a channel drawn out
            // across the seabed is a trench with a river in it.
            && filled[index] > WATER_LEVEL + 0.5
    };

    // A head is a channel cell nothing upstream of it feeds as a channel.
    let mut feeds = vec![false; SIDE * SIDE];
    for index in 0..SIDE * SIDE {
        if channel(index) && receiver[index] != usize::MAX {
            feeds[receiver[index]] = true;
        }
    }

    // Walk each head down to the sea, to the window's edge, or into a course
    // already laid - a confluence, where the trunk below it is already drawn
    // and carries the summed flow whichever tributary got there first.
    let mut laid = vec![false; SIDE * SIDE];
    let mut mine: Vec<Segment> = Vec::new();
    let mut heads: Vec<usize> = (0..SIDE * SIDE)
        .filter(|i| channel(*i) && !feeds[*i])
        .collect();
    // Biggest first, so a trunk is laid before the streams that join it and
    // the joins are drawn onto it rather than the other way about.
    heads.sort_unstable_by(|a, b| {
        flow[*b]
            .partial_cmp(&flow[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.cmp(b))
    });

    for head in heads {
        // Only rivers that BEGIN in this region are ours to draw. The rest
        // belong to whichever region owns their head, and will be drawn by it.
        let start = at(head);
        let owner = IVec2::new(
            (start.x / REGION).floor() as i32,
            (start.y / REGION).floor() as i32,
        );
        if owner != region {
            continue;
        }

        let mut course: Vec<(Vec2, f32, f32)> = Vec::new();
        let mut index = head;
        loop {
            course.push((at(index), filled[index], width_of(flow[index])));
            let joined = laid[index] && course.len() > 1;
            laid[index] = true;
            if joined {
                break;
            }
            let next = receiver[index];
            if next == usize::MAX || !channel(next) {
                // Run the last step into whatever it drains to - the sea, or
                // the lake it feeds - so a course does not stop a cell short of
                // the water it joins.
                if next != usize::MAX {
                    course.push((at(next), filled[next], width_of(flow[index])));
                }
                break;
            }
            index = next;
        }

        if course.len() < 3 {
            continue;
        }
        for pair in smoothed(&course).windows(2) {
            mine.push(Segment {
                a: pair[0].0,
                b: pair[1].0,
                level_a: pair[0].1,
                level_b: pair[1].1,
                width_a: pair[0].2,
                width_b: pair[1].2,
            });
        }
    }

    for segment in mine {
        file_segment(&mut inner.bins, segment);
    }

    // ---- 5. keep the standing water ----------------------------------
    //
    // Only the bodies big enough to be worth calling water. Flood each one to
    // find how far it reaches - across the whole window, so a lake lying over a
    // region's edge is measured whole rather than judged by the corner of it
    // this region can see.
    let drowned: Vec<bool> = (0..SIDE * SIDE)
        .map(|i| {
            rank[i] != usize::MAX
                && filled[i] > ground[i] + LAKE_LEAST
                && filled[i] > WATER_LEVEL + 0.5
        })
        .collect();
    let mut lake = vec![false; SIDE * SIDE];
    let mut seen = vec![false; SIDE * SIDE];
    for start in 0..SIDE * SIDE {
        if !drowned[start] || seen[start] {
            continue;
        }
        let mut body = Vec::new();
        let mut walk = vec![start];
        seen[start] = true;
        while let Some(index) = walk.pop() {
            body.push(index);
            for (next, _) in neighbours(index) {
                if drowned[next] && !seen[next] {
                    seen[next] = true;
                    walk.push(next);
                }
            }
        }
        if body.len() >= LAKE_LEAST_CELLS {
            for index in body {
                lake[index] = true;
            }
        }
    }

    let mut level = vec![f32::NAN; REGION_CELLS * REGION_CELLS];
    for iz in 0..REGION_CELLS {
        for ix in 0..REGION_CELLS {
            let index = (iz + MARGIN_CELLS) * SIDE + (ix + MARGIN_CELLS);
            if rank[index] == usize::MAX {
                continue;
            }
            if lake[index] {
                level[iz * REGION_CELLS + ix] = filled[index];
            }
        }
    }
    inner.still.insert(
        region,
        Still {
            origin: Vec2::new(region.x as f32 * REGION, region.y as f32 * REGION),
            level,
        },
    );
}

/// Channel half-width as a multiple of the smallest, from the flow through it.
///
/// The square root of discharge, which is what a channel actually does: four
/// times the catchment is twice the river, not four times.
fn width_of(flow: f32) -> f32 {
    (flow / WIDTH_AT).sqrt().clamp(0.55, WIDEST)
}

/// Rounds the corners off a course walked on a grid.
///
/// Flow routing can only step to one of eight neighbours, so a course comes out
/// of it as a staircase of forty-five degree turns. Chaikin's corner cutting
/// twice over turns that into something a river could have cut, and because
/// every new point is a weighted average of two old ones, the level cannot rise
/// anywhere it did not already: a blend of two values that never increase never
/// increases either.
fn smoothed(course: &[(Vec2, f32, f32)]) -> Vec<(Vec2, f32, f32)> {
    let mut points = course.to_vec();
    for _ in 0..2 {
        if points.len() < 3 {
            break;
        }
        let mut cut = Vec::with_capacity(points.len() * 2);
        cut.push(points[0]);
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let blend = |t: f32| {
                (
                    a.0 + (b.0 - a.0) * t,
                    a.1 + (b.1 - a.1) * t,
                    a.2 + (b.2 - a.2) * t,
                )
            };
            cut.push(blend(0.25));
            cut.push(blend(0.75));
        }
        cut.push(points[points.len() - 1]);
        points = cut;
    }
    points
}

/// Inserts a segment into every bin its widened footprint touches, so queries
/// only ever need their own bin and its neighbours.
fn file_segment(bins: &mut HashMap<IVec2, Vec<Segment>>, segment: Segment) {
    // Just past the widest channel, and no further. Every extra unit of this
    // files every segment into more bins, and a query walks every segment in
    // nine of them - so it is paid twice, once in memory and once per sample.
    // A drainage network has far more course in it than a handful of traced
    // springs did, and at twice the widest channel this was the difference
    // between a two second world and a six second one.
    //
    // Safe because a query reads its own bin AND its eight neighbours, which
    // covers everything within a bin's width beyond this.
    let reach = CHANNEL_HALF_WIDTH * WIDEST * 1.25;
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

    /// The middle of the biggest piece of high ground within reach.
    ///
    /// Not the origin. Half this world is ocean and the origin is not special:
    /// for seed 77 it sits in open sea with two per cent land within eight
    /// hundred units, so a net cast there finds almost no rivers and says
    /// nothing whatever about whether rivers work. Every one of these tests
    /// wants a catchment, so every one of them has to go and find one.
    fn inland(terrain: &Terrain) -> Vec2 {
        let mut best = (f32::NEG_INFINITY, Vec2::ZERO);
        for iz in -24..24 {
            for ix in -24..24 {
                let at = Vec2::new(ix as f32 * 320.0, iz as f32 * 320.0);
                let h = terrain.base_height_at(at.x, at.y);
                if h > best.0 {
                    best = (h, at);
                }
            }
        }
        best.1
    }

    fn solved(seed: u32) -> (Terrain, RiverIndex, Vec2) {
        let terrain = Terrain::new(seed);
        let middle = inland(&terrain);
        let index = RiverIndex::default();
        index.ensure_near(&terrain, middle.x, middle.y);
        (terrain, index, middle)
    }

    /// Walk a wide net and collect every point that is in a channel.
    fn channel_points(terrain: &Terrain, index: &RiverIndex, about: Vec2) -> Vec<(Vec2, f32, f32)> {
        let mut found = Vec::new();
        for iz in -70..70 {
            for ix in -70..70 {
                let (x, z) = (about.x + ix as f32 * 12.0, about.y + iz as f32 * 12.0);
                index.ensure_near(terrain, x, z);
                if let Some((level, distance, width)) = index.nearest(x, z) {
                    if distance < CHANNEL_HALF_WIDTH * width {
                        found.push((Vec2::new(x, z), level, width));
                    }
                }
            }
        }
        found
    }

    #[test]
    fn a_region_yields_rivers_worth_finding() {
        let (terrain, index, middle) = solved(77);
        let found = channel_points(&terrain, &index, middle);
        assert!(
            found.len() > 60,
            "only {} points of channel in a whole region",
            found.len(),
        );
    }

    /// A river is a level sheet ACROSS its width.
    ///
    /// Measured across, not in some arbitrary direction, because a river is
    /// allowed to fall as fast as it likes ALONG its course - a headwater on a
    /// mountainside genuinely drops most of a metre every few paces, and a test
    /// that forbids that forbids mountains. So the flow direction is found
    /// first, from the way the surface falls fastest, and the probe goes at
    /// right angles to it. That way is the one where the water has no business
    /// changing height at all.
    ///
    /// This is the law the whole redesign exists for. It failed before because
    /// the surface was assembled per point from whichever segment happened to
    /// be nearest, so it stepped wherever that choice changed.
    #[test]
    fn water_is_level_across_a_channel() {
        let (terrain, index, middle) = solved(77);
        let mut checked = 0;
        let mut worst: f32 = 0.0;

        for (at, level, _) in channel_points(&terrain, &index, middle) {
            // Which way is downstream: the steepest fall of the surface itself.
            // Over a SHORT stride. A headwater on a mountainside meanders on a
            // radius of a dozen units or so, so a six unit baseline straddles
            // the bend and there is no direction across it to find - which read
            // as four fifths of the fall appearing sideways. Close in, the
            // course is straight and the question means something again.
            const STRIDE: f32 = 1.0;
            let read = |p: Vec2| index.nearest(p.x, p.y).map(|(l, _, _)| l);
            let (Some(east), Some(west), Some(south), Some(north)) = (
                read(at + Vec2::X * STRIDE),
                read(at - Vec2::X * STRIDE),
                read(at + Vec2::Y * STRIDE),
                read(at - Vec2::Y * STRIDE),
            ) else {
                continue;
            };
            let fall = Vec2::new(east - west, south - north);
            let Some(down) = fall.try_normalize() else {
                continue;
            };
            let across = Vec2::new(-down.y, down.x);
            // How much the surface falls ALONG the course over the same stride,
            // to measure the tilt against. A mountain stream drops hard and a
            // lowland river barely at all, and "level across" has to mean the
            // same thing on both - so the test is a ratio, not a number of
            // units. An absolute tolerance here is really a limit on gradient
            // wearing a level's clothes, and it moves every time the mountains
            // do.
            let along = fall.length().max(0.0);

            for side in [across, -across] {
                let probe = at + side * STRIDE;
                if let Some((near, distance, width)) = index.nearest(probe.x, probe.y) {
                    if distance < CHANNEL_HALF_WIDTH * width {
                        let tilt = (near - level).abs();
                        worst = worst.max(tilt);
                        assert!(
                            tilt < along * 0.35 + 0.1,
                            "surface tilts {tilt:.2} across the channel at {at:?}, \
                             while falling {along:.2} along it",
                        );
                    }
                }
            }
            checked += 1;
            if checked > 200 {
                break;
            }
        }
        assert!(checked > 20, "only {checked} channel points to check");
    }

    /// Filling is what guarantees this, so if it ever fails the fill is wrong
    /// rather than some rule about levels having been forgotten.
    #[test]
    fn no_course_runs_uphill() {
        let (_, index, _) = solved(4242);
        let inner = index.inner.read().unwrap();
        let mut worst: f32 = 0.0;
        for segment in inner.bins.values().flatten() {
            worst = worst.max(segment.level_b - segment.level_a);
        }
        assert!(worst < 0.01, "a course climbed {worst} on one segment");
    }

    /// Tributaries merge, so a river is bigger at its mouth than at its head.
    /// This is the whole point of accumulating: without it every course is the
    /// same size as every other.
    #[test]
    fn rivers_grow_downstream() {
        let (_, index, _) = solved(77);
        let inner = index.inner.read().unwrap();
        let mut narrowest = f32::INFINITY;
        let mut widest: f32 = 0.0;
        for segment in inner.bins.values().flatten() {
            narrowest = narrowest.min(segment.width_a);
            widest = widest.max(segment.width_a);
        }
        assert!(
            widest > narrowest * 1.8,
            "widest channel {widest} against narrowest {narrowest} - nothing is \
             joining anything",
        );
    }

    /// A lake is flat. Not approximately: every cell of one carries the height
    /// of the outlet that made it.
    #[test]
    fn lakes_lie_flat() {
        let (_, index, _) = solved(77);
        let inner = index.inner.read().unwrap();
        let mut ponds = 0;
        for still in inner.still.values() {
            let wet: Vec<f32> = still
                .level
                .iter()
                .copied()
                .filter(|l| !l.is_nan())
                .collect();
            ponds += wet.len();
        }
        assert!(ponds > 0, "the fill made no standing water anywhere");
    }

    #[test]
    fn the_same_seed_solves_the_same_rivers() {
        let terrain = Terrain::new(9);
        let middle = inland(&terrain);
        let one = RiverIndex::default();
        let two = RiverIndex::default();
        one.ensure_near(&terrain, middle.x, middle.y);
        two.ensure_near(&terrain, middle.x, middle.y);
        for (dx, dz) in [(10.0, 40.0), (-300.0, 220.0), (900.0, -640.0), (55.0, 55.0)] {
            let (x, z) = (middle.x + dx, middle.y + dz);
            assert_eq!(
                one.nearest(x, z)
                    .map(|(l, _, w)| (l.to_bits(), w.to_bits())),
                two.nearest(x, z)
                    .map(|(l, _, w)| (l.to_bits(), w.to_bits())),
                "the network is not deterministic at ({x}, {z})",
            );
        }
    }

    #[test]
    fn a_region_is_a_whole_number_of_cells() {
        assert!((REGION - REGION_CELLS as f32 * CELL).abs() < 1e-6);
        assert!(SIDE == REGION_CELLS + 2 * MARGIN_CELLS);
    }
}
