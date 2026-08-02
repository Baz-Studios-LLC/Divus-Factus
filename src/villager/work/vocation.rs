//! Callings: what a villager does all day, and how the village
//! reassigns its hands when a need goes unanswered.

use bevy::prelude::*;

use super::*;
use crate::creature::genome::{Age, CreatureGenome};
use crate::rng::Rng;
use crate::terrain::{Terrain, WATER_LEVEL};
/// A calling. Rolled once at adulthood and kept.
#[derive(
    Component, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
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

/// A working life's accumulated craft, one score per calling ever
/// practised. Grown by doing the work, cycle by cycle; never lost —
/// a retrained mason still remembers the sea. 0 is first-day hands,
/// 1 is mastery.
#[derive(Component, Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Skills(pub Vec<(Vocation, f32)>);

impl Skills {
    pub fn of(&self, vocation: Vocation) -> f32 {
        self.0
            .iter()
            .find(|(v, _)| *v == vocation)
            .map_or(0.0, |(_, s)| *s)
    }

    /// One work cycle's worth of learning. Returns the mastery tier
    /// crossed, if this cycle crossed one — the chronicle wants to know.
    pub fn practice(&mut self, vocation: Vocation, amount: f32) -> Option<&'static str> {
        let entry = match self.0.iter_mut().find(|(v, _)| *v == vocation) {
            Some(entry) => entry,
            None => {
                self.0.push((vocation, 0.0));
                self.0.last_mut().expect("just pushed")
            }
        };
        let before = Self::tier(entry.1);
        entry.1 = (entry.1 + amount).min(1.0);
        let after = Self::tier(entry.1);
        (after != before).then_some(after)
    }

    /// The words for a level of craft, in the order a life earns them.
    pub fn tier_word(skill: f32) -> &'static str {
        Self::tier(skill)
    }

    fn tier(skill: f32) -> &'static str {
        if skill < 0.15 {
            "new to it"
        } else if skill < 0.45 {
            "getting the knack"
        } else if skill < 0.8 {
            "a practiced hand"
        } else {
            "a master"
        }
    }

    /// How this person's current calling reads, craft and all.
    pub fn describe(&self, vocation: Vocation) -> String {
        format!(
            "{} - {}",
            vocation.describe(),
            Self::tier(self.of(vocation))
        )
    }
}

impl Vocation {
    /// The trade as a name rather than a doing: what a person IS, for
    /// the label over their head.
    pub fn trade(self) -> &'static str {
        match self {
            Vocation::Gatherer => "gatherer",
            Vocation::Fisher => "fisher",
            Vocation::Hunter => "hunter",
            Vocation::Miner => "miner",
            Vocation::Forester => "forester",
            Vocation::Carpenter => "carpenter",
            Vocation::Farmer => "farmer",
            Vocation::Mason => "mason",
            Vocation::Cook => "cook",
            Vocation::Healer => "healer",
            Vocation::Priest => "priest",
            Vocation::Explorer => "explorer",
            Vocation::Guard => "guard",
        }
    }

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
        commands
            .entity(entity)
            .insert((vocation, Skills::default()));
        info!("{} {}", person.name, vocation.taking_up());
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), vocation.taking_up());
        }
    }
}

