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
    pub world_page: Entity,
    pub ledger_tab: Entity,
    pub people_tab: Entity,
    pub chronicle_tab: Entity,
    pub deity_tab: Entity,
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

/// A scroll: a page bearing written lines.
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
const RHYTHM: f32 = 11.0;

/// The main split band's height. The plates (96) and the land strip (40)
/// ride above and below it with the rhythm between; the sum is the page
/// band.
const MAIN_BAND: f32 = 494.0;

/// Every page's full height: the ledger's bands sum to it (96 + 5 + 501 +
/// 5 + 45), and the people page is pinned to it directly.
const PAGE_BAND: f32 = 652.0;

pub(crate) fn spawn_village_panel(mut commands: Commands) {
    let window = ui::titled_window(
        &mut commands,
        "THE LEDGER",
        Some("The heart of a living world."),
        1160.0,
        // Dead centre, both axes, and it stays there: the codex is the one
        // grand window, not a floating panel to be shuffled about.
        true,
    );
    commands.entity(window.title_bar).remove::<ui::DragHandle>();
    commands.entity(window.root).insert((
        Name::new("Codex Panel"),
        VillagePanel,
        Visibility::Hidden,
        Node {
            width: px(1160),
            flex_direction: FlexDirection::Column,
            padding: px(5).into(),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(0)),
            ..default()
        },
    ));
    // The body breathes in the page's own rhythm on every side.
    commands.entity(window.body).insert(Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        row_gap: px(RHYTHM),
        padding: px(RHYTHM).into(),
        ..default()
    });

    // The pages, as siblings in the body: exactly one shows at a time, and
    // every page's bands sum to the same height, so the book never changes
    // shape when a page turns.
    let ledger_page = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(RHYTHM),
                ..default()
            },
            Visibility::Inherited,
            ChildOf(window.body),
        ))
        .id();
    let mut bound_page = || {
        commands
            .spawn((
                Node {
                    width: percent(100),
                    height: px(PAGE_BAND),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(ui::theme::GAP),
                    display: Display::None,
                    ..default()
                },
                Visibility::Hidden,
                ChildOf(window.body),
            ))
            .id()
    };
    let people_page = bound_page();
    let chronicle_page = bound_page();
    let deity_page = bound_page();
    let world_page = bound_page();
    let settings_page = bound_page();

    // The codex strip: five pages, two residents. Dark tabs are pages still
    // being written.
    let strip = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: px(6),
            ..default()
        })
        .id();
    commands
        .entity(window.title_bar)
        .insert_children(1, &[strip]);
    let tab = |commands: &mut Commands, active: bool, interactive: bool| -> Entity {
        let mut button = commands.spawn((
            Node {
                width: px(54),
                height: px(40),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(if active {
                ui::theme::panel_bg()
            } else {
                Color::BLACK.with_alpha(0.18)
            }),
            BorderColor::all(if active {
                ui::theme::accent().with_alpha(0.85)
            } else {
                ui::theme::panel_border().with_alpha(0.4)
            }),
            ChildOf(strip),
        ));
        if interactive {
            button.insert(Interaction::default());
        }
        button.id()
    };
    let ink = ui::theme::accent();

    let ledger_tab = tab(&mut commands, true, true);
    house_glyph(&mut commands, ledger_tab, ink);
    commands.entity(ledger_tab).insert((
        CodexTab(CodexPage::Ledger),
        ui::HoverHint::new("The Ledger", "the heart of a living world"),
    ));

    let deity_tab = tab(&mut commands, false, true);
    hands_glyph(&mut commands, deity_tab, ink.with_alpha(0.8));
    commands.entity(deity_tab).insert((
        CodexTab(CodexPage::Deity),
        ui::HoverHint::new("The Deity", "you are the unseen; they are the faithful"),
    ));

    let world_tab = tab(&mut commands, false, true);
    tree_glyph(&mut commands, world_tab, ink.with_alpha(0.8));
    commands.entity(world_tab).insert((
        CodexTab(CodexPage::World),
        ui::HoverHint::new("The World", "the lands your people walk; the seasons turn"),
    ));

    let people_tab = tab(&mut commands, false, true);
    people_glyph(&mut commands, people_tab, ink.with_alpha(0.8));
    commands.entity(people_tab).insert((
        CodexTab(CodexPage::People),
        ui::HoverHint::new("The People", "the mortals of your world"),
    ));

    let chronicle_tab = tab(&mut commands, false, true);
    scroll_glyph(&mut commands, chronicle_tab, ink.with_alpha(0.8));
    commands.entity(chronicle_tab).insert((
        CodexTab(CodexPage::Chronicle),
        ui::HoverHint::new(
            "The Chronicle",
            "the tale of your people, written moment by moment",
        ),
    ));

    let settings_tab = tab(&mut commands, false, true);
    sliders_glyph(&mut commands, settings_tab, ink.with_alpha(0.8));
    commands.entity(settings_tab).insert((
        CodexTab(CodexPage::Settings),
        ui::HoverHint::new("The Settings", "the god's own preferences"),
    ));
    build_settings_page(&mut commands, settings_page);

    commands.insert_resource(Codex {
        page: CodexPage::Ledger,
        root: window.root,
        settings_page,
        settings_tab,
        ledger_page,
        people_page,
        chronicle_page,
        deity_page,
        world_page,
        ledger_tab,
        people_tab,
        chronicle_tab,
        deity_tab,
        world_tab,
        title_text: window.title_text,
        subtitle_text: window.subtitle_text,
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
                height: px(MAIN_BAND),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(ledger_page),
        ))
        .id();
    // One integral geometry, shared with the plates row: the rail IS a
    // plate's width, and every seam lands on the same whole pixel.
    let (rail, detail) = ui::split_row(&mut commands, main, 366.0, RHYTHM);

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
pub(crate) fn handle_codex_tabs(
    tabs: Query<(&Interaction, &CodexTab), Changed<Interaction>>,
    codex: Option<ResMut<Codex>>,
) {
    let Some(mut codex) = codex else {
        return;
    };
    for (interaction, tab) in &tabs {
        if *interaction == Interaction::Pressed && codex.page != tab.0 {
            codex.page = tab.0;
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
        (codex.world_tab, codex.page == CodexPage::World),
    ];
    for (tab, open) in tabs {
        if let Ok(mut fill) = fills.get_mut(tab) {
            fill.0 = if open {
                ui::theme::panel_bg()
            } else {
                Color::BLACK.with_alpha(0.18)
            };
        }
        if let Ok(mut border) = borders.get_mut(tab) {
            *border = BorderColor::all(if open {
                ui::theme::accent().with_alpha(0.85)
            } else {
                ui::theme::panel_border().with_alpha(0.4)
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
            Bind::Deed(Deed::Flourish, "flourish: life where you point"),
            Bind::Deed(Deed::Smite, "smite: the storm answers"),
            Bind::Deed(Deed::Bounty, "bounty: food for the stores"),
            Bind::Deed(Deed::MendOrQuake, "mend or quake, once legend unlocks them"),
            Bind::Fixed(&["click"], "work the chosen miracle"),
            Bind::Fixed(&["right click", "Esc"], "set the miracle aside"),
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
            Bind::Deed(Deed::Roofs, "lift the roofs and look inside"),
            Bind::Deed(Deed::Doings, "every soul says what they are at"),
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
fn build_settings_page(commands: &mut Commands, page: Entity) {
    let tabs = ui::tab_bar(commands, page, &["KEYBINDS", "THE HAND", "VIDEO & SOUND"]);
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
            crate::keymap::save(&keymap);
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
    crate::keymap::save(&keymap);
    arming.0 = None;
    // The whole press is eaten, held state included - a pan key caught
    // here must not also glide the world behind the book while held.
    keys.reset(key);
}
