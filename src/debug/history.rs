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

/// The history panel: everything that has ever happened, stamped.
#[derive(Component)]
pub(crate) struct HistoryPanel;

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
            "broken against",
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

// ---------------------------------------------------------------------------
// Importance: how loudly a happening rings, read from its own words.
// ---------------------------------------------------------------------------

/// The four bells of the chronicle, loudest first.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Importance {
    /// Foundings, the naming of the god, ascension: the page-turners.
    Majestic,
    /// Births, weddings, deaths — the hinges of lives.
    Important,
    /// Buildings raised, ground broken, trades taken up.
    Noteworthy,
    /// The daily hum.
    Common,
}

impl Importance {
    pub(crate) fn of(text: &str) -> Importance {
        let t = text.to_lowercase();
        let hit = |keys: &[&str]| keys.iter().any(|k| t.contains(k));
        if hit(&[
            "is founded",
            "name their god",
            "they name",
            "ascend",
            "the provider",
            "the stormhand",
        ]) {
            Importance::Majestic
        } else if hit(&[
            "born",
            "wed",
            "came of age",
            "died",
            "starved",
            "laid to rest",
            "perish",
            "mauled",
            "slew",
            "broken against",
            "widow",
            "famine",
        ]) {
            Importance::Important
        } else if hit(&[
            "raised",
            "foundation",
            "ground was broken",
            "broke ground",
            "took up",
            "set down",
            "harvest",
            "fire is lit",
            "came home",
            "discover",
        ]) {
            Importance::Noteworthy
        } else {
            Importance::Common
        }
    }

    pub(crate) fn colour(self) -> Color {
        match self {
            Importance::Majestic => palette::shade(&palette::CLOTH_GOLD, 1.0),
            Importance::Important => palette::shade(&palette::CLOTH_RED, 0.8),
            Importance::Noteworthy => palette::shade(&palette::BONE, 0.7),
            Importance::Common => ui::theme::text_dim().with_alpha(0.5),
        }
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Importance::Majestic => "majestic",
            Importance::Important => "important",
            Importance::Noteworthy => "noteworthy",
            Importance::Common => "common",
        }
    }
}

/// The hour word off a stamp like "spring 1, year 1 - morning".
fn hour_of(stamp: &str) -> &str {
    stamp.rsplit(" - ").next().unwrap_or("")
}

/// A day number's (year, season index) under the 28-day calendar.
fn year_season(day: u32) -> (u32, u8) {
    (day / 112 + 1, ((day / 28) % 4) as u8)
}

fn season_name(index: u8) -> &'static str {
    ["SPRING", "SUMMER", "AUTUMN", "WINTER"][index as usize % 4]
}

// ---------------------------------------------------------------------------
// The reader's hand: filters and stars.
// ---------------------------------------------------------------------------

/// What the reader has filtered the book down to.
#[derive(Resource, Default, Clone, PartialEq)]
pub(crate) struct ChronicleView {
    pub ledger: Option<Ledger>,
    /// None reads all time; Some((year, None)) one year; a season narrows it.
    pub year: Option<u32>,
    pub season: Option<u8>,
    pub importance: Option<Importance>,
}

/// Happenings the reader has starred. In memory for now; the stars are the
/// reader's bookmarks, not the world's record.
#[derive(Resource, Default)]
pub(crate) struct ChronicleStars(pub std::collections::HashSet<u64>);

fn star_key(day: u32, text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    day.hash(&mut hasher);
    text.hash(&mut hasher);
    hasher.finish()
}

/// A ledger tab across the chronicle's head. None is ALL EVENTS.
#[derive(Component)]
pub(crate) struct LedgerTabButton(Option<Ledger>);

/// A rail row narrowing time. (None, None) is all time.
#[derive(Component)]
pub(crate) struct TimeRow {
    year: Option<u32>,
    season: Option<u8>,
}

/// A rail row narrowing importance.
#[derive(Component)]
pub(crate) struct ImportanceRow(Importance);

#[derive(Component)]
pub(crate) struct ClearFiltersButton;

/// The rail's rebuilt-with-counts section.
#[derive(Component)]
pub(crate) struct ChronicleRail;

/// The scrolling page of happenings.
#[derive(Component)]
pub(crate) struct ChronicleRows;

/// The census strip along the page's foot.
#[derive(Component)]
pub(crate) struct ChronicleFooter;

