//! The known world, and the people who push its edge.
//!
//! Ordinary villagers live inside a boundary of knowledge: everything the
//! village has stood next to and come home from. The god sees the whole
//! world; the people see this circle — and they mark its edge themselves,
//! with cairns of stacked waystones, so the fog's border is a built thing
//! in the world and not a line on a map.
//!
//! Explorers are the ones who walk past the cairns. An expedition goes out
//! alone (guards come in the next milestone), stands in the unknown long
//! enough to read the land, and comes home with a discovery: a grove, an
//! ore slope, a berry heath, or nothing but wind. What they find becomes a
//! known pocket the working trades can use — and a tale that runs through
//! the gossip mill like any miracle.

use bevy::prelude::*;

use crate::creature::{Airborne, Corpse, Held, MoveTarget};
use crate::scatter::FellableTree;
use crate::terrain::{Terrain, WATER_LEVEL};

use super::{Activity, Chronicle, Needs, Person, SettlementSite, Villager, work};

/// How far past the cairns an expedition pushes.
const STRIDE: f32 = 70.0;

/// One place the village knows beyond its home circle.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Pocket {
    pub at: Vec3,
    pub radius: f32,
}

/// Everything the village knows of the world: the home circle, and the
/// pockets its explorers have brought back.
#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct KnownWorld {
    pub centre: Vec3,
    pub radius: f32,
    pub pockets: Vec<Pocket>,
}

impl Default for KnownWorld {
    fn default() -> Self {
        KnownWorld {
            centre: Vec3::ZERO,
            radius: 170.0,
            pockets: Vec::new(),
        }
    }
}

impl KnownWorld {
    /// Whether the village knows this ground.
    pub fn knows(&self, at: Vec3) -> bool {
        if at.distance(self.centre) < self.radius {
            return true;
        }
        self.pockets
            .iter()
            .any(|pocket| at.distance(pocket.at) < pocket.radius)
    }

    /// Learns a patch of ground, keeping the map inside the veil shader's
    /// budget by tidying rather than by forgetting.
    ///
    /// The old rule was a silent cap: at 128 pockets the map simply
    /// stopped recording, and everything walked afterwards stayed under
    /// fog forever — Brett found a black hole of unknown sitting over a
    /// quarry his miners worked every day. Ground once walked is NEVER
    /// given back to the dark: when the list runs full, the two closest
    /// pockets merge into the circle that holds them both.
    pub fn learn(&mut self, at: Vec3, radius: f32) {
        self.pockets.push(Pocket { at, radius });
        self.tidy();
    }

    fn tidy(&mut self) {
        // Pockets the home circle has grown over are redundant.
        let (centre, reach) = (self.centre, self.radius);
        self.pockets
            .retain(|pocket| pocket.at.distance(centre) + pocket.radius > reach);
        // Pockets lying wholly inside a bigger sibling likewise.
        let mut index = 0;
        while index < self.pockets.len() {
            let inner = &self.pockets[index];
            let swallowed = self.pockets.iter().enumerate().any(|(other, outer)| {
                other != index
                    && outer.radius >= inner.radius
                    && inner.at.distance(outer.at) + inner.radius <= outer.radius
                    // Ties (identical twins) keep the first and drop the rest.
                    && (outer.radius > inner.radius || other < index)
            });
            if swallowed {
                self.pockets.swap_remove(index);
            } else {
                index += 1;
            }
        }
        // Still full: merge the two closest pockets into the circle that
        // holds them both. Slivers of unknown between near neighbours get
        // claimed in the bargain, which is the honest price of a bounded
        // map that never un-knows a walked road.
        while self.pockets.len() > MOST_POCKETS {
            let mut nearest = (0usize, 1usize, f32::MAX);
            for a in 0..self.pockets.len() {
                for b in (a + 1)..self.pockets.len() {
                    let gap = self.pockets[a].at.distance(self.pockets[b].at)
                        - self.pockets[a].radius
                        - self.pockets[b].radius;
                    if gap < nearest.2 {
                        nearest = (a, b, gap);
                    }
                }
            }
            let (a, b, _) = nearest;
            let merged = {
                let (first, second) = (&self.pockets[a], &self.pockets[b]);
                let between = second.at - first.at;
                let span = between.length();
                let radius = ((span + first.radius + second.radius) * 0.5)
                    .max(first.radius.max(second.radius));
                let at = first.at + between.normalize_or_zero() * (radius - first.radius);
                Pocket { at, radius }
            };
            // `b` is the larger index, so removing it leaves `a` in place.
            self.pockets.swap_remove(b);
            self.pockets[a] = merged;
        }
    }
}

