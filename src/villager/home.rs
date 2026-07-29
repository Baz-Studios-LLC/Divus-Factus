//! Hearth and home: the village fire, and where people sleep.
//!
//! The fire is the village's heart and its first standing obligation: it burns
//! timber from the stockpile, someone has to carry wood to it, and a village
//! that lets it die spends the night in the dark. That makes darkness a
//! *consequence* rather than a shader setting — and the lit circle around the
//! fire is where the homeless gather, which is how the player sees at a glance
//! who has a roof and who does not.
//!
//! At night, people with homes walk to them and sleep inside; at dawn they
//! step back out. A sleeping village is quiet, dark but for the fire, and
//! plucking a sleeper out of their house is exactly the kind of thing a god
//! can do and neighbours will talk about.

use bevy::prelude::*;

use super::work::{Hut, Stockpile};
use super::{Activity, Chronicle, Needs, Person, SettlementSite, SimRng, Villager};
use crate::creature::anim::CreatureMotion;
use crate::creature::genome::{Age, CreatureGenome};
use crate::creature::{Airborne, Corpse, Held, MoveTarget};
use crate::palette;

/// Seconds of burning one log buys.
const SECONDS_PER_LOG: f32 = 75.0;

/// Below this much burn-time left, someone goes for wood.
const LOW_FUEL: f32 = 45.0;

/// How many people one house sleeps.
pub const HOUSE_CAPACITY: usize = 4;

/// How many can sleep rough in the fire's circle before the village is
/// genuinely overfull. The founders' allowance, roughly.
pub const FIRE_CIRCLE_SHELTER: usize = 8;

/// How many people the village can shelter at all.
pub fn shelter_capacity(houses: usize) -> usize {
    FIRE_CIRCLE_SHELTER + houses * HOUSE_CAPACITY
}

/// The village fire: fuel in seconds of burning, and who is off fetching wood
/// for it, so the whole village does not converge on one log.
#[derive(Component, Debug)]
pub struct Bonfire {
    pub fuel: f32,
    pub tender: Option<Entity>,
}

/// The flame meshes, for flickering.
#[derive(Component)]
pub struct Flame;

/// The light the fire casts.
#[derive(Component)]
pub struct Firelight;

