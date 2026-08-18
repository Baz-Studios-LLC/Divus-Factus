//! The corner HUD and the live tuning keys.

use crate::render::LookSettings;
use crate::witness::Reaction;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use super::*;
use crate::palette;
use crate::terrain::LoadedChunks;
use crate::ui;

/// The stats-and-tuning panel, top left.
#[derive(Component)]
pub(crate) struct HudPanel;

/// The dev overlay: a bare corner readout on the backquote key, apart from
/// the heavy F1 instrument panel. Starts with the frame rate; grows a line
/// at a time as a number earns its place.
#[derive(Component)]
pub(crate) struct DevOverlay;

/// Which live readout a HUD value text shows. One enum, one update system —
/// adding a row to the HUD is adding a variant, a label and a match arm.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HudValue {
    Fps,
    Date,
    You,
    BeliefTotal,
    PrayersOpen,
    Chunks,
    Altitude,
    Detail,
    Population,
    Store,
    Dead,
    AvgHunger,
    Hungry,
    Eating,
    Watching,
    Teller,
    Aperture,
    Focus,
    Exposure,
    Saturation,
    Frost,
    DepthOfField,
}

/// How wide the dev panel holds itself.
///
/// Wide enough for the zoom row wearing its longest clause - level span, shown,
/// standing, and a backlog in the thousands - because that row is the widest
/// thing in the panel and the only one that changes shape.
const HUD_WIDTH: f32 = 400.0;

