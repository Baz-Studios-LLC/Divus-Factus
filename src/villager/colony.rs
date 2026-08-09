//! Colonies: how one town becomes two.
//!
//! A settlement buds through one of two doors, and both are the regard
//! system's to open:
//!
//! 1. **Fullness.** A THRIVING town outgrows its ground — more mouths than
//!    beds, larder deep, nothing wrong except the size of the place. The
//!    boldest soul with a warm circle gathers their people and goes. This is
//!    the common door: growth, not misery. A starving town does not bud, it
//!    prays.
//! 2. **Fracture.** A heart nursing real grudges — two or more neighbours
//!    held in open hatred — walks out and takes whoever loves them along.
//!    The rare door, and the dramatic one: the town that splits in anger
//!    splits along the lines the regard graph already drew.
//!
//! Either way, **the god is asked first.** The would-be leader kneels and
//! prays for the road — a pink asking on THE PRAYERS board like any other.
//! Set a gift down beside them and the venture goes blessed; smite the
//! ground at their feet and it is forbidden; say nothing and they go anyway,
//! unheard, and remember it.
//!
//! The party is **whole households, chosen by warmth**: whoever holds the
//! leader dearest, spouses together always — a marriage outranks a venture —
//! and children with their parents. Nobody's family is split by a banner.
//!
//! The party walks. They are not teleported: the road is visible, it takes
//! days, they eat from satchels drawn out of the parent's stores, and a god
//! who objects has the whole road to say so.

use bevy::prelude::*;

use super::belief::{self, Faith, Prayer, PrayerKind, PrayerLedger, PrayerOutcome};
use super::regard::Regard;
use super::work::{Rations, Stockpile};
use super::{
    Activity, Chronicle, MemberOf, Needs, Parentage, Person, Settlement, SettlementCulture,
    SettlementGround, SimRng, Spouse, Villager, found_settlement, score_town_ground,
};
use crate::creature::genome::Age;
use crate::creature::{Airborne, Corpse, Held, MoveTarget};
use crate::palette;
use crate::rng::Rng;
use crate::terrain::Terrain;
use crate::witness::{DivineEvent, DivineEventKind};

/// How often the question is asked. A departure is a once-in-a-generation
/// thing; there is no need to check it every frame.
const MUSTER_INTERVAL: f32 = 30.0;

/// The fewest people a town keeps for itself. A settlement does not hollow
/// itself out to seed another — the parent must remain a going concern.
const PARENT_FLOOR: usize = 14;

/// The fewest adults who will set out together. Below this it is not a
/// colony, it is a family alone in the woods, and the woods win.
const PARTY_FLOOR: usize = 5;

/// The most adults who will leave in one party. Children ride along on top.
const PARTY_CEILING: usize = 8;

/// A town this big has outgrown one square whatever its comforts.
const FULL_TOWN: usize = 22;

/// How far a colony must stand from every existing town. Past the parent's
/// DAILY working ground, so two squares are not felling the same grove —
/// though the far-ranging trades (hunters at their widest) will still cross
/// paths between towns, which is the road being real. Close enough that the
/// known world reaches it within a village's first summers of expeditions.
const COLONY_SPACING: f32 = 220.0;

/// The furthest afield a party will settle.
const COLONY_REACH: f32 = 620.0;

/// Walking pace, in world units per second, for estimating the road.
const TRAVEL_PACE: f32 = 2.4;

/// How long the leader kneels before giving up on an answer. Twice a food
/// prayer's patience: nobody is dying, and the asking deserves the chance
/// to be seen.
const ROAD_PATIENCE: f32 = 240.0;

/// A bond at or below this is a grudge that argues for leaving.
const GRUDGE_DEPTH: f32 = -0.6;

/// Warmth toward the leader at or above this will follow them out.
const FOLLOW_WARMTH: f32 = 0.25;

/// A spouse colder than this toward the leader keeps their WHOLE household
/// home — the marriage outranks the venture, in both directions.
const SPOUSE_VETO: f32 = -0.25;

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

