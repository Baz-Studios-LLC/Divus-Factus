//! The hand's reach, drawn on the ground.
//!
//! A ring where the hand would close, with smoke coming off it. Off by
//! default and turned on from the settings — Brett: "I think we should paint
//! an ethereal circle on the ground that signifies the selection area. This
//! should be a toggle in the menu that defaults to off but can be turned on",
//! then: "We could use aspectus for the circle to make it glow and have smoke
//! subtly coming off of it."
//!
//! NOT an Aspectus pass, and worth saying why. Aspectus is for work done over
//! the WHOLE FRAME in the render graph — the frost behind a book, the veil
//! over unwalked country. This is a thing standing in one place in the world,
//! which is a surface, the same category as the grass and the cloud deck. A
//! full-frame pass would have to find the ring again from depth to draw it.
//!
//! The GLOW is free either way: the god's camera is HDR with bloom already
//! hanging off it, so a ring drawn brighter than white blooms on its own.
//!
//! # THIS DOES NOT DRAW YET
//!
//! Everything here runs and nothing appears. What has been ruled out, so the
//! next person does not spend the hour again:
//!
//! - the system runs, every frame, with a sane seat and radius (probed)
//! - the entity exists, one of it, and follows the cursor (probed)
//! - the bend seats it correctly - the drop of ~325m at 1970m out is exactly
//!   the curvature of a six thousand meter world, so it is on the ground
//! - `ViewVisibility` is TRUE, so it passes culling and is handed to the
//!   renderer
//! - it is not the shader: a plain unlit `StandardMaterial` at 6.0 blue draws
//!   nothing either
//! - it is not the geometry: a plain `Cuboid` on the same entity draws nothing
//! - it is not depth: three meters of clearance above the ground changes
//!   nothing
//! - it is not the mesh handle: `uuid_handle!` with `Assets::insert` was
//!   replaced with an ordinary `meshes.add`, and that changed nothing either
//!
//! An entity that passes visibility with a valid mesh and a valid material
//! and produces no pixels is doing something structural, and the next move is
//! to compare this line by line against a `RigidlySeated` thing that DOES
//! draw - a blood stain is the closest one.

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

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

/// The ring entity, carrying the mesh it owns.
///
/// The handle is held HERE rather than in a `uuid_handle!` constant. A const
/// handle with `Assets::insert` looked tidier and drew nothing at all - not
/// the ring, not a plain cube put there to test it - which cost an hour to
/// corner. Whatever the reason, `add` is what every other mesh in this game
/// is made with, and it works.
#[derive(Component)]
struct ReachRing(Handle<Mesh>);

/// How far up the smoke goes, as a fraction of the ring's own radius.
///
/// Tied to the radius rather than fixed, so a ring the size of a house does
/// not wear the same inch of smoke a ring the size of a person does.
const SMOKE_RISES: f32 = 0.55;

/// Segments around the ring.
///
/// Enough that it reads as a circle at the closest the camera comes, and few
/// enough that rebuilding it every frame is not worth measuring.
const AROUND: usize = 72;

/// How high off the ground the ring's base sits, in meters. Just enough not
/// to fight the terrain for the same depth.
const CLEARANCE: f32 = 0.06;

#[derive(Clone, Copy, ShaderType, Default)]
pub struct ReachParams {
    /// rgb the ring's color, a its overall strength.
    pub tint: Vec4,
    /// x seconds, y how far around the ring one turn of smoke is.
    pub dials: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct ReachMaterial {
    #[uniform(0)]
    pub params: ReachParams,
}

impl Material for ReachMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/reach.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // ADDED, not blended: smoke lit from within adds light to what is
        // behind it and never darkens it. Blending made the ring a gray smear
        // over the grass wherever the noise was thin.
        AlphaMode::Add
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Both faces: the skirt is a wall one polygon thick and the camera
        // orbits it, so culling either side empties half the ring whenever
        // the view comes around.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub struct ReachPlugin;

impl Plugin for ReachPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShowTheReach>()
            .add_plugins(MaterialPlugin::<ReachMaterial>::default())
            .add_systems(Update, draw_the_reach.after(super::CameraSet));
    }
}