/// A star at a row's end: the reader's bookmark.
#[derive(Component)]
pub(crate) struct StarButton(u64);

// ---------------------------------------------------------------------------
// The page.
// ---------------------------------------------------------------------------

pub(crate) fn spawn_chronicle_page(mut commands: Commands, codex: Res<super::village::Codex>) {
    let page = codex.chronicle_page;
    commands
        .entity(page)
        .insert((Name::new("Chronicle Page"), HistoryPanel));

    // The head: ALL EVENTS and the four shelves, as engraved tabs.
    let tabs = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(40),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                column_gap: px(6),
                align_items: AlignItems::Stretch,
                ..default()
            },
            ChildOf(page),
        ))
        .id();
    let mut shelf_tab = |label: &str, ledger: Option<Ledger>| {
        let button = commands
            .spawn((
                LedgerTabButton(ledger),
                ui::UiButton,
                ui::KeepFace,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    padding: UiRect::axes(px(14), px(8)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.18)),
                BorderColor::all(ui::theme::panel_border().with_alpha(0.4)),
                Interaction::default(),
                ChildOf(tabs),
            ))
            .id();
        if let Some(ledger) = ledger {
            spawn_glyph(&mut commands, button, ledger);
        }
        commands.spawn((
            Text::new(label),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(ui::theme::SMALL_SIZE),
                ..default()
            },
            TextColor(ui::theme::accent()),
            ChildOf(button),
        ));
    };
    shelf_tab("ALL EVENTS", None);
    shelf_tab("LIVES", Some(Ledger::Lives));
    shelf_tab("WORKS", Some(Ledger::Works));
    shelf_tab("FAITH", Some(Ledger::Faith));
    shelf_tab("THE WORLD", Some(Ledger::World));

    // The body: the filter rail and the book itself.
    let (rail, main) = ui::split_row(&mut commands, page, 240.0, ui::theme::PAD);
    commands.spawn((
        ChronicleRail,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
            ..default()
        },
        ChildOf(rail),
    ));
    let clear = commands
        .spawn((
            ClearFiltersButton,
            ui::UiButton,
            ui::KeepFace,
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(10), px(8)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                margin: UiRect::top(px(6)),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg()),
            BorderColor::all(ui::theme::panel_border().with_alpha(0.4)),
            Interaction::default(),
            ChildOf(rail),
        ))
        .id();
    commands.spawn((
        Text::new("CLEAR FILTERS"),
        ui::DisplayFace,
        TextFont {
            font_size: FontSize::Px(ui::theme::SMALL_SIZE),
            ..default()
        },
        TextColor(ui::theme::accent().with_alpha(0.8)),
        ChildOf(clear),
    ));

    // The book's column header, then the page of happenings.
    let header = commands
        .spawn((
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                padding: UiRect::axes(px(8), px(2)),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(ui::theme::text_dim().with_alpha(0.18)),
            ChildOf(main),
        ))
        .id();
    for (text, width) in [("HOUR", Some(104.0)), ("WHAT HAPPENED", None)] {
        let mut label = commands.spawn((
            Text::new(text),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(ui::theme::SMALL_SIZE),
                ..default()
            },
            TextColor(ui::theme::text_dim()),
            ChildOf(header),
        ));
        if let Some(width) = width {
            label.insert(Node {
                width: px(width),
                flex_shrink: 0.0,
                ..default()
            });
        }
    }
    commands.spawn((
        ChronicleRows,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ui::Scrollable,
        ScrollPosition::DEFAULT,
        Interaction::default(),
        ChildOf(main),
    ));

    // The foot: the census of the whole book.
    let footer = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(34),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(px(12), px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg().with_alpha(0.55)),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(main),
        ))
        .id();
    commands.spawn((ChronicleFooter, ui::dim(""), ChildOf(footer)));
}

