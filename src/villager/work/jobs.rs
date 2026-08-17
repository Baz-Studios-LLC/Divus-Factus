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

/// How far a trade will range for its own work once nothing is left
/// within a working walk. Twice the working walk and no more.
///
/// Five hundred killed a village outright. A gatherer will walk any
/// distance you allow, food trades are exempt from carrying rations
/// because they eat what they harvest, and nothing else was stopping
/// them - so ten people spent their days walking half a kilometer for
/// one bush each and every one of them starved with food in the store.
/// The walk has to be worth the load at the end of it.
pub(crate) const RANGE_REACH: f32 = 330.0;

/// When a hand first came up empty. A trade comes up empty for all sorts
/// of ordinary reasons — a bush picked bare a moment ago, a footing being
/// laid this very instant, a world whose scatter has not finished
/// spawning — so the stopgap below waits out a grace before it overrules
/// the morning's muster.
#[derive(Component)]
pub struct Jobless(pub f64);

/// How long a pair of hands may find nothing before they take up whatever
/// work the ground does offer.
const JOBLESS_GRACE: f64 = 6.0;

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
pub(crate) fn find_shore(terrain: &Terrain, center: Vec3, rng: &mut Rng) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for _ in 0..24 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let (sin, cos) = angle.sin_cos();
        let mut last_dry: Option<Vec3> = None;
        let mut step = 4.0;
        while step < WORK_REACH {
            let x = center.x + cos * step;
            let z = center.z + sin * step;
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
    center: Vec3,
    rng: &mut Rng,
    wanted: impl Fn(&Terrain, f32, f32) -> bool,
) -> Option<Vec3> {
    find_ground_in(terrain, center, rng, 40, wanted)
}

