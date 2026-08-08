//! The pre-game: the studio mark and the title screen.
//!
//! The splash fades the studio's mark in and out over black — skippable with
//! any key — while the world generates behind it. The title that follows is
//! not a screen so much as a vantage: the living, blurred world drifts under
//! the lettering, smoke curls behind the logotype, and Begin sends the camera
//! diving from that god's-height view down to the village itself. No loading
//! screen, no cut — the loading screen survives only as a fallback for
//! machines still building the world when Begin is pressed.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::GameState;
use crate::debug::layers::Layer;
use crate::ui::{self, theme};

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Splash), spawn_splash)
            .add_systems(OnExit(GameState::Splash), despawn_splash)
            .add_systems(OnEnter(GameState::Title), spawn_title)
            .add_systems(OnExit(GameState::Title), begin_farewell)
            .add_systems(
                Update,
                (
                    play_splash.run_if(in_state(GameState::Splash)),
                    drift_title_camera
                        .run_if(in_state(GameState::Splash).or_else(in_state(GameState::Title))),
                    drift_smoke,
                    play_farewell,
                    play_welcome,
                    // Ungated by state: it spans the change from Playing to
                    // Title and has to keep running across the boundary.
                    draw_the_curtain,
                    sync_hud.run_if(state_changed::<GameState>),
                    handle_choice.run_if(in_state(GameState::Title)),
                    auto_begin.run_if(in_state(GameState::Title)),
                    auto_title.run_if(in_state(GameState::Playing)),
                    handle_settings,
                    handle_view_switches,
                    style_menu_buttons,
                    // Before the codex's own Escape handling, so the frame
                    // that shuts the book sees it still open and yields -
                    // one press must never both close the codex and raise
                    // the pause menu.
                    toggle_pause_menu
                        .run_if(in_state(GameState::Playing))
                        .before(crate::debug::handle_tuning_input),
                    handle_pause_menu,
                ),
            );
    }
}

/// True when an unattended capture wants to photograph the title screen
/// itself instead of skipping past it into the world.
pub fn title_capture() -> bool {
    std::env::var("DIVUS_FACTUS_TITLE").is_ok()
}

#[derive(Component)]
struct TitleScreen;

/// The logotype image, so the farewell can fade it.
#[derive(Component)]
struct TitleArt;

/// The title's menu buttons — hidden and disarmed the instant the descent
/// begins, so a half-faded button can never take a click.
#[derive(Component)]
struct TitleMenu;

#[derive(Component)]
struct BeginButton;

#[derive(Component)]
struct SettingsButton;

#[derive(Component)]
struct QuitButton;

/// The door to the maker's bench.
#[derive(Component)]
struct AtelierButton;

/// Where the Atelier stands, if it stands anywhere.
///
/// Beside the game first, which is where a packaged build puts it: one bundle
/// holding both, so the bench a player opens is always the one that matches the
/// game it feeds. That matters more here than tidiness — the two share a file
/// contract, and a bench a release behind the game writes buildings the game
/// reads differently.
///
/// Then the source tree, because the bench is its own crate with its own target
/// directory and does NOT sit beside the game while either is being worked on.
///
/// `None` means it is not installed, and the button does not appear at all. A
/// door that opens onto nothing is worse than no door.
fn atelier_beside_us() -> Option<std::path::PathBuf> {
    // What it is called where it SHIPS, and what cargo calls it in a tree. The
    // shipped name changed because a launcher that runs the first `.exe` in the
    // folder ran the bench when a player pressed PLAY - so the bench now sorts
    // after the game and cannot be mistaken for it either.
    let shipped = if cfg!(windows) {
        "TheAtelier.exe"
    } else {
        "TheAtelier"
    };
    let built = if cfg!(windows) {
        "divus-factus-atelier.exe"
    } else {
        "divus-factus-atelier"
    };
    let us = std::env::current_exe().ok()?;
    let here = us.parent()?;
    for named in [shipped, built] {
        let beside = here.join(named);
        if beside.is_file() {
            return Some(beside);
        }
    }
    // A source tree: the game runs from `target/release`, the bench builds into
    // `atelier/target/release`.
    let workspace = here.parent()?.parent()?;
    let in_tree = workspace.join("atelier/target/release").join(built);
    in_tree.is_file().then_some(in_tree)
}

/// Opens the saves window as a load menu.
#[derive(Component)]
struct LoadGameButton;

/// The in-game pause menu and its buttons.
#[derive(Component)]
pub struct PauseMenu;

#[derive(Component)]
struct ResumeButton;

#[derive(Component)]
struct PauseSettingsButton;

#[derive(Component)]
struct ExitButton;

#[derive(Component)]
struct PauseSavesButton;

/// The pause menu's way back to the front door. The world is not torn down —
/// it keeps living, exactly as it does behind the title — the camera simply
/// climbs back to the god's vantage and the veil returns.
#[derive(Component)]
struct TitleReturnButton;

/// The settings overlay, above the title.
#[derive(Component)]
struct SettingsScreen;

#[derive(Component)]
struct BackButton;

/// A toggle switch in the settings, and which view it governs.
///
/// Switches rather than keybindings, on Brett's call and it is the right one: a
/// hotkey is for something a player reaches for mid-thought, and most of what
/// this game can turn on and off is not that. The keyboard was filling up with
/// letters nobody would remember.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum ViewSwitch {
    /// The weather deck. Off, and the god can see the ground.
    Clouds,
    /// The fog of war over ground no village has walked.
    Veil,
    /// One of the world's own layers, held in [`ViewLayers`].
    ///
    /// Two switches above keep their state elsewhere because they had owners
    /// before the layers existed, and giving either of them a second home would
    /// mean a switch that reads one truth while the world reads another.
    Layer(Layer),
}

impl ViewSwitch {
    const ALL: [ViewSwitch; 8] = [
        ViewSwitch::Clouds,
        ViewSwitch::Veil,
        ViewSwitch::Layer(Layer::Scenery),
        ViewSwitch::Layer(Layer::Patches),
        ViewSwitch::Layer(Layer::Water),
        ViewSwitch::Layer(Layer::Buildings),
        ViewSwitch::Layer(Layer::Folk),
        ViewSwitch::Layer(Layer::Shadows),
    ];

    fn label(self) -> &'static str {
        match self {
            ViewSwitch::Clouds => "clouds",
            ViewSwitch::Veil => "the veil",
            ViewSwitch::Layer(layer) => layer.label(),
        }
    }

    /// What it says when it is on — the state a player wants to read, not the
    /// name of a flag.
    fn note(self) -> &'static str {
        match self {
            ViewSwitch::Clouds => "weather over the world",
            ViewSwitch::Veil => "unwalked ground kept dark",
            ViewSwitch::Layer(layer) => layer.note(),
        }
    }
}

/// A hand-colour swatch, holding its place in [`crate::hand::HAND_STYLES`].
#[derive(Component)]
struct HandSwatch(usize);

// ---------------------------------------------------------------------------
// The splash: the studio's mark over black, while the world builds behind it.
// ---------------------------------------------------------------------------

#[derive(Component)]
struct SplashScreen;

#[derive(Component)]
struct SplashArt;

/// The studio's line at the foot of the mark, fading with it.
#[derive(Component)]
struct SplashMark;

/// Seconds each fade takes, and seconds the mark holds at full strength.
const SPLASH_FADE: f32 = 1.3;
const SPLASH_HOLD: f32 = 1.8;

/// The most one frame may spend of the mark's life.
///
/// The world is generated on the first frames BEHIND the splash, and on this
/// machine that means single frames of four and a half seconds — measured, six
/// frames in the first five seconds of a run. The mark's whole life is four and
/// four tenths. So it was faded in, held and faded out inside one frame that
/// had not yet drawn anything, and the studio's mark never reached the screen
/// at all. Brett: "the newcity entertainment splash screen disappeared at some
/// point".
///
/// Nothing about the splash changed. It went when world generation got heavy
/// enough to swallow it whole, which is why it read as vanishing on its own.
///
/// So a frame buys at most a tenth of a second of fade however long it really
/// took, because time nobody saw is not time the mark was on screen. Capped
/// rather than ignored: a machine slow enough that EVERY frame stalls would
/// otherwise hold the splash for ever, and a bounded step means the mark is
/// always worth about forty-four drawn frames whatever the machine is doing.
const SLOWEST_STEP: f32 = 0.1;

