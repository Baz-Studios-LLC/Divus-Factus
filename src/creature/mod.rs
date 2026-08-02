//! Creatures: generation, locomotion and the small amount of physics they need.
//!
//! There is no physics engine here. The only bodies that ever leave the ground are
//! ones the player has picked up and thrown, and a ballistic arc plus a terrain
//! height check covers that completely. Adding a rigid-body crate to solve a
//! problem this size would cost far more in dependency churn than it returns.

pub mod anim;
pub mod body;
pub mod genome;
pub mod wildlife;

use bevy::prelude::*;

use crate::rng::Rng;
use crate::terrain::{Terrain, WATER_LEVEL};
use anim::CreatureMotion;
use body::{CreatureAssets, build_body, init_creature_assets};
use genome::{CreatureGenome, Species};

/// Gravity, in world units per second squared. Tuned for how a throw *reads*
/// rather than for realism: real gravity makes tossed villagers land too fast to
/// follow at this camera distance.
const GRAVITY: f32 = 19.6;

/// Routes computed per frame across the whole world.
const ROUTES_PER_FRAME: usize = 4;

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
                    wildlife::wild_breeding,
                    wildlife::wild_growth,
                    succumb,
                    anim::advance_motion,
                    anim::animate_creatures,
                )
                    .chain()
                    .in_set(CreatureSet),
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
/// Held separately from `MoveTarget` so behaviour code keeps saying *where* it wants
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
}

impl Route {
    /// The next point to walk toward.
    pub fn next(&self) -> Option<Vec3> {
        self.waypoints.first().copied()
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
    // Picking uses a sphere centred on the creature's midpoint rather than its feet,
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
fn plan_routes(
    terrain: Option<Res<Terrain>>,
    mut creatures: Query<
        (&Transform, &MoveTarget, &mut Route),
        (With<Creature>, Without<Held>, Without<Airborne>),
    >,
) {
    let Some(terrain) = terrain else {
        return;
    };
    // The budget grows with the crowd: four routes a frame was sized for
    // a hamlet, and a city of eighty would starve for paths and stand
    // still in the street.
    let mut budget = ROUTES_PER_FRAME.max(creatures.iter().count() / 8);

    for (transform, target, mut route) in &mut creatures {
        let Some(goal) = target.0 else {
            if route.goal.is_some() {
                route.clear();
            }
            continue;
        };

        // Already routed there.
        if route.goal.is_some_and(|g| g.distance_squared(goal) < 0.25) {
            continue;
        }
        if budget == 0 {
            continue;
        }
        budget -= 1;

        route.goal = Some(goal);
        match crate::navigation::find_path(
            &terrain,
            transform.translation,
            goal,
            crate::navigation::DEFAULT_BUDGET,
        ) {
            Some(waypoints) => {
                route.waypoints = waypoints;
                route.unreachable = false;
            }
            None => {
                route.waypoints.clear();
                route.unreachable = true;
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
        ),
        (
            With<Creature>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs();

    for (genome, mut transform, mut target, mut motion, mut route, vitality) in &mut creatures {
        let mut speed = 0.0;
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
                speed = genome.walk_speed() * approach * vigor * paved;

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
/// still flashes, then lets the dead lie in their own colours.
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
        Option<ResMut<crate::telling::Tongue>>,
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
            // A drowning cry in their own words — urgent enough that the
            // teller answers within a second, and honest silence otherwise.
            if let Some(tongue) = telling.0.as_mut()
                && crate::attention::regard(telling.1.as_deref(), at.translation).worth_composing()
            {
                tongue.muse(crate::telling::Musing {
                    who: entity,
                    voice: None,
                    bearing: crate::villager::traits::Bearing::Plain,
                    faith: crate::telling::FaithBand::Wavering,
                    body: vec!["drowning"],
                    place: Vec::new(),
                    mind: "you are in deep water and cannot swim — cry for help".into(),
                    heard: None,
                    aloud: true,
                    prayer: false,
                    known: Vec::new(),
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
            let d = d2.sqrt().max(0.01);
            // Ease, not teleport: half the overlap each, softened, so a
            // crowd settles instead of popping.
            let push = between / d * (min - d) * 0.25;
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
        // a metre from its own root, where the hand, the bearers and the
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
    for _ in 0..24 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let distance = radius * rng.f32().sqrt();
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
}
