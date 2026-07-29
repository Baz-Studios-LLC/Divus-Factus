//! Callings: what a villager does all day, and how the village
//! reassigns its hands when a need goes unanswered.

use bevy::prelude::*;

use super::*;
use crate::creature::genome::{Age, CreatureGenome};
use crate::rng::Rng;
use crate::terrain::{Terrain, WATER_LEVEL};
/// A calling. Rolled once at adulthood and kept.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Vocation {
    Gatherer,
    Fisher,
    Hunter,
    Miner,
    Forester,
    Carpenter,
    /// Tills fields, tends crops, brings in harvests.
    Farmer,
    /// Lays the stone: foundations and, one day, walls.
    Mason,
    /// Feeds the tavern's kitchen; the village eats better for it.
    Cook,
    /// Tends the hurt back to their feet.
    Healer,
    /// Keeps the shrine, and retells what the god has done.
    Priest,
    /// Walks past the cairns and brings back the world.
    Explorer,
    /// Spear and post: walks the edge, walks the roads, meets the wolves.
    Guard,
}

impl Vocation {
    /// The vocation as the inspector names it.
    pub fn describe(self) -> &'static str {
        match self {
            Vocation::Gatherer => "gathers",
            Vocation::Fisher => "fishes",
            Vocation::Hunter => "hunts",
            Vocation::Miner => "mines",
            Vocation::Forester => "cuts wood",
            Vocation::Carpenter => "builds houses",
            Vocation::Farmer => "works the fields",
            Vocation::Mason => "lays stone",
            Vocation::Cook => "keeps the kitchen",
            Vocation::Healer => "tends the hurt",
            Vocation::Priest => "keeps the shrine",
            Vocation::Explorer => "walks past the cairns",
            Vocation::Guard => "stands guard",
        }
    }

    /// The line a life gains on taking this up.
    pub fn taking_up(self) -> &'static str {
        match self {
            Vocation::Gatherer => "took up the gathering basket",
            Vocation::Fisher => "took up the net",
            Vocation::Hunter => "took up the spear",
            Vocation::Miner => "took up the pick",
            Vocation::Forester => "took up the axe",
            Vocation::Carpenter => "took up the hammer",
            Vocation::Farmer => "took up the plough",
            Vocation::Mason => "took up the chisel",
            Vocation::Cook => "took up the ladle",
            Vocation::Healer => "took up the salves",
            Vocation::Priest => "took up the litany",
            Vocation::Explorer => "took up the wayfarer's staff",
            Vocation::Guard => "took up the guard's spear",
        }
    }
}

/// Rolls a calling from who this person is. Boldness pushes toward the hunt;
/// its absence toward the quiet work. Weighted, not deterministic — a timid
/// hunter exists, and is a story.
pub fn roll_vocation(boldness: f32, rng: &mut Rng) -> Vocation {
    // Cook, healer and priest are missing on purpose: those callings are not
    // rolled into, they are *called* into, when the village needs them.
    let weights = [
        (Vocation::Gatherer, 0.5 + (1.0 - boldness) * 0.8),
        (Vocation::Fisher, 0.9),
        (Vocation::Hunter, 0.25 + boldness * 1.6),
        (Vocation::Miner, 0.7),
        (Vocation::Forester, 0.9),
        (Vocation::Carpenter, 0.8),
        (Vocation::Farmer, 1.0),
        (Vocation::Mason, 0.7),
        (Vocation::Explorer, 0.1 + boldness * 0.4),
    ];
    let total: f32 = weights.iter().map(|(_, w)| w).sum();
    let mut roll = rng.f32() * total;
    for (vocation, weight) in weights {
        roll -= weight;
        if roll <= 0.0 {
            return vocation;
        }
    }
    Vocation::Gatherer
}