/// Presses on the head tabs, the rail rows and the clear button reshape the
/// reader's view of the book.
#[allow(clippy::type_complexity)]
pub(crate) fn handle_chronicle_filters(
    mut view: ResMut<ChronicleView>,
    mut stars: ResMut<ChronicleStars>,
    tabs: Query<(&Interaction, &LedgerTabButton), Changed<Interaction>>,
    times: Query<(&Interaction, &TimeRow), Changed<Interaction>>,
    tiers: Query<(&Interaction, &ImportanceRow), Changed<Interaction>>,
    clears: Query<&Interaction, (Changed<Interaction>, With<ClearFiltersButton>)>,
    star_buttons: Query<(&Interaction, &StarButton), Changed<Interaction>>,
) {
    for (interaction, tab) in &tabs {
        if *interaction == Interaction::Pressed && view.ledger != tab.0 {
            view.ledger = tab.0;
        }
    }
    for (interaction, row) in &times {
        if *interaction == Interaction::Pressed {
            let fresh = (row.year, row.season);
            if (view.year, view.season) == fresh {
                continue;
            }
            view.year = row.year;
            view.season = row.season;
        }
    }
    for (interaction, row) in &tiers {
        if *interaction == Interaction::Pressed {
            view.importance = if view.importance == Some(row.0) {
                None
            } else {
                Some(row.0)
            };
        }
    }
    for interaction in &clears {
        if *interaction == Interaction::Pressed && *view != ChronicleView::default() {
            *view = ChronicleView::default();
        }
    }
    for (interaction, star) in &star_buttons {
        if *interaction == Interaction::Pressed {
            if !stars.0.remove(&star.0) {
                stars.0.insert(star.0);
            }
        }
    }
}

/// One folded happening, ready to lay on the page.
struct Row<'e> {
    event: &'e crate::villager::HistoryEvent,
    tally: u32,
}

