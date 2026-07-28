//! The title screen: the front door.
//!
//! Kit-styled and quiet — the game's name, one line of what it is, and two
//! choices. The world generates behind it while the player reads, so "Begin"
//! usually opens onto a finished landscape with no waiting at all.

use bevy::prelude::*;

use crate::GameState;
use crate::ui::{self, theme};

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Title), (spawn_title, hold_the_world))
            .add_systems(OnExit(GameState::Title), (despawn_title, release_the_world))
            .add_systems(
                Update,
                (
                    handle_choice.run_if(in_state(GameState::Title)),
                    handle_settings,
                    toggle_pause_menu.run_if(in_state(GameState::Playing)),
                    handle_pause_menu,
                ),
            );
    }
}

#[derive(Component)]
struct TitleScreen;

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

/// The settings overlay, above the title.
#[derive(Component)]
struct SettingsScreen;

#[derive(Component)]
struct BackButton;

/// A hand-colour swatch, holding its place in [`crate::hand::HAND_STYLES`].
#[derive(Component)]
struct HandSwatch(usize);

fn menu_button(commands: &mut Commands, parent: Entity, label: &str) -> Entity {
    let button = commands
        .spawn((
            ui::UiButton,
            Node {
                width: px(220),
                padding: UiRect::axes(px(18), px(10)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BackgroundColor(theme::panel_bg().with_alpha(0.4)),
            BorderColor::all(theme::panel_border()),
            Interaction::default(),
            ChildOf(parent),
        ))
        .id();
    let text = commands.spawn(ui::body(label)).id();
    commands.entity(text).insert(ChildOf(button));
    button
}

fn spawn_title(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut next: ResMut<NextState<GameState>>,
) {
    // Unattended captures have nobody to click Begin.
    if crate::capture_path().is_some() {
        next.set(GameState::Loading);
        return;
    }

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
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(14),
                ..default()
            },
            BackgroundColor(theme::panel_bg().with_alpha(1.0)),
            // Above the world, the HUD and the loading screen alike.
            GlobalZIndex(300),
        ))
        .id();

    // The logotype — the game's one blessed piece of non-procedural art.
    let title = commands
        .spawn((
            ImageNode::new(assets.load("title-logo.png")),
            Node {
                width: px(640),
                margin: UiRect::bottom(px(-40.0)).with_top(px(-60.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(title).insert(ChildOf(screen));

    let tagline = commands
        .spawn((
            Text::new("a god of their making"),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(theme::text_dim()),
            Node {
                margin: UiRect::bottom(px(26)),
                ..default()
            },
        ))
        .id();
    commands.entity(tagline).insert(ChildOf(screen));

    let begin = menu_button(&mut commands, screen, "Begin");
    commands.entity(begin).insert(BeginButton);
    let load = menu_button(&mut commands, screen, "Load Game");
    commands.entity(load).insert(LoadGameButton);
    let settings = menu_button(&mut commands, screen, "Settings");
    commands.entity(settings).insert(SettingsButton);
    let quit = menu_button(&mut commands, screen, "Quit");
    commands.entity(quit).insert(QuitButton);
}

/// Escape raises the pause menu - the world holds its breath while it is
/// up, the same stillness the title uses. When a miracle is armed, Escape
/// belongs to disarming it instead.
fn toggle_pause_menu(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    armed: Res<crate::miracles::SelectedMiracle>,
    mut time: ResMut<Time<Virtual>>,
    menus: Query<(Entity, &Visibility), With<PauseMenu>>,
) {
    if !keys.just_pressed(KeyCode::Escape) || armed.0.is_some() {
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
            let exit = menu_button(&mut commands, window.body, "Exit Game");
            commands.entity(exit).insert(ExitButton);
            time.pause();
        }
    }
}

/// The pause menu's three doors.
#[allow(clippy::type_complexity)]
fn handle_pause_menu(
    mut commands: Commands,
    resume: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
    settings: Query<&Interaction, (Changed<Interaction>, With<PauseSettingsButton>)>,
    saves: Query<&Interaction, (Changed<Interaction>, With<PauseSavesButton>)>,
    mut saves_panels: Query<&mut Visibility, With<crate::save::SavesPanel>>,
    exit: Query<&Interaction, (Changed<Interaction>, With<ExitButton>)>,
    open_settings: Query<Entity, With<SettingsScreen>>,
    menus: Query<Entity, With<PauseMenu>>,
    mut time: ResMut<Time<Virtual>>,
    mut app_exit: MessageWriter<AppExit>,
) {
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

/// The world stands still behind the title: generated and visible, but not
/// living. Nobody builds a village while the player reads the menu.
fn hold_the_world(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn release_the_world(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
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

fn despawn_title(
    mut commands: Commands,
    screens: Query<Entity, With<TitleScreen>>,
    settings: Query<Entity, With<SettingsScreen>>,
) {
    for screen in screens.iter().chain(settings.iter()) {
        commands.entity(screen).despawn();
    }
}

fn handle_choice(
    mut commands: Commands,
    begin: Query<&Interaction, (Changed<Interaction>, With<BeginButton>)>,
    loads: Query<&Interaction, (Changed<Interaction>, With<LoadGameButton>)>,
    mut saves_panels: Query<&mut Visibility, With<crate::save::SavesPanel>>,
    settings: Query<&Interaction, (Changed<Interaction>, With<SettingsButton>)>,
    quit: Query<&Interaction, (Changed<Interaction>, With<QuitButton>)>,
    open_settings: Query<Entity, With<SettingsScreen>>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for interaction in &begin {
        if *interaction == Interaction::Pressed {
            next.set(GameState::Loading);
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
