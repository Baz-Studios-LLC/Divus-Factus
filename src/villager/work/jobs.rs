//! The day's assignments: how the idle find their worksites.

use bevy::prelude::*;

use super::*;
use crate::creature::genome::{CreatureGenome, Species};
use crate::creature::{Airborne, Corpse, Creature, Held, Vitality};
use crate::rng::Rng;
use crate::scatter::FoodSource;
use crate::terrain::{Biome, Terrain, WATER_LEVEL};

/// A job in progress: where, how far along, and — for hunters — whom.
#[derive(Component, Debug)]
pub struct Job {
    pub site: Vec3,
    pub progress: f32,
    pub focus: Option<Entity>,
    /// Seconds left before an unreached worksite is given up on. Without this a
    /// site the pathfinder cannot actually deliver them to becomes a career of
    /// standing in a field.
    pub patience: f32,
}

impl Job {
    /// A job at `site` for a worker starting `distance` away. Patience scales
    /// with the commute — a shore forty seconds out must not be abandoned at
    /// thirty-five.
    fn at(site: Vec3, focus: Option<Entity>, distance: f32) -> Job {
        Job {
            site,
            progress: 0.0,
            focus,
            patience: 20.0 + distance * 0.8,
        }
    }
}

/// Whether this is an hour anyone works. Dawn to mid-afternoon; evenings and
/// nights belong to the village.
pub fn is_work_hour(time_of_day: f32) -> bool {
    (0.05..0.62).contains(&time_of_day)
}

/// A worksite this worker gave up on reaching. Without this, the nearest tree
/// across a river is chosen, abandoned, and chosen again, forever — the whole
/// profession stuck on one impossible errand.
#[derive(Component, Debug)]
pub struct Shunned {
    pub site: Vec3,
    pub remaining: f32,
}

/// How close to a shunned site a new job offer has to be to be refused too.
const SHUN_RADIUS: f32 = 5.0;

/// Grudges against unreachable worksites fade.
pub(crate) fn forget_shunned(
    mut commands: Commands,
    time: Res<Time>,
    mut shunned: Query<(Entity, &mut Shunned)>,
) {
    for (entity, mut shun) in &mut shunned {
        shun.remaining -= time.delta_secs();
        if shun.remaining <= 0.0 {
            commands.entity(entity).remove::<Shunned>();
        }
    }
}

/// Finds the nearest shoreline point: walk outward from the settlement until
/// the ground dips under water, and stand on the last dry step.
pub(crate) fn find_shore(terrain: &Terrain, centre: Vec3, rng: &mut Rng) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for _ in 0..24 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let (sin, cos) = angle.sin_cos();
        let mut last_dry: Option<Vec3> = None;
        let mut step = 4.0;
        while step < WORK_REACH {
            let x = centre.x + cos * step;
            let z = centre.z + sin * step;
            let height = terrain.height_at(x, z);
            if height <= WATER_LEVEL || terrain.river_surface_at(x, z).is_some() {
                if let Some(dry) = last_dry
                    && best.is_none_or(|(d, _)| step < d)
                {
                    best = Some((step, dry));
                }
                break;
            }
            if terrain.is_walkable(x, z) {
                last_dry = Some(Vec3::new(x, height, z));
            }
            step += 4.0;
        }
    }
    best.map(|(_, at)| at)
}

/// Finds the nearest ground matching a predicate, by throwing darts around the
/// settlement and keeping the closest hit. Commutes are dead time; the fun is
/// at the worksite, not on the road to it.
pub(crate) fn find_ground(
    terrain: &Terrain,
    centre: Vec3,
    rng: &mut Rng,
    wanted: impl Fn(&Terrain, f32, f32) -> bool,
) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for _ in 0..40 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let distance = rng.range(10.0, WORK_REACH);
        let (sin, cos) = angle.sin_cos();
        let x = centre.x + cos * distance;
        let z = centre.z + sin * distance;
        if terrain.is_walkable(x, z)
            && wanted(terrain, x, z)
            && best.is_none_or(|(d, _)| distance < d)
        {
            best = Some((distance, Vec3::new(x, terrain.height_at(x, z), z)));
        }
    }
    best.map(|(_, at)| at)
}

