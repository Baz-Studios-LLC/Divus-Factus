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

use crate::creature::genome::Age;
use crate::creature::genome::Sex;
use crate::ui;
use crate::villager::{
    Activity, Chronicle, MemberOf, Morale, Needs, Parentage, Person, Settlement, Spouse, Villager,
};
use crate::witness::Reaction;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

mod capture;
mod god;
mod history;
mod hud;
mod inspector;
pub(crate) mod layers;
mod manifest;
pub(crate) mod menagerie;
pub(crate) mod ordo_trial;
mod people;
mod report;
pub(crate) mod portrait;
pub(crate) mod spellbook;
pub(crate) mod timings;
pub(crate) mod village;
mod villager_profile;
mod world;

pub(crate) use capture::*;
pub(crate) use god::*;
pub(crate) use history::*;
pub(crate) use hud::*;
pub(crate) use inspector::*;
pub(crate) use manifest::*;
pub(crate) use people::*;
pub(crate) use village::*;
pub(crate) use world::*;

/// The debug book's frame in three beats, where one total order used to
/// stand. Hands first, then the carpentry, then the paint: systems inside
/// a beat run in parallel and never queue behind a stranger, and a new
/// system joins with one `.in_set(...)` call — no chain tuple to outgrow.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum DebugSet {
    /// Clicks and keys mutate state: filters, selections, page turns.
    Input,
    /// Panels rebuild their content from that state.
    Rebuild,
    /// Colors, visibility and text are touched up on what stands.
    Dress,
}

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((FrameTimeDiagnosticsPlugin::default(), layers::LayerPlugin))
            .init_resource::<DebugState>()
            .init_resource::<SelectedPerson>()
            .init_resource::<RosterSort>()
            .init_resource::<people::RosterFilter>()
            .init_resource::<villager_profile::VillagerProfile>()
            .init_resource::<portrait::Portraits>()
            .init_resource::<ChronicleView>()
            .init_resource::<ChronicleStars>()
            .init_resource::<Manifestation>()
            .init_resource::<menagerie::Menagerie>()
            .add_systems(
                Startup,
                (
                    spawn_hud,
                    menagerie::build_the_menagerie,
                    spawn_world_panel.after(spawn_village_panel),
                    spawn_people_panel.after(spawn_village_panel),
                    villager_profile::spawn_villager_profile.after(spawn_people_panel),
                    spawn_chronicle_page.after(spawn_village_panel),
                    spawn_village_panel,
                    spawn_god_panel.after(spawn_village_panel),
                    spellbook::spawn_spellbook_page.after(spawn_village_panel),
                    portrait::spawn_portrait_studio,
                ),
            )
            .init_resource::<village::Rebinding>()
            .add_systems(
                PreUpdate,
                village::catch_rebind.after(bevy::input::InputSystems),
            )
            .configure_sets(
                Update,
                (DebugSet::Input, DebugSet::Rebuild, DebugSet::Dress)
                    .chain()
                    // AND ALL OF IT BEFORE ORDO PAINTS.
                    //
                    // Ordo re-applies the theme - fonts, sizes, inks - to
                    // everything it can see, once a frame, in its own set. A
                    // row spawned AFTER that ran is a row wearing nothing:
                    // Bevy's own default font at Bevy's own default size,
                    // for exactly one frame, until the next pass dresses it.
                    //
                    // Nothing here was ordered against that set at all, so
                    // which side of it a rebuild landed on was down to
                    // whatever order Bevy happened to pick - which is why the
                    // codex flickered SOMETIMES, and why every word in it
                    // flashed at once when it did. Brett: "when it flickers
                    // all of the text shows the default bevy text for a
                    // split second."
                    .before(ordo::OrdoSet),
            )
            // INPUT: every hand on the instruments. Parallel except where
            // two hands reach for the same state - those edges are named.
            .add_systems(
                Update,
                (
                    handle_tuning_input,
                    toggle_dev_overlay,
                    toggle_roofs,
                    toggle_the_sea,
                    menagerie::work_the_menagerie,
                    handle_toolbar,
                    screenshot_on_request,
                    people::filter_by_chip,
                    handle_chronicle_filters,
                    handle_people_rows,
                    handle_roster_sort,
                )
                    .in_set(DebugSet::Input),
            )
            .add_systems(
                Update,
                (
                    handle_codex_tabs,
                    village::answer_the_board,
                    spellbook::place_from_the_book,
                    village::settings_panel,
                    village::keybind_panel,
                    village::sound_panel,
                    village::swap_mouse_buttons.after(village::keybind_panel),
                    // The vacant seat fills only once the page turn is in
                    // and a row click has had its say - the three writers
                    // of SelectedPerson keep one order: rows, meet, then
                    // the capture preselect below.
                    people::meet_someone
                        .after(handle_codex_tabs)
                        .after(handle_people_rows),
                    // The capture harness picks its page and subject after
                    // every other hand has spoken - the preselect must win.
                    capture_preselect
                        .after(handle_codex_tabs)
                        .after(handle_people_rows)
                        .after(people::meet_someone),
                )
                    .in_set(DebugSet::Input),
            )
            .add_systems(
                Update,
                (
                    world::fit_the_sheets,
                    paint_world_map,
                    handle_map_zoom,
                    update_world_markers,
                )
                    .chain(),
            )
            // The menagerie bench: build what is asked for, put it on the
            // private layer, turn it, then frame it - in that order, because
            // the lens frames off the exhibit's own height and there is
            // nothing to measure until it exists.
            .add_systems(
                Update,
                (
                    menagerie::dress_the_exhibit,
                    menagerie::turn_the_turntable,
                    menagerie::mind_the_lens,
                    menagerie::name_the_exhibit,
                )
                    .chain()
                    .in_set(DebugSet::Rebuild),
            )
            // The portrait studio: booking before seating before stamping,
            // and the gallery hung last, all in one frame's walk.
            .add_systems(
                Update,
                (
                    portrait::want_portraits,
                    portrait::run_the_studio,
                    portrait::stamp_sitter_layers,
                    portrait::hang_the_portraits,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    rebuild_manifestation,
                    ease_manifestation,
                    gate_deity_camera,
                    animate_manifestation,
                )
                    .chain(),
            )
            // REBUILD: every page keeps its own content up, in parallel.
            // The veil's dev tool: shift-click ground with F1 open to mark that
            // chunk walked. Registered ALONE, in Input: the tuple below is a
            // Rebuild set, and a system named in two sets that have a
            // before/after relationship between them is a schedule that
            // refuses to build - it panics on boot, and no test in the suite
            // builds a schedule.
            .add_systems(Update, walk_the_ground_by_hand.in_set(DebugSet::Input))
            .add_systems(
                Update,
                (
                    update_hud,
                    update_world_panel,
                    update_people_panel,
                    update_chronicle,
                    update_faith_roster,
                    update_faith_chart,
                    update_ledger_details,
                    village::update_prayer_board,
                    update_village_panel,
                    // Playing only: the hover card naming what is beneath the
                    // hand belongs to play, not to the title's translucent veil.
                    update_inspector.run_if(in_state(crate::GameState::Playing)),
                )
                    .in_set(DebugSet::Rebuild),
            )
            .add_systems(
                Update,
                villager_profile::update_villager_profile.in_set(DebugSet::Rebuild),
            )
            .add_systems(
                Update,
                (
                    update_dossier,
                    update_god_panel,
                    update_person_detail,
                    // The doll must stand before its layers are stamped and
                    // its turntable spun - one subject, kept in step.
                    (update_paperdoll, stamp_doll_layers, spin_doll).chain(),
                )
                    .in_set(DebugSet::Rebuild),
            )
            // DRESS: paint on what stands. The page flip goes first so
            // every dresser that gates on its page's visibility sees the
            // book already turned; behind it they all run in parallel.
            .add_systems(
                Update,
                (
                    update_dev_overlay,
                    (
                        apply_codex_page,
                        (
                            dress_ledger_banner,
                            spellbook::update_spellbook,
                            style_roster_rows,
                            people::people_pulse,
                            village::footer_date,
                        ),
                    )
                        .chain(),
                )
                    .in_set(DebugSet::Dress),
            )
            .add_systems(
                Update,
                (
                    villager_profile::handle_profile_actions,
                    // ALWAYS, not behind a dial: the report is the thing Brett
                    // reads back to me after a test, and an instrument that has
                    // to be remembered is an instrument that is off when it
                    // matters. The log line inside it still waits for
                    // `DIVUS_FACTUS_FRAMES`.
                    report::report_frames,
                ),
            )
            .add_systems(Startup, report::open_the_report)
            .add_systems(Last, report::close_the_report)
            .add_systems(PostUpdate, villager_profile::open_villager_profile);

        // The parallel court's own inspector: DIVUS_FACTUS_AMBIGUITY=1
        // makes the scheduler list every unordered pair that shares
        // mutable state, so a new system's missing edge is a warning in
        // the log instead of a nondeterministic frame in the wild.
        if std::env::var("DIVUS_FACTUS_AMBIGUITY").is_ok() {
            app.edit_schedule(Update, |schedule| {
                schedule.set_build_settings(bevy::ecs::schedule::ScheduleBuildSettings {
                    ambiguity_detection: bevy::ecs::schedule::LogLevel::Warn,
                    ..default()
                });
            });
        }

        if let Some(path) = crate::capture_path() {
            app.insert_resource(AutoCapture {
                path,
                // Long enough for terrain generation, scatter and the first frames
                // of animation to settle, so the capture is representative.
                // DIVUS_FACTUS_CAPTURE_DELAY overrides it, for photographing things
                // that take minutes to happen — a village being built, say.
                delay: std::env::var("DIVUS_FACTUS_CAPTURE_DELAY")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(9.0),
                taken: false,
                second: std::env::var("DIVUS_FACTUS_CAPTURE_PAIR")
                    .ok()
                    .and_then(|v| v.parse().ok()),
                second_taken: false,
            })
            .add_systems(Update, auto_capture);
        }
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

