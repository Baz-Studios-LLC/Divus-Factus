//! Worn trails: the ground remembers where feet go.
//!
//! Nobody plans a road. Every walking villager leaves a little wear in the
//! cell under their feet; wear accumulates where routes repeat — fire to
//! shore, door to field, square to quarry — and fades where they don't.
//! The path is painted straight into the terrain's vertex colours: grass
//! blends toward bare earth as wear builds and blends back as it fades,
//! so a trail looks like ground that has been walked, not like tiles laid
//! on top of it. Worn ground is faster underfoot, so the village's habits
//! pave its own shortcuts.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::creature::anim::CreatureMotion;
use crate::terrain::{CHUNK_SIZE, Terrain, TerrainChunk, ground_color_at};
use crate::villager::Villager;

/// The side of one wear cell, in world units.
const CELL: f32 = 1.7;

/// Wear at which the dirt fully shows through the grass.
const VISIBLE: f32 = 6.0;

/// Wear stops accumulating here: a road, not a trench.
const WEAR_CAP: f32 = 30.0;

/// Worn ground fades at this rate, per second. Slow: a trail earned over
/// a morning survives an idle afternoon.
const FADE: f32 = 0.008;

/// How much faster feet move on well-worn ground.
const HASTE: f32 = 1.25;

pub struct TrailsPlugin;

impl Plugin for TrailsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Trails>()
            .add_systems(Update, (tread, paint));
    }
}

/// One cell's memory of feet.
pub struct TrailCell {
    pub wear: f32,
    /// The tint band last painted (blend quantized), so the painter only
    /// touches chunks where something actually changed - a mature road
    /// network otherwise re-cost thousands of noise lookups every pass,
    /// and the frame rate sagged with the miles walked.
    painted_band: u8,
}

/// Everywhere the ground has been walked, and how hard.
#[derive(Resource, Default)]
pub struct Trails {
    pub cells: HashMap<IVec2, TrailCell>,
}

impl Trails {
    fn cell_of(x: f32, z: f32) -> IVec2 {
        IVec2::new((x / CELL).floor() as i32, (z / CELL).floor() as i32)
    }

    /// The stride multiplier for ground at this point: worn paths are
    /// quicker than wild grass.
    pub fn haste(&self, x: f32, z: f32) -> f32 {
        match self.cells.get(&Self::cell_of(x, z)) {
            Some(cell) if cell.wear >= VISIBLE => HASTE,
            _ => 1.0,
        }
    }

    /// How worn the ground looks at a point: the strongest nearby cell,
    /// eased off with distance, so a painted path has soft shoulders
    /// instead of cell-shaped stamps.
    fn wear_near(&self, x: f32, z: f32) -> f32 {
        let home = Self::cell_of(x, z);
        let mut strongest: f32 = 0.0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let at = home + IVec2::new(dx, dz);
                let Some(cell) = self.cells.get(&at) else {
                    continue;
                };
                let centre = Vec2::new((at.x as f32 + 0.5) * CELL, (at.y as f32 + 0.5) * CELL);
                let reach = 1.0 - (centre.distance(Vec2::new(x, z)) / (CELL * 1.35)).min(1.0);
                strongest = strongest.max(cell.wear * reach);
            }
        }
        strongest
    }

    /// Restores the bare wear map (from a save); freshly built chunks
    /// repaint themselves as they appear.
    pub fn restore(&mut self, worn: impl Iterator<Item = (i32, i32, f32)>) {
        self.cells.clear();
        for (x, z, wear) in worn {
            self.cells.insert(
                IVec2::new(x, z),
                TrailCell {
                    wear,
                    painted_band: u8::MAX,
                },
            );
        }
    }

    /// Every cell worth saving.
    pub fn export(&self) -> Vec<(i32, i32, f32)> {
        self.cells
            .iter()
            .filter(|(_, cell)| cell.wear > 0.5)
            .map(|(at, cell)| (at.x, at.y, cell.wear))
            .collect()
    }
}

/// Walking wears the ground. Only villagers going somewhere leave wear —
/// standing in the square all evening is not a road.
fn tread(
    time: Res<Time>,
    mut trails: ResMut<Trails>,
    walkers: Query<(&Transform, &CreatureMotion), With<Villager>>,
) {
    let dt = time.delta_secs();
    for (transform, motion) in &walkers {
        if motion.speed < 0.5 || motion.swim > 0.2 {
            continue;
        }
        let cell = trails
            .cells
            .entry(Trails::cell_of(
                transform.translation.x,
                transform.translation.z,
            ))
            .or_insert(TrailCell {
                wear: 0.0,
                painted_band: 0,
            });
        cell.wear = (cell.wear + dt * 0.9).min(WEAR_CAP);
    }
}

