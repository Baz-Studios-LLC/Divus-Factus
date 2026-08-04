//! Divus Factus — a god game about a deity made, sustained and defined by the people
//! who believe in it.

mod attention;
mod avatar;
mod calendar;
mod camera;
mod clouds;
mod creature;
mod debug;
mod doings;
mod fog;
mod founding;
mod globe;
mod grass;
mod hand;
mod keymap;
mod loading;
mod markers;
mod matter;
mod meshbuild;
mod miracles;
mod music;
mod navigation;
mod noise;
mod now;
mod palette;
mod render;
mod rng;
mod save;
mod scatter;
mod sigil;
mod sky;
mod speed;
mod survey;
mod telling;
mod terrain;
mod title;
mod trails;
mod ui;
mod villager;
mod water;
mod weather;
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
pub const CAPTURE_VAR: &str = "DIVUS_FACTUS_CAPTURE";

/// The path to capture to, if capture mode is active.
pub fn capture_path() -> Option<String> {
    std::env::var(CAPTURE_VAR).ok().filter(|p| !p.is_empty())
}

/// Where the sun starts, for the one frame before the sky is computed.
///
/// It used to BE the sun: a single bearing the light, the water's highlight and
/// the sky all agreed on, because a sea lit from a different angle than the
/// land reads as wrong long before anyone can say why. The sun is a body with a
/// place of its own now — see [`calendar::WorldClock::sun_position`] — and this
/// is only the value the lights and the water's uniform are born holding until
/// `drive_sky` first runs.
pub const SUN_DIRECTION: Vec3 = Vec3::new(0.520266, 0.780399, 0.346844);

/// Top-level flow. The world is generated before play begins rather than streamed in
/// under the player's feet, so the opening view is a finished landscape instead of
/// chunks popping into existence around them.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// The studio mark, fading in and out over black while the world
    /// generates behind it.
    #[default]
    Splash,
    /// The front door.
    Title,
    Loading,
    /// The world, empty, and a flag in the god's hand. Nothing has been
    /// founded yet — the player is looking for ground to found it on.
    /// Every instrument is still locked here, because they are all gated
    /// on `Playing`: there is no chronicle to read and no village to
    /// survey until somebody plants the flag.
    Choosing,
    Playing,
}

/// Whether the world is the player's to move around in — choosing ground
/// or playing on it, but not drifting behind the title.
///
/// The camera and the god's hand answer to this rather than to `Playing`
/// alone: during the choosing the game IS theirs, and a player who
/// cannot turn the camera cannot pick anywhere to found.
pub fn world_is_afoot(state: Res<State<GameState>>) -> bool {
    matches!(state.get(), GameState::Choosing | GameState::Playing)
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
    /// not greet the player every time. Set `DIVUS_FACTUS_SEED` to reproduce a specific
    /// world — worth having, since every procedural system here derives from this one
    /// number and a memorable world would otherwise be unrecoverable.
    fn default() -> Self {
        if let Ok(seed) = std::env::var("DIVUS_FACTUS_SEED")
            && let Ok(parsed) = seed.parse::<u32>()
        {
            info!("world seed {parsed} (from DIVUS_FACTUS_SEED)");
            return WorldSeed(parsed);
        }

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32).rotate_left(13))
            .unwrap_or(0x2024_1101);

        info!("world seed {seed} — set DIVUS_FACTUS_SEED={seed} to return here");
        WorldSeed(seed)
    }
}

fn main() {
    // The voice bench: the corpus with no world around it, for judging
    // the writing without hunting two villagers across a valley first.
    if std::env::args().any(|arg| arg == "--voice") {
        telling::bench::run();
        return;
    }
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Divus Factus".into(),
                resolution: (1600u32, 900u32).into(),
                // Measurement tooling: vsync pins every healthy frame to the
                // display's beat, which QUANTIZES costs — a frame one
                // millisecond over budget reads as double. With the dial set,
                // frames run free and the telemetry reads true cost.
                present_mode: if std::env::var("DIVUS_FACTUS_NOVSYNC").is_ok() {
                    bevy::window::PresentMode::AutoNoVsync
                } else {
                    bevy::window::PresentMode::default()
                },
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
            // Nested: a plugin tuple holds sixteen, and the sky's three take
            // it over. The grouping is the honest one anyway — the clock, the
            // weather deck it lights and the sky behind them.
            (calendar::CalendarPlugin, clouds::CloudPlugin, sky::SkyPlugin),
            miracles::MiraclesPlugin,
            title::TitlePlugin,
            matter::MatterPlugin,
            save::SavePlugin,
            weather::WeatherPlugin,
            trails::TrailsPlugin,
            speed::SpeedPlugin,
            survey::SurveyPlugin,
            markers::MarkersPlugin,
            // Before the teller: they decide what the teller is asked for,
            // and what the asking is allowed to say.
            attention::AttentionPlugin,
            now::NowPlugin,
            telling::TellingPlugin,
        ))
        .add_plugins((
            keymap::KeymapPlugin,
            music::MusicPlugin,
            doings::DoingsPlugin,
            avatar::AvatarPlugin,
            founding::FoundingPlugin,
            fog::FogPlugin,
            globe::GlobePlugin,
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

    // Every light covers render layer 1, where the Divine Hand lives so it can
    // draw above the interface. Without this the hand would be lit by nothing and
    // turn black the moment it moved to its own layer.
    //
    // And layer 2, the planet. It used to have a sun of its own, nailed to a
    // fixed bearing and never told the hour, so the ball hung in a permanent
    // mid-morning while the ground beneath the god ran through its day. There
    // is one sun in this world and it lights everything the world is made of.
    let lit_layers = RenderLayers::from_layers(&[0, 1, globe::GLOBE_LAYER]);

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
        lit_layers.clone(),
    ));

    // The moon, which is a real body on the far side of the planet from the
    // sun. It carries the night now: the fill used to do that from a fixed
    // bearing, so after dusk nothing in the world had a direction. No shadows —
    // moonlight this weak casts nothing a player would read as one, and a
    // second shadow map is not free.
    commands.spawn((
        Name::new("Moon"),
        calendar::MoonLight,
        DirectionalLight {
            color: palette::shade(&palette::SKY, 0.95),
            illuminance: 430.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, -140.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
        lit_layers,
    ));
}