/// When real rain sets in, the housed step indoors and wait it out, and
/// the roofless crowd the fire. Work goes on - slower, already taxed by
/// the weather - but idle hands do not stand in a downpour for nothing.
#[allow(clippy::type_complexity)]
pub(super) fn take_shelter(
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    homes: Query<&Transform, (With<Hut>, Without<Villager>)>,
    fires: Query<&GlobalTransform, (With<Bonfire>, Without<Villager>)>,
    mut villagers: Query<
        (
            &Transform,
            Option<&Home>,
            &Needs,
            &mut Activity,
            &mut MoveTarget,
            &mut Visibility,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<crate::creature::Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let Some(weather) = weather else {
        return;
    };
    // Night hands everyone to the sleep routine instead.
    if clock.is_night() {
        return;
    }
    let pouring = weather.intensity > 0.6;
    let fire_pos = fires.iter().next().map(|f| f.translation());

    for (transform, home, needs, mut activity, mut target, mut visibility) in &mut villagers {
        // The starving do not wait out the rain: wet and fed beats dry
        // and dead, and the food systems own them until they have eaten.
        if needs.hunger > 0.7 {
            if *activity == Activity::Sheltering {
                *visibility = Visibility::Inherited;
                *activity = Activity::Idle;
                target.0 = None;
            }
            continue;
        }
        if !pouring {
            if *activity == Activity::Sheltering {
                *visibility = Visibility::Inherited;
                *activity = Activity::Idle;
                target.0 = None;
            }
            continue;
        }
        match *activity {
            Activity::Idle | Activity::Wandering | Activity::Sheltering => {}
            _ => continue,
        }
        match home.and_then(|h| homes.get(h.0).ok()) {
            Some(hut) => {
                let door = hut.translation;
                if transform.translation.distance(door) > 2.2 {
                    *activity = Activity::Sheltering;
                    target.0 = Some(door);
                } else {
                    *activity = Activity::Sheltering;
                    target.0 = None;
                    *visibility = Visibility::Hidden;
                }
            }
            None => {
                // No roof: the fire circle is the next best thing.
                if let Some(fire) = fire_pos
                    && transform.translation.distance(fire) > 5.0
                    && matches!(*activity, Activity::Idle | Activity::Wandering)
                {
                    target.0 = Some(fire);
                }
            }
        }
    }
}

/// Midday at the well: the idle drift over for water and talk. The well is
/// the daylight tavern - the place gossip crosses the village.
#[allow(clippy::type_complexity)]
pub(super) fn well_gatherings(
    clock: Res<crate::calendar::WorldClock>,
    mut rng: ResMut<super::SimRng>,
    wells: Query<(&GlobalTransform, &super::work::Building)>,
    mut idlers: Query<
        (&Transform, &Activity, &mut crate::creature::MoveTarget),
        (
            With<super::Villager>,
            Without<crate::creature::Corpse>,
            Without<crate::creature::Held>,
        ),
    >,
) {
    let hour = clock.time_of_day();
    if !(0.42..0.55).contains(&hour) {
        return;
    }
    let Some(well) = wells
        .iter()
        .find(|(_, b)| b.kind == super::work::BuildingKind::Well)
        .map(|(t, _)| t.translation())
    else {
        return;
    };
    for (at, activity, mut target) in &mut idlers {
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        let distance = at.translation.distance(well);
        if distance > 8.0 && distance < 80.0 && rng.0.chance(0.004) {
            target.0 = Some(well + Vec3::new(rng.0.range(-2.5, 2.5), 0.0, rng.0.range(-2.5, 2.5)));
        }
    }
}

/// Which house this person sleeps in.
#[derive(Component, Debug, Clone, Copy)]
pub struct Home(pub Entity);

/// Builds the village fire near the banner: a stone ring, crossed logs, and a
/// flame that is lit as long as there is fuel.
pub(super) fn spawn_bonfire(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
) -> Entity {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let stone = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::STONE, 0.5),
        perceptual_roughness: 0.95,
        ..default()
    });
    let log = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::WOOD, 0.35),
        perceptual_roughness: 0.95,
        ..default()
    });
    let flame_bright = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::CLOTH_GOLD, 0.95),
        emissive: LinearRgba::from(palette::shade(&palette::CLOTH_GOLD, 0.95)) * 14.0,
        ..default()
    });
    let flame_deep = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::CLOTH_RED, 0.8),
        emissive: LinearRgba::from(palette::shade(&palette::CLOTH_RED, 0.8)) * 9.0,
        ..default()
    });

    let fire = commands
        .spawn((
            Name::new("The village fire"),
            // Cold at the founding: a stone ring and stacked logs are a
            // promise, not a fire. Someone has to carry wood and light it —
            // the village's first chore is making its own hearth.
            Bonfire {
                fuel: 0.0,
                tender: None,
            },
            Transform::from_translation(at),
            Visibility::default(),
            crate::hand::PickRadius(2.0),
            crate::hand::Rooted,
        ))
        .id();

    // The stone ring.
    for step in 0..7 {
        let angle = step as f32 / 7.0 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(cos * 1.15, 0.14, sin * 1.15)
                .with_rotation(Quat::from_rotation_y(-angle))
                .with_scale(Vec3::new(0.5, 0.3, 0.32)),
            ChildOf(fire),
        ));
    }
    // Crossed logs.
    for (yaw, pitch) in [(0.4, 0.28), (1.7, 0.24), (2.9, 0.3)] {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(log.clone()),
            Transform::from_xyz(0.0, 0.28, 0.0)
                .with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_z(pitch))
                .with_scale(Vec3::new(1.5, 0.2, 0.2)),
            ChildOf(fire),
        ));
    }
    // The flame: two boxes, bright inside deep, scaled by the flicker system.
    commands.spawn((
        Flame,
        Mesh3d(cube.clone()),
        MeshMaterial3d(flame_deep),
        Transform::from_xyz(0.0, 0.75, 0.0).with_scale(Vec3::new(0.7, 1.0, 0.7)),
        bevy::light::NotShadowCaster,
        ChildOf(fire),
    ));
    commands.spawn((
        Flame,
        Mesh3d(cube),
        MeshMaterial3d(flame_bright),
        Transform::from_xyz(0.0, 0.9, 0.0).with_scale(Vec3::new(0.4, 0.8, 0.4)),
        bevy::light::NotShadowCaster,
        ChildOf(fire),
    ));
    // Its light.
    commands.spawn((
        Firelight,
        PointLight {
            color: palette::shade(&palette::CLOTH_GOLD, 0.9),
            intensity: 0.0,
            range: 42.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 1.6, 0.0),
        ChildOf(fire),
    ));

    fire
}

