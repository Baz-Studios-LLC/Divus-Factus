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
        // ON FOR NOW, while this is being got right - Brett: "Lets make on
        // the default state for now for testing though." It goes back to off
        // once it has been seen working, which is what the settings switch is
        // for.
        ShowTheReach(true)
    }
}

/// The ring on the ground.
#[derive(Component)]
struct ReachRing;

/// Segments around the ring.
const AROUND: usize = 64;

/// Where the band's inner edge is, as a fraction of the radius.
///
/// NEARLY THE WHOLE DISC. This was 0.82 - a rim of under a fifth of the
/// radius - and at the close zoom where the reach floors out at 0.6m that is
/// a ring the size of a dinner plate with an eleven centimeter rim, drawn
/// under a hand model bigger than the whole thing. Brett had it switched on
/// and was looking straight at it.
///
/// The reach really is that small up close, and the ring must not lie about
/// it, so the answer is not a bigger circle - it is a circle you can see: a
/// broad disc that fades out rather than a hairline.
const BAND: f32 = 0.35;

/// How high off the ground it sits, in meters.
///
/// THE SURVEY SHEET'S OWN NUMBER, and arrived at the same way everything else
/// here finally was: by asking what the working thing does. It floats 0.45,
/// which is a great deal more than the 0.14 that seemed generous, and it
/// measures with `height_at` rather than `base_height_at` - two different
/// surfaces. The base height ignores the rivers and the carving, so wherever
/// they differ a ring measured against it is UNDER the ground that actually
/// gets drawn, and a seventh of a meter is not enough to climb out.
const CLEARANCE: f32 = 0.45;

/// How hard the ring wins the depth fight against the ground.
///
/// EIGHT, which is what blood uses to lie on the earth without the ground
/// eating it - and blood is the proof that eight is enough. This was two
/// thousand on the theory that more is safer, and more is not safer: a bias
/// that large pushes the fragment's depth out of the buffer's range entirely
/// and the test can never pass, which is its own way of drawing nothing.
const OVER_THE_WORLD: f32 = 8.0;

/// TEMPORARY: how many times its true size the ring is drawn at.
///
/// Brett, while we get this seen at all: "Try making it way bigger for now to
/// test too." MUST GO BACK TO 1.0 - the whole claim the ring makes is that it
/// is exactly what the hand would take, and at six times that it is a lie
/// that happens to be easy to find.
const TESTING_BIGGER: f32 = 6.0;

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
    mut said: Local<f32>,
    clock: Res<Time>,
    eyes: Query<&GlobalTransform, With<crate::camera::GodCamera>>,
) {
    // A LOUD PROBE, because this cannot be seen from an unattended capture:
    // the cursor is nowhere in one, and where the ring lands on screen is
    // exactly what a screenshot cannot tell me. `DIVUS_FACTUS_REACH_PROBE=1`
    // says, twice a second, whether the system runs, whether the hand has a
    // point on the ground, and how many rings are standing - which separates
    // "never drawn" from "drawn and invisible" in one line.
    *said += clock.delta_secs();
    let telling = std::env::var("DIVUS_FACTUS_REACH_PROBE").is_ok() && *said > 0.5;
    if telling {
        *said = 0.0;
        info!(
            "reach: on={} held={} ground={:?} rings={}",
            show.0,
            hand.held.is_some(),
            hand.cursor_world.map(|at| (at.x, at.z)),
            ring.iter().count()
        );
    }

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
    let radius =
        super::forgiveness_at(rigs.single().map_or(80.0, |rig| rig.distance)) * TESTING_BIGGER;
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
    let painted = paint_the_ring(&terrain, seat, radius);
    if std::env::var("DIVUS_FACTUS_REACH_PROBE").is_ok()
        && let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(points)) =
            painted.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        info!(
            "reach mesh: {} corners, first {:?}, eye {:?}",
            points.len(),
            points.first(),
            eyes.iter().next().map(|g| g.translation())
        );
    }
    commands.spawn((
        Name::new("The hand's reach"),
        ReachRing,
        Mesh3d(meshes.add(painted)),
        MeshMaterial3d(materials.add(StandardMaterial {
            // Over one, so the bloom already on the god's camera carries it -
            // which is where the "ethereal" is meant to come from.
            // `base_color` is LDR whatever is written here, so the glow does
            // not come from overdriving it - it comes from the bloom on the
            // god's camera, and this is as bright as a color gets before it
            // stops reading as a color at all.
            base_color: Color::srgba(0.62, 0.88, 1.0, 0.55),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            depth_bias: OVER_THE_WORLD,
            ..default()
        })),
        // The corners carry the position; this is never moved. See the module
        // head, and `survey`, which does the same.
        Transform::default(),
        // AND THE BEND MUST NOT TOUCH IT. This is what buried three attempts:
        // an identity transform is not "already in world space" to the bend,
        // it is a flat point at the origin waiting to be wrapped, and
        // wrapping it seats the whole thing about twenty-eight units under
        // the ground. The survey sheet had the same hole, which is why the
        // resource heatmap stopped drawing too - and why copying it faithfully
        // copied the fault.
        crate::globe::BentInPlace,
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
    ));
}

/// A flat band around `seat`, every corner bent onto the sphere.
fn paint_the_ring(terrain: &crate::terrain::Terrain, seat: Vec3, radius: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity((AROUND + 1) * 2);
    // Per-corner alpha: faint across the disc, bright at the rim. A hard band
    // of one strength reads as a decal somebody stuck on the grass; a disc
    // that gathers into its edge reads as light lying on the ground.
    let mut tints: Vec<[f32; 4]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(AROUND * 6);

    for i in 0..=AROUND {
        let angle = i as f32 / AROUND as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let mut corner = |reach: f32, glow: f32| {
            let x = seat.x + cos * reach;
            let z = seat.z + sin * reach;
            // `height_at`, the same as the survey sheet - this has to be the
            // surface that is DRAWN, not the one underneath it. It takes the
            // river index's write lock and can be slow on ground nobody has
            // solved yet, which is why the ring only rebuilds when the cursor
            // has actually moved: these hundred and thirty cells are the ones
            // directly under the player's eye, and those are warm.
            let y = terrain.height_at(x, z) + CLEARANCE;
            let (bent, turn) = crate::globe::bend_frame(Vec3::new(x, y, z));
            positions.push(bent.to_array());
            normals.push((turn * Vec3::Y).to_array());
            uvs.push([i as f32 / AROUND as f32, 0.0]);
            tints.push([0.62, 0.88, 1.0, glow]);
        };
        // Solid for now, both edges. The fade goes back on with the alpha.
        corner(radius * BAND, 1.0);
        corner(radius, 1.0);

        if i < AROUND {
            let base = i as u32 * 2;
            // WOUND TO FACE UP. This was the other way round, which points
            // every triangle at the ground - and a back face is culled, so
            // the ring was drawn, correctly placed, thirty-five units in
            // front of the eye, and facing away from it the whole time.
            //
            // The survey sheet, which is where all of this was copied from,
            // winds its quads `[a, c, b, a, d, c]`. Mine did not, and that
            // one transposition is what survived four attempts, a rewrite,
            // and every other difference being eliminated.
            indices.extend([base, base + 3, base + 1, base, base + 2, base + 3]);
        }
    }

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, tints);
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