pub(crate) fn spawn_hud(mut commands: Commands, mouse: Res<crate::keymap::MouseScheme>) {
    // The dev overlay, hidden until the backquote asks for it. It borrows
    // the HUD's own live values — a row here is a HudValue and a Node.
    commands.spawn((
        DevOverlay,
        // Hand-built rather than ui::dim, which already carries a
        // TextFont - a bundle with two of anything panics, the same trap
        // the splash line fell into. Dressed like the date's own small
        // line, and centered at the top of the screen where it reads as
        // an instrument rather than a subtitle.
        Text::new(""),
        ui::SerifFace,
        TextFont {
            font_size: FontSize::Px(17.0),
            ..default()
        },
        TextColor(Color::WHITE.with_alpha(0.66)),
        TextLayout {
            justify: Justify::Center,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            // The whole width, centering its own words: the text can
            // change length every half second, and a pinned left edge
            // would make it crawl.
            left: px(0),
            right: px(0),
            top: px(10),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(210),
    ));

    // Both panels come from the interface kit; this module only decides what
    // words go in them.
    // A HELD WIDTH, which is what the kit's `min_width` is for: "so panels
    // with changing content hold their shape instead of breathing." Every row
    // here changes as the world does, and the zoom row changes SHAPE - it grows
    // a whole clause while the tree catches up. Brett: "This window changes
    // when zoom level gets owed appended to the end, can we give this a stable
    // width?" Sized to the widest row's widest state, so nothing here can push
    // the edge out.
    let hud = ui::panel(
        &mut commands,
        ui::Anchor::TopLeft,
        Some("DIVUS FACTUS"),
        Some(HUD_WIDTH),
    );
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
        (HudValue::Altitude, "altitude"),
        (HudValue::Detail, "zoom level"),
        (HudValue::Population, "population"),
        (HudValue::Store, "store"),
        (HudValue::Dead, "dead"),
        (HudValue::AvgHunger, "avg hunger"),
        (HudValue::Hungry, "hungry"),
        (HudValue::Eating, "eating"),
        (HudValue::Watching, "watching"),
        (HudValue::Teller, "teller"),
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
        (HudValue::Exposure, "exposure", "F8/F9"),
        (HudValue::Saturation, "saturation", "F11/shift-F11"),
        (HudValue::Frost, "frost", "F6/F7"),
        (HudValue::DepthOfField, "depth of field", "F10"),
    ];
    for (value, label, hint) in look_rows {
        let row = ui::stat_row(&mut commands, hud.root, label, Some(hint));
        commands.entity(row.value).insert(value);
    }

    // Written from the scheme rather than by hand, so the help can never
    // contradict the buttons - it did, the moment the buttons became a
    // setting.
    let hints = commands
        .spawn(ui::dim(format!(
            "{land}-drag grabs the land, MMB orbits, the wheel zooms; the \n\
             letter keys live in the codex settings / hold {action} to pick \n\
             up, flick to throw / F1 hides this panel",
            land = mouse.land_name(),
            action = mouse.action_name(),
        )))
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

    // A dwelling's card has its own quiet hierarchy: facts in aligned rows,
    // people in a small living line, and a concern that only exists when the
    // simulation has something real to put there.
    let house_block = |entity: Entity, commands: &mut Commands| {
        commands.entity(entity).insert(InspectorHouseBlock);
    };
    let home_header = ui::section_header(&mut commands, inspector.root, "THE HOME");
    house_block(home_header, &mut commands);
    for (value, label) in [
        (InspectorHouseValue::Beds, "beds"),
        (InspectorHouseValue::Stores, "stores"),
    ] {
        let row = ui::stat_row(&mut commands, inspector.root, label, None);
        house_block(row.row, &mut commands);
        commands
            .entity(row.value)
            .insert((InspectorHouseBlock, value));
    }
    let household_header = ui::section_header(&mut commands, inspector.root, "HOUSEHOLD");
    house_block(household_header, &mut commands);
    for (value, label) in [
        (InspectorHouseValue::Mood, "mood"),
        (InspectorHouseValue::Faith, "faith"),
    ] {
        let row = ui::stat_row(&mut commands, inspector.root, label, None);
        house_block(row.row, &mut commands);
        commands
            .entity(row.value)
            .insert((InspectorHouseBlock, value));
    }
    let life = commands
        .spawn((InspectorHouseValue::Life, ui::body("")))
        .id();
    commands
        .entity(life)
        .insert((ChildOf(inspector.root), InspectorHouseBlock));
    let concern_header = ui::section_header(&mut commands, inspector.root, "CONCERN");
    commands
        .entity(concern_header)
        .insert((InspectorHouseBlock, InspectorHouseConcern));
    let concern = commands
        .spawn((InspectorHouseValue::Concern, ui::body("")))
        .id();
    commands.entity(concern).insert((
        ChildOf(inspector.root),
        InspectorHouseBlock,
        InspectorHouseConcern,
        TextColor(palette::shade(&palette::CLOTH_PINK, 0.92)),
    ));

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
        (InspectorValue::Feelings, "feelings"),
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

/// Nudges a value and reports whether it actually moved, so the caller only marks
/// the resource changed when something did.
pub(crate) fn nudge(value: &mut f32, delta: f32, lo: f32, hi: f32) -> bool {
    let next = (*value + delta).clamp(lo, hi);
    let moved = (next - *value).abs() > f32::EPSILON;
    *value = next;
    moved
}

pub(crate) fn handle_tuning_input(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    playing: Res<State<crate::GameState>>,
    mut look: ResMut<LookSettings>,
    mut state: ResMut<DebugState>,
    mut codex: Query<&mut Visibility, With<super::VillagePanel>>,
) {
    // Tab belongs to the player now: it opens the codex, the game's own
    // book. The developer's instrument panel retreats to F1.
    if keymap.just_pressed(&keys, crate::keymap::Deed::Codex)
        && matches!(playing.get(), crate::GameState::Playing)
    {
        for mut visibility in &mut codex {
            *visibility = if *visibility == Visibility::Hidden {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    // Escape closes an open codex - the book shuts before the pause menu
    // would rise, and the pause menu stands aside while the book is up.
    if keys.just_pressed(KeyCode::Escape) {
        for mut visibility in &mut codex {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        }
    }
    if keys.just_pressed(KeyCode::F1) {
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

    if keys.just_pressed(KeyCode::F8) {
        changed |= nudge(&mut look.exposure, -0.1, -3.0, 3.0);
    }
    if keys.just_pressed(KeyCode::F9) {
        changed |= nudge(&mut look.exposure, 0.1, -3.0, 3.0);
    }

    // The reading frost, thinned or thickened ten glass pixels at a time.
    if keys.just_pressed(KeyCode::F6) {
        changed |= nudge(&mut look.frost, -10.0, 0.0, 270.0);
    }
    if keys.just_pressed(KeyCode::F7) {
        changed |= nudge(&mut look.frost, 10.0, 0.0, 270.0);
    }

    // Both hands of saturation live on F11 - richer plain, paler shifted.
    // It rode the backquote once (every overlay toggle drained the world's
    // color), then F12, where every press also tripped the screenshot key.
    if keys.just_pressed(KeyCode::F11) {
        let step = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            -0.05
        } else {
            0.05
        };
        changed |= nudge(&mut look.saturation, step, 0.0, 3.0);
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

pub(crate) fn update_hud(
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
    // Paired: update_hud sits exactly on Bevy's sixteen-parameter limit, so
    // a new one has to ride with a relative. These two are the same kind of
    // fact — how much world is standing.
    loaded: (Option<Res<LoadedChunks>>, Res<crate::globe::PlanetDetail>),
    rigs: Query<&crate::camera::CameraRig>,
    villagers: Query<(&Needs, &Activity), With<Villager>>,
    corpses: Query<(), (With<crate::creature::Corpse>, With<Person>)>,
    reactions: Query<(), (With<Reaction>, With<Person>)>,
    stores: Query<&crate::villager::work::Stockpile>,
    voice: Option<Res<crate::sermo::Tongue>>,
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

    let (chunks, detail) = loaded;
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
            // THE CLOCK BESIDE THE FRAMERATE. Brett: "can we get a timer for
            // how long the game has been running?"
            //
            // Because a framerate with no age beside it cannot be argued
            // about. Half of one afternoon's performance hunt went into not
            // knowing whether a number came from a ten-second-old world still
            // streaming its patches in or a ten-minute-old one that had
            // settled - and those are opposite problems with opposite fixes.
            HudValue::Fps => {
                let up = time.elapsed_secs() as u32;
                format!("{}:{:02}   {fps:.0} fps", up / 60, up % 60)
            }
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
                    let mut line = format!(
                        "{:.0} food / {:.0} timber / {:.0} stone",
                        s.food(),
                        s.timber,
                        s.stone,
                    );
                    if s.ore + s.iron > 0.05 {
                        line += &format!(" / {:.0} ore / {:.1} iron", s.ore, s.iron);
                    }
                    if s.clay > 0.05 {
                        line += &format!(" / {:.0} clay", s.clay);
                    }
                    if s.incense + s.dye > 0.05 {
                        line += &format!(" / {:.1} incense / {:.1} dye", s.incense, s.dye);
                    }
                    line
                },
            ),
            HudValue::Chunks => chunk_count.to_string(),
            // The height of the climb, from the ground's zoom or the orbit's
            // own, whichever owns the camera - asked for from the middle of
            // the round-world work, where every gray layer in the sky turned
            // out to live at a different altitude.
            HudValue::Altitude => {
                format!("{:.0}", rigs.iter().next().map_or(0.0, |rig| rig.distance))
            }
            // What the altitude bought. The planet is a quadtree, so "zoom
            // level" is literally a level: which depths of it are on screen,
            // how many patches that is, how many are resident, and — while the
            // tree is still catching up — how many are owed. The last number
            // is the one that says "you are watching it build".
            HudValue::Detail => {
                let owed = if detail.owed > 0 {
                    format!(" / {} owed", detail.owed)
                } else {
                    String::new()
                };
                format!(
                    "L{}-{} / {} shown of {}{owed}",
                    detail.coarsest, detail.finest, detail.shown, detail.built
                )
            }
            HudValue::Population => population.to_string(),
            HudValue::Dead => dead.to_string(),
            HudValue::AvgHunger => format!("{average_hunger:.2}"),
            HudValue::Hungry => hungry.to_string(),
            HudValue::Eating => eating.to_string(),
            HudValue::Watching => watching.to_string(),
            HudValue::Teller => voice.as_ref().map_or_else(
                || "silent".to_string(),
                |tongue| {
                    // WHICH VOICE is answering, because "off" and "no key"
                    // look identical from the outside and the difference is
                    // the whole question when a line does not appear.
                    // WHICH of the three is answering, and how the vault is
                    // filling up while ChatGPT talks.
                    match tongue.speaking_with() {
                        crate::sermo::Voice::Authored => {
                            format!("the corpus - {} lines", tongue.lines())
                        }
                        crate::sermo::Voice::Generated => {
                            let (held, kept) = tongue.vault_standing().unwrap_or((0, 0));
                            format!("ChatGPT - {kept} written this run, {held} in the vault")
                        }
                        crate::sermo::Voice::Vault => {
                            let (held, _) = tongue.vault_standing().unwrap_or((0, 0));
                            format!("the vault - {held} lines")
                        }
                    }
                },
            ),
            HudValue::Aperture => format!("f/{:.0}", look.aperture),
            HudValue::Focus => format!("{:.2}x", look.focus_bias),
            HudValue::Exposure => format!("{:+.1}", look.exposure),
            HudValue::Saturation => format!("{:.2}", look.saturation),
            HudValue::Frost => format!("{:.0}px", look.frost),
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

/// Keeps the overlay's readout live even while the F1 panel sleeps.
pub(crate) fn update_dev_overlay(
    time: Res<Time<Real>>,
    mut since_last: Local<f32>,
    // The worst frame of the last window — the number optimization actually
    // answers to. Averages hide hitches; this one cannot.
    mut worst: Local<f32>,
    diagnostics: Res<DiagnosticsStore>,
    mut overlay: Query<(&mut Text, &Visibility), With<DevOverlay>>,
) {
    // Tracked every frame, whatever the readout cadence: a one-frame spike
    // between refreshes is exactly the thing being hunted.
    *worst = worst.max(time.delta_secs() * 1000.0);
    *since_last += time.delta_secs();
    if *since_last < 0.5 {
        return;
    }
    *since_last = 0.0;
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let frame = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    for (mut text, visibility) in &mut overlay {
        if *visibility == Visibility::Hidden {
            continue;
        }
        // What the village is thinking, under the frame times. Brett, after
        // three rounds of a town hall that would not rise: "Can you have
        // some kind of log output what the priorities are, maybe to the dev
        // panel?" Nobody could see the queue, so nobody could see that the
        // queue was the problem.
        let wants = crate::villager::work::buildings::CIVIC_THINKING
            .read()
            .map(|said| said.clone())
            .unwrap_or_default();
        let chose = crate::villager::work::buildings::CIVIC_CHOICE
            .read()
            .map(|said| said.clone())
            .unwrap_or_default();
        let mut fresh = format!("fps {fps:.0} / {frame:.1}ms / worst {:.0}ms", *worst);
        if !wants.is_empty() {
            fresh.push_str(&format!("\nwants: {wants}"));
        }
        if !chose.is_empty() {
            fresh.push_str(&format!("\nbroke ground: {chose}"));
        }
        // And whether the village is growing, for the same reason. Brett,
        // stalled at seventeen souls through eight seasons: "I wonder what
        // is going on with the needs and wants for courting, marriage,
        // family home and children?" Five gates decide a birth and none of
        // them said a word.
        let growth = crate::villager::GROWTH_THINKING
            .read()
            .map(|said| said.clone())
            .unwrap_or_default();
        if !growth.is_empty() {
            fresh.push_str(&format!("\ngrowth: {growth}"));
        }
        let courting = crate::villager::gossip::COURTING_THINKING
            .read()
            .map(|said| said.clone())
            .unwrap_or_default();
        if !courting.is_empty() {
            fresh.push_str(&format!("\ncourting: {courting}"));
        }
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
    *worst = 0.0;
}

/// H lifts the roofs off the world — the cutaway view into every interior —
/// and puts them back. New roofs built while lifted come up lifted too.
/// (R already belongs to the survey.)
/// How much of a building is standing, for the watching god: all of
/// it, the roof lifted off, or the walls down as well - a dollhouse.
#[derive(Default, Clone, Copy, PartialEq)]
pub(crate) enum Cutaway {
    #[default]
    Whole,
    RoofOff,
    WallsDown,
}

pub(crate) fn toggle_roofs(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    mut cut: Local<Option<Cutaway>>,
    mut roofs: Query<
        &mut Visibility,
        (
            With<crate::villager::work::RoofPart>,
            Without<crate::villager::work::WallPart>,
        ),
    >,
    mut walls: Query<
        &mut Visibility,
        (
            With<crate::villager::work::WallPart>,
            Without<crate::villager::work::RoofPart>,
        ),
    >,
) {
    // Capture tooling: DIVUS_FACTUS_ROOFLESS starts the world cut away.
    let cut = cut.get_or_insert_with(|| {
        if std::env::var("DIVUS_FACTUS_ROOFLESS").is_ok() {
            Cutaway::RoofOff
        } else {
            Cutaway::Whole
        }
    });
    if keymap.just_pressed(&keys, crate::keymap::Deed::Roofs) {
        *cut = match *cut {
            Cutaway::Whole => Cutaway::RoofOff,
            Cutaway::RoofOff => Cutaway::WallsDown,
            Cutaway::WallsDown => Cutaway::Whole,
        };
    }
    let dress = |showing: bool, visibility: &mut Visibility| {
        let wanted = if showing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    };
    for mut visibility in &mut roofs {
        dress(*cut == Cutaway::Whole, &mut visibility);
    }
    for mut visibility in &mut walls {
        dress(*cut != Cutaway::WallsDown, &mut visibility);
    }
}

/// The backquote shows and hides the dev overlay.
pub(crate) fn toggle_dev_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: Query<&mut Visibility, With<DevOverlay>>,
) {
    if !keys.just_pressed(KeyCode::Backquote) {
        return;
    }
    for mut visibility in &mut overlay {
        *visibility = if *visibility == Visibility::Hidden {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Shift-click the ground while the F1 panel is open to mark that chunk
/// walked.
///
/// A dev tool for the veil, which is the one part of the game whose whole
/// judgement is visual: to see what an edge looks like you have to have an
/// edge, and getting one honestly means sending explorers out and waiting.
/// Brett: "Can I have a way to click the ground while in dev mode and mark a
/// chunk exsplored? Maybe while F1 is open if I hold shift and left click?"
///
/// Behind the F1 panel deliberately. This writes to the village's real
/// knowledge - the same `learn` an expedition calls when it comes home - so it
/// is a cheat, and a cheat should be somewhere you have to have opened
/// something to reach.
pub(crate) fn walk_the_ground_by_hand(
    panels: Query<&Visibility, With<HudPanel>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    hand: Res<crate::hand::DivineHand>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    known: Option<ResMut<crate::villager::explore::KnownWorld>>,
) {
    if panels.iter().all(|showing| *showing == Visibility::Hidden) {
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight) {
        return;
    }
    let (Some(terrain), Some(mut known), Some(flat)) = (terrain, known, hand.cursor_world) else {
        return;
    };
    // `cursor_world` is ALREADY FLAT. The hand does its own unbending before it
    // publishes this, which is why `founding` can hand the same value straight
    // to `will_take_a_village` as ground coordinates. Unbending it a second
    // time here sent every click somewhere else on the world - Brett: "I
    // clicked where the hand is and the ground cleared way over in the
    // distance."
    let coord = terrain.chunk_of(flat.x, flat.z);
    let center = Vec3::new(
        (coord.x as f32 + 0.5) * crate::terrain::CHUNK_SIZE,
        0.0,
        (coord.y as f32 + 0.5) * crate::terrain::CHUNK_SIZE,
    );
    // The whole chunk, corners and all: half its diagonal. Learning a circle
    // the width of a chunk would leave its four corners dark and the map
    // would freckle.
    known.learn(
        center,
        crate::terrain::CHUNK_SIZE * std::f32::consts::SQRT_2 * 0.5,
    );
    info!(
        "the god walked chunk {},{} by hand - {} pockets known",
        coord.x,
        coord.y,
        known.pockets.len()
    );
}