/// The fire burns down, and its flame and light follow the fuel.
pub(super) fn burn_weathered(
    weather: Option<Res<crate::weather::Weather>>,
    mut fires: Query<&mut Bonfire>,
    time: Res<Time>,
) {
    // The base burn happens in `burn`; weather adds its tax here, so the
    // two systems stay simple.
    let Some(weather) = weather else {
        return;
    };
    let extra = weather.intensity * 0.5;
    if extra <= 0.0 {
        return;
    }
    for mut fire in &mut fires {
        fire.fuel = (fire.fuel - time.delta_secs() * extra).max(0.0);
    }
}

pub(super) fn burn(
    time: Res<Time>,
    mut fires: Query<&mut Bonfire>,
    mut flames: Query<&mut Transform, (With<Flame>, Without<Bonfire>)>,
    mut lights: Query<&mut PointLight, With<Firelight>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    for mut fire in &mut fires {
        fire.fuel = (fire.fuel - dt).max(0.0);
        let lit = fire.fuel > 0.0;

        // Flicker: two incommensurate sines, so it never loops visibly.
        let flicker = 0.8 + 0.2 * ((t * 9.0).sin() * 0.6 + (t * 23.0).sin() * 0.4);
        let size = if lit { flicker } else { 0.001 };
        for mut flame in &mut flames {
            flame.scale.y = flame.scale.y.signum() * size.abs();
            let base = if flame.translation.y > 0.8 { 0.4 } else { 0.7 };
            flame.scale.x = base * size;
            flame.scale.z = base * size;
            flame.scale.y = (base + 0.35) * size;
        }
        for mut light in &mut lights {
            light.intensity = if lit { 1_600_000.0 * flicker } else { 0.0 };
        }
    }
}

