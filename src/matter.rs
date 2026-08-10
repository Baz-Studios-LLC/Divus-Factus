//! Matter: what things are made of, and how that stuff behaves.
//!
//! A tree is not "a tree" — it is wood, with a weight, a shape that rolls
//! badly, and a body that floats. A boulder is stone: heavy in the hand,
//! eager to roll, deadly downhill. Every system that touches an object asks
//! its matter rather than its name, which is where cross-system chemistry
//! comes from: throw + slope + mass = an avalanche of one; wood + water =
//! a log drifting on the tide; boulder + villager = a death with witnesses.
//!
//! None of this is a physics engine. Rolling is the terrain gradient applied
//! to a velocity, floating is a rest height, spinning is speed over radius —
//! a few honest formulas, which at this scale read exactly like physics.

use bevy::prelude::*;

use crate::creature::anim::CreatureMotion;
use crate::creature::genome::CreatureGenome;
use crate::creature::{Airborne, Corpse, Creature, Vitality};
use crate::terrain::{Terrain, WATER_LEVEL};

const GRAVITY: f32 = 19.6;

/// Speed below which a rolling thing settles.
const REST_SPEED: f32 = 0.9;

/// mass × speed above which a rolling object hurts what it hits.
const CRUSH_MOMENTUM: f32 = 240.0;

pub struct MatterPlugin;

impl Plugin for MatterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (loose_ballistics, roll, float, the_water_claims, sink).chain(),
        )
        .add_systems(Update, (settle, drive_sparks));
    }
}

// ------------------------------------------------------------------ sparks

/// One fleck of a thing becoming its essence: a tree bursting into the
/// timber it was, a rock into stone, a bush into the meal on it.
///
/// Little cubes, because everything in this world is little cubes - the
/// same shape the glory miracle scatters, cut down to a hand-sized burst.
#[derive(Component)]
pub struct Spark {
    velocity: Vec3,
    age: f32,
    life: f32,
    /// The size it was born at; the shrink is measured from here by AGE.
    /// The first cut multiplied the standing scale by a constant every
    /// FRAME - fifteen percent gone sixty times a second - and the whole
    /// burst lived about six frames. Brett: "I am not seeing the particles."
    born: f32,
    /// Where the essence is bound: the pile or site that took the offering.
    /// Brett's shape for the gesture - "explode and then shrink into the
    /// focal point of the pile" - so the burst has a DESTINATION, and what
    /// the pop means (the stores got richer) is readable in the motion.
    home: Vec3,
}

/// How much of a spark's life is the explosion; the rest is the gathering-in.
const SPARK_SCATTER: f32 = 0.38;

