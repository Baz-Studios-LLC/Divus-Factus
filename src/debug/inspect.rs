//! The Inspect window: one frame, a different body for each thing you shift-
//! right-click.
//!
//! Brett: "I think buildings are going to need a shift right click panel that
//! opens up a window specific to that building, For example, the library when
//! shift right clicked will open the research panel"; then the generalization -
//! "Shift right click could be the 'Inspect' window and many things might get
//! it"; and then the shape - "I think the player window is a good blueprint for
//! the inspect windows, although maybe the specific inspect window could define
//! its own fixed size. For example the library inspect would have the research
//! panel which would need more width than the villager inspect panel."
//!
//! So the chrome is the villager profile's chrome - a panel pinned top-right,
//! a header, folder tabs, a scrolling well - and the WIDTH belongs to the
//! subject. A research tree is a wide thing; a person is a narrow one.
//!
//! The villager profile still runs its own copy of that chrome. This frame is
//! built so it can adopt this one, and it should - see `SIZES` for the one place
//! the two disagree today - but that window is somebody's careful work with
//! opinions attached, and rebuilding it is its own change, not a side effect of
//! wanting to see the research tree.

use bevy::prelude::*;

use crate::hand::DivineHand;
use crate::ui::{self, PointerContext};

/// Local, because `ui`'s own is private and every panel in `debug` keeps one.
fn px(v: f32) -> Val {
    Val::Px(v)
}
use crate::villager::study::{Phase, Studies, THE_TREE, frontier, node};
use crate::villager::work::{Building, BuildingKind};

/// What the window is looking at.
///
/// An enum and not a bare `Entity`, because the whole point of Inspect is that
/// the body differs by subject. Adding a subject is a variant, a size, and a
/// `fill` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Subject {
    /// A library, and the town whose books it holds.
    Library { town: Entity },
}

impl Subject {
    /// How wide this subject's window stands, and what it is called.
    ///
    /// THE SIZE BELONGS TO THE SUBJECT. Brett asked for exactly this: the
    /// research panel needs room the villager panel does not, and a window sized
    /// for the narrowest body makes every wider one a compromise.
    fn shape(self) -> (f32, &'static str) {
        match self {
            // Wide, because a frontier of open questions is a list of names with
            // their costs beside them, and folding those is what makes a
            // research panel unreadable.
            Subject::Library { .. } => (760.0, "The library"),
        }
    }
}

/// Whether the window is up, and on what.
#[derive(Resource, Default)]
pub(crate) struct Inspect {
    pub open: bool,
    pub subject: Option<Subject>,
}

/// The window's outer frame.
#[derive(Component)]
pub(crate) struct InspectRoot;

/// The body, emptied and refilled whenever the subject changes.
#[derive(Component)]
pub(crate) struct InspectBody;

/// What the body was last filled for, so it is rebuilt when that changes and
/// not every frame.
#[derive(Component)]
pub(crate) struct InspectShowing(Option<Subject>);

