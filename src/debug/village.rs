//! THE CODEX — the one grand window — and its first resident page: THE
//! LEDGER, the village's whole account at a glance.
//!
//! The codex is the People window's footprint (1160 wide, the same band
//! heights) wearing an icon-tab strip in its title bar: house, wood, temple,
//! faith, people. Today only the house page lives here and the people icon
//! opens the People window where it still stands; each remaining panel
//! migrates in as it is brought up to the People standard, one by one.

use std::collections::BTreeMap;

use crate::keymap::Deed;
use crate::ui;
use crate::villager::Activity;
use crate::villager::Chronicle;
use crate::villager::Morale;
use crate::villager::Needs;
use crate::villager::Person;
use crate::villager::Villager;
use crate::witness::Witnessed;
use bevy::prelude::*;

/// The codex window. Keeps the historical component name so the toolbar
/// wiring and capture tooling stay untouched.
#[derive(Component)]
pub(crate) struct VillagePanel;

/// One of the three big numbers at the top: souls, houses, believers.
#[derive(Component)]
pub(crate) struct VillageCard(u8);

/// A dashboard statistic, drawn as a bar.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VillageStat {
    Happiness,
    Housed,
    Fed,
    Faith,
    Believers,
    Food,
    Timber,
    Stone,
}

/// The fill of a ledger gauge.
#[derive(Component)]
pub(crate) struct VillageGaugeFill(VillageStat);

/// The small value text beside a ledger gauge.
#[derive(Component)]
pub(crate) struct VillageGaugeValue(VillageStat);

/// The one line of prose at the ledger's foot: the land itself.
#[derive(Component)]
pub(crate) struct VillageLand;

/// The FAITH tab's roster: who believes, and the last reason why.
#[derive(Component)]
pub(crate) struct FaithRoster;

/// A faith roster line's live text, keyed to its soul so drifting numbers
/// update in place instead of rebuilding the rows: 0 name, 1 standing,
/// 2 the why.
#[derive(Component)]
pub(crate) struct FaithText {
    person: Entity,
    field: u8,
}

/// One soul's standing row on the FAITH roster. When the ranking shifts,
/// these are reordered in place rather than torn down - despawning cost a
/// layout frame and jolted the scroll.
#[derive(Component)]
pub(crate) struct FaithRow(Entity);

/// The line under the happiness gauge saying WHY it stands where it does.
#[derive(Component)]
pub(crate) struct HappinessWhy;

/// The village's name, writ large at the head of the detail pane.
#[derive(Component)]
pub(crate) struct LedgerName;

/// The name's echo on the rail's single (for now) village entry, in the
/// reading face rather than engraved capitals.
#[derive(Component)]
pub(crate) struct LedgerRailName;

/// The line under the name: what kind of place this is, in the game's voice.
#[derive(Component)]
pub(crate) struct LedgerEpithet;

/// One ACTIVITY row's value: 0 working, 1 resting, 2 praying, 3 about.
#[derive(Component)]
pub(crate) struct ActivityRow(u8);

/// Which page of the codex is open.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodexPage {
    Ledger,
    People,
    Chronicle,
    Deity,
    Prayers,
    World,
    Settings,
}

/// The codex's spine: which page is open, and the handles the page-turn
/// needs — page roots, the two live tabs, and the title texts to rewrite.
#[derive(Resource)]
pub(crate) struct Codex {
    pub page: CodexPage,
    pub root: Entity,
    pub ledger_page: Entity,
    pub people_page: Entity,
    pub chronicle_page: Entity,
    pub deity_page: Entity,
    pub prayers_page: Entity,
    pub world_page: Entity,
    pub ledger_tab: Entity,
    pub people_tab: Entity,
    pub chronicle_tab: Entity,
    pub deity_tab: Entity,
    pub prayers_tab: Entity,
    pub world_tab: Entity,
    pub settings_page: Entity,
    pub settings_tab: Entity,
    pub title_text: Entity,
    pub subtitle_text: Option<Entity>,
}

/// A live tab on the codex strip: pressing it turns to its page.
#[derive(Component)]
pub(crate) struct CodexTab(pub CodexPage);

/// The ledger banner's cloth: dressed with the town's true arms by
/// [`dress_ledger_banner`] once (and whenever) a settlement stands.
#[derive(Component)]
pub(crate) struct LedgerBannerCloth;

/// The DETAILS page wells, rebuilt on a slow clock while the window is open.
#[derive(Component)]
pub(crate) struct BuildingRows;

#[derive(Component)]
pub(crate) struct TradeRows;

/// The prayer board's well of open prayers, rebuilt while the Deity page
/// shows. The first resident of the Deity page: what the faithful ask.
#[derive(Component)]
pub(crate) struct PrayerRows;

/// The strip of recently closed prayers beneath the board — the receipts.
#[derive(Component)]
pub(crate) struct PrayerHistoryRows;

/// A clickable prayer row: pressing it flies the god to whoever is asking.
#[derive(Component)]
pub(crate) struct PrayerRow(pub Entity);

// ---------------------------------------------------------------------------
// Glyphs: little engraved marks drawn from nodes, same hand as everything.
// ---------------------------------------------------------------------------

/// A fixed canvas the glyph bars are placed on, absolutely.
pub(crate) fn glyph_canvas(commands: &mut Commands, parent: Entity, size: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: px(size),
                height: px(size),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(parent),
        ))
        .id()
}

pub(crate) fn bar(
    commands: &mut Commands,
    canvas: Entity,
    (left, top, width, height): (f32, f32, f32, f32),
    turn: f32,
    tint: Color,
    round: bool,
) {
    let mut piece = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            width: px(width),
            height: px(height),
            border_radius: if round {
                BorderRadius::all(px(width.max(height)))
            } else {
                BorderRadius::all(px(0))
            },
            ..default()
        },
        BackgroundColor(tint),
        ChildOf(canvas),
    ));
    if turn != 0.0 {
        piece.insert(UiTransform::from_rotation(Rot2::degrees(turn)));
    }
}

/// A house: walls under a peaked roof.
pub(crate) fn house_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (4.0, 9.0, 10.0, 7.0), 0.0, tint, false);
    bar(commands, c, (1.0, 5.0, 9.0, 2.5), -33.0, tint, false);
    bar(commands, c, (8.0, 5.0, 9.0, 2.5), 33.0, tint, false);
}

/// A tree: the chronicle's mark, at button scale.
pub(crate) fn tree_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (8.0, 11.0, 2.5, 6.0), 0.0, tint, false);
    bar(commands, c, (4.0, 7.5, 10.5, 3.5), 0.0, tint, false);
    bar(commands, c, (5.5, 4.0, 7.5, 3.5), 0.0, tint, false);
    bar(commands, c, (7.0, 1.0, 4.5, 3.0), 0.0, tint, false);
}

/// Faith: two bars leant together in prayer.
pub(crate) fn hands_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (4.0, 4.0, 3.0, 11.0), 22.0, tint, false);
    bar(commands, c, (11.0, 4.0, 3.0, 11.0), -22.0, tint, false);
}

/// The prayer mote: the turned spark that hangs over the praying, with its
/// fall of light beneath — the mark prayers wear in the world, worn again
/// on their page's tab.
#[allow(dead_code)] // Waits for the rail's icon pass.
pub(crate) fn mote_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (6.5, 3.0, 5.0, 5.0), 45.0, tint, false);
    bar(
        commands,
        c,
        (8.25, 11.0, 1.5, 5.0),
        0.0,
        tint.with_alpha(0.55),
        false,
    );
}

/// A scroll: a page bearing written lines.
#[allow(dead_code)] // Waits for the rail's icon pass.
pub(crate) fn scroll_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(
        commands,
        c,
        (3.5, 2.0, 11.0, 14.0),
        0.0,
        tint.with_alpha(0.22),
        false,
    );
    for top in [5.0, 8.0, 11.0] {
        bar(commands, c, (5.5, top, 7.0, 1.5), 0.0, tint, false);
    }
}

/// Sliders: three rails, knobs at their own stations — the settings mark.
#[allow(dead_code)] // Waits for the rail's icon pass.
pub(crate) fn sliders_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    for (top, knob_left) in [(3.5, 11.0), (8.0, 4.5), (12.5, 8.0)] {
        bar(
            commands,
            c,
            (3.0, top + 1.0, 12.0, 1.4),
            0.0,
            tint.with_alpha(0.55),
            false,
        );
        bar(commands, c, (knob_left, top, 2.6, 3.6), 0.0, tint, false);
    }
}

/// A person: head over shoulders.
pub(crate) fn person_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (6.5, 2.0, 5.0, 5.0), 0.0, tint, true);
    bar(commands, c, (4.0, 8.5, 10.0, 7.0), 0.0, tint, true);
}

/// A gathering: two souls shoulder to shoulder.
pub(crate) fn people_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(
        commands,
        c,
        (3.0, 3.5, 4.0, 4.0),
        0.0,
        tint.with_alpha(0.7),
        true,
    );
    bar(
        commands,
        c,
        (1.0, 8.5, 8.0, 6.5),
        0.0,
        tint.with_alpha(0.7),
        true,
    );
    bar(commands, c, (10.5, 2.0, 4.5, 4.5), 0.0, tint, true);
    bar(commands, c, (8.5, 7.5, 9.0, 7.5), 0.0, tint, true);
}

/// Draws a town's sigil - the same rectangles the world raises in gold
/// blocks on the cloth, here as nodes.
pub(crate) fn sigil_glyph(
    commands: &mut Commands,
    parent: Entity,
    sigil: usize,
    tint: Color,
    size: f32,
) {
    let c = glyph_canvas(commands, parent, size);
    let k = size / 16.0;
    for &(x, y, w, h, turn, round) in crate::sigil::rects(sigil) {
        bar(commands, c, (x * k, y * k, w * k, h * k), turn, tint, round);
    }
}

