//! Colonies: how a crowded town becomes two towns.
//!
//! A village that outgrows its ground does not simply keep growing. Once it is
//! pressed for room, for fields and for food, and once its explorers have
//! brought back word of somewhere better, a party walks out of it and founds a
//! settlement of their own — with its own banner, its own name, its own stores,
//! and from that day its own history.
//!
//! Three things must all be true before anyone leaves, and each is a lever the
//! player can pull:
//!
//! 1. **Pressure.** The town is short of roofs, or short of food for the mouths
//!    it has. A god who feeds a village keeps it whole; one who lets it strain
//!    pushes it apart.
//! 2. **Somewhere to go.** The ground must be *known* — an explorer walked it
//!    and came home. Until then there is nowhere to leave for, however crowded
//!    the square gets. This is what makes exploration matter.
//! 3. **Someone to lead.** A bold adult willing to go, and enough others to
//!    make a village rather than a death.
//!
//! The party walks. They are not teleported: the road is visible, it takes
//! days, and a god who objects has that long to intervene.

use bevy::prelude::*;

use super::work::{Stockpile, Vocation};
use super::{
    Activity, Chronicle, MemberOf, Person, Settlement, SettlementCulture, SettlementGround, SimRng,
    Villager, found_settlement, score_town_ground,
};
use crate::creature::genome::Age;
use crate::creature::{Airborne, Childhood, Corpse, Held, MoveTarget};
use crate::palette;
use crate::rng::Rng;
use crate::terrain::Terrain;

/// How often the question is asked. A departure is a once-in-a-generation
/// thing; there is no need to check it every frame.
const MUSTER_INTERVAL: f32 = 30.0;

/// The fewest people a town keeps for itself. A settlement does not hollow
/// itself out to seed another — the parent must remain a going concern.
const PARENT_FLOOR: usize = 14;

/// The fewest who will set out together. Below this it is not a colony, it is
/// a family alone in the woods, and the woods win.
const PARTY_FLOOR: usize = 5;

/// The most who will leave in one party.
const PARTY_CEILING: usize = 8;

/// How far a colony must stand from every existing town. Close enough to walk
/// between in a day or two; far enough to be its own place with its own land.
const COLONY_SPACING: f32 = 260.0;

/// The furthest afield a party will settle.
const COLONY_REACH: f32 = 620.0;

/// Walking pace, in world units per second, for estimating the road.
const TRAVEL_PACE: f32 = 2.4;

/// A party on the road to found a town, and where they are going.
///
/// Held by every member, so the group survives one of them being picked up,
/// scattered, or killed — the rest walk on.
#[derive(Component, Debug, Clone, Copy)]
pub struct Colonist {
    /// Where the banner will go up.
    pub destination: Vec3,
    /// Who is carrying the decision. When the leader dies the party turns back.
    pub leader: Entity,
}

/// The leader of a party, and what they set out with.
#[derive(Component, Debug)]
pub struct ColonyCharter {
    pub destination: Vec3,
    /// The town they left, for the chronicle.
    pub mother_town: Entity,
    /// Provisions carried out of the parent's stores.
    pub food: f32,
    pub timber: f32,
    /// Name and arms, rolled when they set out rather than on arrival, so the
    /// place they are walking to already has a name to be spoken of.
    pub name: String,
    pub banner_ramp: usize,
    pub sigil: usize,
}

/// How hard a town is pressed for room and food, from 0 (comfortable) upward.
///
/// Pure, so the threshold is testable without a world: the whole reason a
/// village splits should be inspectable arithmetic rather than a feeling.
pub fn pressure(population: usize, roofed: usize, food: f32, fields: usize) -> f32 {
    if population == 0 {
        return 0.0;
    }
    // Roofless people are the loudest argument: a third of the town sleeping
    // by the fire is a town that is out of room.
    let roofless = population.saturating_sub(roofed) as f32 / population as f32;
    // A larder under two meals a head is thin; under one is famine.
    let hunger = (1.0 - (food / (population as f32 * 2.0)).min(1.0)).max(0.0);
    // And land: a field per three mouths is comfortable, none at all is not.
    let landless = (1.0 - (fields as f32 / (population as f32 / 3.0).max(1.0)).min(1.0)).max(0.0);
    roofless * 1.6 + hunger * 1.2 + landless * 0.6
}

/// Above this, the town is pressed enough that people will consider leaving.
pub const MUSTER_PRESSURE: f32 = 0.85;