pub(crate) fn spawn_inspect(mut commands: Commands) {
    let root = commands
        .spawn((
            InspectRoot,
            InspectShowing(None),
            ui::Panel,
            Node {
                position_type: PositionType::Absolute,
                right: px(16.0),
                top: px(16.0),
                // Set per subject when it opens; this is only the value it
                // holds while hidden.
                width: px(760.0),
                height: Val::Vh(74.0),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                padding: px(16.0).into(),
                row_gap: px(12.0),
                border: UiRect::all(px(1.5)),
                ..default()
            },
            BackgroundColor(ui::theme::panel_bg()),
            BorderColor::all(ui::theme::panel_border()),
            // Over the world and the hover card, under the codex.
            GlobalZIndex(40),
            Interaction::default(),
            Visibility::Hidden,
        ))
        .id();
    commands.spawn((
        InspectBody,
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: px(10.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ui::Scrollable,
        ChildOf(root),
    ));
}

/// Shift + right-click on a thing that has a window opens it.
///
/// The villager profile owns the same gesture for PEOPLE and runs first; this
/// picks up what it does not want. Both read `hand.hovered`, so there is one
/// answer to "what is under the cursor" and no chance of the two disagreeing.
pub(crate) fn open_on_shift_right_click(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    pointer: Res<PointerContext>,
    hand: Res<DivineHand>,
    buildings: Query<(&Building, &crate::villager::MemberOf)>,
    mut inspect: ResMut<Inspect>,
    mut press_at: Local<Option<Vec2>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    if !(keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)) {
        press_at.take();
        return;
    }
    if buttons.just_pressed(MouseButton::Right) {
        *press_at = window.cursor_position();
    }
    if !buttons.just_released(MouseButton::Right) {
        return;
    }
    // A press and a release in the same place: a click, not a drag of the world.
    let (Some(pressed), Some(released)) = (press_at.take(), window.cursor_position()) else {
        return;
    };
    if pointer.over_ui || pressed.distance(released) > 6.0 {
        return;
    }
    let Some(hovered) = hand.hovered else {
        return;
    };
    let Ok((building, member)) = buildings.get(hovered) else {
        return;
    };
    let subject = match building.kind {
        BuildingKind::Library => Subject::Library { town: member.0 },
        // Every other building will get a body. Until it has one, the click does
        // nothing rather than opening an empty window.
        _ => return,
    };
    inspect.subject = Some(subject);
    inspect.open = true;
}

/// Escape closes it, like every other window.
pub(crate) fn close_inspect(keys: Res<ButtonInput<KeyCode>>, mut inspect: ResMut<Inspect>) {
    if inspect.open && keys.just_pressed(KeyCode::Escape) {
        inspect.open = false;
    }
}

/// Shows, sizes and fills the window.
///
/// Rebuilt only when the SUBJECT changes; the numbers inside are refreshed every
/// tick by `refresh_inspect`. Rebuilding the whole body to move a progress bar
/// would throw away the player's scroll position several times a second.
pub(crate) fn dress_inspect(
    mut commands: Commands,
    inspect: Res<Inspect>,
    mut roots: Query<(&mut Visibility, &mut Node, &mut InspectShowing), With<InspectRoot>>,
    body: Query<Entity, With<InspectBody>>,
    towns: Query<(&crate::villager::Settlement, &Studies)>,
) {
    let Ok((mut visible, mut node, mut showing)) = roots.single_mut() else {
        return;
    };
    let wanted = if inspect.open { inspect.subject } else { None };
    *visible = if wanted.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if showing.0 == wanted {
        return;
    }
    showing.0 = wanted;
    let Ok(body) = body.single() else {
        return;
    };
    commands.entity(body).despawn_related::<Children>();
    let Some(subject) = wanted else {
        return;
    };
    let (width, title) = subject.shape();
    node.width = px(width);

    match subject {
        Subject::Library { town } => {
            let Ok((settlement, studies)) = towns.get(town) else {
                return;
            };
            fill_the_library(&mut commands, body, title, &settlement.name, studies);
        }
    }
}

