//! Hearth and home: the village fire, and where people sleep.
//!
//! The fire is the village's heart and its first standing obligation: it burns
//! timber from the stockpile, someone has to carry wood to it, and a village
//! that lets it die spends the night in the dark. That makes darkness a
//! *consequence* rather than a shader setting — and the lit circle around the
//! fire is where the homeless gather, which is how the player sees at a glance
//! who has a roof and who does not.
//!
//! At night, people with homes walk to them and sleep inside; at dawn they
//! step back out. A sleeping village is quiet, dark but for the fire, and
//! plucking a sleeper out of their house is exactly the kind of thing a god
//! can do and neighbours will talk about.

use bevy::prelude::*;

use super::work::{Bed, Hut, Longhouse, Stockpile};
use super::{Activity, Chronicle, Needs, Parentage, Person, SimRng, Spouse, Villager};
use crate::creature::anim::CreatureMotion;
use crate::creature::genome::{Age, CreatureGenome};
use crate::creature::{Airborne, Childhood, Corpse, Held, MoveTarget};
use crate::palette;

/// Seconds of burning one log buys.
const SECONDS_PER_LOG: f32 = 75.0;

/// Below this much burn-time left, someone goes for wood.
const LOW_FUEL: f32 = 45.0;

/// How many people one house sleeps: a couple and their children.
pub const HOUSE_CAPACITY: usize = 4;

/// How many the longhouse sleeps. Twice a house, and none of them kin.
pub const LONGHOUSE_CAPACITY: usize = 8;

/// How many can sleep rough in the fire's circle before the village is
/// genuinely overfull. The founders' allowance, roughly.
pub const FIRE_CIRCLE_SHELTER: usize = 8;

/// How many people the village can shelter at all.
pub fn shelter_capacity(houses: usize, longhouses: usize) -> usize {
    FIRE_CIRCLE_SHELTER + houses * HOUSE_CAPACITY + longhouses * LONGHOUSE_CAPACITY
}

/// Whether this person belongs under a family roof rather than in the
/// longhouse: the wed, and every child.
///
/// Widowhood does not evict anyone — `Spouse` outlives its person on
/// purpose, so a widow keeps the house she was married into. Coming of age
/// does: an adult with no spouse of their own has no claim on a family's
/// room, whoever their parents are.
pub fn wants_family_roof(spouse: Option<&Spouse>, child: bool) -> bool {
    spouse.is_some() || child
}

/// Whether someone should give up the bed they have.
///
/// Two conditions, and the second is the one that is easy to get backwards:
/// they must be under the wrong roof, and the roof they are moving *to* must
/// have room. Checking the roof they are leaving instead strands people in
/// the square with a bed waiting for them.
fn should_rehome(wants_longhouse: bool, in_longhouse: bool, room_in_wanted: bool) -> bool {
    wants_longhouse != in_longhouse && room_in_wanted
}

/// The village fire: fuel in seconds of burning, and who is off fetching wood
/// for it, so the whole village does not converge on one log.
#[derive(Component, Debug)]
pub struct Bonfire {
    pub fuel: f32,
    pub tender: Option<Entity>,
}

/// The flame meshes, for flickering.
#[derive(Component)]
pub struct Flame;

/// The light the fire casts.
#[derive(Component)]
pub struct Firelight;

/// When real rain sets in, the housed step indoors and wait it out, and
/// the roofless crowd the fire. Work goes on - slower, already taxed by
/// the weather - but idle hands do not stand in a downpour for nothing.
#[allow(clippy::type_complexity)]
/// A claimed berth: which numbered bed in their home is THEIRS.
///
/// Assigned when a home is claimed and reassigned when it changes; the
/// number maps to a physical [`Bed`] child of the building, so sleep is a
/// walk to a real mattress instead of a vanishing act at the door.
#[derive(Component, Debug, Clone, Copy)]
pub struct BedSlot(pub u8);

/// Lying in bed: the pose held until morning. Enforced every frame, because
/// the idle animation would otherwise stand the sleeper back up.
#[derive(Component)]
pub struct Abed {
    pub at: Vec3,
    pub facing: Quat,
}

/// Bends any walk that crosses a home's walls through its doorway.
///
/// No collision, no navmesh: the only solid architecture a villager enters
/// is a home, and a home has one wall of doors. A walker whose goal is on
/// the other side of the shell aims first for the outside of the nearest
/// door, then the inside of it, then their true goal — and because every
/// target-setter runs each frame, this runs after them and quietly rewrites
/// the leg without any of them knowing.
pub(super) fn use_doors(
    shells: Query<(&Transform, &super::work::Shell), Without<Villager>>,
    mut walkers: Query<
        (&Transform, &mut MoveTarget),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
            Without<Abed>,
        ),
    >,
) {
    for (at, mut target) in &mut walkers {
        let Some(goal) = target.0 else {
            continue;
        };
        for (site, shell) in &shells {
            let inv = site.rotation.inverse();
            let me = inv * (at.translation - site.translation);
            let to = inv * (goal - site.translation);
            let inside =
                |p: Vec3| p.x.abs() < shell.half_w + 0.15 && p.z.abs() < shell.half_d + 0.15;
            let (im, it) = (inside(me), inside(to));
            if im == it {
                continue;
            }
            // The nearest doorway, wherever the maker put it, and the
            // two standing places either side of it.
            let here = Vec2::new(me.x, me.z);
            let Some(door) = shell
                .doors
                .iter()
                .min_by(|a, b| a.at.distance(here).total_cmp(&b.at.distance(here)))
            else {
                continue;
            };
            // Far enough out that the two standing places straddle the
            // shell without argument: a doorway sits in the thickness of a
            // wall, not on the line the shell is measured to, and a step
            // too short puts BOTH places on the same side of it.
            let step = door.out * 1.6;
            let outer = Vec3::new(door.at.x + step.x, 0.0, door.at.y + step.y);
            let inner = Vec3::new(door.at.x - step.x, 0.0, door.at.y - step.y);
            let near = if im { inner } else { outer };
            let far = if im { outer } else { inner };
            let leg = if (me - near).with_y(0.0).length() < 1.1 {
                far
            } else {
                near
            };
            let world = site.translation + site.rotation * leg;
            target.0 = Some(Vec3::new(world.x, goal.y, world.z));
            break;
        }
    }
}

