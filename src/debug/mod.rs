//! Developer HUD and live tuning.
//!
//! Built before it was strictly needed, on purpose. Procedurally generated art is
//! only as good as the ability to steer it, and steering it by editing constants and
//! waiting for a rebuild is not steering. Every number that decides how the game
//! looks is adjustable here while it runs.
//!
//! The HUD also doubles as the inspection tool the villager simulation will need:
//! it already reports what the population is doing and what the hand is touching,
//! which is the seed of the per-villager debug panel.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::creature::genome::{Age, Sex};
use crate::hand::DivineHand;
use crate::render::LookSettings;
use crate::terrain::LoadedChunks;
use crate::ui;
use crate::villager::{
    Activity, Chronicle, MemberOf, Morale, Needs, Parentage, Person, Settlement, Spouse, Villager,
};
use crate::witness::{Reaction, Temperament, Witnessed};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<DebugState>()
            .init_resource::<SelectedPerson>()
            .add_systems(
                Startup,
                (
                    spawn_hud,
                    spawn_toolbar,
                    spawn_world_panel,
                    spawn_people_panel,
                    spawn_history_panel,
                    spawn_village_panel,
                    spawn_god_panel,
                ),
            )
            .add_systems(
                Update,
                (
                    handle_tuning_input,
                    handle_toolbar,
                    update_hud,
                    update_world_panel,
                    update_people_panel,
                    update_history_panel,
                    handle_people_rows,
                    update_inspector,
                    screenshot_on_request,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    update_faith_roster,
                    update_god_panel,
                    capture_preselect,
                    style_roster_rows,
                    update_village_panel,
                    update_paperdoll,
                    stamp_doll_layers,
                    spin_doll,
                    update_person_detail,
                )
                    .chain(),
            );

        if let Some(path) = crate::capture_path() {
            app.insert_resource(AutoCapture {
                path,
                // Long enough for terrain generation, scatter and the first frames
                // of animation to settle, so the capture is representative.
                // EGREGORE_CAPTURE_DELAY overrides it, for photographing things
                // that take minutes to happen — a village being built, say.
                delay: std::env::var("EGREGORE_CAPTURE_DELAY")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(9.0),
                taken: false,
            })
            .add_systems(Update, auto_capture);
        }
    }
}

/// Pending automatic screenshot, if one was requested by environment variable.
#[derive(Resource)]
struct AutoCapture {
    path: String,
    delay: f32,
    taken: bool,
}

fn auto_capture(
    mut commands: Commands,
    time: Res<Time>,
    mut capture: ResMut<AutoCapture>,
    target: Option<Res<crate::render::CaptureTarget>>,
    chunk_probe: Option<Res<LoadedChunks>>,
    entity_probe: Query<Entity>,
    fps_probe: Res<DiagnosticsStore>,
    rig_probe: Query<&crate::camera::CameraRig>,
    mut exit: MessageWriter<AppExit>,
) {
    if capture.taken {
        // Give the save a moment to land before tearing the app down.
        if time.elapsed_secs() > capture.delay + 1.5 {
            exit.write(AppExit::Success);
        }
        return;
    }

    if time.elapsed_secs() >= capture.delay {
        capture.taken = true;
        let rig = rig_probe.single().ok();
        info!(
            "PERF chunks={} entities={} fps={:.0} dist={:.0} focus=({:.0},{:.0})",
            chunk_probe.map_or(0, |c| c.count()),
            entity_probe.iter().count(),
            fps_probe
                .get(&FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|d| d.smoothed())
                .unwrap_or(0.0),
            rig.map_or(0.0, |r| r.distance),
            rig.map_or(0.0, |r| r.focus.x),
            rig.map_or(0.0, |r| r.focus.z),
        );
        let path = capture.path.clone();

        // Prefer the offscreen target set up by the render pipeline; fall back to
        // the window if capture mode did not manage to create one.
        let screenshot = match &target {
            Some(target) => Screenshot::image(target.image.clone()),
            None => Screenshot::primary_window(),
        };
        commands.spawn(screenshot).observe(save_to_disk(path));
    }
}

/// Saves a screenshot on F12.
fn screenshot_on_request(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut counter: Local<u32>,
) {
    if keys.just_pressed(KeyCode::F12) {
        let path = format!("egregore-{:03}.png", *counter);
        *counter += 1;
        info!("saving screenshot to {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

#[derive(Resource)]
pub struct DebugState {
    pub hud_visible: bool,
}

impl Default for DebugState {
    /// Hidden until asked for. The HUD is a developer's instrument panel, and
    /// the game should open on the world, not on numbers about the world.
    /// Unattended captures keep it on — they exist to be inspected.
    fn default() -> Self {
        DebugState {
            hud_visible: crate::capture_path().is_some(),
        }
    }
}

/// The stats-and-tuning panel, top left.
#[derive(Component)]
struct HudPanel;

/// Which live readout a HUD value text shows. One enum, one update system —
/// adding a row to the HUD is adding a variant, a label and a match arm.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HudValue {
    Fps,
    Date,
    You,
    BeliefTotal,
    PrayersOpen,
    Chunks,
    Population,
    Store,
    Dead,
    AvgHunger,
    Hungry,
    Eating,
    Watching,
    Aperture,
    Focus,
    Visibility,
    Exposure,
    Saturation,
    DepthOfField,
}

/// The inspector, bottom left: whoever the hand is over, read out as a person.
#[derive(Component)]
struct InspectorPanel;

#[derive(Component)]
struct InspectorName;

/// The line under the name: who they are in a phrase.
#[derive(Component)]
struct InspectorSubtitle;

/// One line of prose for subjects that are not living people — corpses, animals,
/// bushes. Hidden while a living person's rows are showing.
#[derive(Component)]
struct InspectorDetail;

/// Which live readout an inspector row shows.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum InspectorValue {
    State,
    Manner,
    Hunger,
    Rest,
    Health,
    Spirits,
    Heart,
    FaithIn,
    Work,
    Family,
    Seen,
}

/// Everything shown only for a living person — the stat rows, the memory
/// section. One marker so the whole block can be shown and hidden together.
#[derive(Component)]
struct InspectorPersonBlock;

/// The recent-memories text, in the villager's own phrasing.
#[derive(Component)]
struct InspectorMemories;

/// The life-so-far text: the tail of this person's chronicle.
#[derive(Component)]
struct InspectorLife;

/// The toolbar button that flies the camera home to the settlement.
#[derive(Component)]
struct RecenterButton;

/// The toolbar button that opens the world panel.
#[derive(Component)]
struct WorldButton;

/// The world panel: the state of the sky, the season to come, the land.
#[derive(Component)]
struct WorldPanel;

/// The toolbar button that opens the people roster.
#[derive(Component)]
struct PeopleButton;

/// The roster panel: everyone alive, click to follow.
#[derive(Component)]
struct PeoplePanel;

/// The container the roster's rows are rebuilt into.
#[derive(Component)]
struct PeopleRows;

/// One roster row, pointing at its person.
#[derive(Component)]
struct PersonRow(Entity);

/// Where the paperdoll stands: a stage far below the world, on its own
/// render layer, seen by no one but its own camera.
const DOLL_STAGE: Vec3 = Vec3::new(0.0, -600.0, 0.0);
const DOLL_LAYER: usize = 2;

/// The offscreen texture the paperdoll camera draws to.
#[derive(Resource)]
struct PaperdollTarget(#[allow(dead_code)] Handle<Image>);

/// Whose dossier the people window shows.
#[derive(Resource, Default)]
struct SelectedPerson(Option<Entity>);

/// The paperdoll body currently on stage.
#[derive(Component)]
struct DollBody;

/// The text block in the people window's detail pane.
#[derive(Component)]
struct PersonDetailText;

/// A follow button beside a roster name: click to fly to and follow them.
#[derive(Component)]
struct FollowButton(Entity);

/// A live stat row in the people window's detail pane, mirroring the
/// inspector's readouts.
#[derive(Component)]
struct DetailStat(InspectorValue);

/// The name line at the top of the detail pane.
#[derive(Component)]
struct DetailName;

/// The subtitle under the name.
#[derive(Component)]
struct DetailSubtitle;

/// The HAS SEEN body text in the detail pane.
#[derive(Component)]
struct DetailSeen;

/// The LIFE body text in the detail pane.
#[derive(Component)]
struct DetailLife;

/// The dossier content, shown only while someone is selected.
#[derive(Component)]
struct DetailPage;

/// The empty state shown when no one is selected.
#[derive(Component)]
struct DetailEmpty;

/// A roster row's face: who it belongs to and its resting shade, so the
/// selected row can glow and the rest can zebra.
#[derive(Component)]
struct RowFace {
    person: Entity,
    base: f32,
}

/// The big centred village ledger.
#[derive(Component)]
struct VillagePanel;

/// Its toolbar button.
#[derive(Component)]
struct VillageButton;

/// One of the three big numbers at the top: souls, houses, believers.
#[derive(Component)]
struct VillageCard(u8);

/// A dashboard statistic, drawn as a bar.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum VillageStat {
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
struct VillageGaugeFill(VillageStat);

/// The small value text beside a village gauge.
#[derive(Component)]
struct VillageGaugeValue(VillageStat);

/// The one line of prose at the ledger's foot: the land itself.
#[derive(Component)]
struct VillageLand;

/// The FAITH tab's roster: who believes, and the last reason why.
#[derive(Component)]
struct FaithRoster;

/// THE GOD panel and its live pieces.
#[derive(Component)]
struct GodPanel;

#[derive(Component)]
struct GodButton;

#[derive(Component)]
struct GodName;

#[derive(Component)]
struct GodEpithet;

#[derive(Component)]
struct MiracleTile(crate::miracles::Miracle);

#[derive(Component)]
struct MiracleTileLabel(crate::miracles::Miracle);

#[derive(Component)]
struct FeelingsText;

fn spawn_god_panel(mut commands: Commands) {
    let window = ui::big_window(&mut commands, "THE GOD", 560.0);
    commands
        .entity(window.root)
        .insert((Name::new("God Panel"), GodPanel, Visibility::Hidden));

    // The masthead: the name they gave you, writ large, on the warm card.
    let masthead = ui::detail_card(&mut commands, window.body);
    commands.spawn((
        GodName,
        Text::new("..."),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextColor(ui::theme::accent()),
        ChildOf(masthead),
    ));
    commands.spawn((GodEpithet, ui::dim("not yet named"), ChildOf(masthead)));

    // MIRACLES: a tile per power, earned or waiting.
    ui::section_header(&mut commands, window.body, "MIRACLES");
    let grid = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                ..default()
            },
            ChildOf(window.body),
        ))
        .id();
    use crate::miracles::Miracle;
    for miracle in [
        Miracle::Flourish,
        Miracle::Smite,
        Miracle::Bounty,
        Miracle::Mend,
        Miracle::Quake,
    ] {
        let cell = ui::tile(&mut commands, grid, 86.0, false);
        commands.entity(cell).insert(MiracleTile(miracle));
        commands.spawn((
            Text::new(miracle.name()),
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(ui::theme::text()),
            ChildOf(cell),
        ));
        commands.spawn((MiracleTileLabel(miracle), ui::dim(""), ChildOf(cell)));
    }
    commands.spawn((
        ui::dim("locked powers are earned: a legend of providence crystallises Mend; a legend of dread, Quake"),
        ChildOf(window.body),
    ));

    // HOW THEY FEEL: the congregation read as one voice.
    ui::section_header(&mut commands, window.body, "HOW THEY FEEL ABOUT YOU");
    let feelings = ui::inset_well(&mut commands, window.body);
    commands.spawn((FeelingsText, ui::body(""), ChildOf(feelings)));
}

