//! The sea surface.
//!
//! A custom material rather than `StandardMaterial`: still water is a mirror, and the
//! sun's reflection in it collapses to a single blown-out point. What reads as water
//! is *motion* — a moving normal, a fresnel edge, a broad sheen — and none of that
//! comes out of a physically-correct material sitting flat.
//!
//! All of it happens in the fragment shader. The surface stays two triangles and the
//! waves come from perturbing the normal, which at this camera distance is
//! indistinguishable from displaced geometry and costs no tessellation, no vertex
//! work, and nothing to update as the sea follows the player.

use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::palette;

const SHADER_PATH: &str = "shaders/water.wgsl";

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
            .add_systems(Update, follow_the_sky);
    }
}

/// Uniform block handed to the water shader.
#[derive(Clone, ShaderType, Debug)]
pub struct WaterParams {
    /// Colour looking straight down. Alpha is the head-on opacity.
    pub shallow: Vec4,
    /// Colour at a glancing angle.
    pub deep: Vec4,
    /// What the surface reflects. Matching the horizon keeps the sea and the sky
    /// meeting cleanly at the far edge.
    pub sky: Vec4,
    /// Direction toward the sun.
    pub sun: Vec4,
    /// Spatial frequency of the wave field.
    pub wave_scale: f32,
    /// How fast the waves travel.
    pub wave_speed: f32,
    /// How far the normal is bent. Higher is choppier.
    pub wave_strength: f32,
    /// Strength of the specular sheen.
    pub specular: f32,
    /// How much water it takes to go from clear to fully opaque.
    pub depth_fade: f32,
    /// Width of the foam band at the shoreline, in world units of depth.
    pub foam_width: f32,
    pub _pad: Vec2,
}

impl Default for WaterParams {
    fn default() -> Self {
        // Both drawn from the bright end of the water ramp. A god camera spends most
        // of its time at an oblique angle, where the depth mix leans toward `deep` —
        // pick that too dark and the sea reads as navy paint at every useful zoom.
        let shallow = palette::shade(&palette::WATER, 1.0).to_linear();
        let deep = palette::shade(&palette::WATER, 0.62).to_linear();
        let sky = crate::render::horizon_color().to_linear();

        WaterParams {
            shallow: Vec4::new(shallow.red, shallow.green, shallow.blue, 0.86),
            deep: Vec4::new(deep.red, deep.green, deep.blue, 1.0),
            sky: Vec4::new(sky.red, sky.green, sky.blue, 1.0),
            sun: crate::SUN_DIRECTION.extend(0.0),
            // Wavelength is 2*pi/wave_scale in world units, so this is roughly a
            // fourteen-metre swell. The previous value put a single wave across a
            // hundred and fourteen metres, which meant an entire pond sat inside a
            // third of one wave and read as glass. Distance fade in the shader keeps
            // detail this fine from aliasing when the camera pulls back.
            wave_scale: 0.45,
            wave_speed: 0.9,
            wave_strength: 0.42,
            specular: 0.55,
            // A few metres of water is enough to hide the bottom; below that the
            // seabed shows through, which is what sells it as liquid.
            depth_fade: 7.0,
            foam_width: 1.4,
            _pad: Vec2::ZERO,
        }
    }
}

/// Material for the sea surface.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub params: WaterParams,
}

impl Default for WaterMaterial {
    fn default() -> Self {
        WaterMaterial {
            params: WaterParams::default(),
        }
    }
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Keeps the sea reflecting the sky it is actually under.
///
/// The water's sky colour and sun direction were baked at startup before there
/// was a day to change them. With the sun moving, a sea still reflecting noon
/// at midnight would give the whole night away.
fn follow_the_sky(
    sky: Option<Res<crate::calendar::Sky>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    surfaces: Query<&MeshMaterial3d<WaterMaterial>>,
) {
    let Some(sky) = sky else {
        return;
    };
    if !sky.is_changed() {
        return;
    }

    let horizon = sky.horizon.to_linear();
    // The water's body colour and sheen are authored for daylight; scale them
    // down with it, or the sea glows noon-blue through the night.
    let dim = 0.10 + 0.90 * sky.daylight;
    let day = WaterParams::default();
    for handle in &surfaces {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.params.sky = Vec4::new(horizon.red, horizon.green, horizon.blue, 1.0);
            material.params.sun = sky.sun_direction.extend(0.0);
            material.params.shallow = (day.shallow.truncate() * dim).extend(day.shallow.w);
            material.params.deep = (day.deep.truncate() * dim).extend(day.deep.w);
            material.params.specular = day.specular * (0.15 + 0.85 * sky.daylight);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shallow_water_is_lighter_than_deep() {
        // The shader mixes between these by viewing angle; the wrong way round makes
        // the sea darkest where it should be clearest.
        let p = WaterParams::default();
        let luminance = |c: Vec4| c.x * 0.2126 + c.y * 0.7152 + c.z * 0.0722;
        assert!(luminance(p.shallow) > luminance(p.deep));
    }

    #[test]
    fn water_is_translucent_head_on_and_opaque_at_a_glance() {
        let p = WaterParams::default();
        assert!(p.shallow.w < 1.0, "should see into the water from above");
        assert!(p.deep.w >= p.shallow.w);
    }

    #[test]
    fn wave_parameters_are_sane() {
        let p = WaterParams::default();
        assert!(p.wave_scale > 0.0 && p.wave_scale < 1.0);
        assert!(p.wave_speed > 0.0);
        assert!(p.wave_strength > 0.0);
        assert!(p.specular >= 0.0);
    }

    #[test]
    fn shallows_clear_before_the_foam_band_ends() {
        // Foam marks where water thins to nothing. If it were wider than the depth
        // over which water clears, the shoreline would be foam over opaque water
        // rather than foam over visible sand.
        let p = WaterParams::default();
        assert!(p.foam_width > 0.0);
        assert!(p.depth_fade > p.foam_width);
    }

    #[test]
    fn the_sun_direction_is_normalised() {
        let p = WaterParams::default();
        assert!((p.sun.truncate().length() - 1.0).abs() < 1e-4);
    }
}