/// Deals every housed villager a numbered berth, lowest free first.
/// Runs on anyone whose home just changed, so a rehoming re-deals.
pub(super) fn assign_beds(
    mut commands: Commands,
    movers: Query<
        (
            Entity,
            &Home,
            Has<crate::creature::Childhood>,
            Option<&super::Spouse>,
        ),
        (With<Villager>, Changed<Home>),
    >,
    held: Query<(Entity, &Home, &BedSlot), With<Villager>>,
    kinds: Query<Has<Longhouse>, Or<(With<Hut>, With<Longhouse>)>>,
    beds: Query<(&ChildOf, &Bed)>,
) {
    // A finished house is claimed by its whole household in ONE frame, and
    // command-inserted slots are not visible until the next — so the deals
    // made THIS run must be remembered here, or everyone sees an empty
    // house and takes bed zero together.
    let mut dealt: std::collections::HashMap<Entity, Vec<u8>> = std::collections::HashMap::new();
    // The wedded go first, so the pair has claimed the marriage bed
    // before the household's singles spread across the room.
    let mut queue: Vec<_> = movers.iter().collect();
    queue.sort_by_key(|(_, _, _, spouse)| spouse.is_none());
    for (mover, home, child, spouse) in queue {
        if kinds.get(home.0).is_err() {
            continue;
        }
        // Every sleeping place under this roof. A marriage bed is two of
        // them lying alongside each other, so each half is a place of its
        // own and the pair simply takes both.
        let slots: Vec<(u8, bool)> = beds
            .iter()
            .filter(|(parent, _)| parent.parent() == home.0)
            .map(|(_, bed)| (bed.slot, bed.double))
            .collect();
        let mut taken: Vec<u8> = held
            .iter()
            .filter(|(other, theirs, _)| *other != mover && theirs.0 == home.0)
            .map(|(_, _, slot)| slot.0)
            .collect();
        taken.extend(dealt.get(&home.0).into_iter().flatten().copied());
        let free = |wedded: bool, taken: &[u8]| {
            slots
                .iter()
                .filter(|(slot, double)| *double == wedded && !taken.contains(slot))
                .map(|(slot, _)| *slot)
                .min()
        };
        let mut deal = |slot: u8, taken: &mut Vec<u8>| {
            taken.push(slot);
            dealt.entry(home.0).or_default().push(slot);
            commands.entity(mover).insert(BedSlot(slot));
        };

        // A married grown-up takes a half of the marriage bed; the wedded
        // are dealt first, so their spouse finds the other half waiting.
        if !child
            && spouse.is_some()
            && let Some(slot) = free(true, &taken)
        {
            deal(slot, &mut taken);
            continue;
        }
        // Everyone else: the lowest free bed that is NOT a marriage bed.
        if let Some(slot) = free(false, &taken) {
            deal(slot, &mut taken);
            continue;
        }
        // Overflow: a grown-up alone may still take half a marriage bed.
        // A child may not — that bed is not theirs whatever the shortage,
        // and the hearth is warm enough.
        if !child && let Some(slot) = free(true, &taken) {
            deal(slot, &mut taken);
        }
    }
}

/// How far in front of a sleeping place its pillow lies. The bench's own
/// sleeping figure carries its head exactly this far ahead of the mark it
/// is placed on, so a villager whose head lands here lands on the pillow
/// the maker drew - whatever size that villager happens to be.
const PILLOW_AHEAD: f32 = 0.625;

/// Where to stand a body's ROOT so that its HEAD comes down on `ahead`
/// metres in front of the place it was told to sleep.
///
/// A villager's root is between their feet: standing, all of them is
/// above it, but tipped onto their back they reach OUT from it, head
/// first, a whole body's length. A root left on the mark therefore put
/// their feet on the pillow and the rest of them off the end of the bed.
///
/// The head is the part that has to land somewhere in particular, so the
/// head is what this solves for and the rest of the body follows it back.
/// A tall sleeper's feet hang further down the mattress; their head still
/// finds the pillow.
fn laid_from(genome: &crate::creature::genome::CreatureGenome, facing: Quat, ahead: f32) -> Vec3 {
    // Which way the head lies: the body's own up, once it has been tipped.
    let head = facing * Vec3::Y;
    let flat = Vec3::new(head.x, 0.0, head.z).normalize_or(Vec3::X);
    // From the root to the middle of the head, along the body.
    let to_the_head = genome.height() * (1.0 - genome.proportions.head_size * 0.5);
    flat * (ahead - to_the_head) + Vec3::Y * (genome.thickness() * 0.5)
}

/// Holds sleepers to their mattresses — and is the ONE authority on
/// rising. Daybreak frees every held sleeper regardless of what some
/// other system set their activity to overnight; the first version only
/// released the still-Sleeping, and anyone whose activity had been
/// flipped meanwhile stayed nailed to the mattress for the rest of their
/// lives while the ledger showed a village of eleven with nobody working.
pub(super) fn hold_abed(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut sleepers: Query<(Entity, &Abed, &mut Transform, &mut CreatureMotion), With<Villager>>,
) {
    let night = clock.is_night();
    for (entity, abed, mut transform, mut motion) in &mut sleepers {
        if night {
            transform.translation = abed.at;
            transform.rotation = abed.facing;
        } else {
            commands.entity(entity).remove::<Abed>();
            transform.rotation = Quat::IDENTITY;
            transform.translation.y -= 0.3;
            motion.speed = 1.0;
        }
    }
}