/// Fills THE GOD panel while it is open.
#[allow(clippy::type_complexity)]
fn update_god_panel(
    panels: Query<&Visibility, With<GodPanel>>,
    name: Option<Res<crate::villager::DivineName>>,
    legend: Option<Res<crate::villager::belief::Legend>>,
    belief: Option<Res<crate::villager::belief::Belief>>,
    flock: Query<
        (Option<&crate::villager::belief::Faith>, Option<&Witnessed>),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    mut tiles: Query<(&MiracleTile, &mut BackgroundColor, &mut BorderColor)>,
    mut texts: ParamSet<(
        Query<&mut Text, With<GodName>>,
        Query<&mut Text, With<GodEpithet>>,
        Query<(&MiracleTileLabel, &mut Text)>,
        Query<&mut Text, With<FeelingsText>>,
    )>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    use crate::miracles::Miracle;

    if let Ok(mut text) = texts.p0().single_mut() {
        let fresh = name.as_ref().map_or("...".to_string(), |n| n.0.clone());
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
    if let Ok(mut text) = texts.p1().single_mut() {
        let fresh = match legend.as_ref().and_then(|l| l.epithet) {
            Some(epithet) => format!("{epithet} - so the people say"),
            None => "named by the people; no epithet yet earned".to_string(),
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }

    let unlocked = |miracle: Miracle| match miracle {
        Miracle::Flourish | Miracle::Smite => true,
        m => legend.as_ref().is_some_and(|l| l.unlocked == Some(m)),
    };
    for (tile, mut bg, mut border) in &mut tiles {
        let lit = unlocked(tile.0);
        bg.0 = if lit {
            ui::theme::title_bg()
        } else {
            Color::BLACK.with_alpha(0.3)
        };
        *border = BorderColor::all(if lit {
            ui::theme::card_border()
        } else {
            ui::theme::panel_border().with_alpha(0.2)
        });
    }
    for (label, mut text) in &mut texts.p2() {
        let fresh = if unlocked(label.0) {
            format!("{:.0} belief", label.0.cost())
        } else {
            "locked".to_string()
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }

    // The congregation, read as one voice.
    let mut total = 0usize;
    let mut believers = 0usize;
    let mut trust_sum = 0.0f32;
    let mut eyewitnesses = 0usize;
    let mut heard = 0usize;
    for (faith, witnessed) in &flock {
        total += 1;
        if let Some(faith) = faith {
            trust_sum += faith.trust;
            if faith.is_believer() {
                believers += 1;
            }
        }
        if let Some(w) = witnessed {
            if w.total > 0 {
                eyewitnesses += 1;
            } else if w.secondhand > 0 {
                heard += 1;
            }
        }
    }
    let avg = trust_sum / total.max(1) as f32;
    let mood = match avg {
        a if a > 0.6 => "they are sure of you",
        a if a > 0.45 => "they believe, on the whole",
        a if a > 0.25 => "they waver",
        _ => "they doubt you",
    };
    let lean = legend.as_ref().map_or("", |l| {
        if l.providence > l.dread * 1.4 {
            "the stories they tell are of gifts"
        } else if l.dread > l.providence * 1.4 {
            "the stories they tell are of terror"
        } else {
            "their stories cannot decide what you are"
        }
    });
    let power = belief.as_ref().map_or(0.0, |b| b.available());
    let fresh = format!(
        "{believers} of {total} believe - {mood}
{eyewitnesses} have seen you with their own eyes; {heard} know you only from stories
{lean}
{power:.0} belief stands ready to spend",
    );
    if let Ok(mut text) = texts.p3().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }
}

/// The line under the happiness gauge saying WHY it stands where it does.
#[derive(Component)]
struct HappinessWhy;

fn spawn_village_panel(mut commands: Commands) {
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
fn update_village_panel(
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
        .map_or((0.0, 0.0, 0.0), |s| (s.food, s.timber, s.stone));

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

/// The toolbar button that opens the history.
#[derive(Component)]
struct HistoryButton;

/// The history panel: everything that has ever happened, stamped.
#[derive(Component)]
struct HistoryPanel;

/// The history text block.
#[derive(Component)]
struct HistoryText;

/// Which world reading a row shows.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum WorldValue {
    Date,
    SkyState,
    Temperature,
    Country,
}

fn spawn_toolbar(mut commands: Commands) {
    let bar = ui::toolbar(&mut commands);
    let recenter = ui::icon_button(&mut commands, bar);
    commands.entity(recenter).insert(RecenterButton);

    // The icon is the town banner, drawn in nodes: a pole with a flag.
    let pole = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(12),
                top: px(6),
                width: px(3),
                height: px(21),
                ..default()
            },
            BackgroundColor(ui::theme::text_dim()),
        ))
        .id();
    commands.entity(pole).insert(ChildOf(recenter));
    let flag = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(15),
                top: px(7),
                width: px(11),
                height: px(8),
                ..default()
            },
            BackgroundColor(ui::theme::accent()),
        ))
        .id();
    commands.entity(flag).insert(ChildOf(recenter));

    // The village ledger: three rising bars.
    let village = ui::icon_button(&mut commands, bar);
    commands.entity(village).insert(VillageButton);
    for (i, height) in [(0, 8.0), (1, 13.0), (2, 18.0)] {
        let bar_node = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(8.0 + i as f32 * 6.0),
                    bottom: px(6),
                    width: px(4),
                    height: px(height),
                    ..default()
                },
                BackgroundColor(if i == 2 {
                    ui::theme::accent()
                } else {
                    ui::theme::text_dim()
                }),
            ))
            .id();
        commands.entity(bar_node).insert(ChildOf(village));
    }

    // The god's own panel: a gold diamond mark.
    let god = ui::icon_button(&mut commands, bar);
    commands.entity(god).insert(GodButton);
    for (size, top, left, bright) in [(12.0, 10.0, 10.0, true), (6.0, 13.0, 13.0, false)] {
        let mark = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(top),
                    width: px(size),
                    height: px(size),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(if bright {
                    ui::theme::accent()
                } else {
                    ui::theme::title_bg()
                }),
            ))
            .id();
        commands.entity(mark).insert(ChildOf(god));
    }

    // The world button: a globe, drawn as a ringed circle with a belt.
    let world = ui::icon_button(&mut commands, bar);
    commands.entity(world).insert(WorldButton);
    let globe = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(7),
                top: px(7),
                width: px(20),
                height: px(20),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BorderColor::all(ui::theme::text_dim()),
        ))
        .id();
    commands.entity(globe).insert(ChildOf(world));
    let belt = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(8),
                top: px(15),
                width: px(18),
                height: px(3),
                ..default()
            },
            BackgroundColor(ui::theme::accent().with_alpha(0.8)),
        ))
        .id();
    commands.entity(belt).insert(ChildOf(world));

    // The people button: a head above shoulders.
    let people = ui::icon_button(&mut commands, bar);
    commands.entity(people).insert(PeopleButton);
    let head = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(13),
                top: px(6),
                width: px(8),
                height: px(8),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(ui::theme::text_dim()),
        ))
        .id();
    commands.entity(head).insert(ChildOf(people));
    let shoulders = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(9),
                top: px(16),
                width: px(16),
                height: px(11),
                border_radius: BorderRadius::top(px(7)),
                ..default()
            },
            BackgroundColor(ui::theme::accent().with_alpha(0.8)),
        ))
        .id();
    commands.entity(shoulders).insert(ChildOf(people));

    // The history button: a page with written lines.
    let history = ui::icon_button(&mut commands, bar);
    commands.entity(history).insert(HistoryButton);
    let page = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(9),
                top: px(5),
                width: px(16),
                height: px(23),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(2)),
                ..default()
            },
            BorderColor::all(ui::theme::text_dim()),
        ))
        .id();
    commands.entity(page).insert(ChildOf(history));
    for (i, width) in [(0u8, 8.0f32), (1, 8.0), (2, 5.0)] {
        let line = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(13),
                    top: px(10.0 + i as f32 * 5.0),
                    width: px(width),
                    height: px(2),
                    ..default()
                },
                BackgroundColor(ui::theme::accent().with_alpha(0.8)),
            ))
            .id();
        commands.entity(line).insert(ChildOf(history));
    }
}

