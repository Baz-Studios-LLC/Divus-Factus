//! The corner HUD and the live tuning keys.

use crate::render::LookSettings;
use crate::witness::Reaction;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use super::*;
use crate::terrain::LoadedChunks;
use crate::ui;

/// The stats-and-tuning panel, top left.
#[derive(Component)]
pub(crate) struct HudPanel;

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

pub(crate) fn spawn_hud(mut commands: Commands) {
    // Both panels come from the interface kit; this module only decides what
    // words go in them.
    let hud = ui::panel(
        &mut commands,
        ui::Anchor::TopLeft,
        Some("DIVUS FACTUS"),
        None,
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
             hold LMB to grab, flick to throw / Tab opens the codex / F1 hides this panel",
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
    playing: Res<State<crate::GameState>>,
    mut look: ResMut<LookSettings>,
    mut state: ResMut<DebugState>,
    mut codex: Query<&mut Visibility, With<super::VillagePanel>>,
) {
    // Tab belongs to the player now: it opens the codex, the game's own
    // book. The developer's instrument panel retreats to F1.
    if keys.just_pressed(KeyCode::Tab) && matches!(playing.get(), crate::GameState::Playing) {
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
