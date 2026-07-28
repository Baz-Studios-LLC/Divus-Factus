//! Egregore — a god game about a deity made, sustained and defined by the people
//! who believe in it.

mod calendar;
mod camera;
mod creature;
mod debug;
mod grass;
mod hand;
mod loading;
mod matter;
mod meshbuild;
mod miracles;
mod navigation;
mod noise;
mod palette;
mod render;
mod rng;
mod save;
mod scatter;
mod sky;
mod terrain;
mod title;
mod ui;
mod villager;
mod water;
mod witness;

use bevy::camera::visibility::RenderLayers;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

/// Environment variable that puts the game into unattended capture mode: render
/// one frame to a file and quit.
///
/// Reading back a window's swapchain depends on the compositor actually drawing
/// the window, which an unfocused or occluded window on macOS will not do. Capture
/// mode therefore routes the final image through an offscreen target instead, which
/// is compositor-independent and gives the same picture the player would see.
pub const CAPTURE_VAR: &str = "EGREGORE_CAPTURE";

/// The path to capture to, if capture mode is active.
pub fn capture_path() -> Option<String> {
    std::env::var(CAPTURE_VAR).ok().filter(|p| !p.is_empty())
}

/// Direction toward the sun.
///
/// Shared so the water's specular highlight agrees with the light actually casting
/// the world's shadows; a sea lit from a different angle than the land reads as
/// wrong long before anyone can say why.
pub const SUN_DIRECTION: Vec3 = Vec3::new(0.520266, 0.780399, 0.346844);

/// Top-level flow. The world is generated before play begins rather than streamed in
/// under the player's feet, so the opening view is a finished landscape instead of
/// chunks popping into existence around them.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// The front door.
    #[default]
    Title,
    Loading,
    Playing,
}

/// Seed for every procedurally generated part of the world.
///
/// A single seed drives terrain, creature bodies and scatter, so a world can be
/// reproduced exactly from this one number.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WorldSeed(pub u32);

impl Default for WorldSeed {
    /// A different world every launch.
    ///
    /// Taken from the clock rather than a fixed constant, so the same landscape does
    /// not greet the player every time. Set `EGREGORE_SEED` to reproduce a specific
    /// world — worth having, since every procedural system here derives from this one
    /// number and a memorable world would otherwise be unrecoverable.
    fn default() -> Self {
        if let Ok(seed) = std::env::var("EGREGORE_SEED")
            && let Ok(parsed) = seed.parse::<u32>()
        {
            info!("world seed {parsed} (from EGREGORE_SEED)");
            return WorldSeed(parsed);
        }

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32).rotate_left(13))
            .unwrap_or(0x2024_1101);

        info!("world seed {seed} — set EGREGORE_SEED={seed} to return here");
        WorldSeed(seed)
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Egregore".into(),
                resolution: (1600u32, 900u32).into(),
                ..default()
            }),
            // The divine hand is the pointer. The operating system's arrow floating
            // over it breaks the one illusion the game is named after.
            primary_cursor_options: Some(bevy::window::CursorOptions {
                visible: false,
                ..default()
            }),
            ..default()
        }))
        .init_state::<GameState>()
        .init_resource::<WorldSeed>()
        .add_plugins((
            camera::CameraPlugin,
            terrain::TerrainPlugin,
            creature::CreaturePlugin,
            scatter::ScatterPlugin,
            villager::VillagerPlugin,
            hand::HandPlugin,
            render::RenderPlugin,
            debug::DebugPlugin,
            loading::LoadingPlugin,
            water::WaterPlugin,
            witness::WitnessPlugin,
            grass::GrassPlugin,
        ))
        .add_plugins((
            ui::UiPlugin,
            calendar::CalendarPlugin,
            sky::SkyPlugin,
            miracles::MiraclesPlugin,
            title::TitlePlugin,
            matter::MatterPlugin,
            save::SavePlugin,
        ))
        .add_systems(Startup, spawn_lighting)
        .run();
}

/// Sun, sky fill and ambient bounce.
///
/// Warm key light against a cool fill is what separates the HD-2D look from
/// flat-lit low-poly, and it costs one extra directional light.
fn spawn_lighting(mut commands: Commands) {
    // Ambient is deliberately low. A strong ambient fill lights every surface
    // roughly equally, which erases the shading that gives low-poly geometry its
    // form — the "flat and dull" failure mode. Contrast between a bright key light
    // and a dim, cool fill is what makes faceted terrain read as landscape.
    commands.insert_resource(GlobalAmbientLight {
        color: palette::shade(&palette::SKY, 0.7),
        brightness: 130.0,
        ..default()
    });

    // Both lights also cover render layer 1, where the Divine Hand lives so it can
    // draw above the interface. Without this the hand would be lit by nothing and
    // turn black the moment it moved to its own layer.
    let lit_layers = RenderLayers::from_layers(&[0, 1]);

    commands.spawn((
        Name::new("Sun"),
        calendar::SunLight,
        DirectionalLight {
            // Warm neutral rather than gold. A strongly orange key over green ground
            // mixes to lime, which is what made the landscape read as over-saturated
            // however far the grading was pulled back.
            color: palette::shade(&palette::BONE, 1.0),
            illuminance: 17_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        // Cascades sized to the world. The default configuration covers a few dozen
        // units, which on a map this size means shadows simply stop a short way from
        // the camera — one of the things that reads as "flat".
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            first_cascade_far_bound: 40.0,
            maximum_distance: 900.0,
            ..default()
        }
        .build(),
        Transform::from_translation(SUN_DIRECTION * 140.0).looking_at(Vec3::ZERO, Vec3::Y),
        lit_layers.clone(),
    ));

    // A dim, cool counter-light so shadowed faces read as blue rather than black.
    commands.spawn((
        Name::new("Sky Fill"),
        calendar::FillLight,
        DirectionalLight {
            color: palette::shade(&palette::SKY, 0.85),
            illuminance: 3_400.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-50.0, 40.0, -70.0).looking_at(Vec3::ZERO, Vec3::Y),
        lit_layers,
    ));
}