/// Rebuilds the rail, the page and the footer whenever the book grows, the
/// view changes, or a star is placed - and only while the page is open.
#[allow(clippy::type_complexity)]
pub(crate) fn update_chronicle(
    mut commands: Commands,
    codex: Res<super::village::Codex>,
    view: Res<ChronicleView>,
    stars: Res<ChronicleStars>,
    history: Res<crate::villager::WorldChronicle>,
    panels: Query<&Visibility, With<HistoryPanel>>,
    rails: Query<Entity, With<ChronicleRail>>,
    wells: Query<Entity, With<ChronicleRows>>,
    footers: Query<Entity, With<ChronicleFooter>>,
    people: Query<&crate::villager::Person>,
    mut tab_faces: Query<(&LedgerTabButton, &mut BackgroundColor, &mut BorderColor)>,
    mut seen: Local<(usize, bool)>,
) {
    if codex.page != super::village::CodexPage::Chronicle
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        // Force a rebuild on the next opening.
        seen.1 = false;
        return;
    }
    let fresh =
        view.is_changed() || stars.is_changed() || history.events.len() != seen.0 || !seen.1;
    if !fresh {
        return;
    }
    *seen = (history.events.len(), true);
    let (Ok(rail), Ok(well), Ok(footer)) = (rails.single(), wells.single(), footers.single())
    else {
        return;
    };

    // The head tabs wear the open shelf.
    for (tab, mut fill, mut border) in &mut tab_faces {
        let open = tab.0 == view.ledger;
        fill.0 = if open {
            ui::theme::title_bg()
        } else {
            Color::BLACK.with_alpha(0.18)
        };
        *border = BorderColor::all(if open {
            ui::theme::accent().with_alpha(0.85)
        } else {
            ui::theme::panel_border().with_alpha(0.4)
        });
    }

    // Filter passes. Ledger and time narrow everything; importance last.
    let by_ledger: Vec<_> = history
        .events
        .iter()
        .filter(|e| view.ledger.is_none_or(|l| Ledger::of(&e.text) == l))
        .collect();
    let by_time: Vec<_> = by_ledger
        .iter()
        .copied()
        .filter(|e| {
            let (year, season) = year_season(e.day);
            view.year.is_none_or(|y| year == y) && view.season.is_none_or(|s| season == s)
        })
        .collect();
    let chosen: Vec<_> = by_time
        .iter()
        .copied()
        .filter(|e| view.importance.is_none_or(|i| Importance::of(&e.text) == i))
        .collect();

    // ---- The rail: time with counts, then importance with counts. ---------
    commands.entity(rail).despawn_related::<Children>();
    let heading = |commands: &mut Commands, parent, text: &str| {
        commands.spawn((
            Text::new(text),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(ui::theme::SMALL_SIZE),
                ..default()
            },
            TextColor(ui::theme::text_dim()),
            Node {
                margin: UiRect::top(px(6)).with_left(px(4)),
                ..default()
            },
            ChildOf(parent),
        ));
    };
    let rail_row = |commands: &mut Commands, parent, label: &str, count: usize, active: bool| {
        let row = commands
            .spawn((
                ui::UiButton,
                ui::KeepFace,
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(px(10), px(5)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    ..default()
                },
                BackgroundColor(if active {
                    ui::theme::title_bg()
                } else {
                    Color::BLACK.with_alpha(0.14)
                }),
                BorderColor::all(if active {
                    ui::theme::accent().with_alpha(0.85)
                } else {
                    ui::theme::panel_border().with_alpha(0.25)
                }),
                Interaction::default(),
                ChildOf(parent),
            ))
            .id();
        commands.spawn((ui::label(label), ChildOf(row)));
        commands.spawn((ui::dim(format!("{count}")), ChildOf(row)));
        row
    };

    heading(&mut commands, rail, "TIME");
    let all = rail_row(
        &mut commands,
        rail,
        "All Time",
        by_ledger.len(),
        view.year.is_none(),
    );
    commands.entity(all).insert(TimeRow {
        year: None,
        season: None,
    });
    let last_year = history.events.last().map_or(1, |e| year_season(e.day).0);
    for year in 1..=last_year {
        let count = by_ledger
            .iter()
            .filter(|e| year_season(e.day).0 == year)
            .count();
        let active = view.year == Some(year) && view.season.is_none();
        let row = rail_row(&mut commands, rail, &format!("Year {year}"), count, active);
        commands.entity(row).insert(TimeRow {
            year: Some(year),
            season: None,
        });
        // The chosen year unfolds its seasons.
        if view.year == Some(year) {
            for season in 0u8..4 {
                let count = by_ledger
                    .iter()
                    .filter(|e| year_season(e.day) == (year, season))
                    .count();
                let label = format!("  {}", season_name(season));
                let active = view.season == Some(season);
                let row = rail_row(&mut commands, rail, &label, count, active);
                commands.entity(row).insert(TimeRow {
                    year: Some(year),
                    season: Some(season),
                });
            }
        }
    }

    heading(&mut commands, rail, "IMPORTANCE");
    for tier in [
        Importance::Majestic,
        Importance::Important,
        Importance::Noteworthy,
        Importance::Common,
    ] {
        let count = by_time
            .iter()
            .filter(|e| Importance::of(&e.text) == tier)
            .count();
        let active = view.importance == Some(tier);
        let row = rail_row(&mut commands, rail, tier.word(), count, active);
        commands.entity(row).insert(ImportanceRow(tier));
        // The tier's bell, in its colour, ahead of the label.
        commands.entity(row).with_children(|row| {
            row.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(2),
                    top: px(11),
                    width: px(5),
                    height: px(5),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BackgroundColor(tier.colour()),
            ));
        });
    }

    // ---- The page: season bands, folded rows, newest first. ---------------
    commands.entity(well).despawn_related::<Children>();

    // Fold exact consecutive repeats into a tally.
    let mut rows: Vec<Row> = Vec::new();
    for event in chosen.iter().rev() {
        if let Some(last) = rows.last_mut()
            && last.event.text == event.text
            && year_season(last.event.day) == year_season(event.day)
        {
            last.tally += 1;
            continue;
        }
        rows.push(Row { event, tally: 1 });
    }

    const CAP: usize = 250;
    let shown = rows.len().min(CAP);
    let mut last_band: Option<(u32, u8)> = None;
    for (index, row) in rows.iter().take(CAP).enumerate() {
        let (year, season) = year_season(row.event.day);
        if last_band != Some((year, season)) {
            last_band = Some((year, season));
            let band = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(12),
                        padding: UiRect::axes(px(8), px(6)),
                        ..default()
                    },
                    ChildOf(well),
                ))
                .id();
            let rule = |commands: &mut Commands| {
                commands
                    .spawn((
                        Node {
                            flex_grow: 1.0,
                            height: px(1),
                            ..default()
                        },
                        BackgroundColor(ui::theme::accent().with_alpha(0.25)),
                    ))
                    .id()
            };
            let left = rule(&mut commands);
            commands.entity(left).insert(ChildOf(band));
            commands.spawn((
                Text::new(format!("{}, YEAR {}", season_name(season), year)),
                ui::DisplayFace,
                TextFont {
                    font_size: FontSize::Px(ui::theme::SMALL_SIZE + 1.0),
                    ..default()
                },
                TextColor(ui::theme::accent().with_alpha(0.9)),
                ChildOf(band),
            ));
            let right = rule(&mut commands);
            commands.entity(right).insert(ChildOf(band));
        }

        let importance = Importance::of(&row.event.text);
        let ledger = Ledger::of(&row.event.text);
        let line = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    padding: UiRect::axes(px(8), px(5)),
                    border_radius: BorderRadius::all(px(0)),
                    ..default()
                },
                BackgroundColor(if index % 2 == 0 {
                    Color::NONE
                } else {
                    Color::BLACK.with_alpha(0.14)
                }),
                ChildOf(well),
            ))
            .id();
        // The bell and the hour.
        let hour_cell = commands
            .spawn((
                Node {
                    width: px(104),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    ..default()
                },
                ChildOf(line),
            ))
            .id();
        commands.spawn((
            Node {
                width: px(7),
                height: px(7),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(px(7)),
                ..default()
            },
            BackgroundColor(importance.colour()),
            ChildOf(hour_cell),
        ));
        commands.spawn((ui::dim(hour_of(&row.event.stamp)), ChildOf(hour_cell)));
        spawn_glyph(&mut commands, line, ledger);
        // The happening itself, with its tally when folded.
        let text = if row.tally > 1 {
            format!("{}  x{}", row.event.text, row.tally)
        } else {
            row.event.text.clone()
        };
        let body_cell = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    ..default()
                },
                ChildOf(line),
            ))
            .id();
        commands.spawn((
            ui::body(text),
            Node {
                width: percent(100),
                ..default()
            },
            ChildOf(body_cell),
        ));
        // Whose story this is, when the words name someone living.
        let subjects: Vec<&str> = people
            .iter()
            .filter(|p| row.event.text.contains(&p.name))
            .map(|p| p.name.as_str())
            .take(2)
            .collect();
        commands.spawn((
            ui::dim(subjects.join(", ")),
            Node {
                width: px(130),
                flex_shrink: 0.0,
                ..default()
            },
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            ChildOf(line),
        ));
        // The reader's star.
        let key = star_key(row.event.day, &row.event.text);
        let starred = stars.0.contains(&key);
        let star = commands
            .spawn((
                StarButton(key),
                ui::UiButton,
                ui::KeepFace,
                Node {
                    width: px(20),
                    height: px(20),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Interaction::default(),
                ChildOf(line),
            ))
            .id();
        commands.spawn((
            Node {
                width: px(9),
                height: px(9),
                ..default()
            },
            UiTransform::from_rotation(Rot2::degrees(45.0)),
            BackgroundColor(if starred {
                ui::theme::accent()
            } else {
                ui::theme::text_dim().with_alpha(0.25)
            }),
            ChildOf(star),
        ));
    }
    if rows.len() > CAP {
        commands.spawn((
            ui::dim(format!("...and {} earlier happenings", rows.len() - CAP)),
            Node {
                margin: UiRect::all(px(8)),
                ..default()
            },
            ChildOf(well),
        ));
    }
    if rows.is_empty() {
        commands.spawn((
            ui::dim("nothing under this filter - the book waits"),
            Node {
                margin: UiRect::all(px(8)),
                ..default()
            },
            ChildOf(well),
        ));
    }

    // ---- The foot: the census of the whole book. --------------------------
    let counts = |ledger: Ledger| {
        history
            .events
            .iter()
            .filter(|e| Ledger::of(&e.text) == ledger)
            .count()
    };
    commands.entity(footer).despawn_related::<Children>();
    commands.spawn((
        ui::dim(format!("{} happenings in the book", history.events.len())),
        ChildOf(footer),
    ));
    let shelves = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(14),
                ..default()
            },
            ChildOf(footer),
        ))
        .id();
    for (ledger, name) in [
        (Ledger::Lives, "of lives"),
        (Ledger::Works, "of works"),
        (Ledger::Faith, "of faith"),
        (Ledger::World, "of the world"),
    ] {
        let pair = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(5),
                    ..default()
                },
                ChildOf(shelves),
            ))
            .id();
        spawn_glyph(&mut commands, pair, ledger);
        commands.spawn((
            ui::dim(format!("{} {}", counts(ledger), name)),
            ChildOf(pair),
        ));
    }
    let _ = shown;
}