/// The slow keeping of the ground: wear fades, and the terrain's own
/// vertex colours are repainted — toward bare earth where feet insist,
/// back toward the true ground colour where they have stopped.
fn paint(
    time: Res<Time>,
    mut since_last: Local<f32>,
    terrain: Option<Res<Terrain>>,
    mut trails: ResMut<Trails>,
    mut meshes: ResMut<Assets<Mesh>>,
    chunks: Query<(&TerrainChunk, &Mesh3d)>,
    fresh: Query<&TerrainChunk, Added<TerrainChunk>>,
) {
    // Newly streamed-in chunks always need their trails painted on, even
    // on a pass where no wear changed — a loaded save's roads come back
    // this way too.
    let fresh_coords: Vec<IVec2> = fresh.iter().map(|chunk| chunk.coord).collect();

    *since_last += time.delta_secs();
    if *since_last < 2.0 && fresh_coords.is_empty() {
        return;
    }
    let elapsed = *since_last;
    *since_last = 0.0;
    let Some(terrain) = terrain else {
        return;
    };

    // Age the map, remembering only the cells whose PAINT actually needs
    // to change: the band their blend falls in moved since last pass.
    let mut stale: Vec<IVec2> = Vec::new();
    let mut gone: Vec<IVec2> = Vec::new();
    for (at, cell) in trails.cells.iter_mut() {
        cell.wear -= FADE * elapsed;
        let band = (((cell.wear - 0.8).max(0.0) / VISIBLE).min(1.0) * 24.0) as u8;
        if band != cell.painted_band {
            cell.painted_band = band;
            stale.push(*at);
        }
        if cell.wear <= 0.05 {
            gone.push(*at);
        }
    }

    // Which chunks those cells touch (with a margin: a cell near a chunk
    // seam tints vertices in the neighbour too).
    let mut dirty: Vec<IVec2> = fresh_coords;
    for at in &stale {
        let centre = Vec2::new((at.x as f32 + 0.5) * CELL, (at.y as f32 + 0.5) * CELL);
        for (dx, dz) in [(0.0, 0.0), (-2.5, 0.0), (2.5, 0.0), (0.0, -2.5), (0.0, 2.5)] {
            let coord = IVec2::new(
                ((centre.x + dx) / CHUNK_SIZE).floor() as i32,
                ((centre.y + dz) / CHUNK_SIZE).floor() as i32,
            );
            if !dirty.contains(&coord) {
                dirty.push(coord);
            }
        }
    }
    for at in gone {
        trails.cells.remove(&at);
    }
    if dirty.is_empty() {
        return;
    }

    let dirt = crate::palette::shade(&crate::palette::EARTH, 0.42).to_linear();
    for (chunk, mesh_handle) in &chunks {
        if !dirty.contains(&chunk.coord) {
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) else {
            continue;
        };
        let origin = Vec2::new(
            chunk.coord.x as f32 * CHUNK_SIZE,
            chunk.coord.y as f32 * CHUNK_SIZE,
        );
        let Some(positions) = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
            .map(|p| p.to_vec())
        else {
            continue;
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
        else {
            continue;
        };
        for (position, color) in positions.iter().zip(colors.iter_mut()) {
            let (x, z) = (origin.x + position[0], origin.y + position[2]);
            let wear = trails.wear_near(x, z);
            // The alpha channel is a private ledger: 1.0 means untouched
            // ground, anything less means we painted it. Opaque terrain
            // never reads alpha, so it is free bookkeeping - and it lets
            // the pass skip the thousands of vertices it never touched.
            let tinted = color[3] < 0.9995;
            if wear <= 0.0 && !tinted {
                continue;
            }
            // Passing through once is not a path: tint only begins after
            // the same ground has been walked again and again, and even a
            // hard road never fully loses the ground tone underneath.
            let blend = ((wear - 3.0) / (VISIBLE * 1.8)).clamp(0.0, 1.0) * 0.75;
            let base = ground_color_at(&terrain, x, z);
            *color = [
                base[0] + (dirt.red - base[0]) * blend,
                base[1] + (dirt.green - base[1]) * blend,
                base[2] + (dirt.blue - base[2]) * blend,
                if blend > 0.0 { 0.999 } else { 1.0 },
            ];
        }
    }
}
