//! THE ATELIER — the maker's own bench.
//!
//! A standalone companion to Divus Factus: buildings and animations are
//! authored here by hand and exported as JSON for the game to take in. The
//! two programs share no code — the game exports its palette as data
//! (`data/palette.json`) so the Atelier paints with true colours, and the
//! Atelier exports blueprints and clips the game translates at its leisure.

use bevy::prelude::*;

mod builder;
mod camera;
mod gizmo;
mod look;
mod rail;
mod stage;

/// Which bench the maker stands at.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bench {
    /// Buildings: boxes, ramps and widgets on the ground grid.
    #[default]
    Builder,
    /// Animation: the canonical body on its pedestal, and the timeline.
    Rig,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Divus Factus — The Atelier".to_string(),
                resolution: bevy::window::WindowResolution::new(1440, 900),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<Bench>()
        .add_plugins((look::LookPlugin, camera::CameraPlugin, stage::StagePlugin))
        .add_plugins((rail::RailPlugin, builder::BuilderPlugin, gizmo::GizmoPlugin))
        .run();
}