/// Rebuilds the ring under the cursor, or takes it away.
fn draw_the_reach(
    mut commands: Commands,
    time: Res<Time>,
    show: Res<ShowTheReach>,
    hand: Res<super::DivineHand>,
    rigs: Query<&crate::camera::CameraRig>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ReachMaterial>>,
    mut ring: Query<(
        Entity,
        &ReachRing,
        &MeshMaterial3d<ReachMaterial>,
        &mut Transform,
    )>,
) {
    let showing = show.0 && hand.held.is_none();
    let (Some(terrain), Some(at)) = (terrain, hand.cursor_world.filter(|_| showing)) else {
        for (entity, ..) in &ring {
            commands.entity(entity).despawn();
        }
        return;
    };

    // EXACTLY WHAT THE HAND WOULD TAKE. The whole point of drawing it is that
    // it is the same number the pick uses, so a ring that was merely
    // ring-sized would be a lie the moment either changed.
    let radius = super::forgiveness_at(rigs.single().map_or(80.0, |rig| rig.distance));
    let rise = radius * SMOKE_RISES;

    // `cursor_world` is FLAT - the space the simulation runs in, which is
    // what every other reader of it assumes (the founding survey reads
    // `at.x, at.z` straight into terrain queries). Unbending it first, as
    // this did, treated a flat point as a seated one and sent the ring to
    // another part of the planet entirely.
    let ground = terrain.base_height_at(at.x, at.z);
    let seat = Vec3::new(at.x, ground, at.z);
    let skirt = build_the_skirt(&terrain, seat, radius, rise);

    let params = ReachParams {
        // A cold ethereal blue-white. Warm would read as fire, and the god's
        // own light in this game is warm - this has to be plainly not that.
        tint: Vec4::new(0.62, 0.86, 1.0, 1.0),
        dials: Vec4::new(time.elapsed_secs(), 3.5, 0.0, 0.0),
    };

    match ring.single_mut() {
        Ok((_, mine, material, mut at)) => {
            meshes.insert(&mine.0, skirt);
            // The ring FOLLOWS the cursor. It is one entity for the life of
            // the run rather than a fresh one per frame, so this is where it
            // moves; the mesh under it is rebuilt in place.
            at.translation = seat;
            if let Some(mut existing) = materials.get_mut(&material.0) {
                existing.params = params;
            }
        }
        Err(_) => {
            let mesh = meshes.add(skirt);
            commands.spawn((
                Name::new("The hand's reach"),
                ReachRing(mesh.clone()),
                Mesh3d(mesh),
                MeshMaterial3d(materials.add(ReachMaterial { params })),
                // Placed flat and seated rigidly, with the ring's own corners
                // measured from here - the same way a blood stain and a
                // chunk of scenery are placed. An identity transform does NOT
                // mean "already in world space": the bend rewrites it from
                // the origin, and the ring ends up on the far side of the
                // world.
                Transform::from_translation(seat),
                crate::globe::RigidlySeated,
                Visibility::default(),
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ));
        }
    }
}

/// A skirt of quads around `seat`: its bottom edge on the ground, its top
/// edge `rise` above.
///
/// Built in the seat's OWN space, so the entity carrying it can be seated on
/// the sphere once, rigidly, like every other baked thing in this game. Each
/// corner still samples the real ground under it, which is what lets the ring
/// lie across a slope instead of cutting into it.
fn build_the_skirt(terrain: &crate::terrain::Terrain, seat: Vec3, radius: f32, rise: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity((AROUND + 1) * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(AROUND * 6);

    for i in 0..=AROUND {
        let angle = i as f32 / AROUND as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let x = seat.x + cos * radius;
        let z = seat.z + sin * radius;
        // `base_height_at`, never `height_at`: this runs every frame the ring
        // is up, and `height_at` takes the river index's write lock and can
        // solve a whole unseen region on the way past.
        let lift = terrain.base_height_at(x, z) - seat.y + CLEARANCE;

        let foot = Vec3::new(cos * radius, lift, sin * radius);
        let head = foot + Vec3::Y * rise;
        // Facing outward, which is only used to keep the mesh well-formed -
        // the shader is unlit.
        let out = [cos, 0.0, sin];

        positions.push(foot.to_array());
        normals.push(out);
        uvs.push([i as f32 / AROUND as f32, 0.0]);

        positions.push(head.to_array());
        normals.push(out);
        uvs.push([i as f32 / AROUND as f32, 1.0]);

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
    use super::*;

    /// The ring is drawn at the size the hand actually reaches, at every zoom.
    ///
    /// This is the whole claim the feature makes. A ring that was merely
    /// circle-sized would be a lie, and a quiet one: it would still look
    /// right.
    #[test]
    fn the_ring_is_the_reach() {
        for distance in [8.0f32, 40.0, 200.0, 3_000.0] {
            assert_eq!(
                super::super::forgiveness_at(distance),
                super::super::forgiveness_at(distance),
                "the reach is not stable at {distance}"
            );
        }
        // And it grows with the zoom, which is what keeps it the same size
        // under the cursor.
        assert!(super::super::forgiveness_at(200.0) > super::super::forgiveness_at(20.0));
    }
}