/// Whether a spot is far enough from every town already standing.
pub fn clear_of_towns(at: Vec3, towns: &[Vec3]) -> bool {
    towns
        .iter()
        .all(|centre| at.distance(*centre) >= COLONY_SPACING)
}

/// Picks the ground a party would leave for: known, sound, and clear of every
/// town already standing.
///
/// Searched as a ring around the parent rather than the whole world — people
/// leave for somewhere they have heard of, a walk away, not for the far side
/// of the map.
pub fn choose_colony_site(
    terrain: &Terrain,
    rng: &mut Rng,
    from: Vec3,
    known: &super::explore::KnownWorld,
    towns: &[Vec3],
) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for _ in 0..2_500 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let reach = rng.range(COLONY_SPACING, COLONY_REACH);
        let (sin, cos) = angle.sin_cos();
        let (x, z) = (from.x + cos * reach, from.z + sin * reach);
        let at = Vec3::new(x, terrain.height_at(x, z), z);

        // Nobody leaves for ground no one has walked. This is the condition
        // that ties colonies to exploration: an unexplored world cannot
        // spread, however crowded its one village becomes.
        if !known.knows(at) {
            continue;
        }
        if !clear_of_towns(at, towns) {
            continue;
        }
        // Held to the founding's own standards.
        let Some((score, spot, _, _)) = score_town_ground(terrain, x, z, 0.7) else {
            continue;
        };
        // Nearer is better, all else equal: the road home should be walkable.
        let score = score - (reach / COLONY_REACH) * 1.2;
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, spot));
        }
    }
    best.map(|(_, spot)| spot)
}

/// Asks each town, now and then, whether anyone means to leave — and musters
/// the party if so.
#[allow(clippy::type_complexity)]
pub(super) fn muster_colonists(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut since_last: Local<f32>,
    terrain: Res<Terrain>,
    known: Option<Res<super::explore::KnownWorld>>,
    culture: Option<Res<SettlementCulture>>,
    mut rng: ResMut<SimRng>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut chronicles: Query<&mut Chronicle>,
    mut towns: Query<(Entity, &SettlementGround, &Settlement, &mut Stockpile)>,
    fields: Query<&MemberOf, With<super::work::Field>>,
    roofs: Query<&MemberOf, Or<(With<super::work::Hut>, With<super::work::Longhouse>)>>,
    housed: Query<&super::home::Home>,
    already: Query<(), With<Colonist>>,
    folk: Query<
        (
            Entity,
            &MemberOf,
            &crate::creature::genome::CreatureGenome,
            Option<&crate::witness::Temperament>,
            Option<&Vocation>,
            Option<&super::Spouse>,
        ),
        (With<Villager>, Without<Corpse>, Without<Childhood>),
    >,
) {
    *since_last += time.delta_secs();
    if *since_last < MUSTER_INTERVAL {
        return;
    }
    *since_last = 0.0;

    let (Some(known), Some(culture)) = (known, culture) else {
        return;
    };
    // One party at a time in the whole world. Two simultaneous exoduses would
    // read as the map falling apart rather than a town dividing.
    if !already.is_empty() {
        return;
    }

    let centres: Vec<Vec3> = towns
        .iter()
        .map(|(_, ground, _, _)| ground.centre)
        .collect();

    for (town, ground, settlement, mut store) in &mut towns {
        let members: Vec<_> = folk.iter().filter(|(_, m, ..)| m.0 == town).collect();
        let population = members.len();
        if population < PARENT_FLOOR + PARTY_FLOOR {
            continue;
        }
        let roofed = members
            .iter()
            .filter(|(who, ..)| housed.get(*who).is_ok())
            .count();
        let roof_count = roofs.iter().filter(|m| m.0 == town).count();
        let field_count = fields.iter().filter(|m| m.0 == town).count();
        let _ = roof_count;

        let strain = pressure(population, roofed, store.food(), field_count);
        if strain < MUSTER_PRESSURE {
            continue;
        }

        // Somewhere to go. Asked after the pressure test because it is the
        // expensive half.
        let Some(destination) =
            choose_colony_site(&terrain, &mut rng.0, ground.centre, &known, &centres)
        else {
            info!(
                "{} is pressed ({strain:.2}) but knows of nowhere to go",
                settlement.name
            );
            continue;
        };

        // Someone to lead: the boldest unwed adult, or failing that any adult
        // willing. The unwed go first — a colony is made of people with least
        // holding them in place.
        let mut roster: Vec<(Entity, f32)> = members
            .iter()
            .filter(|(_, _, genome, ..)| genome.age == Age::Adult)
            .map(|(who, _, _, manner, _, spouse)| {
                let boldness = manner.map_or(0.5, |t| t.boldness);
                // The unwed and the young-in-trade have least to lose.
                let unattached = if spouse.is_none() { 0.5 } else { 0.0 };
                (*who, boldness + unattached)
            })
            .collect();
        roster.sort_by(|a, b| b.1.total_cmp(&a.1));
        let take = PARTY_CEILING.min(population.saturating_sub(PARENT_FLOOR));
        if take < PARTY_FLOOR {
            continue;
        }
        let party: Vec<Entity> = roster.iter().take(take).map(|(who, _)| *who).collect();
        let Some(&leader) = party.first() else {
            continue;
        };

        // Provisions out of the parent's stores: enough for the road and a
        // first few days. A town too poor to provision a party keeps it.
        let road = ground.centre.distance(destination) / TRAVEL_PACE;
        let want_food = (party.len() as f32 * 2.0 + road / 60.0).min(store.food() * 0.4);
        if want_food < party.len() as f32 {
            info!(
                "{} would send a party but cannot provision one",
                settlement.name
            );
            continue;
        }
        let want_timber = (store.timber * 0.3).min(10.0);
        store.larder.draw(want_food);
        store.timber -= want_timber;

        // The colony is named and blazoned before they set out, in the mother
        // town's tongue: a place people are walking toward needs a name to
        // speak of on the road.
        let name = culture.language.name(&mut rng.0);
        let banner_ramp = *rng.0.pick(palette::CLOTH_RAMPS);
        let sigil = (rng.0.next_u32() as usize) % crate::sigil::SIGILS.len();

        commands.entity(leader).insert(ColonyCharter {
            destination,
            mother_town: town,
            food: want_food,
            timber: want_timber,
            name: name.clone(),
            banner_ramp,
            sigil,
        });
        for who in &party {
            commands.entity(*who).insert(Colonist {
                destination,
                leader,
            });
        }

        info!(
            "{} of {} set out to found {name} ({:.0} strides), pressure {strain:.2}",
            party.len(),
            settlement.name,
            ground.centre.distance(destination),
        );
        notices.write(crate::ui::Notice::fanfare(format!(
            "{} of {} set out to found {name}",
            party.len(),
            settlement.name
        )));
        let day = clock.day();
        for who in &party {
            if let Ok(mut story) = chronicles.get_mut(*who) {
                story.record(
                    day,
                    format!("left {} to help found {name}", settlement.name),
                );
            }
        }
        // One town per pass.
        return;
    }
}