/// How much of the mark's life one frame carries, given how long it really
/// took. See [`SLOWEST_STEP`].
fn splash_step(delta: f32) -> f32 {
    delta.min(SLOWEST_STEP)
}

/// The mark's whole life: fade in, hold, fade out.
fn splash_life() -> f32 {
    SPLASH_FADE * 2.0 + SPLASH_HOLD
}

/// How far through the mark's life we are, and what it really cost.
#[derive(Resource, Default)]
struct SplashClock {
    /// Seconds of DRAWN time, which is what the fade runs on.
    spent: f32,
    /// Real seconds and frames since it appeared, for the line it logs on the
    /// way out. Had this been there, the bug above would have been a glance
    /// rather than an afternoon.
    real: f32,
    frames: u32,
}

fn spawn_splash(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut next: ResMut<NextState<GameState>>,
) {
    // Unattended captures skip the pre-game entirely — except title
    // portraits, which stop at the title itself.
    if crate::capture_path().is_some() {
        next.set(if title_capture() {
            GameState::Title
        } else {
            GameState::Loading
        });
        return;
    }

    commands.insert_resource(SplashClock::default());
    let screen = commands
        .spawn((
            Name::new("Splash Screen"),
            SplashScreen,
            ui::Panel,
            Interaction::default(),
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            // True black, not panel charcoal: a studio mark opens the way a
            // theatre goes dark, and the title's own colour reads warmer for
            // following it.
            BackgroundColor(Color::BLACK),
            GlobalZIndex(320),
        ))
        .id();

    commands.spawn((
        SplashArt,
        ImageNode {
            image: assets.load("NewCityEntertainment.png"),
            color: Color::srgba(1.0, 1.0, 1.0, 0.0),
            ..default()
        },
        Node {
            width: px(620),
            ..default()
        },
        ChildOf(screen),
    ));

    // The studio's line, at the foot of the dark, rising and leaving
    // with the mark above it.
    // Built by hand rather than through `ui::dim`: this line owns its
    // own colour, and a bundle may not carry two.
    commands.spawn((
        SplashMark,
        Text::new(format!(
            "\u{00a9} {STUDIO_YEAR} Baz Studios, LLC. All rights reserved."
        )),
        ui::SerifFace,
        TextFont {
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(34),
            ..default()
        },
        ChildOf(screen),
    ));
}

/// The year on the studio's line.
const STUDIO_YEAR: u32 = 2026;

/// Runs the fade-in, hold, fade-out — and lets any key or click skip ahead.
fn play_splash(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    clock: Option<ResMut<SplashClock>>,
    mut arts: Query<&mut ImageNode, With<SplashArt>>,
    mut marks: Query<&mut TextColor, With<SplashMark>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(mut clock) = clock else {
        return;
    };
    // Only what a frame could actually SHOW spends the mark. See
    // `SLOWEST_STEP` - the world is being generated behind this, and a frame
    // that took four seconds put nothing on the screen for any of them.
    clock.real += time.delta_secs();
    clock.frames += 1;
    clock.spent += splash_step(time.delta_secs());

    let fade_out_at = SPLASH_FADE + SPLASH_HOLD;
    let alpha = if clock.spent < SPLASH_FADE {
        clock.spent / SPLASH_FADE
    } else if clock.spent < fade_out_at {
        1.0
    } else {
        1.0 - (clock.spent - fade_out_at) / SPLASH_FADE
    }
    .clamp(0.0, 1.0);

    // (see `splash_step` for why the step is capped)
    // A key press skips ahead to the fade-out rather than cutting: the mark
    // still leaves the way it always leaves, just now. Jumping to the point
    // on the out-fade with the current alpha keeps the brightness continuous.
    let skipped =
        keys.get_just_pressed().next().is_some() || buttons.get_just_pressed().next().is_some();
    if skipped && clock.spent < fade_out_at {
        clock.spent = fade_out_at + (1.0 - alpha) * SPLASH_FADE;
    }

    for mut art in &mut arts {
        art.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
    // The studio's line keeps to a whisper of the mark's own light.
    for mut mark in &mut marks {
        mark.0 = Color::srgba(0.82, 0.8, 0.76, alpha * 0.62);
    }

    if clock.spent >= fade_out_at + SPLASH_FADE {
        // What the mark actually got, in the two units that matter. A frame
        // count in single figures here means it was swallowed by the world
        // being built behind it, which is exactly how it went missing before.
        info!(
            "the studio's mark showed for {:.1}s over {} frames",
            clock.real, clock.frames
        );
        next.set(GameState::Title);
    }
}

fn despawn_splash(mut commands: Commands, screens: Query<Entity, With<SplashScreen>>) {
    for screen in &screens {
        commands.entity(screen).despawn();
    }
    commands.remove_resource::<SplashClock>();
}

/// A menu button dressed in the kit's language — the same square-cornered,
/// charcoal-and-gold face the People window's tabs wear. Solid, not glassy:
/// these sit over the living world now, and a translucent button reads as
/// unfinished rather than elegant.
fn menu_button(commands: &mut Commands, parent: Entity, label: &str) -> Entity {
    button_of_size(commands, parent, label, 240.0, 15.0, 11.0)
}

/// A menu button at a chosen size.
///
/// The title's are half again the pause menu's: they are the front door, read
/// from across a room, and against a whole planet the small ones looked like a
/// debug panel. The pause menu keeps the size it had — it sits over a world the
/// player is already in and has no business shouting.
fn button_of_size(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    width: f32,
    font: f32,
    padding: f32,
) -> Entity {
    let button = commands
        .spawn((
            ui::UiButton,
            // KeepFace: the kit's generic hover restyle would repaint the
            // resting fill back to translucent glass; these buttons keep
            // their own dress, tended by `style_menu_buttons`.
            ui::KeepFace,
            MenuFace,
            Node {
                width: px(width),
                padding: UiRect::axes(px(22), px(padding)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(theme::title_bg()),
            BorderColor::all(theme::panel_border().with_alpha(0.4)),
            Interaction::default(),
            ChildOf(parent),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_uppercase()),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(font),
                ..default()
            },
            TextColor(theme::accent()),
        ))
        .id();
    commands.entity(text).insert(ChildOf(button));
    button
}

/// Marks buttons wearing the menu dress, so their hover styling stays theirs.
#[derive(Component)]
struct MenuFace;

/// Hover and press on the menu dress: the border takes the gold, the way the
/// People window's active tab does; a press warms the fill.
fn style_menu_buttons(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<MenuFace>, Changed<Interaction>),
    >,
) {
    for (interaction, mut fill, mut border) in &mut buttons {
        match interaction {
            Interaction::None => {
                fill.0 = theme::title_bg();
                *border = BorderColor::all(theme::panel_border().with_alpha(0.4));
            }
            Interaction::Hovered => {
                fill.0 = theme::title_bg();
                *border = BorderColor::all(theme::accent().with_alpha(0.85));
            }
            Interaction::Pressed => {
                fill.0 = theme::accent().with_alpha(0.25);
                *border = BorderColor::all(theme::accent());
            }
        }
    }
}

/// How dark the title lays its veil over the living world behind it.
const SCRIM_ALPHA: f32 = 0.62;