/// A waystone stack marking the edge of the known world.
#[derive(Component)]
pub struct Cairn;

/// How far from the banner counts as being away, where the stores-only
/// law lifts. Past the town's own ground and most of the way to the
/// working reach: a gatherer in the near fields walks home to eat, and
/// somebody three hundred strides out does not.
const AWAY_FROM_HOME: f32 = 110.0;

/// Anybody far from home eats what the land gives them.
///
/// The stores-only law - Brett: "They should not eat from bushes, they
/// should eat from the stores" - ends at the cairns, and until now only
/// expeditions and colony parties knew that. An ordinary forester who
/// ranged three hundred strides after the last good timber was held to
/// the town's table and died walking back to it: "Sperfiko starved on
/// the road, 275 strides from a larder that held food."
///
/// Brett: "they should be able to eat from bushes and if they hunt
/// animals while they are away." So: the satchel first, then a beast
/// already down, then the heath. A kill is a meal for whoever is
/// standing over it hungry - the hunter's harvest can have what is left.
#[allow(clippy::type_complexity)]
pub(super) fn the_road_feeds_who_walks_it(
    time: Res<Time>,
    mut commands: Commands,
    grounds: Query<&crate::villager::SettlementGround>,
    members: Query<&crate::villager::MemberOf>,
    mut walkers: Query<
        (
            Entity,
            &Transform,
            &mut Needs,
            &mut MoveTarget,
            Option<&mut work::Rations>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<crate::creature::Held>,
            Without<Airborne>,
            Without<Expedition>,
            Without<crate::villager::colony::Colonist>,
        ),
    >,
    mut bushes: Query<(Entity, &GlobalTransform, &mut crate::scatter::FoodSource)>,
    fallen: Query<
        (Entity, &Transform),
        (
            With<Corpse>,
            With<crate::creature::Creature>,
            Without<Villager>,
        ),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: the_road_feeds_who_walks_it");
    let dt = time.delta_secs();
    for (who, at, mut needs, mut target, mut rations) in &mut walkers {
        if needs.hunger < 0.55 {
            continue;
        }
        // THEIR town's banner, not whichever one the query hands back
        // first: a colonist's road is measured from the town they left.
        let home = members
            .get(who)
            .ok()
            .and_then(|member| grounds.get(member.0).ok())
            .map(|ground| ground.centre);
        if home.is_none_or(|centre| at.translation.distance(centre) < AWAY_FROM_HOME) {
            continue;
        }
        // A beast already down, close enough to be worth the walk.
        let carcass = fallen
            .iter()
            .map(|(beast, seat)| (beast, seat.translation))
            .filter(|(_, spot)| spot.distance(at.translation) < 30.0)
            .min_by(|a, b| {
                a.1.distance(at.translation)
                    .total_cmp(&b.1.distance(at.translation))
            });
        if let Some((beast, spot)) = carcass {
            if spot.distance(at.translation) > 2.6 {
                target.0 = Some(spot);
            } else {
                needs.hunger = (needs.hunger - 0.7).max(0.0);
                if let Some(rations) = rations.as_deref_mut() {
                    rations.0 = (rations.0 + 1.0).min(3.0);
                }
                commands.entity(beast).despawn();
            }
            continue;
        }
        if let Some(meal) = forage_tick(
            at.translation,
            dt,
            &mut needs,
            rations.as_deref_mut(),
            &mut bushes,
        ) {
            target.0 = Some(meal);
        }
    }
}

/// An expedition in progress.
#[derive(Component)]
pub struct Expedition {
    target: Vec3,
    surveying: f32,
    homeward: bool,
}

/// A guard walking with someone whose road runs past the cairns.
/// Company is armour: the wolves do not test a pair.
#[derive(Component)]
pub struct Escorting {
    pub ward: Entity,
}