/// Walks a party to their ground, and raises the town when they arrive.
#[allow(clippy::type_complexity)]
pub(super) fn walk_to_the_new_ground(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Res<Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut chronicles: Query<&mut Chronicle>,
    charters: Query<(Entity, &ColonyCharter)>,
    mut party: Query<
        (
            Entity,
            &Transform,
            &Colonist,
            &Person,
            &mut Activity,
            &mut MoveTarget,
        ),
        (With<Villager>, Without<Held>, Without<Airborne>),
    >,
) {
    for (leader, charter) in &charters {
        // The party is whoever is still walking under this leader.
        let mut walking = 0usize;
        let mut arrived = 0usize;
        for (_, at, colonist, _, _, _) in &party {
            if colonist.leader != leader {
                continue;
            }
            walking += 1;
            if at.translation.distance(colonist.destination) < 14.0 {
                arrived += 1;
            }
        }

        // The leader dead or taken, or the party thinned to nothing: the
        // venture fails, and the survivors are simply people standing in a
        // field a long way from home.
        if walking == 0 {
            commands.entity(leader).remove::<ColonyCharter>();
            continue;
        }

        // They raise the banner once most of them are standing on the ground.
        if arrived * 2 < walking {
            for (_, _, colonist, _, mut activity, mut target) in &mut party {
                if colonist.leader != leader {
                    continue;
                }
                if !matches!(*activity, Activity::Working) {
                    *activity = Activity::Working;
                }
                // Each walker steers by their OWN record of where the party is
                // bound, so one separated from the leader still walks the
                // right way instead of stopping where they stand.
                target.0 = Some(colonist.destination);
            }
            continue;
        }

        // Arrived. The same code that founded the first village founds this
        // one — banner, fire, woodpile, piles, stores.
        let spot = Vec3::new(
            charter.destination.x,
            terrain.height_at(charter.destination.x, charter.destination.z),
            charter.destination.z,
        );
        let (settlement, _woodpile) = found_settlement(
            &mut commands,
            &mut meshes,
            &mut materials,
            &terrain,
            spot,
            &charter.name,
            clock.day(),
            charter.banner_ramp,
            charter.sigil,
        );
        commands.queue(StockTheColony {
            settlement,
            food: charter.food,
            timber: charter.timber,
        });

        let day = clock.day();
        let mut joined = 0usize;
        for (who, _, colonist, person, mut activity, mut target) in &mut party {
            if colonist.leader != leader {
                continue;
            }
            // Citizenship changes hands here: from this moment every system
            // that asks "which town" gets the new answer for these people.
            commands
                .entity(who)
                .insert(MemberOf(settlement))
                .remove::<Colonist>()
                // Their old house is a hundred strides behind them.
                .remove::<super::home::Home>();
            *activity = Activity::Idle;
            target.0 = None;
            joined += 1;
            if let Ok(mut story) = chronicles.get_mut(who) {
                story.record(day, format!("helped raise {}", charter.name));
            }
            let _ = person;
        }
        commands.entity(leader).remove::<ColonyCharter>();

        info!(
            "{} was founded by {joined} out of {}",
            charter.name,
            charter.mother_town.to_bits()
        );
        notices.write(crate::ui::Notice::fanfare(format!(
            "{} is founded, {joined} souls under a new banner",
            charter.name
        )));
    }
}