fn spawn_history_panel(mut commands: Commands) {
    let window = ui::window(&mut commands, "THE CHRONICLE", 320.0);
    commands.entity(window.root).insert((
        Name::new("History Panel"),
        HistoryPanel,
        Visibility::Hidden,
    ));
    let text = commands.spawn((HistoryText, ui::dim(""))).id();
    commands.entity(text).insert(ChildOf(window.body));
}

/// Fills the history panel with the tail of the world's chronicle.
fn update_history_panel(
    history: Option<Res<crate::villager::WorldChronicle>>,
    panels: Query<&Visibility, With<HistoryPanel>>,
    mut texts: Query<&mut Text, With<HistoryText>>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    let (Some(history), Ok(mut text)) = (history, texts.single_mut()) else {
        return;
    };

    let events = &history.events;
    let shown = 16usize;
    let tail = events.len().saturating_sub(shown);
    let mut lines: Vec<String> = events[tail..]
        .iter()
        .map(|event| format!("{}  {}", event.stamp, event.text))
        .collect();
    if tail > 0 {
        lines.insert(0, format!("({tail} earlier entries)"));
    }
    if lines.is_empty() {
        lines.push("nothing has happened yet".into());
    }
    let fresh = lines.join("\n");
    if text.0 != fresh {
        *text = Text::new(fresh);
    }
}

fn spawn_people_panel(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let split = ui::split_view(&mut commands, "THE PEOPLE", 285.0, 470.0);
    // Capture mode opens the window and picks somebody, so an unattended
    // screenshot can prove the pane works.
    let starts = if crate::capture_path().is_some() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    commands
        .entity(split.window.root)
        .insert((Name::new("People Panel"), PeoplePanel, starts));
    commands.entity(split.list).insert(PeopleRows);

    // The paperdoll: a private little stage far under the world, drawn by its
    // own camera to a texture the detail pane shows. The doll is the person's
    // real body, rebuilt, turning slowly.
    let target = images.add(bevy::image::Image::new_target_texture(
        440,
        520,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        Name::new("Paperdoll Camera"),
        Camera3d::default(),
        Camera {
            order: -20,
            clear_color: bevy::camera::ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        bevy::camera::RenderTarget::Image(target.clone().into()),
        Transform::from_translation(DOLL_STAGE + Vec3::new(0.0, 1.1, 3.1))
            .looking_at(DOLL_STAGE + Vec3::Y * 0.85, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER),
    ));
    commands.spawn((
        Name::new("Paperdoll Light"),
        DirectionalLight {
            illuminance: 14_000.0,
            ..default()
        },
        Transform::from_xyz(2.0, 3.0, 2.5).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER),
    ));

    // The page and its empty state: one shows, the other doesn't.
    let empty = commands
        .spawn((
            DetailEmpty,
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(4),
                ..default()
            },
            ChildOf(split.detail),
        ))
        .id();
    commands.spawn((ui::heading("NO ONE CHOSEN"), ChildOf(empty)));
    commands.spawn((ui::dim("click a name to meet them"), ChildOf(empty)));

    let page = commands
        .spawn((
            DetailPage,
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(ui::theme::GAP),
                ..default()
            },
            Visibility::Hidden,
            ChildOf(split.detail),
        ))
        .id();

    // Portrait beside name and standing: the page's masthead. The portrait
    // sits in a framed plaque, not floating on the panel.
    let masthead = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(page),
        ))
        .id();
    let plaque = commands
        .spawn((
            Node {
                padding: UiRect::all(px(3)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(ui::theme::title_bg()),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(masthead),
        ))
        .id();
    commands.spawn((
        bevy::ui::widget::ImageNode::new(target.clone()),
        Node {
            width: px(104),
            height: px(124),
            ..default()
        },
        ChildOf(plaque),
    ));
    let masthead_text = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            },
            ChildOf(masthead),
        ))
        .id();
    commands.spawn((DetailName, ui::heading(""), ChildOf(masthead_text)));
    commands.spawn((DetailSubtitle, ui::dim(""), ChildOf(masthead_text)));

    // Every readout the hover card has, permanent and orderly.
    for (value, label) in [
        (InspectorValue::State, "state"),
        (InspectorValue::Hunger, "hunger"),
        (InspectorValue::Rest, "rest"),
        (InspectorValue::Health, "health"),
        (InspectorValue::Spirits, "spirits"),
        (InspectorValue::Heart, "heart"),
        (InspectorValue::Manner, "manner"),
        (InspectorValue::FaithIn, "faith"),
        (InspectorValue::Work, "work"),
        (InspectorValue::Family, "family"),
        (InspectorValue::Seen, "seen you"),
    ] {
        let row = ui::stat_row(&mut commands, page, label, None);
        commands.entity(row.value).insert(DetailStat(value));
    }
    ui::section_header(&mut commands, page, "WANTS");
    commands.spawn((PersonDetailText, ui::body(""), ChildOf(page)));
    ui::section_header(&mut commands, page, "HAS SEEN");
    commands.spawn((DetailSeen, ui::body(""), ChildOf(page)));
    ui::section_header(&mut commands, page, "LIFE");
    commands.spawn((DetailLife, ui::dim(""), ChildOf(page)));
    commands.insert_resource(PaperdollTarget(target));
}

