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
        app.add_systems(Update, (loose_ballistics, roll, float).chain());
    }
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
}

impl Matter {
    pub fn boulder(mass: f32, radius: f32) -> Matter {
        Matter {
            substance: Substance::Stone,
            mass,
            roundness: 0.82,
            buoyant: false,
            radius,
        }
    }

    pub fn felled_tree(maturity: f32) -> Matter {
        Matter {
            substance: Substance::Wood,
            mass: 30.0 + maturity * 50.0,
            roundness: 0.55,
            buoyant: true,
            radius: 0.5,
        }
    }

    pub fn bush() -> Matter {
        Matter {
            substance: Substance::Plant,
            mass: 6.0,
            roundness: 0.15,
            buoyant: true,
            radius: 0.4,
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
    mut objects: Query<(Entity, &mut Transform, &mut Airborne, &Matter), Without<CreatureGenome>>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let dt = time.delta_secs();

    for (entity, mut transform, mut body, matter) in &mut objects {
        body.velocity.y -= GRAVITY * dt;
        transform.translation += body.velocity * dt;

        let spin = body.velocity.length() * dt / matter.radius.max(0.2) * 0.4;
        transform.rotate_local_x(spin);

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
            } else {
                commands.entity(entity).remove::<Airborne>();
            }
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
/// iron wants the far hills, clay wants the wet banks, and wanting either
/// means walking there and carrying it home.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DepositKind {
    Iron,
    Clay,
}

impl DepositKind {
    pub fn title(self) -> &'static str {
        match self {
            DepositKind::Iron => "A hillside veined with iron",
            DepositKind::Clay => "A bank of good red clay",
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