/// The morning muster: every day, before the first shift, the village
/// looks at what needs doing and sets its hands to it.
///
/// Trades were once rolled at adulthood and kept for life, with a slow
/// audit moving ONE pair of hands every twenty seconds when a post stood
/// empty. That left a third of a new village standing about: a farmer
/// with no field and a mason with no stone had nothing they were allowed
/// to do, and nothing short of a specific unfilled post would move them.
///
/// Now the want is counted in HANDS, and every hand is dealt to the want
/// it best answers. A trade nobody needs is a trade nobody holds, and a
/// village with more hands than wants puts the rest on the work that
/// always exists - food and firewood. Nobody has nothing to do.
pub(crate) fn morning_muster(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut mustered: Local<u32>,
    mut notices: MessageWriter<crate::ui::Notice>,
    town: (
        Query<&Building>,
        Query<(&ConstructionSite, &Blueprint)>,
        Query<&Stockpile>,
        Query<&Field>,
    ),
    homeless: Query<
        (),
        (
            With<Villager>,
            Without<crate::villager::home::Home>,
            Without<crate::creature::Childhood>,
            Without<Corpse>,
        ),
    >,
    hurt: Query<&Vitality, (With<Villager>, Without<Corpse>)>,
    ground: (
        Option<Res<crate::villager::explore::KnownWorld>>,
        Query<(&GlobalTransform, &crate::scatter::FellableTree)>,
        Option<Res<Terrain>>,
        Option<Res<SettlementSite>>,
    ),
    wild: Query<
        (&Transform, &crate::creature::genome::CreatureGenome),
        (With<crate::creature::wildlife::Wild>, Without<Corpse>),
    >,
    mut workers: Query<
        (
            Entity,
            &Vocation,
            &Person,
            Option<&mut Chronicle>,
            Option<&Skills>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
) {
    // Once a day, when the working morning opens.
    if !clock.work_hours() || *mustered == clock.day() {
        return;
    }
    *mustered = clock.day();

    let (buildings, sites, stores, fields) = town;
    let (known, trees, terrain, site) = ground;
    let mouths = workers.iter().count();
    if mouths == 0 {
        return;
    }

    let store = stores.iter().next();
    let food = store.map_or(0.0, |s| s.food());
    let timber = store.map_or(0.0, |s| s.timber);
    let masonry = store.map_or(0.0, |s| s.stone + s.clay);
    let has = |kind: BuildingKind| buildings.iter().any(|b| b.kind == kind);

    // The larder's floor rises with the population: twelve food is a
    // pantry for eight and a rounding error for twenty-four.
    let floor = (mouths as f32 * 1.2).max(12.0);
    let hungry = ((floor - food) / floor).clamp(0.0, 1.0);

    // Whether any fellable tree stands on ground the village knows. When
    // none does, more foresters are useless - someone has to go find woods.
    let wood_known = known.as_ref().is_none_or(|k| {
        trees
            .iter()
            .any(|(at, tree)| tree.harvestable() && k.knows(at.translation()))
    });
    // Whether water lies within a working walk of the square - twelve
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
    let wolves_near = site.as_ref().map_or(0, |s| {
        wild.iter()
            .filter(|(at, genome)| {
                genome.species == crate::creature::genome::Species::Wolf
                    && at.translation.distance(s.centre) < 130.0
            })
            .count()
    });

    let raising = sites.iter().count() as f32;
    let footings_waiting = sites
        .iter()
        .filter(|(cs, plan)| cs.stone_laid < cs.footing_stone(plan.kind))
        .count() as f32;
    let unworked_fields = fields
        .iter()
        .filter(|f| f.farmer == Entity::PLACEHOLDER)
        .count() as f32;

    // What the village wants, counted in hands. A want of zero is a trade
    // nobody takes up - which is the whole point: a farmer is somebody who
    // has a field, not somebody who once rolled a plough.
    let food_hands = mouths as f32 * (0.2 + 0.4 * hungry);
    let mut wanted: Vec<(Vocation, f32)> = vec![
        (Vocation::Gatherer, food_hands * 0.45),
        (
            Vocation::Fisher,
            if shore_near { food_hands * 0.3 } else { 0.0 },
        ),
        (Vocation::Hunter, food_hands * 0.25),
        (Vocation::Farmer, unworked_fields),
        // Wood: for the fire, for every build, and for the pile that has
        // to exist before a carpenter has anything to carry.
        (
            Vocation::Forester,
            if wood_known {
                1.0 + raising + if timber < 8.0 { 1.0 } else { 0.0 }
            } else {
                0.0
            },
        ),
        (Vocation::Explorer, if wood_known { 0.0 } else { 1.0 }),
        (Vocation::Carpenter, raising.min(3.0)),
        // A mason with no stone is a mason with nothing to do; the miner
        // is who fixes that, so the want lands on the miner first.
        (
            Vocation::Mason,
            if footings_waiting > 0.0 && masonry >= 1.0 {
                footings_waiting.min(2.0)
            } else {
                0.0
            },
        ),
        (
            Vocation::Miner,
            if footings_waiting > 0.0 && masonry < 4.0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            Vocation::Cook,
            if has(BuildingKind::Tavern) { 1.0 } else { 0.0 },
        ),
        (
            Vocation::Priest,
            if has(BuildingKind::Shrine) { 1.0 } else { 0.0 },
        ),
        (
            Vocation::Healer,
            if hurt.iter().filter(|v| v.harm > 0.15).count() >= 2 {
                1.0
            } else {
                0.0
            },
        ),
        (
            Vocation::Guard,
            if wolves_near >= 2 || has(BuildingKind::Watchtower) {
                1.0
            } else {
                0.0
            },
        ),
    ];
    // A roofless village needs a hammer whether or not ground is broken:
    // somebody has to be standing ready when it is.
    if homeless.iter().count() > 2 {
        if let Some(entry) = wanted.iter_mut().find(|(v, _)| *v == Vocation::Carpenter) {
            entry.1 = entry.1.max(1.0);
        }
    }

    // The work that always exists, for hands the wants do not reach. Every
    // one of these has something to do on the first morning of the world.
    let standing: Vec<Vocation> = [
        Vocation::Gatherer,
        Vocation::Forester,
        Vocation::Hunter,
        Vocation::Miner,
        Vocation::Fisher,
    ]
    .into_iter()
    .filter(|v| *v != Vocation::Fisher || shore_near)
    .filter(|v| *v != Vocation::Forester || wood_known)
    .collect();

    // Dealt in a fixed order so a re-run of the same world musters the
    // same way.
    let mut hands: Vec<Entity> = workers.iter().map(|(e, ..)| e).collect();
    hands.sort_unstable_by_key(|e| e.to_bits());

    let mut turned = 0usize;
    for hand in hands {
        let Ok((_, held, person, chronicle, skills)) = workers.get_mut(hand) else {
            continue;
        };
        let held = *held;
        let craft = |v: Vocation| skills.as_ref().map_or(0.0, |s| s.of(v));
        // Need first, aptitude second, and a thumb on the scale for the
        // trade they already know - a village that reshuffles every hand
        // every morning for a hair's advantage never gets good at
        // anything.
        let score =
            // The thumb on the scale for the trade already held. At 1.35
            // the muster reshuffled half the village every morning - the
            // wants shift a little daily, and a hair's advantage kept
            // outweighing the trade in hand - so nobody was ever a
            // second-day anything and no skill accrued.
            |v: Vocation, left: f32| left * (0.55 + craft(v)) * if v == held { 1.9 } else { 1.0 };
        let best = wanted
            .iter()
            .filter(|(_, left)| *left > 0.0)
            .map(|(v, left)| (*v, score(*v, *left)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(v, _)| v)
            .or_else(|| {
                // Nothing left to want: the standing work, at whatever
                // they are best at.
                standing
                    .iter()
                    .copied()
                    .max_by(|a, b| {
                        let (a_craft, b_craft) = (craft(*a), craft(*b));
                        let bump = |v: Vocation| if v == held { 0.2 } else { 0.0 };
                        (a_craft + bump(*a)).total_cmp(&(b_craft + bump(*b)))
                    })
                    .or(Some(Vocation::Gatherer))
            });
        let Some(best) = best else { continue };
        if let Some(entry) = wanted.iter_mut().find(|(v, _)| *v == best) {
            entry.1 -= 1.0;
        }
        if best == held {
            continue;
        }
        turned += 1;
        commands.entity(hand).insert(best);
        info!(
            "{} set down their tools and {}",
            person.name,
            best.taking_up()
        );
        if let Some(mut chronicle) = chronicle {
            chronicle.record(
                clock.day(),
                format!(
                    "set down the tools of one who {} and {}",
                    held.describe(),
                    best.taking_up()
                ),
            );
        }
    }
    // One notice for the whole muster: a village that reassigns six hands
    // should not shout six times.
    if turned > 0 {
        notices.write(crate::ui::Notice::new(if turned == 1 {
            "One pair of hands turned to work the village needed".to_string()
        } else {
            format!("{turned} pairs of hands turned to work the village needed")
        }));
    }
}
