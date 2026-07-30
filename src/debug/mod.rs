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
mod people;
mod village;
mod world;

pub(crate) use capture::*;
pub(crate) use god::*;
pub(crate) use history::*;
pub(crate) use hud::*;
pub(crate) use inspector::*;
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
            .add_systems(
                Startup,
                (
                    spawn_hud,
                    spawn_toolbar,
                    spawn_world_panel,
                    spawn_people_panel.after(spawn_village_panel),
                    spawn_chronicle_page.after(spawn_village_panel),
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
                (
                    update_faith_roster,
                    update_ledger_details,
                    handle_codex_tabs,
                    apply_codex_page,
                    update_dossier,
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

fn spawn_toolbar(mut commands: Commands) {
    let bar = ui::toolbar(&mut commands);

    // The village ledger: three rising bars.
    let village = ui::icon_button(&mut commands, bar);
    commands.entity(village).insert((
        VillageButton,
        ui::HoverHint::new("The Village", "ledger, gauges and the faith roster"),
    ));
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
    commands.entity(god).insert((
        GodButton,
        ui::HoverHint::new(
            "The God",
            "your name, your miracles, how they feel about you",
        ),
    ));
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
    commands.entity(world).insert((
        WorldButton,
        ui::HoverHint::new("The Land", "the world's own numbers"),
    ));
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
    commands.entity(people).insert((
        PeopleButton,
        ui::HoverHint::new("The People", "roster, portraits and every life's story"),
    ));
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
    commands.entity(history).insert((
        HistoryButton,
        ui::HoverHint::new("The Chronicle", "everything that has ever happened here"),
    ));
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
    mut codex: Option<ResMut<Codex>>,
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
            if let Some(codex) = codex.as_mut() {
                for mut visibility in &mut village_panels {
                    if *visibility != Visibility::Hidden && codex.page == CodexPage::People {
                        *visibility = Visibility::Hidden;
                    } else {
                        *visibility = Visibility::Visible;
                        codex.page = CodexPage::People;
                    }
                }
            }
        }
    }
    for interaction in &history_buttons {
        if *interaction == Interaction::Pressed {
            if let Some(codex) = codex.as_mut() {
                for mut visibility in &mut village_panels {
                    if *visibility != Visibility::Hidden && codex.page == CodexPage::Chronicle {
                        *visibility = Visibility::Hidden;
                    } else {
                        *visibility = Visibility::Visible;
                        codex.page = CodexPage::Chronicle;
                    }
                }
            }
        }
    }
    for interaction in &village_buttons {
        if *interaction == Interaction::Pressed {
            if let Some(codex) = codex.as_mut() {
                for mut visibility in &mut village_panels {
                    if *visibility != Visibility::Hidden && codex.page == CodexPage::Ledger {
                        *visibility = Visibility::Hidden;
                    } else {
                        *visibility = Visibility::Visible;
                        codex.page = CodexPage::Ledger;
                    }
                }
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
            // The eye lives on the ledger's village entry now: flying home
            // closes the codex so the village itself fills the view.
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
