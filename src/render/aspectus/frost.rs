//! The frosted glass behind an open book: a blur of the frame already drawn.
//!
//! Aspectus's first pass, and it earned its place by deleting a camera. See
//! `aspectus/mod.rs` for what it replaced.

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

pub struct FrostPass;

impl Plugin for FrostPass {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<Frost>::default());
    }
}
