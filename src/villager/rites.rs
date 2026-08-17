//! Death rites: mourning, bearing, and burial.
//!
//! A death used to leave a body in the grass and nothing else. Now it leaves
//! a hole in the village: family and neighbors gather and weep over the
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

/// Where a body rides on the bearer, above their feet.
const SHOULDER_HEIGHT: f32 = 1.28;

/// Half the length of a laid-out body, for centering it on the shoulders.
const HALF_A_BODY: f32 = 0.85;

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
    /// What ended them.
    ///
    /// COPIED OFF THE BODY, because the body does not survive the burial: the
    /// corpse carries `Vitality` and is despawned the moment the grave goes
    /// up, so a stone that did not take this down could never say what
    /// happened. The card had who and when and no answer at all to how.
    /// Brett: "Hovering headstone should show who died and how at a minimum."
    pub undoing: crate::creature::Undoing,
    /// Whether it was violence rather than want, which is what decides
    /// whether `undoing` is worth reading at all - everything that is not
    /// violent starved, and `Undoing::Hunger` is its own answer.
    pub violent: bool,
}

impl Grave {
    /// What the stone says: how they went, then when they were laid down.
    ///
    /// Guarded the same way the death notice in `creature` is - `undoing`
    /// only means anything when the death was violent, because a quiet death
    /// leaves whatever the last harm was sitting in the field.
    pub fn epitaph(&self) -> String {
        let how = if self.violent {
            self.undoing.how()
        } else {
            "starved"
        };
        format!("{how}, and was laid to rest on day {}", self.day)
    }
}