/// The village banner: a hung cloth with the tree upon it, flying at the
/// detail pane's shoulder the way the mockup flies it.
pub(crate) fn banner_glyph(commands: &mut Commands, parent: Entity) {
    // A banner with a banner's anatomy: a finialled crossbar, the cloth
    // hanging from it, a swallowtail notch at the foot, the tree writ
    // larger on the drop. All in the book's gold - the codex is monochrome
    // on purpose.
    let stage = commands
        .spawn((
            Node {
                width: px(72),
                height: px(88),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let gold = ui::theme::accent();
    // The crossbar, and its finials.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(2),
            top: px(4),
            width: px(68),
            height: px(3),
            ..default()
        },
        BackgroundColor(gold.with_alpha(0.85)),
        ChildOf(stage),
    ));
    for left in [0.0, 66.0] {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(left),
                top: px(2.5),
                width: px(6),
                height: px(6),
                ..default()
            },
            UiTransform::from_rotation(Rot2::degrees(45.0)),
            BackgroundColor(gold.with_alpha(0.9)),
            ChildOf(stage),
        ));
    }
    // The cloth, hanging from the bar.
    let cloth = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(9),
                top: px(7),
                width: px(54),
                height: px(70),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(px(16)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg()),
            BorderColor::all(gold.with_alpha(0.5)),
            ChildOf(stage),
        ))
        .id();
    // The arms arrive by system once the settlement exists: the cloth's
    // true colour, and the sign the town rolled at its founding.
    commands.entity(cloth).insert(LedgerBannerCloth);
    // The swallowtail: a turned square of the pane's own ground, cutting
    // the notch out of the cloth's foot - with two gold threads to hem it.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(26),
            top: px(66),
            width: px(20),
            height: px(20),
            ..default()
        },
        UiTransform::from_rotation(Rot2::degrees(45.0)),
        BackgroundColor(ui::theme::card_bg()),
        ChildOf(stage),
    ));
    for turn in [45.0, -45.0] {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(if turn > 0.0 { 24.0 } else { 34.0 }),
                top: px(73),
                width: px(15),
                height: px(1.5),
                ..default()
            },
            UiTransform::from_rotation(Rot2::degrees(turn)),
            BackgroundColor(gold.with_alpha(0.5)),
            ChildOf(stage),
        ));
    }
}

// ---------------------------------------------------------------------------
// The window.
// ---------------------------------------------------------------------------

/// The page's one spacing rhythm: this many pixels around the content and
/// between every band and column. The codex body's padding is overridden
/// to match, and the maths stays integral: the inner well is 1120 wide,
/// and 1120 = 3 x 366 + 2 x 11.
pub(crate) const RHYTHM: f32 = 11.0;

/// The main split band's height. The plates (96) and the land strip (40)
/// ride above and below it with the rhythm between; the sum is the page
/// band.
const MAIN_BAND: f32 = 494.0;

/// Every page's full height: the ledger's bands sum to it (96 + 5 + 501 +
/// 5 + 45), and the people page is pinned to it directly.
#[allow(dead_code)] // The old fixed-window rhythm; pages fill the book now.
const PAGE_BAND: f32 = 652.0;

pub(crate) fn spawn_village_panel(mut commands: Commands) {
    // The Illuminated Ledger: no longer a floating panel with dead space
    // around it, but the whole screen — Ordo's book, chapters down the
    // left, the world running live and dimmed behind the page. Brett:
    // "the codex should be full screen, since the area around the current
    // panel is unused space."
    let book = ordo::book(&mut commands, "THE LEDGER", "The heart of a living world.");
    commands
        .entity(book.root)
        .insert((Name::new("Codex Panel"), VillagePanel, Visibility::Hidden));

    // The pages, as siblings in the book's content: exactly one shows at
    // a time, each owning the whole reading surface.
    let ledger_page = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(RHYTHM),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ui::Scrollable,
            bevy::ui::ScrollPosition::default(),
            Interaction::default(),
            Visibility::Inherited,
            ChildOf(book.content),
        ))
        .id();
    let mut bound_page = || {
        commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    min_height: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(ui::theme::GAP),
                    overflow: Overflow::scroll_y(),
                    display: Display::None,
                    ..default()
                },
                ui::Scrollable,
                bevy::ui::ScrollPosition::default(),
                Interaction::default(),
                Visibility::Hidden,
                ChildOf(book.content),
            ))
            .id()
    };
    let people_page = bound_page();
    let chronicle_page = bound_page();
    let deity_page = bound_page();
    let prayers_page = bound_page();
    let world_page = bound_page();
    let settings_page = bound_page();

    // The chapters, down the rail: the book's own table of contents.
    let chapter = |commands: &mut Commands,
                   label: &str,
                   page: CodexPage,
                   hint_title: &str,
                   hint_line: &str|
     -> Entity {
        let button = ordo::chapter(commands, book.rail, label);
        commands
            .entity(button)
            .insert((CodexTab(page), ui::HoverHint::new(hint_title, hint_line)));
        button
    };
    let ledger_tab = chapter(
        &mut commands,
        "THE TOWNS",
        CodexPage::Ledger,
        "The Towns",
        "the heart of a living world",
    );
    let people_tab = chapter(
        &mut commands,
        "THE PEOPLE",
        CodexPage::People,
        "The People",
        "the mortals of your world",
    );
    let deity_tab = chapter(
        &mut commands,
        "THE MIRACLES",
        CodexPage::Deity,
        "The Miracles",
        "you are the unseen; they are the faithful",
    );
    let prayers_tab = chapter(
        &mut commands,
        "THE PRAYERS",
        CodexPage::Prayers,
        "The Prayers",
        "what the faithful ask of you",
    );
    let chronicle_tab = chapter(
        &mut commands,
        "THE CHRONICLE",
        CodexPage::Chronicle,
        "The Chronicle",
        "the tale of your people, written moment by moment",
    );
    let world_tab = chapter(
        &mut commands,
        "THE WORLD",
        CodexPage::World,
        "The World",
        "the lands your people walk; the seasons turn",
    );
    let settings_tab = chapter(
        &mut commands,
        "SETTINGS",
        CodexPage::Settings,
        "The Settings",
        "the god's own preferences",
    );

    // The world's pulse, docked in the footer: the same speed buttons the
    // apron wears (one system serves every copy), and the book's close.
    let footer_note = commands
        .spawn((
            ui::dim("the world turns while you read"),
            ChildOf(book.footer),
        ))
        .id();
    let _ = footer_note;
    for (speed, label) in [
        (None, "II"),
        (Some(1.0), "1x"),
        (Some(2.0), "2x"),
        (Some(4.0), "4x"),
        (Some(8.0), "8x"),
    ] {
        let button = commands
            .spawn((
                crate::speed::SpeedButton(speed),
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
                ChildOf(book.footer),
            ))
            .id();
        commands.spawn((ui::dim(label), ChildOf(button)));
    }
    let date = commands
        .spawn((
            FooterDate,
            ui::dim(""),
            Node {
                margin: UiRect::left(px(10)),
                ..default()
            },
            ChildOf(book.footer),
        ))
        .id();
    let _ = date;
    let close = commands
        .spawn((
            CodexClose,
            ui::UiButton,
            Interaction::default(),
            Node {
                margin: UiRect::left(auto()),
                padding: UiRect::axes(px(12), px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BackgroundColor(ui::theme::panel_bg().with_alpha(0.4)),
            BorderColor::all(ui::theme::panel_border()),
            ui::HoverHint::new("Close", "or press the codex key"),
            ChildOf(book.footer),
        ))
        .id();
    commands.spawn((ui::dim("CLOSE"), ChildOf(close)));

    build_settings_page(&mut commands, settings_page);
    build_prayers_page(&mut commands, prayers_page);

    commands.insert_resource(Codex {
        page: CodexPage::Ledger,
        root: book.root,
        settings_page,
        settings_tab,
        ledger_page,
        people_page,
        chronicle_page,
        deity_page,
        prayers_page,
        world_page,
        ledger_tab,
        people_tab,
        chronicle_tab,
        deity_tab,
        prayers_tab,
        world_tab,
        title_text: book.title,
        subtitle_text: Some(book.subtitle),
    });

    // ---- Band one: the three great numbers. -------------------------------
    let plates = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(96),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                column_gap: px(RHYTHM),
                ..default()
            },
            ChildOf(ledger_page),
        ))
        .id();
    for (index, label) in [(0u8, "souls"), (1, "houses"), (2, "believers")] {
        let (seat, number) = ui::stat_plate(&mut commands, plates, label);
        commands.entity(number).insert(VillageCard(index));
        let tint = ui::theme::accent().with_alpha(0.8);
        match index {
            0 => person_glyph(&mut commands, seat, tint),
            1 => house_glyph(&mut commands, seat, tint),
            _ => hands_glyph(&mut commands, seat, tint),
        }
    }

    // ---- Band two: the rail and the reading. ------------------------------
    let main = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(MAIN_BAND),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(ledger_page),
        ))
        .id();
    // The page grid: the same three tracts the plates rule above. The
    // rail takes column one, the reading takes columns two and three, and
    // every seam lands under a plate's edge. Brett: "If everything fit
    // these three column widths then everything would always align."
    let band = commands.spawn((ordo::grid_row(RHYTHM), ChildOf(main))).id();
    commands
        .entity(band)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.flex_grow = 1.0;
            node.min_height = px(0);
        });
    let rail = commands.spawn((ordo::col(1, RHYTHM), ChildOf(band))).id();
    commands
        .entity(rail)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.row_gap = px(RHYTHM);
            // Vertical breathing only: horizontal padding would inset
            // every card off the tract edge, and the rule of three reads
            // by the VISIBLE boxes, not the invisible columns.
            node.padding = UiRect::axes(px(0), px(8));
            node.border = UiRect::all(px(1));
            node.min_height = px(0);
        });
    commands.entity(rail).insert((
        BackgroundColor(Color::BLACK.with_alpha(0.32)),
        BorderColor::all(ui::theme::panel_border().with_alpha(0.35)),
    ));
    let detail = commands.spawn((ordo::col(2, RHYTHM), ChildOf(band))).id();
    commands
        .entity(detail)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.min_height = px(0);
        });

    // The rail: OVERVIEW lists the villages of this world (one banner so
    // far, honestly); FAITH ranks every soul by their trust.
    let rail_pages = ui::tab_bar(&mut commands, rail, &["OVERVIEW", "FAITH"]);
    let villages_page = rail_pages[0];
    let faith_page = rail_pages[1];
    commands.entity(villages_page).insert(Node {
        width: percent(100),
        flex_grow: 1.0,
        min_height: px(0),
        flex_direction: FlexDirection::Column,
        row_gap: px(5),
        ..default()
    });
    let entry = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10),
                padding: UiRect::axes(px(12), px(10)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg().with_alpha(0.55)),
            BorderColor::all(ui::theme::accent().with_alpha(0.85)),
            ChildOf(villages_page),
        ))
        .id();
    house_glyph(&mut commands, entry, ui::theme::accent().with_alpha(0.8));
    let entry_words = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(1),
                ..default()
            },
            ChildOf(entry),
        ))
        .id();
    commands.spawn((LedgerRailName, ui::body(""), ChildOf(entry_words)));
    commands.spawn((ui::dim("the first banner raised"), ChildOf(entry_words)));
    // The eye: fly home to this banner, the way the People rows follow a
    // soul - and the codex steps aside so the village fills the view.
    let eye = commands
        .spawn((
            super::RecenterButton,
            ui::UiButton,
            ui::KeepFace,
            ui::HoverHint::new("Fly home", "the camera returns to the village banner"),
            Node {
                width: px(26),
                height: px(26),
                flex_shrink: 0.0,
                margin: UiRect::left(Val::Auto),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(26)),
                ..default()
            },
            BorderColor::all(ui::theme::accent().with_alpha(0.6)),
            Interaction::default(),
            ChildOf(entry),
        ))
        .id();
    commands.spawn((
        Node {
            width: px(8),
            height: px(8),
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        BackgroundColor(ui::theme::accent().with_alpha(0.85)),
        ChildOf(eye),
    ));

    commands.entity(faith_page).insert((
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
            overflow: Overflow::scroll_y(),
            display: Display::None,
            ..default()
        },
        ui::Scrollable,
        ScrollPosition::DEFAULT,
        Interaction::default(),
    ));
    commands.spawn((
        FaithRoster,
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            ..default()
        },
        ChildOf(faith_page),
    ));

    // The reading: name and standing under the banner, then the pages.
    let header = commands
        .spawn((
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            ChildOf(detail),
        ))
        .id();
    let words = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            },
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        LedgerName,
        Text::new(""),
        ui::DisplayFace,
        TextFont {
            font_size: FontSize::Px(30.0),
            ..default()
        },
        TextColor(ui::theme::accent()),
        ChildOf(words),
    ));
    commands.spawn((LedgerEpithet, ui::dim(""), ChildOf(words)));
    banner_glyph(&mut commands, header);

    let detail_pages = ui::tab_bar(&mut commands, detail, &["OVERVIEW", "DETAILS"]);
    let overview = detail_pages[0];
    let details = detail_pages[1];

    // OVERVIEW: four card wells in a two-by-two, the mockup's grid.
    commands.entity(overview).insert(Node {
        width: percent(100),
        flex_grow: 1.0,
        min_height: px(0),
        flex_direction: FlexDirection::Column,
        row_gap: px(10),
        ..default()
    });
    let grid_row = |commands: &mut Commands| -> Entity {
        commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    min_height: px(0),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(10),
                    align_items: AlignItems::Stretch,
                    ..default()
                },
                ChildOf(overview),
            ))
            .id()
    };
    let top_row = grid_row(&mut commands);
    let bottom_row = grid_row(&mut commands);

    let gauge = |commands: &mut Commands, parent, label: &str, stat, color| {
        let handles = ui::gauge_row(commands, parent, label, color);
        commands.entity(handles.fill).insert(VillageGaugeFill(stat));
        commands
            .entity(handles.value)
            .insert(VillageGaugeValue(stat));
    };

    let wellbeing = ui::card_well(&mut commands, top_row, "WELLBEING");
    gauge(
        &mut commands,
        wellbeing,
        "happiness",
        VillageStat::Happiness,
        crate::palette::shade(&crate::palette::GRASS, 0.7),
    );
    gauge(
        &mut commands,
        wellbeing,
        "fed",
        VillageStat::Fed,
        crate::palette::shade(&crate::palette::CLOTH_RED, 0.6),
    );
    gauge(
        &mut commands,
        wellbeing,
        "housed",
        VillageStat::Housed,
        crate::palette::shade(&crate::palette::WOOD, 0.65),
    );
    commands.spawn((HappinessWhy, ui::dim(""), ChildOf(wellbeing)));

    let faith_card = ui::card_well(&mut commands, top_row, "FAITH");
    gauge(
        &mut commands,
        faith_card,
        "belief in you",
        VillageStat::Faith,
        ui::theme::accent(),
    );
    gauge(
        &mut commands,
        faith_card,
        "believers",
        VillageStat::Believers,
        ui::theme::accent().with_alpha(0.55),
    );

    let stores = ui::card_well(&mut commands, bottom_row, "STORES");
    gauge(
        &mut commands,
        stores,
        "food",
        VillageStat::Food,
        crate::palette::shade(&crate::palette::GRASS, 0.55),
    );
    gauge(
        &mut commands,
        stores,
        "timber",
        VillageStat::Timber,
        crate::palette::shade(&crate::palette::WOOD, 0.5),
    );
    gauge(
        &mut commands,
        stores,
        "stone",
        VillageStat::Stone,
        crate::palette::shade(&crate::palette::STONE, 0.55),
    );

    let activity = ui::card_well(&mut commands, bottom_row, "ACTIVITY");
    for (index, label) in [
        (0u8, "working"),
        (1, "resting"),
        (2, "praying"),
        (3, "about the day"),
    ] {
        let value = ui::ruled_row(&mut commands, activity, label);
        commands.entity(value).insert(ActivityRow(index));
    }

    // DETAILS: the civic ladder and the trades, side by side.
    commands.entity(details).insert(Node {
        width: percent(100),
        flex_grow: 1.0,
        min_height: px(0),
        flex_direction: FlexDirection::Row,
        column_gap: px(10),
        align_items: AlignItems::Stretch,
        display: Display::None,
        ..default()
    });
    let ladder = ui::card_well(&mut commands, details, "THE CIVIC LADDER");
    commands.spawn((
        BuildingRows,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ui::Scrollable,
        ScrollPosition::DEFAULT,
        Interaction::default(),
        ChildOf(ladder),
    ));
    let trades = ui::card_well(&mut commands, details, "THE TRADES");
    commands.spawn((
        TradeRows,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ui::Scrollable,
        ScrollPosition::DEFAULT,
        Interaction::default(),
        ChildOf(trades),
    ));

    // ---- Band three: the land itself. -------------------------------------
    let land = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(40),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10),
                padding: UiRect::axes(px(12), px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg().with_alpha(0.55)),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(ledger_page),
        ))
        .id();
    tree_glyph(&mut commands, land, ui::theme::accent().with_alpha(0.7));
    commands.spawn((VillageLand, ui::dim(""), ChildOf(land)));
}

