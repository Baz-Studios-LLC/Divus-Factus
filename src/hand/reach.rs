//! The hand's reach, drawn on the ground.
//!
//! A ring where the hand would close. Off by default and turned on from the
//! settings — Brett: "I think we should paint an ethereal circle on the
//! ground that signifies the selection area. This should be a toggle in the
//! menu that defaults to off but can be turned on."
//!
//! # THIS IS THE THIRD ATTEMPT, AND IT IS DELIBERATELY THE DUMBEST
//!
//! The first two did not draw, and each failure cost an hour of taking the
//! wrong thing apart. So this one has NOTHING in it that could fail quietly:
//! no custom shader, no smoke, no `RigidlySeated`, no `uuid_handle`, no
//! per-frame mesh replacement, no additive blending. It is a flat unlit ring
//! built exactly the way [`crate::survey`] builds the god's survey sheet —
//! the one thing in this game that has always successfully painted across the
//! ground:
//!
//! - every corner bent onto the sphere as the mesh is built
//! - an ordinary `meshes.add`, a fresh mesh on a fresh entity
//! - an identity transform that is never touched again
//! - a plain unlit `StandardMaterial`
//!
//! Brett asked for glow and smoke — "We could use aspectus for the circle to
//! make it glow and have smoke subtly coming off of it" — and the glow is
//! nearly free once this is confirmed on screen, because the god's camera is
//! HDR with bloom on it and any color over 1.0 blooms by itself. The smoke
//! goes back on after that. Neither is worth another hour before the plain
//! ring has been SEEN.
//!
//! What the first two attempts ruled out, so nobody spends that hour again:
//! the bend seats these correctly (measured — the drop of ~325m at 1970m out
//! is exactly a six-thousand-meter world's curvature), `ViewVisibility` comes
//! back true, and blowing the radius up to sixty meters put the thing on
//! screen immediately. The drawing was never broken. At two meters it simply
//! could not be found, which is its own kind of answer.

use bevy::prelude::*;

/// Whether the ring is drawn. Off, until somebody asks for it.
#[derive(Resource)]
pub struct ShowTheReach(pub bool);

impl Default for ShowTheReach {
    fn default() -> Self {
        // Off, except that an unattended capture has no settings screen to
        // click - `DIVUS_FACTUS_REACH=1` is how the ring gets photographed.
        ShowTheReach(std::env::var("DIVUS_FACTUS_REACH").is_ok())
    }
}

/// The ring on the ground.
#[derive(Component)]
struct ReachRing;

/// Segments around the ring.
const AROUND: usize = 64;

/// How wide the painted band is, as a fraction of the radius.
///
/// A BAND, not a hairline. A one-pixel circle is a thing you squint at, and
/// the whole point of this is to be readable at a glance.
const BAND: f32 = 0.82;

/// How high off the ground it sits, in meters.
///
/// The ground under it is sampled with `base_height_at`, but the terrain that
/// gets DRAWN is a chunk mesh sampled at the chunk's own spacing and
/// interpolated between - so between vertices the drawn surface can sit above
/// the height the ring measured, and the ring is under the world. Blood hit
/// exactly this and answered it with a depth bias; this does both.
const CLEARANCE: f32 = 0.14;

/// How hard the ring wins the depth fight against the ground.
///
/// Blood needed 8 to stop flickering where it lay. A ring is thinner and the
/// grass stands in front of it as well, so it takes a great deal more: this
/// is a pointer aid and it is allowed to be drawn over the world, the way a
/// cursor is.
const OVER_THE_WORLD: f32 = 2_000.0;

/// How far the cursor must move before the ring is rebuilt, in meters.
///
/// Rebuilding every frame means a new mesh asset every frame, and this is a
/// pointer aid - it should not be the busiest allocator in the game.
const WORTH_REDRAWING: f32 = 0.15;

pub struct ReachPlugin;

impl Plugin for ReachPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShowTheReach>()
            .add_systems(Update, draw_the_reach.after(super::CameraSet));
    }
}

/// Redraws the ring under the cursor, or takes it away.
fn draw_the_reach(
    mut commands: Commands,
    show: Res<ShowTheReach>,
    hand: Res<super::DivineHand>,
    rigs: Query<&crate::camera::CameraRig>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ring: Query<Entity, With<ReachRing>>,
    mut standing: Local<Option<(Vec3, f32)>>,
) {
    let showing = show.0 && hand.held.is_none();
    let (Some(terrain), Some(at)) = (terrain, hand.cursor_world.filter(|_| showing)) else {
        for old in &ring {
            commands.entity(old).despawn();
        }
        *standing = None;
        return;
    };

    // EXACTLY WHAT THE HAND WOULD TAKE - the same number the pick itself
    // uses, or the ring is a lie that still looks right.
    let radius = super::forgiveness_at(rigs.single().map_or(80.0, |rig| rig.distance));
    let ground = terrain.base_height_at(at.x, at.z);
    let seat = Vec3::new(at.x, ground, at.z);

    if standing
        .is_some_and(|(was, r)| was.distance(seat) < WORTH_REDRAWING && (r - radius).abs() < 0.05)
    {
        return;
    }
    *standing = Some((seat, radius));

    for old in &ring {
        commands.entity(old).despawn();
    }
    commands.spawn((
        Name::new("The hand's reach"),
        ReachRing,
        Mesh3d(meshes.add(paint_the_ring(&terrain, seat, radius))),
        MeshMaterial3d(materials.add(StandardMaterial {
            // Over one, so the bloom already on the god's camera carries it -
            // which is where the "ethereal" is meant to come from.
            base_color: Color::srgb(1.6, 2.6, 3.4),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            depth_bias: OVER_THE_WORLD,
            ..default()
        })),
        // The corners carry the position; this is never moved. See the module
        // head, and `survey`, which does the same.
        Transform::default(),
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
    ));
}

/// A flat band around `seat`, every corner bent onto the sphere.
fn paint_the_ring(terrain: &crate::terrain::Terrain, seat: Vec3, radius: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(AROUND * 6);

    for i in 0..=AROUND {
        let angle = i as f32 / AROUND as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let mut corner = |reach: f32| {
            let x = seat.x + cos * reach;
            let z = seat.z + sin * reach;
            // `base_height_at`, never `height_at`: the latter takes the river
            // index's write lock and can solve a whole unseen region on the
            // way past, and this runs whenever the cursor moves.
            let y = terrain.base_height_at(x, z) + CLEARANCE;
            let (bent, turn) = crate::globe::bend_frame(Vec3::new(x, y, z));
            positions.push(bent.to_array());
            normals.push((turn * Vec3::Y).to_array());
            uvs.push([i as f32 / AROUND as f32, 0.0]);
        };
        corner(radius * BAND);
        corner(radius);

        if i < AROUND {
            let base = i as u32 * 2;
            indices.extend([base, base + 1, base + 3, base, base + 3, base + 2]);
        }
    }

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    /// The ring is drawn at the size the hand actually reaches. That is the
    /// whole claim it makes, and a ring that was merely circle-sized would be
    /// a lie that still looked right.
    #[test]
    fn the_ring_grows_with_the_view() {
        assert!(super::super::forgiveness_at(200.0) > super::super::forgiveness_at(20.0));
    }
}