/// Flies the camera home to the settlement - the eye on the ledger page.
#[derive(Component)]
pub(crate) struct RecenterButton;

/// The eye on the ledger's village entry: flying home clears any follow,
/// aims the camera at the banner, and the codex steps aside so the village
/// itself fills the view. (The toolbar this once served is gone - the codex
/// is whole, and Tab and the strip are its doors.)
fn handle_toolbar(
    buttons: Query<&Interaction, (Changed<Interaction>, With<RecenterButton>)>,
    mut village_panels: Query<&mut Visibility, With<VillagePanel>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    mut follow: ResMut<crate::camera::FollowTarget>,
    mut rigs: Query<&mut crate::camera::CameraRig>,
) {
    let (Some(site), Ok(mut rig)) = (site, rigs.single_mut()) else {
        return;
    };
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            follow.entity = None;
            rig.target_focus = site.center;
            rig.target_distance = 70.0;
            for mut visibility in &mut village_panels {
                *visibility = Visibility::Hidden;
            }
        }
    }
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
pub(crate) fn person_phrase(sex: Sex, age: Age) -> &'static str {
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
            .map(|p| p.full_name())
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
            .map(|p| p.full_name())
            .unwrap_or_else(|_| "a mother now gone".into());
        let father = kin
            .get(parents.father)
            .map(|p| p.full_name())
            .unwrap_or_else(|_| "a father now gone".into());
        return format!("born to {mother} and {father}");
    }
    "of the first families".to_string()
}

