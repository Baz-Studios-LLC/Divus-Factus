//! Walking around obstacles.
//!
//! Creatures used to steer straight at their destination, which was survivable on a
//! small island and is not on a coastline with bays, lakes and mountain ranges: a
//! villager aiming at a bush across an inlet walks into the water and stays there.
//!
//! The search is A* over a grid sampled from the terrain function on demand. There is
//! no navmesh and no baked grid, because there is no bounded world to bake — the same
//! property that makes the terrain streamable makes it queryable anywhere, so the
//! search generates exactly the cells it visits and discards them afterwards.
//!
//! Every search is bounded by a node budget. An unreachable goal — across an ocean,
//! or up a cliff — must fail quickly and cheaply rather than flood-filling a
//! continent, because in an endless world it would otherwise never terminate.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::terrain::Terrain;

/// World units per grid cell. Fine enough to thread a gap between rocks, coarse
/// enough that crossing a settlement is tens of cells rather than hundreds.
pub const CELL: f32 = 2.5;

/// Maximum cells a single search may expand.
pub const DEFAULT_BUDGET: usize = 3_000;

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Walls>()
            .init_resource::<Reachable>()
            // Before the routes that read it, and in the same frame a building
            // finishes: a hall that is standing but not yet an obstacle is a
            // hall people walk through for as long as their current route
            // lasts.
            .add_systems(
                Update,
                (survey_the_walls, chart_the_ground).before(crate::creature::plan_routes),
            )
            .add_systems(Update, watch_the_walls.after(crate::creature::plan_routes));
    }
}

/// `DIVUS_FACTUS_WALL_PROBE=1`: counts walks that go through a wall.
///
/// Brett: "a lot of people still walk through the walls." The search has
/// known about buildings for a while and the door router bends the last
/// stride, so the question is not WHETHER it happens but which of the two
/// is letting it - and a rate is the only honest answer. Every stride is
/// tested against every footprint, split three ways: a building they were
/// only passing (a search failure), their own building with the router
/// steering (a bend that lost the race), and their own building with no
/// router at all (a bend that never happened).
fn watch_the_walls(
    walls: Res<Walls>,
    time: Res<Time>,
    mut on: Local<Option<bool>>,
    mut said: Local<f32>,
    mut tally: Local<(u32, u32, u32, u32)>,
    mut last: Local<std::collections::HashMap<Entity, Vec2>>,
    walkers: Query<
        (Entity, &Transform, Has<crate::villager::home::Doorbound>),
        (
            With<crate::villager::Villager>,
            Without<crate::creature::Corpse>,
        ),
    >,
) {
    if !*on.get_or_insert_with(|| std::env::var("DIVUS_FACTUS_WALL_PROBE").is_ok()) {
        return;
    }
    for (who, at, doorbound) in &walkers {
        let here = Vec2::new(at.translation.x, at.translation.z);
        let Some(there) = last.insert(who, here) else {
            continue;
        };
        if there.distance(here) < 0.01 {
            continue;
        }
        tally.0 += 1;
        // Two different questions, and only the second is the one the
        // player asks. A stride that crosses a building the walker is
        // neither leaving nor entering is a search failure. A stride
        // that crosses the wall of their OWN destination is excused by
        // the search on purpose - no path on a two-and-a-half metre grid
        // can thread a one metre door - and is meant to be bent through
        // the doorway by `home::use_doors`. When that bending fails, what
        // you SEE is a person walking through their own wall, which is
        // what Brett is reporting.
        let excused = walls.excused(there, here);
        for (slot, building) in walls.buildings.iter().enumerate().take(MOST_BUILDINGS) {
            if !building.crosses(there, here) {
                continue;
            }
            if !excused[slot] {
                tally.1 += 1;
            } else if doorbound {
                // The router HAS them and they are still in the wall: the
                // bend is aimed wrong, or the walk beat it to the wall.
                tally.2 += 1;
            } else {
                // The router never took them at all.
                tally.3 += 1;
            }
            break;
        }
    }
    *said += time.delta_secs();
    if *said < 10.0 {
        return;
    }
    *said = 0.0;
    let (strides, through, bound, adrift) = *tally;
    *tally = (0, 0, 0, 0);
    info!(
        "wall probe: of {strides} strides, {through} cut through a building they were only \
         passing; {bound} went through their own wall with the door router steering, \
         {adrift} with no router at all",
    );
}

// ------------------------------------------------------- what can be reached

/// Cells charted per frame while the map is being drawn.
///
/// The charting is a flood fill over tens of thousands of cells, and each one
/// costs a terrain query - noise plus a walk of the drainage bins. Done in one
/// go that is a visible hitch, so it is spread instead: the map is simply
/// incomplete for a second or so after a village is founded, and an incomplete
/// map answers "I do not know" and costs nothing.
const CHARTED_PER_FRAME: usize = 900;

/// How far around the village the ground is charted, in world units.
///
/// Errands are bounded - `WORK_REACH` is a hundred and seventy, the foresters'
/// radius a hundred and ninety - so a map of the ground a village actually uses
/// is a bounded thing even though the world is not.
const CHARTED_REACH: f32 = 300.0;

/// How often the whole map is drawn again, in seconds of real time.
///
/// The world does change: ground gets terraced, a dock reaches out over water,
/// a quarry cuts a pit. All of those can JOIN two shores that were separate,
/// and a stale map would go on refusing an errand that has become possible. So
/// it is redrawn rather than patched - the whole thing costs a second of
/// background charting, and being occasionally slow to notice a new bridge is
/// the only failure it can have.
const REDRAW_AFTER: f32 = 30.0;

