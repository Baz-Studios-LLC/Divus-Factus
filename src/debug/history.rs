//! THE CHRONICLE window: everything that has ever happened in this world,
//! set like a book worth reading.
//!
//! The full record, not a tail — every notice since founding, newest first.
//! Days are bands, not repeated text; each happening carries the glyph and
//! colour of its shelf (lives, works, faith, the world); a run of identical
//! events folds into one line with a tally, so a stormy night reads as
//! "lightning strikes from the storm ×7" instead of seven lines of the
//! same sentence. This panel is the design language the other windows
//! will grow into: form first.

use crate::palette;
use crate::ui;
use bevy::prelude::*;

/// The toolbar button that opens the history.
#[derive(Component)]
pub(crate) struct HistoryButton;

/// The history panel: everything that has ever happened, stamped.
#[derive(Component)]
pub(crate) struct HistoryPanel;

/// One tab's scrolling well, with the shelf it shows.
#[derive(Component)]
pub(crate) struct HistoryWell(Option<Ledger>);

/// The census strip along the panel's foot.
#[derive(Component)]
pub(crate) struct HistoryFooter;

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
    pub(crate) fn of(text: &str) -> Ledger {
        let t = text.to_lowercase();
        let hit = |keys: &[&str]| keys.iter().any(|k| t.contains(k));
        if hit(&[
            "believe",
            "blessed",
            "smote",
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
            "famine",
            "picked bare",
            "larder",
        ]) {
            Ledger::Lives
        } else if hit(&[
            "ground was broken",
            "raised",
            "took up",
            "set down their tools",
            "harvest",
            "tilled",
            "craft",
        ]) {
            Ledger::Works
        } else {
            Ledger::World
        }
    }

    /// The shelf's colour: its accent bar, its glyph, its tally pill.
    pub(crate) fn colour(self) -> Color {
        match self {
            Ledger::Lives => palette::shade(&palette::CLOTH_RED, 0.72),
            Ledger::Works => palette::shade(&palette::WOOD, 0.72),
            Ledger::Faith => palette::shade(&palette::CLOTH_GOLD, 0.85),
            Ledger::World => palette::shade(&palette::GRASS, 0.62),
        }
    }
}

/// Draws a shelf's glyph into a 16×16 canvas, from plain nodes in the same
/// hand-set vocabulary as the toolbar icons.
pub(crate) fn spawn_glyph(commands: &mut Commands, parent: Entity, ledger: Ledger) {
    let canvas = commands
        .spawn((
            Node {
                width: px(16),
                height: px(16),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let ink = ledger.colour();
    match ledger {
        // A person: head above shoulders.
        Ledger::Lives => {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(5),
                    top: px(1),
                    width: px(6),
                    height: px(6),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(ink),
                ChildOf(canvas),
            ));
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(3),
                    top: px(9),
                    width: px(10),
                    height: px(6),
                    border_radius: BorderRadius::top(px(5)),
                    ..default()
                },
                BackgroundColor(ink),
                ChildOf(canvas),
            ));
        }
        // A house: walls under a turned-square roof.
        Ledger::Works => {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(4),
                    top: px(1),
                    width: px(8),
                    height: px(8),
                    ..default()
                },
                UiTransform::from_rotation(Rot2::degrees(45.0)),
                BackgroundColor(ink),
                ChildOf(canvas),
            ));
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(4),
                    top: px(8),
                    width: px(8),
                    height: px(7),
                    ..default()
                },
                BackgroundColor(ink),
                ChildOf(canvas),
            ));
        }
        // A spark of the divine: a turned square with a bright heart.
        Ledger::Faith => {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(3),
                    top: px(3),
                    width: px(10),
                    height: px(10),
                    ..default()
                },
                UiTransform::from_rotation(Rot2::degrees(45.0)),
                BackgroundColor(ink.with_alpha(0.55)),
                ChildOf(canvas),
            ));
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(6),
                    top: px(6),
                    width: px(4),
                    height: px(4),
                    ..default()
                },
                UiTransform::from_rotation(Rot2::degrees(45.0)),
                BackgroundColor(palette::shade(&palette::BONE, 0.98)),
                ChildOf(canvas),
            ));
        }
        // A tree: trunk under canopy — the world about its business.
        Ledger::World => {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(7),
                    top: px(9),
                    width: px(3),
                    height: px(6),
                    ..default()
                },
                BackgroundColor(palette::shade(&palette::WOOD, 0.45)),
                ChildOf(canvas),
            ));
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(3),
                    top: px(1),
                    width: px(10),
                    height: px(9),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(ink),
                ChildOf(canvas),
            ));
        }
    }
}