/// On a town whose road the god has forbidden: no asking is made again
/// until the memory fades.
#[derive(Component, Debug)]
pub struct RoadBarred {
    pub until: f64,
}

/// How long a smite bars the road, in world seconds — about a season.
const BARRED_FOR: f64 = 1800.0;

/// The fullness door: a THRIVING town that has outgrown its ground.
///
/// Pure, so the threshold is inspectable arithmetic rather than a feeling.
/// Note what is NOT here: hunger and homeless misery. A struggling town
/// keeps its people and prays; only a town doing well — larder deep per
/// head — buds a daughter. Villages must thrive; colonies are what
/// thriving looks like when the square runs out.
pub fn fullness(souls: usize, beds: usize, food: f32) -> bool {
    souls >= FULL_TOWN && souls > beds && food >= souls as f32 * 1.5
}

/// The fracture door, judged for one heart: enough open hatreds that
/// leaving beats staying, in a person bold enough to lead.
pub fn fractured(grudges: usize, boldness: f32) -> bool {
    grudges >= 2 && boldness >= 0.55
}

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

/// One soul as the party-picker sees them: who they are, how warm they run
/// toward the leader, and whose household they belong to.
pub struct Prospect {
    pub who: Entity,
    pub adult: bool,
    /// Warmth toward the would-be leader.
    pub warmth: f32,
    pub spouse: Option<Entity>,
    /// Living parents, for children who follow their households.
    pub parents: Vec<Entity>,
}

/// Chooses the party: whole households, by warmth.
///
/// Households move as one: an adult and their spouse are a unit with their
/// children attached, and the unit goes entire or stays entire. A spouse
/// who hates the leader vetoes their whole household — the marriage
/// outranks the venture, in both directions. Units join warmest-first
/// until the adult ceiling or the parent's floor calls a halt.
///
/// Returns None when the venture cannot fill a party or would hollow the
/// town out — the asking dissolves, no one leaves.
pub fn pick_party(leader: Entity, souls: &[Prospect], town_souls: usize) -> Option<Vec<Entity>> {
    let by_who = |who: Entity| souls.iter().find(|s| s.who == who);

    // Group the town into household units: each adult couple (or single
    // adult) with their children attached by parentage.
    let mut units: Vec<(f32, Vec<Entity>, usize)> = Vec::new(); // (warmth, members, adults)
    let mut grouped: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for soul in souls.iter().filter(|s| s.adult) {
        if grouped.contains(&soul.who) {
            continue;
        }
        let mut members = vec![soul.who];
        let mut adults = 1usize;
        let mut warmth = soul.warmth;
        let mut vetoed = soul.warmth < SPOUSE_VETO;
        if let Some(spouse) = soul.spouse
            && let Some(partner) = by_who(spouse)
        {
            members.push(partner.who);
            adults += 1;
            warmth = warmth.max(partner.warmth);
            vetoed |= partner.warmth < SPOUSE_VETO;
        }
        for member in &members {
            grouped.insert(*member);
        }
        // The children of this couple.
        for child in souls.iter().filter(|s| !s.adult) {
            if !child.parents.is_empty()
                && child
                    .parents
                    .iter()
                    .all(|parent| members.contains(parent) || by_who(*parent).is_none())
                && child.parents.iter().any(|parent| members.contains(parent))
            {
                members.push(child.who);
                grouped.insert(child.who);
            }
        }
        // A unit with a heart set against the leader stays home whole —
        // unless it is the leader's own, which leads the column.
        let leads = members.contains(&leader);
        if vetoed && !leads {
            continue;
        }
        units.push((if leads { f32::INFINITY } else { warmth }, members, adults));
    }

    // Warmest households first, the leader's own always in front.
    units.sort_by(|a, b| b.0.total_cmp(&a.0));

    let allowed = town_souls.saturating_sub(PARENT_FLOOR);
    let mut chosen: Vec<Entity> = Vec::new();
    let mut adults = 0usize;
    for (warmth, members, unit_adults) in units {
        let leads = members.contains(&leader);
        if !leads && warmth < FOLLOW_WARMTH {
            break;
        }
        if adults + unit_adults > PARTY_CEILING || chosen.len() + members.len() > allowed {
            if leads {
                // The leader's own household cannot come: no venture.
                return None;
            }
            continue;
        }
        adults += unit_adults;
        chosen.extend(members);
    }

    if !chosen.contains(&leader) || adults < PARTY_FLOOR {
        return None;
    }
    Some(chosen)
}

