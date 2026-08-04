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
use bevy::window::PrimaryWindow;

use crate::GameState;
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
                    sync_hud.run_if(state_changed::<GameState>),
                    handle_choice.run_if(in_state(GameState::Title)),
                    auto_begin.run_if(in_state(GameState::Title)),
                    auto_title.run_if(in_state(GameState::Playing)),
                    handle_settings,
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

/// Real seconds since the splash appeared.
#[derive(Resource)]
struct SplashClock(f32);

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

    commands.insert_resource(SplashClock(0.0));
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
    clock.0 += time.delta_secs();

    let fade_out_at = SPLASH_FADE + SPLASH_HOLD;
    let alpha = if clock.0 < SPLASH_FADE {
        clock.0 / SPLASH_FADE
    } else if clock.0 < fade_out_at {
        1.0
    } else {
        1.0 - (clock.0 - fade_out_at) / SPLASH_FADE
    }
    .clamp(0.0, 1.0);

    // A key press skips ahead to the fade-out rather than cutting: the mark
    // still leaves the way it always leaves, just now. Jumping to the point
    // on the out-fade with the current alpha keeps the brightness continuous.
    let skipped =
        keys.get_just_pressed().next().is_some() || buttons.get_just_pressed().next().is_some();
    if skipped && clock.0 < fade_out_at {
        clock.0 = fade_out_at + (1.0 - alpha) * SPLASH_FADE;
    }

    for mut art in &mut arts {
        art.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
    // The studio's line keeps to a whisper of the mark's own light.
    for mut mark in &mut marks {
        mark.0 = Color::srgba(0.82, 0.8, 0.76, alpha * 0.62);
    }

    if clock.0 >= fade_out_at + SPLASH_FADE {
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
                top: percent(24),
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
const SMOKE_HEART: Vec2 = Vec2::new(0.5, 0.3);

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
            // Hugging the cropped logotype's band rather than the tall
            // canvas it once shipped inside.
            home_y: 0.21 + 0.17 * (f * 0.377).fract(),
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
    windows: Query<&Window, With<PrimaryWindow>>,
    mut puffs: Query<(&SmokePuff, &mut Node, &mut ImageNode)>,
) {
    if puffs.is_empty() {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let (width, height) = (window.width(), window.height());
    let t = time.elapsed_secs();
    let blow = farewell.map_or(0.0, |f| f.t);

    for (puff, mut node, mut image) in &mut puffs {
        // The crawl along the lane, plus a slow private wander around it.
        let along = (puff.lane + t * puff.drift).fract();
        let mut x = 0.14 + 0.72 * along + 0.02 * (t * 0.043 + puff.phase).sin();
        let mut y = puff.home_y
            + 0.016 * (t * 0.053 + puff.phase * 1.7).sin()
            + 0.010 * (t * 0.031 + puff.phase * 2.3).cos();

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
            let away = (Vec2::new(x, y) - SMOKE_HEART) * 2.6 + Vec2::new(0.35, -0.18);
            x += away.x * s * 0.55;
            y += away.y * s * 0.55;
            alpha *= (1.0 - blow).max(0.0);
        }

        node.left = px(x * width - puff.size * 0.5);
        node.top = px(y * height - puff.size * 0.5);
        node.width = px(puff.size);
        node.height = px(puff.size);
        image.color = SMOKE_TINT.with_alpha(alpha);
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
    mut next: ResMut<NextState<GameState>>,
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
            // Begin from the title is a true new game.
            commands.insert_resource(crate::save::PendingNewWorld);
            next.set(GameState::Title);
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
/// Radians per second the vantage circles the planet. The camera turns and the
/// world does not: the simulation's whole coordinate system is pinned to this
/// sphere, so spinning the planet would spin every village on it. Circling it
/// instead is the same picture and costs nothing. A lap takes five minutes.
const DRIFT_TURN: f32 = 0.021;

/// How far the title turns the camera off its aim, in radians. NEGATIVE slides
/// the world left, which is where the planet belongs while the menu has the
/// right of the frame — turning the camera right is what moves the world left,
/// and I had that back to front.
const DRIFT_AIM: f32 = -0.16;

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
    if let Some(site) = site {
        rig.target_focus.x = site.centre.x;
        rig.target_focus.z = site.centre.z;
    }
    rig.target_distance = DRIFT_DISTANCE;
    rig.target_pitch = DRIFT_PITCH;
    // The planet to the left, the menu to the right: the camera turns off its
    // own aim rather than the world moving. Nothing else in the game shifts its
    // aim, so this is set here and cleared on the way out.
    rig.aim_offset = Vec2::new(DRIFT_AIM, 0.0);
    rig.target_yaw += DRIFT_TURN * time.delta_secs();
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
    mut next: ResMut<NextState<GameState>>,
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
        commands.insert_resource(crate::save::PendingNewWorld);
        next.set(GameState::Title);
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
                row_gap: px(16),
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

    let label = commands
        .spawn((
            ui::dim("the colour of your hand"),
            Node {
                margin: UiRect::top(px(10)),
                ..default()
            },
        ))
        .id();
    commands.entity(label).insert(ChildOf(screen));

    let row = commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(10),
            margin: UiRect::bottom(px(18)),
            ..default()
        },))
        .id();
    commands.entity(row).insert(ChildOf(screen));

    for (index, (_, ramp)) in crate::hand::HAND_STYLES.iter().enumerate() {
        let swatch = commands
            .spawn((
                HandSwatch(index),
                ui::UiButton,
                ui::KeepFace,
                Node {
                    width: px(40),
                    height: px(40),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(crate::palette::shade(ramp, 0.9)),
                BorderColor::all(theme::panel_border()),
                Interaction::default(),
                ChildOf(row),
            ))
            .id();
        let _ = swatch;
    }

    let back = menu_button(&mut commands, screen, "Back");
    commands.entity(back).insert(BackButton);
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