/// A standing order from a full town: find ground fit for a new village.
///
/// Raised by the colony muster when the doors are open but the known world
/// holds no legal site; lowered the moment a road prayer goes up. While it
/// stands, expeditions push out to colony distances and survey the ground
/// they stand on as town-founders, not foragers.
#[derive(Resource, Default)]
pub struct GroundWanted(pub bool);

/// One tick of eating on the road: the satchel first, the land second.
///
/// Brett: "when people are on the road (colonizers and explorers etc.)
/// maybe we can let them eat from bushes and craft rations?" — so the
/// stores-only law ends at the cairns. A wayfarer bites from packed
/// rations while any remain, and past that eats straight off the heath,
/// packing a little of every picking back into the satchel for the miles
/// with no green on them.
///
/// Returns a place to walk when the next meal is a bush away, `None` when
/// the walker is fed enough to keep to their road.
pub(super) fn forage_tick(
    at: Vec3,
    dt: f32,
    needs: &mut Needs,
    mut rations: Option<&mut work::Rations>,
    bushes: &mut Query<(Entity, &GlobalTransform, &mut crate::scatter::FoodSource)>,
) -> Option<Vec3> {
    if needs.hunger < 0.55 {
        return None;
    }
    // The satchel: packed at the sacks, eaten on the move.
    if let Some(rations) = &mut rations
        && rations.0 >= 1.0
    {
        rations.0 -= 1.0;
        needs.hunger = (needs.hunger - 0.6).max(0.0);
        return None;
    }
    // The land: the nearest bush still wearing fruit. Bushes are chunk
    // children, so their globals are BENT — unbend before any distance.
    let nearest = bushes
        .iter()
        .filter(|(_, _, bush)| bush.amount > 0.3)
        .map(|(bush, seat, _)| (bush, crate::globe::unbend(seat.translation())))
        .filter(|(_, spot)| spot.distance(at) < 45.0)
        .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)));
    let (bush, spot) = nearest?;
    if spot.distance(at) > 2.4 {
        return Some(spot);
    }
    // Standing at the bush: eat, and pack a little for the road ahead.
    if let Ok((_, _, mut source)) = bushes.get_mut(bush) {
        let bite = (0.9 * dt).min(source.amount);
        source.amount -= bite;
        needs.hunger = (needs.hunger - bite * 0.9).max(0.0);
        if let Some(rations) = &mut rations {
            rations.0 = (rations.0 + bite * 0.4).min(2.0);
        }
    }
    Some(spot)
}

/// Idle explorers walk out past the cairns, read the land, and come home
/// with what they found.
#[allow(clippy::type_complexity)]
/// How far a pair of feet can see: a villager standing somewhere knows
/// that place and a little around it.
const FOOTFALL: f32 = 34.0;
/// The grid those footfalls are rounded onto, so ten thousand steps
/// across the same meadow leave ONE pocket rather than ten thousand. The
/// cell's diagonal is twice the reach, so neighbouring cells just touch
/// and a walked road opens as a continuous corridor.
const FOOTFALL_GRID: f32 = 48.0;
/// As many pockets as the veil's shader can be handed at once.
const MOST_POCKETS: usize = 128;