/// Dresses the ledger's banner in the town's true arms: the cloth takes
/// the founding roll's colour, and the sigil - the same rectangles the
/// world raises in gold on the real banner - is drawn upon it. Re-dresses
/// itself when a different settlement stands (a new world, a loaded save).
pub(crate) fn dress_ledger_banner(
    mut commands: Commands,
    site: Option<Res<crate::villager::SettlementSite>>,
    settlements: Query<&crate::villager::Settlement>,
    cloths: Query<Entity, With<LedgerBannerCloth>>,
    mut fills: Query<&mut BackgroundColor>,
    mut seen: Local<Option<(usize, usize)>>,
) {
    let Some(arms) = site
        .as_ref()
        .and_then(|site| settlements.get(site.settlement).ok())
        .map(|s| (s.banner_ramp, s.sigil))
    else {
        return;
    };
    if *seen == Some(arms) {
        return;
    }
    *seen = Some(arms);
    let field = crate::palette::shade(&crate::palette::ALL_RAMPS[arms.0], 0.8);
    // The shared rule of tincture, so the ledger's banner and the banner in
    // the square always agree on which metal the sign is struck in.
    let srgb = field.to_srgba();
    let ink = if crate::sigil::gold_reads_on([srgb.red, srgb.green, srgb.blue]) {
        ui::theme::accent()
    } else {
        let dark = crate::sigil::dark_ink();
        Color::srgb(dark[0], dark[1], dark[2])
    };
    for cloth in &cloths {
        commands.entity(cloth).despawn_related::<Children>();
        if let Ok(mut fill) = fills.get_mut(cloth) {
            fill.0 = field;
        }
        sigil_glyph(&mut commands, cloth, arms.1, ink, 30.0);
    }
}

/// Pressing a live tab turns the codex to that page.
/// The book's own close, docked in the footer.
#[derive(Component)]
pub(crate) struct CodexClose;

/// The date, spoken in the footer while the book covers the world's card.
#[derive(Component)]
pub(crate) struct FooterDate;

/// Keeps the footer's date true while the book is open.
pub(crate) fn footer_date(
    time: Res<Time>,
    mut since: Local<f32>,
    clock: Option<Res<crate::calendar::WorldClock>>,
    panels: Query<&Visibility, With<VillagePanel>>,
    mut dates: Query<&mut Text, With<FooterDate>>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    *since += time.delta_secs();
    if *since < 0.5 {
        return;
    }
    *since = 0.0;
    let Some(clock) = clock else {
        return;
    };
    let season = clock.season().name().to_uppercase();
    let fresh = format!(
        "{season} {}  -  YEAR {}",
        clock.day_of_season(),
        clock.year()
    );
    for mut text in &mut dates {
        if text.0 != fresh {
            *text = Text::new(fresh.clone());
        }
    }
}