/// Which island of walkable ground each cell belongs to.
///
/// The point of it: **a search that fails is the most expensive search there
/// is.** It expands its entire budget - three thousand cells - before it can
/// say no, and a villager who wants something across a bay asks again the next
/// frame. That is what `creature::plan_routes` was doing at seventy-six
/// milliseconds of an eighty-one millisecond frame.
///
/// With the ground charted, "can I get there" is two lookups and an integer
/// comparison, and the failing search never runs at all. This is the standard
/// answer to the problem - label the connected regions, compare the labels -
/// and the reason it took a workaround first is that the textbook version
/// assumes a bounded, baked map and this world has neither.
///
/// **It is deliberately a one-way test.** The chart is drawn from the TERRAIN
/// alone and knows nothing about buildings, so it describes a world with strictly
/// more ways through than the real one. Different regions therefore mean "no
/// path exists" and can be trusted; the same region means only "maybe", and the
/// search runs exactly as it always did. A building can never join two shores,
/// so nothing is lost by leaving them out - and leaving them out is what lets
/// one chart serve every errand, since walls are excused per errand (see
/// [`Walls::excused`]) and a chart that knew about them would be wrong for
/// whoever the excusal was for.
#[derive(Resource, Default)]
pub struct Reachable {
    /// Cell to the island it belongs to. Absent means unwalkable or uncharted -
    /// the two are told apart by `covers`.
    island: HashMap<Cell, u32>,
    /// The middle of the charted window, and how far it reaches in cells.
    centre: Cell,
    reach: i32,
    /// Cells still to visit in the fill being drawn, and what is known so far.
    frontier: Vec<(Cell, u32)>,
    drawing: Option<(HashMap<Cell, u32>, u32, Cell, i32)>,
    /// Where the sweep for fresh seeds has got to, as an index into the window.
    swept: i32,
    /// Real seconds since the standing chart was finished.
    age: f32,
}

impl Reachable {
    /// Whether this position is inside the charted window at all.
    fn covers(&self, at: Vec3) -> bool {
        if self.reach == 0 {
            return false;
        }
        let cell = to_cell(at);
        let away = cell - self.centre;
        away.x.abs() <= self.reach && away.y.abs() <= self.reach
    }

    /// Whether the search can be spared: `true` only when both ends are charted
    /// and they are on different islands.
    ///
    /// Every other case answers `false` and the search runs, which is what makes
    /// this safe to be wrong about.
    pub fn hopeless(&self, from: Vec3, to: Vec3) -> bool {
        if !self.covers(from) || !self.covers(to) {
            return false;
        }
        match (
            self.island.get(&to_cell(from)),
            self.island.get(&to_cell(to)),
        ) {
            (Some(here), Some(there)) => here != there,
            // An unwalkable end is the search's own business - it has a cheap
            // test for that already and gives a better answer than this can.
            _ => false,
        }
    }

    /// How much ground is charted, for the developer's panel.
    // Not wired into the panel yet; the chart earns a row when the panel
    // next grows one.
    #[allow(dead_code)]
    pub fn charted(&self) -> usize {
        self.island.len()
    }
}

/// Draws the chart of what can be reached, a few hundred cells a frame.
fn chart_the_ground(
    time: Res<Time<Real>>,
    terrain: Option<Res<Terrain>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    mut chart: ResMut<Reachable>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("navigation: chart_the_ground");
    let (Some(terrain), Some(site)) = (terrain, site) else {
        return;
    };

    // Start a fresh chart when there is none, when the village has moved out
    // from under the old one, or when the standing one has gone stale.
    if chart.drawing.is_none() {
        let centre = to_cell(site.centre);
        let reach = (CHARTED_REACH / CELL) as i32;
        let moved = (centre - chart.centre).abs().max_element() > reach / 3;
        chart.age += time.delta_secs();
        if chart.reach != 0 && !moved && chart.age < REDRAW_AFTER {
            return;
        }
        chart.drawing = Some((HashMap::default(), 0, centre, reach));
        chart.frontier.clear();
        chart.swept = 0;
    }

    let Some((mut found, mut islands, centre, reach)) = chart.drawing.take() else {
        return;
    };
    let mut frontier = std::mem::take(&mut chart.frontier);
    let mut swept = chart.swept;
    let side = reach * 2 + 1;
    let mut spent = 0;

    while spent < CHARTED_PER_FRAME {
        // Spread whatever island is already growing before starting another.
        if let Some((cell, island)) = frontier.pop() {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let next = cell + IVec2::new(dx, dz);
                    let away = next - centre;
                    if away.x.abs() > reach || away.y.abs() > reach {
                        continue;
                    }
                    if found.contains_key(&next) {
                        continue;
                    }
                    spent += 1;
                    let (x, z) = (next.x as f32 * CELL, next.y as f32 * CELL);
                    if !terrain.is_walkable(x, z) {
                        continue;
                    }
                    // Diagonals join only where the search would take one, so
                    // the chart's idea of connected is the pathfinder's own.
                    // Read it the other way and the chart would promise a way
                    // through a gap `step_cost` refuses to cut.
                    if dx != 0 && dz != 0 {
                        let side_a = IVec2::new(next.x, cell.y);
                        let side_b = IVec2::new(cell.x, next.y);
                        let open =
                            |c: Cell| terrain.is_walkable(c.x as f32 * CELL, c.y as f32 * CELL);
                        if !open(side_a) || !open(side_b) {
                            continue;
                        }
                    }
                    found.insert(next, island);
                    frontier.push((next, island));
                }
            }
            continue;
        }

        // No island growing: sweep on for a walkable cell nobody has claimed.
        if swept >= side * side {
            break;
        }
        let cell = centre + IVec2::new(swept % side - reach, swept / side - reach);
        swept += 1;
        spent += 1;
        if found.contains_key(&cell) {
            continue;
        }
        let (x, z) = (cell.x as f32 * CELL, cell.y as f32 * CELL);
        if !terrain.is_walkable(x, z) {
            continue;
        }
        islands += 1;
        found.insert(cell, islands);
        frontier.push((cell, islands));
    }

    if swept >= side * side && frontier.is_empty() {
        // Finished: the new chart takes over from the old one whole, so nobody
        // ever reads a half-drawn map and is told the village is an island.
        chart.island = found;
        chart.centre = centre;
        chart.reach = reach;
        chart.age = 0.0;
        chart.drawing = None;
        chart.frontier.clear();
        chart.swept = 0;
    } else {
        chart.drawing = Some((found, islands, centre, reach));
        chart.frontier = frontier;
        chart.swept = swept;
    }
}

