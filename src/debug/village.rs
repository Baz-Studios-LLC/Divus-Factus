//! THE CODEX — the one grand window — and its first resident page: THE
//! LEDGER, the village's whole account at a glance.
//!
//! The codex is the People window's footprint (1160 wide, the same band
//! heights) wearing an icon-tab strip in its title bar: house, wood, temple,
//! faith, people. Today only the house page lives here and the people icon
//! opens the People window where it still stands; each remaining panel
//! migrates in as it is brought up to the People standard, one by one.

use std::collections::BTreeMap;

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

/// Its toolbar button.
#[derive(Component)]
pub(crate) struct VillageButton;

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
}

/// The codex's spine: which page is open, and the handles the page-turn
/// needs — page roots, the two live tabs, and the title texts to rewrite.
#[derive(Resource)]
pub(crate) struct Codex {
    pub page: CodexPage,
    pub root: Entity,
    pub ledger_page: Entity,
    pub people_page: Entity,
    pub ledger_tab: Entity,
    pub people_tab: Entity,
    pub title_text: Entity,
    pub subtitle_text: Option<Entity>,
}

/// A live tab on the codex strip: pressing it turns to its page.
#[derive(Component)]
pub(crate) struct CodexTab(pub CodexPage);

/// The DETAILS page wells, rebuilt on a slow clock while the window is open.
#[derive(Component)]
pub(crate) struct BuildingRows;

#[derive(Component)]
pub(crate) struct TradeRows;

// ---------------------------------------------------------------------------
// Glyphs: little engraved marks drawn from nodes, same hand as everything.
// ---------------------------------------------------------------------------

/// A fixed canvas the glyph bars are placed on, absolutely.
fn glyph_canvas(commands: &mut Commands, parent: Entity, size: f32) -> Entity {
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

fn bar(
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
fn house_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (4.0, 9.0, 10.0, 7.0), 0.0, tint, false);
    bar(commands, c, (1.0, 5.0, 9.0, 2.5), -33.0, tint, false);
    bar(commands, c, (8.0, 5.0, 9.0, 2.5), 33.0, tint, false);
}

/// A tree: the chronicle's mark, at button scale.
fn tree_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (8.0, 11.0, 2.5, 6.0), 0.0, tint, false);
    bar(commands, c, (4.0, 7.5, 10.5, 3.5), 0.0, tint, false);
    bar(commands, c, (5.5, 4.0, 7.5, 3.5), 0.0, tint, false);
    bar(commands, c, (7.0, 1.0, 4.5, 3.0), 0.0, tint, false);
}

/// A temple: lintel, columns, footing.
fn temple_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (2.0, 3.0, 14.0, 2.5), 0.0, tint, false);
    for left in [3.5, 8.0, 12.5] {
        bar(commands, c, (left, 6.5, 2.0, 6.5), 0.0, tint, false);
    }
    bar(commands, c, (2.0, 13.5, 14.0, 2.5), 0.0, tint, false);
}

/// Faith: two bars leant together in prayer.
fn hands_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (4.0, 4.0, 3.0, 11.0), 22.0, tint, false);
    bar(commands, c, (11.0, 4.0, 3.0, 11.0), -22.0, tint, false);
}

/// A person: head over shoulders.
fn person_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    bar(commands, c, (6.5, 2.0, 5.0, 5.0), 0.0, tint, true);
    bar(commands, c, (4.0, 8.5, 10.0, 7.0), 0.0, tint, true);
}

/// A gathering: two souls shoulder to shoulder.
fn people_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
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

/// The village banner: a hung cloth with the tree upon it, flying at the
/// detail pane's shoulder the way the mockup flies it.
fn banner_glyph(commands: &mut Commands, parent: Entity) {
    let cloth = commands
        .spawn((
            Node {
                width: px(56),
                height: px(72),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg()),
            BorderColor::all(ui::theme::accent().with_alpha(0.5)),
            ChildOf(parent),
        ))
        .id();
    // The crossbar it hangs from.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(-5),
            top: px(-4),
            width: px(62),
            height: px(3),
            ..default()
        },
        BackgroundColor(ui::theme::accent().with_alpha(0.8)),
        ChildOf(cloth),
    ));
    // A thread of fringe at its foot.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(4),
            right: px(4),
            bottom: px(4),
            height: px(2),
            ..default()
        },
        BackgroundColor(ui::theme::accent().with_alpha(0.35)),
        ChildOf(cloth),
    ));
    tree_glyph(commands, cloth, ui::theme::accent().with_alpha(0.9));
}

// ---------------------------------------------------------------------------
// The window.
// ---------------------------------------------------------------------------

/// The main split band's height. The plates (96), gaps and the land strip
/// (40) ride above and below it; the sum is pinned to the People window's
/// measured footprint (761 logical tall), so the codex never changes shape
/// between pages.
const MAIN_BAND: f32 = 501.0;

/// Every page's full height: the ledger's bands sum to it (96 + 5 + 501 +
/// 5 + 45), and the people page is pinned to it directly.
const PAGE_BAND: f32 = 652.0;