/// Rebuilds the roster while it is open: one clickable row per living person.
fn update_people_panel(
    mut commands: Commands,
    time: Res<Time>,
    mut last_rebuild: Local<f32>,
    mut was_open: Local<bool>,
    panels: Query<&Visibility, With<PeoplePanel>>,
    containers: Query<Entity, With<PeopleRows>>,
    people: Query<
        (
            Entity,
            &Person,
            Option<&crate::villager::work::Vocation>,
            &Activity,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
) {
    let open = panels.iter().any(|v| *v != Visibility::Hidden);
    // A window that just opened fills instantly; only the refresh is paced.
    let just_opened = open && !*was_open;
    *was_open = open;
    if !open {
        return;
    }
    *last_rebuild += time.delta_secs();
    if *last_rebuild < 2.0 && !just_opened {
        return;
    }
    *last_rebuild = 0.0;

    let Ok(container) = containers.single() else {
        return;
    };
    commands.entity(container).despawn_related::<Children>();

    let mut names: Vec<_> = people.iter().collect();
    names.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    for (index, (entity, person, vocation, activity)) in names.into_iter().enumerate() {
        let doing = match activity {
            Activity::Working => vocation.map_or("at work", |v| v.describe()),
            other => state_phrase(Some(other), None),
        };
        let base = if index % 2 == 1 { 0.045 } else { 0.0 };
        let row = commands
            .spawn((
                RowFace {
                    person: entity,
                    base,
                },
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(4),
                    padding: UiRect::right(px(4)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(Color::WHITE.with_alpha(base)),
                ChildOf(container),
            ))
            .id();
        let name_button = commands
            .spawn((
                PersonRow(entity),
                ui::UiButton,
                Node {
                    flex_grow: 1.0,
                    padding: UiRect::axes(px(6), px(2)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(ui::theme::panel_bg().with_alpha(0.0)),
                Interaction::default(),
                ChildOf(row),
            ))
            .id();
        commands.spawn((
            ui::body(format!("{} - {}", person.name, doing)),
            ChildOf(name_button),
        ));
        // The little chevron flies the camera to them; the name just opens
        // their page.
        let follow_button = commands
            .spawn((
                FollowButton(entity),
                ui::UiButton,
                Node {
                    width: px(18),
                    height: px(18),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(ui::theme::panel_border()),
                Interaction::default(),
                ChildOf(row),
            ))
            .id();
        commands.spawn((ui::dim(">"), ChildOf(follow_button)));
    }
}

/// The selected roster row glows; the rest keep their zebra shade.
fn style_roster_rows(
    selected: Res<SelectedPerson>,
    mut rows: Query<(&RowFace, &mut BackgroundColor)>,
) {
    for (face, mut bg) in &mut rows {
        bg.0 = if selected.0 == Some(face.person) {
            ui::theme::accent().with_alpha(0.16)
        } else {
            Color::WHITE.with_alpha(face.base)
        };
    }
}

/// Rebuilds the paperdoll when the selection changes: the person's actual
/// genome, rebuilt on the stage.
/// In capture mode, someone is always selected: screenshots need a subject.
fn capture_preselect(
    mut selected: ResMut<SelectedPerson>,
    people: Query<
        Entity,
        (
            With<Villager>,
            With<Person>,
            Without<crate::creature::Corpse>,
        ),
    >,
) {
    if crate::capture_path().is_none() || selected.0.is_some() {
        return;
    }
    if let Some(person) = people.iter().next() {
        selected.0 = Some(person);
    }
}

fn update_paperdoll(
    mut commands: Commands,
    selected: Res<SelectedPerson>,
    assets: Option<Res<crate::creature::body::CreatureAssets>>,
    genomes: Query<&crate::creature::genome::CreatureGenome>,
    dolls: Query<Entity, With<DollBody>>,
) {
    if !selected.is_changed() {
        return;
    }
    let Some(assets) = assets else {
        return;
    };
    for doll in &dolls {
        commands.entity(doll).despawn();
    }
    let Some(genome) = selected.0.and_then(|person| genomes.get(person).ok()) else {
        return;
    };
    let root = commands
        .spawn((
            DollBody,
            Transform::from_translation(DOLL_STAGE),
            Visibility::default(),
            bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER),
        ))
        .id();
    let rig = crate::creature::body::build_body(&mut commands, &assets, root, genome);
    commands.entity(root).insert(rig);
}

/// `RenderLayers` does not inherit: every part the body builder spawned has
/// to be stamped onto the doll's private layer, or it renders in the world.
fn stamp_doll_layers(
    mut commands: Commands,
    dolls: Query<Entity, With<DollBody>>,
    children: Query<&Children>,
    unstamped: Query<(), Without<bevy::camera::visibility::RenderLayers>>,
) {
    for doll in &dolls {
        for part in children.iter_descendants(doll) {
            if unstamped.get(part).is_ok() {
                commands
                    .entity(part)
                    .insert(bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER));
            }
        }
    }
}

/// The doll turns slowly, so the whole person can be seen.
fn spin_doll(time: Res<Time>, mut dolls: Query<&mut Transform, With<DollBody>>) {
    for mut doll in &mut dolls {
        doll.rotate_y(time.delta_secs() * 0.6);
    }
}

/// Fills the detail pane: the whole dossier the hover card shows, plus what
/// they want — the people window is the long look, the hover card the glance.
#[allow(clippy::type_complexity)]
fn update_person_detail(
    selected: Res<SelectedPerson>,
    panels: Query<&Visibility, With<PeoplePanel>>,
    people: Query<
        (
            (
                &Person,
                Option<&crate::creature::genome::CreatureGenome>,
                Option<&MemberOf>,
            ),
            (
                Option<&Needs>,
                Option<&Activity>,
                Option<&crate::creature::Vitality>,
                Option<&Morale>,
            ),
            (
                Option<&Temperament>,
                Option<&Witnessed>,
                Option<&crate::villager::belief::Faith>,
                Option<&Chronicle>,
            ),
            (
                Option<&Spouse>,
                Option<&Parentage>,
                Option<&crate::villager::home::Home>,
                Option<&crate::villager::work::Vocation>,
                Option<&crate::villager::traits::Traits>,
                Has<crate::creature::Childhood>,
            ),
        ),
        Without<crate::creature::Corpse>,
    >,
    kin_names: Query<&Person>,
    corpse_check: Query<Option<&crate::creature::Vitality>, With<crate::creature::Corpse>>,
    settlements: Query<&Settlement>,
    mut page: Query<
        &mut Visibility,
        (With<DetailPage>, Without<DetailEmpty>, Without<PeoplePanel>),
    >,
    mut empty: Query<
        &mut Visibility,
        (With<DetailEmpty>, Without<DetailPage>, Without<PeoplePanel>),
    >,
    mut texts: ParamSet<(
        Query<&mut Text, With<DetailName>>,
        Query<&mut Text, With<DetailSubtitle>>,
        Query<(&DetailStat, &mut Text)>,
        Query<&mut Text, With<PersonDetailText>>,
        Query<&mut Text, With<DetailSeen>>,
        Query<&mut Text, With<DetailLife>>,
    )>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }

    let Some((
        (person, genome, member_of),
        (needs, activity, vitality, morale),
        (temperament, witnessed, faith, chronicle),
        (spouse, parentage, home, vocation, manner, child),
    )) = selected.0.and_then(|entity| people.get(entity).ok())
    else {
        for mut visibility in &mut page {
            *visibility = Visibility::Hidden;
        }
        for mut visibility in &mut empty {
            *visibility = Visibility::Inherited;
        }
        return;
    };
    for mut visibility in &mut page {
        *visibility = Visibility::Inherited;
    }
    for mut visibility in &mut empty {
        *visibility = Visibility::Hidden;
    }

    if let Ok(mut name) = texts.p0().single_mut()
        && name.0 != person.name
    {
        *name = Text::new(person.name.clone());
    }
    let who = genome.map_or("a soul", |g| person_phrase(g.sex, g.age));
    let of = member_of
        .and_then(|m| settlements.get(m.0).ok())
        .map_or_else(|| "the wilds".to_string(), |s| s.name.clone());
    if let Ok(mut subtitle) = texts.p1().single_mut() {
        let fresh = format!("{who} of {of}");
        if subtitle.0 != fresh {
            *subtitle = Text::new(fresh);
        }
    }

    let hunger = needs.map_or(0.0, |n| n.hunger);
    let harm = vitality.map_or(0.0, |v| v.harm);
    for (stat, mut text) in &mut texts.p2() {
        let fresh = match stat.0 {
            InspectorValue::State => state_phrase(activity, None).to_string(),
            InspectorValue::Hunger => hunger_word(hunger).to_string(),
            InspectorValue::Rest => needs.map_or("wakeful", |n| rest_word(n.rest)).to_string(),
            InspectorValue::Health => health_word(harm).to_string(),
            InspectorValue::Spirits => morale
                .map_or("steady", |m| spirits_word(m.spirits))
                .to_string(),
            InspectorValue::Heart => temperament.map_or("unread", |t| t.describe()).to_string(),
            InspectorValue::Manner => manner.map_or("unremarkable".to_string(), |m| m.describe()),
            InspectorValue::FaithIn => faith
                .map_or("has never wondered", |f| f.describe())
                .to_string(),
            InspectorValue::Work => vocation.map_or("none yet", |v| v.describe()).to_string(),
            InspectorValue::Family => family_phrase(spouse, parentage, &kin_names, &corpse_check),
            InspectorValue::Seen => match witnessed {
                Some(w) if w.is_innocent() && w.secondhand > 0 => "only in stories".to_string(),
                Some(w) if w.is_innocent() => "never".to_string(),
                Some(w) => format!("{} times", w.total),
                None => "never".to_string(),
            },
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }

    // Wants, memories, and the life so far — the parts of a person a table
    // cannot hold, each under its own ruled header.
    let mut wants: Vec<&str> = Vec::new();
    if needs.is_some_and(|n| n.hunger > 0.55) {
        wants.push("a full belly");
    }
    if needs.is_some_and(|n| n.rest > 0.7) {
        wants.push("a night's sleep");
    }
    if home.is_none() {
        wants.push("a roof of their own");
    }
    if spouse.is_none() && !child {
        wants.push("someone to come home to");
    }
    if morale.is_some_and(|m| m.spirits < 0.35) {
        wants.push("better days");
    }
    if vocation.is_none() && !child {
        wants.push("a calling");
    }
    if wants.is_empty() {
        wants.push("nothing - life, for now, is enough");
    }
    let fresh = wants
        .iter()
        .map(|want| format!("- {want}"))
        .collect::<Vec<_>>()
        .join("\n");
    if let Ok(mut text) = texts.p3().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }

    let fresh = match witnessed {
        Some(w) if !w.recent.is_empty() => w
            .recent
            .iter()
            .take(3)
            .map(|kind| format!("- {}", kind.describe()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => "nothing they could not explain".to_string(),
    };
    if let Ok(mut text) = texts.p4().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }

    let fresh = chronicle.map_or_else(
        || "unwritten".to_string(),
        |chronicle| {
            let tail = chronicle.events.len().saturating_sub(4);
            chronicle.events[tail..]
                .iter()
                .map(|event| format!("d{}  {}", event.day, event.text))
                .collect::<Vec<_>>()
                .join("\n")
        },
    );
    if let Ok(mut text) = texts.p5().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }
}

/// Rebuilds the FAITH roster while the ledger is open: every soul, ranked
/// by their faith, each with the last reason their heart moved - a god
/// reads congregations the way shepherds count sheep.
#[allow(clippy::type_complexity)]
fn update_faith_roster(
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

/// A click on a roster row flies the camera to that person and follows them.
fn handle_people_rows(
    rows: Query<(&Interaction, &PersonRow), Changed<Interaction>>,
    followers: Query<(&Interaction, &FollowButton), Changed<Interaction>>,
    mut follow: ResMut<crate::camera::FollowTarget>,
    mut selected: ResMut<SelectedPerson>,
) {
    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed {
            selected.0 = Some(row.0);
        }
    }
    for (interaction, button) in &followers {
        if *interaction == Interaction::Pressed {
            follow.entity = Some(button.0);
            follow.style = crate::camera::FollowStyle::Overhead;
            selected.0 = Some(button.0);
        }
    }
}

/// The recenter button flies the view back to the village banner.
fn handle_toolbar(
    buttons: Query<&Interaction, (Changed<Interaction>, With<RecenterButton>)>,
    world_buttons: Query<&Interaction, (Changed<Interaction>, With<WorldButton>)>,
    people_buttons: Query<&Interaction, (Changed<Interaction>, With<PeopleButton>)>,
    village_buttons: Query<&Interaction, (Changed<Interaction>, With<VillageButton>)>,
    god_buttons: Query<&Interaction, (Changed<Interaction>, With<GodButton>)>,
    mut god_panels: Query<
        &mut Visibility,
        (
            With<GodPanel>,
            Without<WorldPanel>,
            Without<PeoplePanel>,
            Without<HistoryPanel>,
            Without<VillagePanel>,
        ),
    >,
    mut village_panels: Query<
        &mut Visibility,
        (
            With<VillagePanel>,
            Without<WorldPanel>,
            Without<PeoplePanel>,
            Without<HistoryPanel>,
            Without<GodPanel>,
        ),
    >,
    site: Option<Res<crate::villager::SettlementSite>>,
    mut follow: ResMut<crate::camera::FollowTarget>,
    mut rigs: Query<&mut crate::camera::CameraRig>,
    history_buttons: Query<&Interaction, (Changed<Interaction>, With<HistoryButton>)>,
    mut world_panels: Query<
        &mut Visibility,
        (
            With<WorldPanel>,
            Without<PeoplePanel>,
            Without<HistoryPanel>,
            Without<VillagePanel>,
        ),
    >,
    mut people_panels: Query<
        &mut Visibility,
        (
            With<PeoplePanel>,
            Without<WorldPanel>,
            Without<HistoryPanel>,
            Without<VillagePanel>,
        ),
    >,
    mut history_panels: Query<
        &mut Visibility,
        (
            With<HistoryPanel>,
            Without<WorldPanel>,
            Without<PeoplePanel>,
            Without<VillagePanel>,
        ),
    >,
) {
    // Windows are toggles now: each button opens or closes its own, and any
    // mix may stay open. They are real windows — drag them, close them.
    let toggle = |visibility: &mut Visibility| {
        *visibility = if *visibility == Visibility::Hidden {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    };
    for interaction in &world_buttons {
        if *interaction == Interaction::Pressed {
            for mut visibility in &mut world_panels {
                toggle(&mut visibility);
            }
        }
    }
    for interaction in &people_buttons {
        if *interaction == Interaction::Pressed {
            for mut visibility in &mut people_panels {
                toggle(&mut visibility);
            }
        }
    }
    for interaction in &history_buttons {
        if *interaction == Interaction::Pressed {
            for mut visibility in &mut history_panels {
                toggle(&mut visibility);
            }
        }
    }
    for interaction in &village_buttons {
        if *interaction == Interaction::Pressed {
            for mut visibility in &mut village_panels {
                toggle(&mut visibility);
            }
        }
    }
    for interaction in &god_buttons {
        if *interaction == Interaction::Pressed {
            for mut visibility in &mut god_panels {
                toggle(&mut visibility);
            }
        }
    }

    let (Some(site), Ok(mut rig)) = (site, rigs.single_mut()) else {
        return;
    };
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            follow.entity = None;
            rig.target_focus = site.centre;
            rig.target_distance = 70.0;
        }
    }
}

fn spawn_world_panel(mut commands: Commands) {
    let window = ui::window(&mut commands, "THE WORLD", 240.0);
    commands
        .entity(window.root)
        .insert((Name::new("World Panel"), WorldPanel, Visibility::Hidden));
    for (value, label) in [
        (WorldValue::Date, "date"),
        (WorldValue::SkyState, "sky"),
        (WorldValue::Temperature, "warmth"),
        (WorldValue::Country, "country"),
    ] {
        let row = ui::stat_row(&mut commands, window.body, label, None);
        commands.entity(row.value).insert(value);
    }
}

/// Fills the world panel while it is open.
fn update_world_panel(
    clock: Option<Res<crate::calendar::WorldClock>>,
    sky: Option<Res<crate::calendar::Sky>>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    panels: Query<&Visibility, With<WorldPanel>>,
    mut values: Query<(&WorldValue, &mut Text)>,
    weather: Option<Res<crate::weather::Weather>>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }

    for (value, mut text) in &mut values {
        let fresh = match value {
            WorldValue::Date => clock
                .as_ref()
                .map_or_else(|| "-".into(), |c| c.date_phrase()),
            WorldValue::SkyState => weather
                .as_ref()
                .map_or_else(|| "-".to_string(), |w| w.kind().describe().to_string()),
            WorldValue::Temperature => match (&weather, &sky) {
                (Some(weather), Some(sky)) => weather.temperature_word(sky.daylight).to_string(),
                _ => "-".to_string(),
            },
            WorldValue::Country => match (&terrain, &site) {
                (Some(terrain), Some(site)) => {
                    match terrain.biome_at(site.centre.x, site.centre.z) {
                        crate::terrain::Biome::Temperate => "temperate country".into(),
                        crate::terrain::Biome::Boreal => "cold forest country".into(),
                        crate::terrain::Biome::Arid => "dry country".into(),
                        crate::terrain::Biome::Wetland => "wet country".into(),
                        crate::terrain::Biome::Alpine => "high country".into(),
                    }
                }
                _ => "-".to_string(),
            },
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}

fn spawn_hud(mut commands: Commands) {
    // Both panels come from the interface kit; this module only decides what
    // words go in them.
    let hud = ui::panel(&mut commands, ui::Anchor::TopLeft, Some("EGREGORE"), None);
    commands
        .entity(hud.root)
        .insert((Name::new("Debug HUD"), HudPanel));

    // The frame rate lives in the title bar: it describes the whole game, not
    // any one row of the panel.
    if let Some(bar) = hud.title_bar {
        let fps = commands.spawn((HudValue::Fps, ui::dim(""))).id();
        commands.entity(fps).insert(ChildOf(bar));
    }

    let world_rows = [
        (HudValue::Date, "date"),
        (HudValue::You, "you"),
        (HudValue::BeliefTotal, "belief"),
        (HudValue::PrayersOpen, "prayers"),
        (HudValue::Chunks, "chunks"),
        (HudValue::Population, "population"),
        (HudValue::Store, "store"),
        (HudValue::Dead, "dead"),
        (HudValue::AvgHunger, "avg hunger"),
        (HudValue::Hungry, "hungry"),
        (HudValue::Eating, "eating"),
        (HudValue::Watching, "watching"),
    ];
    for (value, label) in world_rows {
        let row = ui::stat_row(&mut commands, hud.root, label, None);
        commands.entity(row.value).insert(value);
    }

    let look_section = commands.spawn(ui::section("LOOK")).id();
    commands.entity(look_section).insert(ChildOf(hud.root));

    let look_rows = [
        (HudValue::Aperture, "aperture", "F2/F3"),
        (HudValue::Focus, "focus", "F4/F5"),
        (HudValue::Visibility, "visibility", "F6/F7"),
        (HudValue::Exposure, "exposure", "F8/F9"),
        (HudValue::Saturation, "saturation", "`/F11"),
        (HudValue::DepthOfField, "depth of field", "F10"),
    ];
    for (value, label, hint) in look_rows {
        let row = ui::stat_row(&mut commands, hud.root, label, Some(hint));
        commands.entity(row.value).insert(value);
    }

    let hints = commands
        .spawn(ui::dim(
            "WASD or MMB-drag pan / QE or RMB orbit / wheel zoom\n\
             hold LMB to grab, flick to throw / Tab hides this panel",
        ))
        .id();
    commands.entity(hints).insert((
        ChildOf(hud.root),
        Node {
            margin: UiRect::top(px(7)),
            ..default()
        },
    ));

    let inspector = ui::panel(&mut commands, ui::Anchor::TopRight, None, Some(250.0));
    commands.entity(inspector.root).insert((
        Name::new("Inspector"),
        InspectorPanel,
        Visibility::Hidden,
    ));
    let name = commands.spawn((InspectorName, ui::heading(""))).id();
    commands.entity(name).insert(ChildOf(inspector.root));
    let subtitle = commands.spawn((InspectorSubtitle, ui::dim(""))).id();
    commands.entity(subtitle).insert(ChildOf(inspector.root));
    let detail = commands.spawn((InspectorDetail, ui::body(""))).id();
    commands.entity(detail).insert(ChildOf(inspector.root));

    let person_rows = [
        (InspectorValue::State, "state"),
        (InspectorValue::Hunger, "hunger"),
        (InspectorValue::Rest, "rest"),
        (InspectorValue::Health, "health"),
        (InspectorValue::Spirits, "spirits"),
        (InspectorValue::Heart, "heart"),
        (InspectorValue::Manner, "manner"),
        (InspectorValue::FaithIn, "faith"),
        (InspectorValue::Work, "work"),
        (InspectorValue::Family, "family"),
        (InspectorValue::Seen, "seen you"),
    ];
    for (value, label) in person_rows {
        let row = ui::stat_row(&mut commands, inspector.root, label, None);
        commands.entity(row.value).insert(value);
        commands.entity(row.row).insert(InspectorPersonBlock);
    }

    let life_header = commands.spawn(ui::section("LIFE")).id();
    commands
        .entity(life_header)
        .insert((ChildOf(inspector.root), InspectorPersonBlock));
    let life = commands.spawn((InspectorLife, ui::dim(""))).id();
    commands
        .entity(life)
        .insert((ChildOf(inspector.root), InspectorPersonBlock));

    let memory_header = commands.spawn(ui::section("HAS SEEN")).id();
    commands
        .entity(memory_header)
        .insert((ChildOf(inspector.root), InspectorPersonBlock));
    let memories = commands.spawn((InspectorMemories, ui::dim(""))).id();
    commands
        .entity(memories)
        .insert((ChildOf(inspector.root), InspectorPersonBlock));
}

/// Hunger, in the villager's terms.
fn hunger_word(hunger: f32) -> &'static str {
    match hunger {
        h if h < 0.2 => "sated",
        h if h < 0.35 => "content",
        h if h < 0.6 => "peckish",
        h if h < 0.85 => "hungry",
        h if h < 0.99 => "famished",
        _ => "starving",
    }
}

/// Weariness, in the villager's terms.
fn rest_word(rest: f32) -> &'static str {
    match rest {
        r if r < 0.3 => "well rested",
        r if r < 0.6 => "wakeful",
        r if r < 0.85 => "tired",
        _ => "exhausted",
    }
}