fn spawn_title(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let screen = commands
        .spawn((
            Name::new("Title Screen"),
            TitleScreen,
            // Marked as interface, so the hand spends the whole title screen in
            // its pointing pose — fingertip on the cursor, hovering the buttons
            // and tapping them, the same finger that will answer prayers.
            ui::Panel,
            Interaction::default(),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                // The logotype rides at the top and the menu sits on the right;
                // the middle of the screen belongs to the planet, which is the
                // one thing a title for this game has to say.
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                row_gap: px(14),
                padding: UiRect::top(px(46)),
                ..default()
            },
            // No scrim. It was a veil over a world drifting past underneath -
            // the right idea when the title looked down at a valley from a
            // hundred and seventy-five units and the menu needed to be legible
            // over grass. The title now looks at the whole planet against
            // space, and there is nothing to dim: the background IS black, and
            // a scrim over it only takes the light off the world.
            BackgroundColor(Color::NONE),
            // Above the world, the HUD and the loading screen alike.
            GlobalZIndex(300),
        ))
        .id();

    // The title comes up out of nothing rather than appearing whole — see
    // [`TitleWelcome`]. Inserted here so it covers the splash's first arrival
    // as well as a return from a game.
    commands.insert_resource(TitleWelcome { t: 0.0 });

    // The smoke, spawned before the logotype so it drifts behind the
    // lettering rather than across it.
    spawn_smoke(&mut commands, &mut images, screen);

    // The logotype — the game's one blessed piece of non-procedural art,
    // cropped to its lettering so the layout box is the art, not the
    // transparent padding it once shipped inside.
    let title = commands
        .spawn((
            TitleArt,
            ImageNode::new(assets.load("DivusFactusLogo.png")),
            Node {
                width: px(860),
                margin: UiRect::bottom(px(40.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(title).insert(ChildOf(screen));

    // The menu, off to the right and level with the middle of the screen, so
    // the planet has the middle of the frame to itself.
    let menu = commands
        .spawn((
            Name::new("Title Menu"),
            Node {
                // Anchored down the full height and centred within it, which is
                // what "vertically centred" has to mean in a flex layout: a
                // percentage `top` puts the column's TOP at the middle of the
                // screen and hangs the rest below it.
                // Centred between the foot of the logotype and the foot of the
                // screen, rather than in the screen as a whole: the top third
                // belongs to the lettering, and a menu centred against the whole
                // frame sits up inside it.
                position_type: PositionType::Absolute,
                right: px(110),
                top: percent(MENU_TOP * 100.0),
                bottom: px(0),
                width: px(340),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Stretch,
                row_gap: px(18),
                ..default()
            },
            ChildOf(screen),
        ))
        .id();

    let begin = button_of_size(&mut commands, menu, "Begin", 340.0, 21.0, 16.0);
    commands.entity(begin).insert((BeginButton, TitleMenu));
    let load = button_of_size(&mut commands, menu, "Load Game", 340.0, 21.0, 16.0);
    commands.entity(load).insert((LoadGameButton, TitleMenu));
    let settings = button_of_size(&mut commands, menu, "Settings", 340.0, 21.0, 16.0);
    commands
        .entity(settings)
        .insert((SettingsButton, TitleMenu));
    // Only when there is a bench to open. A button that does nothing teaches a
    // player that buttons here might do nothing.
    if atelier_beside_us().is_some() {
        let bench = button_of_size(&mut commands, menu, "Atelier", 340.0, 21.0, 16.0);
        commands.entity(bench).insert((AtelierButton, TitleMenu));
    }
    let quit = button_of_size(&mut commands, menu, "Quit", 340.0, 21.0, 16.0);
    commands.entity(quit).insert((QuitButton, TitleMenu));

    // The build, small and out of the composition's way — the first thing a
    // bug report needs and the last thing a title should shout.
    commands.spawn((
        Text::new(concat!("v", env!("CARGO_PKG_VERSION"))),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(ui::theme::text_dim().with_alpha(0.55)),
        Node {
            position_type: PositionType::Absolute,
            right: px(14),
            bottom: px(10),
            ..default()
        },
        ChildOf(screen),
    ));
}

// ---------------------------------------------------------------------------
// The smoke: hand-rolled wisps drifting behind the lettering.
// ---------------------------------------------------------------------------

/// One drifting wisp. Position is derived every frame from real time, so the
/// smoke never stops moving whatever the virtual clock is doing.
#[derive(Component)]
struct SmokePuff {
    /// Starting point along the drift lane, as a fraction of the lane.
    lane: f32,
    /// Resting height, as a fraction of the screen.
    home_y: f32,
    /// Lane traversal speed, in lane-fractions per second. Slow: a full
    /// crossing takes minutes.
    drift: f32,
    /// Phase offset, so no two puffs breathe in step.
    phase: f32,
    /// Rendered size in logical pixels.
    size: f32,
    /// Resting opacity, before breathing and the farewell thin it.
    alpha: f32,
}

/// The pale slate the wisps are tinted.
const SMOKE_TINT: Color = Color::srgb(0.72, 0.76, 0.86);

/// Where on screen the blow-away radiates from: the heart of the lettering.
/// How far past the lettering the smoke reaches, and how much of its height the
/// band occupies.
///
/// Fractions of the LOGOTYPE rather than of the window, which is the whole point
/// of them: the band used to be written in screen fractions - 14% to 86% across,
/// 21% to 38% down - and screen fractions were right until the title was rebuilt
/// around the planet. The lettering moved to the top of the frame and the smoke
/// stayed where it had been, sitting below the words it was meant to be behind.
/// Brett: "it was before we redid the title screen and we never fixed it."
///
/// Measured from the art now, so the next layout carries it along.
const SMOKE_SPREAD: f32 = 1.28;
const SMOKE_BAND: f32 = 0.72;

/// A soft, wispy blob: radial falloff carved by value noise. One texture
/// serves every puff — flips, scale and overlap hide the reuse.
fn smoke_texture(images: &mut Assets<Image>) -> Handle<Image> {
    const SIZE: u32 = 160;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / (SIZE - 1) as f32 * 2.0 - 1.0;
            let v = y as f32 / (SIZE - 1) as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let body = (1.0 - r).clamp(0.0, 1.0).powf(1.8);
            let wisp = crate::noise::fbm_2d(u * 2.6 + 5.3, v * 2.6 + 9.1, 0x50F7, 4, 2.1, 0.55);
            let a = (body * (0.35 + 0.65 * wisp) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
            data.extend_from_slice(&[255, 255, 255, a]);
        }
    }
    images.add(Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

fn spawn_smoke(commands: &mut Commands, images: &mut Assets<Image>, screen: Entity) {
    let texture = smoke_texture(images);
    for i in 0..12usize {
        let f = i as f32;
        // Golden-ratio spacing: even coverage of the lane with no visible
        // rhythm, and no random draws to burn.
        let puff = SmokePuff {
            lane: (f * 0.618034).fract(),
            // Where in the band this puff rides, 0 at the top of the lettering
            // and 1 at the foot of it.
            home_y: (f * 0.377).fract(),
            drift: 0.0045 + 0.006 * (f * 0.529).fract(),
            phase: f * 1.947,
            size: 260.0 + 360.0 * (f * 0.732).fract(),
            alpha: 0.09 + 0.08 * (f * 0.851).fract(),
        };
        commands.spawn((
            puff,
            ImageNode {
                image: texture.clone(),
                color: SMOKE_TINT.with_alpha(0.0),
                flip_x: i & 1 == 1,
                flip_y: i & 2 == 2,
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                left: px(-1000),
                top: px(-1000),
                ..default()
            },
            ChildOf(screen),
        ));
    }
}

/// Drifts the wisps — and, once the farewell begins, blows them away from the
/// lettering as the descent starts.
fn drift_smoke(
    time: Res<Time<Real>>,
    farewell: Option<Res<TitleFarewell>>,
    welcome: Option<Res<TitleWelcome>>,
    // The title's own root, which is the FRAME for everything here — and not
    // the window, which is a different space in capture mode. The window
    // reports its logical 1512 while an unattended capture lays the interface
    // out across the render target's 3024, so a position divided by the window
    // came out at twice the fraction it should be and the smoke sat off the
    // right-hand edge. The same mismatch cost hours on the god's hand once.
    // Measure the frame with the same ruler as the thing inside it.
    frames: Query<&ComputedNode, With<TitleScreen>>,
    lettering: Query<(&ComputedNode, &UiGlobalTransform), With<TitleArt>>,
    mut puffs: Query<(&SmokePuff, &mut Node, &mut ImageNode)>,
) {
    if puffs.is_empty() {
        return;
    }
    let Some(frame) = frames
        .iter()
        .next()
        .map(|computed| computed.size() * computed.inverse_scale_factor())
        .filter(|frame| frame.x > 1.0 && frame.y > 1.0)
    else {
        return;
    };

    // The lettering's own rectangle, in the same fractions the drift is written
    // in. Skipped entirely until the layout has measured it — on the first frame
    // or two a node's computed size is nought, and a band of nothing would put
    // every puff in the top-left corner in full view of the player.
    let Some((heart, reach)) = lettering.iter().next().and_then(|(computed, at)| {
        let scale = computed.inverse_scale_factor();
        let half = computed.size() * scale * 0.5;
        (half.x > 1.0 && half.y > 1.0).then(|| {
            (
                Vec2::new(at.translation.x, at.translation.y) * scale / frame,
                half / frame,
            )
        })
    }) else {
        return;
    };
    let t = time.elapsed_secs();
    let blow = farewell.map_or(0.0, |f| f.t);
    // Comes up with the lettering, and owns the whole of its own colour while
    // doing it — one writer per value.
    let risen = welcome.map_or(1.0, |welcome| welcome.risen());

    for (puff, mut node, mut image) in &mut puffs {
        // The crawl along the lane, plus a slow private wander around it — the
        // lane being the lettering's own width, spilling a little past each end.
        let along = (puff.lane + t * puff.drift).fract();
        let span = reach.x * 2.0 * SMOKE_SPREAD;
        let band = reach.y * 2.0 * SMOKE_BAND;
        let mut x = heart.x + (along - 0.5) * span + 0.03 * span * (t * 0.043 + puff.phase).sin();
        let mut y = heart.y
            + (puff.home_y - 0.5) * band
            + 0.10 * band * (t * 0.053 + puff.phase * 1.7).sin()
            + 0.06 * band * (t * 0.031 + puff.phase * 2.3).cos();

        // Born thin at one end of the lane, dying thin at the other, so the
        // wrap-around never pops.
        let mut alpha = puff.alpha
            * (along * std::f32::consts::PI).sin().clamp(0.0, 1.0)
            * (0.72 + 0.28 * (t * 0.11 + puff.phase * 3.1).sin());

        if blow > 0.0 {
            // Begin blows the smoke off the title: every wisp streams outward
            // from the lettering, with the wind's own slant, thinning to
            // nothing as the descent gets under way.
            let s = blow * blow;
            let away = (Vec2::new(x, y) - heart) * 2.6 + Vec2::new(0.35, -0.18);
            x += away.x * s * 0.55;
            y += away.y * s * 0.55;
            alpha *= (1.0 - blow).max(0.0);
        }

        node.left = px(x * frame.x - puff.size * 0.5);
        node.top = px(y * frame.y - puff.size * 0.5);
        node.width = px(puff.size);
        node.height = px(puff.size);
        image.color = SMOKE_TINT.with_alpha(alpha * risen);
    }
}

/// Escape raises the pause menu - the world holds its breath while it is
/// up, the same stillness the title uses. When a miracle is armed, Escape
/// belongs to disarming it instead; when the codex is open, Escape
/// belongs to closing the book.
fn toggle_pause_menu(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    armed: Res<crate::miracles::SelectedMiracle>,
    codex: Query<&Visibility, With<crate::debug::VillagePanel>>,
    mut time: ResMut<Time<Virtual>>,
    menus: Query<(Entity, &Visibility), With<PauseMenu>>,
) {
    if !keys.just_pressed(KeyCode::Escape)
        || armed.0.is_some()
        || codex.iter().any(|v| *v != Visibility::Hidden)
    {
        return;
    }
    match menus.single() {
        Ok((menu, visibility)) => {
            if *visibility == Visibility::Hidden {
                commands.entity(menu).insert(Visibility::Visible);
                time.pause();
            } else {
                commands.entity(menu).insert(Visibility::Hidden);
                time.unpause();
            }
        }
        Err(_) => {
            // First press: build it, open.
            let window = crate::ui::big_window(&mut commands, "PAUSED", 240.0);
            commands
                .entity(window.root)
                .insert((Name::new("Pause Menu"), PauseMenu));
            let resume = menu_button(&mut commands, window.body, "Resume");
            commands.entity(resume).insert(ResumeButton);
            let settings = menu_button(&mut commands, window.body, "Settings");
            commands.entity(settings).insert(PauseSettingsButton);
            let saves = menu_button(&mut commands, window.body, "Saves");
            commands.entity(saves).insert(PauseSavesButton);
            let title = menu_button(&mut commands, window.body, "Title");
            commands.entity(title).insert(TitleReturnButton);
            let exit = menu_button(&mut commands, window.body, "Exit Game");
            commands.entity(exit).insert(ExitButton);
            time.pause();
        }
    }
}

/// The pause menu's doors.
#[allow(clippy::type_complexity)]
fn handle_pause_menu(
    mut commands: Commands,
    resume: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
    settings: Query<&Interaction, (Changed<Interaction>, With<PauseSettingsButton>)>,
    saves: Query<&Interaction, (Changed<Interaction>, With<PauseSavesButton>)>,
    mut saves_panels: Query<&mut Visibility, With<crate::save::SavesPanel>>,
    mut codex: ResMut<crate::debug::Codex>,
    mut codex_panels: Query<
        &mut Visibility,
        (
            With<crate::debug::VillagePanel>,
            Without<crate::save::SavesPanel>,
        ),
    >,
    exit: Query<&Interaction, (Changed<Interaction>, With<ExitButton>)>,
    to_title: Query<&Interaction, (Changed<Interaction>, With<TitleReturnButton>)>,
    open_settings: Query<Entity, With<SettingsScreen>>,
    menus: Query<Entity, With<PauseMenu>>,
    mut time: ResMut<Time<Virtual>>,
    (mut survey, mut markers, mut armed): (
        ResMut<crate::survey::Survey>,
        ResMut<crate::markers::MarkerMode>,
        ResMut<crate::miracles::SelectedMiracle>,
    ),
    mut app_exit: MessageWriter<AppExit>,
) {
    for interaction in &to_title {
        if *interaction == Interaction::Pressed {
            for menu in &menus {
                commands.entity(menu).insert(Visibility::Hidden);
            }
            for mut visibility in &mut saves_panels {
                *visibility = Visibility::Hidden;
            }
            // Nothing of the abandoned game follows the god upstairs: no
            // follow, no open sight, no marks, no armed miracle - and the
            // clock runs on, because the title overlooks a living world.
            // The codex shuts and turns back to its first page, so the
            // next game opens the book fresh.
            for mut visibility in &mut codex_panels {
                *visibility = Visibility::Hidden;
            }
            codex.page = crate::debug::CodexPage::Ledger;
            time.unpause();
            survey.on = false;
            markers.0 = false;
            armed.0 = None;
            // The world itself is replaced with a freshly-founded one, so
            // Begin from the title is a true new game — behind a curtain, so
            // the building of it is not a thing the player watches.
            go_homeward(&mut commands);
        }
    }
    for interaction in &resume {
        if *interaction == Interaction::Pressed {
            for menu in &menus {
                commands.entity(menu).insert(Visibility::Hidden);
            }
            time.unpause();
        }
    }
    for interaction in &settings {
        if *interaction == Interaction::Pressed && open_settings.is_empty() {
            commands.run_system_cached(spawn_settings);
        }
    }
    for interaction in &saves {
        if *interaction == Interaction::Pressed {
            for mut visibility in &mut saves_panels {
                *visibility = Visibility::Visible;
            }
        }
    }
    for interaction in &exit {
        if *interaction == Interaction::Pressed {
            app_exit.write(AppExit::Success);
        }
    }
}

// ---------------------------------------------------------------------------
// The drift, the farewell, and the descent.
// ---------------------------------------------------------------------------

/// Where the title looks from: far enough out that the whole planet sits in
/// the frame, held in the god's hand.
///
/// It used to be a god's-height vantage a hundred and seventy-five units over
/// the village, drifting so slowly nobody caught it moving. That was the right
/// screen for a world with no edge to see. This one has an edge, and the first
/// thing the game should say is what the player is being handed.
const DRIFT_DISTANCE: f32 = 31_000.0;
/// Nearly straight down, which is what CENTRES the ball.
///
/// The rig aims at a point on the ground, not at the planet's middle, and from
/// a low pitch those are two very different directions: the first try looked at
/// the village from off to one side and the planet hung off the bottom of the
/// frame like a hill. Overhead, the aim runs almost through the centre and the
/// whole sphere sits in the middle of the screen. It costs nothing in light —
/// the sun is where the sun is, whatever the camera does — and the yaw drift
/// becomes a globe turning on its own axis rather than a camera flying round a
/// world.
const DRIFT_PITCH: f32 = 1.50;
/// How long the vantage takes to go once round the planet, in seconds.
///
/// The camera travels and the world does not: the simulation's whole coordinate
/// system is pinned to this sphere, so spinning the planet would spin every
/// village on it. Circling it is the same picture and costs nothing.
///
/// It used to circle by adding to the camera's YAW, which is not circling
/// anything. At the title's pitch the eye stands almost straight out from its
/// focus, so yaw barely moves it - it turns the image about its own middle
/// instead, and the planet span like a record seen from above rather than
/// turning like a globe.
///
/// The FOCUS walks instead, east along its own line of latitude, and the eye
/// goes with it because it stands radially out from wherever the focus is. The
/// view stays pointed at the planet's centre the whole way round, so what the
/// eye sees is the world turning past it, edge on.
const DRIFT_LAP: f32 = 300.0;

/// How far the title turns the camera off its aim, in radians. NEGATIVE slides
/// the world left, which is where the planet belongs while the menu has the
/// right of the frame — turning the camera right is what moves the world left,
/// and I had that back to front.
const DRIFT_AIM: f32 = -0.16;

/// Where the menu column starts, as a fraction of the screen's height. It runs
/// from there to the foot of the frame and centres its buttons in what is left,
/// so its own middle is halfway between this and the bottom.
const MENU_TOP: f32 = 0.24;

/// How far the title turns the camera DOWN its aim, so the planet's middle
/// lands level with the menu's.
///
/// Derived rather than dialled. The planet and the menu read as a pair, so the
/// number that places one has to be the number that places the other - and a
/// constant tuned by eye against one build of the camera goes wrong the moment
/// the camera changes, which is exactly what happened: the eye used to be
/// seated by its own coordinates and is a rigid offset in the rig's frame now,
/// which reframes anything as far out as this.
///
/// Turning the camera up moves the world down, so this is positive.
fn menu_middle_aim() -> f32 {
    let middle = (MENU_TOP + 1.0) * 0.5;
    // How far off the frame's centre that is, in half-heights.
    let off = (middle - 0.5) * 2.0;
    (off * (crate::camera::FIELD_OF_VIEW * 0.5).tan()).atan()
}

/// Slowly circles the camera over the village while the pre-game screens are
/// up. The world lives underneath — fires burning, villagers about their
/// day — which is the whole point of looking at it.
fn drift_title_camera(
    time: Res<Time<Real>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    mut rigs: Query<&mut crate::camera::CameraRig>,
) {
    // Unattended captures frame their own shot; only title portraits drift.
    if crate::capture_path().is_some() && !title_capture() {
        return;
    }
    let Ok(mut rig) = rigs.single_mut() else {
        return;
    };
    // A restored world tours its own village's latitude, so the ground the
    // player left is the ground that comes round. Only the latitude: the
    // longitude is what the drift is walking.
    if let Some(site) = site {
        rig.target_focus.z = site.centre.z;
    }
    rig.target_distance = DRIFT_DISTANCE;
    rig.target_pitch = DRIFT_PITCH;
    // The planet to the left, the menu to the right: the camera turns off its
    // own aim rather than the world moving. Nothing else in the game shifts its
    // aim, so this is set here and cleared on the way out.
    rig.aim_offset = Vec2::new(DRIFT_AIM, menu_middle_aim());
    rig.target_focus.x += crate::terrain::planet_circumference() / DRIFT_LAP * time.delta_secs();
    // Sent out to the sphere and brought home, so a lap does not walk the
    // coordinates off the end of the world.
    rig.target_focus = crate::camera::fold_onto_the_sphere(rig.target_focus);
    rig.zoom_anchor = None;
}

/// The descent. When the world finished building behind the title — which on
/// most machines it long since has — there is no loading screen and no cut:
/// the camera simply dives from the title vantage down to the village. The
/// loading screen survives as the fallback for a world still under
/// construction.
fn begin_descent(
    commands: &mut Commands,
    chunks: Option<&crate::terrain::LoadedChunks>,
    site: Option<&crate::villager::SettlementSite>,
    vantage: Option<&crate::founding::OpeningVantage>,
    next: &mut NextState<GameState>,
) {
    // Where the dive lands. A RESTORED world descends onto its own
    // village; a new one has none to descend to - the god has not chosen
    // the ground yet - so it comes down over the middle of the map with
    // the flag in hand.
    //
    // This used to refuse to leave the title at all without a settlement,
    // which was right while the founding happened at startup: the dive
    // would otherwise drop the player into a playing world with the
    // camera still at the title vantage, the village a thousand units
    // below and nothing left that would ever queue a dive. Now an absent
    // village is the ordinary case rather than a gap to wait out.
    let landing = site
        .map(|site| site.centre)
        .or(vantage.map(|vantage| vantage.0))
        .unwrap_or(Vec3::ZERO);
    if chunks.is_some_and(|c| c.is_complete()) {
        next.set(if site.is_some() {
            GameState::Playing
        } else {
            GameState::Choosing
        });
    } else {
        next.set(GameState::Loading);
    }
    commands.insert_resource(crate::camera::CameraDive::descend_to(landing));
}

/// Capture tooling: DIVUS_FACTUS_AUTOBEGIN=seconds presses Begin unattended, so
/// the descent itself can be photographed.
fn auto_begin(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut waited: Local<f32>,
    chunks: Option<Res<crate::terrain::LoadedChunks>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    vantage: Option<Res<crate::founding::OpeningVantage>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(after) = std::env::var("DIVUS_FACTUS_AUTOBEGIN")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
    else {
        return;
    };
    *waited += time.delta_secs();
    if *waited >= after {
        // Reset rather than latch: if the game walks back to the title (the
        // pause menu's Title door), the next Begin fires after a full wait
        // again — pressing it on the title's first frame would dive on the
        // OLD world's coordinates before the fresh founding has landed.
        *waited = 0.0;
        begin_descent(
            &mut commands,
            chunks.as_deref(),
            site.as_deref(),
            vantage.as_deref(),
            &mut next,
        );
    }
}

/// Capture tooling: DIVUS_FACTUS_AUTOTITLE=seconds walks back to the title after
/// N seconds of play — through the same door as the pause menu's Title
/// button, new world and all — so the whole loop can be proven on film.
#[allow(clippy::type_complexity)]
fn auto_title(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut waited: Local<f32>,
    mut fired: Local<bool>,
    (mut survey, mut markers, mut armed): (
        ResMut<crate::survey::Survey>,
        ResMut<crate::markers::MarkerMode>,
        ResMut<crate::miracles::SelectedMiracle>,
    ),
) {
    if *fired {
        return;
    }
    let Some(after) = std::env::var("DIVUS_FACTUS_AUTOTITLE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
    else {
        return;
    };
    *waited += time.delta_secs();
    if *waited >= after {
        *fired = true;
        survey.on = false;
        markers.0 = false;
        armed.0 = None;
        go_homeward(&mut commands);
    }
}

/// The title's arrival, 0 to 1: the lettering and the smoke come up out of
/// nothing.
///
/// The title had a farewell and no welcome. Leaving it was a fade and arriving
/// was not — which nobody noticed while the only arrival was out of the splash's
/// black, where there is nothing to fade FROM. Coming back from a game there is:
/// Brett, on the curtain lifting, "the title screen popped in, it didnt fade in."
///
/// Deliberately AFTER the curtain rather than under it. The curtain lifting
/// reveals the new world, and the lettering then comes up over it, so the two
/// read as one movement — arriving somewhere, then being told where you are —
/// instead of the whole screen appearing at once.
#[derive(Resource)]
struct TitleWelcome {
    t: f32,
}

impl TitleWelcome {
    /// How far up, eased. The ONE definition: the smoke scales its own alpha by
    /// this and so does everything else, and a second copy of the easing would
    /// drift the moment one of them was tuned.
    fn risen(&self) -> f32 {
        self.t * self.t * (3.0 - 2.0 * self.t)
    }
}

/// Seconds the welcome takes. Slower than the farewell: leaving is a dive and
/// should feel like one, arriving is not.
const WELCOME_SECONDS: f32 = 1.1;

/// Brings the lettering and the smoke up, once anything covering them has gone.
///
/// Everything under the title's own root, found by walking down from it rather
/// than by a marker on each piece. The title spawns a good deal — logotype,
/// smoke, four buttons, their words — and tagging every one of them would mean a
/// marker that must be remembered at each new spawn site or the new thing quietly
/// fails to fade. The tree already knows what belongs to the title.
///
/// Only the images and the words. Alpha on a button's border and background
/// would need each one's own resting alpha remembered to scale it, and those are
/// faint against space to begin with; the lettering and the labels are what the
/// eye follows up out of the black.
fn play_welcome(
    mut commands: Commands,
    time: Res<Time<Real>>,
    homeward: Option<Res<Homeward>>,
    welcome: Option<ResMut<TitleWelcome>>,
    screens: Query<Entity, With<TitleScreen>>,
    kin: Query<&Children>,
    // NOT the smoke. Every puff sets its own colour every frame — a tint and an
    // alpha that breathe along its lane — and writing plain white over it from
    // here left the two systems trading the same value once a frame. Brett saw
    // it immediately: "the smoke flashes for a few seconds before stopping",
    // which is exactly the length of this fade. The smoke scales itself by
    // [`TitleWelcome::risen`] instead; see `drift_smoke`.
    mut arts: Query<&mut ImageNode, Without<SmokePuff>>,
    mut texts: Query<&mut TextColor>,
) {
    let Some(mut welcome) = welcome else {
        return;
    };
    // While the curtain is still up, the title waits its turn at nothing.
    if homeward.is_none() {
        welcome.t = (welcome.t + time.delta_secs() / WELCOME_SECONDS).min(1.0);
    }
    let eased = welcome.risen();

    let mut walk: Vec<Entity> = screens.iter().collect();
    while let Some(here) = walk.pop() {
        if let Ok(mut art) = arts.get_mut(here) {
            art.color = Color::srgba(1.0, 1.0, 1.0, eased);
        }
        if let Ok(mut text) = texts.get_mut(here) {
            text.0 = text.0.with_alpha(eased);
        }
        if let Ok(children) = kin.get(here) {
            walk.extend(children.iter());
        }
    }
    if welcome.t >= 1.0 {
        commands.remove_resource::<TitleWelcome>();
    }
}

/// The curtain over the walk back to the title.
///
/// Brett, on the pause menu's Title door: "I saw the world building in." He
/// could — the old world is razed and a new one grown on a fresh seed, and until
/// this the title arrived at once and the player watched the planet assemble
/// itself patch by patch behind the lettering.
///
/// So the way home is covered. The world darkens, the swap happens behind a
/// black screen, and the curtain lifts only once the new world has actually
/// FINISHED — `PlanetDetail::owed` is the number of patches the planet still
/// owes, and nought is the honest signal that there is nothing left to see
/// appear. Not a duration guessed at: on a slow machine a fixed wait would lift
/// on a half-built world, and on a fast one it would sit on black for no reason.
///
/// With a cap all the same, because a curtain that never lifts is worse than
/// the thing it was hiding.
#[derive(Resource)]
struct Homeward {
    /// 0 in the world, 1 fully black.
    t: f32,
    /// Whether the old world has been razed and the new one begun.
    swapped: bool,
    /// Seconds spent holding black, against [`HOMEWARD_PATIENCE`].
    held: f32,
}

/// The black itself.
#[derive(Component)]
struct HomewardCurtain;

/// Seconds to darken the world, and to lift off the new one.
///
/// Longer coming up than going down: dropping out of a world should be brisk,
/// and arriving somewhere should not. The lift was 0.9 and read as a pop —
/// after several seconds of black, nine tenths of a second is not enough of a
/// gradient for the eye to call it a fade.
const HOMEWARD_DARKEN: f32 = 0.4;
const HOMEWARD_LIFT: f32 = 1.6;

/// How long the curtain will wait for the planet to finish before lifting
/// anyway.
///
/// Measured rather than picked: growing a fresh planet took 4.5 seconds here,
/// with `owed` falling steadily from 2,185 patches to nought. Six seconds left
/// almost no room, and a machine slower than this one would have lifted onto a
/// half-built world — which is the very thing being hidden. Nine gives it half
/// again as long as it needed, and still bounds the black.
const HOMEWARD_PATIENCE: f32 = 9.0;

/// Starts for the title, behind a curtain. The one door home, so the pause
/// menu and the capture tooling take the same route.
fn go_homeward(commands: &mut Commands) {
    commands.spawn((
        Name::new("The Way Home"),
        HomewardCurtain,
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.0)),
        // Over everything, the loading screen included: this covers a state
        // change, and whatever else is on screen is what we are hiding.
        GlobalZIndex(400),
    ));
    commands.insert_resource(Homeward {
        t: 0.0,
        swapped: false,
        held: 0.0,
    });
}

/// Darkens, swaps the world, waits for it to stand, then lifts.
fn draw_the_curtain(
    mut commands: Commands,
    time: Res<Time<Real>>,
    detail: Res<crate::globe::PlanetDetail>,
    mut next: ResMut<NextState<GameState>>,
    homeward: Option<ResMut<Homeward>>,
    mut curtains: Query<(Entity, &mut BackgroundColor), With<HomewardCurtain>>,
) {
    let Some(mut homeward) = homeward else {
        return;
    };
    let step = time.delta_secs();
    if !homeward.swapped {
        homeward.t = (homeward.t + step / HOMEWARD_DARKEN).min(1.0);
        if homeward.t >= 1.0 {
            // Black. Now, and not a frame before, the world is taken away.
            homeward.swapped = true;
            commands.insert_resource(crate::save::PendingNewWorld);
            next.set(GameState::Title);
        }
    } else {
        homeward.held += step;
        // `owed` climbs the moment the tree is felled, so a patient frame or two
        // is needed before nought means finished rather than not-yet-started.
        let standing = homeward.held > HOMEWARD_DARKEN && detail.owed == 0;
        if standing || homeward.held > HOMEWARD_PATIENCE {
            homeward.t = (homeward.t - step / HOMEWARD_LIFT).max(0.0);
        }
        if homeward.t <= 0.0 {
            for (curtain, _) in &curtains {
                commands.entity(curtain).despawn();
            }
            commands.remove_resource::<Homeward>();
            return;
        }
    }
    // Eased, so neither end of the fade has a corner in it.
    let eased = homeward.t * homeward.t * (3.0 - 2.0 * homeward.t);
    for (_, mut curtain) in &mut curtains {
        *curtain = BackgroundColor(Color::BLACK.with_alpha(eased));
    }
}

/// The title's exit, in progress from 0 to 1: the scrim thins, the lettering
/// fades, the smoke blows away — all while the camera dives.
#[derive(Resource)]
struct TitleFarewell {
    t: f32,
}

/// Seconds the farewell takes — inside the camera's dive, so the veil is gone
/// before the descent lands.
const FAREWELL_SECONDS: f32 = 1.7;

/// Leaving the title does not cut to black; it lets go. The buttons vanish at
/// once — a half-faded button must never take a click — and everything else
/// hands itself to [`play_farewell`] to fade out over the descent.
fn begin_farewell(
    mut commands: Commands,
    screens: Query<Entity, With<TitleScreen>>,
    buttons: Query<Entity, With<TitleMenu>>,
    settings: Query<Entity, With<SettingsScreen>>,
) {
    for screen in &settings {
        commands.entity(screen).despawn();
    }
    for button in &buttons {
        // Hidden, not despawned: yanking four buttons out of the centred
        // column would reflow it, and the lettering above them visibly
        // jumped. The slots stay; the buttons just stop being there to see
        // or to press.
        commands
            .entity(button)
            .remove::<Interaction>()
            .insert(Visibility::Hidden);
    }
    let mut leaving = false;
    for screen in &screens {
        leaving = true;
        // The scrim stops being a wall the moment the descent starts: no
        // more picking, no more hand-over-interface pose.
        commands.entity(screen).remove::<(Interaction, ui::Panel)>();
    }
    if leaving {
        // And the arrival is over, whatever it had reached. Begin pressed within
        // a second of the title appearing would otherwise leave the welcome
        // fading things IN while the farewell faded the same things out.
        commands.remove_resource::<TitleWelcome>();
        commands.insert_resource(TitleFarewell { t: 0.0 });
    }
}

fn play_farewell(
    mut commands: Commands,
    time: Res<Time<Real>>,
    farewell: Option<ResMut<TitleFarewell>>,
    mut screens: Query<(Entity, &mut BackgroundColor), With<TitleScreen>>,
    mut arts: Query<&mut ImageNode, With<TitleArt>>,
) {
    let Some(mut farewell) = farewell else {
        return;
    };
    farewell.t = (farewell.t + time.delta_secs() / FAREWELL_SECONDS).min(1.0);
    let eased = farewell.t * farewell.t * (3.0 - 2.0 * farewell.t);
    let fade = 1.0 - eased;

    for (_, mut scrim) in &mut screens {
        *scrim = BackgroundColor(theme::panel_bg().with_alpha(SCRIM_ALPHA * fade));
    }
    for mut art in &mut arts {
        art.color = Color::srgba(1.0, 1.0, 1.0, fade);
    }

    if farewell.t >= 1.0 {
        for (screen, _) in &screens {
            commands.entity(screen).despawn();
        }
        commands.remove_resource::<TitleFarewell>();
    }
}

/// The game's own furniture — toolbar, time controls, belief meter, toasts —
/// belongs to play. Over the pre-game's translucent veil it reads as a bug.
fn sync_hud(state: Res<State<GameState>>, mut huds: Query<&mut Visibility, With<ui::GameHud>>) {
    let playing = matches!(state.get(), GameState::Playing);
    for mut visibility in &mut huds {
        *visibility = if playing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// The settings overlay: for now, one setting — the colour of the hand.
/// The hand itself hangs over this screen in its pointing pose, so every
/// swatch click is previewed on the actual instrument.
/// The settings, on the title screen and in the pause menu.
///
/// It shows the CODEX's settings panel, which is the only settings panel in this
/// game. It used to have its own — hand colours and nothing else — so there were
/// two places to add a setting to and two chances to forget the second, and the
/// two had already drifted. Brett's call, and plainly right: one panel, hosted in
/// whichever frame is on screen.
fn spawn_settings(mut commands: Commands) {
    let screen = commands
        .spawn((
            Name::new("Settings Screen"),
            SettingsScreen,
            ui::Panel,
            Interaction::default(),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(14),
                ..default()
            },
            BackgroundColor(theme::panel_bg().with_alpha(1.0)),
            GlobalZIndex(310),
        ))
        .id();

    let heading = commands
        .spawn((
            Text::new("SETTINGS"),
            TextFont {
                font_size: FontSize::Px(34.0),
                ..default()
            },
            TextColor(theme::accent()),
        ))
        .id();
    commands.entity(heading).insert(ChildOf(screen));

    // The panel itself, in a frame the size of the codex's own page so the
    // tabs and their tables have the room they were laid out for.
    let frame = commands
        .spawn((
            Node {
                width: px(940),
                height: percent(66),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                padding: UiRect::all(px(18)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::title_bg()),
            BorderColor::all(theme::panel_border().with_alpha(0.5)),
            ChildOf(screen),
        ))
        .id();
    crate::debug::village::build_settings_page(&mut commands, frame);

    let back = menu_button(&mut commands, screen, "Back");
    commands.entity(back).insert(BackButton);
}

/// Builds the view switches into a parent, for whoever is showing the settings.
pub(crate) fn build_view_switches(commands: &mut Commands, parent: Entity) {
    let screen = parent;
    // The view switches.
    let label = commands
        .spawn((
            ui::dim("what the world shows you"),
            Node {
                margin: UiRect::top(px(4)),
                ..default()
            },
        ))
        .id();
    commands.entity(label).insert(ChildOf(screen));

    for switch in ViewSwitch::ALL {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(14),
                    width: px(360),
                    margin: UiRect::vertical(px(4)),
                    ..default()
                },
                ChildOf(screen),
            ))
            .id();

        // The switch itself: a track with a knob, which is the one shape
        // everybody already knows how to read.
        let track = commands
            .spawn((
                switch,
                ui::UiButton,
                ui::KeepFace,
                Node {
                    width: px(52),
                    height: px(26),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(13)),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(3)),
                    ..default()
                },
                BackgroundColor(theme::title_bg()),
                BorderColor::all(theme::panel_border().with_alpha(0.5)),
                Interaction::default(),
                ChildOf(row),
            ))
            .id();
        commands.spawn((
            SwitchKnob(switch),
            Node {
                width: px(18),
                height: px(18),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BackgroundColor(theme::accent()),
            ChildOf(track),
        ));

        let words = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(1),
                    ..default()
                },
                ChildOf(row),
            ))
            .id();
        let name = commands
            .spawn((
                Text::new(switch.label()),
                ui::DisplayFace,
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(theme::accent()),
            ))
            .id();
        commands.entity(name).insert(ChildOf(words));
        let note = commands.spawn(ui::dim(switch.note())).id();
        commands.entity(note).insert(ChildOf(words));
    }
}

