//! THE OPIFICIUM — the maker's own bench.
//!
//! A standalone companion to Divus Factus: buildings and animations are
//! authored here by hand and exported as JSON for the game to take in. The
//! two programs share no code — the game exports its palette as data
//! (`data/palette.json`) so Opificium paints with true colours, and the
//! Opificium exports blueprints and clips the game translates at its leisure.

use bevy::prelude::*;

mod builder;
mod camera;
mod gizmo;
mod look;
mod rail;
mod rig;
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
                title: "Divus Factus — Opificium".to_string(),
                resolution: bevy::window::WindowResolution::new(1440, 900),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<Bench>()
        .add_plugins((look::LookPlugin, camera::CameraPlugin, stage::StagePlugin))
        .add_plugins((rail::RailPlugin, builder::BuilderPlugin, gizmo::GizmoPlugin))
        .add_plugins(rig::RigPlugin)
        // Which bench the maker opens at. A maker working on the rig for an
        // hour should not walk across the builder to reach it every time.
        .insert_resource(match std::env::var("OPIFICIUM_BENCH").as_deref() {
            Ok("rig") => Bench::Rig,
            _ => Bench::Builder,
        })
        .run();
}