/// Spirits, in the villager's terms.
fn spirits_word(spirits: f32) -> &'static str {
    match spirits {
        s if s > 0.75 => "bright",
        s if s > 0.5 => "steady",
        s if s > 0.3 => "weary",
        _ => "hollow",
    }
}

/// Harm, in the villager's terms.
fn health_word(harm: f32) -> &'static str {
    match harm {
        h if h < 0.05 => "hale",
        h if h < 0.3 => "bruised",
        h if h < 0.6 => "hurt",
        h if h < 0.85 => "failing",
        _ => "dying",
    }
}

/// Who someone is, in a phrase.
fn person_phrase(sex: Sex, age: Age) -> &'static str {
    match (age, sex) {
        (Age::Child, Sex::Female) => "a girl",
        (Age::Child, Sex::Male) => "a boy",
        (Age::Adult, Sex::Female) => "a woman",
        (Age::Adult, Sex::Male) => "a man",
        (Age::Elder, Sex::Female) => "an elder woman",
        (Age::Elder, Sex::Male) => "an elder man",
    }
}

/// A person's family, in a phrase: marriage first, then parentage, and the
/// founding generation belongs to nobody but the beginning.
fn family_phrase(
    spouse: Option<&Spouse>,
    parentage: Option<&Parentage>,
    kin: &Query<&Person>,
    corpses: &Query<Option<&crate::creature::Vitality>, With<crate::creature::Corpse>>,
) -> String {
    if let Some(spouse) = spouse {
        let name = kin
            .get(spouse.0)
            .map(|p| p.name.clone())
            .unwrap_or_else(|_| "one now gone".into());
        return if corpses.get(spouse.0).is_ok() {
            format!("widowed of {name}")
        } else {
            format!("wed to {name}")
        };
    }
    if let Some(parents) = parentage {
        let mother = kin
            .get(parents.mother)
            .map(|p| p.name.clone())
            .unwrap_or_else(|_| "a mother now gone".into());
        let father = kin
            .get(parents.father)
            .map(|p| p.name.clone())
            .unwrap_or_else(|_| "a father now gone".into());
        return format!("born to {mother} and {father}");
    }
    "of the first families".to_string()
}