/// The knob inside a switch's track, which slides to say which way it is set.
#[derive(Component)]
struct SwitchKnob(ViewSwitch);

/// Clicks flip a switch; the knob and the track then say so.
///
/// Reads the same state the world reads, so the switch cannot drift out of step
/// with what it governs — there is one truth and the switch is a window onto it.
fn handle_view_switches(
    clicks: Query<(&Interaction, &ViewSwitch), Changed<Interaction>>,
    mut clear: ResMut<crate::clouds::TheSkyIsClear>,
    mut fog: ResMut<crate::fog::FogMode>,
    mut layers: ResMut<crate::debug::layers::ViewLayers>,
    mut tracks: Query<(&ViewSwitch, &mut BackgroundColor, &mut BorderColor)>,
    mut knobs: Query<(&SwitchKnob, &mut Node, &mut BackgroundColor), Without<ViewSwitch>>,
) {
    for (interaction, switch) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match switch {
            ViewSwitch::Clouds => clear.0 = !clear.0,
            ViewSwitch::Veil => fog.0 = !fog.0,
            ViewSwitch::Layer(layer) => layers.toggle(*layer),
        }
    }

    let on = |switch: &ViewSwitch| match switch {
        // The switch reads as the THING, not as its absence: "clouds on" means
        // there is weather, so the sky being clear is the switch being off.
        ViewSwitch::Clouds => !clear.0,
        ViewSwitch::Veil => fog.0,
        ViewSwitch::Layer(layer) => layers.shown(*layer),
    };
    for (switch, mut fill, mut border) in &mut tracks {
        let lit = on(switch);
        *fill = BackgroundColor(if lit {
            theme::accent().with_alpha(0.30)
        } else {
            theme::title_bg()
        });
        *border = BorderColor::all(if lit {
            theme::accent()
        } else {
            theme::panel_border().with_alpha(0.5)
        });
    }
    for (knob, mut node, mut fill) in &mut knobs {
        let lit = on(&knob.0);
        node.margin = if lit {
            UiRect::left(px(26))
        } else {
            UiRect::left(px(0))
        };
        *fill = BackgroundColor(if lit {
            theme::accent()
        } else {
            theme::panel_border()
        });
    }
}