pub(crate) fn handle_codex_tabs(
    tabs: Query<(&Interaction, &CodexTab), Changed<Interaction>>,
    closes: Query<&Interaction, (With<CodexClose>, Changed<Interaction>)>,
    codex: Option<ResMut<Codex>>,
    mut panels: Query<&mut Visibility, With<VillagePanel>>,
) {
    let Some(mut codex) = codex else {
        return;
    };
    for (interaction, tab) in &tabs {
        if *interaction == Interaction::Pressed && codex.page != tab.0 {
            codex.page = tab.0;
        }
    }
    for interaction in &closes {
        if *interaction == Interaction::Pressed {
            for mut panel in &mut panels {
                *panel = Visibility::Hidden;
            }
        }
    }
}

/// Applies the open page: the page roots' layout and visibility mirror the
/// window and the open page every frame (the gates every update system reads
/// live on the page nodes), while the strip's tab dress and the title bar
/// rename only on an actual page-turn.
pub(crate) fn apply_codex_page(
    codex: Res<Codex>,
    mut nodes: Query<&mut Node>,
    mut visibilities: Query<&mut Visibility>,
    mut fills: Query<&mut BackgroundColor>,
    mut borders: Query<&mut BorderColor>,
    mut texts: Query<&mut Text>,
) {
    let window_open = visibilities
        .get(codex.root)
        .is_ok_and(|v| *v != Visibility::Hidden);
    // A closed codex leaves the LAYOUT tree entirely, not just the screen.
    // Hidden-but-displayed, its hundreds of nodes were re-laid-out every time
    // any bubble anywhere dirtied the UI — most of the cost of showing a
    // thought was recomputing a book nobody had open.
    if let Ok(mut node) = nodes.get_mut(codex.root) {
        let display = if window_open {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    let pages = [
        (codex.settings_page, codex.page == CodexPage::Settings),
        (codex.ledger_page, codex.page == CodexPage::Ledger),
        (codex.people_page, codex.page == CodexPage::People),
        (codex.chronicle_page, codex.page == CodexPage::Chronicle),
        (codex.deity_page, codex.page == CodexPage::Deity),
        (codex.prayers_page, codex.page == CodexPage::Prayers),
        (codex.world_page, codex.page == CodexPage::World),
    ];
    for (page, open) in pages {
        if let Ok(mut node) = nodes.get_mut(page) {
            let display = if open { Display::Flex } else { Display::None };
            if node.display != display {
                node.display = display;
            }
        }
        if let Ok(mut visibility) = visibilities.get_mut(page) {
            let fresh = if open && window_open {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != fresh {
                *visibility = fresh;
            }
        }
    }
    if !codex.is_changed() {
        return;
    }
    let tabs = [
        (codex.settings_tab, codex.page == CodexPage::Settings),
        (codex.ledger_tab, codex.page == CodexPage::Ledger),
        (codex.people_tab, codex.page == CodexPage::People),
        (codex.chronicle_tab, codex.page == CodexPage::Chronicle),
        (codex.deity_tab, codex.page == CodexPage::Deity),
        (codex.prayers_tab, codex.page == CodexPage::Prayers),
        (codex.world_tab, codex.page == CodexPage::World),
    ];
    // The open chapter is unmistakable: a filled gold band in the rail.
    // Everything else waits quietly with no box at all until hovered.
    for (tab, open) in tabs {
        if let Ok(mut fill) = fills.get_mut(tab) {
            fill.0 = if open {
                ui::theme::accent().with_alpha(0.24)
            } else {
                Color::NONE
            };
        }
        if let Ok(mut border) = borders.get_mut(tab) {
            *border = BorderColor::all(if open {
                ui::theme::accent()
            } else {
                Color::NONE
            });
        }
    }
    let (title, subtitle) = match codex.page {
        CodexPage::Ledger => ("THE LEDGER", "The heart of a living world."),
        CodexPage::People => ("THE PEOPLE", "The mortals of your world."),
        CodexPage::Chronicle => (
            "THE CHRONICLE",
            "The tale of your people, written moment by moment.",
        ),
        CodexPage::Deity => ("THE DEITY", "You are the unseen. They are the faithful."),
        CodexPage::Prayers => ("THE PRAYERS", "What the faithful ask of you."),
        CodexPage::World => (
            "THE WORLD",
            "The lands your people walk. The seasons turn. The world endures.",
        ),
        CodexPage::Settings => ("THE SETTINGS", "The god's own preferences."),
    };
    if let Ok(mut text) = texts.get_mut(codex.title_text)
        && text.0 != title
    {
        *text = Text::new(title);
    }
    if let Some(subtitle_text) = codex.subtitle_text
        && let Ok(mut text) = texts.get_mut(subtitle_text)
        && text.0 != subtitle
    {
        *text = Text::new(subtitle);
    }
}

/// What kind of place this is, in the game's voice.
fn epithet(souls: usize, believer_fraction: f32) -> String {
    let size = match souls {
        0..=14 => "A hamlet",
        15..=39 => "A small village",
        40..=79 => "A village",
        80..=149 => "A town",
        _ => "A city",
    };
    let standing = if believer_fraction >= 0.6 {
        "strong in faith"
    } else if believer_fraction >= 0.3 {
        "growing in faith"
    } else if believer_fraction > 0.05 {
        "of wavering faith"
    } else {
        "that does not yet believe"
    };
    format!("{size}, {standing}")
}

/// Fills the ledger while it is open.
#[allow(clippy::type_complexity)]
pub(crate) fn update_village_panel(
    codex: Res<Codex>,
    panels: Query<&Visibility, With<VillagePanel>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    settlements: Query<&crate::villager::Settlement>,
    stores: Query<&crate::villager::work::Stockpile>,
    villagers: Query<
        (
            Option<&Needs>,
            Option<&Morale>,
            Option<&crate::villager::belief::Faith>,
            Option<&crate::villager::home::Home>,
            Has<crate::creature::Childhood>,
            Option<&Activity>,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    huts: Query<
        (),
        Or<(
            With<crate::villager::work::Hut>,
            With<crate::villager::work::Longhouse>,
        )>,
    >,
    trees: Query<&crate::scatter::FellableTree>,
    wildlife: Query<
        (),
        (
            With<crate::creature::wildlife::Wild>,
            Without<crate::creature::Corpse>,
        ),
    >,
    graves: Query<(), With<crate::villager::rites::Grave>>,
    mut whys: Query<
        &mut Text,
        (
            With<HappinessWhy>,
            Without<VillageCard>,
            Without<VillageGaugeValue>,
            Without<VillageLand>,
            Without<LedgerName>,
            Without<LedgerRailName>,
            Without<LedgerEpithet>,
            Without<ActivityRow>,
        ),
    >,
    mut gauges: ParamSet<(
        Query<(&VillageCard, &mut Text)>,
        Query<(&VillageGaugeFill, &mut Node)>,
        Query<(&VillageGaugeValue, &mut Text)>,
        Query<&mut Text, (With<VillageLand>, Without<HappinessWhy>)>,
        Query<&mut Text, With<LedgerName>>,
        Query<&mut Text, With<LedgerRailName>>,
        Query<&mut Text, With<LedgerEpithet>>,
        Query<(&ActivityRow, &mut Text)>,
    )>,
) {
    if codex.page != CodexPage::Ledger || !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }

    let living = villagers.iter().count().max(1);
    let mut spirits = 0.0;
    let mut fed = 0.0;
    let mut housed = 0usize;
    let mut trust = 0.0;
    let mut believers = 0usize;
    let mut roofless_adults = 0usize;
    let mut weary = 0usize;
    let mut hungry = 0usize;
    let mut working = 0usize;
    let mut resting = 0usize;
    let mut praying = 0usize;
    for (needs, morale, faith, home, child, activity) in &villagers {
        spirits += morale.map_or(0.6, |m| m.spirits);
        fed += 1.0 - needs.map_or(0.3, |n| n.hunger);
        if home.is_some() {
            housed += 1;
        } else if !child {
            roofless_adults += 1;
        }
        if needs.is_some_and(|n| n.rest > 0.7) {
            weary += 1;
        }
        if needs.is_some_and(|n| n.hunger > 0.5) {
            hungry += 1;
        }
        let t = faith.map_or(0.0, |f| f.trust);
        trust += t;
        if t > crate::villager::belief::Faith::BELIEVER {
            believers += 1;
        }
        match activity {
            Some(Activity::Working)
            | Some(Activity::Hauling)
            | Some(Activity::TendingFire)
            | Some(Activity::Bearing) => working += 1,
            Some(Activity::Sleeping) | Some(Activity::Sheltering) => resting += 1,
            Some(Activity::Praying) => praying += 1,
            _ => {}
        }
    }
    let about = living.saturating_sub(working + resting + praying);
    // Both roofs count toward the headline number: it answers "how many
    // buildings do these people sleep in", not "how many are family homes".
    let houses = huts.iter().count();
    let (food, timber, stone) = site
        .as_ref()
        .and_then(|site| stores.get(site.settlement).ok())
        .map_or((0.0, 0.0, 0.0), |s| (s.food(), s.timber, s.stone));

    // The name at the head of the page, and its standing beneath.
    let name = site
        .as_ref()
        .and_then(|site| settlements.get(site.settlement).ok())
        .map_or_else(|| "the village".to_string(), |s| s.name.clone());
    for mut text in &mut gauges.p4() {
        let engraved = name.to_uppercase();
        if text.0 != engraved {
            *text = Text::new(engraved);
        }
    }
    for mut text in &mut gauges.p5() {
        if text.0 != name {
            *text = Text::new(name.clone());
        }
    }
    let standing = epithet(living, believers as f32 / living as f32);
    for mut text in &mut gauges.p6() {
        if text.0 != standing {
            *text = Text::new(standing.clone());
        }
    }
    for (row, mut text) in &mut gauges.p7() {
        let count = match row.0 {
            0 => working,
            1 => resting,
            2 => praying,
            _ => about,
        };
        let fresh = format!("{count} / {living}");
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }

    for (card, mut text) in &mut gauges.p0() {
        let fresh = match card.0 {
            0 => format!("{}", living),
            1 => format!("{houses}"),
            _ => format!("{believers}"),
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }

    let fraction = |stat: VillageStat| -> f32 {
        match stat {
            VillageStat::Happiness => spirits / living as f32,
            VillageStat::Fed => fed / living as f32,
            VillageStat::Housed => housed as f32 / living as f32,
            VillageStat::Faith => (trust / living as f32) / 0.8,
            VillageStat::Believers => believers as f32 / living as f32,
            VillageStat::Food => food / 60.0,
            VillageStat::Timber => timber / 30.0,
            VillageStat::Stone => stone / 24.0,
        }
        .clamp(0.0, 1.0)
    };
    for (fill, mut node) in &mut gauges.p1() {
        node.width = percent(fraction(fill.0) * 100.0);
    }
    for (value, mut text) in &mut gauges.p2() {
        let fresh = match value.0 {
            VillageStat::Food => format!("{food:.0}"),
            VillageStat::Timber => format!("{timber:.0}"),
            VillageStat::Stone => format!("{stone:.0}"),
            VillageStat::Housed => format!("{housed}/{living}"),
            VillageStat::Believers => format!("{believers}/{living}"),
            stat => format!("{:.0}%", fraction(stat) * 100.0),
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
    // Why the happiness bar stands where it does, biggest weights first.
    let mut reasons: Vec<(usize, String)> = Vec::new();
    if roofless_adults > 0 {
        reasons.push((
            roofless_adults,
            format!("{roofless_adults} sleep without a roof"),
        ));
    }
    if weary > 0 {
        reasons.push((weary, format!("{weary} are worn out")));
    }
    if hungry > 0 {
        reasons.push((hungry, format!("{hungry} go hungry")));
    }
    reasons.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    let fresh = if reasons.is_empty() {
        "no great weight on anyone".to_string()
    } else {
        reasons
            .into_iter()
            .take(3)
            .map(|(_, why)| why)
            .collect::<Vec<_>>()
            .join("  -  ")
    };
    if let Ok(mut text) = whys.single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }

    let standing_trees = trees.iter().filter(|t| t.harvestable()).count();
    let fresh = format!(
        "{standing_trees} trees standing  -  {} wild things  -  {} at rest in the ground",
        wildlife.iter().count(),
        graves.iter().count(),
    );
    if let Ok(mut text) = gauges.p3().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }
}

/// Rebuilds the DETAILS wells on a slow clock while the codex is open: every
/// standing building by kind, and every trade by its head-count.
#[allow(clippy::type_complexity)]
pub(crate) fn update_ledger_details(
    mut commands: Commands,
    codex: Res<Codex>,
    time: Res<Time>,
    mut last_rebuild: Local<f32>,
    mut fingerprint: Local<u64>,
    panels: Query<&Visibility, With<VillagePanel>>,
    building_wells: Query<Entity, With<BuildingRows>>,
    trade_wells: Query<Entity, With<TradeRows>>,
    buildings: Query<&crate::villager::work::Building>,
    pending: Query<(), With<crate::villager::work::ConstructionSite>>,
    trades: Query<
        &crate::villager::work::Vocation,
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
) {
    if codex.page != CodexPage::Ledger || !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    *last_rebuild += time.delta_secs();
    if *last_rebuild < 2.0 {
        return;
    }
    *last_rebuild = 0.0;
    let (Ok(building_well), Ok(trade_well)) = (building_wells.single(), trade_wells.single())
    else {
        return;
    };

    let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
    for building in &buildings {
        *kinds.entry(building.kind.name()).or_default() += 1;
    }
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for vocation in &trades {
        *counts.entry(vocation.describe()).or_default() += 1;
    }
    let rising = pending.iter().count();
    // Rebuild only when the content itself changes: tearing the rows down
    // on a timer made the whole page flinch every two seconds.
    let fresh = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        for entry in &kinds {
            entry.hash(&mut hasher);
        }
        for entry in &counts {
            entry.hash(&mut hasher);
        }
        rising.hash(&mut hasher);
        hasher.finish()
    };
    if fresh == *fingerprint {
        return;
    }
    *fingerprint = fresh;
    commands.entity(building_well).despawn_related::<Children>();
    for (name, count) in &kinds {
        let value = ui::ruled_row(&mut commands, building_well, name);
        commands.entity(value).insert(Text::new(format!("{count}")));
    }
    if rising > 0 {
        commands.spawn((
            ui::dim(format!("{rising} under construction")),
            ChildOf(building_well),
        ));
    }
    if kinds.is_empty() && rising == 0 {
        commands.spawn((
            ui::dim("nothing yet stands but the banner"),
            ChildOf(building_well),
        ));
    }

    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by_key(|(name, count)| (std::cmp::Reverse(*count), *name));
    commands.entity(trade_well).despawn_related::<Children>();
    for (name, count) in ranked {
        let value = ui::ruled_row(&mut commands, trade_well, name);
        commands.entity(value).insert(Text::new(format!("{count}")));
    }
}

/// THE PRAYERS page: the prayer board. Every open prayer in the world in
/// the praying's own words, and what lately became of the closed ones.
/// Brett: "It should work almost like a quest board. So the prayer should
/// come up there even if I dont see it." Its own page — the first cut
/// squatted on the Deity page, on the strength of a spawn-time grep that
/// missed the god panel building its home at runtime, and the two crushed
/// each other.
fn build_prayers_page(commands: &mut Commands, page: Entity) {
    let leaf = ordo::page(commands, page, RHYTHM);
    commands.spawn((
        ui::dim("press a prayer to fly to whoever is asking"),
        ChildOf(leaf.header),
    ));
    commands.spawn((
        PrayerRows,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(RHYTHM),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ui::Scrollable,
        ScrollPosition::DEFAULT,
        Interaction::default(),
        ChildOf(leaf.body),
    ));

    // The receipts, in a short fixed band under the board: answered,
    // unanswered, and the dark third kind.
    let lately = ui::card_well(commands, leaf.footer, "LATELY");
    commands.entity(lately).insert(Node {
        width: percent(100),
        height: px(168),
        flex_grow: 0.0,
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        padding: UiRect::all(px(10)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(0)),
        overflow: Overflow::clip(),
        ..default()
    });
    commands.spawn((
        PrayerHistoryRows,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ui::Scrollable,
        ScrollPosition::DEFAULT,
        Interaction::default(),
        ChildOf(lately),
    ));
}

/// How firmly hope still holds, in coarse words — coarse ON PURPOSE: a
/// countdown ticking every second forces the rows to rebuild every second,
/// and a page that flinches on a timer is the exact fault the ledger's
/// fingerprint idiom exists to prevent.
pub(crate) fn hope_band(remaining: f32) -> &'static str {
    if remaining > 60.0 {
        "hope holds"
    } else if remaining > 20.0 {
        "hope fading"
    } else {
        "the last moments"
    }
}

/// Rebuilds the prayer board on a slow clock while the Deity page is open.
#[allow(clippy::type_complexity)]
pub(crate) fn update_prayer_board(
    mut commands: Commands,
    codex: Res<Codex>,
    time: Res<Time>,
    mut last_rebuild: Local<f32>,
    mut fingerprint: Local<u64>,
    panels: Query<&Visibility, With<VillagePanel>>,
    boards: Query<Entity, With<PrayerRows>>,
    histories: Query<Entity, With<PrayerHistoryRows>>,
    portraits: Res<super::portrait::Portraits>,
    ledger: Res<crate::villager::belief::PrayerLedger>,
    praying: Query<
        (
            Entity,
            &Person,
            &crate::villager::belief::Prayer,
            Option<&crate::villager::work::Vocation>,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
) {
    if codex.page != CodexPage::Prayers || !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    *last_rebuild += time.delta_secs();
    if *last_rebuild < 1.0 {
        return;
    }
    *last_rebuild = 0.0;
    let (Ok(board), Ok(history)) = (boards.single(), histories.single()) else {
        return;
    };

    // Most urgent first: the board is a to-do list, and the top of a to-do
    // list is the thing about to be lost.
    let mut open: Vec<_> = praying.iter().collect();
    open.sort_by(|a, b| {
        a.2.remaining
            .partial_cmp(&b.2.remaining)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let fresh = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        for (entity, _, prayer, _) in &open {
            entity.to_bits().hash(&mut hasher);
            hope_band(prayer.remaining).hash(&mut hasher);
        }
        ledger.closed.len().hash(&mut hasher);
        hasher.finish()
    };
    if fresh == *fingerprint {
        return;
    }
    *fingerprint = fresh;

    let pink = crate::palette::shade(&crate::palette::CLOTH_PINK, 1.0);
    commands.entity(board).despawn_related::<Children>();
    if open.is_empty() {
        commands.spawn((ui::dim("nobody is asking anything of you"), ChildOf(board)));
    }
    // Three prayers to a row, on the page grid: every card's edges land
    // on the same three verticals as every other chapter's. Brett: "the
    // prayers can be more polished with the name of the person, their
    // portrait and the prayer and stuff better laid out."
    for third in open.chunks(3) {
        let grid_line = commands
            .spawn((ordo::grid_row(RHYTHM), ChildOf(board)))
            .id();
        commands
            .entity(grid_line)
            .entry::<Node>()
            .and_modify(|mut node| {
                node.flex_shrink = 0.0;
            });
        for (who, person, prayer, vocation) in third {
            let card = commands
                .spawn((
                    PrayerRow(*who),
                    ordo::col(1, RHYTHM),
                    Interaction::default(),
                    ui::HoverHint::new(&person.name, "press to fly to them"),
                    BackgroundColor(Color::BLACK.with_alpha(0.32)),
                    BorderColor::all(pink.with_alpha(0.55)),
                    ChildOf(grid_line),
                ))
                .id();
            commands
                .entity(card)
                .entry::<Node>()
                .and_modify(|mut node| {
                    node.row_gap = px(6);
                    node.padding = UiRect::all(px(10));
                    node.border = UiRect::all(px(2));
                    node.border_radius = BorderRadius::all(px(8));
                });

            // The head: the TRUE portrait in a frame of their trade's own
            // colour, the name beside it, hope in the corner. The studio's
            // stand-in bust holds the frame until their sitting comes up.
            let head = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(8),
                        ..default()
                    },
                    ChildOf(card),
                ))
                .id();
            let livery = vocation
                .map(|trade| crate::villager::attire::livery(*trade).cloth)
                .map(|tone| crate::palette::color_at(tone.palette_index()))
                .unwrap_or_else(ui::theme::text_dim);
            let bust = commands
                .spawn((
                    Node {
                        width: px(34),
                        height: px(34),
                        flex_shrink: 0.0,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(livery.with_alpha(0.16)),
                    BorderColor::all(livery.with_alpha(0.9)),
                    ChildOf(head),
                ))
                .id();
            super::portrait::set_the_face(
                &mut commands,
                bust,
                &portraits,
                *who,
                livery.with_alpha(0.9),
            );
            let names = commands
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    ChildOf(head),
                ))
                .id();
            commands.spawn((ui::body(person.name.clone()), ChildOf(names)));
            commands.spawn((
                ui::dim(
                    vocation
                        .map(|trade| trade.describe().to_string())
                        .unwrap_or_else(|| "of the village".to_string()),
                ),
                ChildOf(names),
            ));
            commands.spawn((ui::dim(hope_band(prayer.remaining)), ChildOf(head)));

            // The asking, and their own words under it.
            commands.spawn((ui::body(prayer.kind.ask_line(&person.name)), ChildOf(card)));
            if let Some(words) = &prayer.words {
                let quoted = commands
                    .spawn((ui::dim(format!("\u{201c}{words}\u{201d}")), ChildOf(card)))
                    .id();
                commands
                    .entity(quoted)
                    .insert(TextColor(pink.with_alpha(0.8)));
            }

            // The kind, sealed in its own colour at the card's foot.
            let (kind_word, kind_color) = match &prayer.kind {
                crate::villager::belief::PrayerKind::Food => ("BREAD", ui::theme::accent()),
                crate::villager::belief::PrayerKind::Dark { .. } => (
                    "WRATH",
                    crate::palette::shade(&crate::palette::CLOTH_WINE, 0.8),
                ),
                crate::villager::belief::PrayerKind::Road { .. } => (
                    "THE ROAD",
                    crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.85),
                ),
                crate::villager::belief::PrayerKind::Devotion { .. } => ("DEVOTION", pink),
            };
            let foot = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        margin: UiRect::top(auto()),
                        ..default()
                    },
                    ChildOf(card),
                ))
                .id();
            let seal = commands
                .spawn((
                    Node {
                        padding: UiRect::axes(px(7), px(2)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(999)),
                        ..default()
                    },
                    BackgroundColor(kind_color.with_alpha(0.14)),
                    BorderColor::all(kind_color.with_alpha(0.85)),
                    ChildOf(foot),
                ))
                .id();
            let seal_word = commands.spawn((ui::dim(kind_word), ChildOf(seal))).id();
            commands
                .entity(seal_word)
                .insert(TextColor(kind_color.with_alpha(0.95)));
            commands.spawn((ui::dim("press to fly"), ChildOf(foot)));
        }
        // Empty tracts hold the seams where cards are missing.
        for _ in third.len()..3 {
            commands.spawn((ordo::col(1, RHYTHM), ChildOf(grid_line)));
        }
    }

    commands.entity(history).despawn_related::<Children>();
    if ledger.closed.is_empty() {
        commands.spawn((ui::dim("no prayer has closed yet"), ChildOf(history)));
    }
    for closed in ledger.closed.iter().rev() {
        let line = commands
            .spawn((
                ui::dim(format!("{} - {}", closed.name, closed.outcome.describe())),
                Interaction::default(),
                ChildOf(history),
            ))
            .id();
        // What they asked, kept a hover away rather than crowding the strip.
        if let Some(words) = &closed.words {
            commands.entity(line).insert(ui::HoverHint::new(
                &closed.name,
                format!("\u{201c}{words}\u{201d}"),
            ));
        }
    }
}

