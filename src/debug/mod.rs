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
mod manifest;
mod people;
mod village;
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

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<DebugState>()
            .init_resource::<SelectedPerson>()
            .init_resource::<RosterSort>()
            .init_resource::<ChronicleView>()
            .init_resource::<ChronicleStars>()
            .init_resource::<Manifestation>()
            .add_systems(
                Startup,
                (
                    spawn_hud,
                    spawn_world_panel.after(spawn_village_panel),
                    spawn_people_panel.after(spawn_village_panel),
                    spawn_chronicle_page.after(spawn_village_panel),
                    spawn_village_panel,
                    spawn_god_panel.after(spawn_village_panel),
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
                    handle_chronicle_filters,
                    update_chronicle,
                    handle_people_rows,
                    handle_roster_sort,
                    // Playing only: the hover card naming what is beneath the
                    // hand belongs to play, not to the title's translucent veil.
                    update_inspector.run_if(in_state(crate::GameState::Playing)),
                    screenshot_on_request,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (paint_world_map, handle_map_zoom, update_world_markers).chain(),
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
            .add_systems(
                Update,
                (
                    update_faith_roster,
                    update_ledger_details,
                    handle_codex_tabs,
                    apply_codex_page,
                    dress_ledger_banner,
                    update_dossier,
                    update_god_panel,
                    update_faith_chart,
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
                // DIVUS_FACTUS_CAPTURE_DELAY overrides it, for photographing things
                // that take minutes to happen — a village being built, say.
                delay: std::env::var("DIVUS_FACTUS_CAPTURE_DELAY")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(9.0),
                taken: false,
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
            rig.target_focus = site.centre;
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