/// Gathers every standing building's footprint, once, when the village changes
/// shape.
///
/// Rebuilt only when a shell appears or moves rather than every frame: a
/// village's walls are the most static thing in the world, and walking the
/// whole list each tick to rediscover that nothing had changed would be the
/// cheapest possible way to spend a millisecond.
fn survey_the_walls(
    mut walls: ResMut<Walls>,
    // `Transform`, and this matters more than it looks: the search runs in the
    // sim's FLAT coordinates, and a building's `GlobalTransform` has been seated
    // on the sphere by the bend. Surveying the walls from the global would have
    // put every footprint thousands of units from the ground it stands on, and
    // blocked a patch of empty world instead. `home::use_doors` reads the same
    // component for the same reason.
    shells: Query<(&Transform, &crate::villager::work::Shell)>,
    changed: Query<
        Entity,
        (
            With<crate::villager::work::Shell>,
            Or<(Changed<Transform>, Added<crate::villager::work::Shell>)>,
        ),
    >,
    mut standing: Local<usize>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("navigation: survey_the_walls");
    let count = shells.iter().count();
    if changed.is_empty() && count == *standing {
        return;
    }
    *standing = count;
    walls.buildings.clear();
    walls.buildings.extend(shells.iter().map(|(at, shell)| {
        Footprint {
            at: Vec2::new(at.translation.x, at.translation.z),
            // A shoulder's width in from the wall itself, so a walk routed
            // around a building does not graze it - and, more importantly, so
            // that standing a hair inside the wall is still a place a route can
            // begin from.
            half: Vec2::new(shell.half_w - 0.35, shell.half_d - 0.35).max(Vec2::splat(0.2)),
            yaw: at.rotation.to_euler(EulerRot::YXZ).0,
        }
    }));
}

/// A building's footprint on the ground, in world space.
///
/// Walls were never in the search. It knew about water and cliffs and nothing
/// built, so a villager crossing the village walked through the hall — the door
/// router only ever bent a walk with one end INSIDE a building and one outside,
/// and a walk that merely passes through has both ends outside. Brett saw people
/// and animals alike stroll through walls.
#[derive(Clone, Copy)]
pub struct Footprint {
    pub at: Vec2,
    pub half: Vec2,
    /// The building's turn about Y, so a hall at an angle blocks its own
    /// rectangle rather than the box that contains it.
    pub yaw: f32,
}

impl Footprint {
    /// Whether a point is inside this footprint once it is grown by
    /// `margin`. The footprint is drawn a shoulder's width INSIDE the
    /// wall so a route never grazes the building, which means a point in
    /// the wall's own thickness is "outside" it - fine for deciding
    /// where a walk may go, wrong for deciding which building a walk
    /// belongs to.
    pub fn within(&self, p: Vec2, margin: f32) -> bool {
        let (sin, cos) = (-self.yaw).sin_cos();
        let local = p - self.at;
        let turned = Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos);
        turned.x.abs() < self.half.x + margin && turned.y.abs() < self.half.y + margin
    }

    /// Whether a straight walk from `a` to `b` passes through this footprint at
    /// any point.
    ///
    /// Asked of the SEGMENT, not of points along it. Straightening used to
    /// sample the line every half cell and ask `contains` at each sample, which
    /// cannot answer this: a line that nicks a corner between two samples is
    /// clipping the building and every sample says it is not. No step is small
    /// enough to fix that, because a clip can be as thin as you like.
    ///
    /// The slab test instead - the segment brought into the footprint's own
    /// frame, then clipped against each axis in turn. What survives is the
    /// stretch of the walk that is inside, and if any of it survives, the walk
    /// goes through.
    pub fn crosses(&self, a: Vec2, b: Vec2) -> bool {
        // Two circles first, and only then the trigonometry.
        //
        // This is asked once per building per EDGE of the search: a
        // three-thousand-node search steps eight ways from every node, so a
        // village of a few dozen halls is millions of these in one path — and
        // every one of them opened with a `sin_cos` before it could decide the
        // building was two hundred metres away.
        //
        // The footprint's circumscribed circle against the segment's own. Both
        // are exact bounds whatever the yaw, so a rejection here is a rejection
        // the slab clip would also have made, and anything that survives goes
        // on to be answered properly. A LONG segment keeps a large radius and
        // is never waved through on this — which is the case that matters,
        // since a walk that clips a far corner has its middle nowhere near the
        // building it crosses.
        let reach = self.half.length() + a.distance(b) * 0.5;
        if ((a + b) * 0.5).distance_squared(self.at) > reach * reach {
            return false;
        }

        let (sin, cos) = (-self.yaw).sin_cos();
        let into = |p: Vec2| {
            let local = p - self.at;
            Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos)
        };
        let (a, b) = (into(a), into(b));
        let run = b - a;

        let (mut enter, mut leave) = (0.0f32, 1.0f32);
        for axis in 0..2 {
            let (from, along, half) = (a[axis], run[axis], self.half[axis]);
            if along.abs() < 1e-6 {
                // Parallel to this pair of walls: either between them for the
                // whole walk, or outside them for the whole walk.
                if from.abs() >= half {
                    return false;
                }
                continue;
            }
            let (mut near, mut far) = ((-half - from) / along, (half - from) / along);
            if near > far {
                std::mem::swap(&mut near, &mut far);
            }
            enter = enter.max(near);
            leave = leave.min(far);
            if enter > leave {
                return false;
            }
        }
        true
    }
}

/// Every building the search must walk around.
#[derive(Resource, Default)]
pub struct Walls {
    pub buildings: Vec<Footprint>,
}

impl Walls {
    /// Which buildings this journey is allowed to be inside.
    ///
    /// The rule, and it is the same for a villager and for a wolf: YOU MAY ENTER
    /// THE BUILDING YOU ARE GOING TO, AND LEAVE THE ONE YOU ARE IN. You may not
    /// cut through one you are merely passing.
    ///
    /// It has to be a rule about the journey rather than about the walker,
    /// because the grid is two and a half metres to a cell and a door is one
    /// metre wide: no search on this grid can thread a doorway, so a building
    /// blocked outright would be a building nobody could ever enter and no bed
    /// anyone could reach. The door itself is steered by `home::use_doors`, which
    /// is the right tool at that scale; this only has to stop people walking
    /// through the walls on their way past.
    /// The excuse is judged GENEROUSLY - a stride's margin outside the
    /// footprint - while the barring below stays strict. The asymmetry is
    /// the whole point: a footprint is drawn a shoulder's width inside
    /// the wall, so a goal standing in the wall's own thickness, or a
    /// pace outside the door, belonged to no building at all. Every step
    /// into it was barred and the walk was refused outright.
    ///
    /// That is how eight hundred walks to a full larder were refused in
    /// four minutes: the village's sacks stand against the longhouse, the
    /// hungry stood twenty strides off with nowhere their feet were
    /// allowed to go, and `locomotion` drops an errand it cannot route.
    /// They starved in sight of the food, for the third time in this
    /// project, and the first two fixes were both about the terrain.
    fn excused(&self, start: Vec2, goal: Vec2) -> [bool; MOST_BUILDINGS] {
        let mut excused = [false; MOST_BUILDINGS];
        for (slot, building) in self.buildings.iter().enumerate().take(MOST_BUILDINGS) {
            excused[slot] = building.within(start, EITHER_END_MARGIN)
                || building.within(goal, EITHER_END_MARGIN);
        }
        excused
    }