/// When the fire runs low, someone fetches wood for it.
pub(super) fn tend_fire(
    clock: Res<crate::calendar::WorldClock>,
    mut notices: MessageWriter<crate::ui::Notice>,
    site: Option<Res<SettlementSite>>,
    mut stores: Query<&mut Stockpile>,
    mut fires: Query<(Entity, &GlobalTransform, &mut Bonfire)>,
    mut villagers: Query<
        (
            Entity,
            &Transform,
            &CreatureGenome,
            &mut Activity,
            &mut MoveTarget,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let Some(site) = site else {
        return;
    };
    let Ok(mut store) = stores.get_mut(site.settlement) else {
        return;
    };

    for (fire_entity, fire_at, mut fire) in &mut fires {
        let fire_pos = fire_at.translation();

        // Current tender walks the wood over and feeds the flame.
        if let Some(tender) = fire.tender {
            match villagers.get_mut(tender) {
                Ok((_, transform, _, mut activity, mut target))
                    if *activity == Activity::TendingFire =>
                {
                    if transform.translation.distance(fire_pos) > 2.4 {
                        target.0 = Some(fire_pos);
                        continue;
                    }
                    if store.timber >= 1.0 {
                        store.timber -= 1.0;
                        if fire.fuel <= 0.0 {
                            info!("the village fire was lit");
                            notices.write(crate::ui::Notice::new(
                                "The village fire is lit against the night",
                            ));
                        }
                        fire.fuel += SECONDS_PER_LOG;
                    }
                    *activity = Activity::Idle;
                    target.0 = None;
                    fire.tender = None;
                }
                // Died, got grabbed, or wandered off the task some other way.
                _ => fire.tender = None,
            }
            continue;
        }

        // Send for wood while there is wood to send for — but a fire is a
        // night thing: nobody stokes it under the noon sun, so it is built
        // toward dusk and burns down through the morning.
        let evening_on = clock.time_of_day() > 0.58 || clock.is_night();
        if evening_on && fire.fuel < LOW_FUEL && store.timber >= 1.0 {
            let volunteer = villagers
                .iter_mut()
                .filter(|(_, _, genome, activity, _)| {
                    genome.age == Age::Adult
                        && matches!(**activity, Activity::Idle | Activity::Wandering)
                })
                .map(|(entity, transform, _, activity, target)| {
                    (
                        entity,
                        transform.translation.distance(fire_pos),
                        activity,
                        target,
                    )
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((entity, _, mut activity, mut target)) = volunteer {
                *activity = Activity::TendingFire;
                target.0 = Some(fire_pos);
                fire.tender = Some(entity);
                let _ = fire_entity;
            }
        }
    }
}

/// People claim beds in finished houses, nearest first, four to a roof.
pub(super) fn assign_homes(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    huts: Query<(Entity, &Transform), With<Hut>>,
    tenants: Query<&Home>,
    mut homeless: Query<
        (Entity, &Transform, &Person, Option<&mut Chronicle>),
        (With<Villager>, Without<Home>, Without<Corpse>),
    >,
) {
    if huts.is_empty() {
        return;
    }

    let mut occupancy: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for home in &tenants {
        *occupancy.entry(home.0).or_default() += 1;
    }

    for (entity, transform, person, chronicle) in &mut homeless {
        let Some((hut, _)) = huts
            .iter()
            .filter(|(hut, _)| occupancy.get(hut).copied().unwrap_or(0) < HOUSE_CAPACITY)
            .map(|(hut, hut_transform)| {
                (
                    hut,
                    hut_transform.translation.distance(transform.translation),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            return;
        };

        commands.entity(entity).insert(Home(hut));
        *occupancy.entry(hut).or_default() += 1;
        info!("{} moved into a house", person.name);
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), "moved under a roof of their own");
        }
    }
}

/// Night sends people home; dawn brings them out again.
///
/// The housed sleep indoors and vanish from the night streets. The homeless
/// drift to the fire instead — the lit circle is a census of who has no roof.
pub(super) fn night_routine(
    clock: Res<crate::calendar::WorldClock>,
    homes: Query<&Transform, (With<Hut>, Without<Villager>)>,
    fires: Query<&GlobalTransform, (With<Bonfire>, Without<Villager>)>,
    mut rng: ResMut<SimRng>,
    mut villagers: Query<
        (
            &Transform,
            Option<&Home>,
            &Needs,
            &mut Activity,
            &mut MoveTarget,
            &mut Visibility,
            &mut CreatureMotion,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let night = clock.is_night();
    let fire_pos = fires.iter().next().map(|f| f.translation());

    for (transform, home, needs, mut activity, mut target, mut visibility, mut motion) in
        &mut villagers
    {
        if !night {
            if *activity == Activity::Sleeping {
                *visibility = Visibility::Inherited;
                *activity = Activity::Idle;
                target.0 = None;
            }
            continue;
        }

        // The fire-tender finishes the errand first; sleep can wait a minute.
        if *activity == Activity::TendingFire {
            continue;
        }

        // Nobody sleeps through starving. A night is longer than an empty
        // stomach can bear, so the truly hungry are left to the food systems
        // — roused from bed if need be — until they have eaten enough to
        // survive until dawn. Without this, a roof was a death sentence the
        // firelit homeless escaped: sleep held the housed while hunger ran.
        if needs.hunger > 0.7 {
            if *activity == Activity::Sleeping {
                *visibility = Visibility::Inherited;
                *activity = Activity::Idle;
                target.0 = None;
            }
            continue;
        }

        match home.and_then(|h| homes.get(h.0).ok()) {
            Some(hut) => {
                let door = hut.translation;
                if transform.translation.distance(door) > 2.2 {
                    if *activity != Activity::Sleeping {
                        *activity = Activity::Sleeping;
                    }
                    target.0 = Some(door);
                } else if *visibility != Visibility::Hidden {
                    // Indoors. The house stands for them until morning.
                    *activity = Activity::Sleeping;
                    *visibility = Visibility::Hidden;
                    target.0 = None;
                    motion.speed = 0.0;
                }
            }
            None => {
                // No roof: keep to the firelight.
                let Some(fire) = fire_pos else { continue };
                if !matches!(*activity, Activity::Idle | Activity::Wandering) {
                    continue;
                }
                if transform.translation.distance(fire) > 7.0 {
                    *activity = Activity::Wandering;
                    target.0 =
                        Some(fire + Vec3::new(rng.0.range(-4.0, 4.0), 0.0, rng.0.range(-4.0, 4.0)));
                } else {
                    *activity = Activity::Idle;
                    target.0 = None;
                }
            }
        }
    }
}

/// Evenings belong to the tavern, once the village has one.
///
/// Between the workday's end and sleep, anyone whose spirits could use it
/// drifts to the tavern and lingers. The crowd this makes is not decoration:
/// gossip runs on proximity, so the tavern becomes the place stories change
/// hands — the village's rumour engine, built out of nothing but a building
/// and a schedule.
pub(super) fn tavern_evenings(
    clock: Res<crate::calendar::WorldClock>,
    taverns: Query<(&GlobalTransform, &super::work::Building)>,
    mut rng: ResMut<SimRng>,
    mut villagers: Query<
        (&Transform, &super::Morale, &mut Activity, &mut MoveTarget),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let t = clock.time_of_day();
    let evening = (0.62..0.74).contains(&t);
    if !evening {
        return;
    }
    let Some(tavern) = taverns
        .iter()
        .find(|(_, b)| b.kind == super::work::BuildingKind::Tavern)
        .map(|(at, _)| at.translation())
    else {
        return;
    };

    for (transform, morale, mut activity, mut target) in &mut villagers {
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        if morale.spirits > 0.85 {
            continue;
        }
        if transform.translation.distance(tavern) > 6.0 {
            *activity = Activity::Wandering;
            target.0 =
                Some(tavern + Vec3::new(rng.0.range(-3.5, 3.5), 0.0, rng.0.range(-3.5, 3.5)));
        } else {
            *activity = Activity::Idle;
            target.0 = None;
        }
    }
}

/// Waking wears people down; sleep knits them back up — and spirits follow.
///
/// A bed is best; the ground by the fire is half a bed; a night spent awake is
/// none. Exhaustion drains the spirits, and a hollowed-out villager stops
/// showing up for work. This is the first thread of psychological health: a
/// god who keeps snatching sleepers from their beds is *doing something* to
/// this number, and the village will show it.
pub(super) fn weariness(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    sky: Option<Res<crate::calendar::Sky>>,
    buildings: Query<&super::work::Building>,
    mut villagers: Query<
        (
            &mut super::Needs,
            &mut super::Morale,
            &Activity,
            &Visibility,
            Option<&Home>,
            Option<&super::traits::Traits>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
) {
    let dt = time.delta_secs();
    let night = clock.is_night();
    // Warm cloth on every back: the weaver's work is felt hardest by those
    // sleeping out - a blanket is half a roof.
    let woven = buildings
        .iter()
        .any(|b| b.kind == super::work::BuildingKind::Weaver);
    let roofless_ceiling = if woven { 0.68 } else { 0.55 };

    for (mut needs, mut morale, activity, visibility, home, manner) in &mut villagers {
        let brightness = manner.map_or(1.0, |m| m.brightness());
        let endurance = manner.map_or(1.0, |m| m.endurance());
        let asleep_indoors = *activity == Activity::Sleeping && *visibility == Visibility::Hidden;
        let dozing_by_fire = night && matches!(activity, Activity::Idle | Activity::Wandering);

        if asleep_indoors {
            needs.rest = (needs.rest - dt / 55.0).max(0.0);
        } else if dozing_by_fire {
            needs.rest = (needs.rest - dt / 130.0).max(0.0);
        } else {
            // A full waking day builds most of the way toward needing sleep.
            needs.rest = (needs.rest + dt * endurance / 700.0).min(1.0);
        }

        if needs.rest > 0.85 {
            morale.spirits = (morale.spirits - dt / 240.0).max(0.0);
        } else if needs.rest < 0.4 {
            morale.spirits = (morale.spirits + dt * brightness / 320.0).min(1.0);
        }

        // Homelessness is a weight that never quite lifts: nights on the
        // ground grind the spirits down, and no amount of firelight raises a
        // roofless life above resignation.
        if home.is_none() {
            if night {
                // A cold, wet night grinds harder than a mild one.
                let chill = weather.as_ref().map_or(0.0, |w| {
                    1.0 - w.temperature(sky.as_ref().map_or(0.0, |s| s.daylight))
                });
                morale.spirits = (morale.spirits - dt * (1.0 + chill) / 200.0).max(0.0);
            }
            morale.spirits = morale.spirits.min(roofless_ceiling);
        }
    }
}

/// Company by the tavern door mends spirits faster than solitude does.
pub(super) fn tavern_cheer(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    taverns: Query<(&GlobalTransform, &super::work::Building)>,
    mut villagers: Query<(&Transform, &mut super::Morale), (With<Villager>, Without<Corpse>)>,
) {
    let t = clock.time_of_day();
    if !(0.62..0.76).contains(&t) {
        return;
    }
    let Some(tavern) = taverns
        .iter()
        .find(|(_, b)| b.kind == super::work::BuildingKind::Tavern)
        .map(|(at, _)| at.translation())
    else {
        return;
    };
    let dt = time.delta_secs();
    for (transform, mut morale) in &mut villagers {
        if transform.translation.distance(tavern) < 7.0 {
            morale.spirits = (morale.spirits + dt / 70.0).min(1.0);
        }
    }
}

/// A hand that reaches into a house comes out with someone visible. Being
/// plucked from bed wakes you.
pub(super) fn rouse_the_taken(
    mut taken: Query<(&mut Visibility, &mut Activity), (With<Villager>, With<Held>)>,
) {
    for (mut visibility, mut activity) in &mut taken {
        if *visibility == Visibility::Hidden {
            *visibility = Visibility::Inherited;
        }
        if *activity == Activity::Sleeping {
            *activity = Activity::Idle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_log_buys_more_burning_than_the_low_mark() {
        // If a log burned out before the next one could plausibly be fetched,
        // the fire would flap between lit and dead all night.
        assert!(SECONDS_PER_LOG > LOW_FUEL * 1.5);
    }

    #[test]
    fn houses_sleep_families_not_crowds() {
        assert!((2..=6).contains(&HOUSE_CAPACITY));
    }
}