pub(crate) fn spawn_village_panel(mut commands: Commands) {
    let window = ui::titled_window(
        &mut commands,
        "THE LEDGER",
        Some("The heart of a living world."),
        1160.0,
    );
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

    // The pages, as siblings in the body: exactly one shows at a time, and
    // every page's bands sum to the same height, so the book never changes
    // shape when a page turns.
    let ledger_page = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(ui::theme::GAP),
                ..default()
            },
            Visibility::Inherited,
            ChildOf(window.body),
        ))
        .id();
    let people_page = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(PAGE_BAND),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                display: Display::None,
                ..default()
            },
            Visibility::Hidden,
            ChildOf(window.body),
        ))
        .id();

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
    let faint = ui::theme::accent().with_alpha(0.35);

    let ledger_tab = tab(&mut commands, true, true);
    house_glyph(&mut commands, ledger_tab, ink);
    commands.entity(ledger_tab).insert((
        CodexTab(CodexPage::Ledger),
        ui::HoverHint::new("The Ledger", "the heart of a living world"),
    ));

    let wood_tab = tab(&mut commands, false, true);
    tree_glyph(&mut commands, wood_tab, faint);
    commands.entity(wood_tab).insert(ui::HoverHint::new(
        "The Wilds",
        "this page of the codex is still being written",
    ));

    let civic_tab = tab(&mut commands, false, true);
    temple_glyph(&mut commands, civic_tab, faint);
    commands.entity(civic_tab).insert(ui::HoverHint::new(
        "The Works",
        "this page of the codex is still being written",
    ));

    let faith_tab = tab(&mut commands, false, true);
    hands_glyph(&mut commands, faith_tab, faint);
    commands.entity(faith_tab).insert(ui::HoverHint::new(
        "The Faith",
        "this page of the codex is still being written",
    ));

    let people_tab = tab(&mut commands, false, true);
    people_glyph(&mut commands, people_tab, ink.with_alpha(0.8));
    commands.entity(people_tab).insert((
        CodexTab(CodexPage::People),
        ui::HoverHint::new("The People", "the mortals of your world"),
    ));

    commands.insert_resource(Codex {
        page: CodexPage::Ledger,
        root: window.root,
        ledger_page,
        people_page,
        ledger_tab,
        people_tab,
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
                column_gap: px(8),
                ..default()
            },
            ChildOf(ledger_page),
        ))
        .id();
    for (index, label) in [(0u8, "souls"), (1, "houses"), (2, "believers")] {
        let (row, number) = ui::stat_plate(&mut commands, plates, label);
        commands.entity(number).insert(VillageCard(index));
        let badge = commands.spawn((Node::default(), ChildOf(row))).id();
        commands.entity(row).insert_children(0, &[badge]);
        let tint = ui::theme::accent().with_alpha(0.55);
        match index {
            0 => person_glyph(&mut commands, badge, tint),
            1 => house_glyph(&mut commands, badge, tint),
            _ => hands_glyph(&mut commands, badge, tint),
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
    let (rail, detail) = ui::split_row(&mut commands, main, 320.0);

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
                margin: UiRect::top(px(5)),
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
    let pages = [
        (codex.ledger_page, codex.page == CodexPage::Ledger),
        (codex.people_page, codex.page == CodexPage::People),
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
        (codex.ledger_tab, codex.page == CodexPage::Ledger),
        (codex.people_tab, codex.page == CodexPage::People),
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
    huts: Query<(), With<crate::villager::work::Hut>>,
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
    commands.entity(building_well).despawn_related::<Children>();
    for (name, count) in &kinds {
        let value = ui::ruled_row(&mut commands, building_well, name);
        commands.entity(value).insert(Text::new(format!("{count}")));
    }
    let rising = pending.iter().count();
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

    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for vocation in &trades {
        *counts.entry(vocation.describe()).or_default() += 1;
    }
    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by_key(|(name, count)| (std::cmp::Reverse(*count), *name));
    commands.entity(trade_well).despawn_related::<Children>();
    for (name, count) in ranked {
        let value = ui::ruled_row(&mut commands, trade_well, name);
        commands.entity(value).insert(Text::new(format!("{count}")));
    }
}

/// Rebuilds the FAITH roster while the codex is open: every soul, ranked
/// by their faith, each with the last reason their heart moved - a god
/// reads congregations the way shepherds count sheep.
#[allow(clippy::type_complexity)]
pub(crate) fn update_faith_roster(
    mut commands: Commands,
    codex: Res<Codex>,
    time: Res<Time>,
    mut last_rebuild: Local<f32>,
    panels: Query<&Visibility, With<VillagePanel>>,
    rosters: Query<Entity, With<FaithRoster>>,
    flock: Query<
        (
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
    commands.entity(roster).despawn_related::<Children>();

    let mut souls: Vec<_> = flock.iter().collect();
    souls.sort_by(|a, b| {
        let fa = a.1.map_or(0.0, |f| f.trust);
        let fb = b.1.map_or(0.0, |f| f.trust);
        fb.total_cmp(&fa)
    });
    for (person, faith, chronicle, witnessed) in souls {
        let trust = faith.map_or(0.0, |f| f.trust);
        let believer = faith.is_some_and(|f| f.is_believer());
        let name_line = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                ChildOf(roster),
            ))
            .id();
        commands.spawn((
            ui::body(format!(
                "{}{}",
                person.name,
                if believer { "  *" } else { "" }
            )),
            ChildOf(name_line),
        ));
        commands.spawn((
            ui::dim(format!(
                "{}  ({:.0}%)",
                faith.map_or("has never wondered", |f| f.describe()),
                trust * 100.0,
            )),
            ChildOf(name_line),
        ));
        // The why: the last line of their life that touched the god.
        let why = chronicle
            .and_then(|c| {
                c.events.iter().rev().find(|e| {
                    e.text.contains("saw")
                        || e.text.contains("heard")
                        || e.text.contains("prayed")
                        || e.text.contains("answered")
                        || e.text.contains("believe")
                })
            })
            .map(|e| format!("d{}  {}", e.day, e.text))
            .unwrap_or_else(|| match witnessed {
                Some(w) if w.secondhand > 0 => "knows the god only from stories".to_string(),
                _ => "has neither seen nor heard of you".to_string(),
            });
        commands.spawn((ui::dim(format!("   {why}")), ChildOf(roster)));
    }
}