    /// Whether a straight walk from `a` to `b` passes through any building it
    /// is not excused from. See `Footprint::crosses`.
    fn barred(&self, a: Vec2, b: Vec2, excused: &[bool; MOST_BUILDINGS]) -> bool {
        self.buildings.iter().enumerate().any(|(slot, building)| {
            !excused.get(slot).copied().unwrap_or(false) && building.crosses(a, b)
        })
    }
}

/// How far outside a footprint still counts as belonging to that building
/// when deciding whether a walk may enter it. Wide enough to cover the
/// shoulder's width the footprint is shrunk by, the wall's own thickness,
/// and a pace of doorstep beyond it - so a sack, a bed or a doorstep
/// standing anywhere in a building's fabric is somewhere a walk can end.
const EITHER_END_MARGIN: f32 = 1.2;

/// How many buildings one search can hold excuses for. A village outgrowing
/// this does not break — the excess simply cannot be entered on that journey,
/// and the ones a walk actually starts or ends in are the early ones in the
/// list far more often than not.
const MOST_BUILDINGS: usize = 256;

/// A cell in the navigation grid.
type Cell = IVec2;

fn to_cell(p: Vec3) -> Cell {
    IVec2::new((p.x / CELL).round() as i32, (p.z / CELL).round() as i32)
}

/// The nearest walkable cell to a point that is not itself walkable — beside
/// the plinth, at the foot of the hall steps. Searched ring by ring out to
/// three cells, nearest ring first, nearest cell within it winning. `None`
/// only for a goal buried deeper than that, which is a true refusal.
fn stand_beside(terrain: &Terrain, goal: Vec3) -> Option<Cell> {
    let centre = to_cell(goal);
    for ring in 1i32..=3 {
        let mut best: Option<(f32, Cell)> = None;
        for dx in -ring..=ring {
            for dz in -ring..=ring {
                if dx.abs().max(dz.abs()) != ring {
                    continue;
                }
                let cell = centre + IVec2::new(dx, dz);
                let (x, z) = (cell.x as f32 * CELL, cell.y as f32 * CELL);
                if !terrain.is_walkable(x, z) {
                    continue;
                }
                let near = Vec2::new(x - goal.x, z - goal.z).length_squared();
                if best.is_none_or(|(held, _)| near < held) {
                    best = Some((near, cell));
                }
            }
        }
        if let Some((_, cell)) = best {
            return Some(cell);
        }
    }
    None
}

fn to_world(c: Cell, terrain: &Terrain) -> Vec3 {
    let x = c.x as f32 * CELL;
    let z = c.y as f32 * CELL;
    // Stand height, not ground height: a waypoint on a dock's deck is at
    // plank level, so the climb charge prices the ramp and not the seabed.
    Vec3::new(x, terrain.stand_height_at(x, z), z)
}

/// Priority-queue entry. `Ord` is reversed so `BinaryHeap` pops the lowest score.
#[derive(PartialEq)]
struct Candidate {
    score: f32,
    cell: Cell,
}

impl Eq for Candidate {}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .score
            .partial_cmp(&self.score)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Octile distance: the exact cost of moving on an eight-connected grid, which makes
/// it an admissible heuristic that never overestimates.
fn heuristic(a: Cell, b: Cell) -> f32 {
    let dx = (a.x - b.x).abs() as f32;
    let dy = (a.y - b.y).abs() as f32;
    let (long, short) = if dx > dy { (dx, dy) } else { (dy, dx) };
    (long - short) + short * std::f32::consts::SQRT_2
}

/// What one cell is, worked out once and kept for the rest of the search.
#[derive(Clone, Copy)]
struct Spot {
    at: Vec3,
    walkable: bool,
}

/// The terrain's answers, remembered per cell.
///
/// This is where the frame time was. The search asked the terrain afresh for
/// every EDGE, and every cell has eight of them: `step_cost` derived the cell it
/// was leaving, the cell it was stepping to and both shoulders of a diagonal,
/// then the caller derived two of those a THIRD time for the wall test. Every
/// one of those answers is a multi-octave noise field plus a walk of the
/// drainage bins for the rivers, and a three-thousand-node search was asking
/// for tens of thousands of them to learn a few thousand facts.
///
/// Measured before: `creature: plan_routes` at 76ms of an 81ms frame, four
/// searches a frame, so about nineteen milliseconds each. Asked once per cell
/// instead, a cell costs what it costs and no more.
#[derive(Default)]
struct Ground {
    known: HashMap<Cell, Spot>,
}

impl Ground {
    fn spot(&mut self, cell: Cell, terrain: &Terrain) -> Spot {
        *self.known.entry(cell).or_insert_with(|| {
            let x = cell.x as f32 * CELL;
            let z = cell.y as f32 * CELL;
            Spot {
                // Stand height, not ground height: a waypoint on a dock's deck
                // is at plank level, so the climb charge prices the ramp and
                // not the seabed.
                at: Vec3::new(x, terrain.stand_height_at(x, z), z),
                walkable: terrain.is_walkable(x, z),
            }
        })
    }
}

/// Cost of stepping between two adjacent cells and where it lands, or `None` if
/// it cannot be walked.
///
/// Slope is charged for rather than forbidden outright, so a creature will take the
/// gentle way round a hill when one exists but still climb when it must.
fn step_cost(
    ground: &mut Ground,
    terrain: &Terrain,
    from: Cell,
    to: Cell,
) -> Option<(f32, Vec3, Vec3)> {
    let target = ground.spot(to, terrain);
    if !target.walkable {
        return None;
    }

    let diagonal = from.x != to.x && from.y != to.y;
    if diagonal {
        // Refuse to cut a corner between two blocked cells, or creatures clip
        // through the diagonal gap between rocks.
        let side_a = ground.spot(IVec2::new(to.x, from.y), terrain);
        let side_b = ground.spot(IVec2::new(from.x, to.y), terrain);
        if !side_a.walkable || !side_b.walkable {
            return None;
        }
    }

    let base = if diagonal {
        std::f32::consts::SQRT_2
    } else {
        1.0
    };
    let here = ground.spot(from, terrain);
    let climb = (target.at.y - here.at.y).abs();
    // Both ends handed back, so the wall test below does not derive them again.
    Some((base + climb * 0.35, here.at, target.at))
}

