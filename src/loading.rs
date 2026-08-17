//! The world-generation loading screen.
//!
//! Terrain is streamed, so the world *could* simply assemble itself around the
//! player. It looks bad: the opening seconds are chunks and forests popping into
//! existence a few meters away. Building the first full view before handing over
//! control costs a couple of seconds and means the first thing the player sees is a
//! finished landscape.
//!
//! This is deliberately not a fix for frame rate. Loading moves a cost from during
//! play to before it; it does nothing about per-frame work, and the streamed world's
//! frame cost was always the number of entities being shepherded, not the cost of
//! making them.

use bevy::prelude::*;

use crate::GameState;
use crate::palette;
use crate::terrain::LoadedChunks;
use crate::ui::theme;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), spawn_loading_screen)
            .add_systems(OnExit(GameState::Loading), despawn_loading_screen)
            .add_systems(
                Update,
                (update_progress, finish_when_world_is_built)
                    .chain()
                    .run_if(in_state(GameState::Loading)),
            );
    }
}

#[derive(Component)]
struct LoadingScreen;

#[derive(Component)]
struct ProgressFill;

#[derive(Component)]
struct ProgressLabel;

/// Minimum time the screen stays up.
///
/// Without it a fast machine flashes the screen for two frames, which reads as a
/// glitch rather than as loading.
const MINIMUM_SECONDS: f32 = 0.6;

#[derive(Resource)]
struct LoadingStarted(f32);

fn spawn_loading_screen(mut commands: Commands, time: Res<Time<Real>>) {
    // Real time, deliberately: the virtual clock can arrive here paused
    // (the title holds it), and a minimum measured on a stopped clock
    // never passes - the game once deadlocked on this exact stillness.
    commands.insert_resource(LoadingStarted(time.elapsed_secs()));

    commands
        .spawn((
            Name::new("Loading Screen"),
            LoadingScreen,
            // Interface, like the title: the hand stays a pointing finger
            // across the whole pre-game rather than flickering poses.
            crate::ui::Panel,
            Interaction::default(),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(18),
                ..default()
            },
            // The splash is the one screen with no world behind it, so it takes
            // the panel color at full opacity.
            BackgroundColor(theme::panel_bg().with_alpha(1.0)),
            // Above everything, including the debug HUD.
            GlobalZIndex(100),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("DIVUS FACTUS"),
                TextFont {
                    font_size: FontSize::Px(40.0),
                    ..default()
                },
                TextColor(theme::accent()),
            ));

            parent.spawn((
                ProgressLabel,
                Text::new("shaping the world"),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(theme::text_dim()),
            ));

            // Track.
            parent
                .spawn((
                    Node {
                        width: px(420),
                        height: px(6),
                        ..default()
                    },
                    BackgroundColor(palette::shade(&palette::STONE, 0.18)),
                ))
                .with_children(|track| {
                    track.spawn((
                        ProgressFill,
                        Node {
                            width: percent(0),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(theme::accent()),
                    ));
                });
        });
}

fn despawn_loading_screen(mut commands: Commands, screens: Query<Entity, With<LoadingScreen>>) {
    for entity in &screens {
        commands.entity(entity).despawn();
    }
}

fn update_progress(
    chunks: Option<Res<LoadedChunks>>,
    mut fill: Query<&mut Node, With<ProgressFill>>,
    mut label: Query<&mut Text, With<ProgressLabel>>,
) {
    let progress = chunks.map_or(0.0, |c| c.progress());

    for mut node in &mut fill {
        node.width = percent(progress * 100.0);
    }
    for mut text in &mut label {
        *text = Text::new(format!("shaping the world   {:.0}%", progress * 100.0));
    }
}

fn finish_when_world_is_built(
    time: Res<Time<Real>>,
    started: Option<Res<LoadingStarted>>,
    chunks: Option<Res<LoadedChunks>>,
    // A FOUNDED world, not a restoring one. `RestoringSeed` is a marker
    // the restore carries while it re-raises the fixtures and takes away
    // again the moment it is done - so by the time this ran it was always
    // gone, and every loaded world was handed back to the player as a
    // fresh one: standing on the title's flag, in a world already full of
    // people. Brett: "From the title screen you are still holding a flag."
    //
    // Whether the world has a settlement in it is the honest question, and
    // razing for a new world takes that away, so it cannot go stale.
    settled: Option<Res<crate::villager::SettlementSite>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let elapsed = started.map_or(0.0, |s| time.elapsed_secs() - s.0);
    if elapsed < MINIMUM_SECONDS {
        return;
    }

    if chunks.is_some_and(|c| c.is_complete()) {
        next.set(door_after_loading(settled.is_some()));
    }
}

/// Which door a finished load opens.
///
/// A world with a settlement in it is already founded, and the player is
/// put back into it. A world with none has never been planted, so it opens
/// on the founding screen with a flag in hand.
fn door_after_loading(settled: bool) -> GameState {
    if settled {
        GameState::Playing
    } else {
        GameState::Choosing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_reports_zero_before_anything_is_wanted() {
        // A view that has not been computed yet must not report as finished, or the
        // loading screen would vanish on frame one.
        let chunks = LoadedChunks::default();
        assert_eq!(chunks.progress(), 0.0);
        assert!(!chunks.is_complete());
    }

    /// A world with a settlement in it is a world already founded, and the
    /// player is put back in it rather than handed a flag.
    ///
    /// The fork used to read `RestoringSeed`, which the restore inserts to
    /// re-raise the fixtures and REMOVES the moment it is done - so it was
    /// always gone by the time this asked, and every loaded world opened on
    /// the founding screen with a flag in hand over a world already full of
    /// people. Brett: "From the title screen you are still holding a flag."
    #[test]
    fn a_founded_world_is_played_and_an_empty_one_is_founded() {
        assert_eq!(
            door_after_loading(true),
            GameState::Playing,
            "a world with a village in it is one to be played",
        );
        assert_eq!(
            door_after_loading(false),
            GameState::Choosing,
            "a world with none still asks where to plant the flag",
        );
    }
}