/// Asks each town, now and then, whether anyone means to leave — and if so,
/// sends the would-be leader to their knees to ask the god for the road.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn muster_colonists(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut since_last: Local<f32>,
    // Bundled: this system rides Bevy's sixteen-parameter ceiling, and
    // these three are one subject - the world as the town knows it.
    world: (
        Res<Terrain>,
        Option<Res<super::explore::KnownWorld>>,
        Option<Res<SettlementCulture>>,
    ),
    mut rng: ResMut<SimRng>,
    mut wanted: ResMut<super::explore::GroundWanted>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut tongue: Option<ResMut<crate::telling::Tongue>>,
    mut visuals: (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut chronicles: Query<&mut Chronicle>,
    towns: Query<(
        Entity,
        &SettlementGround,
        &Settlement,
        &Stockpile,
        Option<&RoadBarred>,
    )>,
    buildings: Query<(&super::work::Building, &MemberOf)>,
    asking: Query<&Prayer>,
    already: Query<(), With<Colonist>>,
    folk: Query<
        (
            Entity,
            &MemberOf,
            &crate::creature::genome::CreatureGenome,
            Option<&crate::witness::Temperament>,
            Option<&Regard>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
) {
    *since_last += time.delta_secs();
    if *since_last < MUSTER_INTERVAL {
        return;
    }
    *since_last = 0.0;

    let (terrain, known, culture) = world;
    let (Some(known), Some(culture)) = (known, culture) else {
        return;
    };
    // One venture at a time in the whole world: no simultaneous exoduses,
    // and no second asking while one road prayer is already on the board.
    if !already.is_empty() {
        return;
    }
    if asking
        .iter()
        .any(|prayer| matches!(prayer.kind, PrayerKind::Road { .. }))
    {
        return;
    }

    let centres: Vec<Vec3> = towns
        .iter()
        .map(|(_, ground, _, _, _)| ground.centre)
        .collect();

    for (town, ground, settlement, store, barred) in &towns {
        if barred.is_some_and(|bar| clock.elapsed < bar.until) {
            continue;
        }
        let members: Vec<_> = folk.iter().filter(|(_, m, ..)| m.0 == town).collect();
        let souls = members.len();
        if souls < PARENT_FLOOR + PARTY_FLOOR {
            continue;
        }

        // Beds under every roof the town has raised, hall headroom included.
        let (mut houses, mut longhouses, mut halls) = (0usize, 0usize, 0usize);
        for (building, owner) in &buildings {
            if owner.0 != town {
                continue;
            }
            match building.kind {
                super::work::BuildingKind::House => houses += 1,
                super::work::BuildingKind::Longhouse => longhouses += 1,
                super::work::BuildingKind::TownHall => halls += 1,
                _ => {}
            }
        }
        let beds = super::home::shelter_capacity(houses, longhouses, halls);

        // The two doors. Fracture is looked for first: a town can be both
        // full and angry, and the angry story is the truer one to tell.
        let town_folk: std::collections::HashSet<Entity> =
            members.iter().map(|(who, ..)| *who).collect();
        let aggrieved = members
            .iter()
            .filter(|(_, _, genome, ..)| genome.age == Age::Adult)
            .filter_map(|(who, _, _, manner, regard)| {
                let regard = regard.as_ref()?;
                let grudges = regard
                    .bonds
                    .iter()
                    .filter(|bond| bond.warmth <= GRUDGE_DEPTH && town_folk.contains(&bond.toward))
                    .count();
                let boldness = manner.map_or(0.5, |t| t.boldness);
                fractured(grudges, boldness).then_some((*who, grudges))
            })
            .max_by_key(|(_, grudges)| *grudges);

        let full = fullness(souls, beds, store.food());
        if aggrieved.is_none() && !full {
            continue;
        }

        // Somewhere to go. Asked after the doors because it is the
        // expensive half.
        let Some(destination) =
            choose_colony_site(&terrain, &mut rng.0, ground.centre, &known, &centres)
        else {
            // Raise the order: the explorers push for colony ground until
            // somewhere legal is known and the asking can be made.
            if !wanted.0 {
                info!(
                    "{} would send out a party but knows of nowhere to go - \
                     the explorers are given the order",
                    settlement.name
                );
                notices.write(crate::ui::Notice::new(format!(
                    "{} wants ground for a new village; its explorers push the frontier",
                    settlement.name
                )));
            }
            wanted.0 = true;
            continue;
        };
        wanted.0 = false;

        // Who carries it: the aggrieved, or — through the fullness door —
        // the boldest adult enough hearts are warm toward to make a party.
        let leader = aggrieved.map(|(who, _)| who).or_else(|| {
            members
                .iter()
                .filter(|(_, _, genome, ..)| genome.age == Age::Adult)
                .map(|(who, _, _, manner, _)| {
                    let followers = members
                        .iter()
                        .filter(|(_, _, _, _, regard)| {
                            regard
                                .as_ref()
                                .is_some_and(|r| r.toward(*who) >= FOLLOW_WARMTH)
                        })
                        .count();
                    (*who, manner.map_or(0.5, |t| t.boldness), followers)
                })
                .filter(|(_, _, followers)| *followers + 1 >= PARTY_FLOOR)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(who, _, _)| who)
        });
        let Some(leader) = leader else {
            continue;
        };

        // The asking. The colony is named and blazoned NOW, so the blessing
        // blesses a particular flag — and the board can speak of the place.
        let name = culture.language.name(&mut rng.0);
        let banner_ramp = *rng.0.pick(palette::CLOTH_RAMPS);
        let sigil = (rng.0.next_u32() as usize) % crate::sigil::SIGILS.len();
        let words = tongue.as_mut().and_then(|tongue| {
            tongue.pray(leader, &["road"], crate::telling::FaithBand::Sure, None)
        });
        commands.entity(leader).insert((
            Prayer {
                remaining: ROAD_PATIENCE,
                words,
                bubbled: false,
                kind: PrayerKind::Road {
                    destination,
                    colony: name.clone(),
                    mother: town,
                    banner_ramp,
                    sigil,
                },
            },
            // Down on their knees, wherever the asking found them. Life may
            // stand them back up — a meal, a wolf — and the asking stands
            // on the board regardless; the kneeling is how it BEGINS.
            Activity::Praying,
            MoveTarget::default(),
        ));
        belief::raise_prayer_mote(&mut commands, &mut visuals.0, &mut visuals.1, leader);

        let door = if aggrieved.is_some() {
            "bitter at their neighbours"
        } else {
            "the town grown past its beds"
        };
        info!(
            "a road prayer rises from {}: {name} would stand {:.0} strides out ({door})",
            settlement.name,
            ground.centre.distance(destination),
        );
        notices.write(crate::ui::Notice::prayer(format!(
            "A blessing is asked for the road: {name} would be founded from {}",
            settlement.name
        )));
        if let Ok(mut story) = chronicles.get_mut(leader) {
            story.record(
                clock.day(),
                format!("asked the god's blessing for the road to {name}"),
            );
        }
        // One asking per pass.
        return;
    }
}