/// Pressing a prayer flies the god to whoever is asking: the codex closes
/// and the camera pins to them — the same follow a right-click takes —
/// with the orbit and zoom left free for the answering.
pub(crate) fn answer_the_board(
    mut panels: Query<&mut Visibility, With<VillagePanel>>,
    askers: Query<&Transform>,
    mut rigs: Query<&mut crate::camera::CameraRig>,
    rows: Query<(&Interaction, &PrayerRow), Changed<Interaction>>,
) {
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // The camera goes TO them and is handed straight back - a jump
        // and a dive to answering height, never a lock. Brett: "it should
        // zoom in on the person, but it shouldnt lock on." The god flies
        // in to answer, then the world is theirs to drag again.
        let (Ok(asker), Ok(mut rig)) = (askers.get(row.0), rigs.single_mut()) else {
            continue;
        };
        rig.target_focus.x = asker.translation.x;
        rig.target_focus.z = asker.translation.z;
        rig.target_distance = 22.0;
        for mut panel in &mut panels {
            *panel = Visibility::Hidden;
        }
    }
}

/// The FAITH roster: rows are torn down only when someone joins or leaves
/// the roll. When souls merely trade places in the ranking, the standing
/// rows are reordered in place - despawning them cost a layout frame and
/// jolted the scroll every time trust drifted past a neighbour. The
/// drifting numbers and why-lines always update in place.
#[allow(clippy::type_complexity)]
pub(crate) fn update_faith_roster(
    mut commands: Commands,
    codex: Res<Codex>,
    time: Res<Time>,
    mut last_rebuild: Local<f32>,
    mut fingerprints: Local<(u64, u64)>,
    panels: Query<&Visibility, With<VillagePanel>>,
    rosters: Query<Entity, With<FaithRoster>>,
    rows: Query<(Entity, &FaithRow)>,
    mut lines: Query<(&FaithText, &mut Text)>,
    flock: Query<
        (
            Entity,
            &Person,
            Option<&crate::villager::belief::Faith>,
            Option<&Chronicle>,
            Option<&Witnessed>,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
) {
    if codex.page != CodexPage::Ledger || !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    *last_rebuild += time.delta_secs();
    if *last_rebuild < 2.0 {
        return;
    }
    *last_rebuild = 0.0;
    let Ok(roster) = rosters.single() else {
        return;
    };

    let compose = |field: u8,
                   person: &Person,
                   faith: Option<&crate::villager::belief::Faith>,
                   chronicle: Option<&Chronicle>,
                   witnessed: Option<&Witnessed>|
     -> String {
        match field {
            0 => {
                let believer = faith.is_some_and(|f| f.is_believer());
                format!("{}{}", person.name, if believer { "  *" } else { "" })
            }
            1 => format!(
                "{}  ({:.0}%)",
                faith.map_or("has never wondered", |f| f.describe()),
                faith.map_or(0.0, |f| f.trust) * 100.0,
            ),
            _ => chronicle
                .and_then(|c| {
                    c.events.iter().rev().find(|e| {
                        e.text.contains("saw")
                            || e.text.contains("heard")
                            || e.text.contains("prayed")
                            || e.text.contains("answered")
                            || e.text.contains("believe")
                    })
                })
                .map(|e| format!("   d{}  {}", e.day, e.text))
                .unwrap_or_else(|| match witnessed {
                    Some(w) if w.secondhand > 0 => "   knows the god only from stories".to_string(),
                    _ => "   has neither seen nor heard of you".to_string(),
                }),
        }
    };

    let mut souls: Vec<_> = flock.iter().collect();
    souls.sort_by(|a, b| {
        let fa = a.2.map_or(0.0, |f| f.trust);
        let fb = b.2.map_or(0.0, |f| f.trust);
        fb.total_cmp(&fa)
    });

    // Two fingerprints: who is on the roll at all, and in what order.
    let (membership, order) = {
        use std::hash::{Hash, Hasher};
        let mut order_hasher = std::hash::DefaultHasher::new();
        for (entity, ..) in &souls {
            entity.hash(&mut order_hasher);
        }
        let mut names: Vec<Entity> = souls.iter().map(|(entity, ..)| *entity).collect();
        names.sort();
        let mut membership_hasher = std::hash::DefaultHasher::new();
        names.hash(&mut membership_hasher);
        (membership_hasher.finish(), order_hasher.finish())
    };

    if membership != fingerprints.0 {
        *fingerprints = (membership, order);
        commands.entity(roster).despawn_related::<Children>();
        for (entity, person, faith, chronicle, witnessed) in &souls {
            let row = commands
                .spawn((
                    FaithRow(*entity),
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(5),
                        ..default()
                    },
                    ChildOf(roster),
                ))
                .id();
            let name_line = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    },
                    ChildOf(row),
                ))
                .id();
            commands.spawn((
                FaithText {
                    person: *entity,
                    field: 0,
                },
                ui::body(compose(0, person, *faith, *chronicle, *witnessed)),
                ChildOf(name_line),
            ));
            commands.spawn((
                FaithText {
                    person: *entity,
                    field: 1,
                },
                ui::dim(compose(1, person, *faith, *chronicle, *witnessed)),
                ChildOf(name_line),
            ));
            commands.spawn((
                FaithText {
                    person: *entity,
                    field: 2,
                },
                ui::dim(compose(2, person, *faith, *chronicle, *witnessed)),
                ChildOf(row),
            ));
        }
        return;
    }

    if order != fingerprints.1 {
        fingerprints.1 = order;
        let standing: std::collections::HashMap<Entity, Entity> =
            rows.iter().map(|(row, mark)| (mark.0, row)).collect();
        let ordered: Vec<Entity> = souls
            .iter()
            .filter_map(|(entity, ..)| standing.get(entity).copied())
            .collect();
        commands.entity(roster).replace_children(&ordered);
    }

    // The quiet path: the rows stand; only the words that drift are set.
    for (line, mut text) in &mut lines {
        let Ok((_, person, faith, chronicle, witnessed)) = flock.get(line.person) else {
            continue;
        };
        let fresh = compose(line.field, person, faith, chronicle, witnessed);
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}