/// What they are doing, favouring a reaction over routine: fear interrupts lunch.
fn state_phrase(activity: Option<&Activity>, reaction: Option<&Reaction>) -> &'static str {
    if let Some(reaction) = reaction {
        return reaction.kind.describe();
    }
    match activity {
        Some(Activity::Idle) | None => "at ease",
        Some(Activity::Wandering) => "wandering",
        Some(Activity::SeekingFood(_)) => "off to find food",
        Some(Activity::Eating(_)) => "eating",
        Some(Activity::Working) => "at their work",
        Some(Activity::VisitingStore) => "fetching from the store",
        Some(Activity::TendingFire) => "feeding the fire",
        Some(Activity::Hauling) => "carrying timber home",
        Some(Activity::Praying) => "praying",
        Some(Activity::Sleeping) => "asleep",
        Some(Activity::Mourning) => "mourning",
        Some(Activity::Chatting) => "deep in conversation",
        Some(Activity::Sheltering) => "waiting out the rain",
        Some(Activity::Bearing) => "bearing the dead",
    }
}

/// Nudges a value and reports whether it actually moved, so the caller only marks
/// the resource changed when something did.
fn nudge(value: &mut f32, delta: f32, lo: f32, hi: f32) -> bool {
    let next = (*value + delta).clamp(lo, hi);
    let moved = (next - *value).abs() > f32::EPSILON;
    *value = next;
    moved
}

fn handle_tuning_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut look: ResMut<LookSettings>,
    mut state: ResMut<DebugState>,
) {
    // Tab, because function keys on a laptop keyboard are behind a modifier
    // and a prayer. F1 still works out of habit.
    if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::F1) {
        state.hud_visible = !state.hud_visible;
    }

    // Bypass change detection unless something is actually edited: the look settings
    // rebuild the render target when they change, and doing that every frame would
    // thrash the GPU.
    let mut changed = false;

    // Aperture runs backwards from how it reads: a *lower* f-stop is a shallower,
    // more miniature look, so F2 opens the lens up and F3 stops it down.
    if keys.just_pressed(KeyCode::F2) {
        changed |= nudge(&mut look.aperture, -1.0, 0.0, 40.0);
    }
    if keys.just_pressed(KeyCode::F3) {
        changed |= nudge(&mut look.aperture, 1.0, 0.0, 40.0);
    }

    if keys.just_pressed(KeyCode::F4) {
        changed |= nudge(&mut look.focus_bias, -0.05, 0.2, 3.0);
    }
    if keys.just_pressed(KeyCode::F5) {
        changed |= nudge(&mut look.focus_bias, 0.05, 0.2, 3.0);
    }

    if keys.just_pressed(KeyCode::F6) {
        changed |= nudge(&mut look.visibility, -0.04, 0.1, 1.0);
    }
    if keys.just_pressed(KeyCode::F7) {
        changed |= nudge(&mut look.visibility, 0.04, 0.1, 1.0);
    }

    if keys.just_pressed(KeyCode::F8) {
        changed |= nudge(&mut look.exposure, -0.1, -3.0, 3.0);
    }
    if keys.just_pressed(KeyCode::F9) {
        changed |= nudge(&mut look.exposure, 0.1, -3.0, 3.0);
    }

    if keys.just_pressed(KeyCode::F11) {
        changed |= nudge(&mut look.saturation, 0.05, 0.0, 3.0);
    }
    if keys.just_pressed(KeyCode::Backquote) {
        changed |= nudge(&mut look.saturation, -0.05, 0.0, 3.0);
    }

    // Toggle depth of field off and back, which is the fastest way to see what it
    // is actually contributing.
    if keys.just_pressed(KeyCode::F10) {
        look.aperture = if look.depth_of_field_enabled() {
            0.0
        } else {
            12.0
        };
        changed = true;
    }

    if !changed {
        // Nothing edited — undo the implicit change mark from taking `ResMut`.
        look.bypass_change_detection();
    }
}

