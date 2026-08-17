//! Creatures: generation, locomotion and the small amount of physics they need.
//!
//! There is no physics engine here. The only bodies that ever leave the ground are
//! ones the player has picked up and thrown, and a ballistic arc plus a terrain
//! height check covers that completely. Adding a rigid-body crate to solve a
//! problem this size would cost far more in dependency churn than it returns.

pub mod anim;
pub mod body;
pub mod clip;
pub mod genome;
pub mod wildlife;

use bevy::prelude::*;

use crate::rng::Rng;
use crate::terrain::{Terrain, WATER_LEVEL};
use anim::CreatureMotion;
use body::{CreatureAssets, build_body, init_creature_assets};
use genome::{CreatureGenome, Species};

/// Gravity, in world units per second squared. Tuned for how a throw *reads*
/// rather than for realizm: real gravity makes tossed villagers land too fast to
/// follow at this camera distance.
const GRAVITY: f32 = 19.6;

/// Routes computed per frame across the whole world.
const ROUTES_PER_FRAME: usize = 4;

/// How far one leg of a long walk reaches before the walker stops and
/// looks again.
///
/// Short enough that the search always has budget to find it, long
/// enough that a walk home from the far woods is a handful of legs and
/// not a hundred. A walker who cannot be routed the whole way is walked
/// as far as can be seen - which is what anybody does crossing country.
const A_LEG_OF_THE_WAY: f32 = 90.0;

/// How near a doorway a routed walk gives way to a straight one. Long
/// enough to reach the far end of a longhouse, since a sleeper crossing
/// their own hall to the door is the walk this exists for, and short
/// enough that the straight line stays within one building's own ground.
const THREAD_THE_DOOR: f32 = 16.0;

pub struct CreaturePlugin;

impl Plugin for CreaturePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CreatureDied>()
            .add_systems(Startup, init_creature_assets)
            .add_systems(
                Update,
                (
                    plan_routes,
                    locomotion,
                    keep_apart,
                    drowning,
                    carrion_fades,
                    hurt_flashes,
                    apply_ballistics,
                    wildlife::wild_hunger,
                    wildlife::graze_and_flee,
                    wildlife::wolves_hunt,
                    wildlife::wolves_stalk,
                    wildlife::flee_to_safety,
                    wildlife::wild_breeding,
                    wildlife::wild_growth,
                    succumb,
                    anim::advance_motion,
                    anim::animate_creatures,
                    // After the sines, always: a clip writes back over the few
                    // joints it was drawn with and leaves the rest walking.
                    clip::clips_follow_the_day,
                    clip::play_clips,
                )
                    .chain()
                    .in_set(CreatureSet)
                    // After the door router, always. Movement targets are
                    // written by the activity systems and REWRITTEN by
                    // use_doors when a wall stands between walker and goal;
                    // when locomotion ran between those two writers, the
                    // walker stepped toward the table one frame and the
                    // doorstep the next, whipsawing at the threshold -
                    // Brett: "people still shake violently near doors."
                    .after(crate::villager::home::use_doors),
            );
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreatureSet;

/// Marks any living thing.
#[derive(Component)]
pub struct Creature;

/// Life, and the losing of it.
///
/// One bar, filled from two directions: starvation fills it slowly, violence fills
/// it in lumps. Rest and food drain it. At 1.0 the creature dies — which is the
/// moment this stops being a terrarium and starts being a world with stakes.
#[derive(Component, Debug, Default)]
pub struct Vitality {
    /// 0 healthy, 1 dead.
    pub harm: f32,
    /// Harm as of last frame, so the flash knows a fresh wound from an old one.
    pub last_harm: f32,
    /// Whether the killing blow, if it comes now, was violence rather than want.
    /// Doctrine will care about the difference; the dead do not.
    pub violent: bool,
    /// What last did them harm. The chronicle used to say only "was
    /// broken against the earth" for every violent death - lightning, a
    /// wolf, a fall, a hunter's spear, all one line - which told the
    /// player nothing about what their village is actually losing people
    /// to.
    pub undoing: Undoing,
}

/// What killed someone, in the plainest terms the chronicle can put it.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Undoing {
    /// An empty stomach and no larder to answer it.
    #[default]
    Hunger,
    /// A wolf.
    Teeth,
    /// The ground, arriving hard - thrown by the god, or dropped.
    Fall,
    /// Struck out of the sky.
    Lightning,
    /// Crushed by something the god was carrying.
    Weight,
    /// A blow from another living thing that was not a wolf.
    Blow,
}

impl Undoing {
    /// How the chronicle says it, after the name.
    pub fn how(self) -> &'static str {
        match self {
            Undoing::Hunger => "starved",
            Undoing::Teeth => "was killed by a wolf",
            Undoing::Fall => "was thrown down and died of the fall",
            Undoing::Lightning => "was struck dead by lightning",
            Undoing::Weight => "was crushed",
            Undoing::Blow => "was struck down",
        }
    }
}

/// Harm inflicted by hitting the ground at `impact` severity.
///
/// Gentle handling costs nothing; a full-strength hurl is worth most of a life, so
/// two of them kill. The gods people believe in have always been able to do this —
/// the question the game asks is what the survivors make of it.
pub fn impact_harm(impact: f32) -> f32 {
    ((impact - 0.45) * 1.1).max(0.0)
}

/// The dead. Still present, still named, still grabbable — a corpse is an *object
/// of consequence*, not a despawn.
#[derive(Component)]
pub struct Corpse;