/// Answers the road prayer: a gift blesses it, a smite forbids it, and
/// silence lets it lapse — in which case the party goes anyway, unheard.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn bless_or_bar_the_road(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut ledger: ResMut<PrayerLedger>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut legend: ResMut<belief::Legend>,
    kids: Query<&Children>,
    motes: Query<Entity, With<belief::PrayerMote>>,
    gifts: Query<&Transform, (With<crate::hand::DivinelyPlaced>, Without<Held>)>,
    mut smites: MessageReader<DivineEvent>,
    mut towns: Query<&mut Stockpile>,
    grounds: Query<&SettlementGround>,
    mut chronicles: Query<&mut Chronicle>,
    mut praying: Query<(
        Entity,
        &Transform,
        &Person,
        &Prayer,
        &mut Faith,
        &mut Activity,
    )>,
    folk: Query<
        (
            Entity,
            &MemberOf,
            &crate::creature::genome::CreatureGenome,
            Option<&Spouse>,
            Option<&Regard>,
            Option<&Parentage>,
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
) {
    let strikes: Vec<Vec3> = smites
        .read()
        .filter(|event| matches!(event.kind, DivineEventKind::Smote))
        .map(|event| event.position)
        .collect();

    for (leader, at, person, prayer, mut faith, mut activity) in &mut praying {
        let PrayerKind::Road {
            destination,
            colony,
            mother,
            banner_ramp,
            sigil,
        } = &prayer.kind
        else {
            continue;
        };

        // The god's NO: a smite near the kneeling asker. The road is
        // barred, the town remembers, and the fear itself is a kind of
        // faith — they were heard.
        if strikes
            .iter()
            .any(|strike| strike.distance(at.translation) < 30.0)
        {
            faith.shift(0.05);
            legend.dread += 1.0;
            ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Curdled);
            if let Ok(mut story) = chronicles.get_mut(leader) {
                story.record(
                    clock.day(),
                    format!("asked for the road to {colony}, and the god forbade it"),
                );
            }
            notices.write(crate::ui::Notice::new(format!(
                "The road to {colony} is forbidden"
            )));
            commands.entity(*mother).insert(RoadBarred {
                until: clock.elapsed + BARRED_FOR,
            });
            if *activity == Activity::Praying {
                *activity = Activity::Idle;
            }
            belief::end_prayer(&mut commands, leader, &kids, &motes);
            continue;
        }

        // The god's YES: any gift set down beside them. The blessing goes
        // with them — the gift itself lands near the party and travels in
        // their arms or their bellies.
        let blessed = gifts
            .iter()
            .any(|gift| gift.translation.distance(at.translation) < belief::ANSWER_RADIUS);
        // Silence: the patience runs out, and they go anyway.
        let lapsed = prayer.remaining <= 0.5;
        if !blessed && !lapsed {
            continue;
        }

        if blessed {
            faith.shift(0.2);
            ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Answered);
            if let Ok(mut story) = chronicles.get_mut(leader) {
                story.record(
                    clock.day(),
                    format!("asked for the road to {colony}, and it was blessed"),
                );
            }
            notices.write(crate::ui::Notice::fanfare(format!(
                "The road to {colony} is blessed"
            )));
        } else {
            faith.shift(-0.06);
            ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Curdled);
            if let Ok(mut story) = chronicles.get_mut(leader) {
                story.record(
                    clock.day(),
                    format!("asked for the road to {colony}; no answer came, and they went anyway"),
                );
            }
        }

        // Muster the party NOW, by warmth as it stands today — hearts may
        // have moved while the prayer hung.
        let town_members: Vec<_> = folk.iter().filter(|(_, m, ..)| m.0 == *mother).collect();
        let souls = town_members.len();
        let prospects: Vec<Prospect> = town_members
            .iter()
            .map(|(who, _, genome, spouse, regard, parentage)| {
                let warmth = regard.as_ref().map_or(0.0, |r| r.toward(leader));
                let spouse = spouse
                    .map(|s| s.0)
                    .filter(|s| town_members.iter().any(|(other, ..)| other == s));
                let parents = parentage.map_or(Vec::new(), |kin| {
                    [kin.mother, kin.father]
                        .into_iter()
                        .filter(|parent| town_members.iter().any(|(other, ..)| other == parent))
                        .collect()
                });
                Prospect {
                    who: *who,
                    adult: genome.age == Age::Adult,
                    warmth,
                    spouse,
                    parents,
                }
            })
            .collect();

        let party = pick_party(leader, &prospects, souls);
        belief::end_prayer(&mut commands, leader, &kids, &motes);
        if *activity == Activity::Praying {
            *activity = Activity::Idle;
        }

        let Some(party) = party else {
            info!("the road to {colony} found no party willing; the venture dissolves");
            notices.write(crate::ui::Notice::new(format!(
                "Nobody would walk the road to {colony}"
            )));
            continue;
        };

        // Provisions out of the parent's stores: satchels for the road and
        // seed for the first days. A town too poor to provision a party
        // keeps it.
        let Ok(mut store) = towns.get_mut(*mother) else {
            continue;
        };
        let adults = prospects
            .iter()
            .filter(|p| p.adult && party.contains(&p.who))
            .count()
            + 1;
        let road = grounds
            .get(*mother)
            .map_or(300.0, |g| g.centre.distance(*destination))
            / TRAVEL_PACE;
        let satchels = adults as f32 * 2.0;
        let seed = (party.len() as f32 * 2.0 + road / 60.0).min(store.food() * 0.4 - satchels);
        if seed < party.len() as f32 {
            info!("the road to {colony} is unprovisioned; the venture dissolves");
            notices.write(crate::ui::Notice::new(format!(
                "{colony} goes unfounded: the stores cannot provision the road"
            )));
            continue;
        }
        store.larder.draw(satchels + seed);
        let timber = (store.timber * 0.3).min(10.0);
        store.timber -= timber;

        commands.entity(leader).insert(ColonyCharter {
            destination: *destination,
            mother_town: *mother,
            food: seed,
            timber,
            name: colony.clone(),
            banner_ramp: *banner_ramp,
            sigil: *sigil,
        });
        for who in &party {
            commands.entity(*who).insert(Colonist {
                destination: *destination,
                leader,
            });
            let is_adult = prospects.iter().any(|p| p.who == *who && p.adult) || *who == leader;
            if is_adult {
                commands.entity(*who).insert(Rations(2.0));
            }
            if let Ok(mut story) = chronicles.get_mut(*who) {
                story.record(clock.day(), format!("set out to help found {colony}"));
            }
        }

        info!(
            "{} souls set out to found {colony}, {}",
            party.len(),
            if blessed { "blessed" } else { "unheard" },
        );
        notices.write(crate::ui::Notice::fanfare(format!(
            "{} souls set out to found {colony}",
            party.len()
        )));
    }
}