fn update_hud(
    time: Res<Time>,
    mut fps_cache: Local<(f32, f64)>,
    state: Res<DebugState>,
    look: Res<LookSettings>,
    diagnostics: Res<DiagnosticsStore>,
    clock: Option<Res<crate::calendar::WorldClock>>,
    divine: (
        Option<Res<crate::villager::DivineName>>,
        Option<Res<crate::villager::belief::Belief>>,
        Option<Res<crate::villager::belief::Legend>>,
        Query<(), With<crate::villager::belief::Prayer>>,
    ),
    chunks: Option<Res<LoadedChunks>>,
    villagers: Query<(&Needs, &Activity), With<Villager>>,
    corpses: Query<(), (With<crate::creature::Corpse>, With<Person>)>,
    reactions: Query<(), (With<Reaction>, With<Person>)>,
    stores: Query<&crate::villager::work::Stockpile>,
    mut panels: Query<&mut Visibility, With<HudPanel>>,
    mut values: Query<(&HudValue, &mut Text)>,
) {
    let (divine_name, belief, legend, prayers) = divine;
    let Ok(mut visibility) = panels.single_mut() else {
        return;
    };

    *visibility = if state.hud_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    if !state.hud_visible {
        return;
    }

    // Refreshed once a second. A readout that changes every frame cannot be read,
    // only watched flicker.
    if time.elapsed_secs() >= fps_cache.0 {
        fps_cache.0 = time.elapsed_secs() + 1.0;
        fps_cache.1 = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed())
            .unwrap_or(0.0);
    }
    let fps = fps_cache.1;

    let chunk_count = chunks.map_or(0, |c| c.count());
    let population = villagers.iter().count();
    let dead = corpses.iter().count();
    let average_hunger = if population == 0 {
        0.0
    } else {
        villagers.iter().map(|(n, _)| n.hunger).sum::<f32>() / population as f32
    };
    let hungry = villagers.iter().filter(|(n, _)| n.hunger > 0.35).count();
    let eating = villagers
        .iter()
        .filter(|(_, a)| matches!(a, Activity::Eating(_)))
        .count();
    let watching = reactions.iter().count();

    for (value, mut text) in &mut values {
        let fresh = match value {
            HudValue::Fps => format!("{fps:.0} fps"),
            HudValue::Date => clock
                .as_ref()
                .map_or_else(|| "-".to_string(), |c| c.date_phrase()),
            HudValue::You => divine_name.as_ref().map_or_else(
                || "unnamed, so far".to_string(),
                |name| {
                    let epithet = legend
                        .as_ref()
                        .and_then(|l| l.epithet)
                        .map(|e| format!(" {e}"))
                        .unwrap_or_default();
                    format!("they call you {}{epithet}", name.0)
                },
            ),
            HudValue::BeliefTotal => belief
                .as_ref()
                .map_or_else(|| "-".into(), |b| format!("{:.1}", b.available())),
            HudValue::PrayersOpen => prayers.iter().count().to_string(),
            HudValue::Store => stores.iter().next().map_or_else(
                || "-".to_string(),
                |s| {
                    format!(
                        "{:.0} food / {:.0} timber / {:.0} stone",
                        s.food, s.timber, s.stone,
                    )
                },
            ),
            HudValue::Chunks => chunk_count.to_string(),
            HudValue::Population => population.to_string(),
            HudValue::Dead => dead.to_string(),
            HudValue::AvgHunger => format!("{average_hunger:.2}"),
            HudValue::Hungry => hungry.to_string(),
            HudValue::Eating => eating.to_string(),
            HudValue::Watching => watching.to_string(),
            HudValue::Aperture => format!("f/{:.0}", look.aperture),
            HudValue::Focus => format!("{:.2}x", look.focus_bias),
            HudValue::Visibility => format!("{:.2}", look.visibility),
            HudValue::Exposure => format!("{:+.1}", look.exposure),
            HudValue::Saturation => format!("{:.2}", look.saturation),
            HudValue::DepthOfField => if look.depth_of_field_enabled() {
                "on"
            } else {
                "off"
            }
            .to_string(),
        };
        // Only touch the text when it actually changed; rewriting every value
        // every frame re-runs text layout for the whole panel.
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}

