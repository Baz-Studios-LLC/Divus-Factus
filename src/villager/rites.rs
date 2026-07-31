//! Death rites: mourning, bearing, and burial.
//!
//! A death used to leave a body in the grass and nothing else. Now it leaves
//! a hole in the village: family and neighbours gather and weep over the
//! dead, spirits fall, chronicles record who was wept for — and then someone
//! (the priest, when the village has one) shoulders the body and carries it
//! to a resting ground on the outskirts, where a mound and a headstone stand
//! for good. The grave keeps the dead person's name and chronicle: hover it
//! and read the life that ended there.
//!
//! None of this mentions the god. But when the god caused the death, every
//! mourner is a witness standing still for a long moment — and the funeral
//! procession is the consequence made visible, walking slowly across town.

use bevy::prelude::*;

use crate::creature::anim::CreatureMotion;
use crate::creature::{Airborne, Childhood, Corpse, Held, MoveTarget};
use crate::terrain::{Terrain, WATER_LEVEL};

use super::{Activity, Chronicle, Morale, Parentage, Person, Spouse, Villager, work};

/// How long the village stands with its dead before anyone thinks of spades.
const MOURNING_SECONDS: f64 = 35.0;

/// How long a body lies in state before a bearer comes for it.
const BURIAL_DELAY: f64 = 45.0;

/// A villager's corpse, waiting on its rites. The clock starts at death.
#[derive(Component)]
pub struct Passing {
    pub since: f64,
}

/// Standing with the dead.
#[derive(Component)]
pub struct Grieving {
    pub at: Vec3,
    pub until: f64,
}

/// Carrying a body to the resting ground.
#[derive(Component)]
pub struct Bearing(pub Entity);

/// A body on a bearer's shoulders.
#[derive(Component)]
pub struct Borne;

/// Where each town buries its dead. A town's ground is chosen at its first
/// burial and kept — keyed by settlement, because a second town does not walk
/// its dead across the map to lie in the first one's graveyard.
#[derive(Resource, Default)]
pub struct RestingGround(pub std::collections::HashMap<Entity, Vec3>);

/// A grave. The dead person's [`Person`] and [`Chronicle`] live on the same
/// entity, so hovering a headstone reads the life that ended under it.
#[derive(Component)]
pub struct Grave {
    pub day: u32,
}

