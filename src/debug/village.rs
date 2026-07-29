//! THE VILLAGE window: ledger, gauges, and the faith roster.

use crate::ui;
use crate::villager::Chronicle;
use crate::villager::Morale;
use crate::villager::Needs;
use crate::villager::Person;
use crate::villager::Villager;
use crate::witness::Witnessed;
use bevy::prelude::*;
/// The big centred village ledger.
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

/// The fill of a village gauge.
#[derive(Component)]
pub(crate) struct VillageGaugeFill(VillageStat);

/// The small value text beside a village gauge.
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

pub(crate) fn spawn_village_panel(mut commands: Commands) {
    let window = ui::big_window(&mut commands, "THE VILLAGE", 720.0);
    commands.entity(window.root).insert((
        Name::new("Village Panel"),
        VillagePanel,
        Visibility::Hidden,
    ));

    // Three big numbers first: the shape of the place at a glance.
    let cards = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: px(8),
                ..default()
            },
            ChildOf(window.body),
        ))
        .id();
    for (index, label) in [(0u8, "souls"), (1, "houses"), (2, "believers")] {
        let card = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(2),
                    padding: UiRect::axes(px(10), px(8)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(ui::theme::title_bg()),
                BorderColor::all(ui::theme::panel_border()),
                ChildOf(cards),
            ))
            .id();
        commands.spawn((
            VillageCard(index),
            Text::new("0"),
            TextFont {
                font_size: FontSize::Px(24.0),
                ..default()
            },
            TextColor(ui::theme::accent()),
            ChildOf(card),
        ));
        commands.spawn((ui::dim(label), ChildOf(card)));
    }

    let pages = ui::tab_bar(&mut commands, window.body, &["OVERVIEW", "FAITH"]);
    let overview = pages[0];
    let faith_page = pages[1];
    commands.entity(faith_page).insert(Node {
        width: percent(100),
        min_height: px(320),
        max_height: px(460),
        flex_direction: FlexDirection::Column,
        row_gap: px(3),
        overflow: Overflow::scroll_y(),
        display: Display::None,
        ..default()
    });
    commands.entity(faith_page).insert((
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

    let gauge = |commands: &mut Commands, parent, label: &str, stat, color| {
        let handles = ui::gauge_row(commands, parent, label, color);
        commands.entity(handles.fill).insert(VillageGaugeFill(stat));
        commands
            .entity(handles.value)
            .insert(VillageGaugeValue(stat));
    };

    ui::section_header(&mut commands, overview, "WELLBEING");
    gauge(
        &mut commands,
        overview,
        "happiness",
        VillageStat::Happiness,
        crate::palette::shade(&crate::palette::GRASS, 0.7),
    );
    commands.spawn((
        HappinessWhy,
        ui::dim(""),
        Node {
            margin: UiRect::left(px(ui::theme::LABEL_WIDTH + 10.0)),
            ..default()
        },
        ChildOf(overview),
    ));
    gauge(
        &mut commands,
        overview,
        "fed",
        VillageStat::Fed,
        crate::palette::shade(&crate::palette::CLOTH_RED, 0.6),
    );
    gauge(
        &mut commands,
        overview,
        "housed",
        VillageStat::Housed,
        crate::palette::shade(&crate::palette::WOOD, 0.65),
    );

    ui::section_header(&mut commands, overview, "FAITH");
    gauge(
        &mut commands,
        overview,
        "belief in you",
        VillageStat::Faith,
        ui::theme::accent(),
    );
    gauge(
        &mut commands,
        overview,
        "believers",
        VillageStat::Believers,
        ui::theme::accent().with_alpha(0.55),
    );

    ui::section_header(&mut commands, overview, "STORES");
    gauge(
        &mut commands,
        overview,
        "food",
        VillageStat::Food,
        crate::palette::shade(&crate::palette::GRASS, 0.55),
    );
    gauge(
        &mut commands,
        overview,
        "timber",
        VillageStat::Timber,
        crate::palette::shade(&crate::palette::WOOD, 0.5),
    );
    gauge(
        &mut commands,
        overview,
        "stone",
        VillageStat::Stone,
        crate::palette::shade(&crate::palette::STONE, 0.55),
    );

    ui::section_header(&mut commands, overview, "THE LAND");
    commands.spawn((VillageLand, ui::dim(""), ChildOf(overview)));
}

/// Fills the ledger while it is open.
#[allow(clippy::type_complexity)]
pub(crate) fn update_village_panel(
    panels: Query<&Visibility, With<VillagePanel>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    stores: Query<&crate::villager::work::Stockpile>,
    villagers: Query<
        (
            Option<&Needs>,
            Option<&Morale>,
            Option<&crate::villager::belief::Faith>,
            Option<&crate::villager::home::Home>,
            Has<crate::creature::Childhood>,
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
        ),
    >,
    mut gauges: ParamSet<(
        Query<(&VillageCard, &mut Text)>,
        Query<(&VillageGaugeFill, &mut Node)>,
        Query<(&VillageGaugeValue, &mut Text)>,
        Query<&mut Text, (With<VillageLand>, Without<HappinessWhy>)>,
    )>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
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
    for (needs, morale, faith, home, child) in &villagers {
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
    }
    let houses = huts.iter().count();
    let (food, timber, stone) = site
        .and_then(|site| stores.get(site.settlement).ok())
        .map_or((0.0, 0.0, 0.0), |s| (s.food(), s.timber, s.stone));

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

    let standing = trees.iter().filter(|t| t.harvestable()).count();
    let fresh = format!(
        "{standing} trees standing  -  {} wild things  -  {} at rest in the ground",
        wildlife.iter().count(),
        graves.iter().count(),
    );
    if let Ok(mut text) = gauges.p3().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }
}

/// Rebuilds the FAITH roster while the ledger is open: every soul, ranked
/// by their faith, each with the last reason their heart moved - a god
/// reads congregations the way shepherds count sheep.
#[allow(clippy::type_complexity)]
pub(crate) fn update_faith_roster(
    mut commands: Commands,
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
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
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