/// A swatch on the settings page, holding its place in
/// [`crate::hand::HAND_STYLES`].
#[derive(Component)]
pub(crate) struct HandSwatch(pub usize);

/// The button that trades the mouse's two hands, and the line that says
/// which way they stand. See [`crate::keymap::MouseScheme`].
#[derive(Component)]
pub(crate) struct MouseSchemeButton;

#[derive(Component)]
pub(crate) struct MouseSchemeLabel;

/// Flips the scheme on a press and keeps the label honest.
pub(crate) fn swap_mouse_buttons(
    buttons: Query<&Interaction, (Changed<Interaction>, With<MouseSchemeButton>)>,
    mut mouse: ResMut<crate::keymap::MouseScheme>,
    keymap: Res<crate::keymap::Keymap>,
    mut labels: Query<&mut Text, With<MouseSchemeLabel>>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            mouse.reversed = !mouse.reversed;
            crate::keymap::save(&keymap, &mouse);
        }
    }
    // Rewritten every frame it might have changed; two short strings.
    if mouse.is_changed() {
        for mut label in &mut labels {
            label.0 = format!(
                "{} grabs the land - {} picks up and works",
                mouse.land_name(),
                mouse.action_name()
            );
        }
    }
}

/// The line naming the style the hand wears now.
#[derive(Component)]
pub(crate) struct HandStyleName;