/// [`find_ground`] with the number of darts thrown made explicit.
///
/// Forty is plenty when the question is loose - somewhere flat, somewhere
/// wooded - because such ground is everywhere and any dart will do. It is far
/// too few when the question is NARROW, and a miss then is not "there is none"
/// but "we did not look hard enough". A settlement went eight days without
/// ever wanting a mine because forty darts kept failing to land on the one
/// bank it could have cut a drift into, and stone stayed at nine while timber
/// passed four hundred. A negative from this function is only worth what the
/// search behind it cost.
pub(crate) fn find_ground_in(
    terrain: &Terrain,
    center: Vec3,
    rng: &mut Rng,
    tries: usize,
    wanted: impl Fn(&Terrain, f32, f32) -> bool,
) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for _ in 0..tries {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let distance = rng.range(10.0, WORK_REACH);
        let (sin, cos) = angle.sin_cos();
        let x = center.x + cos * distance;
        let z = center.z + sin * distance;
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
    members: Query<&crate::villager::MemberOf>,
    mut rng: ResMut<SimRng>,
    mut workers: Query<
        (
            Entity,
            &Transform,
            &Needs,
            &Vocation,
            &Person,
            &mut Activity,
            Option<&Shunned>,
            Option<&Jobless>,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
            // Somebody running from a wolf is not available for work,
            // however idle they look on the way past.
            Without<crate::creature::wildlife::Fleeing>,
            // Nor is somebody the god is currently walking around in.
            Without<crate::avatar::Ridden>,
        ),
    >,
    // `Transform`, never `GlobalTransform`, for EVERYTHING a job can be sited
    // on. The sim runs in flat coordinates and the bend rewrites every global
    // onto the sphere - so a site read from a global sits below the flat
    // ground by the bend's own drop, about seventeen units at four hundred
    // and fifty from the origin. The 3D arrival check could then never pass:
    // a forester walked to the tree, stood ON it, counted as sixteen units
    // short, ran out of patience, shunned the spot and picked the next tree.
    // For ever. Timber read a flat nought on every seed while food (farmers
    // and hunters, sited flat) flowed - which was the shape of the mystery.
    //
    // Third time this class of bug has bitten: `wood_known` and
    // `survey_the_walls` each learned it separately. If a job is ever sited
    // from a new component, it reads the flat `Transform` too.
    bushes: Query<(Entity, &Transform, &FoodSource), Without<Villager>>,
    build_sites: Query<(
        Entity,
        &Transform,
        &ConstructionSite,
        &Blueprint,
        &crate::villager::MemberOf,
    )>,
    trees: Query<(Entity, &Transform, &crate::scatter::FellableTree)>,
    boulders: Query<(Entity, &Transform), With<crate::matter::Boulder>>,
    town: (
        Query<(Entity, &Transform, &Building)>,
        Query<(Entity, &Transform, &Field)>,
        Query<(Entity, &Transform, &Vitality), (With<Villager>, Without<Corpse>)>,
        Query<(Entity, &Transform, &crate::matter::Deposit)>,
        Query<(Entity, &Transform, &crate::scatter::SacredFlora)>,
        // The town's gates, and the posts already kept, so guards spread
        // over the gates instead of crowding the nearest one. Seated in
        // this satchel rather than taking parameters of their own: the
        // list stands at Bevy's sixteen and will not hold more.
        Query<&Transform, With<crate::villager::rampart::Gate>>,
        Query<&Job, With<Villager>>,
    ),
    mut towns: Query<(&crate::villager::SettlementGround, &mut Stockpile)>,
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
    let (buildings, fields, patients, deposits, sacred, gates, watched) = town;
    // A thin larder keeps the food trades working after dark - lanterns
    // on the dock - because the village eats all night whether or not
    // anyone is producing. Counted per TOWN: one settlement's famine is no
    // reason for another's fishers to work through the night. Gathered
    // before the loop, because the worker query is borrowed mutably in it.
    let mut mouths_by_town: std::collections::HashMap<Entity, usize> =
        std::collections::HashMap::new();
    for (worker, ..) in workers.iter() {
        if let Ok(member) = members.get(worker) {
            *mouths_by_town.entry(member.0).or_default() += 1;
        }
    }
    let daylight = is_work_hour(clock.time_of_day());

    for (entity, transform, needs, vocation, person, mut activity, shunned, jobless) in &mut workers
    {
        // Everything below is asked of the worker's OWN town: its square, its
        // larder, its building plots.
        let Ok(&crate::villager::MemberOf(home)) = members.get(entity) else {
            continue;
        };
        let Some((center, food, timber, stone, clay)) = towns.get(home).ok().map(|(g, store)| {
            (
                g.center,
                store.food(),
                store.timber,
                store.stone,
                store.clay,
            )
        }) else {
            continue;
        };
        let mouths = mouths_by_town.get(&home).copied().unwrap_or(0);
        let larder_thin = food < mouths as f32;
        // Sheltering counts as available: rain sent them to the fire, but
        // rain does not excuse the able-bodied — least of all from raising
        // the roofs that would get them OUT of it. Without this, one long
        // downpour parked the whole workforce at a cold fire while the
        // timber pile sat untouched.
        if !matches!(
            *activity,
            Activity::Idle | Activity::Wandering | Activity::Sheltering
        ) {
            continue;
        }
        let night_shift = larder_thin
            && matches!(
                vocation,
                Vocation::Fisher | Vocation::Gatherer | Vocation::Hunter
            );
        // The day has hours now: no new work at the midday meal or in the
        // evening, so a followed villager's day has a SHAPE — shift, meal,
        // shift, supper, bed — instead of labor scattered like static.
        // (A thin larder still overrides: hunger keeps no schedule.)
        if !(clock.work_hours() || night_shift || larder_thin) {
            continue;
        }
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
        // ...and only while the larder is THIN, which is the emergency
        // the exemption was written for. Applied to a full store it
        // meant a gatherer never once turned for home.
        if needs.hunger > HUNGRY_THRESHOLD && !(feeds_the_village && larder_thin) {
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

        // The works this trade raises for itself, if ground is broken for
        // it and there is anything to build with. See OWN_WORKS.
        let raising = |kind: BuildingKind| {
            (timber >= 1.0)
                .then(|| {
                    build_sites.iter().find(|(_, _, cs, plan, member)| {
                        member.0 == home
                            && plan.kind == kind
                            && cs.stone_laid >= cs.footing_stone(plan.kind)
                    })
                })
                .flatten()
                .map(|(site, at, ..)| {
                    Job::at(
                        at.translation,
                        Some(site),
                        at.translation.distance(transform.translation),
                    )
                })
        };

        let job = match vocation {
            // Gatherers fill the larder first; with food put by, they go
            // after the rarer gifts — incense herb and dyeflowers.
            Vocation::Gatherer => {
                let food_job = nearest_job(
                    bushes
                        .iter()
                        .filter(|(_, _, source)| source.amount > 0.5)
                        .map(|(bush, t, _)| (bush, t.translation)),
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
                            .map(|(stand, t, _)| (stand, t.translation)),
                        transform.translation,
                        WORK_REACH,
                        &known_far,
                        &permitted,
                    )
                };
                if food >= 25.0 {
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

            // The dock is the fisher's post when one stands - and when
            // one is RISING, laying its planks is the work. A fisher who
            // fishes off a bare shore all season while the dock they
            // would rather stand on waits for a spare carpenter has the
            // wrong priorities, and so did the code.
            Vocation::Fisher => raising(BuildingKind::Dock).or_else(|| {
                buildings
                    .iter()
                    .find(|(_, _, b)| b.kind == BuildingKind::Dock)
                    .map(|(dock, dock_at, _)| {
                        // The post is out at the deck's end, past the shallows —
                        // the planks are ground now, and the fish run under the
                        // far rail.
                        let at = dock_at.translation + dock_at.rotation * Vec3::new(0.0, 0.0, 4.6);
                        Job::at(at, Some(dock), at.distance(transform.translation))
                    })
                    .or_else(|| {
                        find_shore(&terrain, center, &mut rng.0)
                            .filter(|at| permitted(*at))
                            .map(|at| Job::at(at, None, at.distance(transform.translation)))
                    })
            }),

            // Miners feed two hungers: the stone every foundation wants,
            // and the ore the blacksmith's fire waits on. Stone while the
            // pile runs thin; the far vein once the village can spare the
            // walk.
            // A mine going up outranks everything: three cartloads a
            // swing once it stands, against one barrow off a boulder.
            Vocation::Miner if raising(BuildingKind::Mine).is_some() => raising(BuildingKind::Mine),
            Vocation::Miner => {
                // A built mine outranks loose boulders: the drift is dug,
                // the stone is waiting, and the yard is the miner's post.
                let mine_job = buildings
                    .iter()
                    .find(|(_, _, b)| b.kind == BuildingKind::Mine)
                    .map(|(works, works_at, _)| {
                        let at = works_at.translation;
                        Job::at(at, Some(works), at.distance(transform.translation))
                    });
                // Then the quarry, which is where a village's stone actually
                // comes from now that the ground is not strewn with it. Ahead
                // of loose boulders because it is a face worth returning to
                // rather than one barrow's worth, and behind the mine only
                // because a drift under cover beats an open working.
                //
                // It has to be ahead of the boulders and not merely in the
                // list: a miner who takes the nearest stone first spends the
                // village's whole morning on the three pebbles left in the
                // meadow and never walks to the quarry at all.
                let quarry_job = || {
                    nearest_job(
                        deposits
                            .iter()
                            .filter(|(_, _, deposit)| {
                                deposit.kind == crate::matter::DepositKind::Stone
                                    && deposit.amount > 0.5
                            })
                            .map(|(face, t, _)| (face, t.translation)),
                        transform.translation,
                        WORK_REACH,
                        &known_far,
                        &permitted,
                    )
                };
                let stone_job = mine_job
                    .or_else(quarry_job)
                    .or_else(|| {
                        nearest_job(
                            boulders.iter().map(|(rock, t)| (rock, t.translation)),
                            transform.translation,
                            WORK_REACH,
                            &known_far,
                            &permitted,
                        )
                    })
                    .or_else(|| {
                        find_ground(&terrain, center, &mut rng.0, |t, x, z| {
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
                        .map(|(vein, t, _)| (vein, t.translation)),
                    transform.translation,
                    WORK_REACH,
                    &known_far,
                    &permitted,
                );
                if stone >= 12.0 {
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
                    .map(|(tree, t, _)| (tree, t.translation)),
                transform.translation,
                WORK_REACH,
                &known_far,
                &permitted,
            )
            // No wood standing in reach: raise the mill instead. Never
            // before that - the mill is built out of timber they have
            // not cut yet.
            .or_else(|| raising(BuildingKind::Sawmill)),

            // Carpenters go where ground is broken — if there is timber to work.
            // A builder serves any site in their own town, footing or
            // frame, whichever that site is waiting on. This was two
            // trades: a carpenter who could only work a site whose
            // footing was already laid, and a mason who could only lay
            // it - so a build stalled dead every time the muster had
            // dealt one and not the other, with the stone sitting in the
            // pile. One pair of hands carries a building the whole way
            // now.
            Vocation::Builder => {
                let masonry = stone >= 1.0 || clay >= 1.0;
                let laying = build_sites
                    .iter()
                    .filter(|(_, _, cs, plan, member)| {
                        member.0 == home
                            && if cs.stone_laid < cs.footing_stone(plan.kind) {
                                masonry
                            } else {
                                timber >= 1.0 || masonry
                            }
                    })
                    .map(|(site, at, cs, plan, _)| {
                        (
                            site,
                            at.translation,
                            (
                                // A footing waiting on its first stone
                                // outranks a frame: nothing else can
                                // begin on that plot until it is down.
                                cs.stone_laid >= cs.footing_stone(plan.kind),
                                // Then the most beds. Hands used to go to
                                // whichever site was closest, and since a
                                // house costs half what a hall does it
                                // always finished first - the hall the
                                // planner put ahead of it stood open
                                // while a family moved in and the rest
                                // slept out.
                                std::cmp::Reverse(plan.kind.sleeps()),
                            ),
                            at.translation.distance(transform.translation),
                        )
                    })
                    .min_by(|a, b| a.2.cmp(&b.2).then(a.3.total_cmp(&b.3)))
                    .map(|(site, at, _, d)| Job::at(at, Some(site), d));
                // Nothing to build on: cut stone like a miner — or, when
                // the clay store is thin, dig the red bank instead.
                laying
                    .or_else(|| {
                        nearest_job(
                            boulders.iter().map(|(rock, t)| (rock, t.translation)),
                            transform.translation,
                            WORK_REACH,
                            &known_far,
                            &permitted,
                        )
                    })
                    .or_else(|| {
                        if clay >= 14.0 {
                            return None;
                        }
                        nearest_job(
                            deposits
                                .iter()
                                .filter(|(_, _, deposit)| {
                                    deposit.kind == crate::matter::DepositKind::Clay
                                        && deposit.amount > 0.5
                                })
                                .map(|(bank, t, _)| (bank, t.translation)),
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
                    .map(|(f, t, _)| (f, t.translation))
                    // Failing that, the nearest plot nobody has claimed —
                    // which is how a homestead's own ground, turned when its
                    // roof went on, ends up worked by whoever lives there.
                    .or_else(|| {
                        fields
                            .iter()
                            .filter(|(_, _, f)| f.farmer == Entity::PLACEHOLDER)
                            .map(|(f, t, _)| (f, t.translation))
                            .filter(|(_, at)| at.distance(transform.translation) < WORK_REACH)
                            .min_by(|a, b| {
                                a.1.distance(transform.translation)
                                    .total_cmp(&b.1.distance(transform.translation))
                            })
                            .inspect(|(field, _)| {
                                // Taken: from here it is theirs, and the
                                // ordinary "work my own field" path owns it.
                                commands.entity(*field).insert(ClaimField(entity));
                            })
                    });
                match mine {
                    Some((field, at)) => {
                        Some(Job::at(at, Some(field), at.distance(transform.translation)))
                    }
                    None => {
                        // Farmland grows as one farm, not scattered allotments:
                        // each new plot takes the next open cell of the grid the
                        // first field started, rows shared, a path's width apart.
                        let good_ground = |at: Vec3| {
                            terrain.is_walkable(at.x, at.z)
                                && at.y > WATER_LEVEL + 2.0
                                && !matches!(
                                    terrain.biome_at(at.x, at.z),
                                    Biome::Alpine | Biome::Arid
                                )
                                && fields
                                    .iter()
                                    .all(|(_, t, _)| t.translation.distance(at) > 3.6)
                                && trees
                                    .iter()
                                    .all(|(_, t, _)| t.translation.distance(at) > 3.5)
                                && permitted(at)
                        };
                        let anchor = fields
                            .iter()
                            .min_by(|a, b| {
                                a.1.translation
                                    .distance(center)
                                    .total_cmp(&b.1.translation.distance(center))
                            })
                            .map(|(_, t, _)| (t.translation, t.rotation));
                        let gridded = anchor.and_then(|(origin, rotation)| {
                            let across = rotation * Vec3::new(4.9, 0.0, 0.0);
                            let down = rotation * Vec3::new(0.0, 0.0, 4.1);
                            // Ring by ring outward, so the farm stays compact.
                            (1..=4i32)
                                .flat_map(|ring| {
                                    (-ring..=ring).flat_map(move |i| {
                                        (-ring..=ring).filter_map(move |j| {
                                            (i.abs().max(j.abs()) == ring).then_some((i, j))
                                        })
                                    })
                                })
                                .map(|(i, j)| {
                                    let spot = origin + across * i as f32 + down * j as f32;
                                    Vec3::new(spot.x, terrain.height_at(spot.x, spot.z), spot.z)
                                })
                                .find(|at| good_ground(*at))
                        });
                        // The first plot — or a farm hemmed in on every side —
                        // falls back to the open ring slots.
                        gridded
                            .or_else(|| {
                                village_slots(center, 5..7, 12.0)
                                    .into_iter()
                                    .map(|(x, z, _)| Vec3::new(x, terrain.height_at(x, z), z))
                                    .find(|at| {
                                        good_ground(*at)
                                            && fields
                                                .iter()
                                                .all(|(_, t, _)| t.translation.distance(*at) > 7.0)
                                    })
                            })
                            .map(|at| Job::at(at, None, at.distance(transform.translation)))
                    }
                }
            }

            // The cook's post is the tavern kitchen.
            Vocation::Cook => buildings
                .iter()
                .find(|(_, _, b)| b.kind == BuildingKind::Tavern)
                .map(|(tavern, at, _)| {
                    let at = at.translation;
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
                })
                // Nobody to tend: raise the hut the salves will live in.
                .or_else(|| raising(BuildingKind::Herbalist)),

            // Explorers muster on their own; the expedition system owns them.
            Vocation::Explorer => None,

            // The priest's post is the shrine.
            Vocation::Priest => buildings
                .iter()
                .find(|(_, _, b)| b.kind == BuildingKind::Shrine)
                .map(|(shrine, at, _)| {
                    let at = at.translation;
                    Job::at(at, Some(shrine), at.distance(transform.translation))
                }),

            // A guard's post is the tower if one stands, the village edge
            // otherwise; the walking of it is the work.
            //
            // Unless one is RISING, in which case that is the work. The
            // fear that made them a guard is the fear that broke the
            // ground, and a spear walked in a circle at twenty-two
            // strides never reaches a mauling out at ninety. The tower
            // does, because wolves will not come near it.
            // A guard keeps the GATE where the town has one - it is the
            // hole in the wall, and the only ground a wolf can walk in
            // by. Brett: "Guards could patrol it and stand watch at the
            // gates." Which gate is the guard's own business: the
            // nearest unkept one, so a town with three gates posts three
            // guards rather than crowding one. Failing a gate, the
            // watchtower; failing that, a turn about the treeline, which
            // is what a guard did before there were walls.
            Vocation::Guard => raising(BuildingKind::Watchtower).or_else(|| {
                let kept: Vec<Vec3> = watched.iter().map(|job| job.site).collect();
                let post = gates
                    .iter()
                    .map(|at| at.translation)
                    .filter(|gate| !kept.iter().any(|taken| taken.distance(*gate) < 3.0))
                    .min_by(|a, b| {
                        a.distance(transform.translation)
                            .total_cmp(&b.distance(transform.translation))
                    })
                    .or_else(|| {
                        buildings
                            .iter()
                            .find(|(_, _, b)| b.kind == BuildingKind::Watchtower)
                            .map(|(_, at, _)| at.translation)
                    })
                    .unwrap_or_else(|| {
                        let angle = rng.0.range(0.0, std::f32::consts::TAU);
                        let (sin, cos) = angle.sin_cos();
                        let (x, z) = (center.x + cos * 22.0, center.z + sin * 22.0);
                        Vec3::new(x, terrain.height_at(x, z), z)
                    });
                Some(Job::at(post, None, post.distance(transform.translation)))
            }),
        };

        // Nothing in reach for their own trade. Before they give the
        // trade up, they RANGE: the same work they were doing, at
        // whatever distance it is actually to be found, on ground the
        // village may never have walked.
        //
        // This is the difference between a miner who runs out of rock and
        // goes looking for more, and a miner who quietly stops being a
        // miner - which is what happened before, within six seconds. The
        // road is not free: the rations rule below charges the larder for
        // a long walk and refuses the job outright when the village
        // cannot provision it, so ranging is a thing a fed village does
        // and a hungry one cannot afford.
        let range_out = || -> Option<Job> {
            // Not while the larder is thin. A village with nothing put by
            // needs its food trades working the near ground, where the
            // yield per hour is highest - the long walk is a thing a fed
            // village can afford and a hungry one cannot, which is the
            // same rule the rations already state for every other trade.
            if larder_thin {
                return None;
            }
            let far = |candidates: &mut dyn Iterator<Item = (Entity, Vec3)>| {
                nearest_job(
                    candidates,
                    transform.translation,
                    RANGE_REACH,
                    |_, _| true,
                    &permitted,
                )
            };
            match vocation {
                Vocation::Miner => far(&mut boulders
                    .iter()
                    .map(|(rock, t)| (rock, t.translation))
                    .chain(
                        deposits
                            .iter()
                            .filter(|(_, _, deposit)| deposit.amount > 0.5)
                            .map(|(vein, t, _)| (vein, t.translation)),
                    )),
                Vocation::Forester => far(&mut trees
                    .iter()
                    .filter(|(_, _, tree)| tree.harvestable())
                    .map(|(tree, t, _)| (tree, t.translation))),
                Vocation::Gatherer => far(&mut bushes
                    .iter()
                    .filter(|(_, _, source)| source.amount > 0.5)
                    .map(|(bush, t, _)| (bush, t.translation))),
                Vocation::Hunter => far(&mut game
                    .iter()
                    .filter(|(_, _, genome)| genome.species != Species::Human)
                    .map(|(prey, t, _)| (prey, t.translation))),
                _ => None,
            }
        };
        let job = job.or_else(range_out);

        // Their own trade had nothing for them anywhere. A pair of hands
        // with nothing to do is the one thing the muster exists to
        // prevent, so
        // instead of standing about until tomorrow morning they take up
        // whatever work the ground DOES offer, and become that trade
        // until the next muster deals them again. This is what keeps nine
        // carpenters from watching an unlaid footing for a whole day
        // because the one miner walked off the pick at dawn.
        //
        // Explorers are exempt: an explorer with no job is not idle, they
        // are being held for a muster of their own.
        let stopgap = || -> Option<(Vocation, Job)> {
            if *vocation == Vocation::Explorer {
                return None;
            }
            let cut = || {
                nearest_job(
                    boulders.iter().map(|(rock, t)| (rock, t.translation)),
                    transform.translation,
                    WORK_REACH,
                    &known_far,
                    &permitted,
                )
                .map(|job| (Vocation::Miner, job))
            };
            let fell = || {
                nearest_job(
                    trees
                        .iter()
                        .filter(|(_, _, tree)| tree.harvestable())
                        .map(|(tree, t, _)| (tree, t.translation)),
                    transform.translation,
                    WORK_REACH,
                    &known_far,
                    &permitted,
                )
                .map(|job| (Vocation::Forester, job))
            };
            let pick = || {
                nearest_job(
                    bushes
                        .iter()
                        .filter(|(_, _, source)| source.amount > 0.5)
                        .map(|(bush, t, _)| (bush, t.translation)),
                    transform.translation,
                    WORK_REACH,
                    &known_far,
                    &permitted,
                )
                .map(|job| (Vocation::Gatherer, job))
            };
            let hunt = || {
                nearest_job(
                    game.iter()
                        .filter(|(_, _, genome)| genome.species != Species::Human)
                        .map(|(prey, t, _)| (prey, t.translation)),
                    transform.translation,
                    WORK_REACH * 1.6,
                    &known_far,
                    &permitted,
                )
                .map(|job| (Vocation::Hunter, job))
            };
            // A tower going up takes any spare pair of hands before
            // anything else does. One frightened guard raising a watch
            // post alone is a long job, and the whole village has an
            // interest in it being a short one.
            let raise_the_watch = || {
                OWN_WORKS
                    .iter()
                    .map(|(_, works, _)| *works)
                    .chain([BuildingKind::Watchtower])
                    .find_map(|works| raising(works).map(|job| (Vocation::Builder, job)))
            };
            // A thin larder outranks everything; otherwise stone, because
            // stone is the want that stalls a village hardest and berries
            // are the one thing always within reach.
            if larder_thin {
                pick()
                    .or_else(hunt)
                    .or_else(raise_the_watch)
                    .or_else(cut)
                    .or_else(fell)
            } else {
                raise_the_watch()
                    .or_else(cut)
                    .or_else(fell)
                    .or_else(pick)
                    .or_else(hunt)
            }
        };
        let found = match job {
            Some(job) => Some((*vocation, job)),
            // Empty-handed. The clock starts; only a hand that has stayed
            // empty through the grace is dealt again.
            None => match jobless {
                Some(Jobless(since)) if clock.elapsed - since > JOBLESS_GRACE => stopgap(),
                Some(_) => None,
                None => {
                    commands.entity(entity).insert(Jobless(clock.elapsed));
                    None
                }
            },
        };
        let Some((trade, job)) = found else {
            continue;
        };
        if trade == Vocation::Hunter && std::env::var("DIVUS_FACTUS_HUNT_PROBE").is_ok() {
            info!(
                "hunt probe: {} takes the hunt, quarry {:.0} strides out",
                person.name,
                job.site.distance(transform.translation)
            );
        }

        // The road eats first. A worksite past a safe walk is taken
        // only with rations out of the larder - about one meal per
        // half-starvation of travel, capped at three - and a village
        // too poor to provision the road keeps its people near home,
        // where the famine watch will say so out loud.
        let round_trip = job.site.distance(transform.translation) + job.site.distance(center);
        let meals = ((round_trip / 2.4) / (super::super::SECONDS_TO_STARVE * 0.5))
            .floor()
            .min(3.0);
        // Rations for the road - for the FOOD trades most of all, since
        // they are the ones who walk furthest. They were the one group
        // excluded, on the reasoning that they eat what they harvest:
        // they do nibble at the worksite, but nothing feeds them on the
        // walk out or the walk back, and eight of them starved together
        // a hundred and twenty strides from a larder holding sixty.
        //
        // A thin larder is the one exception, and the one the exclusion
        // was written for: a village too poor to provision the road
        // sends its gatherers anyway, because the alternative is
        // everybody starving at home instead of somebody starving out.
        let feeds_the_village = feeds_the_village
            || matches!(
                trade,
                Vocation::Fisher | Vocation::Gatherer | Vocation::Hunter | Vocation::Farmer
            );
        if meals >= 1.0 && !(feeds_the_village && larder_thin) {
            let Ok((_, mut store)) = towns.get_mut(home) else {
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
        if trade != *vocation {
            info!(
                "{} found no work as a {} and {}",
                person.name,
                vocation.trade(),
                trade.taking_up()
            );
            commands.entity(entity).insert(trade);
        }
        *activity = Activity::Working;
        commands.entity(entity).remove::<Jobless>();
        commands.entity(entity).insert(job);
    }
}
