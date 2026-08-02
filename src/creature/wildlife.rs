//! Wildlife with lives of its own.
//!
//! The wilderness stops being set dressing here. Deer graze and flee; wolves
//! hunger, hunt, kill and eat; herds thin and recover toward what the land
//! can carry. None of it references the god — this is the world's own drama,
//! running whether or not anyone divine is watching. It matters to the game
//! precisely *because* it ignores the game: a wolf pulling down a deer on the
//! ridge line at dusk is the world insisting it is real.
//!
//! And the food web crosses into the village without any new rules: wolves
//! and village hunters compete for the same deer; a wolf bold enough to
//! hunt near the settlement becomes the hunters' quarry in turn; a famine
//! year for deer is a famine year for everyone.

use bevy::prelude::*;

use super::anim::CreatureMotion;
use super::genome::{Age, CreatureGenome, Species};
use super::{
    Airborne, Corpse, Creature, Held, MoveTarget, Vitality, build_body, random_walkable_point,
    spawn_creature,
};
use crate::creature::body::CreatureAssets;
use crate::rng::Rng;
use crate::terrain::Terrain;
use crate::villager::Villager;

/// Seconds for a wild belly to go from full to hunting.
const WILD_SECONDS_TO_HUNGER: f32 = 380.0;

/// How far a wolf can smell prey.
const HUNT_RANGE: f32 = 110.0;

/// How near a wolf gets before prey bolts.
const FLIGHT_RANGE: f32 = 26.0;

/// A wild animal's own needs and rhythms.
#[derive(Component, Debug, Default)]
pub struct Wild {
    /// 0 fed, 1 ravenous.
    pub hunger: f32,
    /// Seconds left in whatever it is doing (grazing, feeding).
    pub busy: f32,
    /// The centre of this animal's territory. Herds keep to a range instead
    /// of diffusing across the map, so the deer meadow stays a place.
    pub home: Vec3,
}

/// How far an animal wanders from its home range before turning back.
const HOME_RANGE: f32 = 55.0;

/// A wild juvenile, growing toward a rebuilt adult body.
#[derive(Component, Debug)]
pub struct WildYouth {
    pub remaining: f32,
}

/// Wild bellies empty slowly.
pub(super) fn wild_hunger(time: Res<Time>, mut wild: Query<&mut Wild, Without<Corpse>>) {
    let dt = time.delta_secs();
    for mut animal in &mut wild {
        animal.hunger = (animal.hunger + dt / WILD_SECONDS_TO_HUNGER).min(1.0);
        animal.busy = (animal.busy - dt).max(0.0);
    }
}

