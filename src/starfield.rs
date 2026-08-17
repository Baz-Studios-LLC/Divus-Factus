//! The stars, in three shells at three depths.
//!
//! Real stars, in world space, at real distances — which is the whole point. A
//! procedural sky shader would paint the same field of dots for a tenth of the
//! cost and it would be pinned at infinity: turn the camera and nothing in it
//! moves against anything else. Put the stars at three depths instead and the
//! parallax comes free, because the near shell sweeps past the far one exactly
//! as much as the geometry says it should. The title screen circles the planet
//! on a thirty-seven-thousand-unit arc, so there is a great deal of sweeping to
//! be had.
//!
//! They fade in as the sky goes dark: with the climb, the way the sky itself
//! thins to black, and again at night down at the ground. Stars in a blue
//! midday sky would be a bug, and stars that only exist on the title screen
//! would be a waste of them.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;

use crate::camera::{CameraRig, GodCamera};
use crate::palette;
use crate::rng::hash_2d_f32;

/// The three shells, in units from the planet's center.
///
/// All inside the camera's seventy-thousand far plane, and spread widely enough
/// that the nearest sweeps visibly against the furthest. Bring them closer
/// together and the parallax dies; push the near one in and the stars start to
/// read as fireflies a mile off rather than as a sky.
const SHELLS: [f32; 3] = [52_000.0, 60_000.0, 67_500.0];

/// How many stars each shell carries, near to far. More in the far shells, so
/// the depth reads as depth rather than as three separate fields.
const COUNT: [usize; 3] = [420, 760, 1_150];

/// How big a star is, in units, at the nearest shell. A frame is about
/// twenty-two units to the pixel out there, so this is a star a pixel or two
/// across, with the bright ones a little more.
const SIZE: f32 = 34.0;

/// Marks the field, so it can be found and faded.
#[derive(Component)]
struct Starfield;

pub struct StarfieldPlugin;

impl Plugin for StarfieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, hang_the_stars)
            .add_systems(Update, let_the_stars_out);
    }
}

/// Builds the three shells, once, as one mesh apiece.
fn hang_the_stars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    seed: Res<crate::WorldSeed>,
) {
    // One material for all three shells: unlit, because a star is a light and
    // not a thing lit by one, and added rather than blended so a field of them
    // over black reads as light rather than as pale paint.
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::WHITE * 6.0,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        // Both sides. A star is a flat quad, and which side of a flat quad you
        // are looking at is decided by its WINDING and not by the normal I gave
        // it — so getting the winding backwards makes the whole sky invisible
        // rather than merely dark. The globe learned this the same way. Double-
        // sided costs nothing here and settles the question.
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    let center = crate::globe::planet_center();
    info!(
        "the stars are hung: {} of them over three shells",
        COUNT.iter().sum::<usize>()
    );
    for (shell, (radius, count)) in SHELLS.iter().zip(COUNT).enumerate() {
        let mesh = build_a_shell(
            *radius,
            count,
            seed.0 ^ (shell as u32).wrapping_mul(0x9e37_79b9),
        );
        commands.spawn((
            Name::new(format!("Stars {}", shell + 1)),
            Starfield,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center),
            // Seen from the ground and from orbit both.
            RenderLayers::from_layers(&[0, crate::globe::GLOBE_LAYER]),
            NotShadowCaster,
            NotShadowReceiver,
            // Its vertices are already world-space offsets from the planet's
            // center; the bend must leave the shell where it stands.
            crate::globe::BentInPlace,
        ));
    }
}

/// One shell: `count` quads scattered over a sphere of `radius`, each facing the
/// middle of it.
fn build_a_shell(radius: f32, count: usize, seed: u32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(count * 4);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(count * 4);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(count * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(count * 6);

    for star in 0..count {
        let i = star as i32;
        // Scattered EVENLY over the sphere, which a naive pair of angles does
        // not do: taking latitude straight from a random number crowds the
        // poles, and a sky with two bald patches and two bright ones is
        // immediately readable as a mistake. Inverting the cosine spreads them.
        let a = hash_2d_f32(i, 1, seed);
        let b = hash_2d_f32(i, 2, seed);
        let height = a * 2.0 - 1.0;
        let ring = (1.0 - height * height).max(0.0).sqrt();
        let angle = b * std::f32::consts::TAU;
        let direction = Vec3::new(ring * angle.cos(), height, ring * angle.sin());
        let at = direction * radius;

        // Two axes across the star's own facing, so the quad squares up to the
        // middle of the shell — which is where the camera always is, near
        // enough, at this range.
        let across = direction.cross(Vec3::Y).normalize_or(Vec3::X);
        let down = across.cross(direction).normalize_or(Vec3::Z);

        // Most stars are small and faint; a few are neither. Squaring the roll
        // is what makes the field read as a sky rather than as a spray of
        // identical dots.
        let roll = hash_2d_f32(i, 3, seed);
        let brightness = 0.25 + roll * roll * roll * 2.6;
        let size = SIZE * (radius / SHELLS[0]) * (0.6 + roll * roll * 1.9);

        // And a few of them are warm or cold rather than white. Kept subtle:
        // this is a palette-disciplined world and a rainbow sky would shout.
        let tint = hash_2d_f32(i, 4, seed);
        let color = if tint > 0.86 {
            palette::shade(&palette::CLOTH_GOLD, 0.9).to_linear()
        } else if tint < 0.14 {
            palette::shade(&palette::SKY, 1.0).to_linear()
        } else {
            palette::shade(&palette::BONE, 1.0).to_linear()
        };

        let base = positions.len() as u32;
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            positions.push((at + across * (u * size) + down * (v * size)).to_array());
            normals.push((-direction).to_array());
            colors.push([
                color.red * brightness,
                color.green * brightness,
                color.blue * brightness,
                1.0,
            ]);
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}

/// Lets the stars out as the sky goes dark, and puts them away when it does not.
fn let_the_stars_out(
    sky: Res<crate::calendar::Sky>,
    rigs: Query<&CameraRig, With<GodCamera>>,
    fields: Query<&MeshMaterial3d<StandardMaterial>, With<Starfield>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(rig) = rigs.single() else {
        return;
    };
    // Two ways for a sky to be dark, and the stars answer to both: the climb,
    // on the same ramp that takes the sky itself to black, and nightfall at any
    // height.
    let climbed = ((rig.distance - crate::globe::ASCENT) / 9_000.0).clamp(0.0, 1.0);
    let nightfall = 1.0 - sky.daylight;
    let out = climbed.max(nightfall * 0.85);

    for material in &fields {
        if let Some(mut stuff) = materials.get_mut(&material.0) {
            stuff.emissive = LinearRgba::WHITE * (6.0 * out);
            stuff.base_color = Color::WHITE.with_alpha(out);
        }
    }
}