/// Fills the inspector with whoever the hand is over or carrying.
///
/// A living person gets the full dossier: state, hunger, health, heart, what
/// they have seen you do and how they would put it. This is the seed of the
/// doctrine panel — the point of names, temperaments and memories is that you
/// can look at one villager and find a person rather than a unit, and this is
/// where that person will eventually speak.
fn update_inspector(
    hand: Res<DivineHand>,
    follow: Res<crate::camera::FollowTarget>,
    people: Query<(
        &Person,
        &Temperament,
        &Witnessed,
        Option<&Reaction>,
        Option<&Needs>,
        Option<&Activity>,
        Option<&crate::creature::Vitality>,
        Option<&crate::creature::genome::CreatureGenome>,
        Option<&crate::villager::Spouse>,
        Option<&crate::villager::Parentage>,
        Option<&MemberOf>,
        Option<&Chronicle>,
        Option<&crate::villager::work::Vocation>,
        (
            Option<&Morale>,
            Option<&crate::villager::belief::Faith>,
            Option<&crate::villager::traits::Traits>,
        ),
    )>,
    corpse_check: Query<Option<&crate::creature::Vitality>, With<crate::creature::Corpse>>,
    cards: (
        Query<(&crate::villager::rites::Grave, &Person, &Chronicle), Without<Temperament>>,
        Query<&crate::villager::work::StorePile>,
        Option<Res<crate::villager::work::StoreTrends>>,
        Option<Res<crate::villager::SettlementSite>>,
        Query<&crate::villager::work::Building>,
    ),
    kin_names: Query<&Person>,
    settlements: Query<&Settlement>,
    huts: Query<(), With<crate::villager::work::Hut>>,
    rising: Query<(
        &crate::villager::work::ConstructionSite,
        &crate::villager::work::Blueprint,
    )>,
    households: Query<
        (&Person, &crate::villager::home::Home, &Activity),
        Without<crate::creature::Corpse>,
    >,
    settlement_info: Query<(&Settlement, &crate::villager::work::Stockpile)>,
    residents: Query<
        (&MemberOf, &crate::creature::genome::CreatureGenome),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    names: Query<&Name>,
    mut panels: Query<&mut Visibility, (With<InspectorPanel>, Without<InspectorPersonBlock>)>,
    mut person_block: Query<
        (&mut Visibility, &mut Node),
        (With<InspectorPersonBlock>, Without<InspectorPanel>),
    >,
    mut texts: ParamSet<(
        Query<&mut Text, With<InspectorName>>,
        Query<&mut Text, With<InspectorSubtitle>>,
        Query<
            (&mut Text, &mut Visibility),
            (
                With<InspectorDetail>,
                Without<InspectorPanel>,
                Without<InspectorPersonBlock>,
            ),
        >,
        Query<(&InspectorValue, &mut Text)>,
        Query<&mut Text, With<InspectorMemories>>,
        Query<&mut Text, With<InspectorLife>>,
    )>,
) {
    let Ok(mut visibility) = panels.single_mut() else {
        return;
    };

    // Whoever the hand holds or hovers — and failing that, whoever the camera
    // is following. The card of a follow stays up for the whole ride.
    let subject = hand
        .held
        .as_ref()
        .map(|h| h.entity)
        .or(hand.hovered)
        .or(follow.entity);
    let Some(entity) = subject else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Visible;

    let held = hand.held.is_some();
    let corpse = corpse_check.get(entity);

    // A pile in the square: the store it fronts, and which way it is going.
    if let Ok(pile) = cards.1.get(entity) {
        use crate::villager::work::PileKind;
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Hidden;
            node.display = Display::None;
        }
        let store = cards
            .3
            .as_ref()
            .and_then(|site| settlement_info.get(site.settlement).ok())
            .map(|(_, store)| store);
        let (title, amount) = match (pile.0, store) {
            (PileKind::Food, Some(s)) => ("The food store", s.food),
            (PileKind::Timber, Some(s)) => ("The woodpile", s.timber),
            (PileKind::Stone, Some(s)) => ("The stone pile", s.stone),
            (_, None) => ("The stores", 0.0),
        };
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(title);
        }
        if let Ok(mut subtitle) = texts.p1().single_mut() {
            let fresh = match pile.0 {
                PileKind::Food => format!("{amount:.0} food laid by"),
                PileKind::Timber => format!("{amount:.0} logs on the pile"),
                PileKind::Stone => format!("{amount:.0} blocks cut and stacked"),
            };
            if subtitle.0 != fresh {
                *subtitle = Text::new(fresh);
            }
        }
        if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
            let rate = cards
                .2
                .as_ref()
                .map_or(0.0, |trends| trends.rate_per_minute(pile.0));
            let fresh = if rate > 0.5 {
                format!("growing - about {rate:.0} more each minute")
            } else if rate < -0.5 {
                format!("being drawn down - about {:.0} a minute", -rate)
            } else {
                "holding steady".to_string()
            };
            if detail.0 != fresh {
                *detail = Text::new(fresh);
            }
            *detail_visibility = Visibility::Inherited;
        }
        return;
    }

    // A storehouse or granary: what shelters under its roof, and the drift.
    if let Ok(building) = cards.4.get(entity) {
        use crate::villager::work::{BuildingKind, PileKind};
        let holds: &[(PileKind, &str)] = match building.kind {
            BuildingKind::Storehouse => &[(PileKind::Timber, "logs"), (PileKind::Stone, "stone")],
            BuildingKind::Granary => &[(PileKind::Food, "food")],
            _ => &[],
        };
        if !holds.is_empty() {
            for (mut block, mut node) in &mut person_block {
                *block = Visibility::Hidden;
                node.display = Display::None;
            }
            let store = cards
                .3
                .as_ref()
                .and_then(|site| settlement_info.get(site.settlement).ok())
                .map(|(_, store)| store);
            if let Ok(mut name) = texts.p0().single_mut() {
                *name = Text::new(building.kind.name());
            }
            if let Ok(mut subtitle) = texts.p1().single_mut() {
                let fresh = holds
                    .iter()
                    .map(|(kind, word)| {
                        let amount = store.map_or(0.0, |s| match kind {
                            PileKind::Food => s.food,
                            PileKind::Timber => s.timber,
                            PileKind::Stone => s.stone,
                        });
                        format!("{amount:.0} {word}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let fresh = format!("{fresh} under its roof");
                if subtitle.0 != fresh {
                    *subtitle = Text::new(fresh);
                }
            }
            if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
                let fresh = holds
                    .iter()
                    .map(|(kind, word)| {
                        let rate = cards
                            .2
                            .as_ref()
                            .map_or(0.0, |trends| trends.rate_per_minute(*kind));
                        if rate > 0.5 {
                            format!("{word}: growing, about {rate:.0} a minute")
                        } else if rate < -0.5 {
                            format!("{word}: being drawn down, {:.0} a minute", -rate)
                        } else {
                            format!("{word}: holding steady")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if detail.0 != fresh {
                    *detail = Text::new(fresh);
                }
                *detail_visibility = Visibility::Inherited;
            }
            return;
        }
    }

    // A grave: the life that ended under it, read back from the stone.
    if let Ok((grave, person, story)) = cards.0.get(entity) {
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Hidden;
            node.display = Display::None;
        }
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(format!("The grave of {}", person.name));
        }
        if let Ok(mut subtitle) = texts.p1().single_mut() {
            *subtitle = Text::new(format!("laid to rest on day {}", grave.day));
        }
        if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
            let tail = story.events.len().saturating_sub(6);
            let life = story.events[tail..]
                .iter()
                .map(|event| format!("d{}  {}", event.day, event.text))
                .collect::<Vec<_>>()
                .join("\n");
            *detail = Text::new(life);
            *detail_visibility = Visibility::Inherited;
        }
        return;
    }

    // A living person: the full dossier.
    if let Ok((
        person,
        temperament,
        witnessed,
        reaction,
        needs,
        activity,
        vitality,
        genome,
        spouse,
        parentage,
        member_of,
        chronicle,
        vocation,
        (morale, faith, manner),
    )) = people.get(entity)
        && corpse.is_err()
    {
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Inherited;
            node.display = Display::Flex;
        }
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(person.name.clone());
        }

        let who = genome.map_or("a soul", |g| person_phrase(g.sex, g.age));
        let home = member_of
            .and_then(|m| settlements.get(m.0).ok())
            .map_or_else(|| "the wilds".to_string(), |s| s.name.clone());
        if let Ok(mut subtitle) = texts.p1().single_mut() {
            *subtitle = Text::new(format!("{who} of {home}"));
        }
        if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
            *detail = Text::new("");
            *detail_visibility = Visibility::Hidden;
        }

        let hunger = needs.map_or(0.0, |n| n.hunger);
        let harm = vitality.map_or(0.0, |v| v.harm);
        for (value, mut text) in &mut texts.p3() {
            let fresh = match value {
                InspectorValue::State => if held {
                    "in your grasp"
                } else {
                    state_phrase(activity, reaction)
                }
                .to_string(),
                InspectorValue::Hunger => hunger_word(hunger).to_string(),
                InspectorValue::Rest => needs.map_or("wakeful", |n| rest_word(n.rest)).to_string(),
                InspectorValue::Health => health_word(harm).to_string(),
                InspectorValue::Spirits => morale
                    .map_or("steady", |m| spirits_word(m.spirits))
                    .to_string(),
                InspectorValue::Heart => temperament.describe().to_string(),
                InspectorValue::Manner => {
                    manner.map_or("unremarkable".to_string(), |m| m.describe())
                }
                InspectorValue::FaithIn => faith
                    .map_or("has never wondered", |f| f.describe())
                    .to_string(),
                InspectorValue::Work => vocation.map_or("none yet", |v| v.describe()).to_string(),
                InspectorValue::Family => {
                    family_phrase(spouse, parentage, &kin_names, &corpse_check)
                }
                InspectorValue::Seen => {
                    if witnessed.is_innocent() && witnessed.secondhand > 0 {
                        "only in stories".to_string()
                    } else if witnessed.is_innocent() {
                        "never".to_string()
                    } else {
                        format!("{} times", witnessed.total)
                    }
                }
            };
            if text.0 != fresh {
                *text = Text::new(fresh);
            }
        }

        if let Ok(mut memories) = texts.p4().single_mut() {
            let fresh = if witnessed.recent.is_empty() {
                "nothing they could not explain".to_string()
            } else {
                witnessed
                    .recent
                    .iter()
                    .take(4)
                    .map(|kind| format!("- {}", kind.describe()))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if memories.0 != fresh {
                *memories = Text::new(fresh);
            }
        }

        if let Ok(mut life) = texts.p5().single_mut() {
            let fresh = chronicle.map_or_else(
                || "unwritten".to_string(),
                |chronicle| {
                    let events = &chronicle.events;
                    let tail = events.len().saturating_sub(4);
                    events[tail..]
                        .iter()
                        .map(|event| format!("d{}  {}", event.day, event.text))
                        .collect::<Vec<_>>()
                        .join("\n")
                },
            );
            if life.0 != fresh {
                *life = Text::new(fresh);
            }
        }
        return;
    }

    // Anything else — a corpse, an animal, a bush — gets a name and one line.
    for (mut block, mut node) in &mut person_block {
        *block = Visibility::Hidden;
        node.display = Display::None;
    }

    let (title, description) = if huts.get(entity).is_ok() {
        // A finished house: the household, and what each of them is up to.
        let village = settlements
            .iter()
            .next()
            .map_or_else(|| "the village".to_string(), |s| s.name.clone());
        let residents: Vec<String> = households
            .iter()
            .filter(|(_, home, _)| home.0 == entity)
            .map(|(person, _, activity)| {
                format!("{} - {}", person.name, state_phrase(Some(activity), None))
            })
            .collect();
        (
            format!("A house of {village}"),
            if residents.is_empty() {
                "no one yet calls it home".to_string()
            } else {
                residents.join("\n")
            },
        )
    } else if let Ok((construction, plan)) = rising.get(entity) {
        // Say what the site is actually waiting on: a foundation short of
        // stone blocks the carpenters, and that must be legible, or an
        // honest wait reads as a broken village.
        let stone_cost = plan.kind.stone_cost();
        let line = if construction.stone_laid < stone_cost {
            format!(
                "waiting on stone - {:.0} of {:.0} laid in the foundation",
                construction.stone_laid, stone_cost,
            )
        } else {
            format!(
                "{:.0} of {:.0} timber worked into it",
                construction.progress.min(plan.kind.timber_cost()),
                plan.kind.timber_cost(),
            )
        };
        (format!("{}, rising", plan.kind.name()), line)
    } else if let Ok((settlement, store)) = settlement_info.get(entity) {
        // The banner: the settlement's own dossier.
        let mut grown = 0;
        let mut children = 0;
        for (member, genome) in &residents {
            if member.0 == entity {
                match genome.age {
                    Age::Child => children += 1,
                    _ => grown += 1,
                }
            }
        }
        (
            settlement.name.clone(),
            format!(
                "a village, founded on day {}\n\
                 {grown} grown, {children} children\n\
                 stores  {:.0} food, {:.0} timber, {:.0} stone",
                settlement.founded, store.food, store.timber, store.stone,
            ),
        )
    } else if let Ok(vitality) = corpse {
        let name = people
            .get(entity)
            .map(|(person, ..)| format!("the body of {}", person.name))
            .unwrap_or_else(|_| "a body".to_string());
        let cause = match vitality {
            Some(v) if v.violent => "broken against the earth",
            Some(_) => "wasted away by hunger",
            None => "still",
        };
        (name, cause.to_string())
    } else {
        let what = names
            .get(entity)
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|_| "something".into());
        (
            what,
            if held {
                "in your grasp"
            } else {
                "beneath your hand"
            }
            .to_string(),
        )
    };

    if let Ok(mut name) = texts.p0().single_mut() {
        *name = Text::new(title);
    }
    if let Ok(mut subtitle) = texts.p1().single_mut() {
        *subtitle = Text::new("");
    }
    if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
        *detail = Text::new(description);
        *detail_visibility = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nudge_reports_movement_and_clamps() {
        let mut value = 0.5;
        assert!(nudge(&mut value, 0.1, 0.0, 1.0));
        assert!((value - 0.6).abs() < 1e-6);

        // Already at the ceiling: no movement, no change reported.
        let mut value = 1.0;
        assert!(!nudge(&mut value, 0.1, 0.0, 1.0));
        assert_eq!(value, 1.0);

        let mut value = 0.0;
        assert!(!nudge(&mut value, -0.1, 0.0, 1.0));
        assert_eq!(value, 0.0);
    }

    #[test]
    fn nudge_never_leaves_the_range() {
        let mut value = 0.5;
        for _ in 0..1_000 {
            nudge(&mut value, 0.3, 0.0, 1.0);
            assert!((0.0..=1.0).contains(&value));
        }
        for _ in 0..1_000 {
            nudge(&mut value, -0.3, 0.0, 1.0);
            assert!((0.0..=1.0).contains(&value));
        }
    }
}