pub(super) fn take_shelter(
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    homes: Query<&Transform, (Or<(With<Hut>, With<Longhouse>)>, Without<Villager>)>,
    fires: Query<&GlobalTransform, (With<Bonfire>, Without<Villager>)>,
    mut villagers: Query<
        (
            &Transform,
            Option<&Home>,
            &Needs,
            &mut Activity,
            &mut MoveTarget,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<crate::creature::Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let Some(weather) = weather else {
        return;
    };
    // Night hands everyone to the sleep routine instead.
    if clock.is_night() {
        return;
    }
    let pouring = weather.intensity > 0.6;
    let fire_pos = fires.iter().next().map(|f| f.translation());

    for (transform, home, needs, mut activity, mut target) in &mut villagers {
        // The starving do not wait out the rain: wet and fed beats dry
        // and dead, and the food systems own them until they have eaten.
        if needs.hunger > 0.7 {
            if *activity == Activity::Sheltering {
                *activity = Activity::Idle;
                target.0 = None;
            }
            continue;
        }
        if !pouring {
            if *activity == Activity::Sheltering {
                *activity = Activity::Idle;
                target.0 = None;
            }
            continue;
        }
        match *activity {
            Activity::Idle | Activity::Wandering | Activity::Sheltering => {}
            _ => continue,
        }
        match home.and_then(|h| homes.get(h.0).ok()) {
            Some(hut) => {
                // Inside means INSIDE now: stand under the actual roof,
                // visible to anyone who lifts it. No vanishing at the door.
                let indoors = hut.translation;
                if transform.translation.distance(indoors) > 1.4 {
                    *activity = Activity::Sheltering;
                    target.0 = Some(indoors);
                } else {
                    *activity = Activity::Sheltering;
                    target.0 = None;
                }
            }
            None => {
                // Nobody with no roof waits out the rain during working
                // hours. A ring of soaked people standing around a fire
                // they are not under is not shelter — it is the whole
                // workforce parked while the timber that would ROOF them
                // sits in the pile. Wet and building beats wet and idle.
                if clock.work_hours() {
                    if *activity == Activity::Sheltering {
                        *activity = Activity::Idle;
                        target.0 = None;
                    }
                    continue;
                }
                // Off the clock, the fire circle is the next best thing — a
                // spot in a loose RING around it, held as Sheltering. The
                // first version left them Idle at the fire's exact centre,
                // so the idle wander walked them ten feet off and this
                // system marched them straight back, all day, forever.
                if let Some(fire) = fire_pos {
                    let stand = fire
                        + (transform.translation - fire)
                            .with_y(0.0)
                            .normalize_or(Vec3::X)
                            * 3.2;
                    if transform.translation.distance(stand) > 1.6
                        && matches!(
                            *activity,
                            Activity::Idle | Activity::Wandering | Activity::Sheltering
                        )
                    {
                        *activity = Activity::Sheltering;
                        target.0 = Some(stand);
                    } else if matches!(*activity, Activity::Idle | Activity::Wandering) {
                        *activity = Activity::Sheltering;
                        target.0 = None;
                    }
                }
            }
        }
    }
}

/// Midday at the well: the idle drift over for water and talk. The well is
/// the daylight tavern - the place gossip crosses the village.
#[allow(clippy::type_complexity)]
pub(super) fn well_gatherings(
    clock: Res<crate::calendar::WorldClock>,
    mut rng: ResMut<super::SimRng>,
    wells: Query<(&GlobalTransform, &super::work::Building)>,
    mut idlers: Query<
        (&Transform, &Activity, &mut crate::creature::MoveTarget),
        (
            With<super::Villager>,
            Without<crate::creature::Corpse>,
            Without<crate::creature::Held>,
        ),
    >,
) {
    let hour = clock.time_of_day();
    if !(0.42..0.55).contains(&hour) {
        return;
    }
    let Some(well) = wells
        .iter()
        .find(|(_, b)| b.kind == super::work::BuildingKind::Well)
        .map(|(t, _)| t.translation())
    else {
        return;
    };
    for (at, activity, mut target) in &mut idlers {
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        let distance = at.translation.distance(well);
        if distance > 8.0 && distance < 80.0 && rng.0.chance(0.004) {
            target.0 = Some(well + Vec3::new(rng.0.range(-2.5, 2.5), 0.0, rng.0.range(-2.5, 2.5)));
        }
    }
}

/// Which house this person sleeps in.
#[derive(Component, Debug, Clone, Copy)]
pub struct Home(pub Entity);

/// Builds the village fire near the banner: a stone ring, crossed logs, and a
/// flame that is lit as long as there is fuel.
pub(super) fn spawn_bonfire(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
) -> Entity {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let stone = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::STONE, 0.5),
        perceptual_roughness: 0.95,
        ..default()
    });
    let log = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::WOOD, 0.35),
        perceptual_roughness: 0.95,
        ..default()
    });
    let flame_bright = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::CLOTH_GOLD, 0.95),
        emissive: LinearRgba::from(palette::shade(&palette::CLOTH_GOLD, 0.95)) * 14.0,
        ..default()
    });
    let flame_deep = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::CLOTH_RED, 0.8),
        emissive: LinearRgba::from(palette::shade(&palette::CLOTH_RED, 0.8)) * 9.0,
        ..default()
    });

    let fire = commands
        .spawn((
            Name::new("The village fire"),
            // Cold at the founding: a stone ring and stacked logs are a
            // promise, not a fire. Someone has to carry wood and light it —
            // the village's first chore is making its own hearth.
            Bonfire {
                fuel: 0.0,
                tender: None,
            },
            Transform::from_translation(at),
            Visibility::default(),
            crate::hand::PickRadius(2.0),
            crate::hand::Rooted,
        ))
        .id();

    // The stone ring.
    for step in 0..7 {
        let angle = step as f32 / 7.0 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(cos * 1.15, 0.14, sin * 1.15)
                .with_rotation(Quat::from_rotation_y(-angle))
                .with_scale(Vec3::new(0.5, 0.3, 0.32)),
            ChildOf(fire),
        ));
    }
    // Crossed logs.
    for (yaw, pitch) in [(0.4, 0.28), (1.7, 0.24), (2.9, 0.3)] {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(log.clone()),
            Transform::from_xyz(0.0, 0.28, 0.0)
                .with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_z(pitch))
                .with_scale(Vec3::new(1.5, 0.2, 0.2)),
            ChildOf(fire),
        ));
    }
    // The flame: two boxes, bright inside deep, scaled by the flicker system.
    commands.spawn((
        Flame,
        Mesh3d(cube.clone()),
        MeshMaterial3d(flame_deep),
        Transform::from_xyz(0.0, 0.75, 0.0).with_scale(Vec3::new(0.7, 1.0, 0.7)),
        bevy::light::NotShadowCaster,
        ChildOf(fire),
    ));
    commands.spawn((
        Flame,
        Mesh3d(cube),
        MeshMaterial3d(flame_bright),
        Transform::from_xyz(0.0, 0.9, 0.0).with_scale(Vec3::new(0.4, 0.8, 0.4)),
        bevy::light::NotShadowCaster,
        ChildOf(fire),
    ));
    // Its light.
    commands.spawn((
        Firelight,
        PointLight {
            color: palette::shade(&palette::CLOTH_GOLD, 0.9),
            intensity: 0.0,
            range: 42.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 1.6, 0.0),
        ChildOf(fire),
    ));

    fire
}

/// The fire burns down, and its flame and light follow the fuel.
pub(super) fn burn_weathered(
    weather: Option<Res<crate::weather::Weather>>,
    mut fires: Query<&mut Bonfire>,
    time: Res<Time>,
) {
    // The base burn happens in `burn`; weather adds its tax here, so the
    // two systems stay simple.
    let Some(weather) = weather else {
        return;
    };
    let extra = weather.intensity * 0.5;
    if extra <= 0.0 {
        return;
    }
    for mut fire in &mut fires {
        fire.fuel = (fire.fuel - time.delta_secs() * extra).max(0.0);
    }
}