/// Finds a walkable path from `start` to `goal`, or `None` if there is not one
/// within the budget.
///
/// The returned path excludes the starting cell and ends at the goal.
pub fn find_path(
    terrain: &Terrain,
    walls: &Walls,
    start: Vec3,
    goal: Vec3,
    budget: usize,
) -> Option<Vec<Vec3>> {
    let start_cell = to_cell(start);
    // A goal that lands on an unwalkable cell — a banner plinth, a hall
    // slab, a freshly terraced ledge — is not a refusal, it is an errand to
    // the nearest ground BESIDE it. Refusing it outright starved a village:
    // the square went unwalkable while the hall rose, every meal was aimed
    // at the square's exact centre, and every path to a full larder failed
    // on this line. Six villagers died standing at the radius where their
    // routes were abandoned, and not one of them ever reached a meal.
    let goal_cell = if terrain.is_walkable(goal.x, goal.z) {
        to_cell(goal)
    } else {
        stand_beside(terrain, goal)?
    };

    if start_cell == goal_cell {
        return Some(Vec::new());
    }
    let excused = walls.excused(Vec2::new(start.x, start.z), Vec2::new(goal.x, goal.z));

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<Cell, Cell> = HashMap::default();
    let mut cost: HashMap<Cell, f32> = HashMap::default();
    let mut closed: HashSet<Cell> = HashSet::default();
    let mut ground = Ground::default();

    open.push(Candidate {
        score: heuristic(start_cell, goal_cell),
        cell: start_cell,
    });
    cost.insert(start_cell, 0.0);

    let mut expanded = 0;

    while let Some(Candidate { cell, .. }) = open.pop() {
        if cell == goal_cell {
            return Some(reconstruct(terrain, walls, &excused, &came_from, cell));
        }
        if !closed.insert(cell) {
            continue;
        }

        expanded += 1;
        if expanded > budget {
            return None;
        }

        let here = *cost.get(&cell).unwrap_or(&f32::MAX);
        for dz in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let next = cell + IVec2::new(dx, dz);
                if closed.contains(&next) {
                    continue;
                }
                let Some((step, here_at, stands)) = step_cost(&mut ground, terrain, cell, next)
                else {
                    continue;
                };
                // Through a wall is not a step - and the STEP is what has to
                // be clear, not just the cell it lands on.
                //
                // Asking only about the destination lets a diagonal cut the
                // corner off a building: two cells either side of a corner are
                // both outside it, and the line between their middles is not.
                // With cells at two and a half metres the bite taken out is
                // most of a metre, and the whole point of `Walls` is that
                // nobody walks through a hall they are only passing.
                //
                // It was invisible until the terrain was smoothed and a route
                // happened to want that particular diagonal, which is worth
                // remembering: the test had been passing for want of the
                // geometry rather than for want of the bug.
                //
                // Both ends come back from `step_cost`, which has just had to
                // work them out anyway.
                if walls.barred(
                    Vec2::new(here_at.x, here_at.z),
                    Vec2::new(stands.x, stands.z),
                    &excused,
                ) {
                    continue;
                }

                let candidate = here + step;
                if candidate < *cost.get(&next).unwrap_or(&f32::MAX) {
                    cost.insert(next, candidate);
                    came_from.insert(next, cell);
                    open.push(Candidate {
                        score: candidate + heuristic(next, goal_cell),
                        cell: next,
                    });
                }
            }
        }
    }

    None
}

/// Walks the parent chain back to the start and straightens the result.
fn reconstruct(
    terrain: &Terrain,
    walls: &Walls,
    excused: &[bool; MOST_BUILDINGS],
    came_from: &HashMap<Cell, Cell>,
    end: Cell,
) -> Vec<Vec3> {
    let mut cells = vec![end];
    let mut cursor = end;
    while let Some(previous) = came_from.get(&cursor) {
        cells.push(*previous);
        cursor = *previous;
    }
    cells.reverse();

    // Drop waypoints that can be skipped by walking straight. A* on a grid produces
    // staircases, and following them literally makes creatures zigzag across open
    // ground that they could have crossed in one line.
    let mut path = Vec::new();
    let mut anchor = 0;
    let mut index = 1;
    while index < cells.len() {
        if !walkable_line(terrain, walls, excused, cells[anchor], cells[index]) {
            path.push(to_world(cells[index - 1], terrain));
            anchor = index - 1;
        }
        index += 1;
    }
    path.push(to_world(*cells.last().unwrap(), terrain));

    // The first cell is where the creature already stands.
    path.retain(|p| p.distance_squared(to_world(cells[0], terrain)) > 0.01);
    path
}