/// Prey flees wolves; otherwise it grazes when hungry and drifts when not.
#[allow(clippy::type_complexity)]
pub(super) fn graze_and_flee(
    terrain: Option<Res<Terrain>>,
    mut rng: Local<Option<Rng>>,
    wolves: Query<(&Transform, &CreatureGenome), (With<Creature>, Without<Corpse>, Without<Held>)>,
    mut prey: Query<
        (
            &Transform,
            &CreatureGenome,
            &mut Wild,
            &mut MoveTarget,
            &mut CreatureMotion,
        ),
        (
            With<Creature>,
            Without<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let rng = rng.get_or_insert_with(|| Rng::new(0x57A6));

    let wolf_positions: Vec<Vec3> = wolves
        .iter()
        .filter(|(_, genome)| genome.species == Species::Wolf)
        .map(|(transform, _)| transform.translation)
        .collect();

    for (transform, genome, mut wild, mut target, mut motion) in &mut prey {
        if !matches!(genome.species, Species::Deer | Species::Boar) {
            continue;
        }
        let at = transform.translation;

        // A wolf too near overrides everything: run directly away, and a
        // little past, so the chase has room to be a chase.
        if let Some(threat) = wolf_positions
            .iter()
            .find(|wolf| wolf.distance(at) < FLIGHT_RANGE)
        {
            let away = (at - *threat).normalize_or_zero();
            let goal = at + away * 34.0;
            if terrain.is_walkable(goal.x, goal.z) {
                target.0 = Some(goal);
            } else {
                target.0 = random_walkable_point(&terrain, rng, at, 30.0);
            }
            wild.busy = 0.0;
            continue;
        }

        if wild.busy > 0.0 {
            // Head down, grazing: standing still is the animation.
            target.0 = None;
            motion.speed = 0.0;
            if wild.busy < 0.1 {
                wild.hunger = 0.0;
            }
            continue;
        }

        if wild.hunger > 0.6 {
            wild.busy = 5.0;
        } else if target.0.is_none() && rng.chance(0.01) {
            // Idle drift, a few steps at a time — turning home when the
            // wandering has carried them out of their range.
            if at.distance(wild.home) > HOME_RANGE {
                target.0 = random_walkable_point(&terrain, rng, wild.home, 25.0);
            } else {
                target.0 = random_walkable_point(&terrain, rng, at, 22.0);
            }
        }
    }
}

/// Wolves hunt: pick the nearest prey, run it down, strike, and eat the kill.
#[allow(clippy::type_complexity)]
pub(super) fn wolves_hunt(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    towers: Query<&GlobalTransform, With<crate::villager::work::Building>>,
    tower_kinds: Query<&crate::villager::work::Building>,
    mut rng: Local<Option<Rng>>,
    mut hunters: Query<
        (
            &Transform,
            &CreatureGenome,
            &mut Wild,
            &mut MoveTarget,
            &mut CreatureMotion,
        ),
        (
            With<Creature>,
            Without<Corpse>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
    mut quarry: Query<
        (
            Entity,
            &Transform,
            &CreatureGenome,
            &mut Vitality,
            Has<Corpse>,
        ),
        (With<Creature>, Without<Villager>, Without<Held>),
    >,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let rng = rng.get_or_insert_with(|| Rng::new(0xF01F));
    let _ = (&terrain, &rng);

    // Collect prey snapshot first; wolves are also in `quarry`'s archetype, so
    // mutation happens through targeted lookups afterwards.
    struct Mark {
        entity: Entity,
        at: Vec3,
        dead: bool,
    }
    // A manned horn on a high post: wolves will not hunt in its shadow.
    let watch_posts: Vec<Vec3> = towers
        .iter()
        .zip(tower_kinds.iter())
        .filter(|(_, b)| b.kind == crate::villager::work::BuildingKind::Watchtower)
        .map(|(t, _)| t.translation())
        .collect();

    let marks: Vec<Mark> = quarry
        .iter()
        .filter(|(_, _, genome, _, _)| matches!(genome.species, Species::Deer | Species::Boar))
        .map(|(entity, transform, _, _, dead)| Mark {
            entity,
            at: transform.translation,
            dead,
        })
        .filter(|mark| watch_posts.iter().all(|post| post.distance(mark.at) > 55.0))
        .collect();

    let mut strikes: Vec<Entity> = Vec::new();
    let mut eaten: Vec<Entity> = Vec::new();

    for (transform, genome, mut wild, mut target, mut motion) in &mut hunters {
        if genome.species != Species::Wolf {
            continue;
        }
        let at = transform.translation;

        if wild.busy > 0.0 {
            // Feeding at a kill.
            target.0 = None;
            motion.speed = 0.0;
            continue;
        }
        if wild.hunger < 0.55 {
            continue;
        }

        // A fresh carcass beats a chase.
        if let Some(meal) = marks
            .iter()
            .filter(|m| m.dead)
            .min_by(|a, b| a.at.distance(at).total_cmp(&b.at.distance(at)))
            .filter(|m| m.at.distance(at) < HUNT_RANGE)
        {
            if meal.at.distance(at) > 2.2 {
                target.0 = Some(meal.at);
            } else {
                wild.busy = 6.0;
                wild.hunger = 0.0;
                eaten.push(meal.entity);
                target.0 = None;
            }
            continue;
        }

        // Else: the nearest living quarry.
        let Some(mark) = marks
            .iter()
            .filter(|m| !m.dead)
            .min_by(|a, b| a.at.distance(at).total_cmp(&b.at.distance(at)))
            .filter(|m| m.at.distance(at) < HUNT_RANGE)
        else {
            continue;
        };

        if mark.at.distance(at) > 2.0 {
            target.0 = Some(mark.at);
        } else {
            // One lunge, then a beat of recovery — which is the beat the prey
            // uses to break away. Hunts are chases now, and chases can fail.
            strikes.push(mark.entity);
            wild.busy = 1.1;
            target.0 = None;
        }
    }

    for victim in strikes {
        if let Ok((_, _, _, mut vitality, dead)) = quarry.get_mut(victim)
            && !dead
        {
            vitality.harm += 0.45;
            vitality.violent = true;
        }
    }
    for meal in eaten {
        // The kill is consumed; the bones go back to the world.
        commands.entity(meal).despawn();
    }
}

/// Wild things multiply, up to what the land nearby will carry.
#[allow(clippy::type_complexity)]
pub(super) fn wild_breeding(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut rng: Local<Option<Rng>>,
    terrain: Option<Res<Terrain>>,
    assets: Option<Res<CreatureAssets>>,
    animals: Query<
        (&Transform, &CreatureGenome, &Wild),
        (
            With<Creature>,
            Without<Villager>,
            Without<Corpse>,
            Without<Held>,
        ),
    >,
) {
    *since_last += time.delta_secs();
    if *since_last < 16.0 {
        return;
    }
    *since_last = 0.0;
    let (Some(terrain), Some(assets)) = (terrain, assets) else {
        return;
    };
    let rng = rng.get_or_insert_with(|| Rng::new(0xB1B7));

    let all: Vec<(Vec3, &CreatureGenome, f32)> = animals
        .iter()
        .map(|(transform, genome, wild)| (transform.translation, genome, wild.hunger))
        .collect();

    for (i, (at, genome, hunger)) in all.iter().enumerate() {
        if genome.age != Age::Adult || *hunger > 0.5 {
            continue;
        }
        // A fed pair of the same kind, close together...
        let Some((_, mate, _)) = all
            .iter()
            .skip(i + 1)
            .find(|(other_at, other, other_hunger)| {
                other.species == genome.species
                    && other.age == Age::Adult
                    && *other_hunger < 0.5
                    && other_at.distance(*at) < 16.0
            })
        else {
            continue;
        };
        // ...on land that is not already full of their kind.
        let herd = all
            .iter()
            .filter(|(other_at, other, _)| {
                other.species == genome.species && other_at.distance(*at) < 90.0
            })
            .count();
        if herd >= 9 || !rng.chance(0.5) {
            continue;
        }

        let mut young = CreatureGenome::child_of(genome, mate, rng);
        young.age = Age::Child;
        let grow_time = rng.range(150.0, 260.0);
        let Some(spot) = random_walkable_point(&terrain, rng, *at, 6.0) else {
            continue;
        };
        let entity = spawn_creature(
            &mut commands,
            &assets,
            young,
            spot,
            rng.range(0.0, std::f32::consts::TAU),
            rng.f32() * 6.0,
        );
        commands.entity(entity).insert((
            Wild {
                home: *at,
                ..default()
            },
            WildYouth {
                remaining: grow_time,
            },
        ));
    }
}

/// Wild young grow into rebuilt adult bodies, the same way village children do.
pub(super) fn wild_growth(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<CreatureAssets>>,
    mut rng: Local<Option<Rng>>,
    mut young: Query<
        (Entity, &mut CreatureGenome, &mut WildYouth),
        (Without<Corpse>, Without<Held>),
    >,
) {
    let Some(assets) = assets else {
        return;
    };
    let rng = rng.get_or_insert_with(|| Rng::new(0x9007));
    let _ = &rng;

    for (entity, mut genome, mut youth) in &mut young {
        youth.remaining -= time.delta_secs();
        if youth.remaining > 0.0 {
            continue;
        }
        genome.age = Age::Adult;
        commands.entity(entity).despawn_related::<Children>();
        let rig = build_body(&mut commands, &assets, entity, &genome);
        commands
            .entity(entity)
            .remove::<WildYouth>()
            .insert((rig, crate::hand::PickRadius(genome.height() * 0.45)));
    }
}

/// When a mauling was last cried out about, so that one long attack is
/// one memory rather than one a frame. Not saved: a mauling that ends
/// with the load is over, and the memory it laid down is what persists.
#[derive(Component)]
pub struct Torn(pub f64);

/// Seconds before the same person being savaged counts as a fresh
/// alarm. Long enough that a single wolf worrying at somebody is one
/// story; short enough that being caught again the next morning is a
/// second one.
const TEETH_REMEMBERED: f64 = 90.0;

/// Wolves test the roads. A villager alone and far from the square —
/// a miner on the ore road, an explorer past the cairns — is prey the
/// pack understands. Company is armour: anyone within a dozen strides
/// makes a walker no longer alone, which is the whole argument for
/// guards walking with expeditions. Towers hold their old dread.
#[allow(clippy::type_complexity)]
pub(super) fn wolves_stalk(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut commands: Commands,
    mut alarms: MessageWriter<crate::witness::DivineEvent>,
    site: Option<Res<crate::villager::SettlementSite>>,
    towers: Query<(&GlobalTransform, &crate::villager::work::Building)>,
    mut telling: (
        Option<ResMut<crate::telling::Tongue>>,
        Option<Res<crate::attention::Attention>>,
    ),
    mut rng: ResMut<crate::villager::SimRng>,
    mut wolves: Query<
        (&Transform, &CreatureGenome, &mut Wild, &mut MoveTarget),
        (Without<Corpse>, Without<Held>, Without<Villager>),
    >,
    mut walkers: Query<
        (
            Entity,
            &Transform,
            Option<&crate::villager::work::Vocation>,
            &mut crate::creature::Vitality,
            &mut CreatureMotion,
            Option<&Torn>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<Wild>,
        ),
    >,
) {
    let Some(site) = site else {
        return;
    };
    let dt = time.delta_secs();
    let posts: Vec<Vec3> = towers
        .iter()
        .filter(|(_, b)| b.kind == crate::villager::work::BuildingKind::Watchtower)
        .map(|(t, _)| t.translation())
        .collect();
    // Where everyone stands, for the loneliness test.
    let folk: Vec<(Entity, Vec3, bool)> = walkers
        .iter()
        .map(|(entity, at, vocation, ..)| {
            (
                entity,
                at.translation,
                matches!(vocation, Some(crate::villager::work::Vocation::Guard)),
            )
        })
        .collect();

    for (wolf_at, genome, mut wild, mut target) in &mut wolves {
        if genome.species != Species::Wolf || wild.hunger < 0.35 {
            continue;
        }
        let at = wolf_at.translation;
        // The mark: nearest villager who is far from the square, past the
        // towers' dread, not a guard, and truly alone.
        let mark = folk
            .iter()
            .filter(|(who, spot, is_guard)| {
                !is_guard
                    && spot.distance(at) < 42.0
                    && spot.distance(site.centre) > 60.0
                    && posts.iter().all(|post| post.distance(*spot) > 55.0)
                    && !folk
                        .iter()
                        .any(|(other, other_at, _)| other != who && other_at.distance(*spot) < 12.0)
            })
            .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)));
        let Some((quarry, quarry_at, _)) = mark else {
            continue;
        };
        if quarry_at.distance(at) > 1.7 {
            target.0 = Some(*quarry_at);
            continue;
        }
        // Teeth. A mauling is slow enough to run from, fast enough to
        // kill the one who cannot.
        target.0 = None;
        if let Ok((_, quarry_here, _, mut vitality, mut motion, torn)) = walkers.get_mut(*quarry) {
            vitality.harm += dt * 0.3;
            vitality.violent = true;
            motion.flail = 1.0;
            // The village learns of this. Once per attack, not once per
            // frame: teeth stay in for seconds, and a memory laid down
            // sixty times would push everything else this person has ever
            // seen out of their head inside a breath.
            let fresh = torn.is_none_or(|torn| clock.elapsed - torn.0 > TEETH_REMEMBERED);
            if fresh {
                let where_at = quarry_here.translation;
                commands.entity(*quarry).insert(Torn(clock.elapsed));
                info!(
                    "a wolf set upon one of the village, {:.0} strides out from the square",
                    where_at.distance(site.centre)
                );
                alarms.write(crate::witness::DivineEvent {
                    kind: crate::witness::DivineEventKind::Mauled,
                    position: where_at,
                    subject: Some(*quarry),
                    intensity: 0.8,
                });
            }
            if rng.0.chance(dt * 0.4)
                && let Some(tongue) = telling.0.as_mut()
            {
                // The scream is composed for the one being torn at; the
                // attention gate keeps off-screen drama from spending the
                // teller's slots.
                let seen = walkers
                    .get(*quarry)
                    .map(|(_, at, ..)| {
                        crate::attention::regard(telling.1.as_deref(), at.translation)
                            .worth_composing()
                    })
                    .unwrap_or(false);
                if seen {
                    tongue.muse(crate::telling::Musing {
                        who: *quarry,
                        voice: None,
                        bearing: crate::villager::traits::Bearing::Plain,
                        faith: crate::telling::FaithBand::Wavering,
                        body: vec!["hurt"],
                        place: Vec::new(),
                        mind: "a wolf is upon you, teeth in your leg — scream".into(),
                        heard: None,
                        aloud: true,
                        prayer: false,
                        known: Vec::new(),
                    });
                }
            }
        }
        wild.hunger = (wild.hunger - dt * 0.1).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunger_paces_the_wild_day() {
        // A wolf should need to hunt roughly once a world-day, not constantly
        // and not never.
        assert!(WILD_SECONDS_TO_HUNGER > 120.0);
        assert!(WILD_SECONDS_TO_HUNGER < 600.0);
    }

    #[test]
    fn prey_notices_wolves_before_teeth_reach_it() {
        assert!(FLIGHT_RANGE > 10.0, "prey must get a head start");
        assert!(
            HUNT_RANGE > FLIGHT_RANGE * 3.0,
            "wolves must see farther than prey flees, or no chase ever starts",
        );
    }
}
