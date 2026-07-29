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
pub fn find_path(terrain: &Terrain, start: Vec3, goal: Vec3, budget: usize) -> Option<Vec<Vec3>> {
    let start_cell = to_cell(start);
    let goal_cell = to_cell(goal);

    if start_cell == goal_cell {
        return Some(Vec::new());
    }
    if !terrain.is_walkable(goal.x, goal.z) {
        return None;
    }

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
            return Some(reconstruct(terrain, &came_from, cell));
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
fn reconstruct(terrain: &Terrain, came_from: &HashMap<Cell, Cell>, end: Cell) -> Vec<Vec3> {
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
        if !walkable_line(terrain, cells[anchor], cells[index]) {
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
fn walkable_line(terrain: &Terrain, from: Cell, to: Cell) -> bool {
    let a = to_world(from, terrain);
    let b = to_world(to, terrain);
    let distance = a.distance(b);
    let steps = (distance / (CELL * 0.5)).ceil() as i32;

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

    #[test]
    fn a_path_to_where_you_stand_is_empty() {
        let t = Terrain::new(77);
        let here = walkable_near(&t);
        assert_eq!(find_path(&t, here, here, DEFAULT_BUDGET), Some(Vec::new()));
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
        assert_eq!(find_path(&t, start, sea, DEFAULT_BUDGET), None);
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

            let Some(path) = find_path(&t, start, goal, DEFAULT_BUDGET) else {
                continue;
            };

            let mut cursor = start;
            for step in &path {
                assert!(
                    walkable_line(&t, to_cell(cursor), to_cell(*step)),
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

        let path = find_path(&t, start, goal, DEFAULT_BUDGET).expect("no path");
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

        if let Some(path) = find_path(&t, start, goal, DEFAULT_BUDGET) {
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
        assert_eq!(find_path(&t, start, far, 400), None);
    }
}