/// One row on the keybinds page: a deed whose key the player may change,
/// or a fitting that stays where it is (the mouse, escape, the workbench).
enum Bind {
    Deed(Deed, &'static str),
    Fixed(&'static [&'static str], &'static str),
}

/// Every key the game answers to, in the player's tongue. One table, so
/// the page never drifts from the truth; the keys themselves live in the
/// [`crate::keymap::Keymap`], which is the single word on what does what.
const KEYBINDS: &[(&str, &[Bind])] = &[
    (
        "THE CAMERA",
        &[
            Bind::Deed(Deed::PanNorth, "glide north; the arrows serve too"),
            Bind::Deed(Deed::PanSouth, "glide south"),
            Bind::Deed(Deed::PanWest, "glide west"),
            Bind::Deed(Deed::PanEast, "glide east"),
            Bind::Deed(Deed::TurnLeft, "swing left around the place you watch"),
            Bind::Deed(Deed::TurnRight, "swing right"),
            Bind::Fixed(&["right drag"], "swing and tilt by hand"),
            Bind::Fixed(&["middle drag"], "pull the map beneath you"),
            Bind::Fixed(&["scroll"], "draw near, or pull away"),
        ],
    ),
    (
        "TIME",
        &[
            Bind::Deed(Deed::Pause, "hold the world still; let it go again"),
            Bind::Deed(Deed::Slower, "let the days walk"),
            Bind::Deed(Deed::Faster, "make the days run"),
        ],
    ),
    (
        "MIRACLES",
        &[
            Bind::Deed(
                Deed::Slot1,
                "fire hotbar slot one, wherever the hand points",
            ),
            Bind::Deed(Deed::Slot2, "fire hotbar slot two"),
            Bind::Deed(Deed::Slot3, "fire hotbar slot three"),
            Bind::Deed(Deed::Slot4, "fire hotbar slot four"),
            Bind::Deed(Deed::Slot5, "fire hotbar slot five"),
            Bind::Deed(Deed::Slot6, "fire hotbar slot six"),
            Bind::Deed(Deed::Slot7, "fire hotbar slot seven"),
            Bind::Deed(Deed::Slot8, "fire hotbar slot eight"),
            Bind::Deed(Deed::Slot9, "fire hotbar slot nine"),
            Bind::Deed(Deed::Slot10, "fire hotbar slot ten"),
            Bind::Fixed(&["click"], "arm a miracle; click the world to work it"),
            Bind::Fixed(&["drag"], "carry a miracle to another slot"),
            Bind::Fixed(&["right click", "Esc"], "set an armed miracle aside"),
        ],
    ),
    (
        "THE GOD'S SIGHT",
        &[
            Bind::Deed(Deed::Codex, "open and shut this codex"),
            Bind::Deed(Deed::Markers, "mark every soul"),
            Bind::Deed(
                Deed::Survey,
                "the surveyor's sight: woods, stone, clay, iron, wild food",
            ),
            Bind::Deed(
                Deed::Roofs,
                "the cutaway: whole, roof off,
walls down as well",
            ),
            Bind::Deed(
                Deed::Doings,
                "nameplates: off, names,
then the whole soul",
            ),
            Bind::Deed(Deed::Fog, "only the ground your people know"),
        ],
    ),
    (
        "THE WORKBENCH",
        &[
            Bind::Fixed(&["`"], "the frame counter"),
            Bind::Fixed(&["F1"], "the tuner's panel"),
            Bind::Fixed(&["F2", "F3"], "open and close the lens"),
            Bind::Fixed(&["F4", "F5"], "pull the focus near and far"),
            Bind::Fixed(&["F6", "F7"], "thicken and thin the haze"),
            Bind::Fixed(&["F8", "F9"], "darken and brighten the light"),
            Bind::Fixed(&["F10"], "the miniature look, on and off"),
            Bind::Fixed(&["F11", "shift F11"], "richer and paler colour"),
            Bind::Fixed(&["F12"], "a photograph, saved beside the game"),
        ],
    ),
];

/// The keycap button of a rebindable deed on the settings page.
#[derive(Component)]
pub(crate) struct BindButton(pub Deed);

/// The text inside a bind button's cap, kept true to the keymap.
#[derive(Component)]
pub(crate) struct BindCap(pub Deed);

/// The button that puts every key back where it started.
#[derive(Component)]
pub(crate) struct ResetBinds;

/// Which deed, if any, is waiting for its new key.
#[derive(Resource, Default)]
pub(crate) struct Rebinding(pub Option<Deed>);

/// Lays out the settings page in the codex's own manner: a tab bar like the
/// People page wears, and card wells like the ledger's grid. Keybinds stand
/// first — the page a player actually comes here for — then the hand's
/// colour, brought in from the title screen, then what little video and
/// sound there is to speak of. The model picker is gone; the teller keeps
/// its voice from the models folder without asking.
/// Builds the settings, into whatever parent is handed to it.
///
/// `pub(crate)` because there is ONE settings panel in this game and the title
/// screen shows that one. It used to have a little settings screen of its own —
/// hand colours and nothing else — which meant two places to add a setting to
/// and two chances to forget the second. This is the panel; the title just hosts
/// it in a different frame.
pub(crate) fn build_settings_page(commands: &mut Commands, page: Entity) {
    let tabs = ui::tab_bar(
        commands,
        page,
        &["KEYBINDS", "THE HAND", "VIDEO & SOUND", "THE VIEW"],
    );
    for (index, tab_page) in tabs.iter().copied().enumerate() {
        commands.entity(tab_page).insert((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ui::Scrollable,
            bevy::ui::ScrollPosition::default(),
        ));
        if index != 0 {
            commands
                .entity(tab_page)
                .entry::<Node>()
                .and_modify(|mut node| {
                    node.display = Display::None;
                });
        }
    }

    let row_of = |commands: &mut Commands, parent: Entity| -> Entity {
        commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(10),
                    align_items: AlignItems::Stretch,
                    ..default()
                },
                ChildOf(parent),
            ))
            .id()
    };

    // KEYBINDS: the table above, dealt into two rows of card wells.
    let keycap = |commands: &mut Commands, parent: Entity, cap: &str| {
        let key = commands
            .spawn((
                Node {
                    min_width: px(26),
                    padding: UiRect::axes(px(7), px(2)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.35)),
                BorderColor::all(ui::theme::panel_border().with_alpha(0.8)),
                ChildOf(parent),
            ))
            .id();
        // The machine's own face, kept deliberately: keycaps set in plain
        // type read as keys, the way manuals have always done it.
        commands.spawn((
            Text::new(cap.to_string()),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(ui::theme::accent().with_alpha(0.95)),
            ChildOf(key),
        ));
    };
    commands.spawn((
        ui::dim(
            "press a cap, then the key you would rather use; a key already \
             spoken for trades places. escape thinks better of it.",
        ),
        ChildOf(tabs[0]),
    ));
    let top = row_of(commands, tabs[0]);
    let bottom = row_of(commands, tabs[0]);
    for (index, (title, binds)) in KEYBINDS.iter().enumerate() {
        let card = ui::card_well(commands, if index < 3 { top } else { bottom }, title);
        for bind in binds.iter() {
            let row = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(8),
                        ..default()
                    },
                    ChildOf(card),
                ))
                .id();
            let hands = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(4),
                        flex_shrink: 0.0,
                        min_width: px(96),
                        ..default()
                    },
                    ChildOf(row),
                ))
                .id();
            let tale = match bind {
                Bind::Deed(deed, tale) => {
                    let cap = commands
                        .spawn((
                            BindButton(*deed),
                            ui::UiButton,
                            ui::KeepFace,
                            ui::HoverHint::new(
                                "rebind",
                                "press the cap, then the key you would rather use",
                            ),
                            Interaction::default(),
                            Node {
                                min_width: px(34),
                                padding: UiRect::axes(px(7), px(2)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(px(1)),
                                border_radius: BorderRadius::all(px(4)),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            BackgroundColor(Color::BLACK.with_alpha(0.35)),
                            BorderColor::all(ui::theme::panel_border().with_alpha(0.8)),
                            ChildOf(hands),
                        ))
                        .id();
                    commands.spawn((
                        BindCap(*deed),
                        Text::new(String::new()),
                        TextFont {
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(ui::theme::accent().with_alpha(0.95)),
                        ChildOf(cap),
                    ));
                    tale
                }
                Bind::Fixed(caps, tale) => {
                    for cap in caps.iter() {
                        keycap(commands, hands, cap);
                    }
                    tale
                }
            };
            commands.spawn((
                ui::dim(*tale),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                ChildOf(row),
            ));
        }
    }
    // The foot of the page: every key back where it started.
    let reset = commands
        .spawn((
            ResetBinds,
            ui::UiButton,
            Interaction::default(),
            Node {
                align_self: AlignSelf::FlexStart,
                padding: UiRect::axes(px(14), px(6)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(ui::theme::panel_border().with_alpha(0.4)),
            ChildOf(tabs[0]),
        ))
        .id();
    commands.spawn((
        Text::new("RESTORE THE OLD WAYS"),
        ui::DisplayFace,
        TextFont {
            font_size: FontSize::Px(ui::theme::SMALL_SIZE),
            ..default()
        },
        TextColor(ui::theme::accent()),
        ChildOf(reset),
    ));

    // THE HAND: the swatches from the title's settings, now also living in
    // the book. The hand itself previews every press.
    let hand_row = row_of(commands, tabs[1]);
    let hand_card = ui::card_well(commands, hand_row, "THE HAND");
    commands.spawn((
        ui::dim("the colour of the hand that works your will."),
        ChildOf(hand_card),
    ));
    let swatches = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                margin: UiRect::top(px(4)),
                ..default()
            },
            ChildOf(hand_card),
        ))
        .id();
    for (index, (name, ramp)) in crate::hand::HAND_STYLES.iter().enumerate() {
        commands.spawn((
            HandSwatch(index),
            ui::UiButton,
            ui::KeepFace,
            ui::HoverHint::new(*name, "the hand, restyled"),
            Node {
                width: px(40),
                height: px(40),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(crate::palette::shade(ramp, 0.9)),
            BorderColor::all(ui::theme::panel_border()),
            Interaction::default(),
            ChildOf(swatches),
        ));
    }
    commands.spawn((
        HandStyleName,
        ui::body(""),
        Node {
            margin: UiRect::top(px(2)),
            ..default()
        },
        ChildOf(hand_card),
    ));

    // The mouse, beside the hand it drives. Black and White's own controls -
    // left grabs the land, right is the hand - and one switch for anyone whose
    // reflexes were trained by the sequel, which dealt them the other way
    // round. Brett: "use B&W 1 controls but let people reverse the mouse
    // buttons in the settings".
    let mouse_card = ui::card_well(commands, hand_row, "THE MOUSE");
    commands.spawn((
        ui::dim("left hand on the world, right hand at work."),
        ChildOf(mouse_card),
    ));
    let mouse_row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10),
                margin: UiRect::top(px(6)),
                ..default()
            },
            ChildOf(mouse_card),
        ))
        .id();
    commands.spawn((
        MouseSchemeButton,
        ui::UiButton,
        ui::KeepFace,
        ui::HoverHint::new("swap the buttons", "for hands the sequel taught"),
        Node {
            padding: UiRect::axes(px(12), px(6)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BackgroundColor(ui::theme::title_bg()),
        BorderColor::all(ui::theme::panel_border().with_alpha(0.5)),
        Interaction::default(),
        ChildOf(mouse_row),
    ));
    commands.spawn((MouseSchemeLabel, ui::body(""), ChildOf(mouse_row)));

    // VIDEO & SOUND: spoken for honestly, small as they are.
    let av_row = row_of(commands, tabs[2]);
    let video = ui::card_well(commands, av_row, "VIDEO");
    commands.spawn((
        ui::dim(
            "the world draws as fast as your glass allows. the lens is tuned \
             from the workbench keys, and a photograph falls from F12.",
        ),
        Node {
            max_width: px(430),
            ..default()
        },
        ChildOf(video),
    ));
    let sound = ui::card_well(commands, av_row, "SOUND");
    commands.spawn((
        ui::dim(
            "three airs, by the hour: the title's theme, the hearth by day, the presence by night.",
        ),
        Node {
            max_width: px(430),
            ..default()
        },
        ChildOf(sound),
    ));
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10),
                margin: UiRect::top(px(6)),
                ..default()
            },
            ChildOf(sound),
        ))
        .id();
    let nudge = |commands: &mut Commands, glyph: &str, step: i8| {
        let button = commands
            .spawn((
                NudgeMusic(step),
                ui::UiButton,
                Interaction::default(),
                Node {
                    padding: UiRect::axes(px(10), px(4)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(7)),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.18)),
                BorderColor::all(ui::theme::panel_border().with_alpha(0.5)),
                ChildOf(row),
            ))
            .id();
        commands.spawn((
            Text::new(glyph.to_string()),
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(ui::theme::text()),
            ChildOf(button),
        ));
    };
    nudge(commands, "<", -1);
    commands.spawn((MusicVolumeText, ui::body(""), ChildOf(row)));
    nudge(commands, ">", 1);

    // THE VIEW: switches for what the world shows, which is the fourth tab
    // rather than four more letters on the keyboard. A hotkey is for something
    // reached for mid-thought; none of these are.
    crate::title::build_view_switches(commands, tabs[3]);
}