/// Announcement of a death, for witnesses and, later, for history.
#[derive(Message, Clone, Debug)]
pub struct CreatureDied {
    /// Who died. Kept valid past death — a corpse is still an entity — so
    /// witnesses can name them and reckon kinship.
    pub entity: Entity,
    pub position: Vec3,
    /// Read by the coming doctrine system; witnesses today only care where.
    #[allow(dead_code)]
    pub name: Option<String>,
    pub violent: bool,
}

/// Where this creature is trying to walk. `None` means standing still.
#[derive(Component, Default)]
pub struct MoveTarget(pub Option<Vec3>);

/// Waypoints toward the current move target.
///
/// Held separately from `MoveTarget` so behavior code keeps saying *where* it wants
/// to go and never has to think about *how* — the route is recomputed here whenever
/// the destination changes.
#[derive(Component, Default)]
pub struct Route {
    /// Remaining waypoints, in order.
    pub waypoints: Vec<Vec3>,
    /// The destination these waypoints were computed for.
    goal: Option<Vec3>,
    /// Set when pathfinding failed, so it is not retried every frame.
    unreachable: bool,
    /// Where the last refusal was, and when it was given up on.
    ///
    /// This is the one field `clear` does not touch, and that is its whole
    /// purpose. `unreachable` above says it is not retried every frame, and it
    /// was not true: `locomotion` reads the flag, abandons the destination and
    /// calls `clear`, which puts the flag back to false — so whatever wanted to
    /// go there asked again on the very next frame, and got the most expensive
    /// answer there is. A search that FAILS expands its entire three-thousand
    /// node budget before it can say so.
    ///
    /// Measured: `creature: plan_routes` at 76ms of an 81ms frame, held there
    /// for thirty seconds at a stretch, which is about eighteen hundred
    /// consecutive full-budget searches for the same handful of places nobody
    /// could get to. That is the "fps all over the place".
    denied: Option<(Vec3, f32)>,
}

/// How long a refused destination stays refused, in seconds of world time.
///
/// It forgets rather than bans, because the world does change — a bridge gets
/// laid, a hall goes up and opens a way round. Long enough that a creature
/// fixed on somewhere unreachable costs one search instead of sixty a second,
/// short enough that nobody stands about after the way opens.
const GIVE_UP_FOR: f32 = 8.0;

/// How near a destination has to be to a refused one to count as the same
/// errand, squared. Two and a half meters — the navigation grid's own cell,
/// which is the finest distinction any of this can make anyway.
const SAME_ERRAND: f32 = crate::navigation::CELL * crate::navigation::CELL;

impl Route {
    /// The next point to walk toward.
    pub fn next(&self) -> Option<Vec3> {
        self.waypoints.first().copied()
    }

    /// Whether this destination was refused recently enough to not be worth
    /// asking about again.
    fn still_refused(&self, goal: Vec3, now: f32) -> bool {
        self.denied.is_some_and(|(refused, when)| {
            refused.distance_squared(goal) < SAME_ERRAND && now - when < GIVE_UP_FOR
        })
    }

    fn clear(&mut self) {
        self.waypoints.clear();
        self.goal = None;
        self.unreachable = false;
    }
}

/// Present while the player is holding this creature in the Divine Hand.
///
/// A held creature is exempt from locomotion and gravity — the Hand owns its
/// position outright.
#[derive(Component)]
pub struct Held;

/// Present while this creature's arms are full - a log on the shoulder, a
/// stone at the chest. The animator stills the arm swing and bends the
/// elbows to hold the burden instead of swinging through it.
#[derive(Component)]
pub struct Laden;

/// Present while the creature is in the air, carrying its current velocity.
#[derive(Component)]
pub struct Airborne {
    pub velocity: Vec3,
}

/// A creature moving faster than its own ordinary pace: a multiplier on the
/// walk, applied by [`locomotion`].
///
/// Only the god sprinting a body it is driving, for now. Kept here beside the
/// walking rather than in the miracle, because the walking is what has to
/// honor it — and because a villager who learns to run one day should find
/// the mechanism already waiting.
#[derive(Component)]
pub struct Sprinting(pub f32);

/// A childhood in progress. Ticks down; at zero the child comes of age and the
/// body is rebuilt as an adult's.
///
/// This is the piece that makes death survivable at the population scale:
/// without it, children stay children forever, the breeding pool only shrinks,
/// and every village is an extinction in progress no matter how well it eats.
#[derive(Component, Debug)]
pub struct Childhood {
    pub remaining: f32,
}

/// Spawns a creature and its body.
pub fn spawn_creature(
    commands: &mut Commands,
    assets: &CreatureAssets,
    genome: CreatureGenome,
    position: Vec3,
    facing: f32,
    idle_offset: f32,
) -> Entity {
    // Picking uses a sphere centered on the creature's midpoint rather than its feet,
    // so the grab target is the body rather than the ground under it.
    let pick_radius = genome.height() * 0.45;

    let root = commands
        .spawn((
            Name::new(match genome.species {
                Species::Human => "Villager",
                Species::Deer => "Deer",
                Species::Wolf => "Wolf",
                Species::Boar => "Boar",
            }),
            Creature,
            Transform::from_translation(position).with_rotation(Quat::from_rotation_y(facing)),
            Visibility::default(),
            MoveTarget::default(),
            Route::default(),
            Vitality::default(),
            CreatureMotion::new(idle_offset),
            crate::hand::PickRadius(pick_radius),
        ))
        .id();

    let rig = build_body(commands, assets, root, &genome);
    commands.entity(root).insert((genome, rig));
    root
}