/// Throws a burst of flecks where something was just taken as an offering.
///
/// Brett: "can we have the tree pop in a kind of particle burst animation?"
/// The colours are the thing's own - bark and leaf for a tree, grey for
/// stone, berry-red for food - so what it WAS is readable in the pop.
pub fn burst_of(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    home: Vec3,
    colors: &[Color],
    count: usize,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let coats: Vec<Handle<StandardMaterial>> = colors
        .iter()
        .map(|color| {
            materials.add(StandardMaterial {
                base_color: *color,
                emissive: LinearRgba::from(*color) * 0.6,
                perceptual_roughness: 1.0,
                ..default()
            })
        })
        .collect();
    // Golden-angle spread, no two flecks alike - the same trick the glory
    // motes use, so bursts read as one family of magic.
    for i in 0..count {
        let angle = i as f32 * 2.399963;
        let (sin, cos) = angle.sin_cos();
        let pace = 3.0 + (i % 3) as f32 * 1.4;
        let born = 0.3 + (i % 3) as f32 * 0.12;
        commands.spawn((
            Spark {
                velocity: Vec3::new(cos * pace, 4.2 + (i % 4) as f32 * 1.1, sin * pace),
                age: 0.0,
                life: 0.9 + (i % 5) as f32 * 0.12,
                born,
                home: home + Vec3::Y * 0.5,
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(coats[i % coats.len()].clone()),
            Transform::from_translation(at + Vec3::new(cos * 0.4, 0.8, sin * 0.4))
                .with_scale(Vec3::splat(born)),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// The puff: a breath of cloth, close against the body.
///
/// The change-of-clothes gesture. The offering burst throws its essence
/// metres and gathers it back grandly - right for a gift becoming stores,
/// far too much for a coat. These flecks are small, barely clear the
/// silhouette, and settle back INTO the body, like cloth falling into
/// place. Brett, twice: the swap's particles were "a little much", then
/// "still way too big and move way too far away from the body".
pub fn puff_of(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    colors: &[Color],
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let coats: Vec<Handle<StandardMaterial>> = colors
        .iter()
        .map(|color| {
            materials.add(StandardMaterial {
                base_color: *color,
                emissive: LinearRgba::from(*color) * 0.6,
                perceptual_roughness: 1.0,
                ..default()
            })
        })
        .collect();
    for i in 0..5 {
        let angle = i as f32 * 2.399963;
        let (sin, cos) = angle.sin_cos();
        let pace = 0.5 + (i % 3) as f32 * 0.25;
        let born = 0.07 + (i % 3) as f32 * 0.04;
        commands.spawn((
            Spark {
                velocity: Vec3::new(cos * pace, 0.7 + (i % 4) as f32 * 0.25, sin * pace),
                age: 0.0,
                life: 0.35 + (i % 5) as f32 * 0.06,
                born,
                home: at,
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(coats[i % coats.len()].clone()),
            Transform::from_translation(
                at + Vec3::new(cos * 0.25, (i % 3) as f32 * 0.25 - 0.2, sin * 0.25),
            )
            .with_scale(Vec3::splat(born)),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// The spill: earth shaken loose, falling home to the ground.
///
/// A tree torn up by the roots does not sparkle - it RAINS. Clumps of
/// soil break off the root ball, tumble outward, and gather back into
/// the ground they came from: the same Spark machinery as every other
/// small magic, so the world keeps one family of motion. About one
/// clump in fourteen is a pale stone shaken out with the dirt -
/// Brett: "probably 95% brown and 5% gray?"
pub fn spill_of(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let coats: [Handle<StandardMaterial>; 3] = [
        crate::palette::shade(&crate::palette::EARTH, 0.35),
        crate::palette::shade(&crate::palette::EARTH, 0.55),
        crate::palette::shade(&crate::palette::STONE, 0.55),
    ]
    .map(|color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 1.0,
            ..default()
        })
    });
    for i in 0..14 {
        let angle = i as f32 * 2.399963;
        let (sin, cos) = angle.sin_cos();
        // Low and lazy: dirt falls off, it is not flung.
        let pace = 0.6 + (i % 3) as f32 * 0.5;
        let born = 0.10 + (i % 4) as f32 * 0.05;
        // The one pale stone in the root ball.
        let coat = if i == 5 { &coats[2] } else { &coats[i % 2] };
        commands.spawn((
            Spark {
                velocity: Vec3::new(cos * pace, 0.4 + (i % 3) as f32 * 0.5, sin * pace),
                age: 0.0,
                life: 0.45 + (i % 5) as f32 * 0.08,
                born,
                // Home is the ground the tree stood in, scattered a little,
                // so the clumps FALL and settle instead of vanishing upward.
                home: at + Vec3::new(cos * (0.5 + pace * 0.3), 0.05, sin * (0.5 + pace * 0.3)),
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(coat.clone()),
            Transform::from_translation(
                at + Vec3::new(cos * 0.3, 0.7 + (i % 3) as f32 * 0.3, sin * 0.3),
            )
            .with_scale(Vec3::splat(born)),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// The suck: sparks that skip the explosion and go straight to gathering.
///
/// The reverse gesture of an offering's burst, for scooping a lump OFF a
/// deposit: flecks of the bank stream from the ground into the god's grip
/// and the lump is what they condense into. Brett: "the places resources
/// should let me suck up the particles and turn into an item in my hand."
/// Implemented as sparks born with act one already spent - same component,
/// same driver, no new machinery.
pub fn gather_to(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    from: Vec3,
    to: Vec3,
    colors: &[Color],
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let coats: Vec<Handle<StandardMaterial>> = colors
        .iter()
        .map(|color| {
            materials.add(StandardMaterial {
                base_color: *color,
                emissive: LinearRgba::from(*color) * 0.6,
                perceptual_roughness: 1.0,
                ..default()
            })
        })
        .collect();
    for i in 0..14 {
        let angle = i as f32 * 2.399963;
        let (sin, cos) = angle.sin_cos();
        let reach = 0.6 + (i % 4) as f32 * 0.5;
        let born = 0.22 + (i % 3) as f32 * 0.09;
        let life = 0.55 + (i % 5) as f32 * 0.09;
        commands.spawn((
            Spark {
                velocity: Vec3::ZERO,
                // Born past the scatter, so the driver goes straight to the
                // gather - staggered a touch so they stream rather than swarm.
                age: life * (SPARK_SCATTER + (i % 4) as f32 * 0.04),
                life,
                born,
                home: to,
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(coats[i % coats.len()].clone()),
            Transform::from_translation(from + Vec3::new(cos * reach, 0.25, sin * reach))
                .with_scale(Vec3::splat(born)),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// Flies every spark through its two acts: OUT, in a burst under gravity,
/// then HOME - swooping into the pile that took the offering, shrinking as
/// it closes, gone as it arrives. The gather is an ease-in, so the flecks
/// hang at the top of their scatter for a beat and then commit.
fn drive_sparks(
    mut commands: Commands,
    time: Res<Time>,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (fleck, mut spark, mut transform) in &mut sparks {
        spark.age += dt;
        let t = spark.age / spark.life;
        if t >= 1.0 {
            commands.entity(fleck).despawn();
            continue;
        }
        if t < SPARK_SCATTER {
            // The explosion: ballistic, full size.
            spark.velocity.y -= 12.0 * dt;
            let velocity = spark.velocity;
            transform.translation += velocity * dt;
        } else {
            // The gathering: pulled to the pile ever harder, shrinking as
            // it goes. Position is steered rather than integrated - a
            // fraction of the remaining gap per second - so every fleck
            // arrives no matter where its scatter left it.
            let gather = (t - SPARK_SCATTER) / (1.0 - SPARK_SCATTER);
            let pull = 1.0 - (-(3.0 + gather * 14.0) * dt).exp();
            let home = spark.home;
            transform.translation = transform.translation.lerp(home, pull);
            transform.scale = Vec3::splat(spark.born * (1.0 - gather * 0.9));
            if transform.translation.distance(home) < 0.25 {
                commands.entity(fleck).despawn();
            }
        }
    }
}

/// A double handful scooped off a deposit by the god: clay from the bank,
/// ore from the vein, stone from the quarry face.
///
/// Deposits are PLACES - rooted, worked by miners, sunk when spent - and the
/// hand does not uproot places. But the god as hauler needs SOMETHING to
/// carry off them, or clay and iron are the two resources providence cannot
/// touch. Brett: "there are other resources like clay, let's make sure we can
/// pick up everything we need too." So a grab on a deposit tears off a lump:
/// a real object, thrown and offered like any rock, paid by its kind.
#[derive(Component, Debug, Clone, Copy)]
pub struct Lump {
    pub kind: DepositKind,
    pub amount: f32,
}

/// What a thing is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Substance {
    Wood,
    Stone,
    Plant,
}

/// The physical character of an object the hand can move.
#[derive(Component, Debug, Clone, Copy)]
pub struct Matter {
    /// Read by nothing yet; fire will care first.
    #[allow(dead_code)]
    pub substance: Substance,
    /// Scales throw distance down and crushing power up.
    pub mass: f32,
    /// 0 flops where it lands, 1 rolls like a wheel.
    pub roundness: f32,
    /// Rests on water rather than under it.
    pub buoyant: bool,
    /// Visual radius, for spin rate and ground clearance.
    pub radius: f32,
    /// How far the body's visual CENTRE sits above its origin. A tree is
    /// built from its trunk base, so its heart is half its height up; a
    /// boulder is built around its middle and its heart is nought. Tumbling
    /// happens about the heart - about the origin, a thrown tree pivoted on
    /// its root and swept its crown through the turf on every turn: "it
    /// spins super fast and clips through the ground. It spins from the
    /// base of the trunk."
    pub heart: f32,
}

impl Matter {
    pub fn boulder(mass: f32, radius: f32) -> Matter {
        Matter {
            substance: Substance::Stone,
            mass,
            roundness: 0.82,
            buoyant: false,
            radius,
            heart: 0.0,
        }
    }

    pub fn felled_tree(maturity: f32) -> Matter {
        Matter {
            substance: Substance::Wood,
            mass: 30.0 + maturity * 50.0,
            roundness: 0.55,
            buoyant: true,
            radius: 0.5,
            // Half a tree's height: baked from the trunk base, crown above.
            heart: 2.2 + maturity * 1.0,
        }
    }

    pub fn bush() -> Matter {
        Matter {
            substance: Substance::Plant,
            mass: 6.0,
            roundness: 0.15,
            buoyant: true,
            radius: 0.4,
            heart: 0.35,
        }
    }

    /// How much of the hand's throw this mass absorbs.
    pub fn throw_factor(&self) -> f32 {
        70.0 / (70.0 + self.mass)
    }
}

/// A boulder: loose stone the hand can throw and a miner can work.
#[derive(Component)]
pub struct Boulder;

/// Rolling along the ground under gravity.
#[derive(Component, Debug)]
pub struct Rolling {
    pub velocity: Vec3,
}

/// At rest on water.
#[derive(Component)]
pub struct Floating;

/// One step of rolling: downhill pull, friction, and the decision to stop.
/// Pure, so the behaviour is testable without a world.
pub fn roll_step(velocity: Vec3, downhill: Vec3, roundness: f32, dt: f32) -> Vec3 {
    let pulled = velocity + downhill * GRAVITY * roundness * dt;
    // The less round it is, the harder the ground grips it.
    pulled * (1.0 - (1.55 - roundness) * 0.55 * dt).max(0.0)
}

/// Downhill direction and steepness at a point, from the terrain gradient.
fn downhill(terrain: &Terrain, x: f32, z: f32) -> Vec3 {
    let step = 1.2;
    let here = terrain.height_at(x, z);
    let dx = terrain.height_at(x + step, z) - here;
    let dz = terrain.height_at(x, z + step) - here;
    Vec3::new(-dx / step, 0.0, -dz / step)
}

/// Ballistics for loose matter — everything airborne that is not a creature.
///
/// Creatures land on their feet and take their harm; matter lands on its
/// nature: round things roll away downhill, buoyant things find the water's
/// surface, and the rest flop where they fall.
fn loose_ballistics(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut sounds: MessageWriter<crate::sfx::PlaySfx>,
    mut objects: Query<(Entity, &mut Transform, &mut Airborne, &Matter), Without<CreatureGenome>>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs();

    for (entity, mut transform, mut body, matter) in &mut objects {
        body.velocity.y -= GRAVITY * dt;
        transform.translation += body.velocity * dt;

        // End over end ABOUT THE HEART, the way a hurled log actually goes.
        // Rotating about the origin pivoted a tree on its trunk base: the
        // crown swept a six-unit arc - enormous tip speed no angular cap
        // could tame - and dipped under the turf on every turn.
        let speed = body.velocity.length();
        if let Ok(axis) = bevy::math::Dir3::new(Vec3::Y.cross(body.velocity)) {
            let rate = (speed / matter.radius.max(0.4) * 0.07).min(2.4);
            let pivot = transform.translation + transform.rotation * (Vec3::Y * matter.heart);
            transform.rotate_around(pivot, Quat::from_axis_angle(axis.as_vec3(), rate * dt));
        }

        // And the ground repels the swinging BODY, not the origin: the heart
        // stays a whole half-length above the floor while tumbling, so no
        // part of the turn goes below the turf.
        if matter.heart > 0.0 {
            let ground_here = terrain
                .height_at(transform.translation.x, transform.translation.z)
                .max(WATER_LEVEL);
            let clearance = matter.heart + matter.radius;
            let pivot_y =
                transform.translation.y + (transform.rotation * (Vec3::Y * matter.heart)).y;
            if pivot_y < ground_here + clearance && body.velocity.y < 0.0 {
                // Hit hard, and it BOUNCES - one diminishing hop, wood keeping
                // about a third of its fall - before the landing proper.
                // "When they hit the ground they just stop. Can we add just a
                // tad physics?"
                if body.velocity.y < -4.5 {
                    body.velocity.y = -body.velocity.y * 0.32;
                    body.velocity.x *= 0.62;
                    body.velocity.z *= 0.62;
                    transform.translation.y += ground_here + clearance - pivot_y;
                    sounds.write(crate::sfx::PlaySfx {
                        kind: crate::sfx::SfxKind::Thud,
                        at: Some(transform.translation),
                    });
                    continue;
                }
                // Close enough and slow enough to touch down: land it here
                // rather than clip, skidding out what remains of the throw.
                transform.translation.y = ground_here + matter.radius * 0.5;
                commands.entity(entity).remove::<Airborne>();
                if matter.roundness <= 0.6 {
                    let laid = lying_flat(transform.rotation);
                    commands.entity(entity).insert(Settling {
                        target: laid,
                        slide: Vec3::new(body.velocity.x, 0.0, body.velocity.z) * 0.5,
                    });
                }
                continue;
            }
        }

        let ground = terrain.height_at(transform.translation.x, transform.translation.z);
        let water_here = ground < WATER_LEVEL;

        // Splashdown for floaters happens at the surface, not the seabed.
        if matter.buoyant && water_here && transform.translation.y <= WATER_LEVEL {
            transform.translation.y = WATER_LEVEL + matter.radius * 0.4;
            commands
                .entity(entity)
                .remove::<Airborne>()
                .insert(Floating);
            continue;
        }

        let floor = ground.max(if matter.buoyant {
            f32::MIN
        } else {
            WATER_LEVEL - 20.0
        });
        if transform.translation.y <= floor + matter.radius * 0.5 {
            transform.translation.y = floor + matter.radius * 0.5;

            let lateral = Vec3::new(body.velocity.x, 0.0, body.velocity.z);
            if matter.roundness > 0.35 && lateral.length() > 1.6 {
                commands
                    .entity(entity)
                    .remove::<Airborne>()
                    .insert(Rolling { velocity: lateral });
            } else if body.velocity.y < -4.5 {
                // The same single hop the heart-landing takes.
                body.velocity.y = -body.velocity.y * 0.32;
                body.velocity.x *= 0.62;
                body.velocity.z *= 0.62;
                sounds.write(crate::sfx::PlaySfx {
                    kind: crate::sfx::SfxKind::Thud,
                    at: Some(transform.translation),
                });
            } else {
                commands.entity(entity).remove::<Airborne>();
                // The long and the flat come to REST rather than freezing
                // mid-tumble: a felled tree crashing down on its crown held
                // that pose, half its length under the turf. It lies down
                // instead - see `settle`.
                if matter.roundness <= 0.6 {
                    let laid = lying_flat(transform.rotation);
                    commands.entity(entity).insert(Settling {
                        target: laid,
                        slide: Vec3::new(body.velocity.x, 0.0, body.velocity.z) * 0.5,
                    });
                }
            }
        }
    }
}

/// Going under: the water has claimed this thing, and it is on its way
/// down and out of the world.
#[derive(Component)]
pub struct Sinking {
    /// The water surface it went under, for knowing when it is truly gone.
    surface: f32,
}

/// The water claims what falls in. Anything loose sitting at or under a
/// water surface — thrown there, dropped there, or bobbing on it from the
/// days when floaters floated forever — splashes, sinks, and is gone.
/// Brett: "I threw all of the villagers food into the ocean... and the
/// food just sat on the surface of the water... tons of it, lol."
///
/// A sweep on a slow tick rather than a hook in every landing path: the
/// ballistics, the roll, the drop and the legacy flotsam all end up
/// resting somewhere, and resting on water is the one condition that
/// matters. Creatures are never claimed — a thrown villager wading out
/// of the shallows is their own story, not the sea's.
#[allow(clippy::type_complexity)]
fn the_water_claims(
    mut commands: Commands,
    time: Res<Time>,
    mut since: Local<f32>,
    terrain: Option<Res<Terrain>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sounds: MessageWriter<crate::sfx::PlaySfx>,
    loose: Query<
        (
            Entity,
            &Transform,
            &Matter,
            Has<Floating>,
            Has<Rolling>,
            Has<crate::hand::DivinelyPlaced>,
        ),
        (
            Without<crate::creature::Held>,
            Without<crate::creature::Airborne>,
            Without<Sinking>,
            Without<CreatureGenome>,
            Without<crate::hand::Rooted>,
        ),
    >,
) {
    let Some(terrain) = terrain else {
        return;
    };
    *since += time.delta_secs();
    if *since < 1.2 {
        return;
    }
    *since = 0.0;

    for (entity, transform, matter, afloat, rolling, god_given) in &loose {
        let (ground, wet) =
            terrain.ground_and_water_at(transform.translation.x, transform.translation.z);
        // Only where water genuinely stands over the ground, and only for
        // things sitting at or under its surface - a log on the bank is dry.
        if wet <= ground + 0.15 || transform.translation.y > wet + matter.radius * 0.6 {
            continue;
        }
        // A splash is an ARRIVAL. Things thrown by the god, things that
        // rolled or floated in - those hit the water and say so. Things
        // merely DISCOVERED sitting wet (worldgen seeded a coastal rock,
        // a chunk streamed in with its boulders) sink without a sound:
        // nobody threw them, and Brett heard the coastline "splashing"
        // at every game start as the sweep found them one by one.
        let arrived = afloat || rolling || god_given;
        commands
            .entity(entity)
            .remove::<(Floating, Rolling, Settling)>()
            .insert(Sinking { surface: wet });
        if !arrived {
            continue;
        }
        sounds.write(crate::sfx::PlaySfx {
            kind: crate::sfx::SfxKind::Splash,
            at: Some(transform.translation),
        });
        // The splash: water-coloured flecks, gathering back to the ring.
        burst_of(
            &mut commands,
            &mut meshes,
            &mut materials,
            transform.translation.with_y(wet),
            transform.translation.with_y(wet),
            &[
                crate::palette::shade(&crate::palette::CLOTH_BLUE, 0.8),
                crate::palette::shade(&crate::palette::CLOTH_BLUE, 0.55),
                crate::palette::shade(&crate::palette::BONE, 0.95),
            ],
            18,
        );
    }
}

/// Down and gone: a claimed thing settles through the water, and once it
/// is properly under, the world lets go of it.
fn sink(
    mut commands: Commands,
    time: Res<Time>,
    mut sinking: Query<(Entity, &mut Transform, &Matter, &Sinking), Without<crate::creature::Held>>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, matter, depth) in &mut sinking {
        transform.translation.y -= dt * 1.1;
        // A slow roll on the way down, the way sunk things turn.
        transform.rotation = Quat::from_rotation_x(dt * 0.3) * transform.rotation;
        if transform.translation.y < depth.surface - matter.radius * 1.6 - 0.8 {
            commands.entity(entity).despawn();
        }
    }
}

/// The nearest lying-down orientation: the thing's own long axis (local Y,
/// which is how trees and most props are built) brought level with the
/// ground, keeping as much of the current tumble as possible.
fn lying_flat(rotation: Quat) -> Quat {
    let long = rotation * Vec3::Y;
    // Where that axis points once flattened; if the tumble left it dead
    // vertical, any horizontal will do.
    let level = (long - Vec3::Y * long.dot(Vec3::Y)).normalize_or(Vec3::X);
    Quat::from_rotation_arc(long, level) * rotation
}

/// What a landed thing still owes its resting pose.
#[derive(Component)]
pub struct Settling {
    target: Quat,
    /// What was left of the throw when it touched down: skidded out along
    /// the ground while the body rolls over, dying fast. The tad of physics
    /// between "airborne" and "furniture".
    slide: Vec3,
}

/// Eases the landed onto their sides: a short, dying tumble instead of a
/// freeze-frame - the "ragdoll" half of the ask.
fn settle(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut landed: Query<(Entity, &mut Transform, &mut Settling, &Matter), Without<Airborne>>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    // Gently - the first cut snapped flat at nine a second, and the whip at
    // the end of the arc read as MORE spin, not a landing.
    let dt = time.delta_secs();
    let ease = 1.0 - (-3.6 * dt).exp();
    let drag = (-4.2 * dt).exp();
    for (entity, mut transform, mut settling, matter) in &mut landed {
        // The skid: what the throw had left, spent along the ground and
        // gone in under half a second.
        transform.translation += settling.slide * dt;
        settling.slide *= drag;
        transform.rotation = transform.rotation.slerp(settling.target, ease);
        // Pinned to the turf while it rolls over, so no part of the turn
        // dips below ground.
        let floor = terrain
            .height_at(transform.translation.x, transform.translation.z)
            .max(WATER_LEVEL);
        transform.translation.y = floor + matter.radius * 0.5;
        if transform.rotation.angle_between(settling.target) < 0.02 {
            commands.entity(entity).remove::<Settling>();
        }
    }
}

/// Round things roll downhill, gather speed, and break what they meet.
#[allow(clippy::type_complexity)]
fn roll(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut rolling: Query<(Entity, &mut Transform, &mut Rolling, &Matter)>,
    mut victims: Query<
        (&Transform, &mut Vitality, &mut CreatureMotion),
        (With<Creature>, Without<Corpse>, Without<Rolling>),
    >,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs();

    for (entity, mut transform, mut state, matter) in &mut rolling {
        let at = transform.translation;
        let slope = downhill(&terrain, at.x, at.z);
        state.velocity = roll_step(state.velocity, slope, matter.roundness, dt);

        let speed = state.velocity.length();
        if speed < REST_SPEED {
            commands.entity(entity).remove::<Rolling>();
            continue;
        }

        transform.translation += state.velocity * dt;
        let ground = terrain.height_at(transform.translation.x, transform.translation.z);

        // Rolled into deep water: floaters float, sinkers grind to a stop.
        if ground < WATER_LEVEL {
            if matter.buoyant {
                transform.translation.y = WATER_LEVEL + matter.radius * 0.4;
                commands.entity(entity).remove::<Rolling>().insert(Floating);
                continue;
            }
            state.velocity *= 1.0 - 2.2 * dt;
        }
        transform.translation.y = ground.max(WATER_LEVEL - 20.0) + matter.radius * 0.5;

        // Spin about the axis it travels around.
        let axis = Vec3::Y.cross(state.velocity.normalize_or_zero());
        if axis.length_squared() > 1e-5 {
            transform.rotate_axis(
                Dir3::new(axis.normalize()).unwrap(),
                speed * dt / matter.radius.max(0.2),
            );
        }

        // Mass in motion is a weapon whether or not anyone meant it.
        if matter.mass * speed > CRUSH_MOMENTUM {
            for (victim, mut vitality, mut motion) in &mut victims {
                if victim.translation.distance(transform.translation) < matter.radius + 1.3 {
                    vitality.harm =
                        (vitality.harm + (matter.mass * speed / 1500.0).clamp(0.2, 1.2)).min(1.5);
                    vitality.violent = true;
                    vitality.undoing = crate::creature::Undoing::Weight;
                    motion.flail = 1.0;
                    // The blow costs the roller most of its force.
                    state.velocity *= 0.4;
                }
            }
        }
    }
}

/// Floaters ride the surface.
fn float(time: Res<Time>, mut floating: Query<(&mut Transform, &Matter), With<Floating>>) {
    let t = time.elapsed_secs();
    for (mut transform, matter) in &mut floating {
        transform.translation.y = WATER_LEVEL
            + matter.radius * 0.4
            + (t * 1.3 + transform.translation.x * 0.7).sin() * 0.08;
        // A slow settle of any leftover tilt into a lie-flat.
        transform.rotation = transform.rotation.slerp(
            Quat::from_rotation_y(transform.translation.x),
            0.2 * time.delta_secs(),
        );
    }
}

/// What a placed deposit holds. Deposits are the map making demands:
/// iron wants the far hills, clay wants the wet banks, stone wants the
/// broken ground, and wanting any of them means walking there and carrying
/// it home.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DepositKind {
    Iron,
    Clay,
    /// A quarry: a face of workable rock, cut rather than gathered.
    ///
    /// Added when the loose stone came off the ground. Every scatter point in
    /// the world had a one-in-twenty chance of a pebble, which is ten a chunk
    /// over every chunk there is, and Brett — twice now — "there is still
    /// entirely too many rocks on the ground". The village still has to build
    /// out of something, so the stone that was strewn everywhere is gathered
    /// into places worth walking to.
    ///
    /// It is a deposit and not a building on purpose. Miners already work
    /// deposits, the survey overlay already has a colour called "quarry rock",
    /// and there is already a `Mine` for the other half of this — a drift
    /// driven into a hillside, which only hill country can offer. A quarry is
    /// what flat country gets instead.
    Stone,
}

impl DepositKind {
    pub fn title(self) -> &'static str {
        match self {
            DepositKind::Iron => "A hillside veined with iron",
            DepositKind::Clay => "A bank of good red clay",
            DepositKind::Stone => "A quarry of good building stone",
        }
    }
}

/// A worked deposit: what it is and how much is left in the ground.
#[derive(Component, Debug)]
pub struct Deposit {
    pub kind: DepositKind,
    pub amount: f32,
}

/// Raises a deposit in the world: a rust-streaked outcrop for iron, a low
/// red mound for clay. Same procedural cloth as everything else.
pub(crate) fn spawn_deposit(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    kind: DepositKind,
    amount: f32,
) -> Entity {
    use crate::palette as pal;
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let root = commands
        .spawn((
            Name::new(kind.title()),
            Deposit { kind, amount },
            Transform::from_translation(at),
            Visibility::default(),
            crate::hand::PickRadius(2.2),
            crate::hand::Rooted,
            crate::globe::RigidlySeated,
        ))
        .id();
    match kind {
        DepositKind::Iron => {
            let dark = materials.add(StandardMaterial {
                base_color: pal::shade(&pal::STONE, 0.28),
                perceptual_roughness: 1.0,
                ..default()
            });
            let rust = materials.add(StandardMaterial {
                base_color: Color::srgb(0.48, 0.26, 0.14),
                perceptual_roughness: 1.0,
                ..default()
            });
            for (x, z, s, h, rusty) in [
                (0.0, 0.0, 1.6, 1.4, false),
                (1.1, 0.5, 1.0, 0.9, false),
                (-0.9, 0.6, 1.1, 1.0, true),
                (0.4, -0.9, 0.9, 0.7, true),
                (-0.5, -0.7, 0.7, 0.5, false),
            ] {
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(if rusty { rust.clone() } else { dark.clone() }),
                    Transform::from_xyz(x, h * 0.4, z)
                        .with_rotation(Quat::from_rotation_y(x + z))
                        .with_scale(Vec3::new(s, h, s * 0.85)),
                    ChildOf(root),
                ));
            }
        }
        DepositKind::Clay => {
            let clay = materials.add(StandardMaterial {
                base_color: Color::srgb(0.62, 0.36, 0.24),
                perceptual_roughness: 1.0,
                ..default()
            });
            for (x, z, s) in [(0.0, 0.0, 2.4), (1.3, 0.8, 1.5), (-1.2, -0.6, 1.7)] {
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(clay.clone()),
                    Transform::from_xyz(x, 0.14, z)
                        .with_rotation(Quat::from_rotation_y(x - z))
                        .with_scale(Vec3::new(s, 0.3, s * 0.8)),
                    ChildOf(root),
                ));
            }
        }
        // A quarry is a natural outcrop somebody has been at with a chisel,
        // and it has to read as BOTH halves of that at once.
        //
        // The first cut was all squared benches, and Brett: "it looks weird
        // lol". Two things were wrong. It stood on unworked ground, so on any
        // slope half of it hung in the air — fixed by cutting a pit first, see
        // `villager::QUARRY_DEPTH`, which is why everything below can assume a
        // flat floor at zero. And it was squared all the way through, which
        // reads as a BUILDING; three of them in view had him asking why the
        // village had put them up.
        //
        // So: a rough mass at the back, turned off-axis and unequal the way
        // the outcrops are, and the worked part squared, pale and axis-aligned
        // against it. The contrast is the whole story — the eye reads the
        // straight edges as somebody's work precisely because the rest is not.
        DepositKind::Stone => {
            let rock = materials.add(StandardMaterial {
                base_color: pal::shade(&pal::STONE, 0.45),
                perceptual_roughness: 1.0,
                ..default()
            });
            let cut = materials.add(StandardMaterial {
                base_color: pal::shade(&pal::STONE, 0.80),
                perceptual_roughness: 0.9,
                ..default()
            });

            // The living rock: a mass standing out of the pit floor, no two
            // pieces alike and none of them square to the working.
            for (x, y, z, w, h, d, turn) in [
                (-0.6, 1.5, -3.0, 5.2, 3.0, 2.2, 0.13),
                (2.4, 1.0, -2.6, 2.6, 2.0, 2.0, -0.31),
                (-3.2, 0.9, -1.6, 2.2, 1.8, 2.4, 0.42),
                (0.9, 0.6, -1.7, 2.0, 1.2, 1.6, -0.18),
            ] {
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(rock.clone()),
                    Transform::from_xyz(x, y, z)
                        .with_rotation(Quat::from_rotation_y(turn))
                        .with_scale(Vec3::new(w, h, d)),
                    ChildOf(root),
                ));
            }

            // The cut: a squared face taken out of the mass, and one bench
            // below it. Dead straight, dead level, and paler where the stone
            // has been opened.
            for (x, y, z, w, h, d) in [
                (-0.6, 1.05, -1.75, 4.0, 2.1, 0.5),
                (-0.6, 0.30, -1.15, 4.0, 0.6, 0.9),
            ] {
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(cut.clone()),
                    Transform::from_xyz(x, y, z).with_scale(Vec3::new(w, h, d)),
                    ChildOf(root),
                ));
            }

            // And the blocks got out of it, squared and stacked on the floor
            // where a barrow would come for them.
            for (x, y, z) in [(2.1, 0.4, 0.6), (3.0, 0.4, 0.9), (2.1, 1.15, 0.6)] {
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(cut.clone()),
                    Transform::from_xyz(x, y, z).with_scale(Vec3::splat(0.75)),
                    ChildOf(root),
                ));
            }
        }
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_accelerates_downhill_and_stalls_on_flat() {
        let downhill = Vec3::new(0.6, 0.0, 0.0);
        let mut v = Vec3::new(0.5, 0.0, 0.0);
        for _ in 0..60 {
            v = roll_step(v, downhill, 0.82, 1.0 / 60.0);
        }
        assert!(v.x > 0.5, "a boulder on a slope should gather speed");

        let mut flat = Vec3::new(4.0, 0.0, 0.0);
        for _ in 0..600 {
            flat = roll_step(flat, Vec3::ZERO, 0.82, 1.0 / 60.0);
        }
        assert!(flat.length() < REST_SPEED, "flat ground must bleed it out");
    }

    #[test]
    fn shape_decides_how_far_things_roll() {
        let slope = Vec3::new(0.3, 0.0, 0.0);
        let mut boulder = Vec3::new(2.0, 0.0, 0.0);
        let mut bush = boulder;
        for _ in 0..120 {
            boulder = roll_step(boulder, slope, 0.82, 1.0 / 60.0);
            bush = roll_step(bush, slope, 0.15, 1.0 / 60.0);
        }
        assert!(boulder.length() > bush.length() * 1.5);
    }

    #[test]
    fn mass_takes_the_snap_out_of_a_throw() {
        let pebble = Matter::boulder(20.0, 0.4);
        let boulder = Matter::boulder(150.0, 1.0);
        assert!(pebble.throw_factor() > boulder.throw_factor() * 2.0);
        let log = Matter::felled_tree(1.0);
        assert!(log.throw_factor() < 1.0);
    }
}
