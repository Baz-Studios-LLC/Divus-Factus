//! Callings: what a villager does all day, and how the village
//! reassigns its hands when a need goes unanswered.

use bevy::prelude::*;

use super::*;
use crate::creature::genome::{Age, CreatureGenome};
use crate::rng::Rng;
use crate::terrain::{Terrain, WATER_LEVEL};

/// Stone the village likes to have in the pile whether or not a footing
/// is presently waiting on it. Four blocks is one longhouse footing: the
/// difference between breaking ground and building, and standing on the
/// broken ground for a day.
const STONE_RESERVE: f32 = 4.0;

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
    /// Footing to roof: the one pair of hands that can carry a building
    /// the whole way. Mason and carpenter were two trades, and a build
    /// stalled whenever the muster had dealt one and not the other - the
    /// stone sat in the pile while the frame waited for a chisel that was
    /// off cutting wood.
    Builder,
    /// Tills fields, tends crops, brings in harvests.
    Farmer,
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
/// practiced. Grown by doing the work, cycle by cycle; never lost —
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
            Vocation::Builder => "builder",
            Vocation::Farmer => "farmer",
            Vocation::Cook => "cook",
            Vocation::Healer => "healer",
            Vocation::Priest => "priest",
            Vocation::Explorer => "explorer",
            Vocation::Guard => "guard",
        }
    }

    /// The trade dressed for a plaque: the name wearing its capital, the
    /// way the ledger's roster and chips write it. Brett: "I want the
    /// jobs to be written like this: Miner, Fisher, Explorer..."
    pub fn title(self) -> &'static str {
        match self {
            Vocation::Gatherer => "Gatherer",
            Vocation::Fisher => "Fisher",
            Vocation::Hunter => "Hunter",
            Vocation::Miner => "Miner",
            Vocation::Forester => "Forester",
            Vocation::Builder => "Builder",
            Vocation::Farmer => "Farmer",
            Vocation::Cook => "Cook",
            Vocation::Healer => "Healer",
            Vocation::Priest => "Priest",
            Vocation::Explorer => "Explorer",
            Vocation::Guard => "Guard",
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
            Vocation::Builder => "raises buildings",
            Vocation::Farmer => "works the fields",
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
            Vocation::Hunter => "took up the bow",
            Vocation::Miner => "took up the pick",
            Vocation::Forester => "took up the axe",
            Vocation::Builder => "took up the hammer",
            Vocation::Farmer => "took up the plow",
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
        (Vocation::Builder, 1.5),
        (Vocation::Farmer, 1.0),
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
///
/// It musters at the working morning, and again whenever the SHAPE of the
/// want changes - ground broken, a footing finally laid, a field left
/// unworked. A morning's plan held against needs that have since moved is
/// no plan at all: it is how three carpenters who went to the rock for
/// want of a footing stay on the rock all afternoon while the laid
/// footing waits for a hammer.
pub(crate) fn morning_muster(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut mustered: Local<(u32, u64, f64)>,
    mut reckoned: Local<f64>,
    mut notices: MessageWriter<crate::ui::Notice>,
    town: (
        Query<&Building>,
        Query<(&ConstructionSite, &Blueprint)>,
        Query<&Stockpile>,
        Query<&Field>,
        Query<&crate::villager::civic::CivicPriority>,
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
    // What the village is carrying, body and mind: the harm a healer is
    // wanted for, and the memories a guard is wanted for.
    // What the village is carrying: the harm a healer answers, the
    // memories a guard answers, and the belief a priest answers.
    hurt: Query<
        (
            &Vitality,
            Option<&crate::witness::Witnessed>,
            Option<&crate::villager::belief::Faith>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
    courting: Query<&crate::villager::Courting, (With<Villager>, Without<Corpse>)>,
    // WHAT THE VILLAGE HAS BURIED. Fear from memory alone could not answer a
    // wolf: an event only reaches souls inside its carry, tens of meters, and a
    // gatherer is killed a hundred and fifty strides out with nobody there. The
    // one person who held that fear was the person the wolf killed, so a village
    // could bury six and want no guard at all - measured, over fifteen days.
    //
    // A grave needs no witness. Somebody carried the body home and the whole
    // village stood over it, which is the most reliable thing a village knows
    // about the woods.
    graves: Query<&crate::villager::rites::Grave>,
    ground: (
        Option<Res<crate::villager::explore::KnownWorld>>,
        // The tree's own Transform and the chunk it hangs from, NOT its
        // GlobalTransform. See `wood_known`.
        Query<(&Transform, &ChildOf, &crate::scatter::FellableTree)>,
        Query<&Transform, Without<crate::scatter::FellableTree>>,
        Option<Res<Terrain>>,
        Option<Res<SettlementSite>>,
        // What the land offers a food trade: the berries in a gatherer's
        // walk, the game in a hunter's. Counted the same way the famine
        // watch counts them, so the dealer and the coroner agree.
        Query<(&GlobalTransform, &crate::scatter::FoodSource)>,
        Query<
            (&Transform, &CreatureGenome),
            (
                With<crate::creature::Creature>,
                Without<Corpse>,
                Without<crate::creature::Held>,
            ),
        >,
    ),
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
    // Only in working hours - and the reckoning below walks every tree
    // on the map, so it is taken a few times a minute rather than every
    // frame. A want counted in whole hands does not turn faster than
    // that.
    if !clock.work_hours() {
        return;
    }
    if clock.elapsed - *reckoned < 5.0 {
        return;
    }
    *reckoned = clock.elapsed;

    let (buildings, sites, stores, fields, decree) = town;
    let (known, trees, chunks, terrain, site, bushes, game) = ground;
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
    // And its ceiling is what the town's roofs can keep. Brett, at ten
    // thousand food: "when the food reserves get too high people should
    // stop wanting to be hunters, farmers and fishers and gatherers."
    // They wind down from three quarters full rather than stopping dead,
    // so a village eases off the bushes instead of downing baskets on one
    // berry - and the hands freed go to the stone and timber the town is
    // actually short of.
    let ceiling = crate::villager::work::stores::larder_ceiling(
        mouths,
        has(BuildingKind::Storehouse),
        has(BuildingKind::Granary),
        has(BuildingKind::Smokehouse),
    );
    let put_by = (((food / ceiling.max(1.0)) - 0.75) / 0.25).clamp(0.0, 1.0);
    let still_wanted = 1.0 - put_by;

    // Whether any fellable tree stands on ground the village knows. When
    // none does, more foresters are useless - someone has to go find woods.
    //
    // In FLAT coordinates, which is the only space `KnownWorld` speaks: its
    // center is where the flag went in and its pockets are written from
    // villagers' own Transforms. A tree's GLOBAL transform is a point on the
    // bent globe, thousands of units from any of that, so this asked whether a
    // tree on the planet's surface stood within a hundred and seventy meters of
    // a spot in the flat sim - and the answer was always no.
    //
    // Which is why nobody was building. No tree was known, so the forester was
    // struck off the trades a village may take up; with no forester there is no
    // timber, and with no timber there is nothing to raise. Brett: "not sure why
    // nobody is building anything?"
    let wood_known = known.as_ref().is_none_or(|k| {
        trees.iter().any(|(at, parent, tree)| {
            if !tree.harvestable() {
                return false;
            }
            // A tree hangs from its chunk, and the chunk's own Transform is
            // where that chunk stands in the flat world.
            let seated = chunks
                .get(parent.parent())
                .map(|chunk| chunk.translation)
                .unwrap_or(Vec3::ZERO);
            k.knows(seated + at.translation)
        })
    });
    // Whether water lies within a working walk of the square - twelve
    // spokes, no dart-throwing, so no rng is needed here.
    let shore_near = terrain.as_ref().zip(site.as_ref()).is_some_and(|(t, s)| {
        (0..12).any(|i| {
            let angle = i as f32 / 12.0 * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            (1..=14).any(|step| {
                let d = step as f32 * 4.0;
                t.height_at(s.center.x + cos * d, s.center.z + sin * d) <= WATER_LEVEL
            })
        })
    });
    // How badly this village fears the woods, summed over the people who
    // carry a fresh memory of the teeth - seen with their own eyes or
    // told by someone who did.
    //
    // This was a count of live wolves within a hundred and thirty meters
    // of the square: a god's-eye census nobody in the village had access
    // to. It posted guards against wolves no one had ever seen and left
    // a child who limped home torn open to change nothing at all. A
    // village's fear should be made of what it has been through.
    let peril = crate::witness::peril_of(hurt.iter().filter_map(|(_, held, _)| held), clock.day());
    let believers = hurt
        .iter()
        .filter(|(_, _, faith)| faith.is_some_and(|f| f.is_believer()))
        .count();
    // Couples who have courted their four days and have nowhere to be
    // wed. They are the loudest argument a hamlet has for a priest: the
    // shrine is not scenery, it is where the vows are made.
    let betrothed = courting
        .iter()
        .filter(|courting| courting.ripe(clock.day()))
        .count();
    // AN ARMORY IS A STANDING ANSWER, and it changes what the muster is for.
    //
    // A watchtower means somebody watches: one spear, and the fright is
    // handled. An armory means the village decided the thing out there is not
    // going away - so the ceiling comes off, and the spears go on being dealt
    // in proportion to how many souls are still carrying the fear. Brett:
    // "seeing goblins should massively increase the want for soldiers."
    //
    // Still off `peril`, which is counted in PEOPLE and not in enemies, so
    // this rises with how widely the fright has spread rather than with a
    // census of goblins nobody has met.
    // The violent dead of the last fortnight, which is the same window a
    // mauling frightens for - see `witness::PERIL_FADES`. Starvation is not in
    // it: a guard cannot answer an empty larder, and posting one against hunger
    // would take the hands that could have filled it.
    let slain = graves
        .iter()
        .filter(|grave| grave.violent)
        .filter(|grave| {
            clock.day().saturating_sub(grave.day) as f32 <= crate::witness::PERIL_FADES
        })
        .count() as f32;
    let guards = if has(BuildingKind::Armory) {
        (peril / 2.0).ceil().max(2.0).min(6.0)
    } else if has(BuildingKind::Watchtower) {
        1.0
    } else {
        // One spear per three frightened souls, and ONE PER GRAVE. A survivor is
        // enough for a spear; somebody in the ground is worth the same, because
        // a village that has buried one of its own to teeth is a village that
        // knows. Whichever fear is louder decides.
        (peril / 3.0).ceil().max(slain).min(2.0)
    };

    let raising = sites.iter().count() as f32;
    let roofless = homeless.iter().count() as f32;
    let footings_waiting = sites
        .iter()
        .filter(|(cs, plan)| cs.stone_laid < cs.footing_stone(plan.kind))
        .count() as f32;
    // Stone the broken ground still wants, block by block, against what
    // the pile actually holds. A standing reserve of four keeps the NEXT
    // ground broken from starting on an empty pile and losing a day to
    // it - the whole village waits on the footing, and the footing waits
    // on one pick.
    let footing_short: f32 = sites
        .iter()
        .map(|(cs, plan)| (cs.footing_stone(plan.kind) - cs.stone_laid).max(0.0))
        .sum();
    let stone_short = (footing_short.max(STONE_RESERVE) - masonry).max(0.0);
    // Shelter is the loudest want there is. A roofless village puts hands
    // on the frames until it has none to spare - but only where ground is
    // actually broken, since a builder with no site is a builder standing
    // in a field. A hammer is kept ready even so, so somebody is there
    // the moment the planner breaks ground.
    let builders = if raising > 0.0 {
        (raising + footings_waiting)
            .max((roofless / 2.0).ceil())
            // Never more than a third of the village on the frames.
            // Builders eat and do not gather, and a crew that big with
            // nobody left in the woods is how a village starves inside
            // its own full larder.
            .min((mouths as f32 / 3.0).floor().max(1.0))
    } else if roofless > 2.0 {
        1.0
    } else {
        0.0
    };
    let unworked_fields = fields
        .iter()
        .filter(|f| f.farmer == Entity::PLACEHOLDER)
        .count() as f32;

    // Which food trades the land can actually pay. The dealer used to
    // weight them blind - and at a founding deep in the woods, with the
    // closest berries seven hundred strides out, it kept handing out
    // gathering baskets while two spear-hands stood beside prey nineteen
    // strides from the square. Ten founders fell to six before the
    // ledger said why. A trade the land cannot pay gets no hands, and
    // its share flows to the trades that can.
    let center = site.as_ref().map(|s| s.center);
    let berries_near = center.is_none_or(|c| {
        bushes
            .iter()
            .any(|(at, bush)| bush.amount > 0.3 && at.translation().distance(c) < WORK_REACH)
    });
    let game_near = center.is_none_or(|c| {
        game.iter().any(|(at, genome)| {
            matches!(
                genome.species,
                crate::creature::genome::Species::Deer | crate::creature::genome::Species::Boar
            ) && at.translation.distance(c) < WORK_REACH * 1.6
        })
    });
    let mut gather_share = if berries_near { 0.45 } else { 0.0 };
    let mut fish_share = if shore_near { 0.30 } else { 0.0 };
    let mut hunt_share = if game_near { 0.25 } else { 0.0 };
    let offered = gather_share + fish_share + hunt_share;
    if offered > 0.0 {
        gather_share /= offered;
        fish_share /= offered;
        hunt_share /= offered;
    }
    // Whether the land feeds them at all without being farmed. A village
    // can hunt a wood empty in a few days - the herd went from forty
    // head to two - so this is a fact that CHANGES, and the muster's
    // fingerprint below carries it so the village takes stock the day
    // the last deer is gone.
    let wild_food = berries_near || shore_near || game_near;
    let plots = fields.iter().count() as f32;
    // Enough is enough in the other sheds too. Curbing the larder alone
    // just moved the absurdity one door over: with the food trades wound
    // down, every spare hand went to the woods and the rock instead, and
    // a soak ended on four and a half thousand stone. What a town can
    // use is bounded by the number of mouths in it, and a village that
    // has everything it needs sends its spare hands down the road to
    // find out what else is out there.
    let timber_deep = timber >= mouths as f32 * 25.0;
    let stone_deep = masonry >= mouths as f32 * 20.0;

    // What the village wants, counted in hands. A want of zero is a trade
    // nobody takes up - which is the whole point: a farmer is somebody who
    // has a field, not somebody who once rolled a plow.
    let food_hands = mouths as f32 * (0.2 + 0.4 * hungry) * still_wanted;
    let mut wanted: Vec<(Vocation, f32)> = vec![
        (Vocation::Gatherer, food_hands * gather_share),
        (Vocation::Fisher, food_hands * fish_share),
        (Vocation::Hunter, food_hands * hunt_share),
        // A plot wants a pair of hands whether or not somebody has
        // already claimed it. Counting only the UNCLAIMED plots meant
        // the want fell to nothing the moment a farmer took a furrow,
        // the next muster struck them off it, and the field went back to
        // weeds - farming in five-minute shifts.
        //
        // And where the land offers nothing wild - no berries in a
        // walk, no shore, no game left - the want is for the FIRST
        // furrow, before any field exists. Without that a village in the
        // deep woods had no path to food at all: the basket and the
        // spear were dealt against a land that could not pay them, and
        // nobody could ever be dealt the plow, because the plow was
        // wanted only where a field already stood. Ten souls starved to
        // six in a forest, holding tools for work that was not there.
        (
            Vocation::Farmer,
            plots.min(4.0).max(if wild_food { 0.0 } else { 2.0 }) * still_wanted,
        ),
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
        // Hands on the sites: one per build, and one more for every
        // footing still wanting stone, because a builder with no course
        // to lay cuts rock and a builder with rock in the pile lays it.
        // These were two trades and two wants, and a build stalled every
        // time the muster dealt a carpenter and no mason.
        (Vocation::Builder, (raising + footings_waiting).min(4.0)),
        // One pick per two blocks the pile is short. This was a flat ONE
        // miner, and only while the pile held under four - so a village
        // with a single scrap of stone mustered nobody to the rock and
        // then stood a whole day waiting on a footing that wanted four.
        (Vocation::Miner, (stone_short * 0.5).ceil().min(3.0)),
        (
            Vocation::Cook,
            if has(BuildingKind::Tavern) { 1.0 } else { 0.0 },
        ),
        // Belief calls the priest, not the building. A priest was wanted
        // only where a shrine already stood, and the shrine was behind a
        // civic ladder that wants twelve souls and everyone housed - so
        // neither ever happened, and a village of believers had nobody to
        // keep its god. Whoever takes it up raises the shrine themselves;
        // that is what OWN_WORKS is for.
        (
            Vocation::Priest,
            if has(BuildingKind::Shrine) || believers * 3 >= mouths || betrothed > 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            Vocation::Healer,
            if hurt.iter().filter(|(v, ..)| v.harm > 0.15).count() >= 2 {
                1.0
            } else {
                0.0
            },
        ),
        (Vocation::Guard, guards),
    ];
    // The mayor's decree leans the muster: the standing priority raises
    // its trades' wants and touches nothing else, so a chain in a hurry
    // concentrates hands without ever starving the other work.
    if let Some(priority) = decree.iter().next() {
        for (vocation, want) in wanted.iter_mut() {
            *want *= crate::villager::civic::lean_scale(priority.lean, *vocation);
        }
    }
    // A roofless village needs a hammer whether or not ground is broken:
    // somebody has to be standing ready when it is.
    if homeless.iter().count() > 2 {
        if let Some(entry) = wanted.iter_mut().find(|(v, _)| *v == Vocation::Builder) {
            entry.1 = entry.1.max(1.0);
        }
    }

    // The SHAPE of the want, in whole hands - and only the parts of it
    // worth putting a tool down for. The food trades are deliberately
    // absent: the larder rises and falls all day, and a gatherer is not
    // made a mason by one good afternoon of berries. Counting them here
    // turned two hands every five seconds and nobody got good at
    // anything.
    let shape = [
        raising,
        footings_waiting,
        unworked_fields,
        (stone_short * 0.5).ceil(),
        builders,
        betrothed.min(3) as f32,
        wood_known as u8 as f32,
        shore_near as u8 as f32,
        // What the land can still pay. The day the last deer within a
        // hunter's walk is gone, the village must take stock again -
        // otherwise the muster keeps dealing spears at an empty wood
        // until the morning.
        berries_near as u8 as f32,
        game_near as u8 as f32,
        plots,
        // A larder crossing its ceiling changes every food trade's want,
        // so the muster has to notice the day it happens - and the day a
        // new granary raises the ceiling back over it. The other sheds
        // filling up moves hands the same way.
        (put_by * 8.0) as u64 as f32,
        timber_deep as u8 as f32,
        stone_deep as u8 as f32,
        guards,
        hurt.iter().filter(|(v, ..)| v.harm > 0.15).count().min(3) as f32,
        has(BuildingKind::Tavern) as u8 as f32,
        has(BuildingKind::Shrine) as u8 as f32,
        has(BuildingKind::Watchtower) as u8 as f32,
    ]
    .iter()
    .fold(0u64, |acc, n| {
        acc.wrapping_mul(1_000_003).wrapping_add(n.max(0.0) as u64)
    });
    // The morning's deal always happens; an in-day one waits on both a
    // changed shape and a cooling-off, so the village takes stock a few
    // times a day rather than a few times a minute.
    let (last_day, last_shape, last_at) = *mustered;
    let morning = last_day != clock.day();
    if !morning && (last_shape == shape || clock.elapsed - last_at < 60.0) {
        return;
    }
    *mustered = (clock.day(), shape, clock.elapsed);

    // The work that always exists, for hands the wants do not reach. Every
    // one of these has something to do on the first morning of the world.
    // A basket is no work where nothing fruits and a spear is no work
    // where nothing grazes: spare hands used to be dealt both anyway,
    // and spent their days walking to a heath that had been picked bare
    // a week ago. The plow is the other way about - it is standing
    // work only where the wild will NOT feed them, since a village
    // beside a full berry patch should pick before it plows.
    let standing: Vec<Vocation> = [
        Vocation::Gatherer,
        Vocation::Forester,
        Vocation::Hunter,
        Vocation::Miner,
        Vocation::Fisher,
        Vocation::Farmer,
    ]
    .into_iter()
    .filter(|v| *v != Vocation::Fisher || shore_near)
    .filter(|v| *v != Vocation::Forester || (wood_known && !timber_deep))
    .filter(|v| *v != Vocation::Miner || !stone_deep)
    .filter(|v| *v != Vocation::Gatherer || berries_near)
    .filter(|v| *v != Vocation::Hunter || game_near)
    .filter(|v| *v != Vocation::Farmer || !wild_food)
    // And none of the food trades at all once the sacks are full: a
    // spare pair of hands at a full larder belongs in the woods or at
    // the rock, which is what the town is actually short of.
    .filter(|v| {
        still_wanted > 0.0
            || !matches!(
                v,
                Vocation::Gatherer | Vocation::Hunter | Vocation::Fisher | Vocation::Farmer
            )
    })
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
                    // Nothing standing left to do. A town with full
                    // sheds and nothing to raise sends its spare hands
                    // down the road - which is how the next town gets
                    // found. Otherwise: turn the ground over where the
                    // wild will not feed them, and pick where it will.
                    .or(Some(if still_wanted <= 0.0 && timber_deep && stone_deep {
                        Vocation::Explorer
                    } else if still_wanted <= 0.0 {
                        Vocation::Forester
                    } else if wild_food {
                        Vocation::Gatherer
                    } else {
                        Vocation::Farmer
                    }))
            });
        let Some(best) = best else { continue };
        if let Some(entry) = wanted.iter_mut().find(|(v, _)| *v == best) {
            entry.1 -= 1.0;
        }
        // The muster is the new plan, so the stopgap's patience starts
        // over with it: yesterday's dry spell must not overrule this
        // morning's deal on its very first frame.
        commands.entity(hand).remove::<Jobless>();
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