/// Recomputes routes when a creature's destination changes.
///
/// Bounded per frame: pathfinding is the most expensive thing the simulation does,
/// and a settlement all deciding to eat at once would otherwise stall the tick.
pub(crate) fn plan_routes(
    terrain: Option<Res<Terrain>>,
    walls: Res<crate::navigation::Walls>,
    chart: Res<crate::navigation::Reachable>,
    mut creatures: Query<
        (
            &Transform,
            &MoveTarget,
            &mut Route,
            Has<crate::villager::home::Doorbound>,
        ),
        (
            With<Creature>,
            Without<Held>,
            Without<Airborne>,
            // A body the god is driving is steered, not routed. Its goal is
            // only a few strides ahead and moves every frame, so pathfinding
            // to it re-planned continuously and handed locomotion a first
            // waypoint almost underfoot — and locomotion eases off as it
            // nears a waypoint, so the god crawled. With no route, the walk
            // aims straight at where the god is pointing, which is the whole
            // idea of driving.
            Without<crate::avatar::Ridden>,
        ),
    >,
    time: Res<Time>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("creature: plan_routes");
    let Some(terrain) = terrain else {
        return;
    };
    let now = time.elapsed_secs();
    // The budget grows with the crowd: four routes a frame was sized for
    // a hamlet, and a city of eighty would starve for paths and stand
    // still in the street.
    let mut budget = ROUTES_PER_FRAME.max(creatures.iter().count() / 8);

    for (transform, target, mut route, doorbound) in &mut creatures {
        let Some(goal) = target.0 else {
            if route.goal.is_some() {
                route.clear();
            }
            continue;
        };

        // A walk the door router has hold of, close enough that the
        // doorway IS the journey, steers straight at it.
        //
        // The search cannot help here and quietly hurts: no path on a
        // two-and-a-half meter grid can thread a one meter door, so a
        // building somebody is entering or leaving is EXCUSED from the
        // walls - and a route to the door is then free to cut out
        // through the side wall on its way there. Measured, that was
        // every single wall-crossing in the village: a hundred and
        // twenty-one strides through their own walls in ten seconds,
        // every one of them with the router steering. Brett: "a lot of
        // people still walk through the walls."
        //
        // Straight is the honest line at this range. The router's near
        // leg stands inside a single room and its far leg straight out
        // through the opening, so the walk it describes never wanted a
        // path in the first place.
        if doorbound && transform.translation.distance(goal) < THREAD_THE_DOOR {
            route.waypoints.clear();
            route.goal = None;
            route.unreachable = false;
            continue;
        }

        // Already routed there - unless the route has run out underneath
        // them and the goal is still a long way off, which is what a
        // legged walk (below) leaves behind on purpose.
        if route.goal.is_some_and(|g| g.distance_squared(goal) < 0.25)
            && !(route.waypoints.is_empty()
                && transform.translation.distance(goal) > A_LEG_OF_THE_WAY * 0.5)
        {
            continue;
        }
        // Or refused it a moment ago. The world has not changed since, and a
        // refusal is the most expensive answer the search can give - see
        // `Route::denied`, which is the whole of this fix. The flag still goes
        // up so `locomotion` abandons the errand exactly as before; what does
        // not happen is the search.
        if route.still_refused(goal, now) {
            route.unreachable = true;
            continue;
        }
        // Or the chart already knows the two ends are on different islands, in
        // which case there is nothing to search FOR. See
        // `navigation::Reachable`: this is the real fix, and the refusal above
        // is the workaround it replaces - the refusal still has to pay for one
        // full failed search before it can remember anything, and this pays for
        // none. It never blocks a possible errand, because "different islands"
        // is the only answer it will give.
        //
        // Costs nothing against the budget either: no search is run, so a
        // village whose errands are all across a bay does not spend its whole
        // per-frame allowance discovering that.
        if chart.hopeless(transform.translation, goal) {
            route.waypoints.clear();
            route.unreachable = true;
            route.denied = Some((goal, now));
            continue;
        }
        if budget == 0 {
            continue;
        }
        budget -= 1;

        route.goal = Some(goal);
        match crate::navigation::find_path(
            &terrain,
            &walls,
            transform.translation,
            goal,
            crate::navigation::DEFAULT_BUDGET,
        ) {
            Some(waypoints) => {
                route.waypoints = waypoints;
                route.unreachable = false;
                route.denied = None;
            }
            None => {
                // Refused - but a walk home is not a thing anyone may be
                // told they cannot make.
                //
                // The search is bounded at three thousand cells, and a
                // walk of three hundred strides through broken country
                // can spend that and come back with nothing. What
                // happened then was fatal: `locomotion` reads
                // `unreachable`, drops the errand, and `still_refused`
                // makes sure the question is never asked again - so the
                // walker STANDS THERE. Brett's ledger: "Sperfiko starved
                // on the road, 275 strides from a larder that held
                // food." Twice in one day, and the larder held ten
                // thousand.
                //
                // So a refused walk is legged instead. Route as far
                // along the line as the search can manage in one go, and
                // ask again from there - which is how anybody crosses
                // country they cannot see the end of.
                let toward = (goal - transform.translation).normalize_or_zero();
                let far = transform.translation.distance(goal);
                if far > A_LEG_OF_THE_WAY * 1.2 {
                    let leg = transform.translation + toward * A_LEG_OF_THE_WAY;
                    if let Some(waypoints) = crate::navigation::find_path(
                        &terrain,
                        &walls,
                        transform.translation,
                        leg,
                        crate::navigation::DEFAULT_BUDGET,
                    ) {
                        // The GOAL stays the far one, so nothing else
                        // thinks the errand has changed; the waypoints
                        // only reach as far as this leg, and the test
                        // above brings them back here for the next one.
                        route.waypoints = waypoints;
                        route.unreachable = false;
                        route.denied = None;
                        continue;
                    }
                }
                // Under the wall probe the refusal says so out loud, with
                // the two facts that decide it - whether the ground at
                // either end is walkable at all, and how far apart they
                // are.
                if std::env::var("DIVUS_FACTUS_WALL_PROBE").is_ok() {
                    info!(
                        "route refused: {:.0},{:.0} -> {:.0},{:.0} ({:.0} apart, standing on {}, aiming at {})",
                        transform.translation.x,
                        transform.translation.z,
                        goal.x,
                        goal.z,
                        transform.translation.distance(goal),
                        if terrain.is_walkable(transform.translation.x, transform.translation.z) {
                            "walkable ground"
                        } else {
                            "UNWALKABLE ground"
                        },
                        if terrain.is_walkable(goal.x, goal.z) {
                            "walkable ground"
                        } else {
                            "UNWALKABLE ground"
                        },
                    );
                }
                route.waypoints.clear();
                route.unreachable = true;
                route.denied = Some((goal, now));
            }
        }
    }
}

