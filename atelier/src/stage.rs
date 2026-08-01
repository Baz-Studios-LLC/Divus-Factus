//! The stage: floor, grid, axis marks, lights, and the rig bench's pedestal.

use bevy::prelude::*;

use crate::Bench;
use crate::look::Palette;

/// The grid spans this many metres each way from the origin.
const REACH: f32 = 14.0;

/// Stage furniture that belongs to the builder's bench.
#[derive(Component)]
pub struct BuilderFurniture;

/// Stage furniture that belongs to the rig bench.
#[derive(Component)]
pub struct RigFurniture;

pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, dress_stage)
            .add_systems(Update, follow_bench);
    }
}

fn dress_stage(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
) {
    // Workshop light: a keyed sun and a soft fill, no weather, no hour.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.75, 0.78, 0.85),
        brightness: 220.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.8, -0.9, 0.0)),
    ));

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let matte = |materials: &mut Assets<StandardMaterial>, color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.95,
            reflectance: 0.03,
            ..default()
        })
    };

    // The bench floor: near-black, so authored colour reads true against it.
    let floor = matte(&mut materials, Color::srgb(0.05, 0.055, 0.065));
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(floor),
        Transform::from_xyz(0.0, -0.5, 0.0).with_scale(Vec3::new(
            REACH * 2.0 + 8.0,
            1.0,
            REACH * 2.0 + 8.0,
        )),
    ));

    // The metre grid, drawn in dim bone. Every fourth line stands brighter,
    // which is enough structure to count cells by eye.
    let faint = matte(&mut materials, palette.shade("bone", 0.3).with_alpha(1.0));
    let strong = matte(&mut materials, palette.shade("bone", 0.55));
    let span = REACH * 2.0;
    let lines = (REACH * 2.0) as i32 + 1;
    for i in 0..lines {
        let at = -REACH + i as f32;
        let bold = (at as i32) % 4 == 0;
        let material = if bold { strong.clone() } else { faint.clone() };
        let lift = if bold { 0.012 } else { 0.008 };
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(at, lift, 0.0).with_scale(Vec3::new(0.024, 0.016, span)),
        ));
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, lift, at).with_scale(Vec3::new(span, 0.016, 0.024)),
        ));
    }

    // The plot centre, marked with a gold cross: the origin every exported
    // part is measured from. Buildings need not be centred perfectly - the
    // god recentres on import - but the mark keeps the eye honest.
    for (sx, sz) in [(1.4, 0.05), (0.05, 1.4)] {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(gold_center(&mut materials, &palette)),
            Transform::from_xyz(0.0, 0.02, 0.0).with_scale(Vec3::new(sx, 0.02, sz)),
        ));
    }

    // The door mark: buildings face the village with their local +X, so the
    // +X edge of the grid wears a gold sill. What you build toward the gold
    // is what the village sees first.
    let gold = matte(&mut materials, palette.shade("cloth-gold", 0.85));
    commands.spawn((
        BuilderFurniture,
        Mesh3d(cube.clone()),
        MeshMaterial3d(gold.clone()),
        Transform::from_xyz(REACH + 0.6, 0.05, 0.0).with_scale(Vec3::new(0.12, 0.1, 3.0)),
    ));

    // The rig bench's pedestal, waiting for its body.
    let stone = matte(&mut materials, palette.shade("stone", 0.6));
    let dark_stone = matte(&mut materials, palette.shade("stone", 0.4));
    commands.spawn((
        RigFurniture,
        Mesh3d(cube.clone()),
        MeshMaterial3d(dark_stone),
        Transform::from_xyz(0.0, 0.11, 0.0).with_scale(Vec3::new(3.4, 0.22, 3.4)),
        Visibility::Hidden,
    ));
    commands.spawn((
        RigFurniture,
        Mesh3d(cube),
        MeshMaterial3d(stone),
        Transform::from_xyz(0.0, 0.34, 0.0).with_scale(Vec3::new(2.6, 0.24, 2.6)),
        Visibility::Hidden,
    ));
}

fn gold_center(
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: palette.shade("cloth-gold", 0.7),
        perceptual_roughness: 0.95,
        reflectance: 0.03,
        ..default()
    })
}

/// Each bench keeps its own furniture on stage and the other's put away.
fn follow_bench(
    bench: Res<Bench>,
    mut builder: Query<&mut Visibility, (With<BuilderFurniture>, Without<RigFurniture>)>,
    mut rig: Query<&mut Visibility, With<RigFurniture>>,
) {
    if !bench.is_changed() {
        return;
    }
    for mut visibility in &mut builder {
        *visibility = if *bench == Bench::Builder {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut visibility in &mut rig {
        *visibility = if *bench == Bench::Rig {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