/// Death is noticed: the family and the nearest neighbors put down what
/// they are doing and gather over the body.
#[allow(clippy::type_complexity)]
pub(super) fn mark_the_dead(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut telling: (
        Option<ResMut<crate::sermo::Tongue>>,
        Option<Res<crate::attention::Attention>>,
    ),
    fallen: Query<
        (Entity, &Transform, &Person, Option<&Parentage>),
        // `Person`, NOT `Villager` - see the note in `burials`. This whole
        // module has never run a single funeral because of it.
        (Added<Corpse>, With<Person>),
    >,
    // Death closes the asking: a corpse that kept its prayer held a
    // place in the town's chorus and a card's worth of count on the
    // board - the ledger read seven asking over six living souls.
    children: Query<&Children>,
    motes: Query<Entity, With<crate::villager::belief::PrayerMote>>,
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
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: mark_the_dead");
    let day = clock.day();
    for (dead, at, person, dead_parentage) in &fallen {
        commands.entity(dead).insert(Passing {
            since: clock.elapsed,
        });
        super::belief::end_prayer(&mut commands, dead, &children, &motes);

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
                // Grief thinks in grief's words - tagged `grieving`, so the
                // pick can never hand a mourner a cheerful idle thought, and
                // the want-list demands mourning lines until they exist.
                if let Some(tongue) = telling.0.as_mut() {
                    tongue.muse(crate::sermo::Musing {
                        who: mourner,
                        voice: None,
                        faith: crate::sermo::FaithBand::Wavering,
                        body: vec!["grieving"],
                        heard: None,
                        aloud: false,
                        about: None,
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
        (
            With<Villager>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
        ),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: mourn");
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
            Option<&crate::creature::Vitality>,
        ),
        (
            With<Corpse>,
            // A DEAD VILLAGER IS NOT A `Villager`.
            //
            // `succumb` strips `Villager`, `Needs` and `Activity` off a body on
            // its way to being a corpse - so `With<Corpse> + With<Villager>` is
            // a pair that can never both be true, and every query in this file
            // asked for exactly that. The mourning, the bearers, the
            // procession, the graves: written, tested by eye once presumably,
            // and silently never run since. The dead simply rotted where they
            // fell like deer, which is the one thing the file above swears they
            // do not do.
            //
            // What marks the village's own dead is that they still have a NAME.
            // Brett found it from the other end: "when someone dies their
            // corpse should remain. The town should have to deal with that."
            With<Person>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
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
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
        ),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: burials");
    let Some(terrain) = terrain else {
        return;
    };

    // A body waits long enough for the weeping; then the village needs a
    // resting ground, if it has never needed one before.
    let due: Vec<Entity> = bodies
        .iter()
        .filter(|(_, _, _, _, passing, borne, _)| {
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
            let spot = work::village_slots(home_ground.center, 7..10, 12.0)
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
        let Ok((_, mut body_transform, dead_person, dead_chronicle, _, borne, end)) =
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

        // ACROSS THE SHOULDERS, and the offset is the whole of it.
        //
        // A corpse's own origin is at its FEET, and laying it flat by rotating
        // ninety degrees about Z swings it out along the bearer's -X. Pinned at
        // the bearer's own position it therefore hung entirely off one side and
        // through the bearer's face - which is exactly what it did, for as long
        // as this code has existed, unseen because no funeral had ever run.
        // Brett, on the first one he watched: "the corpse carrying position
        // needs to be fixed."
        //
        // Shifted back along that same axis by half a body, the load sits
        // centered on the shoulders. Lowered too: 1.5 above the feet is over the
        // head of a villager who is 1.75 tall.
        let laid_flat = at.rotation * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        body_transform.translation =
            at.translation + Vec3::Y * SHOULDER_HEIGHT + at.rotation * Vec3::X * HALF_A_BODY;
        body_transform.rotation = laid_flat;

        // The graveyard of whichever town lies nearest the bearer.
        //
        // AND IF THERE IS NO GRAVEYARD, ONE IS MADE HERE. This used to
        // `continue`, which means a bearer who could not be told where to go
        // stood holding the body for ever - Brett watched exactly that: "I
        // killed someone and another psron walked up and picked him up and just
        // stood there for days on end holding him." A resting ground is only
        // chosen if the ring search finds walkable dry land in a town's outer
        // rings, and when it does not, nothing ever chose one and nothing ever
        // would.
        //
        // Better a grave on the spot than a man carrying his neighbor until
        // one of them starves.
        let resting = match resting_for(at.translation) {
            Some(spot) => spot,
            None => {
                let here = at.translation;
                Vec3::new(here.x, terrain.height_at(here.x, here.z), here.z)
            }
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
            .map(|(_, home_ground)| home_ground.center)
            .min_by(|a, b| a.distance(spot).total_cmp(&b.distance(spot)))
            .unwrap_or(spot + Vec3::X);
        let toward = (nearest_square - spot).with_y(0.0).normalize_or_zero();
        let yaw = (-toward.z).atan2(toward.x);
        let grave = commands
            .spawn((
                Grave {
                    day,
                    undoing: end.map_or(crate::creature::Undoing::Hunger, |v| v.undoing),
                    violent: end.is_some_and(|v| v.violent),
                },
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

    /// The stone names the cause. Both branches, because `undoing` is only
    /// trustworthy on a violent death and a stone that read it anyway would
    /// tell the player a starved villager was killed by a wolf.
    #[test]
    fn a_headstone_says_how_they_went() {
        let killed = Grave {
            day: 12,
            undoing: crate::creature::Undoing::Teeth,
            violent: true,
        };
        assert_eq!(
            killed.epitaph(),
            "was killed by a wolf, and was laid to rest on day 12"
        );

        // A quiet death carries whatever harm happened to be last on the
        // body - here a wolf that bit them years ago and did not finish it.
        let starved = Grave {
            day: 40,
            undoing: crate::creature::Undoing::Teeth,
            violent: false,
        };
        assert_eq!(starved.epitaph(), "starved, and was laid to rest on day 40");
    }

    #[test]
    fn a_death_gathers_mourners() {
        let mut app = App::new();
        app.init_resource::<crate::debug::timings::Timings>();
        app.init_resource::<crate::calendar::WorldClock>();
        app.add_message::<crate::ui::Say>();

        // THE DEAD ARE SPAWNED THE WAY `succumb` LEAVES THEM: a corpse with a
        // name and NO `Villager`. This test used to hand itself a body that was
        // both a `Corpse` and a `Villager` at once - a pair the running game
        // cannot produce, because `succumb` strips the second when it adds the
        // first. So it passed for as long as it has existed while not one
        // funeral ever happened in a real village, and it would have gone on
        // passing after the bug was fixed OR unfixed. A fixture that cannot
        // fail is worse than no fixture.
        app.world_mut().spawn((
            Corpse,
            Person::born("Odo".into(), "Gravely".into()),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
        let neighbor = app
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
            *world.get::<Activity>(neighbor).unwrap(),
            Activity::Mourning,
            "a neighbor drops everything for the dead",
        );
        assert!(world.get::<Grieving>(neighbor).is_some());
        let story = world.get::<Chronicle>(neighbor).unwrap();
        assert!(story.events.iter().any(|e| e.text.contains("wept for Odo")));
        assert!(
            world.get::<Morale>(neighbor).unwrap().spirits < 0.8,
            "grief costs something",
        );
    }

    /// The guard on the fixture above: if anybody ever writes a corpse query
    /// against `Villager` again, this says why it will never match.
    #[test]
    fn the_dead_keep_their_name_and_nothing_else() {
        let mut app = App::new();
        app.add_message::<crate::creature::CreatureDied>();
        let dead = app
            .world_mut()
            .spawn((
                Villager,
                crate::creature::Creature,
                Person::born("Odo".into(), "Gravely".into()),
                crate::creature::Vitality {
                    harm: 1.0,
                    ..Default::default()
                },
                Transform::default(),
                crate::creature::anim::CreatureMotion::new(0.0),
                MoveTarget::default(),
            ))
            .id();
        app.world_mut()
            .run_system_once(crate::creature::succumb_for_tests)
            .unwrap();
        let world = app.world();
        assert!(
            world.get::<Corpse>(dead).is_some(),
            "the dead are a corpse",
        );
        assert!(
            world.get::<Person>(dead).is_some(),
            "and they keep their name, which is what the rites find them by",
        );
        assert!(
            world.get::<Villager>(dead).is_none(),
            "but they are no longer a Villager - every corpse query in this \
             file was written against that and matched nothing for months",
        );
    }
}