/// Death is noticed: the family and the nearest neighbours put down what
/// they are doing and gather over the body.
#[allow(clippy::type_complexity)]
pub(super) fn mark_the_dead(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut say: MessageWriter<crate::ui::Say>,
    mut telling: (
        Option<ResMut<crate::telling::Tongue>>,
        Option<Res<crate::attention::Attention>>,
    ),
    fallen: Query<
        (Entity, &Transform, &Person, Option<&Parentage>),
        (Added<Corpse>, With<Villager>),
    >,
    mut mourners: Query<
        (
            Entity,
            &Transform,
            Option<&Spouse>,
            Option<&Parentage>,
            &mut Activity,
            &mut Morale,
            Option<&mut Chronicle>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
) {
    let day = clock.day();
    for (dead, at, person, dead_parentage) in &fallen {
        commands.entity(dead).insert(Passing {
            since: clock.elapsed,
        });

        let mut gathered = 0;
        for (mourner, mourner_at, spouse, parentage, mut activity, mut morale, chronicle) in
            &mut mourners
        {
            let widowed = spouse.is_some_and(|s| s.0 == dead);
            let orphaned = parentage.is_some_and(|p| p.mother == dead || p.father == dead);
            let bereaved_parent =
                dead_parentage.is_some_and(|p| p.mother == mourner || p.father == mourner);
            let family = widowed || orphaned || bereaved_parent;
            let near = mourner_at.translation.distance(at.translation) < 20.0;
            if !family && (!near || gathered >= 5) {
                continue;
            }
            gathered += 1;

            morale.spirits = (morale.spirits - if family { 0.2 } else { 0.06 }).max(0.0);
            *activity = Activity::Mourning;
            commands.entity(mourner).insert(Grieving {
                at: at.translation,
                until: clock.elapsed + MOURNING_SECONDS,
            });
            if let Some(mut chronicle) = chronicle {
                chronicle.record(day, format!("wept for {}", person.name));
            }
            if family {
                // Watched grief is composed: this mourner, over this body,
                // today. The dead one's name is the only name grief may say.
                let composed = telling
                    .0
                    .as_mut()
                    .filter(|_| {
                        crate::attention::regard(telling.1.as_deref(), at.translation)
                            .worth_composing()
                    })
                    .map(|tongue| {
                        tongue.muse(crate::telling::Musing {
                            who: mourner,
                            voice: None,
                            bearing: crate::villager::traits::Bearing::Plain,
                            faith: crate::telling::FaithBand::Wavering,
                            body: Vec::new(),
                            place: Vec::new(),
                            mind: format!(
                                "you stand over the body of {}, dead this day",
                                person.name
                            ),
                            heard: None,
                            known: vec![person.name.clone()],
                        })
                    })
                    .is_some();
                if !composed {
                    say.write(crate::ui::Say {
                        speaker: mourner,
                        text: format!("{}...", person.name),
                        thought: true,
                        own_words: false,
                    });
                }
            }
        }
    }
}

/// Mourners walk to the body and stand with it until the grief lets go.
#[allow(clippy::type_complexity)]
pub(super) fn mourn(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut mourners: Query<
        (
            Entity,
            &Transform,
            &Grieving,
            &mut Activity,
            &mut MoveTarget,
            &mut CreatureMotion,
        ),
        (With<Villager>, Without<Held>, Without<Airborne>),
    >,
) {
    for (mourner, at, grieving, mut activity, mut target, mut motion) in &mut mourners {
        if *activity != Activity::Mourning {
            commands.entity(mourner).remove::<Grieving>();
            continue;
        }
        if clock.elapsed > grieving.until {
            commands.entity(mourner).remove::<Grieving>();
            *activity = Activity::Idle;
            target.0 = None;
            continue;
        }
        if at.translation.distance(grieving.at) > 2.6 {
            target.0 = Some(grieving.at);
        } else {
            // Head bowed, still. Stillness over a body is the animation.
            target.0 = None;
            motion.speed = 0.0;
        }
    }
}

/// The dead are carried to rest: a bearer is called, the body is shouldered,
/// and a grave is raised at the resting ground — mound, headstone, and name.
#[allow(clippy::type_complexity)]
pub(super) fn burials(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Option<Res<Terrain>>,
    towns: Query<(Entity, &super::SettlementGround)>,
    mut ground: ResMut<RestingGround>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    graves: Query<(), With<Grave>>,
    mut bodies: Query<
        (
            Entity,
            &mut Transform,
            &Person,
            Option<&Chronicle>,
            &Passing,
            Has<Borne>,
        ),
        (
            With<Corpse>,
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
    mut bearers: Query<
        (
            Entity,
            &Transform,
            Option<&work::Vocation>,
            Has<Childhood>,
            &mut Activity,
            &mut MoveTarget,
            Option<&Bearing>,
            &Person,
            Option<&mut Chronicle>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
) {
    let Some(terrain) = terrain else {
        return;
    };

    // A body waits long enough for the weeping; then the village needs a
    // resting ground, if it has never needed one before.
    let due: Vec<Entity> = bodies
        .iter()
        .filter(|(_, _, _, _, passing, borne)| {
            !borne && clock.elapsed - passing.since > BURIAL_DELAY
        })
        .map(|(body, ..)| body)
        .collect();

    // Each town's resting ground, chosen the first time that town needs one:
    // past its own fields, on dry ground, in the far ring of its own pattern.
    if !due.is_empty() {
        for (town, home_ground) in &towns {
            if ground.0.contains_key(&town) {
                continue;
            }
            let spot = work::village_slots(home_ground.centre, 7..10)
                .into_iter()
                .map(|(x, z, _)| Vec3::new(x, terrain.height_at(x, z), z))
                .find(|at| terrain.is_walkable(at.x, at.z) && at.y > WATER_LEVEL + 2.0);
            if let Some(spot) = spot {
                ground.0.insert(town, spot);
            }
        }
    }
    // The nearest town's graveyard: a body is carried to the ground of
    // whoever will bury it, which in practice is the town it died beside.
    let resting_for = |at: Vec3| {
        ground
            .0
            .iter()
            .map(|(town, spot)| (*town, *spot))
            .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)))
            .map(|(_, spot)| spot)
    };

    // Call a bearer for each unclaimed body: the priest if the village has
    // one, otherwise whoever stands idle.
    let claimed: Vec<Entity> = bearers
        .iter()
        .filter_map(|(_, _, _, _, _, _, bearing, _, _)| bearing.map(|b| b.0))
        .collect();
    for body in due {
        if claimed.contains(&body) {
            continue;
        }
        let volunteer = bearers
            .iter_mut()
            .filter(|(_, _, _, child, activity, _, bearing, _, _)| {
                !child
                    && bearing.is_none()
                    && matches!(
                        **activity,
                        Activity::Idle | Activity::Wandering | Activity::Working
                    )
            })
            .min_by_key(|(_, _, vocation, ..)| match vocation {
                Some(work::Vocation::Priest) => 0,
                _ => 1,
            });
        let Some((bearer, _, _, _, mut activity, _, _, _, _)) = volunteer else {
            continue;
        };
        *activity = Activity::Bearing;
        commands
            .entity(bearer)
            .insert(Bearing(body))
            .remove::<work::Job>();
    }

    // Bearers walk, shoulder the body, and walk again; the grave is raised
    // where they stop.
    for (bearer, at, _, _, mut activity, mut target, bearing, person, chronicle) in &mut bearers {
        let Some(Bearing(body)) = bearing else {
            continue;
        };
        let body = *body;
        if *activity != Activity::Bearing {
            // Night or hunger broke the procession; set the body down.
            commands.entity(bearer).remove::<Bearing>();
            if let Ok((_, mut body_transform, ..)) = bodies.get_mut(body) {
                let x = body_transform.translation.x;
                let z = body_transform.translation.z;
                body_transform.translation.y = terrain.height_at(x, z) + 0.2;
                body_transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            }
            commands.entity(body).remove::<Borne>();
            continue;
        }
        let Ok((_, mut body_transform, dead_person, dead_chronicle, _, borne)) =
            bodies.get_mut(body)
        else {
            commands.entity(bearer).remove::<Bearing>();
            *activity = Activity::Idle;
            continue;
        };

        if !borne {
            if at.translation.distance(body_transform.translation) > 1.9 {
                target.0 = Some(body_transform.translation);
            } else {
                commands.entity(body).insert(Borne);
            }
            continue;
        }

        // On the shoulder: the body rides flat, above the bearer's own head.
        body_transform.translation = at.translation + Vec3::Y * 1.5;
        body_transform.rotation = at.rotation * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);

        // The graveyard of whichever town lies nearest the bearer.
        let Some(resting) = resting_for(at.translation) else {
            continue;
        };
        // Graves stand in rows, filled in the order the village filled them.
        let index = graves.iter().count() as f32;
        let spot = resting
            + Vec3::new(
                (index % 4.0) * 2.0 - 3.0,
                0.0,
                (index / 4.0).floor() * 2.6 - 1.3,
            );
        let spot = Vec3::new(spot.x, terrain.height_at(spot.x, spot.z), spot.z);

        if at.translation.distance(spot) > 1.8 {
            target.0 = Some(spot);
            continue;
        }

        // Lay them down. The grave takes their name and their story.
        let day = clock.day();
        let mut story = dead_chronicle.cloned().unwrap_or_default();
        story.record(day, "was laid to rest");
        let nearest_square = towns
            .iter()
            .map(|(_, home_ground)| home_ground.centre)
            .min_by(|a, b| a.distance(spot).total_cmp(&b.distance(spot)))
            .unwrap_or(spot + Vec3::X);
        let toward = (nearest_square - spot).with_y(0.0).normalize_or_zero();
        let yaw = (-toward.z).atan2(toward.x);
        let grave = commands
            .spawn((
                Grave { day },
                dead_person.clone(),
                story,
                Name::new(format!("The grave of {}", dead_person.name)),
                Transform::from_translation(spot).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
                crate::hand::PickRadius(1.6),
                crate::hand::Rooted,
            ))
            .id();
        let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let earth = materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::EARTH, 0.35),
            perceptual_roughness: 1.0,
            ..default()
        });
        let stone = materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::STONE, 0.55),
            perceptual_roughness: 0.95,
            ..default()
        });
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(earth),
            Transform::from_xyz(0.0, 0.16, 0.0).with_scale(Vec3::new(1.6, 0.32, 0.9)),
            ChildOf(grave),
        ));
        commands.spawn((
            Mesh3d(cube),
            MeshMaterial3d(stone),
            Transform::from_xyz(0.75, 0.42, 0.0).with_scale(Vec3::new(0.14, 0.84, 0.5)),
            ChildOf(grave),
        ));

        commands.entity(body).despawn();
        commands.entity(bearer).remove::<Bearing>();
        *activity = Activity::Idle;
        target.0 = None;
        info!("{} was laid to rest", dead_person.name);
        notices.write(crate::ui::Notice::new(format!(
            "{} was laid to rest",
            dead_person.name
        )));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(
                day,
                format!("carried {} to the resting ground", dead_person.name),
            );
        }
        let _ = person;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_death_gathers_mourners() {
        let mut app = App::new();
        app.init_resource::<crate::calendar::WorldClock>();
        app.add_message::<crate::ui::Say>();

        app.world_mut().spawn((
            Villager,
            Corpse,
            Person::born("Odo".into(), "Gravely".into()),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
        let neighbour = app
            .world_mut()
            .spawn((
                Villager,
                Person::born("Ma".into(), "Gravely".into()),
                Transform::from_xyz(4.0, 0.0, 0.0),
                Activity::Idle,
                Morale::default(),
                Chronicle::default(),
            ))
            .id();

        app.world_mut().run_system_once(mark_the_dead).unwrap();

        let world = app.world();
        assert_eq!(
            *world.get::<Activity>(neighbour).unwrap(),
            Activity::Mourning,
            "a neighbour drops everything for the dead",
        );
        assert!(world.get::<Grieving>(neighbour).is_some());
        let story = world.get::<Chronicle>(neighbour).unwrap();
        assert!(story.events.iter().any(|e| e.text.contains("wept for Odo")));
        assert!(
            world.get::<Morale>(neighbour).unwrap().spirits < 0.8,
            "grief costs something",
        );
    }
}