/// The music volume readout on the sound card.
#[derive(Component)]
pub(crate) struct MusicVolumeText;

/// A button that steps the music volume down (-1) or up (+1).
#[derive(Component)]
pub(crate) struct NudgeMusic(pub i8);

/// The sound card at work: presses walk the volume in tenths, the readout
/// stays true, and the choice is written down beside the saves.
pub(crate) fn sound_panel(
    mut volume: ResMut<crate::music::MusicVolume>,
    buttons: Query<(&Interaction, &NudgeMusic), Changed<Interaction>>,
    mut readouts: Query<&mut Text, With<MusicVolumeText>>,
) {
    for (interaction, nudge) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        volume.0 = (volume.0 + nudge.0 as f32 * 0.1).clamp(0.0, 1.0);
        crate::music::save_volume(&volume);
    }
    let fresh = if volume.0 <= 0.0 {
        "music - silent".to_string()
    } else {
        format!("music - {:.0}%", volume.0 * 100.0)
    };
    for mut text in &mut readouts {
        if text.0 != fresh {
            *text = Text::new(fresh.clone());
        }
    }
}

/// Presses on the hand swatches restyle the hand, mid-game; the chosen
/// swatch wears the gold border, and the style's name stands beneath the
/// row. The same cloth as the title's settings, cut for the codex.
pub(crate) fn settings_panel(
    style: Option<ResMut<crate::hand::HandStyle>>,
    mut swatches: Query<(&Interaction, &HandSwatch, &mut BorderColor)>,
    mut names: Query<&mut Text, With<HandStyleName>>,
) {
    let Some(mut style) = style else {
        return;
    };
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
        let dress = BorderColor::all(if chosen {
            ui::theme::accent()
        } else {
            ui::theme::panel_border()
        });
        if *border != dress {
            *border = dress;
        }
    }
    if let Some((name, _)) = crate::hand::HAND_STYLES
        .iter()
        .find(|(_, ramp)| style.ramp == *ramp)
    {
        let fresh = format!("the hand wears {name}.");
        for mut text in &mut names {
            if text.0 != fresh {
                *text = Text::new(fresh.clone());
            }
        }
    }
}

/// The keybinds page at work: cap presses arm a rebind, the reset button
/// restores the defaults, and every cap and border is kept true.
pub(crate) fn keybind_panel(
    mut arming: ResMut<Rebinding>,
    mut keymap: ResMut<crate::keymap::Keymap>,
    mouse: Res<crate::keymap::MouseScheme>,
    clicked: Query<(&Interaction, &BindButton), Changed<Interaction>>,
    reset: Query<&Interaction, (Changed<Interaction>, With<ResetBinds>)>,
    tabs: Query<&Interaction, (Changed<Interaction>, With<ui::TabButton>)>,
    mut caps: Query<(&BindCap, &mut Text)>,
    mut borders: Query<(&BindButton, &mut BorderColor)>,
) {
    // Walking to another tab stands an armed rebind down - a cap left
    // listening behind a hidden page would eat the next key in silence.
    if tabs.iter().any(|t| *t == Interaction::Pressed) {
        arming.0 = None;
    }
    for (interaction, bind) in &clicked {
        if *interaction == Interaction::Pressed {
            // Pressing the cap that is already listening stands it down.
            arming.0 = if arming.0 == Some(bind.0) {
                None
            } else {
                Some(bind.0)
            };
        }
    }
    for interaction in &reset {
        if *interaction == Interaction::Pressed {
            keymap.restore_defaults();
            crate::keymap::save(&keymap, &mouse);
            arming.0 = None;
        }
    }
    for (cap, mut text) in &mut caps {
        let fresh = if arming.0 == Some(cap.0) {
            "...".to_string()
        } else {
            crate::keymap::key_name(keymap.key(cap.0))
                .unwrap_or("?")
                .to_string()
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
    for (bind, mut border) in &mut borders {
        let dress = BorderColor::all(if arming.0 == Some(bind.0) {
            ui::theme::accent()
        } else {
            ui::theme::panel_border().with_alpha(0.8)
        });
        if *border != dress {
            *border = dress;
        }
    }
}

/// Catches the new key while a rebind is armed. Runs in `PreUpdate`, right
/// after the input arrives, and eats the press - a key given to the map
/// must not also do its old work on the way in.
pub(crate) fn catch_rebind(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut arming: ResMut<Rebinding>,
    mut keymap: ResMut<crate::keymap::Keymap>,
    mouse: Res<crate::keymap::MouseScheme>,
    codex: Option<Res<Codex>>,
    panels: Query<&Visibility, With<VillagePanel>>,
) {
    let Some(deed) = arming.0 else {
        return;
    };
    // The book closed, or turned to another page, mid-listen: stand down.
    let listening = codex.is_some_and(|codex| codex.page == CodexPage::Settings)
        && panels.iter().any(|seen| *seen != Visibility::Hidden);
    if !listening {
        arming.0 = None;
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        arming.0 = None;
        keys.clear_just_pressed(KeyCode::Escape);
        return;
    }
    let Some(key) = keys
        .get_just_pressed()
        .copied()
        .find(|key| crate::keymap::key_name(*key).is_some())
    else {
        return;
    };
    keymap.bind(deed, key);
    crate::keymap::save(&keymap, &mouse);
    arming.0 = None;
    // The whole press is eaten, held state included - a pan key caught
    // here must not also glide the world behind the book while held.
    keys.reset(key);
}
