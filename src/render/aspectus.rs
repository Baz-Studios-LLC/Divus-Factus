//! ASPECTUS: the passes this game owns.
//!
//! Everything else the game draws goes through Bevy's `core_3d` pipeline with
//! our materials hung on it - the ground and its veil, the planet's skin, the
//! grass, the clouds, the sky. Those are surfaces. What lives here are PASSES:
//! work done over the whole frame, in the render graph, on our terms.
//!
//! It starts with one, because a renderer is not a thing you declare - it is a
//! thing you take over a job at a time, and each job has to earn its place by
//! doing something the pipeline was doing worse.
//!
//! **The frost** is the first, and it earns it by deleting a camera. An open
//! book used to wake a second `Camera3d` that rendered the whole world again -
//! every chunk, tree and villager - into a 480x270 image with a Gaussian
//! depth-of-field, which a fullscreen UI quad then stretched back over the
//! screen. The image was cheap; the second scene submission was not. Blurring
//! the frame that has already been drawn costs sixteen texture reads and no
//! geometry whatsoever, and it blurs the real screen rather than a stretched
//! thumbnail of it.
//!
//! The next candidates, when they earn it: our own tonemapping and grading
//! (Bevy's is a good filmic curve and this is not a filmic game), an outline
//! pass, and a stylised shading model to replace `StandardMaterial` across the
//! world. None of those are here yet, and none of them should be here until
//! the frame actually wants them.

use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

/// The frosted glass behind an open book: a blur of the frame already drawn.
///
/// Carried by the camera it applies to, and only while it applies - the pass
/// runs whenever the component is present, so a book that is shut takes it off
/// rather than leaving a pass running at zero strength.
#[derive(Component, ExtractComponent, ShaderType, Debug, Clone, Copy)]
pub struct Frost {
    /// How much of the blur is mixed over the sharp frame, 0 to 1.
    pub strength: f32,
    /// The blur's reach, as a fraction of the screen's width.
    pub radius: f32,
    /// The window's aspect, so a round blur stays round.
    pub aspect: f32,
    pub _pad: f32,
}

impl Default for Frost {
    fn default() -> Self {
        Frost {
            strength: 0.0,
            radius: 0.0,
            aspect: 16.0 / 9.0,
            _pad: 0.0,
        }
    }
}

impl FullscreenMaterial for Frost {
    fn fragment_shader() -> ShaderRef {
        "shaders/frost.wgsl".into()
    }
}

pub struct AspectusPlugin;

impl Plugin for AspectusPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<Frost>::default());
    }
}