/// Walks a party to their ground, feeds them from their satchels, and
/// raises the town when they arrive.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn walk_to_the_new_ground(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Res<Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut chronicles: Query<&mut Chronicle>,
    charters: Query<(Entity, &ColonyCharter)>,
    settlements: Query<&Settlement>,
    mut bushes: Query<(Entity, &GlobalTransform, &mut crate::scatter::FoodSource)>,
    mut party: Query<
        (
            Entity,
            &Transform,
            &Colonist,
            &mut Activity,
            &mut MoveTarget,
            &mut Needs,
            Option<&mut Rations>,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
        ),
    >,
) {
    for (leader, charter) in &charters {
        // The party is whoever is still walking under this leader.
        let mut walking = 0usize;
        let mut arrived = 0usize;
        for (_, at, colonist, ..) in &party {
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
            for (_, at, colonist, mut activity, mut target, mut needs, mut rations) in &mut party {
                if colonist.leader != leader {
                    continue;
                }
                // A walker on their knees is left to their prayer — the
                // road waits for the god's business, never the reverse.
                if matches!(*activity, Activity::Praying) {
                    continue;
                }
                // The road feeds itself: the satchel first, the heath
                // after, and every picking packs a little back into the
                // satchel. The empty satchel is KEPT — the next bush
                // refills it.
                if let Some(meal) = super::explore::forage_tick(
                    at.translation,
                    time.delta_secs(),
                    &mut needs,
                    rations.as_deref_mut(),
                    &mut bushes,
                ) {
                    if !matches!(*activity, Activity::Working) {
                        *activity = Activity::Working;
                    }
                    target.0 = Some(meal);
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
        for (who, _, colonist, mut activity, mut target, ..) in &mut party {
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
        }
        commands.entity(leader).remove::<ColonyCharter>();

        let mother = settlements
            .get(charter.mother_town)
            .map_or("their old home".to_string(), |s| s.name.clone());
        info!(
            "{} was founded by {joined} souls out of {mother}",
            charter.name
        );
        notices.write(crate::ui::Notice::fanfare(format!(
            "{} is founded — {joined} souls out of {mother}, under a new banner",
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
    fn a_comfortable_town_is_not_full() {
        // Room to spare and a deep larder: nobody leaves.
        assert!(!fullness(20, 24, 60.0));
    }

    #[test]
    fn a_thriving_town_grown_past_its_beds_is_full() {
        assert!(fullness(24, 20, 60.0));
    }

    #[test]
    fn a_starving_town_does_not_bud() {
        // More mouths than beds but a thin larder: this town PRAYS, it does
        // not colonise. Misery is the god's lever, not the mapmaker's.
        assert!(!fullness(24, 20, 10.0));
    }

    #[test]
    fn a_small_town_is_never_full() {
        assert!(!fullness(12, 4, 100.0));
    }

    #[test]
    fn fracture_needs_both_grudges_and_nerve() {
        assert!(fractured(2, 0.6));
        assert!(!fractured(1, 0.9), "one grudge is a quarrel, not an exodus");
        assert!(!fractured(4, 0.3), "the timid stew instead of leaving");
    }

    #[test]
    fn colonies_keep_their_distance() {
        let towns = vec![Vec3::ZERO, Vec3::new(400.0, 0.0, 0.0)];
        assert!(!clear_of_towns(Vec3::new(50.0, 0.0, 0.0), &towns));
        assert!(clear_of_towns(Vec3::new(0.0, 0.0, 300.0), &towns));
        // And with no towns at all, anywhere will do.
        assert!(clear_of_towns(Vec3::ZERO, &[]));
    }

    #[test]
    fn a_colony_stands_within_a_walk_of_its_parent() {
        // Far enough to be its own place — clear of the parent's daily
        // working ground — and near enough to matter to the first.
        assert!(COLONY_SPACING < COLONY_REACH);
        assert!(COLONY_SPACING > crate::villager::work::WORK_REACH);
    }

    fn prospect(who: Entity, warmth: f32) -> Prospect {
        Prospect {
            who,
            adult: true,
            warmth,
            spouse: None,
            parents: Vec::new(),
        }
    }

    #[test]
    fn the_party_is_chosen_by_warmth() {
        let leader = Entity::from_raw_u32(1).unwrap();
        let mut souls: Vec<Prospect> = (2..30)
            .map(|n| {
                prospect(
                    Entity::from_raw_u32(n).unwrap(),
                    if n < 10 { 0.8 } else { 0.1 },
                )
            })
            .collect();
        souls.push(prospect(leader, 1.0));

        let party = pick_party(leader, &souls, souls.len()).expect("a party should muster");
        assert!(party.contains(&leader));
        assert_eq!(party.len(), PARTY_CEILING, "the warm fill the party");
        for who in &party {
            let soul = souls.iter().find(|s| s.who == *who).unwrap();
            assert!(
                soul.warmth >= FOLLOW_WARMTH || *who == leader,
                "nobody cold toward the leader walks"
            );
        }
    }

    #[test]
    fn a_hating_spouse_keeps_the_household_home() {
        let leader = Entity::from_raw_u32(1).unwrap();
        let devoted = Entity::from_raw_u32(2).unwrap();
        let their_spouse = Entity::from_raw_u32(3).unwrap();
        let mut souls = vec![prospect(leader, 1.0)];
        souls.push(Prospect {
            who: devoted,
            adult: true,
            warmth: 0.9,
            spouse: Some(their_spouse),
            parents: Vec::new(),
        });
        souls.push(Prospect {
            who: their_spouse,
            adult: true,
            warmth: -0.7,
            spouse: Some(devoted),
            parents: Vec::new(),
        });
        // Enough warm singles that only the couple is in question.
        for n in 10..40 {
            souls.push(prospect(Entity::from_raw_u32(n).unwrap(), 0.5));
        }

        let party = pick_party(leader, &souls, souls.len()).expect("a party should muster");
        assert!(
            !party.contains(&devoted) && !party.contains(&their_spouse),
            "the marriage outranks the venture: the household stays whole, at home"
        );
    }

    #[test]
    fn spouses_walk_together_and_children_follow() {
        let leader = Entity::from_raw_u32(1).unwrap();
        let wife = Entity::from_raw_u32(2).unwrap();
        let husband = Entity::from_raw_u32(3).unwrap();
        let child = Entity::from_raw_u32(4).unwrap();
        let mut souls = vec![prospect(leader, 1.0)];
        souls.push(Prospect {
            who: wife,
            adult: true,
            warmth: 0.9,
            spouse: Some(husband),
            parents: Vec::new(),
        });
        souls.push(Prospect {
            who: husband,
            adult: true,
            warmth: 0.3,
            spouse: Some(wife),
            parents: Vec::new(),
        });
        souls.push(Prospect {
            who: child,
            adult: false,
            warmth: 0.0,
            spouse: None,
            parents: vec![wife, husband],
        });
        for n in 10..40 {
            souls.push(prospect(Entity::from_raw_u32(n).unwrap(), 0.5));
        }

        let party = pick_party(leader, &souls, souls.len()).expect("a party should muster");
        assert!(
            party.contains(&wife) && party.contains(&husband),
            "spouses go together"
        );
        assert!(party.contains(&child), "children follow their households");
    }

    /// Every colony system, initialized against one world.
    ///
    /// Bevy's system-parameter conflicts (B0001) panic when a system first
    /// runs, not when it compiles — a suite of pure-function tests proves
    /// nothing about them. This runs each system once so a conflicting
    /// query pair fails here instead of on Brett's next boot.
    #[test]
    fn the_colony_systems_share_the_world_without_conflict() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.init_resource::<crate::calendar::WorldClock>();
        app.insert_resource(Terrain::new(77));
        app.insert_resource(SimRng(crate::rng::Rng::new(7)));
        app.init_resource::<PrayerLedger>();
        app.init_resource::<belief::Legend>();
        app.init_resource::<crate::villager::explore::GroundWanted>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.add_message::<crate::ui::Notice>();
        app.add_message::<DivineEvent>();

        let world = app.world_mut();
        world.run_system_once(muster_colonists).unwrap();
        world.run_system_once(bless_or_bar_the_road).unwrap();
        world.run_system_once(walk_to_the_new_ground).unwrap();
    }

    #[test]
    fn a_town_never_hollows_itself_out() {
        let leader = Entity::from_raw_u32(1).unwrap();
        let mut souls = vec![prospect(leader, 1.0)];
        for n in 2..=(PARENT_FLOOR + PARTY_FLOOR) as u32 {
            souls.push(prospect(Entity::from_raw_u32(n).unwrap(), 0.9));
        }
        // The smallest eligible town: the party may take exactly its floor.
        let party = pick_party(leader, &souls, souls.len()).expect("a party should muster");
        assert!(
            souls.len() - party.len() >= PARENT_FLOOR,
            "the parent keeps its floor"
        );
        assert!(party.len() >= PARTY_FLOOR);

        // One soul fewer and no party can be cut that keeps the floor.
        souls.pop();
        assert!(
            pick_party(leader, &souls, souls.len()).is_none(),
            "too small to split: the venture dissolves"
        );
    }
}
