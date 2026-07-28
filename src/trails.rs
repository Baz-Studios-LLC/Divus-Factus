//! Worn trails: the ground remembers where feet go.
//!
//! Nobody plans a road. Every walking villager leaves a little wear in the
//! cell under their feet; wear accumulates where routes repeat — fire to
//! shore, door to field, square to quarry — and fades where they don't.
//! Past a threshold the grass gives up and a dirt path shows, and worn
//! ground is faster underfoot, so the village's habits literally pave its
//! own shortcuts. The map of trails IS the map of the village's life, and
//! when work moves far afield, the road there draws itself.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::creature::anim::CreatureMotion;
use crate::terrain::Terrain;
use crate::villager::Villager;

/// The side of one wear cell, in world units.
const CELL: f32 = 1.7;

/// Wear at which the dirt shows through the grass.
const VISIBLE: f32 = 6.0;

/// Wear stops accumulating here: a road, not a trench.
const WEAR_CAP: f32 = 30.0;

/// Worn ground fades at this rate, per second. Slow: a trail earned over
/// a morning survives an idle afternoon.
const FADE: f32 = 0.008;

/// How much faster feet move on visibly worn ground.
const HASTE: f32 = 1.25;

/// At most this many dirt patches at once — a cap, not a plan.
const PATCH_CAP: usize = 1600;

pub struct TrailsPlugin;

impl Plugin for TrailsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Trails>()
            .add_systems(Startup, init_trail_assets)
            .add_systems(Update, (tread, maintain));
    }
}

/// One cell's memory of feet.
pub struct TrailCell {
    pub wear: f32,
    patch: Option<Entity>,
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

    /// Restores the bare wear map (from a save); patches respawn lazily.
    pub fn restore(&mut self, worn: impl Iterator<Item = (i32, i32, f32)>) {
        self.cells.clear();
        for (x, z, wear) in worn {
            self.cells
                .insert(IVec2::new(x, z), TrailCell { wear, patch: None });
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

/// A visible stretch of bare earth where the grass gave up.
#[derive(Component)]
pub struct TrailPatch;

/// The one mesh and material every patch shares.
#[derive(Resource)]
struct TrailAssets {
    mesh: Handle<Mesh>,
    dirt: Handle<StandardMaterial>,
}

fn init_trail_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(TrailAssets {
        mesh: meshes.add(Cuboid::new(CELL * 1.08, 0.05, CELL * 1.08)),
        dirt: materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::EARTH, 0.42),
            perceptual_roughness: 1.0,
            ..default()
        }),
    });
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
                patch: None,
            });
        cell.wear = (cell.wear + dt * 0.9).min(WEAR_CAP);
    }
}

/// The slow keeping of the ground: wear fades, dirt shows where it has
/// earned it and grasses over where it hasn't.
fn maintain(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    terrain: Option<Res<Terrain>>,
    assets: Option<Res<TrailAssets>>,
    mut trails: ResMut<Trails>,
) {
    *since_last += time.delta_secs();
    if *since_last < 2.0 {
        return;
    }
    let elapsed = *since_last;
    *since_last = 0.0;
    let (Some(terrain), Some(assets)) = (terrain, assets) else {
        return;
    };

    let mut patches = trails
        .cells
        .values()
        .filter(|cell| cell.patch.is_some())
        .count();
    let mut gone: Vec<IVec2> = Vec::new();
    for (at, cell) in trails.cells.iter_mut() {
        cell.wear -= FADE * elapsed;
        if cell.wear <= 0.05 {
            if let Some(patch) = cell.patch.take() {
                commands.entity(patch).despawn();
                patches -= 1;
            }
            gone.push(*at);
            continue;
        }
        if cell.wear >= VISIBLE && cell.patch.is_none() && patches < PATCH_CAP {
            let x = (at.x as f32 + 0.5) * CELL;
            let z = (at.y as f32 + 0.5) * CELL;
            let patch = commands
                .spawn((
                    TrailPatch,
                    Mesh3d(assets.mesh.clone()),
                    MeshMaterial3d(assets.dirt.clone()),
                    Transform::from_xyz(x, terrain.height_at(x, z) + 0.03, z),
                    bevy::light::NotShadowCaster,
                ))
                .id();
            cell.patch = Some(patch);
            patches += 1;
        } else if cell.wear < VISIBLE - 1.0
            && let Some(patch) = cell.patch.take()
        {
            // Hysteresis: a whole point of wear between showing and
            // grassing over, so a border cell does not flicker.
            commands.entity(patch).despawn();
            patches -= 1;
        }
    }
    for at in gone {
        trails.cells.remove(&at);
    }
}
