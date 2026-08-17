//! The sky dome.
//!
//! Until now the sky was one flat color — the fog color, stretched over
//! everything, which read as permanent overcast. This dome puts an actual sky
//! behind the fog: blue overhead, procedural clouds adrift, the sun's glow —
//! all shaded in `sky.wgsl` from the same [`crate::calendar::Sky`] state that
//! drives the lights, so dusk gilds the clouds and night blacks them out.
//!
//! The one hard rule is inherited from the fog work: the dome's color *at the
//! horizon* must be exactly `Sky::horizon`, or fully-fogged terrain stops
//! matching the sky behind it and the seam draws itself.
//!
//! Cloudiness is a parameter with one value today. It is the handle weather
//! will turn tomorrow.

use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use crate::camera::GodCamera;
use crate::palette;

const SHADER_PATH: &str = "shaders/sky.wgsl";

/// Dome radius: inside the camera's far plane (3000), and beyond the farthest
/// the fog ever reaches — the dome must only ever be seen through full fog, or
/// its rim would draw a line across the world.
const DOME_RADIUS: f32 = 2700.0;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        // The dome is not raised. It was built in the fog's era with a radius
        // of 2,700 and a rule — "only ever seen through full fog" — that the
        // fog's removal quietly broke: a camera-riding sphere painted the
        // sky's color occludes everything past its radius, and once the
        // planet stood behind the world, the dome blanketed it gray by day
        // and navy by night. Every altitude where the world "turned gray"
        // was exactly where the visible ground crossed 2,700 units. Removed
        // to prove the diagnosis on screen; if the sky wants a dome again it
        // must grow with altitude so the planet's limb always fits inside.
        app.add_plugins(MaterialPlugin::<SkyDomeMaterial>::default())
            .add_systems(Update, (follow_camera, follow_the_hours));
    }
}

/// Uniform block for the sky shader.
#[derive(Clone, ShaderType, Debug)]
pub struct SkyParams {
    pub horizon: Vec4,
    pub zenith: Vec4,
    pub cloud: Vec4,
    pub sun_dir: Vec4,
    /// x: time, y: cloudiness, z: daylight, w: unused.
    pub misc: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SkyDomeMaterial {
    #[uniform(0)]
    pub params: SkyParams,
}

impl Material for SkyDomeMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Viewed from inside; the outside of the sphere is never seen.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Marks the dome entity.
#[allow(dead_code)]
#[derive(Component)]
struct SkyDome;

fn linear(color: Color) -> Vec4 {
    let c = color.to_linear();
    Vec4::new(c.red, c.green, c.blue, 1.0)
}

#[allow(dead_code)]
fn spawn_dome(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkyDomeMaterial>>,
) {
    commands.spawn((
        Name::new("Sky"),
        SkyDome,
        Mesh3d(meshes.add(Sphere::new(DOME_RADIUS).mesh().uv(48, 24))),
        MeshMaterial3d(materials.add(SkyDomeMaterial {
            params: SkyParams {
                horizon: linear(crate::render::horizon_color()),
                zenith: linear(palette::shade(&palette::SKY, 0.5)),
                cloud: linear(palette::shade(&palette::BONE, 1.0)),
                sun_dir: crate::SUN_DIRECTION.extend(0.0),
                misc: Vec4::new(0.0, 0.55, 1.0, 0.0),
            },
        })),
        Transform::default(),
        bevy::light::NotShadowCaster,
    ));
}

/// The dome rides with the camera, so its horizon is always the horizon.
fn follow_camera(
    cameras: Query<&GlobalTransform, With<GodCamera>>,
    mut domes: Query<&mut Transform, With<SkyDome>>,
) {
    let (Ok(camera), Ok(mut dome)) = (cameras.single(), domes.single_mut()) else {
        return;
    };
    dome.translation = camera.translation();
}

/// The dome follows the hours: horizon and zenith from the calendar's sky,
/// clouds gilded at dusk and swallowed at night.
fn follow_the_hours(
    time: Res<Time>,
    weather: Option<Res<crate::weather::Weather>>,
    sky: Option<Res<crate::calendar::Sky>>,
    mut materials: ResMut<Assets<SkyDomeMaterial>>,
    domes: Query<&MeshMaterial3d<SkyDomeMaterial>, With<SkyDome>>,
) {
    let Some(sky) = sky else {
        return;
    };

    for handle in &domes {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.params.horizon = linear(sky.horizon);

            // Overhead: rich blue by day, near-black by night.
            let day_zenith = palette::shade(&palette::SKY, 0.5);
            let night_zenith = Color::srgb(0.012, 0.016, 0.045);
            material.params.zenith = linear(crate::calendar::mix_colors(
                night_zenith,
                day_zenith,
                sky.daylight,
            ));

            // Clouds: white by day, gilded by the low sun, dark shapes at night.
            let day_cloud = palette::shade(&palette::BONE, 1.0);
            let night_cloud = Color::srgb(0.05, 0.055, 0.08);
            let base = crate::calendar::mix_colors(night_cloud, day_cloud, sky.daylight);
            material.params.cloud = linear(crate::calendar::mix_colors(
                base,
                sky.sun_color,
                (1.0 - sky.daylight).clamp(0.0, 1.0) * sky.daylight * 2.0,
            ));

            material.params.sun_dir = sky.sun_direction.extend(0.0);
            material.params.misc.x = time.elapsed_secs();
            material.params.misc.z = sky.daylight;
            // The weather owns the cloud deck.
            if let Some(weather) = &weather {
                material.params.misc.y = 0.18 + weather.intensity * 0.8;
            }
        }
    }
}
