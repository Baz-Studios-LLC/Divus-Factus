//! The god's grip on time: pause, and one-to-eight-fold haste.
//!
//! A year is a hundred and twelve ten-minute days; nobody should have to
//! leave the machine running overnight to see spring again. The whole
//! simulation already runs on virtual time, so speed is one dial —
//! `Time<Virtual>`'s relative speed — and pausing is the same door the
//! pause menu already uses. The dial rules everywhere but under the pause
//! menu: the world lives behind the splash and the title, drifting past
//! under the lettering, so even the pre-game keeps the clock beating.

use bevy::prelude::*;

use crate::ui;

/// The speeds the buttons offer, in order.
const STEPS: [f32; 4] = [1.0, 2.0, 4.0, 8.0];

pub struct SpeedPlugin;

impl Plugin for SpeedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimSpeed>()
            .add_systems(Startup, spawn_speed_strip)
            .add_systems(Update, (command_time, apply_speed, style_speed_strip));
    }
}

/// What the player has asked of time. Applied every frame while playing.
#[derive(Resource)]
pub struct SimSpeed {
    pub paused: bool,
    pub speed: f32,
}

impl Default for SimSpeed {
    /// EGREGORE_SPEED starts a run hasted — the fast-forward dial that
    /// makes hour-scale balance testable in minutes.
    fn default() -> Self {
        let speed = std::env::var("EGREGORE_SPEED")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(1.0)
            .clamp(0.25, 16.0);
        SimSpeed {
            paused: false,
            speed,
        }
    }
}

/// A button on the strip: a speed, or None for the pause toggle.
#[derive(Component)]
struct SpeedButton(Option<f32>);

fn spawn_speed_strip(mut commands: Commands) {
    // The corner margin lives as padding on an invisible apron, so the
    // approach to the strip - sides, top, and the gap to the screen's very
    // corner - is already interface and the hand holds its pointing pose.
    let apron = commands
        .spawn((
            Name::new("Time Controls"),
            ui::Panel,
            ui::GameHud,
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                bottom: px(0),
                padding: UiRect::new(px(12), px(14), px(12), px(12)),
                ..default()
            },
        ))
        .id();
    let strip = commands
        .spawn((
            ui::Panel,
            Interaction::default(),
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(4),
                padding: px(4).into(),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(ui::theme::panel_bg()),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(apron),
        ))
        .id();
    for (speed, label) in [
        (None, "II"),
        (Some(1.0), "1x"),
        (Some(2.0), "2x"),
        (Some(4.0), "4x"),
        (Some(8.0), "8x"),
    ] {
        let button = commands
            .spawn((
                SpeedButton(speed),
                ui::UiButton,
                Node {
                    width: px(30),
                    height: px(24),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BackgroundColor(ui::theme::panel_bg().with_alpha(0.4)),
                BorderColor::all(ui::theme::panel_border()),
                Interaction::default(),
                ChildOf(strip),
            ))
            .id();
        commands.spawn((ui::dim(label), ChildOf(button)));
    }
}

/// Space pauses; comma and period walk the speed steps; the buttons do
/// both. Picking a speed while paused also resumes — reaching for haste
/// means wanting the world to move.
fn command_time(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &SpeedButton), Changed<Interaction>>,
    mut sim: ResMut<SimSpeed>,
) {
    if keys.just_pressed(KeyCode::Space) {
        sim.paused = !sim.paused;
    }
    let step = STEPS
        .iter()
        .position(|s| (*s - sim.speed).abs() < 0.01)
        .unwrap_or(0);
    if keys.just_pressed(KeyCode::Comma) && step > 0 {
        sim.speed = STEPS[step - 1];
    }
    if keys.just_pressed(KeyCode::Period) && step + 1 < STEPS.len() {
        sim.speed = STEPS[step + 1];
    }
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button.0 {
            None => sim.paused = !sim.paused,
            Some(speed) => {
                sim.speed = speed;
                sim.paused = false;
            }
        }
    }
}

/// Applies the player's wish to the actual clock — but only while the
/// world is being played. The title screen and the pause menu hold time
/// by their own authority, and this system stays out of their way.
fn apply_speed(
    menus: Query<&Visibility, With<crate::title::PauseMenu>>,
    sim: Res<SimSpeed>,
    mut time: ResMut<Time<Virtual>>,
) {
    // The world lives behind the splash and the title now, so the dial is
    // enforced across the whole pre-game: if anything leaves the virtual
    // clock paused on the way in, this is what starts it beating again.
    // Only the pause menu retains authority over the clock.
    if menus.iter().any(|v| *v == Visibility::Visible) {
        return;
    }
    time.set_relative_speed(sim.speed);
    if sim.paused && !time.is_paused() {
        time.pause();
    } else if !sim.paused && time.is_paused() {
        time.unpause();
    }
}

/// The active choice wears the accent; everything else stays quiet.
fn style_speed_strip(sim: Res<SimSpeed>, mut buttons: Query<(&SpeedButton, &mut BorderColor)>) {
    if !sim.is_changed() {
        return;
    }
    for (button, mut border) in &mut buttons {
        let active = match button.0 {
            None => sim.paused,
            Some(speed) => !sim.paused && (speed - sim.speed).abs() < 0.01,
        };
        *border = BorderColor::all(if active {
            ui::theme::accent()
        } else {
            ui::theme::panel_border()
        });
    }
}