/// Adults take up a calling.
pub(crate) fn assign_vocations(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut rng: Option<ResMut<SimRng>>,
    mut grown: Query<
        (
            Entity,
            &CreatureGenome,
            &Person,
            &crate::witness::Temperament,
            Option<&mut Chronicle>,
        ),
        (With<Villager>, Without<Vocation>, Without<Corpse>),
    >,
) {
    let Some(rng) = rng.as_mut() else {
        return;
    };

    for (entity, genome, person, temperament, chronicle) in &mut grown {
        if genome.age != Age::Adult {
            continue;
        }
        let vocation = roll_vocation(temperament.boldness, &mut rng.0);
        commands.entity(entity).insert(vocation);
        info!("{} {}", person.name, vocation.taking_up());
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), vocation.taking_up());
        }
    }
}

/// The village calls people into the trades it lacks.
///
/// A tavern with no cook, a shrine with no keeper, a bruised village with no
/// healer: when demand appears, someone from the crowded trades sets their
/// tools down and takes up new ones — and their chronicle records the turn.
pub(crate) fn retrain(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    clock: Res<crate::calendar::WorldClock>,
    buildings: Query<&Building>,
    sites: Query<(&ConstructionSite, &Blueprint)>,
    stores: Query<&Stockpile>,
    homeless: Query<
        (),
        (
            With<Villager>,
            Without<crate::villager::home::Home>,
            Without<crate::creature::Childhood>,
            Without<Corpse>,
        ),
    >,
    mut notices: MessageWriter<crate::ui::Notice>,
    hurt: Query<&Vitality, (With<Villager>, Without<Corpse>)>,
    known: Option<Res<crate::villager::explore::KnownWorld>>,
    trees: Query<(&GlobalTransform, &crate::scatter::FellableTree)>,
    terrain: Option<Res<Terrain>>,
    site: Option<Res<SettlementSite>>,
    wild: Query<
        (&Transform, &crate::creature::genome::CreatureGenome),
        (With<crate::creature::wildlife::Wild>, Without<Corpse>),
    >,
    mut workers: Query<
        (Entity, &Vocation, &Person, Option<&mut Chronicle>),
        (With<Villager>, Without<Corpse>),
    >,
) {
    // A standing audit, not a frame-by-frame twitch: the village looks at
    // itself every little while and reallocates one pair of hands at most.
    *since_last += time.delta_secs();
    if *since_last < 20.0 {
        return;
    }
    *since_last = 0.0;

    let has_building = |kind: BuildingKind| buildings.iter().any(|b| b.kind == kind);
    let count_of = |v: Vocation| workers.iter().filter(|(_, w, _, _)| **w == v).count();
    let has_vocation = |v: Vocation| count_of(v) > 0;
    let timber_low = stores.iter().next().is_none_or(|s| s.timber < 6.0);
    // Whether any fellable tree stands on ground the village knows. When
    // none does, more foresters are useless — someone has to go find woods.
    let wood_known = known.as_ref().is_none_or(|k| {
        trees
            .iter()
            .any(|(at, tree)| tree.harvestable() && k.knows(at.translation()))
    });

    // The village's needs, in the order they kill. Every want here is a
    // deadlock somewhere else: no woodcutter starves the fire and every
    // build; no carpenter leaves the homeless in the rain; and so on.
    // Hunger kills first, so food hands top the ladder: a thin larder
    // with too few of the trades that fill it retrains someone toward
    // the water or the bushes before anything else gets a say.
    let mouths = workers.iter().count();
    // The larder's floor rises with the population: twelve food is a
    // pantry for eight and a rounding error for twenty-four.
    let food_low = stores
        .iter()
        .next()
        .is_none_or(|s| s.food() < (mouths as f32 * 1.2).max(12.0));
    let food_hands: usize = [
        Vocation::Fisher,
        Vocation::Gatherer,
        Vocation::Hunter,
        Vocation::Farmer,
    ]
    .into_iter()
    .map(|v| count_of(v))
    .sum();
    // Whether water lies within a working walk of the square — twelve
    // spokes, no dart-throwing, so no rng is needed here.
    let shore_near = terrain.as_ref().zip(site.as_ref()).is_some_and(|(t, s)| {
        (0..12).any(|i| {
            let angle = i as f32 / 12.0 * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            (1..=14).any(|step| {
                let d = step as f32 * 4.0;
                t.height_at(s.centre.x + cos * d, s.centre.z + sin * d) <= WATER_LEVEL
            })
        })
    });
    // Wolves pressing the village, or a manned post waiting for its
    // man: danger calls a guard the way hunger calls a fisher.
    let wolves_near = site.as_ref().map_or(0, |s| {
        wild.iter()
            .filter(|(at, genome)| {
                genome.species == crate::creature::genome::Species::Wolf
                    && at.translation.distance(s.centre) < 130.0
            })
            .count()
    });
    let mut wanted: Option<Vocation> = None;
    if food_low && food_hands * 5 < mouths.max(4) {
        wanted = Some(if shore_near {
            Vocation::Fisher
        } else {
            Vocation::Gatherer
        });
    } else if !has_vocation(Vocation::Guard)
        && (wolves_near >= 2 || has_building(BuildingKind::Watchtower))
    {
        wanted = Some(Vocation::Guard);
    } else if timber_low && !wood_known && !has_vocation(Vocation::Explorer) {
        wanted = Some(Vocation::Explorer);
    } else if !has_vocation(Vocation::Forester) && timber_low && wood_known {
        wanted = Some(Vocation::Forester);
    } else if has_building(BuildingKind::Dock) && !has_vocation(Vocation::Fisher) {
        wanted = Some(Vocation::Fisher);
    } else if !has_vocation(Vocation::Carpenter)
        && (!sites.is_empty() || homeless.iter().count() > 2)
    {
        wanted = Some(Vocation::Carpenter);
    } else if !has_vocation(Vocation::Mason)
        && sites
            .iter()
            .any(|(cs, plan)| cs.stone_laid < plan.kind.stone_cost())
    {
        wanted = Some(Vocation::Mason);
    } else if has_building(BuildingKind::Tavern) && !has_vocation(Vocation::Cook) {
        wanted = Some(Vocation::Cook);
    } else if has_building(BuildingKind::Shrine) && !has_vocation(Vocation::Priest) {
        wanted = Some(Vocation::Priest);
    } else if !has_vocation(Vocation::Healer) && hurt.iter().filter(|v| v.harm > 0.15).count() >= 2
    {
        wanted = Some(Vocation::Healer);
    }
    let Some(wanted) = wanted else {
        return;
    };

    // The most crowded trade gives someone up — and no trade gives up its
    // last pair of hands. The village rebalances itself by need.
    let mut counts: Vec<(Vocation, usize)> = [
        Vocation::Gatherer,
        Vocation::Fisher,
        Vocation::Hunter,
        Vocation::Miner,
        Vocation::Forester,
        Vocation::Carpenter,
        Vocation::Farmer,
        Vocation::Mason,
    ]
    .into_iter()
    .map(|v| (v, count_of(v)))
    .collect();
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let Some((give, _)) = counts.into_iter().find(|(v, n)| *v != wanted && *n >= 2) else {
        return;
    };
    let donor = workers.iter_mut().find(|(_, v, _, _)| **v == give);
    let Some((entity, old, person, chronicle)) = donor else {
        return;
    };
    let old = *old;

    commands.entity(entity).insert(wanted);
    info!(
        "{} set down their tools and {}",
        person.name,
        wanted.taking_up()
    );
    notices.write(crate::ui::Notice::new(format!(
        "{} {} - the village needed it",
        person.name,
        wanted.taking_up()
    )));
    if let Some(mut chronicle) = chronicle {
        chronicle.record(
            clock.day(),
            format!(
                "set down the tools of one who {} and {}",
                old.describe(),
                wanted.taking_up()
            ),
        );
    }
}