/// The research panel.
///
/// Three things, in the order a reader wants them: what the town is working on
/// and how far in, what it is waiting for and WHERE, and what else is open to it.
fn fill_the_library(
    commands: &mut Commands,
    body: Entity,
    title: &str,
    town: &str,
    studies: &Studies,
) {
    commands.spawn((ui::heading(format!("{title} of {town}")), ChildOf(body)));

    // --- What is on the table ---
    ui::section_header(commands, body, "on the table");
    match studies.at_hand.as_deref().and_then(node) {
        Some(reading) => {
            commands.spawn((ui::heading(reading.name), ChildOf(body)));
            commands.spawn((ui::label(reading.blurb), ChildOf(body)));
            // TWO BARS, because a node has two halves and a journey between
            // them. One bar would have to lie about the middle.
            // The bars' VALUES come from `refresh_inspect` every tick; building
            // them only has to decide which is which.
            let reading_bar = ui::gauge_row(commands, body, "the reading", ui::theme::accent());
            commands
                .entity(reading_bar.fill)
                .insert(InspectGauge(Readout::TheoryShare));
            commands
                .entity(reading_bar.value)
                .insert(InspectReadout(Readout::TheoryShare));
            let work_bar = ui::gauge_row(commands, body, "the work", ui::theme::accent());
            commands
                .entity(work_bar.fill)
                .insert(InspectGauge(Readout::PracticeShare));
            commands
                .entity(work_bar.value)
                .insert(InspectReadout(Readout::PracticeShare));

            // --- What it is waiting for ---
            if let Some(need) = studies.wanting {
                ui::section_header(commands, body, "waiting on");
                commands.spawn((
                    ui::heading(format!("{:.0} {}", need.short, need.sample.word())),
                    ChildOf(body),
                ));
                commands.spawn((
                    ui::label(match need.toward {
                        Some(_) => "The scholars know where the nearest is.",
                        None => "Nobody knows where to find any. Somebody has to go looking.",
                    }),
                    ChildOf(body),
                ));
            }
        }
        None => {
            commands.spawn((
                ui::label(if studies.known.len() >= THE_TREE.len() {
                    "Everything there is to know is known here."
                } else {
                    "Nobody is reading. The library wants a scholar."
                }),
                ChildOf(body),
            ));
        }
    }

    // --- What else is open ---
    ui::section_header(commands, body, "open to them");
    let open = frontier(&studies.known);
    if open.is_empty() {
        commands.spawn((ui::label("Nothing further, for now."), ChildOf(body)));
    }
    for next in open.iter().take(8) {
        let row = ui::ruled_row(commands, body, next.name);
        commands.spawn((
            ui::dim(match next.sample {
                Some((sample, wanted)) => {
                    format!("wants {:.0} {}", wanted, sample.word())
                }
                None => "read at home".to_string(),
            }),
            ChildOf(row),
        ));
    }

    // --- What is already known ---
    ui::section_header(commands, body, "worked out");
    if studies.known.is_empty() {
        commands.spawn((ui::label("Nothing yet."), ChildOf(body)));
    }
    let learned: Vec<&str> = studies
        .known
        .iter()
        .filter_map(|key| node(key).map(|had| had.name))
        .collect();
    if !learned.is_empty() {
        commands.spawn((ui::label(learned.join(", ")), ChildOf(body)));
    }
    commands.spawn((
        ui::dim(format!(
            "{} of {} on the tree",
            learned.len(),
            THE_TREE.len()
        )),
        ChildOf(body),
    ));
}

/// A bar this window keeps up to date, and WHICH of the two it is.
///
/// The role is decided once, when the bar is built, and carried on the entity -
/// so `refresh_inspect` never has to guess which gauge it is looking at from its
/// current value. A first cut stored the share instead and could not tell the two
/// apart at all.
#[derive(Component)]
pub(crate) struct InspectGauge(Readout);

/// Which number a text node is showing.
#[derive(Component)]
pub(crate) struct InspectReadout(Readout);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Readout {
    TheoryShare,
    PracticeShare,
}

/// The numbers, every tick. The body itself is only rebuilt when the subject
/// changes, so this is what makes the bars move.
pub(crate) fn refresh_inspect(
    inspect: Res<Inspect>,
    towns: Query<&Studies>,
    mut gauges: Query<(&mut Node, &InspectGauge)>,
    mut readouts: Query<(&mut Text, &InspectReadout)>,
) {
    let Some(Subject::Library { town }) = inspect.subject.filter(|_| inspect.open) else {
        return;
    };
    let Ok(studies) = towns.get(town) else {
        return;
    };
    let phase = studies.phase();
    let (theory, practice) = match phase {
        Some(Phase::Theory) => (studies.share(), 0.0),
        // Read out and waiting: the reading really is finished, and the work
        // genuinely has not started. A single bar could not say both.
        Some(Phase::Wanting) => (1.0, 0.0),
        Some(Phase::Practice) => (1.0, studies.share()),
        None => (0.0, 0.0),
    };
    for (mut node, gauge) in &mut gauges {
        let share = match gauge.0 {
            Readout::TheoryShare => theory,
            Readout::PracticeShare => practice,
        };
        node.width = Val::Percent(share * 100.0);
    }
    for (mut text, readout) in &mut readouts {
        let share = match readout.0 {
            Readout::TheoryShare => theory,
            Readout::PracticeShare => practice,
        };
        let fresh = format!("{:.0}%", share * 100.0);
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}
