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
            // Before the routes that read it, and in the same frame a building
            // finishes: a hall that is standing but not yet an obstacle is a
            // hall people walk through for as long as their current route
            // lasts.
            .add_systems(
                Update,
                survey_the_walls.before(crate::creature::plan_routes),
            );
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
) {
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
    pub fn contains(&self, p: Vec2) -> bool {
        let (sin, cos) = (-self.yaw).sin_cos();
        let local = p - self.at;
        let turned = Vec2::new(
            local.x * cos - local.y * sin,
            local.x * sin + local.y * cos,
        );
        turned.x.abs() < self.half.x && turned.y.abs() < self.half.y
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
        let (sin, cos) = (-self.yaw).sin_cos();
        let into = |p: Vec2| {
            let local = p - self.at;
            Vec2::new(
                local.x * cos - local.y * sin,
                local.x * sin + local.y * cos,
            )
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
    fn excused(&self, start: Vec2, goal: Vec2) -> [bool; MOST_BUILDINGS] {
        let mut excused = [false; MOST_BUILDINGS];
        for (slot, building) in self.buildings.iter().enumerate().take(MOST_BUILDINGS) {
            excused[slot] = building.contains(start) || building.contains(goal);
        }
        excused
    }

    /// Whether a straight walk from `a` to `b` passes through any building it
    /// is not excused from. See `Footprint::crosses`.
    fn barred(&self, a: Vec2, b: Vec2, excused: &[bool; MOST_BUILDINGS]) -> bool {
        self.buildings
            .iter()
            .enumerate()
            .any(|(slot, building)| {
                !excused.get(slot).copied().unwrap_or(false) && building.crosses(a, b)
            })
    }
}

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

/// Cost of stepping between two adjacent cells, or `None` if it cannot be walked.
///
/// Slope is charged for rather than forbidden outright, so a creature will take the
/// gentle way round a hill when one exists but still climb when it must.
fn step_cost(terrain: &Terrain, from: Cell, to: Cell) -> Option<f32> {
    let target = to_world(to, terrain);
    if !terrain.is_walkable(target.x, target.z) {
        return None;
    }

    let diagonal = from.x != to.x && from.y != to.y;
    if diagonal {
        // Refuse to cut a corner between two blocked cells, or creatures clip
        // through the diagonal gap between rocks.
        let side_a = to_world(IVec2::new(to.x, from.y), terrain);
        let side_b = to_world(IVec2::new(from.x, to.y), terrain);
        if !terrain.is_walkable(side_a.x, side_a.z) || !terrain.is_walkable(side_b.x, side_b.z) {
            return None;
        }
    }

    let base = if diagonal {
        std::f32::consts::SQRT_2
    } else {
        1.0
    };
    let climb = (target.y - to_world(from, terrain).y).abs();
    Some(base + climb * 0.35)
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
    let goal_cell = to_cell(goal);

    if start_cell == goal_cell {
        return Some(Vec::new());
    }
    if !terrain.is_walkable(goal.x, goal.z) {
        return None;
    }
    let excused = walls.excused(
        Vec2::new(start.x, start.z),
        Vec2::new(goal.x, goal.z),
    );

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<Cell, Cell> = HashMap::default();
    let mut cost: HashMap<Cell, f32> = HashMap::default();
    let mut closed: HashSet<Cell> = HashSet::default();

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
                let Some(step) = step_cost(terrain, cell, next) else {
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
                let stands = to_world(next, terrain);
                let here_at = to_world(cell, terrain);
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
            hall.contains(Vec2::new(middle.x, middle.z)),
            "the hall is not on the line, so this proves nothing"
        );
        let walls = Walls {
            buildings: vec![hall],
        };
        let path = find_path(&t, &walls, start, goal, DEFAULT_BUDGET).expect("a way round");
        for leg in &path {
            assert!(
                !hall.contains(Vec2::new(leg.x, leg.z)),
                "the route goes through the hall at {leg:?}"
            );
        }
        // And the straightening must not put it back: walk the whole route.
        let mut cursor = start;
        for leg in path.iter().chain(std::iter::once(&goal)) {
            for i in 0..=20 {
                let p = cursor.lerp(*leg, i as f32 / 20.0);
                assert!(
                    !hall.contains(Vec2::new(p.x, p.z)),
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
        assert!(hall.contains(Vec2::new(3.0, 3.0)), "along its own length");
        assert!(
            !hall.contains(Vec2::new(-3.5, 3.5)),
            "across it: this is inside the box that contains the hall, and \
             outside the hall"
        );
    }

    #[test]
    fn a_path_to_where_you_stand_is_empty() {
        let t = Terrain::new(77);
        let here = walkable_near(&t);
        assert_eq!(find_path(&t, &Walls::default(), here, here, DEFAULT_BUDGET), Some(Vec::new()));
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
        assert_eq!(find_path(&t, &Walls::default(), start, sea, DEFAULT_BUDGET), None);
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
                    walkable_line(&t, &Walls::default(), &[false; MOST_BUILDINGS], to_cell(cursor), to_cell(*step)),
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
}