/// Swatch clicks restyle the hand; the chosen swatch wears the gold border.
fn handle_settings(
    mut commands: Commands,
    back: Query<&Interaction, (Changed<Interaction>, With<BackButton>)>,
    screens: Query<Entity, With<SettingsScreen>>,
    mut style: ResMut<crate::hand::HandStyle>,
    mut swatches: Query<(&Interaction, &HandSwatch, &mut BorderColor)>,
) {
    for (interaction, swatch, _) in &swatches {
        if *interaction == Interaction::Pressed
            && let Some((_, ramp)) = crate::hand::HAND_STYLES.get(swatch.0)
            && style.ramp != *ramp
        {
            style.ramp = ramp;
        }
    }
    for (_, swatch, mut border) in &mut swatches {
        let chosen = crate::hand::HAND_STYLES
            .get(swatch.0)
            .is_some_and(|(_, ramp)| style.ramp == *ramp);
        *border = BorderColor::all(if chosen {
            theme::accent()
        } else {
            theme::panel_border()
        });
    }

    for interaction in &back {
        if *interaction == Interaction::Pressed {
            for screen in &screens {
                commands.entity(screen).despawn();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_choice(
    mut commands: Commands,
    begin: Query<&Interaction, (Changed<Interaction>, With<BeginButton>)>,
    loads: Query<&Interaction, (Changed<Interaction>, With<LoadGameButton>)>,
    mut saves_panels: Query<&mut Visibility, With<crate::save::SavesPanel>>,
    settings: Query<&Interaction, (Changed<Interaction>, With<SettingsButton>)>,
    quit: Query<&Interaction, (Changed<Interaction>, With<QuitButton>)>,
    bench: Query<&Interaction, (Changed<Interaction>, With<AtelierButton>)>,
    open_settings: Query<Entity, With<SettingsScreen>>,
    chunks: Option<Res<crate::terrain::LoadedChunks>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    vantage: Option<Res<crate::founding::OpeningVantage>>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for interaction in &begin {
        if *interaction == Interaction::Pressed {
            begin_descent(
                &mut commands,
                chunks.as_deref(),
                site.as_deref(),
                vantage.as_deref(),
                &mut next,
            );
        }
    }
    // The Atelier takes over: the bench opens and the game stands down.
    //
    // Two programs at once would be two programs fighting over one machine's
    // graphics for no reason - nobody draws a building and plays at the same
    // time - and the bench's whole promise is that what is saved there is
    // carried in by hand afterwards, which is a thing you do on the way BACK.
    for interaction in &bench {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(path) = atelier_beside_us() else {
            continue;
        };
        // From its own folder, so it finds its palette and its fonts the way it
        // does when a maker runs it themselves.
        let home = path.parent().map(std::path::Path::to_path_buf);
        let mut opening = std::process::Command::new(&path);
        if let Some(home) = home {
            opening.current_dir(home);
        }
        match opening.spawn() {
            Ok(_) => {
                info!("the bench is open: {}", path.display());
                exit.write(AppExit::Success);
            }
            // Left standing rather than quitting into nothing: a game that
            // closed and opened neither would look like a crash.
            Err(why) => warn!("the bench would not open ({why}): {}", path.display()),
        }
    }

    // Load Game opens the saves window over the title; picking a slot there
    // restores the world and walks through the same door Begin uses.
    for interaction in &loads {
        if *interaction == Interaction::Pressed {
            for mut visibility in &mut saves_panels {
                *visibility = Visibility::Visible;
            }
        }
    }
    for interaction in &settings {
        if *interaction == Interaction::Pressed && open_settings.is_empty() {
            commands.run_system_cached(spawn_settings);
        }
    }
    for interaction in &quit {
        if *interaction == Interaction::Pressed {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests {
    /// The planet's middle lands level with the menu's.
    ///
    /// Both are placed from `MENU_TOP`, so the only way they can part company
    /// is if this arithmetic is wrong - which is worth saying out loud, because
    /// the symptom is "the planet looks a bit high" and nobody can check that
    /// by eye against a number in another file.
    #[test]
    fn the_planet_is_aimed_at_the_menus_middle() {
        let aim = super::menu_middle_aim();
        // Undo it: an angle back into a fraction of the screen.
        let off = aim.tan() / (crate::camera::FIELD_OF_VIEW * 0.5).tan();
        let landed = 0.5 + off * 0.5;
        let menu = (super::MENU_TOP + 1.0) * 0.5;
        assert!(
            (landed - menu).abs() < 1e-4,
            "the planet lands at {landed} of the screen and the menu sits at {menu}",
        );
        assert!(
            aim > 0.0,
            "turning the camera up is what moves the world down",
        );
    }

    /// Frames the way a real launch actually delivers them: the world is
    /// generated behind the splash, so the first few take SECONDS.
    ///
    /// Measured off a run, not invented. `DIVUS_FACTUS_FRAMES=1` reported six
    /// frames in the first five seconds, worst 4.65s.
    const A_REAL_LAUNCH: [f32; 6] = [4.65, 0.84, 0.31, 0.12, 0.09, 0.05];

    #[test]
    fn a_slow_first_frame_cannot_swallow_the_studios_mark() {
        // The bug Brett found: the mark's whole life is 4.4 seconds of real
        // time, and ONE frame was longer than that. It faded in, held and
        // faded out inside a frame that had drawn nothing, so the studio's
        // mark never reached the screen.
        let mut spent = 0.0;
        for delta in A_REAL_LAUNCH {
            spent += super::splash_step(delta);
        }
        assert!(
            spent < super::splash_life(),
            "the whole mark was spent in the {} frames it takes to open a \
             world, before any of it was drawn: {spent}",
            A_REAL_LAUNCH.len()
        );
    }

    #[test]
    fn the_mark_is_worth_a_useful_number_of_drawn_frames() {
        let mut spent = 0.0;
        let mut frames = 0;
        for delta in A_REAL_LAUNCH {
            spent += super::splash_step(delta);
            frames += 1;
        }
        while spent < super::splash_life() {
            spent += super::splash_step(1.0 / 60.0);
            frames += 1;
        }
        assert!(
            frames > 40,
            "the mark only got {frames} frames on screen - a studio card that \
             blinks is worse than none"
        );
    }

    #[test]
    fn the_mark_still_leaves_a_machine_that_never_speeds_up() {
        // The cap must not become a hang. If EVERY frame stalls the splash
        // still ends - which is why the step is bounded rather than thrown
        // away, and the reason that distinction is worth a test.
        let mut spent = 0.0;
        let mut frames = 0;
        while spent < super::splash_life() {
            spent += super::splash_step(9.0);
            frames += 1;
            assert!(frames < 1_000, "the splash never ended");
        }
    }
}