/// Meals in the satchel: taken from the larder when a worksite lies past
/// a safe walk, eaten on the road the moment hunger bites. Travel time
/// made into a food cost, which is what it always was.
#[derive(Component)]
pub struct Rations(pub f32);

/// The one shape every scouting trade shares: from candidate worksites,
/// the nearest that lies inside working reach — or on ground an explorer
/// has brought home — and is not recently shunned. Nine trades used to
/// hand-roll this pipeline; now there is exactly one copy of the rule.
fn nearest_job(
    candidates: impl Iterator<Item = (Entity, Vec3)>,
    from: Vec3,
    reach: f32,
    known_far: impl Fn(Vec3, f32) -> bool,
    permitted: impl Fn(Vec3) -> bool,
) -> Option<Job> {
    candidates
        .map(|(entity, at)| (entity, at, at.distance(from)))
        .filter(|(_, at, d)| (*d < reach || known_far(*at, *d)) && permitted(*at))
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(entity, at, d)| Job::at(at, Some(entity), d))
}

/// The idle and able take up the day's work.
pub(crate) fn take_up_work(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Res<Terrain>,
    known: Option<Res<crate::villager::explore::KnownWorld>>,
    site: Option<Res<SettlementSite>>,
    mut rng: ResMut<SimRng>,
    mut workers: Query<
        (
            Entity,
            &Transform,
            &Needs,
            &Vocation,
            &mut Activity,
            Option<&Shunned>,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
    bushes: Query<(Entity, &GlobalTransform, &FoodSource), Without<Villager>>,
    build_sites: Query<(Entity, &Transform, &ConstructionSite, &Blueprint)>,
    trees: Query<(Entity, &GlobalTransform, &crate::scatter::FellableTree)>,
    boulders: Query<(Entity, &GlobalTransform), With<crate::matter::Boulder>>,
    town: (
        Query<(Entity, &GlobalTransform, &Building)>,
        Query<(Entity, &Transform, &Field)>,
        Query<(Entity, &Transform, &Vitality), (With<Villager>, Without<Corpse>)>,
        Query<(Entity, &GlobalTransform, &crate::matter::Deposit)>,
        Query<(Entity, &GlobalTransform, &crate::scatter::SacredFlora)>,
    ),
    mut stores: Query<&mut Stockpile>,
    game: Query<
        (Entity, &Transform, &CreatureGenome),
        (
            With<Creature>,
            Without<Villager>,
            Without<Corpse>,
            Without<Held>,
        ),
    >,
    carcasses: Query<
        (Entity, &Transform, &CreatureGenome),
        (
            With<Creature>,
            With<Corpse>,
            Without<Villager>,
            Without<Held>,
        ),
    >,
) {
    let Some(site) = site else {
        return;
    };
    let (buildings, fields, patients, deposits, sacred) = town;
    // A thin larder keeps the food trades working after dark - lanterns
    // on the dock - because the village eats all night whether or not
    // anyone is producing.
    let mouths = workers.iter().count();
    let larder_thin = stores
        .get(site.settlement)
        .is_ok_and(|s| s.food() < mouths as f32);
    let daylight = is_work_hour(clock.time_of_day());

    for (entity, transform, needs, vocation, mut activity, shunned) in &mut workers {
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        let night_shift = larder_thin
            && matches!(
                vocation,
                Vocation::Fisher | Vocation::Gatherer | Vocation::Hunter
            );
        if !daylight && !night_shift {
            continue;
        }
        // Hunger sends most trades off to find a meal — but the trades
        // that MAKE food work through it, or the village deadlocks: a
        // fisher too hungry to fish is how everyone starves beside an
        // empty larder. The food trades eat from their own yield instead.
        let feeds_the_village = matches!(
            vocation,
            Vocation::Fisher | Vocation::Gatherer | Vocation::Hunter | Vocation::Farmer
        );
        if needs.hunger > HUNGRY_THRESHOLD && !feeds_the_village {
            continue;
        }
        // The exhausted do not show up. Sleep is the cure, and the fire or a
        // roof is where sleep lives.
        if needs.rest > 0.9 {
            continue;
        }

        // A recently unreachable worksite is off the table; anything near it too.
        let permitted = |at: Vec3| shunned.is_none_or(|shun| shun.site.distance(at) > SHUN_RADIUS);
        // Ground an explorer has brought home counts as workable even far
        // out: the pockets are why expeditions matter to the trades.
        let known_far = |at: Vec3, d: f32| d < 700.0 && known.as_ref().is_some_and(|k| k.knows(at));

        let job = match vocation {
            // Gatherers fill the larder first; with food put by, they go
            // after the rarer gifts — incense herb and dyeflowers.
            Vocation::Gatherer => {
                let food_job = nearest_job(
                    bushes
                        .iter()
                        .filter(|(_, _, source)| source.amount > 0.5)
                        .map(|(bush, t, _)| (bush, t.translation())),
                    transform.translation,
                    WORK_REACH,
                    &known_far,
                    &permitted,
                );
                let sacred_job = || {
                    nearest_job(
                        sacred
                            .iter()
                            .filter(|(_, _, flora)| flora.amount > 0.5)
                            .map(|(stand, t, _)| (stand, t.translation())),
                        transform.translation,
                        WORK_REACH,
                        &known_far,
                        &permitted,
                    )
                };
                if stores.get(site.settlement).is_ok_and(|s| s.food() >= 25.0) {
                    sacred_job().or(food_job)
                } else {
                    food_job.or_else(sacred_job)
                }
            }

            // A carcass already down is free meat: harvest before hunting,
            // and the village stops drowning in carrion.
            Vocation::Hunter => nearest_job(
                carcasses
                    .iter()
                    .filter(|(_, _, genome)| genome.species != Species::Human)
                    .map(|(kill, t, _)| (kill, t.translation)),
                transform.translation,
                WORK_REACH * 1.6,
                &known_far,
                &permitted,
            )
            .or_else(|| {
                nearest_job(
                    game.iter()
                        .filter(|(_, _, genome)| genome.species != Species::Human)
                        .map(|(prey, t, _)| (prey, t.translation)),
                    transform.translation,
                    WORK_REACH * 1.6,
                    &known_far,
                    &permitted,
                )
            }),

            // The dock is the fisher's post when one stands; the bare
            // shore otherwise.
            Vocation::Fisher => buildings
                .iter()
                .find(|(_, _, b)| b.kind == BuildingKind::Dock)
                .map(|(dock, dock_at, _)| {
                    let at = dock_at.translation();
                    Job::at(at, Some(dock), at.distance(transform.translation))
                })
                .or_else(|| {
                    find_shore(&terrain, site.centre, &mut rng.0)
                        .filter(|at| permitted(*at))
                        .map(|at| Job::at(at, None, at.distance(transform.translation)))
                }),

            // Miners feed two hungers: the stone every foundation wants,
            // and the ore the blacksmith's fire waits on. Stone while the
            // pile runs thin; the far vein once the village can spare the
            // walk.
            Vocation::Miner => {
                let stone_job = nearest_job(
                    boulders.iter().map(|(rock, t)| (rock, t.translation())),
                    transform.translation,
                    WORK_REACH,
                    &known_far,
                    &permitted,
                )
                .or_else(|| {
                    find_ground(&terrain, site.centre, &mut rng.0, |t, x, z| {
                        matches!(t.biome_at(x, z), Biome::Alpine)
                            || t.height_at(x, z) > WATER_LEVEL + 40.0
                    })
                    .filter(|at| permitted(*at))
                    .map(|at| Job::at(at, None, at.distance(transform.translation)))
                });
                let ore_job = nearest_job(
                    deposits
                        .iter()
                        .filter(|(_, _, deposit)| {
                            deposit.kind == crate::matter::DepositKind::Iron && deposit.amount > 0.5
                        })
                        .map(|(vein, t, _)| (vein, t.translation())),
                    transform.translation,
                    WORK_REACH,
                    &known_far,
                    &permitted,
                );
                if stores.get(site.settlement).is_ok_and(|s| s.stone >= 12.0) {
                    ore_job.or(stone_job)
                } else {
                    stone_job.or(ore_job)
                }
            }

            // Foresters fell real trees, and only real trees — where none
            // stand on known ground, the want goes unmet until an explorer
            // brings home a wood. Timber that appears from nowhere would
            // let the village stay home forever, and staying home is death.
            Vocation::Forester => nearest_job(
                trees
                    .iter()
                    .filter(|(_, _, tree)| tree.harvestable())
                    .map(|(tree, t, _)| (tree, t.translation())),
                transform.translation,
                WORK_REACH,
                &known_far,
                &permitted,
            ),

            // Carpenters go where ground is broken — if there is timber to work.
            Vocation::Carpenter => {
                if stores
                    .get(site.settlement)
                    .is_ok_and(|s| s.timber >= 1.0 || s.stone >= 1.0 || s.clay >= 1.0)
                {
                    build_sites
                        .iter()
                        .filter(|(_, _, cs, plan)| cs.stone_laid >= plan.kind.stone_cost())
                        .map(|(house, house_transform, ..)| {
                            (
                                house,
                                house_transform.translation,
                                house_transform.translation.distance(transform.translation),
                            )
                        })
                        .min_by(|a, b| a.2.total_cmp(&b.2))
                        .map(|(house, at, d)| Job::at(at, Some(house), d))
                } else {
                    None
                }
            }

            // Masons serve any site whose foundation still wants stone.
            Vocation::Mason => {
                let laying = if stores
                    .get(site.settlement)
                    .is_ok_and(|s| s.stone >= 1.0 || s.clay >= 1.0)
                {
                    build_sites
                        .iter()
                        .filter(|(_, _, cs, plan)| cs.stone_laid < plan.kind.stone_cost())
                        .map(|(b, t, ..)| {
                            (
                                b,
                                t.translation,
                                t.translation.distance(transform.translation),
                            )
                        })
                        .min_by(|a, b| a.2.total_cmp(&b.2))
                        .map(|(b, at, d)| Job::at(at, Some(b), d))
                } else {
                    None
                };
                // No foundations to lay: cut stone like a miner — or, when
                // the clay store is thin, dig the red bank instead.
                laying
                    .or_else(|| {
                        nearest_job(
                            boulders.iter().map(|(rock, t)| (rock, t.translation())),
                            transform.translation,
                            WORK_REACH,
                            &known_far,
                            &permitted,
                        )
                    })
                    .or_else(|| {
                        if stores.get(site.settlement).is_ok_and(|s| s.clay >= 14.0) {
                            return None;
                        }
                        nearest_job(
                            deposits
                                .iter()
                                .filter(|(_, _, deposit)| {
                                    deposit.kind == crate::matter::DepositKind::Clay
                                        && deposit.amount > 0.5
                                })
                                .map(|(bank, t, _)| (bank, t.translation())),
                            transform.translation,
                            WORK_REACH,
                            &known_far,
                            &permitted,
                        )
                    })
            }

            // Farmers work their own field, tilling a new one if they lack it.
            Vocation::Farmer => {
                let mine = fields
                    .iter()
                    .find(|(_, _, f)| f.farmer == entity)
                    .map(|(f, t, _)| (f, t.translation));
                match mine {
                    Some((field, at)) => {
                        Some(Job::at(at, Some(field), at.distance(transform.translation)))
                    }
                    // A new plot: flat, dry, out of everyone's way. The Job
                    // carries no focus; arriving farmers till on the spot.
                    None => village_slots(site.centre, 5..7)
                        .into_iter()
                        .map(|(x, z, _)| Vec3::new(x, terrain.height_at(x, z), z))
                        .find(|at| {
                            terrain.is_walkable(at.x, at.z)
                                && at.y > WATER_LEVEL + 2.0
                                && !matches!(
                                    terrain.biome_at(at.x, at.z),
                                    Biome::Alpine | Biome::Arid
                                )
                                && fields
                                    .iter()
                                    .all(|(_, t, _)| t.translation.distance(*at) > 7.0)
                                && trees
                                    .iter()
                                    .all(|(_, t, _)| t.translation().distance(*at) > 3.5)
                                && permitted(*at)
                        })
                        .map(|at| Job::at(at, None, at.distance(transform.translation))),
                }
            }

            // The cook's post is the tavern kitchen.
            Vocation::Cook => buildings
                .iter()
                .find(|(_, _, b)| b.kind == BuildingKind::Tavern)
                .map(|(tavern, at, _)| {
                    let at = at.translation();
                    Job::at(at, Some(tavern), at.distance(transform.translation))
                }),

            // The healer goes to whoever is worst hurt.
            Vocation::Healer => patients
                .iter()
                .filter(|(patient, _, vitality)| *patient != entity && vitality.harm > 0.15)
                .max_by(|a, b| a.2.harm.total_cmp(&b.2.harm))
                .map(|(patient, at, _)| {
                    Job::at(
                        at.translation,
                        Some(patient),
                        at.translation.distance(transform.translation),
                    )
                }),

            // Explorers muster on their own; the expedition system owns them.
            Vocation::Explorer => None,

            // The priest's post is the shrine.
            Vocation::Priest => buildings
                .iter()
                .find(|(_, _, b)| b.kind == BuildingKind::Shrine)
                .map(|(shrine, at, _)| {
                    let at = at.translation();
                    Job::at(at, Some(shrine), at.distance(transform.translation))
                }),

            // A guard's post is the tower if one stands, the village edge
            // otherwise; the walking of it is the work.
            Vocation::Guard => {
                let post = buildings
                    .iter()
                    .find(|(_, _, b)| b.kind == BuildingKind::Watchtower)
                    .map(|(_, at, _)| at.translation())
                    .unwrap_or_else(|| {
                        let angle = rng.0.range(0.0, std::f32::consts::TAU);
                        let (sin, cos) = angle.sin_cos();
                        let (x, z) = (site.centre.x + cos * 22.0, site.centre.z + sin * 22.0);
                        Vec3::new(x, terrain.height_at(x, z), z)
                    });
                Some(Job::at(post, None, post.distance(transform.translation)))
            }
        };

        if let Some(job) = job {
            // The road eats first. A worksite past a safe walk is taken
            // only with rations out of the larder - about one meal per
            // half-starvation of travel, capped at three - and a village
            // too poor to provision the road keeps its people near home,
            // where the famine watch will say so out loud.
            let round_trip =
                job.site.distance(transform.translation) + job.site.distance(site.centre);
            let meals = ((round_trip / 2.4) / (super::super::SECONDS_TO_STARVE * 0.5))
                .floor()
                .min(3.0);
            if meals >= 1.0 && !feeds_the_village {
                let Ok(mut store) = stores.get_mut(site.settlement) else {
                    continue;
                };
                if store.food() < meals + 2.0 {
                    continue;
                }
                let mut owed = meals;
                while owed > 0.0 {
                    store.larder.draw(1.0);
                    owed -= 1.0;
                }
                commands.entity(entity).insert(Rations(meals));
            }
            *activity = Activity::Working;
            commands.entity(entity).insert(job);
        }
    }
}