/// Puts the party's provisions into their new town's stores.
struct StockTheColony {
    settlement: Entity,
    food: f32,
    timber: f32,
}

impl Command for StockTheColony {
    type Out = ();

    fn apply(self, world: &mut World) {
        if let Some(mut store) = world.get_mut::<Stockpile>(self.settlement) {
            store.larder.berries += self.food;
            store.timber += self.timber;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_comfortable_town_feels_no_pressure() {
        // Everyone roofed, larder deep, fields enough: nobody leaves.
        assert!(pressure(20, 20, 60.0, 8) < MUSTER_PRESSURE);
    }

    #[test]
    fn roofless_and_hungry_towns_press_outward() {
        // Half sleeping rough, a thin larder and no fields is a town that
        // has run out of room.
        assert!(pressure(20, 10, 8.0, 0) > MUSTER_PRESSURE);
    }

    #[test]
    fn pressure_rises_as_a_town_runs_short() {
        // Monotonic in each of the three strains, which is what makes the
        // threshold meaningful rather than arbitrary.
        let base = pressure(20, 20, 60.0, 8);
        assert!(pressure(20, 12, 60.0, 8) > base, "roofless should press");
        assert!(pressure(20, 20, 6.0, 8) > base, "hunger should press");
        assert!(pressure(20, 20, 60.0, 0) > base, "no land should press");
    }

    #[test]
    fn an_empty_town_is_not_under_pressure() {
        // Guards the divide-by-zero, and the nonsense of a town of nobody
        // mustering a party.
        assert_eq!(pressure(0, 0, 0.0, 0), 0.0);
    }

    #[test]
    fn colonies_keep_their_distance() {
        let towns = vec![Vec3::ZERO, Vec3::new(400.0, 0.0, 0.0)];
        assert!(!clear_of_towns(Vec3::new(50.0, 0.0, 0.0), &towns));
        assert!(!clear_of_towns(Vec3::new(380.0, 0.0, 0.0), &towns));
        assert!(clear_of_towns(Vec3::new(0.0, 0.0, 300.0), &towns));
        // And with no towns at all, anywhere will do.
        assert!(clear_of_towns(Vec3::ZERO, &[]));
    }

    #[test]
    fn a_town_never_hollows_itself_out() {
        // The floors have to leave a going concern behind: the smallest town
        // that can send the smallest party must still keep the parent floor.
        assert!(PARENT_FLOOR >= PARTY_CEILING);
        assert!(PARTY_FLOOR <= PARTY_CEILING);
        let smallest = PARENT_FLOOR + PARTY_FLOOR;
        let take = PARTY_CEILING.min(smallest - PARENT_FLOOR);
        assert!(
            take >= PARTY_FLOOR,
            "the smallest eligible town cannot fill a party"
        );
        assert!(
            smallest - take >= PARENT_FLOOR,
            "the parent would drop below its floor"
        );
    }

    #[test]
    fn a_colony_stands_within_a_walk_of_its_parent() {
        // Far enough to be its own place, near enough to matter to the first.
        assert!(COLONY_SPACING < COLONY_REACH);
        assert!(COLONY_SPACING > 100.0);
    }
}