/// Ground is known because somebody stood on it.
///
/// Expeditions still do the real work - they push past the cairns on
/// purpose and bring back whole regions - but a gatherer who walks half
/// a mile after a berry bush has, in the plainest sense, been there. The
/// map used to disagree, and the fog sat over a man standing in it.
pub(super) fn walk_the_world(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut known: Option<ResMut<KnownWorld>>,
    walkers: Query<
        &Transform,
        (
            With<crate::villager::Villager>,
            Without<crate::creature::Corpse>,
        ),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: walk_the_world");
    // Feet are slow. Once a second is finer than anyone can walk out of.
    *since_last += time.delta_secs();
    if *since_last < 1.0 {
        return;
    }
    *since_last = 0.0;
    let Some(known) = known.as_mut() else {
        return;
    };
    for at in &walkers {
        if known.knows(at.translation) {
            continue;
        }
        // Rounded onto the grid, so the same meadow is opened once.
        let cell = Vec3::new(
            (at.translation.x / FOOTFALL_GRID).round() * FOOTFALL_GRID,
            at.translation.y,
            (at.translation.z / FOOTFALL_GRID).round() * FOOTFALL_GRID,
        );
        if known
            .pockets
            .iter()
            .any(|pocket| pocket.at.distance(cell) < 1.0)
        {
            continue;
        }
        // Never capped: the map tidies itself instead of going blind.
        known.learn(cell, FOOTFALL);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn expeditions(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Option<Res<Terrain>>,
    site: Option<Res<SettlementSite>>,
    homes: (
        Query<&super::MemberOf>,
        Query<&super::SettlementGround>,
        Option<Res<GroundWanted>>,
        Res<crate::debug::timings::Timings>,
    ),
    // Bundled with a spare slot's worth of company: this system sits at
    // Bevy's parameter ceiling.
    mut known: ResMut<KnownWorld>,
    weather: Option<Res<crate::weather::Weather>>,
    mut rng: ResMut<super::SimRng>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut say: (
        Option<ResMut<crate::sermo::Tongue>>,
        Option<Res<crate::attention::Attention>>,
    ),
    mut stores: Query<&mut crate::villager::work::Stockpile>,
    trees: Query<(&GlobalTransform, &FellableTree)>,
    mut bushes: Query<(Entity, &GlobalTransform, &mut crate::scatter::FoodSource)>,
    deposits: Query<(&GlobalTransform, &crate::matter::Deposit)>,
    mut explorers: Query<
        (
            Entity,
            &Transform,
            &work::Vocation,
            &Person,
            &mut Activity,
            &mut MoveTarget,
            Option<&mut Expedition>,
            Option<&mut Chronicle>,
            &mut Needs,
            Option<&mut work::Rations>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
        ),
    >,
) {
    let (members, grounds, wanted, watch) = homes;
    let _t = watch.watch("villager: expeditions");
    let (Some(terrain), Some(site)) = (terrain, site) else {
        return;
    };
    let wanted = wanted.is_some_and(|order| order.0);
    let dt = time.delta_secs();

    // What the village wants and cannot reach is what sends people out.
    // Scarcity is the engine of the map: a full woodpile keeps everyone
    // home, an empty one with no known tree left to fell puts someone on
    // the road. The two wants that kill are timber and food.
    let timber_short = stores.iter().all(|s| s.timber < 8.0) || stores.iter().next().is_none();
    // Trees and bushes are chunk children: their globals are BENT, and any
    // distance or knowledge test must unbend them first or the map lies
    // once the village is a few hundred strides from the origin.
    let wood_known = trees.iter().any(|(at, tree)| {
        tree.harvestable() && known.knows(crate::globe::unbend(at.translation()))
    });
    let wood_want = timber_short && !wood_known;
    let food_short = stores.iter().all(|s| s.food() < 10.0) || stores.iter().next().is_none();
    let berries_known = bushes.iter().any(|(_, at, bush)| {
        bush.amount > 0.5 && known.knows(crate::globe::unbend(at.translation()))
    });
    let food_want = food_short && !berries_known;
    // Hungry villages muster expeditions in earnest, and a town that wants
    // colony ground likewise; content ones only when wanderlust strikes.
    let urgency = if wood_want || food_want || wanted {
        0.02
    } else {
        0.002
    };

    // Idle guards, ready to fall in beside whoever sets out.
    let mut guard_pool: Vec<(Entity, Vec3)> = explorers
        .iter()
        .filter(|(_, _, vocation, _, activity, ..)| {
            **vocation == work::Vocation::Guard
                && matches!(**activity, Activity::Idle | Activity::Wandering)
        })
        .map(|(guard, at, ..)| (guard, at.translation))
        .collect();

    for (
        entity,
        at,
        vocation,
        person,
        mut activity,
        mut target,
        expedition,
        chronicle,
        mut needs,
        mut rations,
    ) in &mut explorers
    {
        if *vocation != work::Vocation::Explorer {
            continue;
        }

        // An expedition underway runs to its end, whatever the hour.
        if let Some(mut expedition) = expedition {
            if *activity != Activity::Working {
                // Sleep broke the journey; they will set out again.
                commands.entity(entity).remove::<Expedition>();
                continue;
            }
            // The road feeds itself: the satchel, then the heath. A meal
            // pauses the survey and the miles both — the road waits.
            if let Some(meal) = forage_tick(
                at.translation,
                dt,
                &mut needs,
                rations.as_deref_mut(),
                &mut bushes,
            ) {
                target.0 = Some(meal);
                continue;
            }
            // The road ends when the bread does. Hungry, satchel dry, and
            // no green in reach: the only meal left is the larder at home,
            // so turn for it NOW, while the walk back is still shorter
            // than the hunger. Kileb starved 172 strides out with the
            // town's sacks full — nothing on the old road ever turned him
            // around.
            if needs.hunger > 0.7
                && !expedition.homeward
                && rations.as_ref().is_none_or(|satchel| satchel.0 < 1.0)
            {
                expedition.homeward = true;
                info!("{} turns for home hungry", person.name);
            }
            if expedition.homeward {
                let square = members
                    .get(entity)
                    .ok()
                    .and_then(|member| grounds.get(member.0).ok())
                    .map_or(site.centre, |ground| ground.centre);
                if at.translation.distance(square) > 6.0 {
                    target.0 = Some(square);
                } else {
                    commands.entity(entity).remove::<Expedition>();
                    *activity = Activity::Idle;
                    target.0 = None;
                }
                continue;
            }
            if at.translation.distance(expedition.target) > 3.0 {
                target.0 = Some(expedition.target);
                continue;
            }
            // Standing in the unknown, reading the land.
            target.0 = None;
            expedition.surveying += dt;
            if expedition.surveying < 6.0 {
                continue;
            }

            // What is actually here decides what is found.
            let spot = expedition.target;
            let near_trees = trees
                .iter()
                .filter(|(t, _)| crate::globe::unbend(t.translation()).distance(spot) < 45.0)
                .count();
            let near_bushes = bushes
                .iter()
                .filter(|(_, b, _)| crate::globe::unbend(b.translation()).distance(spot) < 45.0)
                .count();
            let high_ground = terrain.height_at(spot.x, spot.z) > WATER_LEVEL + 12.0;
            // A deposit within sight of the survey is the find of a
            // lifetime: rarer than any wood, and named accordingly.
            let near_deposit = deposits
                .iter()
                .filter(|(at, deposit)| {
                    deposit.amount > 0.5
                        && crate::globe::unbend(at.translation()).distance(spot) < 45.0
                })
                .map(|(_, deposit)| deposit.kind)
                .next();
            // Ground fit for a town is the find the whole village is
            // waiting on, when the muster has raised the order for it.
            let town_centres: Vec<Vec3> = grounds.iter().map(|g| g.centre).collect();
            let good_town_ground = wanted
                && super::colony::clear_of_towns(spot, &town_centres)
                && super::score_town_ground(&terrain, spot.x, spot.z, 0.7).is_some();
            let (what, radius) = if good_town_ground {
                ("a wide vale fit for a new village", 60.0)
            } else if let Some(kind) = near_deposit {
                match kind {
                    crate::matter::DepositKind::Iron => ("a hillside veined with iron", 45.0),
                    crate::matter::DepositKind::Clay => ("a bank of good red clay", 40.0),
                    crate::matter::DepositKind::Stone => ("a face of good building stone", 50.0),
                }
            } else if near_trees >= 6 {
                ("a green wood past the cairns", 55.0)
            } else if near_bushes >= 3 {
                ("a heath heavy with berries", 45.0)
            } else if high_ground {
                ("a slope of good bare stone", 45.0)
            } else {
                ("nothing but wind out there", 0.0)
            };

            if radius > 0.0 {
                known.learn(spot, radius);
                notices.write(crate::ui::Notice::fanfare(format!(
                    "{} found {}",
                    person.name, what
                )));
            }
            // Every return stretches the cairn ring a little: even a walk
            // that found nothing proves the ground between.
            known.radius += 9.0;
            // The homecoming is told in the explorer's own words - tagged
            // `returned`, so the want-list demands homecoming lines until
            // the corpus holds them.
            if let Some(tongue) = say.0.as_mut() {
                tongue.muse(crate::sermo::Musing {
                    who: entity,
                    voice: Some(crate::villager::work::Vocation::Explorer),
                    faith: crate::sermo::FaithBand::Wavering,
                    body: vec!["returned"],
                    heard: None,
                    aloud: true,
                    about: None,
                });
            }
            if let Some(mut chronicle) = chronicle {
                chronicle.record(
                    clock.day(),
                    format!("walked past the cairns and found {what}"),
                );
            }
            info!("{} explored and found {}", person.name, what);
            expedition.homeward = true;
            continue;
        }

        // A fresh expedition musters only in working hours, from the idle.
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        if !work::is_work_hour(clock.time_of_day()) || !rng.0.chance(urgency) {
            continue;
        }
        // Nobody walks past the cairns into a storm.
        if weather
            .as_ref()
            .is_some_and(|w| w.kind() == crate::weather::WeatherKind::Storm)
        {
            continue;
        }
        // A want the village cannot meet aims the walk: a forest shows on
        // the horizon long before anyone has stood under it, so a village
        // short of wood heads for the nearest green it can see and does
        // not yet know — likewise berries. Only the contented wander at
        // random.
        let mut found = None;
        if wood_want {
            found = trees
                .iter()
                .map(|(at, tree)| (crate::globe::unbend(at.translation()), tree))
                .filter(|(at, tree)| tree.harvestable() && !known.knows(*at))
                .map(|(at, _)| at)
                .min_by(|a, b| {
                    a.distance(known.centre)
                        .total_cmp(&b.distance(known.centre))
                })
                .map(|at| Vec3::new(at.x, terrain.height_at(at.x, at.z), at.z));
        }
        if found.is_none() && food_want {
            found = bushes
                .iter()
                .map(|(_, at, bush)| (crate::globe::unbend(at.translation()), bush))
                .filter(|(at, bush)| bush.amount > 0.5 && !known.knows(*at))
                .map(|(at, _)| at)
                .min_by(|a, b| {
                    a.distance(known.centre)
                        .total_cmp(&b.distance(known.centre))
                })
                .map(|at| Vec3::new(at.x, terrain.height_at(at.x, at.z), at.z));
        }
        // Otherwise, a frontier point: out past the edge, on walkable ground.
        for _ in 0..24 {
            if found.is_some() {
                break;
            }
            let angle = rng.0.range(0.0, std::f32::consts::TAU);
            // A town hunting colony ground pushes the frontier at double
            // stride: the muster is waiting on somewhere to point at.
            let reach = if wanted {
                known.radius + rng.0.range(STRIDE * 0.8, STRIDE * 1.8)
            } else {
                known.radius + rng.0.range(STRIDE * 0.4, STRIDE)
            };
            let (sin, cos) = angle.sin_cos();
            let x = known.centre.x + cos * reach;
            let z = known.centre.z + sin * reach;
            if terrain.is_walkable(x, z) && terrain.height_at(x, z) > WATER_LEVEL + 1.5 {
                found = Some(Vec3::new(x, terrain.height_at(x, z), z));
                break;
            }
        }
        let Some(frontier) = found else {
            continue;
        };
        *activity = Activity::Working;
        commands.entity(entity).insert(Expedition {
            target: frontier,
            surveying: 0.0,
            homeward: false,
        });
        target.0 = Some(frontier);
        // Packed at the sacks before the walk: bread first, whatever the
        // larder runs deepest in after. A thin larder sends them out lean —
        // the heath feeds the road now.
        if let Ok(member) = members.get(entity)
            && let Ok(mut store) = stores.get_mut(member.0)
            && store.food() >= 6.0
        {
            store.larder.draw(2.0);
            commands.entity(entity).insert(work::Rations(2.0));
        }
        // A guard falls in if one is free: nobody should walk past the
        // cairns alone while the village can spare a spear.
        if let Some(index) = guard_pool
            .iter()
            .enumerate()
            .filter(|(_, (_, spot))| spot.distance(at.translation) < 60.0)
            .min_by(|a, b| {
                a.1.1
                    .distance(at.translation)
                    .total_cmp(&b.1.1.distance(at.translation))
            })
            .map(|(index, _)| index)
        {
            let (guard, _) = guard_pool.swap_remove(index);
            commands
                .entity(guard)
                .insert((Escorting { ward: entity }, Activity::Working));
            // The guard packs a satchel too, if the sacks can spare it.
            if let Ok(member) = members.get(guard)
                && let Ok(mut store) = stores.get_mut(member.0)
                && store.food() >= 6.0
            {
                store.larder.draw(2.0);
                commands.entity(guard).insert(work::Rations(2.0));
            }
            info!("{} walks out with an escort", person.name);
        }
    }
}

/// The escort's whole job: stay at the ward's shoulder until the road
/// brings them both home, then stand down.
#[allow(clippy::type_complexity)]
pub(super) fn escort_duty(
    mut commands: Commands,
    wards: Query<(&Transform, Has<Expedition>), With<Villager>>,
    mut escorts: Query<
        (
            Entity,
            &Escorting,
            &mut MoveTarget,
            &mut Activity,
            &mut Needs,
            Option<&mut work::Rations>,
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
) {
    for (guard, escorting, mut target, mut activity, mut needs, rations) in &mut escorts {
        // The guard eats from the satchel on the move, and never leaves
        // the ward's side for a bush — the ward's own detours bring the
        // pair to the green together anyway.
        if needs.hunger > 0.55
            && let Some(mut rations) = rations
            && rations.0 >= 1.0
        {
            rations.0 -= 1.0;
            needs.hunger = (needs.hunger - 0.6).max(0.0);
        }
        let stand_down = match wards.get(escorting.ward) {
            Ok((_, on_expedition)) => !on_expedition,
            Err(_) => true,
        };
        if stand_down {
            commands.entity(guard).remove::<Escorting>();
            *activity = Activity::Idle;
            target.0 = None;
            continue;
        }
        if let Ok((ward_at, _)) = wards.get(escorting.ward) {
            target.0 = Some(ward_at.translation - Vec3::new(1.4, 0.0, 1.1));
        }
    }
}

/// Raises the waystone cairns along the edge of the known world, and again
/// each time the edge moves.
pub(super) fn raise_cairns(
    mut commands: Commands,
    known: Res<KnownWorld>,
    terrain: Option<Res<Terrain>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<Cairn>>,
) {
    if !known.is_changed() {
        return;
    }
    let Some(terrain) = terrain else {
        return;
    };
    for cairn in &old {
        commands.entity(cairn).despawn();
    }

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let stone = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::STONE, 0.6),
        perceptual_roughness: 1.0,
        ..default()
    });

    let count = ((std::f32::consts::TAU * known.radius) / 36.0)
        .floor()
        .max(8.0) as u32;
    for i in 0..count {
        let angle = i as f32 / count as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let x = known.centre.x + cos * known.radius;
        let z = known.centre.z + sin * known.radius;
        if !terrain.is_walkable(x, z) || terrain.height_at(x, z) < WATER_LEVEL + 1.0 {
            continue;
        }
        let at = Vec3::new(x, terrain.height_at(x, z), z);
        let cairn = commands
            .spawn((
                Cairn,
                Name::new("A waystone cairn"),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(angle)),
                Visibility::default(),
                crate::hand::Rooted,
            ))
            .id();
        // Three stones, biggest at the bottom, each turned a little.
        for (level, (size, yaw)) in [(0.5_f32, 0.0_f32), (0.36, 0.5), (0.24, 1.1)]
            .into_iter()
            .enumerate()
        {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(0.0, 0.18 + level as f32 * 0.3, 0.0)
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::new(size, 0.3, size * 0.9)),
                ChildOf(cairn),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_world_is_known_in_circles() {
        let mut known = KnownWorld {
            centre: Vec3::ZERO,
            radius: 100.0,
            pockets: Vec::new(),
        };
        assert!(known.knows(Vec3::new(50.0, 0.0, 0.0)));
        assert!(!known.knows(Vec3::new(300.0, 0.0, 0.0)));
        known.pockets.push(Pocket {
            at: Vec3::new(300.0, 0.0, 0.0),
            radius: 40.0,
        });
        assert!(known.knows(Vec3::new(310.0, 0.0, 20.0)));
        assert!(
            !known.knows(Vec3::new(200.0, 0.0, 0.0)),
            "the ground between home and a pocket stays unknown",
        );
    }

    /// The road-diet, in order: satchel first, then the heath, and every
    /// picking packs a little back into the satchel.
    #[test]
    fn the_road_eats_satchel_first_then_the_heath() {
        use bevy::ecs::system::SystemState;
        let mut world = World::new();
        // Seated like the world seats it: bushes are chunk children, their
        // globals are BENT, and forage_tick unbends them — a flat global
        // here would test a data shape production never produces.
        let (seat, _) = crate::globe::bend_frame(Vec3::new(10.0, 0.0, 0.0));
        world.spawn((
            GlobalTransform::from_translation(seat),
            crate::scatter::FoodSource {
                amount: 3.0,
                regrowth: 0.0,
            },
        ));
        let mut bushes: SystemState<
            Query<(Entity, &GlobalTransform, &mut crate::scatter::FoodSource)>,
        > = SystemState::new(&mut world);

        // Fed: the road holds.
        let mut needs = Needs {
            hunger: 0.2,
            ..default()
        };
        let mut query = bushes.get_mut(&mut world).unwrap();
        assert!(forage_tick(Vec3::ZERO, 0.1, &mut needs, None, &mut query).is_none());

        // Hungry with a packed satchel: the satchel pays, no detour.
        needs.hunger = 0.8;
        let mut satchel = work::Rations(2.0);
        let mut query = bushes.get_mut(&mut world).unwrap();
        assert!(forage_tick(Vec3::ZERO, 0.1, &mut needs, Some(&mut satchel), &mut query).is_none());
        assert!(satchel.0 < 2.0, "the satchel should be bitten");
        assert!(needs.hunger < 0.4, "the bite should feed");

        // Hungry with an empty satchel: the bush is the meal — walk to it,
        // and standing at it, eat and pack.
        needs.hunger = 0.9;
        satchel.0 = 0.0;
        let mut query = bushes.get_mut(&mut world).unwrap();
        let meal = forage_tick(Vec3::ZERO, 0.1, &mut needs, Some(&mut satchel), &mut query)
            .expect("a fruiting bush in reach is a meal");
        assert!(
            meal.distance(Vec3::new(10.0, 0.0, 0.0)) < 1.0,
            "walk to the bush"
        );

        let hungry_before = needs.hunger;
        let mut query = bushes.get_mut(&mut world).unwrap();
        forage_tick(meal, 0.5, &mut needs, Some(&mut satchel), &mut query)
            .expect("standing at the bush is still the bush's business");
        assert!(needs.hunger < hungry_before, "the bush should feed");
        assert!(satchel.0 > 0.0, "the picking should pack the satchel");

        // Bare land, empty satchel: no detour to give — the road goes on.
        needs.hunger = 0.9;
        let mut world_bare = World::new();
        let mut bare: SystemState<
            Query<(Entity, &GlobalTransform, &mut crate::scatter::FoodSource)>,
        > = SystemState::new(&mut world_bare);
        let mut query = bare.get_mut(&mut world_bare).unwrap();
        assert!(forage_tick(Vec3::ZERO, 0.1, &mut needs, Some(&mut satchel), &mut query).is_none());
    }

    /// The map never stops learning, and never un-knows a walked road.
    ///
    /// The old silent cap left a black hole of fog over a quarry the
    /// miners worked daily: at 128 pockets the map went blind, forever.
    /// Now the list tidies itself — and every spot ever learned must
    /// still be known afterwards, however hard the merging squeezed.
    #[test]
    fn the_map_never_stops_learning() {
        let mut known = KnownWorld::default();
        let mut learned: Vec<Vec3> = Vec::new();
        // Far more ground than the shader can hold, scattered wide on a
        // deterministic spiral well outside the home circle.
        for n in 0..300 {
            let angle = n as f32 * 2.399963;
            let reach = 220.0 + (n as f32) * 9.0;
            let spot = Vec3::new(angle.cos() * reach, 0.0, angle.sin() * reach);
            known.learn(spot, 34.0);
            learned.push(spot);
        }
        assert!(
            known.pockets.len() <= 128,
            "the veil shader holds 128 pockets; the map holds {}",
            known.pockets.len(),
        );
        for spot in &learned {
            assert!(
                known.knows(*spot),
                "ground once learned went back to the dark at {spot}",
            );
        }
    }

    /// Pockets the home circle grows over are dropped as redundant.
    #[test]
    fn the_home_circle_swallows_its_pockets() {
        let mut known = KnownWorld::default();
        known.learn(Vec3::new(60.0, 0.0, 0.0), 34.0);
        assert!(
            known.pockets.is_empty(),
            "a pocket wholly inside the home circle is redundant",
        );
        // But one straddling the edge is real knowledge and stays.
        known.learn(Vec3::new(180.0, 0.0, 0.0), 34.0);
        assert_eq!(known.pockets.len(), 1);
    }
}