/// Walks creatures toward their move target and keeps them on the ground.
fn locomotion(
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    trails: Option<Res<crate::trails::Trails>>,
    mut creatures: Query<
        (
            &CreatureGenome,
            &mut Transform,
            &mut MoveTarget,
            &mut CreatureMotion,
            &mut Route,
            Option<&Vitality>,
            Option<&Sprinting>,
        ),
        (
            With<Creature>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("creature: locomotion");
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs();

    for (genome, mut transform, mut target, mut motion, mut route, vitality, sprint) in
        &mut creatures
    {
        let mut speed = 0.0;
        // A kneeling pose is rooted to the ground. Prayer clears its current
        // destination, but a path can still be carrying old waypoints from
        // the walk that preceded it; following those made villagers slide
        // across the field on their knees. Locomotion owns routes, so it is
        // the one place that can reliably stop both kinds of movement.
        if motion.kneeling {
            target.0 = None;
            route.clear();
            motion.speed = 0.0;
            continue;
        }
        // The dying walk like the dying.
        let vigor = vitality.map_or(1.0, |v| 1.0 - v.harm * 0.55);

        // A destination with no route to it is abandoned rather than walked at.
        // Steering straight for an unreachable goal is how creatures ended up
        // standing in the sea.
        if route.unreachable {
            target.0 = None;
            route.clear();
        }

        // Follow the route, not the goal. The final waypoint *is* the goal.
        let step_target = route.next().or(target.0);

        if let Some(destination) = step_target {
            let to_target = Vec3::new(
                destination.x - transform.translation.x,
                0.0,
                destination.z - transform.translation.z,
            );
            let distance = to_target.length();

            // Arrival tolerance scales with size, so a boar does not jitter trying
            // to land on the same point a villager can hit.
            let tolerance = genome.height() * 0.3;

            if distance <= tolerance {
                if route.next().is_some() {
                    route.waypoints.remove(0);
                } else {
                    target.0 = None;
                    route.clear();
                }
            } else {
                let direction = to_target / distance;

                // Ease into and out of the destination rather than starting and
                // stopping instantly. Worn trails are quicker underfoot —
                // the village's own habits pave its shortcuts.
                let approach = (distance / (genome.height() * 2.0)).clamp(0.25, 1.0);
                let paved = trails.as_ref().map_or(1.0, |t| {
                    t.haste(transform.translation.x, transform.translation.z)
                });
                let running = sprint.map_or(1.0, |s| s.0);
                speed = genome.walk_speed() * approach * vigor * paved * running;

                // Swimming is slow: most of the stride is lost to the water.
                let step = (speed * dt).min(distance) * (1.0 - motion.swim * 0.55);
                let ahead_x = transform.translation.x + direction.x * step;
                let ahead_z = transform.translation.z + direction.z * step;
                // Legs are not climbing gear: a stride onto genuinely
                // steep ground is refused when it climbs. Routes avoid
                // such cells, but straight-line steering before a route
                // lands — and corners cut between waypoints — used to
                // walk people up cliff faces. Downhill is always allowed,
                // so nobody strands on a crag they somehow reached.
                let here_y = terrain.height_at(transform.translation.x, transform.translation.z);
                let blocked = terrain.slope_at(ahead_x, ahead_z) >= 0.55
                    && terrain.height_at(ahead_x, ahead_z) > here_y + 0.1
                    && terrain.boardwalk_at(ahead_x, ahead_z).is_none();
                if !blocked {
                    transform.translation.x += direction.x * step;
                    transform.translation.z += direction.z * step;
                }

                // Turn toward travel rather than snapping, so direction changes read
                // as the creature deciding rather than teleporting.
                let desired = facing_rotation(direction);
                let turn = 1.0 - (-9.0 * dt).exp();
                transform.rotation = transform.rotation.slerp(desired, turn);
            }
        }

        // Stick to the ground. Terrain is the authority on height, so this is
        // a lookup rather than a collision test. In deep water there is no
        // ground to stand on: the body rides just under the surface and the
        // stride becomes a paddle, at less than half pace.
        // Stand height includes built decks: on a dock the planks are the
        // floor, and the water check below sees no depth to swim in.
        let floor = terrain.stand_height_at(transform.translation.x, transform.translation.z);
        let surface = terrain
            .river_surface_at(transform.translation.x, transform.translation.z)
            .unwrap_or(WATER_LEVEL)
            .max(WATER_LEVEL);
        let depth = surface - floor;
        if depth > 1.0 {
            motion.swim = (motion.swim + dt * 4.0).min(1.0);
        } else {
            motion.swim = (motion.swim - dt * 4.0).max(0.0);
        }
        if motion.swim > 0.5 {
            transform.translation.y = surface - 0.45;
        } else if depth > 0.0 {
            // Wading: the feet find the seabed and the water climbs the
            // body, instead of the old film-walking on the surface.
            transform.translation.y = floor;
        } else {
            transform.translation.y = floor;
        }

        motion.speed = speed;
    }
}

/// A body mid-flash: how long remains, and every part's true material so
/// the red can be taken back off.
#[derive(Component)]
pub struct HurtFlash {
    remaining: f32,
    restore: Vec<(Entity, Handle<StandardMaterial>)>,
}

/// The one shared flash material.
#[derive(Resource)]
struct FlashMaterial(Handle<StandardMaterial>);

/// Fresh harm turns the body itself red for a beat: every part's material
/// swaps to the flash and swaps back. Corpses tick too - a killing blow
/// still flashes, then lets the dead lie in their own colors.
#[allow(clippy::type_complexity)]
fn hurt_flashes(
    mut commands: Commands,
    time: Res<Time>,
    flash_material: Option<Res<FlashMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    children: Query<&Children>,
    parts: Query<&MeshMaterial3d<StandardMaterial>>,
    mut hurt: Query<(Entity, &mut Vitality, Option<&mut HurtFlash>), With<Creature>>,
) {
    let red = match flash_material {
        Some(handle) => handle.0.clone(),
        None => {
            let handle = materials.add(StandardMaterial {
                base_color: crate::palette::shade(&crate::palette::CLOTH_RED, 0.9),
                emissive: LinearRgba::from(crate::palette::shade(&crate::palette::CLOTH_RED, 0.9))
                    * 6.0,
                ..default()
            });
            commands.insert_resource(FlashMaterial(handle.clone()));
            handle
        }
    };

    let dt = time.delta_secs();
    for (entity, mut vitality, flash) in &mut hurt {
        let harm = vitality.harm;
        let fresh = harm > vitality.last_harm + 0.01;
        vitality.last_harm = harm;
        match flash {
            Some(mut flash) => {
                if fresh {
                    flash.remaining = 0.18;
                } else {
                    flash.remaining -= dt;
                    if flash.remaining <= 0.0 {
                        for (part, original) in flash.restore.drain(..) {
                            if let Ok(mut e) = commands.get_entity(part) {
                                e.insert(MeshMaterial3d(original));
                            }
                        }
                        commands.entity(entity).remove::<HurtFlash>();
                    }
                }
            }
            None if fresh => {
                let mut restore = Vec::new();
                for part in children.iter_descendants(entity) {
                    if let Ok(material) = parts.get(part) {
                        restore.push((part, material.0.clone()));
                        commands.entity(part).insert(MeshMaterial3d(red.clone()));
                    }
                }
                if !restore.is_empty() {
                    commands.entity(entity).insert(HurtFlash {
                        remaining: 0.18,
                        restore,
                    });
                }
            }
            None => {}
        }
    }
}

/// What the wild leaves unclaimed does not lie there forever: an animal's
/// carcass keeps a while for hunters and wolves, then sinks back into the
/// ground. Villagers are never carrion - their dead get rites.
#[derive(Component)]
pub struct Carrion {
    pub remaining: f32,
}

#[allow(clippy::type_complexity)]
fn carrion_fades(
    mut commands: Commands,
    time: Res<Time>,
    fresh: Query<
        Entity,
        (
            With<Corpse>,
            With<Creature>,
            Without<crate::villager::Villager>,
            Without<Carrion>,
        ),
    >,
    mut fading: Query<(Entity, &mut Carrion, &mut Transform), Without<Held>>,
) {
    for corpse in &fresh {
        commands.entity(corpse).insert(Carrion { remaining: 210.0 });
    }
    let dt = time.delta_secs();
    for (entity, mut carrion, mut transform) in &mut fading {
        carrion.remaining -= dt;
        if carrion.remaining < 6.0 {
            // The last moments: settling into the earth.
            transform.translation.y -= dt * 0.14;
        }
        if carrion.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// People cannot swim. Deep water is a slow emergency: they thrash toward
/// the nearest dry ground while the water takes its toll — and a god who
/// drops someone in the sea has done exactly what it looks like. Animals
/// paddle without drama.
#[allow(clippy::type_complexity)]
fn drowning(
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut say_timer: Local<f32>,
    mut telling: (
        Option<ResMut<crate::sermo::Tongue>>,
        Option<Res<crate::attention::Attention>>,
    ),
    mut rng: Local<Option<crate::rng::Rng>>,
    mut swimmers: Query<
        (
            Entity,
            &Transform,
            &CreatureMotion,
            &mut Vitality,
            &mut MoveTarget,
        ),
        (
            With<crate::villager::Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs();
    *say_timer += dt;
    let rng = rng.get_or_insert_with(|| crate::rng::Rng::new(0x5EA));

    for (entity, at, motion, mut vitality, mut target) in &mut swimmers {
        if motion.swim < 0.5 {
            continue;
        }
        // The water wins slowly enough for a god to intervene.
        vitality.harm = (vitality.harm + dt / 14.0).min(1.0);

        // Thrash toward the nearest dry ground.
        let needs_course = target
            .0
            .is_none_or(|goal| !terrain.is_walkable(goal.x, goal.z));
        if needs_course {
            let mut best: Option<(f32, Vec3)> = None;
            for step in 0..12 {
                let angle = step as f32 / 12.0 * std::f32::consts::TAU;
                let (sin, cos) = angle.sin_cos();
                for reach in [10.0_f32, 22.0, 40.0, 70.0] {
                    let x = at.translation.x + cos * reach;
                    let z = at.translation.z + sin * reach;
                    if terrain.is_walkable(x, z)
                        && terrain.height_at(x, z) > WATER_LEVEL + 0.5
                        && best.is_none_or(|(d, _)| reach < d)
                    {
                        best = Some((reach, Vec3::new(x, terrain.height_at(x, z), z)));
                        break;
                    }
                }
            }
            if let Some((_, shore)) = best {
                target.0 = Some(shore);
            }
        }

        if *say_timer > 6.0 && rng.chance(0.3) {
            *say_timer = 0.0;
            // A drowning cry, picked on the spot; the showing decides who
            // hears it.
            if let Some(tongue) = telling.0.as_mut() {
                tongue.muse(crate::sermo::Musing {
                    who: entity,
                    voice: None,
                    faith: crate::sermo::FaithBand::Wavering,
                    body: vec!["drowning"],
                    heard: None,
                    aloud: true,
                    about: None,
                });
            }
        }
    }
}

/// Bodies take up room: any two grounded creatures standing inside each
/// other's space are eased apart. Not physics — just the polite firmness of
/// shoulders in a crowd, enough that a conversation is two people and not
/// one strange chimera.
#[allow(clippy::type_complexity)]
fn keep_apart(
    time: Res<Time>,
    mut creatures: Query<
        (
            Entity,
            &mut Transform,
            &CreatureGenome,
            Option<&crate::villager::Activity>,
        ),
        (
            With<Creature>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let bodies: Vec<(Entity, Vec3, f32)> = creatures
        .iter()
        .filter(|(_, _, _, activity)| {
            !matches!(activity, Some(crate::villager::Activity::Sleeping))
        })
        .map(|(entity, transform, genome, _)| {
            (entity, transform.translation, genome.height() * 0.21)
        })
        .collect();

    // The ease is a RATE, not a per-frame fraction: framerate must not
    // change how firm a shoulder is.
    let ease = (time.delta_secs() * 14.0).min(0.5);

    let mut nudges: Vec<(Entity, Vec3)> = Vec::new();
    for (i, (a, at_a, r_a)) in bodies.iter().enumerate() {
        for (b, at_b, r_b) in bodies.iter().skip(i + 1) {
            let min = r_a + r_b;
            let mut between = *at_a - *at_b;
            between.y = 0.0;
            let d2 = between.length_squared();
            if d2 >= min * min {
                continue;
            }
            let d = d2.sqrt();
            // Two bodies on the SAME SPOT have no honest direction apart,
            // and a direction recomputed from noise every frame is a
            // violent vibration - Brett watched two neighbors buzz
            // against each other like flies. The tie is broken by WHO
            // they are instead: stable across frames, so a stack slides
            // apart in one smooth motion.
            let apart = if d > 0.05 {
                between / d
            } else {
                let angle = (a.to_bits() ^ b.to_bits().rotate_left(17)) as f32 * 0.0001;
                Vec3::new(angle.cos(), 0.0, angle.sin())
            };
            let push = apart * (min - d).max(0.0) * ease;
            nudges.push((*a, push));
            nudges.push((*b, -push));
        }
    }
    for (entity, push) in nudges {
        if let Ok((_, mut transform, _, _)) = creatures.get_mut(entity) {
            transform.translation += push;
        }
    }
}

/// Integrates thrown and dropped creatures, and lands them.
fn apply_ballistics(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut witnessed: MessageWriter<crate::witness::DivineEvent>,
    mut airborne: Query<(
        Entity,
        &mut Transform,
        &mut Airborne,
        Option<&mut CreatureMotion>,
        Option<&mut Vitality>,
        &CreatureGenome,
    )>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs();

    for (entity, mut transform, mut body, motion, vitality, genome) in &mut airborne {
        body.velocity.y -= GRAVITY * dt;
        transform.translation += body.velocity * dt;

        // Tumble while falling. Rotation rate follows speed, so a gentle drop is a
        // gentle turn and a hard throw is a spin.
        let spin = body.velocity.length() * 0.06 * dt;
        transform.rotate_local_x(spin);
        transform.rotate_y(spin * 0.6);

        if let Some(mut motion) = motion {
            motion.flail = 1.0;
            motion.speed = 0.0;
        }

        let ground = terrain
            .stand_height_at(transform.translation.x, transform.translation.z)
            .max(WATER_LEVEL);

        if transform.translation.y <= ground {
            transform.translation.y = ground;

            // Land upright. Whatever tumble the creature picked up is discarded,
            // keeping the yaw so it faces wherever it was thrown.
            let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            transform.rotation = Quat::from_rotation_y(yaw);

            // Keep them shaken for a moment after impact, scaled by how hard they hit.
            let impact = (-body.velocity.y / 18.0).clamp(0.0, 1.0);

            // Falls have consequences now. This is where a throw becomes a killing.
            if let Some(mut vitality) = vitality {
                let harm = impact_harm(impact);
                if harm > 0.0 {
                    vitality.harm = (vitality.harm + harm).min(1.0);
                    vitality.violent = true;
                    vitality.undoing = Undoing::Fall;
                }
            }

            commands.entity(entity).remove::<Airborne>();

            // Anything dropped hard enough forgets where it was going.
            if impact > 0.3 {
                commands.entity(entity).insert(MoveTarget(None));

                // A hard landing is its own event, separate from the throw: people
                // out of sight of the launch may still see where it came down.
                witnessed.write(crate::witness::DivineEvent {
                    kind: crate::witness::DivineEventKind::Impact,
                    position: transform.translation,
                    subject: Some(entity),
                    intensity: impact,
                });
            }

            let _ = genome;
        }
    }
}

/// Rotation that points a creature's front along `direction`.
///
/// Bodies are built facing -Z, which is Bevy's forward. Deriving the angle as
/// `atan2(x, z)` instead aims +Z along the travel direction, which turns every
/// creature through 180 degrees and has the whole settlement walking backwards.
/// `looking_to` states the intent rather than restating the trigonometry.
pub fn facing_rotation(direction: Vec3) -> Quat {
    let flat = Vec3::new(direction.x, 0.0, direction.z);
    if flat.length_squared() < 1e-8 {
        return Quat::IDENTITY;
    }
    Transform::default().looking_to(flat, Vec3::Y).rotation
}

/// Ends creatures whose harm has run its course.
///
/// The entity is not despawned: it keeps its body, its name and its grabbability,
/// loses everything that made it *do* — and lies down. The dead stay in the world,
/// because a death nobody can see is a stat, and this game is about what people
/// make of what they see.
fn succumb(
    mut commands: Commands,
    mut died: MessageWriter<CreatureDied>,
    mut creatures: Query<
        (
            Entity,
            &mut Transform,
            &Vitality,
            Option<&crate::villager::Person>,
            Option<&body::CreatureRig>,
        ),
        (
            With<Creature>,
            Without<Corpse>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
    // Body nodes carry no Creature of their own, which keeps this second
    // transform query disjoint from the one above.
    mut parts: Query<&mut Transform, Without<Creature>>,
) {
    for (entity, mut transform, vitality, person, rig) in &mut creatures {
        if vitality.harm < 1.0 {
            continue;
        }

        commands
            .entity(entity)
            .remove::<crate::villager::Villager>()
            .remove::<crate::villager::Needs>()
            .remove::<crate::villager::Activity>()
            .remove::<crate::witness::Reaction>()
            .remove::<MoveTarget>()
            .remove::<Route>()
            .remove::<CreatureMotion>()
            .remove::<Laden>()
            .insert(Corpse);

        // Laid on their side, keeping the way they were facing.
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        transform.rotation =
            Quat::from_rotation_y(yaw) * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);

        // The animator stops with CreatureMotion gone, so whatever it last
        // wrote to the body node freezes in. Frozen limbs are the charm of a
        // death mid-stride; a frozen KNEEL SINK is a corpse lying a third of
        // a meter from its own root, where the hand, the bearers and the
        // grave all reach. The body node stands back up before the fall.
        if let Some(rig) = rig
            && let Ok(mut body) = parts.get_mut(rig.body)
        {
            *body = Transform::default();
        }

        let name = person.map(|p| p.name.clone());
        match &name {
            // Named, so the log says what the village is actually losing
            // people to. "died violently" covered a wolf, a fall, a
            // lightning strike and a hunter's spear alike.
            Some(name) => info!(
                "{name} {}",
                if vitality.violent {
                    vitality.undoing.how()
                } else {
                    "starved"
                }
            ),
            None => info!("a creature has died"),
        }

        died.write(CreatureDied {
            entity,
            position: transform.translation,
            name,
            violent: vitality.violent,
        });
    }
}

/// Picks a random walkable point within `radius` of `origin`.
///
/// Returns `None` if no walkable point turned up, which the caller should treat as
/// "stay put this tick" rather than as an error.
pub fn random_walkable_point(
    terrain: &Terrain,
    rng: &mut Rng,
    origin: Vec3,
    radius: f32,
) -> Option<Vec3> {
    random_walkable_ring(terrain, rng, origin, 0.0, radius)
}

/// [`random_walkable_point`], held outside an inner ring.
///
/// The disc sampler runs from NOUGHT: "within ninety units" includes the
/// middle of the village square, which is how a founding world came to deal
/// a wolf pack onto the doorstep. Anything that must start at arm's length -
/// predators, above all - asks for a floor as well as a ceiling.
pub fn random_walkable_ring(
    terrain: &Terrain,
    rng: &mut Rng,
    origin: Vec3,
    nearest: f32,
    radius: f32,
) -> Option<Vec3> {
    for _ in 0..24 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        // Area-uniform over the ring, so a wide ring does not bunch its
        // spawns against the inner edge.
        let (r0, r1) = (nearest * nearest, radius * radius);
        let distance = (r0 + (r1 - r0) * rng.f32()).sqrt();
        let x = origin.x + angle.cos() * distance;
        let z = origin.z + angle.sin() * distance;

        if terrain.is_walkable(x, z) {
            return Some(Vec3::new(x, terrain.height_at(x, z), z));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::Terrain;

    #[test]
    fn random_points_are_walkable_and_within_radius() {
        let terrain = Terrain::new(2024);
        let mut rng = Rng::new(1);

        // Start from a point known to be walkable.
        let mut origin = None;
        'outer: for iz in 0..96 {
            for ix in 0..96 {
                let x = ix as f32 * 40.0 - 1920.0;
                let z = iz as f32 * 40.0 - 1920.0;
                if terrain.is_walkable(x, z) {
                    origin = Some(Vec3::new(x, terrain.height_at(x, z), z));
                    break 'outer;
                }
            }
        }
        let origin = origin.expect("terrain has nowhere to stand");

        let mut found = 0;
        for _ in 0..200 {
            if let Some(p) = random_walkable_point(&terrain, &mut rng, origin, 12.0) {
                assert!(terrain.is_walkable(p.x, p.z));
                let flat = Vec2::new(p.x - origin.x, p.z - origin.z).length();
                assert!(flat <= 12.0 + 1e-3);
                found += 1;
            }
        }
        assert!(
            found > 100,
            "wander target search failed too often: {found}"
        );
    }

    #[test]
    fn the_mortally_harmed_become_corpses_exactly_once() {
        let mut app = App::new();
        app.add_message::<CreatureDied>();
        app.add_systems(Update, succumb);

        let doomed = app
            .world_mut()
            .spawn((
                Creature,
                Transform::default(),
                Vitality {
                    harm: 1.0,
                    violent: true,
                    ..default()
                },
            ))
            .id();
        let healthy = app
            .world_mut()
            .spawn((Creature, Transform::default(), Vitality::default()))
            .id();

        app.update();

        assert!(app.world().get::<Corpse>(doomed).is_some(), "did not die");
        assert!(app.world().get::<Corpse>(healthy).is_none(), "died healthy");

        // Lying down, not standing.
        let pose = app.world().get::<Transform>(doomed).unwrap();
        let up = pose.rotation * Vec3::Y;
        assert!(up.y < 0.3, "the dead are still standing");

        // Dying twice would double-announce to every witness.
        app.update();
        let deaths = app.world().resource::<Messages<CreatureDied>>();
        assert_eq!(deaths.len(), 1, "announced {} deaths", deaths.len());
    }

    #[test]
    fn gentle_landings_are_harmless_and_hurled_ones_are_not() {
        assert_eq!(impact_harm(0.0), 0.0);
        assert_eq!(impact_harm(0.45), 0.0, "the harm threshold moved");
        assert!(impact_harm(0.6) > 0.0);

        // Two full-strength hurls should be lethal; one should not.
        let full = impact_harm(1.0);
        assert!(full < 1.0, "a single throw kills outright");
        assert!(full * 2.0 >= 1.0, "throws are toothless");
    }

    #[test]
    fn creatures_face_the_way_they_walk() {
        // Regression: the facing angle was derived so that +Z aimed along travel,
        // but bodies are built facing -Z. Everyone walked backwards.
        for (x, z) in [
            (1.0f32, 0.0f32),
            (-1.0, 0.0),
            (0.0, 1.0),
            (0.0, -1.0),
            (0.7, 0.7),
            (-0.3, 0.95),
        ] {
            let direction = Vec3::new(x, 0.0, z).normalize();
            let rotation = facing_rotation(direction);

            // The model's front is its local -Z.
            let front = rotation * Vec3::NEG_Z;
            assert!(
                front.distance(direction) < 1e-4,
                "facing {front:?} for travel {direction:?}",
            );
        }
    }

    #[test]
    fn facing_ignores_vertical_travel_and_survives_standing_still() {
        let rotation = facing_rotation(Vec3::new(0.0, 5.0, -2.0));
        let front = rotation * Vec3::NEG_Z;
        assert!(
            front.y.abs() < 1e-5,
            "creature tipped out of the ground plane"
        );

        // A zero direction must not produce a NaN rotation.
        assert!(facing_rotation(Vec3::ZERO).is_near_identity());
    }

    #[test]
    fn gravity_pulls_a_throw_back_down() {
        // A launched creature must come down within a sensible time, otherwise a
        // missed throw could strand a villager in the air indefinitely.
        let mut y: f32 = 10.0;
        let mut vy: f32 = 12.0;
        let dt = 1.0 / 60.0;
        let mut landed = false;

        for _ in 0..600 {
            vy -= GRAVITY * dt;
            y += vy * dt;
            if y <= 0.0 {
                landed = true;
                break;
            }
        }
        assert!(landed, "thrown creature never came down");
    }

    /// The frame-time bug, as a test.
    ///
    /// Measured before the fix: `creature: plan_routes` at 76ms of an 81ms
    /// frame, held there for thirty seconds. The cause was not the search being
    /// slow — it was the same failed search being run again every single frame,
    /// because `locomotion` reads `unreachable`, abandons the errand and calls
    /// `clear`, which puts `unreachable` back to false. Whatever wanted to go
    /// there asked again on the next frame and got the most expensive answer
    /// the pathfinder has: a full three-thousand-node expansion ending in
    /// "no".
    #[test]
    fn a_place_that_cannot_be_reached_is_not_asked_about_every_frame() {
        let mut route = Route::default();
        let nowhere = Vec3::new(900.0, 12.0, -400.0);

        // The search fails, and the refusal is remembered.
        route.unreachable = true;
        route.denied = Some((nowhere, 0.0));

        // What `locomotion` does with a failed route. This is the step that
        // used to undo the guard.
        route.clear();

        assert!(
            route.still_refused(nowhere, 0.0),
            "the refusal did not survive the abandonment, so the next frame \
             will pay for the whole search again - which IS the bug"
        );
        // And the same errand a moment later, which is what a villager who has
        // not changed their mind actually asks.
        assert!(route.still_refused(nowhere, GIVE_UP_FOR * 0.5));
    }

    #[test]
    fn a_refusal_is_forgotten_and_is_only_about_that_one_place() {
        let mut route = Route::default();
        let nowhere = Vec3::new(900.0, 12.0, -400.0);
        route.denied = Some((nowhere, 0.0));

        // The world changes - a bridge, a hall, a felled tree opening a way -
        // so this forgets rather than bans.
        assert!(
            !route.still_refused(nowhere, GIVE_UP_FOR + 0.1),
            "a place refused once is refused for ever, and the way there may \
             have opened since"
        );
        // And somewhere else entirely was never refused at all, however near
        // in time.
        assert!(
            !route.still_refused(nowhere + Vec3::new(80.0, 0.0, 0.0), 0.0),
            "one unreachable place is holding up errands to every other"
        );
    }
}
