//! THE CHRONICLE window: everything that has ever happened in this world.
//!
//! The full book of record, not a tail — every notice the game has raised
//! since founding, newest first, sorted into tabs so a reign can be read
//! by its lives, its works, its faith, or the world's own turnings. The
//! events carry no category; they are classified from their own words at
//! display time, which costs nothing to save and works on every save ever
//! written.

use crate::ui;
use bevy::prelude::*;

/// The toolbar button that opens the history.
#[derive(Component)]
pub(crate) struct HistoryButton;

/// The history panel: everything that has ever happened, stamped.
#[derive(Component)]
pub(crate) struct HistoryPanel;

/// One tab's text block, with the filter it shows.
#[derive(Component)]
pub(crate) struct HistoryText(Option<Ledger>);

/// The chronicle's shelves.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ledger {
    /// Births, weddings, comings of age, deaths.
    Lives,
    /// Ground broken, buildings raised, tools taken up.
    Works,
    /// Miracles, conversions, the naming of the god.
    Faith,
    /// Discoveries, seasons, wolves — the world doing its own things.
    World,
}

impl Ledger {
    /// Reads an event's shelf from its own words.
    fn of(text: &str) -> Ledger {
        let t = text.to_lowercase();
        let hit = |keys: &[&str]| keys.iter().any(|k| t.contains(k));
        if hit(&[
            "believe",
            "blessed",
            "smote",
            "lightning",
            "flourish",
            "shrine",
            "miracle",
            "name their god",
            "they name",
            "bounty",
            "mended",
        ]) {
            Ledger::Faith
        } else if hit(&[
            "born",
            "wed",
            "came of age",
            "starved",
            "laid to rest",
            "perish",
            "died",
            "mauled",
            "slew",
        ]) {
            Ledger::Lives
        } else if hit(&[
            "ground was broken",
            "raised",
            "took up",
            "set down their tools",
        ]) {
            Ledger::Works
        } else {
            Ledger::World
        }
    }
}

pub(crate) fn spawn_history_panel(mut commands: Commands) {
    let window = ui::window(&mut commands, "THE CHRONICLE", 440.0);
    commands.entity(window.root).insert((
        Name::new("History Panel"),
        HistoryPanel,
        Visibility::Hidden,
    ));
    let pages = ui::tab_bar(
        &mut commands,
        window.body,
        &["ALL", "LIVES", "WORKS", "FAITH", "THE WORLD"],
    );
    let filters = [
        None,
        Some(Ledger::Lives),
        Some(Ledger::Works),
        Some(Ledger::Faith),
        Some(Ledger::World),
    ];
    for (page, filter) in pages.into_iter().zip(filters) {
        // Each page is its own scrolling well of record.
        let well = commands
            .spawn((
                Node {
                    width: percent(100),
                    max_height: px(380),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                    padding: UiRect::all(px(8)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.35)),
                ui::Scrollable,
                ChildOf(page),
            ))
            .id();
        let text = commands
            .spawn((
                HistoryText(filter),
                ui::dim(""),
                Node {
                    max_width: px(400),
                    ..default()
                },
            ))
            .id();
        commands.entity(text).insert(ChildOf(well));
    }
}

/// Fills every shelf of the chronicle, newest first, in full.
pub(crate) fn update_history_panel(
    history: Option<Res<crate::villager::WorldChronicle>>,
    panels: Query<&Visibility, With<HistoryPanel>>,
    mut texts: Query<(&HistoryText, &mut Text)>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    let Some(history) = history else {
        return;
    };
    for (shelf, mut text) in &mut texts {
        let mut lines: Vec<String> = history
            .events
            .iter()
            .rev()
            .filter(|event| {
                shelf
                    .0
                    .is_none_or(|ledger| Ledger::of(&event.text) == ledger)
            })
            .map(|event| format!("{}  {}", event.stamp, event.text))
            .collect();
        if lines.is_empty() {
            lines.push("nothing yet".into());
        }
        let fresh = lines.join("\n");
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}