pub(crate) fn spawn_history_panel(mut commands: Commands) {
    let window = ui::big_window(&mut commands, "THE CHRONICLE", 720.0);
    commands.entity(window.root).insert((
        Name::new("History Panel"),
        HistoryPanel,
        Visibility::Hidden,
        // An explicit width, or the percent-sized pages inside resolve
        // against the whole screen and drag the frame to the edges.
        Node {
            width: Val::Vw(58.0),
            min_width: px(760),
            flex_direction: FlexDirection::Column,
            padding: px(5).into(),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(12)),
            ..default()
        },
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
        // The column headline, set quiet above the record.
        let head = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(10),
                    padding: UiRect::axes(px(16), px(6)),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(ui::theme::text_dim().with_alpha(0.25)),
                ChildOf(page),
            ))
            .id();
        commands.spawn((
            ui::dim(""),
            Node {
                width: px(29),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(head),
        ));
        commands.spawn((
            ui::dim("HOUR"),
            Node {
                width: px(124),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(head),
        ));
        commands.spawn((
            ui::dim("WHAT HAPPENED"),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ChildOf(head),
        ));

        // The record itself: a deep well, most of the screen tall.
        commands.spawn((
            HistoryWell(filter),
            Node {
                width: percent(100),
                height: Val::Vh(60.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                padding: UiRect::axes(px(8), px(8)),
                row_gap: px(2),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.35)),
            ui::Scrollable,
            ChildOf(page),
        ));
    }
    // The census along the foot: how much life the book holds.
    commands.spawn((
        HistoryFooter,
        ui::dim(""),
        Node {
            width: percent(100),
            padding: UiRect::axes(px(16), px(7)),
            border: UiRect::top(px(1)),
            ..default()
        },
        BorderColor::all(ui::theme::text_dim().with_alpha(0.25)),
        ChildOf(window.body),
    ));
}

/// One line of the book, after folding: the shelf, the hour, the words,
/// and how many times in a row they were true.
struct Entry<'a> {
    day: &'a str,
    hour: &'a str,
    text: &'a str,
    ledger: Ledger,
    count: usize,
}