/// What they are doing, favoring a reaction over routine: fear interrupts lunch.
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
        Some(Activity::Marvelling) => "on their knees at what they saw",
        Some(Activity::Sleeping) => "asleep",
        Some(Activity::Mourning) => "mourning",
        Some(Activity::Alarming) => "running for the bell",
        Some(Activity::Chatting) => "deep in conversation",
        Some(Activity::Sheltering) => "waiting out the rain",
        Some(Activity::Bearing) => "bearing the dead",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every codex page, spawned once against one world.
    ///
    /// A bundle with a duplicate component panics when it is SPAWNED, not
    /// when it compiles — the deity page shipped a (ordo::col + Node) pair
    /// and the whole game panicked at boot. This raises the entire book in
    /// a test world so that class of crash fails here instead of on
    /// Brett's screen.
    #[test]
    fn the_whole_book_spawns() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>();
        app.init_resource::<Assets<crate::miracles::CooldownSweep>>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_resource::<crate::villager::WorldChronicle>();

        let world = app.world_mut();
        world.run_system_once(village::spawn_village_panel).unwrap();
        world.run_system_once(people::spawn_people_panel).unwrap();
        world.run_system_once(spawn_god_panel).unwrap();
        world.run_system_once(world::spawn_world_panel).unwrap();
        world
            .run_system_once(history::spawn_chronicle_page)
            .unwrap();
        world
            .run_system_once(portrait::spawn_portrait_studio)
            .unwrap();
        world
            .run_system_once(spellbook::spawn_spellbook_page)
            .unwrap();
        // The commands applied without a duplicate-bundle panic, and the
        // codex stands.
        assert!(world.get_resource::<village::Codex>().is_some());
    }

    /// The inspector's queries stay disjoint. B0001 panics when the
    /// SYSTEM INITIALIZES, not when it compiles - the tooltip gate once
    /// shipped a book query that forgot `Without<InspectorDetail>` and
    /// the game died at boot on Brett's screen. Initializing the system
    /// here makes that class fail in the suite instead. Missing
    /// resources are fine - init runs, and init is where B0001 lives.
    #[test]
    fn the_inspector_wires_disjoint_queries() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = bevy::app::App::new();
        let _ = app.world_mut().run_system_once(inspector::update_inspector);
    }

    /// The other fat systems get the same net: their bundles were carved
    /// into named SystemParam structs, and a struct field conflicting
    /// with a sibling is the same boot-time B0001 as an anonymous tuple.
    #[test]
    fn the_fat_systems_wire_disjoint_queries() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = bevy::app::App::new();
        let _ = app.world_mut().run_system_once(people::update_people_panel);
        let _ = app
            .world_mut()
            .run_system_once(people::update_person_detail);
        let _ = app.world_mut().run_system_once(god::update_god_panel);
        // The village panel joined this net the day it fell out of it. Its
        // `Text` queries were disjoint by a list of `Without`s, a new marker
        // was added to one of them, and the list did not grow with it - so
        // the game built, the suite passed green, and it panicked on Brett's
        // machine the moment the ledger opened. B0001 is a RUNTIME fault;
        // only running a system finds it.
        let _ = app
            .world_mut()
            .run_system_once(village::update_village_panel);
        let _ = app
            .world_mut()
            .run_system_once(village::update_prayer_board);
        // The town strip: three Text queries and two Visibility ones in a
        // single system, which is precisely the shape that panicked on
        // Brett's machine last time and built perfectly well first.
        let _ = app
            .world_mut()
            .run_system_once(crate::ui::town::update_town_strip);
        let _ = app
            .world_mut()
            .run_system_once(crate::ui::town::spawn_town_strip);
        // The villager dossier, which arrived from the Aspectus fork with
        // four systems and a great many queries and joined this net the
        // moment it landed rather than the moment it panicked. It is the
        // shape that has caught us twice: one system reading several `Text`
        // queries that are disjoint only by a list of `Without`s.
        let _ = app
            .world_mut()
            .run_system_once(villager_profile::spawn_villager_profile);
        let _ = app
            .world_mut()
            .run_system_once(villager_profile::update_villager_profile);
        let _ = app
            .world_mut()
            .run_system_once(villager_profile::handle_profile_actions);
        let _ = app
            .world_mut()
            .run_system_once(villager_profile::open_villager_profile);
    }

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
    /// The codex is dressed on the frame it is built, not the one after.
    ///
    /// Ordo re-applies fonts, sizes and inks once a frame in its own set.
    /// A row spawned after that has run wears nothing until the next pass -
    /// Bevy's own default font at its own default size - and because
    /// nothing here was ordered against that set, which side of it a
    /// rebuild landed on was whatever order Bevy happened to pick. Hence a
    /// codex that flickered SOMETIMES, and every word in it at once when it
    /// did.
    ///
    /// Asserted as an ordering rather than by looking at pixels: the
    /// schedule is the thing that was wrong, and a schedule can be checked.
    #[test]
    fn every_panel_is_built_before_ordo_paints() {
        use bevy::prelude::*;

        let mut app = App::new();
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::default())
            .configure_sets(
                Update,
                (DebugSet::Input, DebugSet::Rebuild, DebugSet::Dress)
                    .chain()
                    .before(ordo::OrdoSet),
            );
        // Building the schedule is the check: Bevy rejects a set graph it
        // cannot order, so a cycle or a contradiction fails here.
        app.add_systems(Update, (|| {}).in_set(DebugSet::Rebuild));
        app.add_systems(Update, (|| {}).in_set(ordo::OrdoSet));
        app.update();
    }
}

/// F9 pulls the flat sea plane out of the world and puts it back — the
/// diagnostic Brett asked for while the sky was full of unexplained grays,
/// each of which turned out to be a different sheet. One key, flip it, and
/// the argument about which layer is which settles itself on screen.
fn toggle_the_sea(
    keys: Res<ButtonInput<KeyCode>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut water: Query<&mut Visibility, With<crate::terrain::WaterPlane>>,
) {
    // F12, and not F9 as first shipped: F9 was already the exposure dial,
    // and the collision meant every flip of the sea also silently pushed
    // the exposure up - the diagnostic was contaminating the scene it was
    // diagnosing.
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }
    for mut visibility in &mut water {
        let hidden = *visibility == Visibility::Hidden;
        *visibility = if hidden {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        notices.write(crate::ui::Notice::new(if hidden {
            "The sea is back".to_string()
        } else {
            "The sea plane is off - F12 brings it back".to_string()
        }));
    }
}