pub(super) fn burn(
    time: Res<Time>,
    mut fires: Query<&mut Bonfire>,
    mut flames: Query<&mut Transform, (With<Flame>, Without<Bonfire>)>,
    mut lights: Query<&mut PointLight, With<Firelight>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    for mut fire in &mut fires {
        fire.fuel = (fire.fuel - dt).max(0.0);
        let lit = fire.fuel > 0.0;

        // Flicker: two incommensurate sines, so it never loops visibly.
        let flicker = 0.8 + 0.2 * ((t * 9.0).sin() * 0.6 + (t * 23.0).sin() * 0.4);
        let size = if lit { flicker } else { 0.001 };
        for mut flame in &mut flames {
            flame.scale.y = flame.scale.y.signum() * size.abs();
            let base = if flame.translation.y > 0.8 { 0.4 } else { 0.7 };
            flame.scale.x = base * size;
            flame.scale.z = base * size;
            flame.scale.y = (base + 0.35) * size;
        }
        for mut light in &mut lights {
            light.intensity = if lit { 1_600_000.0 * flicker } else { 0.0 };
        }
    }
}

/// When the fire runs low, someone fetches wood for it.
pub(super) fn tend_fire(
    clock: Res<crate::calendar::WorldClock>,
    mut notices: MessageWriter<crate::ui::Notice>,
    hearths: Query<&super::MemberOf>,
    mut stores: Query<&mut Stockpile>,
    mut fires: Query<(Entity, &GlobalTransform, &mut Bonfire)>,
    mut villagers: Query<
        (
            Entity,
            &Transform,
            &CreatureGenome,
            &mut Activity,
            &mut MoveTarget,
            Option<&super::MemberOf>,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    for (fire_entity, fire_at, mut fire) in &mut fires {
        let fire_pos = fire_at.translation();
        // Each hearth burns its own town's timber, and is tended by its own
        // town's people.
        let Some(town) = hearths.get(fire_entity).ok().map(|member| member.0) else {
            continue;
        };
        let Ok(mut store) = stores.get_mut(town) else {
            continue;
        };

        // Current tender walks the wood over and feeds the flame.
        if let Some(tender) = fire.tender {
            match villagers.get_mut(tender) {
                Ok((_, transform, _, mut activity, mut target, _))
                    if *activity == Activity::TendingFire =>
                {
                    if transform.translation.distance(fire_pos) > 2.4 {
                        target.0 = Some(fire_pos);
                        continue;
                    }
                    if store.timber >= 1.0 {
                        store.timber -= 1.0;
                        if fire.fuel <= 0.0 {
                            info!("the village fire was lit");
                            notices.write(crate::ui::Notice::new(
                                "The village fire is lit against the night",
                            ));
                        }
                        fire.fuel += SECONDS_PER_LOG;
                    }
                    *activity = Activity::Idle;
                    target.0 = None;
                    fire.tender = None;
                }
                // Died, got grabbed, or wandered off the task some other way.
                _ => fire.tender = None,
            }
            continue;
        }

        // Send for wood while there is wood to send for — but a fire is a
        // night thing: nobody stokes it under the noon sun, so it is built
        // toward dusk and burns down through the morning.
        let evening_on = clock.time_of_day() > 0.58 || clock.is_night();
        if evening_on && fire.fuel < LOW_FUEL && store.timber >= 1.0 {
            let volunteer = villagers
                .iter_mut()
                .filter(|(_, _, genome, activity, _, member)| {
                    genome.age == Age::Adult
                        && member.is_some_and(|m| m.0 == town)
                        && matches!(**activity, Activity::Idle | Activity::Wandering)
                })
                .map(|(entity, transform, _, activity, target, _)| {
                    (
                        entity,
                        transform.translation.distance(fire_pos),
                        activity,
                        target,
                    )
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((entity, _, mut activity, mut target)) = volunteer {
                *activity = Activity::TendingFire;
                target.0 = Some(fire_pos);
                fire.tender = Some(entity);
                let _ = fire_entity;
            }
        }
    }
}

/// People claim beds, nearest first — but not just any bed.
///
/// A house is a family's: the wed and their children. The longhouse takes
/// everyone else, which in practice means the village's young adults, in
/// the stretch of life between coming of age and marrying out of it. The
/// two roofs are not interchangeable, and which one someone walks into at
/// dusk says exactly where they stand in the village.
///
/// The one exception is the founding: before any longhouse stands, the
/// unattached take houses rather than sleep in the rain. `rehome_the_misplaced`
/// sorts them out the day a longhouse is finished.
#[allow(clippy::type_complexity)]
pub(super) fn assign_homes(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    roofs: Query<(Entity, &Transform, Has<Longhouse>), Or<(With<Hut>, With<Longhouse>)>>,
    tenants: Query<&Home>,
    mut homeless: Query<
        (
            Entity,
            &Transform,
            &Person,
            Option<&mut Chronicle>,
            Option<&Spouse>,
            Option<&Parentage>,
            Has<Childhood>,
        ),
        (With<Villager>, Without<Home>, Without<Corpse>),
    >,
) {
    if roofs.is_empty() {
        return;
    }

    let mut occupancy: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for home in &tenants {
        *occupancy.entry(home.0).or_default() += 1;
    }
    // Placements made this pass, so a couple widowed of their house — or
    // newly wed out of the longhouse — lands under ONE roof rather than two.
    // `commands` are deferred, so `tenants` cannot see them yet.
    let mut placed: std::collections::HashMap<Entity, Entity> = std::collections::HashMap::new();

    let capacity = |longhouse: bool| {
        if longhouse {
            LONGHOUSE_CAPACITY
        } else {
            HOUSE_CAPACITY
        }
    };
    let has_room =
        |roof: Entity, longhouse: bool, occupancy: &std::collections::HashMap<Entity, usize>| {
            occupancy.get(&roof).copied().unwrap_or(0) < capacity(longhouse)
        };
    for (entity, transform, person, chronicle, spouse, parentage, child) in &mut homeless {
        // The rule holds even at the founding, when no longhouse stands yet:
        // strangers do not move in with a family. An earlier version let the
        // unwed take houses until the first longhouse was finished, and
        // because a house costs half what a longhouse does it always finished
        // first — so every new game opened with a married couple and two
        // strangers under one roof, which is the exact thing the longhouse
        // exists to prevent. The unwed keep to the fire until their roof is
        // up, and the planner puts that roof first.
        let want_longhouse = !wants_family_roof(spouse, child);

        // Kin first: a child joins a parent's roof, and a spouse joins
        // their partner's — but only if it is the roof they both belong
        // under. A newly wed pair still bedded down in the longhouse move
        // to a house together rather than one following the other in and
        // both being moved out again the next morning.
        let kin: Option<Entity> = if child {
            parentage.and_then(|p| {
                [p.mother, p.father].into_iter().find_map(|parent| {
                    placed
                        .get(&parent)
                        .copied()
                        .or_else(|| tenants.get(parent).ok().map(|h| h.0))
                })
            })
        } else {
            spouse.and_then(|s| {
                placed
                    .get(&s.0)
                    .copied()
                    .or_else(|| tenants.get(s.0).ok().map(|h| h.0))
            })
        };
        let kin_roof = kin.filter(|roof| {
            roofs.get(*roof).is_ok_and(|(_, _, long)| {
                long == want_longhouse && has_room(*roof, long, &occupancy)
            })
        });

        let roof = kin_roof.or_else(|| {
            // Otherwise the nearest roof of the right kind. The unattached
            // want the longhouse; the wed and their children want a house —
            // and a house holds ONE family: without kin already under it,
            // only an empty house will do. Boarding with somebody else's
            // family is exactly what the longhouse exists to prevent, and
            // it was happening to widows and fresh couples squeezed into
            // half-full homes.
            roofs
                .iter()
                .filter(|(roof, _, long)| {
                    *long == want_longhouse
                        && if *long {
                            has_room(*roof, *long, &occupancy)
                        } else {
                            occupancy.get(roof).copied().unwrap_or(0) == 0
                        }
                })
                .map(|(roof, at, _)| (roof, at.translation.distance(transform.translation)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(roof, _)| roof)
        });

        let Some(roof) = roof else {
            // No bed of the right kind. Sleeping by the fire beats taking a
            // family's room, so they wait for the build.
            continue;
        };

        commands.entity(entity).insert(Home(roof));
        *occupancy.entry(roof).or_default() += 1;
        placed.insert(entity, roof);

        let longhouse = roofs.get(roof).map(|(_, _, long)| long).unwrap_or(false);
        if longhouse {
            info!("{} took a bed in the longhouse", person.name);
            if let Some(mut chronicle) = chronicle {
                chronicle.record(clock.day(), "took a bed in the longhouse");
            }
        } else {
            info!("{} moved into a house", person.name);
            if let Some(mut chronicle) = chronicle {
                chronicle.record(clock.day(), "moved under a roof of their own");
            }
        }
    }
}

/// The village sorts itself: whoever is sleeping under the wrong roof gives
/// up their bed as soon as a right one exists.
///
/// This is where the lifecycle actually turns. A child comes of age and the
/// family house is no longer theirs — they carry their blanket to the
/// longhouse. Two longhouse sleepers marry, and a house falls open to them.
/// Neither move is scripted anywhere; both fall out of asking, once a day,
/// whether anyone is in the wrong building.
///
/// It only takes the `Home` away. `assign_homes` does the placing on the
/// next pass, so there is exactly one piece of code that decides who sleeps
/// where.
#[allow(clippy::type_complexity)]
pub(super) fn rehome_the_misplaced(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut since_last: Local<f32>,
    roofs: Query<(Entity, Has<Longhouse>), Or<(With<Hut>, With<Longhouse>)>>,
    tenants: Query<&Home>,
    housed: Query<
        (
            Entity,
            &Person,
            &Home,
            &Activity,
            Option<&Spouse>,
            Has<Childhood>,
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
) {
    *since_last += time.delta_secs();
    if *since_last < 9.0 {
        return;
    }
    *since_last = 0.0;

    // Never at night, and never mid-errand: a villager hidden inside a
    // building is being stood in for by that building, and pulling their
    // home out from under them there would strand them invisible.
    if clock.is_night() {
        return;
    }

    let mut occupancy: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for home in &tenants {
        *occupancy.entry(home.0).or_default() += 1;
    }
    let room_of_kind = |longhouse: bool, occupancy: &std::collections::HashMap<Entity, usize>| {
        let cap = if longhouse {
            LONGHOUSE_CAPACITY
        } else {
            HOUSE_CAPACITY
        };
        roofs.iter().any(|(roof, long)| {
            long == longhouse && occupancy.get(&roof).copied().unwrap_or(0) < cap
        })
    };

    // One move a pass. A village that reshuffles six people at once reads
    // as a glitch; one person walking their bedding across the square reads
    // as a life changing.
    for (entity, person, home, activity, spouse, child) in &housed {
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        let Ok((_, in_longhouse)) = roofs.get(home.0) else {
            continue;
        };
        let wants_longhouse = !wants_family_roof(spouse, child);
        if !should_rehome(
            wants_longhouse,
            in_longhouse,
            room_of_kind(wants_longhouse, &occupancy),
        ) {
            continue;
        }
        commands.entity(entity).remove::<Home>();
        if wants_longhouse {
            info!("{} left the family house for the longhouse", person.name);
        } else {
            info!("{} left the longhouse for a house", person.name);
        }
        return;
    }
}

/// Night sends people home; dawn brings them out again.
///
/// The housed sleep indoors and vanish from the night streets. The homeless
/// drift to the fire instead — the lit circle is a census of who has no roof.
pub(super) fn night_routine(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    homes: Query<&Transform, (Or<(With<Hut>, With<Longhouse>)>, Without<Villager>)>,
    beds: Query<(&ChildOf, &Transform, &Bed), Without<Villager>>,
    fires: Query<&GlobalTransform, (With<Bonfire>, Without<Villager>)>,
    mut rng: ResMut<SimRng>,
    mut villagers: Query<
        (
            Entity,
            &Transform,
            Option<&Home>,
            Option<&BedSlot>,
            &crate::creature::genome::CreatureGenome,
            &Needs,
            &mut Activity,
            &mut MoveTarget,
            &mut CreatureMotion,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let night = clock.is_night();
    let fire_pos = fires.iter().next().map(|f| f.translation());

    // Where each numbered bed stands, in the world: the building's own
    // rotation and place applied to the mattress's local offset.
    let bed_of = |building: Entity, slot: u8, site: &Transform| {
        beds.iter()
            .find(|(parent, _, bed)| parent.parent() == building && bed.slot == slot)
            .map(|(_, local, bed)| {
                (
                    site.transform_point(local.translation),
                    bed.lie,
                    site.rotation,
                )
            })
    };

    for (entity, transform, home, berth, genome, needs, mut activity, mut target, mut motion) in
        &mut villagers
    {
        if !night {
            if *activity == Activity::Sleeping {
                *activity = Activity::Idle;
                target.0 = None;
                // Rise: stand beside the bed, upright, on the ground.
                commands.entity(entity).remove::<Abed>();
                motion.speed = 1.0;
            }
            continue;
        }

        // The fire-tender finishes the errand first; sleep can wait a minute.
        if *activity == Activity::TendingFire {
            continue;
        }

        // Nobody sleeps through starving. A night is longer than an empty
        // stomach can bear, so the truly hungry are left to the food systems
        // — roused from bed if need be — until they have eaten enough to
        // survive until dawn. Without this, a roof was a death sentence the
        // firelit homeless escaped: sleep held the housed while hunger ran.
        if needs.hunger > 0.7 {
            if *activity == Activity::Sleeping {
                *activity = Activity::Idle;
                target.0 = None;
                commands.entity(entity).remove::<Abed>();
                motion.speed = 1.0;
            }
            continue;
        }

        match home.map(|h| h.0) {
            Some(building) => {
                let Ok(site) = homes.get(building) else {
                    continue;
                };
                // Their OWN bed, by claimed number. No claim yet (or no
                // such bed in an old save's home): the door will do.
                let berth = berth.and_then(|slot| bed_of(building, slot.0, site));
                let Some((bed_at, lie, site_spin)) = berth else {
                    if transform.translation.distance(site.translation) > 2.2 {
                        *activity = Activity::Sleeping;
                        target.0 = Some(site.translation);
                    } else {
                        *activity = Activity::Sleeping;
                        target.0 = None;
                        motion.speed = 0.0;
                    }
                    continue;
                };
                let walk_goal = Vec3::new(bed_at.x, transform.translation.y, bed_at.z);
                if transform.translation.distance(walk_goal) > 0.9 {
                    if *activity != Activity::Sleeping {
                        *activity = Activity::Sleeping;
                    }
                    target.0 = Some(walk_goal);
                } else if *activity != Activity::Sleeping || target.0.is_some() {
                    // Into bed: lie along the mattress, head to the pillow,
                    // held there until morning. VISIBLY — the roof is what
                    // hides a sleeper now, and the roof can be lifted.
                    *activity = Activity::Sleeping;
                    target.0 = None;
                    motion.speed = 0.0;
                    motion.flail = 0.0;
                    // Flat on the back, length along the mattress, head on
                    // the pillow — whichever end the bed says its wall is.
                    // The quarter-turn is the other way from first instinct;
                    // the playtest photo of a sleeper lying ACROSS the bed
                    // is the authority here.
                    let facing = site_spin
                        * Quat::from_rotation_y(lie)
                        * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                    info!("a sleeper settles into their bed");
                    commands.entity(entity).insert(Abed {
                        at: bed_at + laid_from(genome, facing, PILLOW_AHEAD),
                        facing,
                    });
                }
            }
            None => {
                // No roof: the firelight, and then the ground. They used
                // to stand there all night, which is not what anybody
                // does at a fire at three in the morning - and it made
                // the roofless look content with the arrangement.
                let Some(fire) = fire_pos else { continue };
                if !matches!(*activity, Activity::Idle | Activity::Wandering) {
                    continue;
                }
                if transform.translation.distance(fire) > 7.0 {
                    *activity = Activity::Sleeping;
                    target.0 =
                        Some(fire + Vec3::new(rng.0.range(-4.0, 4.0), 0.0, rng.0.range(-4.0, 4.0)));
                    continue;
                }
                // Down where they stand, feet to the warmth and head to
                // the dark - which is how anybody lies by a fire, and
                // lays them in a ring around it without anyone having to
                // arrange one.
                *activity = Activity::Sleeping;
                target.0 = None;
                motion.speed = 0.0;
                motion.flail = 0.0;
                let outward = (transform.translation - fire)
                    .with_y(0.0)
                    .normalize_or(Vec3::X);
                let facing = Quat::from_rotation_y(super::work::lie_toward(outward))
                    * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                commands.entity(entity).insert(Abed {
                    // No pillow on the bare ground: they simply lie where
                    // they stood, straddling the spot.
                    at: transform.translation + laid_from(genome, facing, genome.height() * 0.35),
                    facing,
                });
            }
        }
    }
}

/// The midday bell: tools down, and the square fills for the meal.
///
/// Workers are released a few at a time (the chance staggers them, so the
/// square fills like a square and not like a fire drill), drift to their
/// town's centre, eat if they are hungry, and fall into the gossip mill by
/// sheer proximity — which is the point of a midday meal.
#[allow(clippy::type_complexity)]
pub(super) fn midday_meal(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    grounds: Query<&super::SettlementGround>,
    mut rng: ResMut<SimRng>,
    mut workers: Query<
        (
            Entity,
            &Transform,
            Option<&super::MemberOf>,
            &mut Activity,
            &mut MoveTarget,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    if !clock.midday_meal() {
        return;
    }
    for (entity, at, member, mut activity, mut target) in &mut workers {
        if *activity != Activity::Working || !rng.0.chance(0.05) {
            continue;
        }
        let Some(centre) = member.and_then(|m| grounds.get(m.0).ok()).map(|g| g.centre) else {
            continue;
        };
        commands.entity(entity).remove::<super::work::Job>();
        *activity = Activity::Wandering;
        target.0 = Some(centre + Vec3::new(rng.0.range(-4.0, 4.0), 0.0, rng.0.range(-4.0, 4.0)));
        let _ = at;
    }
}

/// The evening bell: families sup at their own hearth.
///
/// A house-dweller heads home at dusk and stays under their roof until the
/// night hands them to bed — through the door, visibly, together. Ten
/// seconds of theatre a day that makes a house a household. The unwed keep
/// their evening: the tavern claims the low, the fire the rest.
#[allow(clippy::type_complexity)]
pub(super) fn family_supper(
    clock: Res<crate::calendar::WorldClock>,
    homes: Query<&Transform, (With<Hut>, Without<Villager>)>,
    tables: Query<(&ChildOf, &Transform), (With<super::work::Table>, Without<Villager>)>,
    mut families: Query<
        (Entity, &Transform, &Home, &mut Activity, &mut MoveTarget),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    if !clock.is_evening() {
        return;
    }
    for (entity, at, home, mut activity, mut target) in &mut families {
        // Only house-dwellers: the hearth is the family's.
        let Ok(hut) = homes.get(home.0) else {
            continue;
        };
        match *activity {
            Activity::Idle | Activity::Wandering | Activity::Sheltering | Activity::Working => {}
            _ => continue,
        }
        // A seat at the family table, each their own side of it — falling
        // back to the room itself for houses raised before tables were.
        let seat = tables
            .iter()
            .find(|(parent, _)| parent.parent() == home.0)
            .map(|(_, table)| {
                let corner = match entity.index().index() % 4 {
                    0 => Vec3::new(-1.15, 0.0, 0.0),
                    1 => Vec3::new(1.15, 0.0, 0.0),
                    2 => Vec3::new(0.0, 0.0, -0.95),
                    _ => Vec3::new(0.0, 0.0, 0.95),
                };
                hut.transform_point(table.translation + corner)
            })
            .unwrap_or(hut.translation);
        let seat = Vec3::new(seat.x, at.translation.y, seat.z);
        if at.translation.distance(seat) > 0.8 {
            *activity = Activity::Sheltering;
            target.0 = Some(seat);
        } else if *activity != Activity::Sheltering || target.0.is_some() {
            *activity = Activity::Sheltering;
            target.0 = None;
        }
    }
}

/// Evenings belong to the tavern, once the village has one.
///
/// Between the workday's end and sleep, anyone whose spirits could use it
/// drifts to the tavern and lingers. The crowd this makes is not decoration:
/// gossip runs on proximity, so the tavern becomes the place stories change
/// hands — the village's rumour engine, built out of nothing but a building
/// and a schedule.
pub(super) fn tavern_evenings(
    clock: Res<crate::calendar::WorldClock>,
    taverns: Query<(&GlobalTransform, &super::work::Building)>,
    mut rng: ResMut<SimRng>,
    mut villagers: Query<
        (&Transform, &super::Morale, &mut Activity, &mut MoveTarget),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let t = clock.time_of_day();
    let evening = (0.62..0.74).contains(&t);
    if !evening {
        return;
    }
    let Some(tavern) = taverns
        .iter()
        .find(|(_, b)| b.kind == super::work::BuildingKind::Tavern)
        .map(|(at, _)| at.translation())
    else {
        return;
    };

    for (transform, morale, mut activity, mut target) in &mut villagers {
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        if morale.spirits > 0.85 {
            continue;
        }
        if transform.translation.distance(tavern) > 6.0 {
            *activity = Activity::Wandering;
            target.0 =
                Some(tavern + Vec3::new(rng.0.range(-3.5, 3.5), 0.0, rng.0.range(-3.5, 3.5)));
        } else {
            *activity = Activity::Idle;
            target.0 = None;
        }
    }
}

/// Waking wears people down; sleep knits them back up — and spirits follow.
///
/// A bed is best; the ground by the fire is half a bed; a night spent awake is
/// none. Exhaustion drains the spirits, and a hollowed-out villager stops
/// showing up for work. This is the first thread of psychological health: a
/// god who keeps snatching sleepers from their beds is *doing something* to
/// this number, and the village will show it.
pub(super) fn weariness(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    sky: Option<Res<crate::calendar::Sky>>,
    buildings: Query<&super::work::Building>,
    mut villagers: Query<
        (
            &mut super::Needs,
            &mut super::Morale,
            &Activity,
            &Visibility,
            Option<&Home>,
            Option<&super::traits::Traits>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
) {
    let dt = time.delta_secs();
    let night = clock.is_night();
    // Warm cloth on every back: the weaver's work is felt hardest by those
    // sleeping out - a blanket is half a roof.
    let woven = buildings
        .iter()
        .any(|b| b.kind == super::work::BuildingKind::Weaver);
    let roofless_ceiling = if woven { 0.68 } else { 0.55 };

    for (mut needs, mut morale, activity, visibility, home, manner) in &mut villagers {
        let brightness = manner.map_or(1.0, |m| m.brightness());
        let endurance = manner.map_or(1.0, |m| m.endurance());
        let asleep_indoors = *activity == Activity::Sleeping && *visibility == Visibility::Hidden;
        let dozing_by_fire = night && matches!(activity, Activity::Idle | Activity::Wandering);

        if asleep_indoors {
            needs.rest = (needs.rest - dt / 55.0).max(0.0);
        } else if dozing_by_fire {
            needs.rest = (needs.rest - dt / 130.0).max(0.0);
        } else {
            // A full waking day builds most of the way toward needing sleep.
            needs.rest = (needs.rest + dt * endurance / 700.0).min(1.0);
        }

        if needs.rest > 0.85 {
            morale.spirits = (morale.spirits - dt / 240.0).max(0.0);
        } else if needs.rest < 0.4 {
            morale.spirits = (morale.spirits + dt * brightness / 320.0).min(1.0);
        }

        // Homelessness is a weight that never quite lifts: nights on the
        // ground grind the spirits down, and no amount of firelight raises a
        // roofless life above resignation.
        if home.is_none() {
            if night {
                // A cold, wet night grinds harder than a mild one.
                let chill = weather.as_ref().map_or(0.0, |w| {
                    1.0 - w.temperature(sky.as_ref().map_or(0.0, |s| s.daylight))
                });
                morale.spirits = (morale.spirits - dt * (1.0 + chill) / 200.0).max(0.0);
            }
            morale.spirits = morale.spirits.min(roofless_ceiling);
        }
    }
}

/// Company by the tavern door mends spirits faster than solitude does.
pub(super) fn tavern_cheer(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    taverns: Query<(&GlobalTransform, &super::work::Building)>,
    mut villagers: Query<(&Transform, &mut super::Morale), (With<Villager>, Without<Corpse>)>,
) {
    let t = clock.time_of_day();
    if !(0.62..0.76).contains(&t) {
        return;
    }
    let Some(tavern) = taverns
        .iter()
        .find(|(_, b)| b.kind == super::work::BuildingKind::Tavern)
        .map(|(at, _)| at.translation())
    else {
        return;
    };
    let dt = time.delta_secs();
    for (transform, mut morale) in &mut villagers {
        if transform.translation.distance(tavern) < 7.0 {
            morale.spirits = (morale.spirits + dt / 70.0).min(1.0);
        }
    }
}

/// A hand that reaches into a house comes out with someone visible. Being
/// plucked from bed wakes you.
pub(super) fn rouse_the_taken(
    mut taken: Query<(&mut Visibility, &mut Activity), (With<Villager>, With<Held>)>,
) {
    for (mut visibility, mut activity) in &mut taken {
        if *visibility == Visibility::Hidden {
            *visibility = Visibility::Inherited;
        }
        if *activity == Activity::Sleeping {
            *activity = Activity::Idle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_log_buys_more_burning_than_the_low_mark() {
        // If a log burned out before the next one could plausibly be fetched,
        // the fire would flap between lit and dead all night.
        assert!(SECONDS_PER_LOG > LOW_FUEL * 1.5);
    }

    #[test]
    fn houses_sleep_families_not_crowds() {
        assert!((2..=6).contains(&HOUSE_CAPACITY));
    }

    #[test]
    fn a_longhouse_is_worth_more_than_a_house() {
        // The whole point of the long roof is that it sleeps more per
        // building than the family homes it takes pressure off.
        assert!(LONGHOUSE_CAPACITY > HOUSE_CAPACITY);
    }

    #[test]
    fn both_roofs_count_toward_shelter() {
        let bare = shelter_capacity(0, 0);
        assert_eq!(bare, FIRE_CIRCLE_SHELTER);
        assert_eq!(shelter_capacity(1, 0), bare + HOUSE_CAPACITY);
        assert_eq!(shelter_capacity(0, 1), bare + LONGHOUSE_CAPACITY);
        assert_eq!(
            shelter_capacity(3, 2),
            bare + 3 * HOUSE_CAPACITY + 2 * LONGHOUSE_CAPACITY
        );
    }

    #[test]
    fn the_wed_and_the_children_want_houses() {
        let spouse = Spouse(Entity::from_raw_u32(3).unwrap());

        // A married adult, and any child.
        assert!(wants_family_roof(Some(&spouse), false));
        assert!(wants_family_roof(None, true));
        // A widow keeps the house: Spouse outlives its person.
        assert!(wants_family_roof(Some(&spouse), false));
    }

    /// A village with one house, one longhouse, and a clock at midday.
    ///
    /// Both housing systems run each update, in the order the plugin runs
    /// them, and the clock is set well clear of night — `rehome_the_misplaced`
    /// will not move anyone who might be indoors and hidden.
    fn village() -> (App, Entity, Entity) {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(10));
        app.insert_resource(time);
        app.insert_resource(crate::calendar::WorldClock {
            elapsed: (crate::calendar::DAY_SECONDS as f64) * 0.4,
        });
        app.add_systems(Update, (assign_homes, rehome_the_misplaced).chain());

        let house = app.world_mut().spawn((Hut, Transform::default())).id();
        let longhouse = app
            .world_mut()
            .spawn((Longhouse, Transform::from_xyz(20.0, 0.0, 0.0)))
            .id();
        (app, house, longhouse)
    }

    fn villager(app: &mut App, name: &str, home: Option<Entity>) -> Entity {
        let mut person = app.world_mut().spawn((
            Villager,
            Transform::default(),
            Activity::Idle,
            Person::born(name.into(), "Testly".into()),
        ));
        if let Some(home) = home {
            person.insert(Home(home));
        }
        person.id()
    }

    fn home_of(app: &App, who: Entity) -> Option<Entity> {
        app.world().entity(who).get::<Home>().map(|h| h.0)
    }

    #[test]
    fn coming_of_age_moves_a_grown_child_to_the_longhouse() {
        // The lifecycle's first turn, and nothing scripts it: the day
        // `Childhood` comes off, the family room stops being theirs.
        let (mut app, house, longhouse) = village();
        let mother = villager(&mut app, "Mother", Some(house));
        let father = villager(&mut app, "Father", Some(house));
        app.world_mut().entity_mut(mother).insert(Spouse(father));
        app.world_mut().entity_mut(father).insert(Spouse(mother));
        let grown = villager(&mut app, "Grown", Some(house));

        // Several passes: one move per pass, then the placement after it.
        for _ in 0..4 {
            app.update();
        }

        assert_eq!(
            home_of(&app, grown),
            Some(longhouse),
            "an adult with no spouse should have left the family house",
        );
        assert_eq!(home_of(&app, mother), Some(house), "the wed keep the house");
        assert_eq!(home_of(&app, father), Some(house), "the wed keep the house");
    }

    #[test]
    fn a_couple_marries_out_of_the_longhouse_into_one_house() {
        // And the turn back. The pair must land under the SAME roof — the
        // failure this guards is a newly wed couple assigned to two
        // different empty houses on the same pass.
        let (mut app, house, longhouse) = village();
        let groom = villager(&mut app, "Groom", Some(longhouse));
        let bride = villager(&mut app, "Bride", Some(longhouse));

        app.update();
        assert_eq!(home_of(&app, groom), Some(longhouse), "unwed belong here");

        app.world_mut().entity_mut(groom).insert(Spouse(bride));
        app.world_mut().entity_mut(bride).insert(Spouse(groom));
        for _ in 0..4 {
            app.update();
        }

        assert_eq!(home_of(&app, groom), Some(house));
        assert_eq!(
            home_of(&app, bride),
            Some(house),
            "a married pair sleep under one roof",
        );
    }

    #[test]
    fn strangers_never_move_in_with_a_family() {
        // The bug this pins, reported from a fresh game: the first building
        // finished was a house, and it filled with a married couple and two
        // unrelated adults. A house costs half what a longhouse does, so it
        // always wins the race — the rule has to hold before any longhouse
        // exists, not just after.
        let (mut app, house, _longhouse) = village();
        // Strip the longhouse out: this is the founding, with only a house.
        app.world_mut().entity_mut(_longhouse).despawn();

        let husband = villager(&mut app, "Tiahok", None);
        let wife = villager(&mut app, "Yebuzia", None);
        app.world_mut().entity_mut(husband).insert(Spouse(wife));
        app.world_mut().entity_mut(wife).insert(Spouse(husband));
        let stranger_a = villager(&mut app, "Wokle", None);
        let stranger_b = villager(&mut app, "Drehe", None);

        for _ in 0..6 {
            app.update();
        }

        assert_eq!(
            home_of(&app, husband),
            Some(house),
            "the couple get the house"
        );
        assert_eq!(home_of(&app, wife), Some(house), "and they get it together");
        assert_eq!(
            home_of(&app, stranger_a),
            None,
            "an unwed adult waits for the longhouse rather than joining a family",
        );
        assert_eq!(home_of(&app, stranger_b), None);
    }

    #[test]
    fn the_unwed_wait_for_their_own_roof_even_when_a_house_stands_empty() {
        // No longhouse, one wholly empty house, one unwed adult: he still
        // sleeps by the fire. A house is a family's, and an empty one is a
        // family's that has not arrived yet.
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(10));
        app.insert_resource(time);
        app.insert_resource(crate::calendar::WorldClock {
            elapsed: (crate::calendar::DAY_SECONDS as f64) * 0.4,
        });
        app.add_systems(Update, (assign_homes, rehome_the_misplaced).chain());
        app.world_mut().spawn((Hut, Transform::default()));
        let lodger = villager(&mut app, "Lodger", None);

        for _ in 0..4 {
            app.update();
        }

        assert_eq!(home_of(&app, lodger), None);
    }

    #[test]
    fn a_bed_is_given_up_only_for_one_that_is_free() {
        // Under the wrong roof, with the right kind free: go.
        assert!(should_rehome(true, false, true));
        assert!(should_rehome(false, true, true));
        // Under the wrong roof, but nothing free to move into: stay put.
        // Leaving here would trade a bed for the fire circle.
        assert!(!should_rehome(true, false, false));
        assert!(!should_rehome(false, true, false));
        // Already home. Room elsewhere is not a reason to move.
        assert!(!should_rehome(true, true, true));
        assert!(!should_rehome(false, false, true));
    }

    #[test]
    fn coming_of_age_ends_the_claim_on_a_family_room() {
        // The lifecycle turns on exactly this: the same person, unwed, is
        // family while a child and longhouse material the day after.
        assert!(wants_family_roof(None, true));
        assert!(!wants_family_roof(None, false));
    }
}