/// Rebuilds every shelf of the chronicle when new history has happened.
/// Rows are real nodes — bands, glyphs, stripes — not one long string.
pub(crate) fn update_history_panel(
    mut commands: Commands,
    history: Option<Res<crate::villager::WorldChronicle>>,
    panels: Query<&Visibility, With<HistoryPanel>>,
    wells: Query<(Entity, &HistoryWell)>,
    mut footers: Query<&mut Text, With<HistoryFooter>>,
    mut seen: Local<usize>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    let Some(history) = history else {
        return;
    };
    if history.events.len() == *seen && *seen != 0 {
        return;
    }
    *seen = history.events.len().max(1);

    // Fold the record once: newest first, consecutive repeats within a
    // day gathered under one tally.
    let mut entries: Vec<Entry> = Vec::new();
    for event in history.events.iter().rev() {
        let (day, hour) = event
            .stamp
            .split_once(" - ")
            .unwrap_or((event.stamp.as_str(), ""));
        if let Some(last) = entries.last_mut()
            && last.day == day
            && last.text == event.text
        {
            last.count += 1;
            continue;
        }
        entries.push(Entry {
            day,
            hour,
            text: &event.text,
            ledger: Ledger::of(&event.text),
            count: 1,
        });
    }

    let mut tallies = [0usize; 4];
    for event in history.events.iter() {
        tallies[match Ledger::of(&event.text) {
            Ledger::Lives => 0,
            Ledger::Works => 1,
            Ledger::Faith => 2,
            Ledger::World => 3,
        }] += 1;
    }

    for (well, shelf) in &wells {
        commands.entity(well).despawn_related::<Children>();
        let mut current_day = "";
        let mut stripe = false;
        let mut shown = 0;
        let mine = entries
            .iter()
            .filter(|e| shelf.0.is_none_or(|wanted| e.ledger == wanted))
            .count();
        for entry in &entries {
            if shelf.0.is_some_and(|wanted| entry.ledger != wanted) {
                continue;
            }
            if shown >= 250 {
                commands.spawn((
                    ui::dim(format!(
                        "... and {} earlier happenings held in the book",
                        mine - shown
                    )),
                    Node {
                        padding: UiRect::axes(px(16), px(10)),
                        ..default()
                    },
                    ChildOf(well),
                ));
                break;
            }
            // A new day opens as a band, so dates read once, not on
            // every line.
            if entry.day != current_day {
                current_day = entry.day;
                stripe = false;
                let band = commands
                    .spawn((
                        Node {
                            width: percent(100),
                            padding: UiRect::axes(px(12), px(5)),
                            margin: UiRect::top(px(7)),
                            border_radius: BorderRadius::all(px(5)),
                            border: UiRect::left(px(3)),
                            ..default()
                        },
                        BorderColor::all(ui::theme::accent().with_alpha(0.8)),
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.85)),
                        ChildOf(well),
                    ))
                    .id();
                commands.spawn((
                    Text::new(entry.day.to_uppercase()),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(ui::theme::accent()),
                    ChildOf(band),
                ));
            }
            // The happening itself.
            let row = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        padding: UiRect::axes(px(8), px(5)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(if stripe {
                        Color::WHITE.with_alpha(0.028)
                    } else {
                        Color::NONE
                    }),
                    ChildOf(well),
                ))
                .id();
            stripe = !stripe;
            commands.spawn((
                Node {
                    width: px(3),
                    height: px(17),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(px(2)),
                    ..default()
                },
                BackgroundColor(entry.ledger.colour().with_alpha(0.9)),
                ChildOf(row),
            ));
            spawn_glyph(&mut commands, row, entry.ledger);
            commands.spawn((
                ui::dim(entry.hour.to_string()),
                Node {
                    width: px(124),
                    flex_shrink: 0.0,
                    ..default()
                },
                ChildOf(row),
            ));
            let words = commands
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: px(0),
                        ..default()
                    },
                    ChildOf(row),
                ))
                .id();
            commands.spawn((
                ui::body(entry.text.clone_words()),
                Node {
                    width: percent(100),
                    ..default()
                },
                ChildOf(words),
            ));
            if entry.count > 1 {
                let pill = commands
                    .spawn((
                        Node {
                            padding: UiRect::axes(px(8), px(2)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(999)),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        BackgroundColor(entry.ledger.colour().with_alpha(0.16)),
                        BorderColor::all(entry.ledger.colour().with_alpha(0.6)),
                        ChildOf(row),
                    ))
                    .id();
                commands.spawn((
                    Text::new(format!("x{}", entry.count)),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(entry.ledger.colour()),
                    ChildOf(pill),
                ));
            }
            shown += 1;
        }
        if shown == 0 {
            commands.spawn((
                ui::dim("nothing yet"),
                Node {
                    padding: UiRect::axes(px(16), px(10)),
                    ..default()
                },
                ChildOf(well),
            ));
        }
    }

    for mut footer in &mut footers {
        *footer = Text::new(format!(
            "{} happenings in the book  -  {} of lives, {} of works, {} of faith, {} of the world",
            history.events.len(),
            tallies[0],
            tallies[1],
            tallies[2],
            tallies[3],
        ));
    }
}

/// A tiny seam so `Entry` can hand its borrowed words to an owned `Text`.
trait CloneWords {
    fn clone_words(&self) -> String;
}

impl CloneWords for &str {
    fn clone_words(&self) -> String {
        (*self).to_string()
    }
}