/// Whether a straight line between two cells stays on walkable ground.
fn walkable_line(
    terrain: &Terrain,
    walls: &Walls,
    excused: &[bool; MOST_BUILDINGS],
    from: Cell,
    to: Cell,
) -> bool {
    let a = to_world(from, terrain);
    let b = to_world(to, terrain);
    let distance = a.distance(b);
    let steps = (distance / (CELL * 0.5)).ceil() as i32;

    // Straightening must not cut the corner off a building: the staircase A*
    // produced went round it, and a straight line between two of its steps can
    // go straight back through. Asked of the whole segment at once, because
    // sampling it cannot answer - a clip thinner than the step falls between
    // two samples that both say the line is clear.
    if walls.barred(Vec2::new(a.x, a.z), Vec2::new(b.x, b.z), excused) {
        return false;
    }

    // The ground still has to be sampled: walkability is a field, not a shape,
    // and there is nothing to intersect a segment against.
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let p = a.lerp(b, t);
        if !terrain.is_walkable(p.x, p.z) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws a chart the way the system does, but all at once.
    ///
    /// The same fill, not a second implementation of it - a test that agreed
    /// with a copy of the code rather than with the code would prove nothing.
    fn chart_around(terrain: &Terrain, centre: Vec3, reach_units: f32) -> Reachable {
        let mut chart = Reachable {
            centre: to_cell(centre),
            reach: (reach_units / CELL) as i32,
            ..Default::default()
        };
        let (centre_cell, reach) = (chart.centre, chart.reach);
        let side = reach * 2 + 1;
        let mut islands = 0;
        for step in 0..side * side {
            let cell = centre_cell + IVec2::new(step % side - reach, step / side - reach);
            if chart.island.contains_key(&cell) {
                continue;
            }
            if !terrain.is_walkable(cell.x as f32 * CELL, cell.y as f32 * CELL) {
                continue;
            }
            islands += 1;
            chart.island.insert(cell, islands);
            let mut frontier = vec![cell];
            while let Some(at) = frontier.pop() {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        let next = at + IVec2::new(dx, dz);
                        let away = next - centre_cell;
                        if away.x.abs() > reach || away.y.abs() > reach {
                            continue;
                        }
                        if chart.island.contains_key(&next) {
                            continue;
                        }
                        let open =
                            |c: Cell| terrain.is_walkable(c.x as f32 * CELL, c.y as f32 * CELL);
                        if !open(next) {
                            continue;
                        }
                        if dx != 0
                            && dz != 0
                            && (!open(IVec2::new(next.x, at.y)) || !open(IVec2::new(at.x, next.y)))
                        {
                            continue;
                        }
                        chart.island.insert(next, islands);
                        frontier.push(next);
                    }
                }
            }
        }
        chart
    }

    /// The chart must never refuse an errand the search could have walked.
    ///
    /// This is the whole safety of the thing. It is allowed to be ignorant —
    /// "I do not know" costs a search and nothing else — but it is never
    /// allowed to be WRONG, because a false refusal is a villager standing
    /// still beside work they could have done, and that is far worse than the
    /// frame time it was written to save.
    #[test]
    fn the_chart_never_refuses_a_walk_that_exists() {
        let terrain = Terrain::new(4242);
        let walls = Walls::default();
        let here = walkable_near(&terrain);
        let chart = chart_around(&terrain, here, 180.0);

        let mut refused = 0;
        let mut walked = 0;
        for i in 0..260 {
            // A spread of destinations across the charted ground, most
            // reachable and some deliberately not.
            let turn = i as f32 * 0.618 * std::f32::consts::TAU;
            let (sin, cos) = turn.sin_cos();
            let reach = 20.0 + (i % 13) as f32 * 12.0;
            let (x, z) = (here.x + cos * reach, here.z + sin * reach);
            let there = Vec3::new(x, terrain.height_at(x, z), z);

            let path = find_path(&terrain, &walls, here, there, DEFAULT_BUDGET);
            if path.is_some() {
                walked += 1;
                assert!(
                    !chart.hopeless(here, there),
                    "the chart called {there:?} hopeless and the search walked \
                     there - a villager would stand still beside work they \
                     could have done"
                );
            } else if chart.hopeless(here, there) {
                refused += 1;
            }
        }
        assert!(
            walked > 40,
            "only {walked} destinations were walkable at all"
        );
        let _ = refused;
    }

    /// And it does spare searches — on ground where there is anything to spare.
    ///
    /// Kept apart from the safety test above deliberately. That one sweeps
    /// ordinary ground and proves the chart is never WRONG; this one has to go
    /// looking for a coast, because the case worth saving only exists where
    /// water or cliff genuinely cuts the ground in two, and the first fixture
    /// tried had none within a hundred and eighty units. A test that asserted
    /// both things at once would have gone green the day somebody moved it to
    /// a meadow.
    #[test]
    fn the_chart_spares_the_search_where_the_ground_is_cut_in_two() {
        let walls = Walls::default();
        let mut spared = 0;
        let mut looked = 0;

        'seeds: for seed in [4242u32, 7, 99, 2024, 31337, 555] {
            let terrain = Terrain::new(seed);
            // A shore, which is where ground gets cut in two.
            let Some(here) = shore_near(&terrain) else {
                continue;
            };
            let chart = chart_around(&terrain, here, 200.0);
            for i in 0..900 {
                let turn = i as f32 * 0.618 * std::f32::consts::TAU;
                let (sin, cos) = turn.sin_cos();
                let reach = 30.0 + (i % 17) as f32 * 9.0;
                let (x, z) = (here.x + cos * reach, here.z + sin * reach);
                if !terrain.is_walkable(x, z) {
                    continue;
                }
                let there = Vec3::new(x, terrain.height_at(x, z), z);
                looked += 1;
                if !chart.hopeless(here, there) {
                    continue;
                }
                // The chart says no. The search must agree - this is the same
                // claim as the safety test, asked of the cases that matter.
                assert!(
                    find_path(&terrain, &walls, here, there, DEFAULT_BUDGET).is_none(),
                    "seed {seed}: the chart refused a walk the search made"
                );
                spared += 1;
                if spared >= 3 {
                    break 'seeds;
                }
            }
        }

        assert!(looked > 100, "only {looked} walkable destinations examined");
        assert!(
            spared >= 3,
            "over six worlds the chart never once spared a failing search, so \
             it is pure cost - either no seed here has a cut coastline within \
             two hundred units, or the fill is joining ground it should not"
        );
    }

    /// Walkable ground with water close by, or `None` if this world has no
    /// coast near the origin.
    fn shore_near(terrain: &Terrain) -> Option<Vec3> {
        for i in 0..40_000 {
            let turn = i as f32 * 0.618 * std::f32::consts::TAU;
            let (sin, cos) = turn.sin_cos();
            let reach = 40.0 + (i % 400) as f32 * 6.0;
            let (x, z) = (cos * reach, sin * reach);
            if !terrain.is_walkable(x, z) {
                continue;
            }
            // Water within a short walk, in any direction.
            let wet = [(30.0, 0.0), (-30.0, 0.0), (0.0, 30.0), (0.0, -30.0)]
                .iter()
                .any(|(dx, dz)| terrain.is_submerged(x + dx, z + dz));
            if wet {
                return Some(Vec3::new(x, terrain.height_at(x, z), z));
            }
        }
        None
    }

    /// And it says nothing at all about ground it has not charted.
    #[test]
    fn uncharted_ground_is_never_refused() {
        let terrain = Terrain::new(4242);
        let here = walkable_near(&terrain);
        let chart = chart_around(&terrain, here, 60.0);
        let far = here + Vec3::new(4_000.0, 0.0, 4_000.0);
        assert!(
            !chart.hopeless(here, far),
            "somewhere off the edge of the map was refused on no evidence"
        );
        assert!(
            !chart.hopeless(far, here),
            "and the same the other way round"
        );
        let empty = Reachable::default();
        assert!(
            !empty.hopeless(here, here + Vec3::new(30.0, 0.0, 0.0)),
            "a chart that has not been drawn yet refused an errand"
        );
    }

    /// Finds somewhere walkable near the origin to test from.
    fn walkable_near(terrain: &Terrain) -> Vec3 {
        for i in 0..40_000 {
            let x = (i % 200) as f32 * 12.0 - 1200.0;
            let z = (i / 200) as f32 * 12.0 - 1200.0;
            if terrain.is_walkable(x, z) {
                return Vec3::new(x, terrain.height_at(x, z), z);
            }
        }
        panic!("no walkable ground near the origin");
    }

    /// Two points with open ground between them, far enough apart to put a
    /// building in the middle of.
    fn a_clear_walk(terrain: &Terrain) -> Option<(Vec3, Vec3)> {
        let start = walkable_near(terrain);
        for step in 8..40 {
            for turn in 0..16 {
                let angle = turn as f32 * std::f32::consts::TAU / 16.0;
                let reach = step as f32 * 2.0;
                let goal = start + Vec3::new(angle.cos() * reach, 0.0, angle.sin() * reach);
                if !terrain.is_walkable(goal.x, goal.z) {
                    continue;
                }
                let straight = (0..=40).all(|i| {
                    let p = start.lerp(goal, i as f32 / 40.0);
                    terrain.is_walkable(p.x, p.z)
                });
                if straight && reach > 24.0 {
                    return Some((start, goal));
                }
            }
        }
        None
    }

    #[test]
    fn the_cheap_rejection_never_waves_a_crossing_through() {
        // `crosses` opens with a circle-against-circle test so it can throw out
        // a distant building without paying for two trig calls. A cheap
        // rejection that is ever WRONG is worse than no rejection at all -
        // people would walk through walls, and only for buildings far from the
        // middle of their stride, which is about the nastiest bug shape there
        // is.
        //
        // Swept rather than sampled: a hall turned every which way, and a long
        // walk swung all the way round it. Every claim of "does not cross" is
        // checked against the geometry directly.
        let hall = |yaw: f32| Footprint {
            at: Vec2::new(30.0, -12.0),
            half: Vec2::new(6.0, 2.5),
            yaw,
        };
        for turn in 0..12 {
            let footprint = hall(turn as f32 * 0.26);
            for step in 0..72 {
                let angle = step as f32 * std::f32::consts::TAU / 72.0;
                let arm = Vec2::from_angle(angle) * 40.0;
                // A long walk right past the hall, and a short one nowhere
                // near it, and a walk that ends on top of it.
                for (a, b) in [
                    (footprint.at - arm, footprint.at + arm),
                    (footprint.at + arm, footprint.at + arm * 1.5),
                    (footprint.at + arm, footprint.at),
                ] {
                    if footprint.crosses(a, b) {
                        continue;
                    }
                    // It says no. Then no point along it may be inside, and
                    // that is a different computation from the one that said
                    // so.
                    for i in 0..=400 {
                        let along = a + (b - a) * (i as f32 / 400.0);
                        assert!(
                            !footprint.within(along, 0.0),
                            "crosses() said no but ({along}) is inside a hall \
                             turned {turn} at step {step}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_building_is_not_a_shortcut() {
        // The bug, as a test: open ground, a hall dropped squarely in the
        // middle of the only sensible line, and a walk from one side to the
        // other. Every waypoint must be outside it.
        let t = Terrain::new(77);
        let Some((start, goal)) = a_clear_walk(&t) else {
            return;
        };
        let middle = (start + goal) * 0.5;
        let hall = Footprint {
            at: Vec2::new(middle.x, middle.z),
            half: Vec2::splat(5.0),
            yaw: 0.0,
        };
        // The test only means something if the straight line really does go
        // through the building.
        assert!(
            hall.within(Vec2::new(middle.x, middle.z), 0.0),
            "the hall is not on the line, so this proves nothing"
        );
        let walls = Walls {
            buildings: vec![hall],
        };
        let path = find_path(&t, &walls, start, goal, DEFAULT_BUDGET).expect("a way round");
        for leg in &path {
            assert!(
                !hall.within(Vec2::new(leg.x, leg.z), 0.0),
                "the route goes through the hall at {leg:?}"
            );
        }
        // And the straightening must not put it back: walk the whole route.
        let mut cursor = start;
        for leg in path.iter().chain(std::iter::once(&goal)) {
            for i in 0..=20 {
                let p = cursor.lerp(*leg, i as f32 / 20.0);
                assert!(
                    !hall.within(Vec2::new(p.x, p.z), 0.0),
                    "a leg of the route cuts the corner off the hall"
                );
            }
            cursor = *leg;
        }
    }

    #[test]
    fn the_building_you_are_going_to_is_the_one_you_may_enter() {
        // A door is one metre and a cell is two and a half, so a building
        // blocked outright is a building with no way in and a bed nobody can
        // reach. The journey's own destination is always excused.
        let t = Terrain::new(77);
        let Some((start, goal)) = a_clear_walk(&t) else {
            return;
        };
        let home = Footprint {
            at: Vec2::new(goal.x, goal.z),
            half: Vec2::splat(4.0),
            yaw: 0.0,
        };
        let walls = Walls {
            buildings: vec![home],
        };
        assert!(
            find_path(&t, &walls, start, goal, DEFAULT_BUDGET).is_some(),
            "nobody can walk home: the house they are going to is blocking them"
        );
        // And the reverse, so somebody indoors can leave.
        assert!(
            find_path(&t, &walls, goal, start, DEFAULT_BUDGET).is_some(),
            "nobody can leave the house they are standing in"
        );
    }

    #[test]
    fn a_turned_building_blocks_its_own_rectangle() {
        // Not the axis-aligned box around it: a hall at forty-five degrees
        // leaves its corners open ground.
        let hall = Footprint {
            at: Vec2::ZERO,
            half: Vec2::new(6.0, 2.0),
            yaw: std::f32::consts::FRAC_PI_4,
        };
        assert!(
            hall.within(Vec2::new(3.0, 3.0), 0.0),
            "along its own length"
        );
        assert!(
            !hall.within(Vec2::new(-3.5, 3.5), 0.0),
            "across it: this is inside the box that contains the hall, and \
             outside the hall"
        );
    }

    #[test]
    fn a_path_to_where_you_stand_is_empty() {
        let t = Terrain::new(77);
        let here = walkable_near(&t);
        assert_eq!(
            find_path(&t, &Walls::default(), here, here, DEFAULT_BUDGET),
            Some(Vec::new())
        );
    }

    #[test]
    fn an_unwalkable_goal_has_no_path() {
        // Aiming into the sea must fail rather than walking in.
        let t = Terrain::new(77);
        let start = walkable_near(&t);

        let mut sea = None;
        for i in 0..40_000 {
            let x = (i % 200) as f32 * 12.0 - 1200.0;
            let z = (i / 200) as f32 * 12.0 - 1200.0;
            if t.is_submerged(x, z) {
                sea = Some(Vec3::new(x, 0.0, z));
                break;
            }
        }
        let sea = sea.expect("no water found");
        assert_eq!(
            find_path(&t, &Walls::default(), start, sea, DEFAULT_BUDGET),
            None
        );
    }

    #[test]
    fn every_step_of_a_path_is_walkable() {
        // The whole point: a path must never route through water or up a cliff.
        let t = Terrain::new(77);
        let start = walkable_near(&t);
        let mut checked = 0;

        for angle in 0..12 {
            let a = angle as f32 / 12.0 * std::f32::consts::TAU;
            let goal_xz = Vec2::new(start.x + a.cos() * 70.0, start.z + a.sin() * 70.0);
            if !t.is_walkable(goal_xz.x, goal_xz.y) {
                continue;
            }
            let goal = Vec3::new(goal_xz.x, t.height_at(goal_xz.x, goal_xz.y), goal_xz.y);

            let Some(path) = find_path(&t, &Walls::default(), start, goal, DEFAULT_BUDGET) else {
                continue;
            };

            let mut cursor = start;
            for step in &path {
                assert!(
                    walkable_line(
                        &t,
                        &Walls::default(),
                        &[false; MOST_BUILDINGS],
                        to_cell(cursor),
                        to_cell(*step)
                    ),
                    "path crosses unwalkable ground",
                );
                cursor = *step;
            }
            checked += 1;
        }

        assert!(checked > 3, "only checked {checked} paths");
    }

    #[test]
    fn a_path_ends_at_its_goal() {
        let t = Terrain::new(77);
        let start = walkable_near(&t);
        let goal_xz = Vec2::new(start.x + 40.0, start.z + 25.0);
        if !t.is_walkable(goal_xz.x, goal_xz.y) {
            return;
        }
        let goal = Vec3::new(goal_xz.x, t.height_at(goal_xz.x, goal_xz.y), goal_xz.y);

        let path = find_path(&t, &Walls::default(), start, goal, DEFAULT_BUDGET).expect("no path");
        let end = *path.last().expect("empty path");
        assert!(end.distance(goal) < CELL * 1.5, "ended at {end:?}");
    }

    #[test]
    fn straightening_removes_the_grid_staircase() {
        // A* on a grid zigzags across open ground. Over clear terrain the result
        // should collapse to very few waypoints.
        let t = Terrain::new(77);
        let start = walkable_near(&t);
        let goal_xz = Vec2::new(start.x + 30.0, start.z + 30.0);
        if !t.is_walkable(goal_xz.x, goal_xz.y) {
            return;
        }
        let goal = Vec3::new(goal_xz.x, t.height_at(goal_xz.x, goal_xz.y), goal_xz.y);

        if let Some(path) = find_path(&t, &Walls::default(), start, goal, DEFAULT_BUDGET) {
            let diagonal_cells = (30.0 / CELL) as usize;
            assert!(
                path.len() < diagonal_cells,
                "{} waypoints for a mostly straight walk",
                path.len(),
            );
        }
    }

    #[test]
    fn an_unreachable_goal_gives_up_inside_its_budget() {
        // In an endless world an unbounded search never terminates. This has to fail
        // fast, not flood-fill a continent.
        let t = Terrain::new(77);
        let start = walkable_near(&t);
        let far = Vec3::new(start.x + 40_000.0, 0.0, start.z + 40_000.0);
        assert_eq!(find_path(&t, &Walls::default(), start, far, 400), None);
    }

    /// A goal on unwalkable ground is an errand to its edge, not a refusal.
    ///
    /// `eat_from_store` aims every meal at one exact point, and when the
    /// hall's terraces made that point unwalkable, every path to a full
    /// larder failed on the old outright-refusal line: six villagers starved
    /// standing at the radius where their routes were abandoned. The search
    /// now walks to the nearest stand-able cell beside a buried goal.
    #[test]
    fn a_goal_on_unwalkable_ground_walks_to_its_edge() {
        let terrain = Terrain::new(4242);
        let walls = Walls::default();
        let start = walkable_near(&terrain);

        // The nearest spot to the start that cannot be stood on but has
        // stand-able ground beside it — a shoreline, a ledge, a boulder.
        let mut buried = None;
        'hunt: for ring in 1..80 {
            for step in 0..(ring * 8) {
                let angle = step as f32 / (ring * 8) as f32 * std::f32::consts::TAU;
                let spot = start + Vec3::new(angle.cos(), 0.0, angle.sin()) * (ring as f32 * CELL);
                if !terrain.is_walkable(spot.x, spot.z) && stand_beside(&terrain, spot).is_some() {
                    buried = Some(spot);
                    break 'hunt;
                }
            }
        }
        let goal = buried.expect("no unwalkable ground within eighty cells of a walkable start");

        let Some(waypoints) = find_path(&terrain, &walls, start, goal, DEFAULT_BUDGET) else {
            panic!("a goal beside stand-able ground was refused outright");
        };
        let landed = waypoints.last().copied().unwrap_or(start);
        let short = Vec2::new(landed.x - goal.x, landed.z - goal.z).length();
        assert!(
            short <= CELL * 3.0 + 0.1,
            "the walk should end beside the buried goal, not {short} strides short",
        );
    }
}
