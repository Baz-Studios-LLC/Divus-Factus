//! Procedural animation.
//!
//! Nothing here is keyframed. A creature whose proportions are decided at runtime
//! cannot have hand-authored animation, so gait is computed from the genome: stride
//! frequency scales with leg length, swing amplitude with speed, bob with stride.
//! Generate a creature with longer legs and its walk adapts on its own.
//!
//! The whole system is one sine wave per limb plus phase offsets. That is enough,
//! because at the render resolution the silhouette is doing the work.

use bevy::prelude::*;

use super::body::CreatureRig;
use super::genome::CreatureGenome;
use super::genome::Garment;

/// Upper-arm and elbow angles for a person working at a real job. The two arms
/// intentionally differ: a pick needs a bracing hand, a basket stays low, and
/// a censer swings from a relaxed wrist rather than both arms pumping alike.
fn work_arm_pose(
    vocation: crate::villager::work::Vocation,
    arm: usize,
    time: f32,
    offset: f32,
) -> (f32, f32) {
    use crate::villager::work::Vocation;
    let right = arm == 1;
    let strike = (time * 4.7 + offset).sin();
    let tap = (time * 7.5 + offset).sin().max(0.0);
    match vocation {
        Vocation::Forester | Vocation::Miner => {
            if right {
                (-0.75 + strike * 0.92, 1.05)
            } else {
                (-0.48 + strike * 0.54, 0.82)
            }
        }
        Vocation::Builder => {
            if right {
                (-0.42 + tap * 0.72, 1.02)
            } else {
                (-0.34, 0.78)
            }
        }
        Vocation::Farmer => {
            if right {
                (-0.56 + strike * 0.38, 0.96)
            } else {
                (-0.42 - strike * 0.22, 0.88)
            }
        }
        Vocation::Fisher => {
            if right {
                (-0.48 + (time * 2.4 + offset).sin() * 0.16, 0.82)
            } else {
                (-0.30, 0.62)
            }
        }
        Vocation::Hunter => {
            if right {
                // The drawing hand comes back and releases; the bow hand is
                // held out toward the quarry instead of making a spear jab.
                (-0.18 + (time * 2.1 + offset).sin().max(0.0) * 0.42, 1.08)
            } else {
                (-0.82, 0.72)
            }
        }
        Vocation::Guard => {
            if right {
                (-0.20, 0.64)
            } else {
                (-0.34, 0.72)
            }
        }
        Vocation::Gatherer => {
            if right {
                (-0.22 + (time * 3.5 + offset).sin() * 0.22, 0.70)
            } else {
                (-0.16, 0.60)
            }
        }
        Vocation::Cook => {
            if right {
                (-0.28, 0.90)
            } else {
                (-0.20, 0.62)
            }
        }
        Vocation::Healer => {
            if right {
                (-0.22 + (time * 2.8 + offset).sin() * 0.14, 0.90)
            } else {
                (-0.30, 0.78)
            }
        }
        Vocation::Priest => {
            if right {
                (0.04, 0.76)
            } else {
                (-0.08, 0.70)
            }
        }
        Vocation::Explorer => {
            if right {
                (-0.16, 0.62)
            } else {
                (-0.04, 0.48)
            }
        }
    }
}

/// Per-creature animation state, advanced every frame.
#[derive(Component, Default)]
pub struct CreatureMotion {
    /// Ground speed in world units per second. Written by movement systems.
    pub speed: f32,
    /// Position within the stride cycle, in radians.
    pub phase: f32,
    /// How hard the creature is struggling, 0 to 1. Raised while held or falling.
    pub flail: f32,
    /// How deep in water this creature is, 0 dry to 1 swimming. Slows the
    /// stride and floats the body at the surface.
    pub swim: f32,
    /// Per-creature offset so idle motion does not synchronise across a crowd.
    pub idle_offset: f32,
    /// A world point this creature is watching, if any.
    ///
    /// A head that tracks the player across a field communicates more than any
    /// number on a panel — it is the difference between a crowd of props and a crowd
    /// that has noticed you.
    pub look_at: Option<Vec3>,
    /// Whether this creature should be on its knees. Written by whatever
    /// system owns the reason (prayer, for now); the blend eases so they
    /// sink down and rise rather than snapping.
    pub kneeling: bool,
    /// The kneel blend the animator actually drives, 0 standing to 1 down.
    pub kneel: f32,
}

impl CreatureMotion {
    pub fn new(idle_offset: f32) -> Self {
        CreatureMotion {
            idle_offset,
            ..default()
        }
    }
}

/// Advances stride phase and decays flailing.
pub fn advance_motion(
    time: Res<Time>,
    mut creatures: Query<(&CreatureGenome, &mut CreatureMotion)>,
) {
    let dt = time.delta_secs();

    for (genome, mut motion) in &mut creatures {
        // Stride frequency is set by leg length: short legs take more steps to
        // cover the same ground. Without this, small creatures look like they are
        // gliding and large ones look like they are wading.
        let leg_len = (genome.proportions.leg_length * genome.height()).max(0.15);
        let steps_per_second = motion.speed / (leg_len * 1.7);
        let rate = steps_per_second * genome.gait.stride_rate * std::f32::consts::PI;

        motion.phase = (motion.phase + rate * dt).rem_euclid(std::f32::consts::TAU);

        // Flailing decays on its own, so anything that sets it can just poke it up
        // and forget about clearing it.
        motion.flail = (motion.flail - dt * 1.2).max(0.0);

        // Kneeling eases toward its target: sinking down is a little slower
        // than rising, the way weight actually moves.
        let kneel_to = if motion.kneeling { 1.0 } else { 0.0 };
        let rate = if motion.kneeling { 2.2 } else { 3.2 };
        motion.kneel += (kneel_to - motion.kneel).clamp(-rate * dt, rate * dt);
    }
}

/// Drives every rigged part from the creature's motion state.
pub fn animate_creatures(
    time: Res<Time>,
    creatures: Query<(
        &CreatureRig,
        &CreatureGenome,
        &CreatureMotion,
        &GlobalTransform,
        Option<&crate::villager::work::Vocation>,
        Option<&crate::villager::Activity>,
        Has<super::Held>,
        Has<super::Airborne>,
        Has<super::Laden>,
    )>,
    mut transforms: Query<&mut Transform>,
) {
    let t = time.elapsed_secs();

    for (rig, genome, motion, global, vocation, activity, held, airborne, laden) in &creatures {
        let gait = &genome.gait;

        // Walking blend: 0 standing still, 1 at full walking speed. Everything
        // scales off this so the transition into and out of motion is continuous.
        let walk = (motion.speed / genome.walk_speed().max(0.01)).clamp(0.0, 1.4);
        let idle = 1.0 - walk.min(1.0);
        let breath = ((t * 1.1 + motion.idle_offset) * std::f32::consts::TAU * 0.25).sin();

        let (sin_phase, cos_phase) = motion.phase.sin_cos();

        // Body: bob at twice stride frequency (one dip per footfall), sway at
        // stride frequency, lean forward proportional to speed.
        let kneel = motion.kneel;
        let leg_len = genome.proportions.leg_length * rig.height;
        if let Ok(mut transform) = transforms.get_mut(rig.body) {
            let bob = -(motion.phase * 2.0).cos() * gait.bounce * rig.height * walk;
            let breathe = breath * 0.004 * rig.height * idle;
            let sway = sin_phase * gait.sway * walk;
            // Forward is negative-X in this rig: lean INTO the stride.
            // Kneeling bows the body forward a little over the folded legs.
            let lean =
                -(gait.lean + walk * 0.09) - kneel * 0.18 + motion.flail * 0.25 * (1.0 - kneel);

            // Folded legs sink the body: the shins lie along the ground and
            // the weight rests just above the heels.
            let sink = kneel * leg_len * 0.48;
            transform.translation = Vec3::new(0.0, bob + breathe - sink, 0.0);
            transform.rotation = Quat::from_rotation_z(sway) * Quat::from_rotation_x(lean);
        }

        // Head: counter-rotates against body sway so it stays level, plus a slow
        // idle scan. Villagers who never move their heads read as props.
        if let Ok(mut transform) = transforms.get_mut(rig.head) {
            // Watching something overrides the idle scan entirely. The yaw is worked
            // out in the creature's own frame, so it holds however the body is turned.
            let yaw = match motion.look_at {
                Some(target) => {
                    let to_target = target - global.translation();
                    let local = global.rotation().inverse() * to_target;
                    // Clamped to what a neck can manage; past that they would have to
                    // turn their whole body, which the walk cycle handles.
                    local.x.atan2(-local.z).clamp(-1.2, 1.2)
                }
                None => {
                    let scan = ((t * 0.37 + motion.idle_offset * 3.1).sin()
                        * (t * 0.23 + motion.idle_offset).cos())
                        * 0.5;
                    scan * idle * 0.6
                }
            };
            // A kneeler bows their head; the breath stays, which reads as
            // murmured prayer rather than a statue.
            let pitch = -walk * 0.06 + breath * 0.02 * idle + kneel * 0.38;
            let counter_sway = -sin_phase * gait.sway * 0.6 * walk;

            // Composed onto the rest pose rather than replacing the transform. A
            // quadruped's head is counter-rotated at build time to sit level on its
            // angled neck; overwriting that left every animal staring at the sky.
            transform.rotation = rig.head_rest
                * Quat::from_rotation_y(yaw)
                * Quat::from_rotation_x(pitch)
                * Quat::from_rotation_z(counter_sway);
        }

        // Limbs: one sine per segment pair, offset by the limb's phase. The
        // upper joint swings the whole limb; the hinge bends against it.
        //
        // Sign convention, settled the way the sleeper's was - by looking:
        // for a box hanging on -Y, POSITIVE X rotation carries its free end
        // forward. So elbows bend positive (forearm rises in front) and
        // knees bend negative (shin folds behind).
        let at_work = activity
            .is_some_and(|activity| *activity == crate::villager::Activity::Working)
            && vocation.is_some()
            && walk < 0.2
            && !held;
        let mut arm_number = 0usize;
        for limb in &rig.limbs {
            let Ok(mut transform) = transforms.get_mut(limb.entity) else {
                continue;
            };

            // A long robe does not allow a long stride: robed legs take
            // shorter, straighter steps, which also keeps the knees inside
            // the skirt.
            let robed = !limb.is_arm
                && genome.species.is_biped()
                && matches!(genome.garment, Garment::Robe);
            let amplitude = if limb.is_arm {
                gait.stride_swing * 0.55
            } else if robed {
                gait.stride_swing * 0.6
            } else {
                gait.stride_swing
            };

            // Laden arms hold a burden to the chest: the swing all but stops.
            let carry = if limb.is_arm && laden { 0.15 } else { 1.0 };
            let swing = (motion.phase + limb.phase).sin() * amplitude * walk * carry;

            // Flailing is two different things. Off the ground it is panic,
            // and every limb kicks. On the ground it is *work* — hammering,
            // reaping, tending — arms raised out in front and pumping, while
            // the legs are stood on and stay put.
            let off_ground = held || airborne;
            let working = motion.flail > 0.0 && !off_ground;
            let craft_pose = if limb.is_arm && at_work {
                let arm = arm_number;
                arm_number += 1;
                vocation.map(|vocation| work_arm_pose(*vocation, arm, t, motion.idle_offset))
            } else {
                None
            };
            let flail = if craft_pose.is_some() {
                0.0
            } else if motion.flail > 0.0 && (limb.is_arm || off_ground) {
                let thrash = ((t * 13.0 + limb.phase * 2.0).sin()) * motion.flail;
                if working {
                    // Lift the arms forward and pump around that pose.
                    -1.05 * motion.flail.min(1.0) + thrash * 0.4
                } else {
                    thrash * 0.9
                }
            } else {
                0.0
            };

            // Idle arms drift very slightly, which stops standing villagers from
            // looking like mannequins.
            let drift = if limb.is_arm {
                breath * 0.03 * idle
            } else {
                0.0
            };

            // Kneeling: the sink does most of the work; the thighs tilt
            // gently forward over the grounded knees, the arms lift a
            // little toward the clasp of prayer.
            let kneel_upper = if limb.is_arm {
                kneel * 0.15
            } else {
                kneel * 0.35
            };

            let crafted_upper = craft_pose.map_or(0.0, |pose| pose.0);
            transform.rotation = Quat::from_rotation_x(
                (swing + flail + drift + crafted_upper) * (1.0 - kneel) + kneel_upper,
            );

            // The hinge. Bends are one-signed - a knee does not hyperextend -
            // so each contribution is shaped before it is summed.
            let Ok(mut hinge) = transforms.get_mut(limb.lower) else {
                continue;
            };
            let bend = if limb.is_arm {
                // A living elbow is never quite straight; it bends further as
                // the arm swings forward, while working pumps it around a
                // hammering angle, burdens hold it fast to the chest, and
                // panic in the air throws it around.
                let rest = 0.18;
                let stride = swing.max(0.0) * 0.8;
                let work = if let Some(pose) = craft_pose {
                    pose.1
                } else if working {
                    // Damped by the kneel like every stride term: a pray-er
                    // brushed by a nearby commotion must not hammer the air.
                    (0.9 + ((t * 13.0 + limb.phase).sin()) * 0.35)
                        * motion.flail.min(1.0)
                        * (1.0 - kneel)
                } else if off_ground && motion.flail > 0.0 {
                    ((t * 11.0 + limb.phase * 3.0).sin()) * motion.flail * 0.7
                } else {
                    0.0
                };
                let hold = if laden { 1.25 } else { 0.0 };
                let pray = kneel * 0.55;
                (rest + stride * (1.0 - kneel) + work + hold).max(0.0) + pray
            } else {
                // The knee: a touch of life standing, a bend that peaks as the
                // leg swings through its recovery, folded right under when
                // kneeling, kicking when carried off.
                let rest = -0.06;
                let knee_room = if robed { 0.3 } else { 1.15 };
                // The cosine is positive exactly while this leg sweeps from
                // back to front - the recovery - so the knee folds through
                // the swing and lands straight for the plant. The first cut
                // used a lagged sine, and the review caught it bending knees
                // while the leg stood planted.
                let stride = -(motion.phase + limb.phase).cos().max(0.0)
                    * gait.stride_swing
                    * knee_room
                    * walk;
                let panic = if off_ground && motion.flail > 0.0 {
                    -((t * 12.0 + limb.phase * 2.0).sin().abs()) * motion.flail * 0.8
                } else {
                    0.0
                };
                let fold = kneel * -1.85;
                (rest + stride * (1.0 - kneel) + panic).min(0.0) + fold
            };
            hinge.rotation = Quat::from_rotation_x(bend);
        }

        // Tail: trails behind the stride, with a slow idle swish.
        if let Some(tail) = rig.tail
            && let Ok(mut transform) = transforms.get_mut(tail)
        {
            let swish = cos_phase * 0.35 * walk + breath * 0.18 * idle;
            let lift = walk * 0.3 + motion.flail * 0.5;
            transform.rotation = Quat::from_rotation_z(swish) * Quat::from_rotation_x(-lift);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creature::genome::Species;
    use crate::rng::Rng;

    fn genome(species: Species, seed: u64) -> CreatureGenome {
        CreatureGenome::random(species, &mut Rng::new(seed))
    }

    /// Reimplements the phase advance from `advance_motion` for a single step, so
    /// the timing maths can be tested without standing up a full Bevy app.
    fn step_phase(genome: &CreatureGenome, motion: &mut CreatureMotion, dt: f32) {
        let leg_len = (genome.proportions.leg_length * genome.height()).max(0.15);
        let steps_per_second = motion.speed / (leg_len * 1.7);
        let rate = steps_per_second * genome.gait.stride_rate * std::f32::consts::PI;
        motion.phase = (motion.phase + rate * dt).rem_euclid(std::f32::consts::TAU);
    }

    #[test]
    fn standing_creatures_do_not_advance_their_stride() {
        let g = genome(Species::Human, 1);
        let mut m = CreatureMotion::new(0.0);
        m.speed = 0.0;
        for _ in 0..100 {
            step_phase(&g, &mut m, 1.0 / 60.0);
        }
        assert_eq!(m.phase, 0.0);
    }

    #[test]
    fn phase_stays_bounded_over_long_runs() {
        let g = genome(Species::Wolf, 2);
        let mut m = CreatureMotion::new(0.0);
        m.speed = g.walk_speed();
        for _ in 0..100_000 {
            step_phase(&g, &mut m, 1.0 / 60.0);
            assert!(m.phase >= 0.0 && m.phase < std::f32::consts::TAU);
        }
    }

    #[test]
    fn longer_legs_take_fewer_steps_over_the_same_distance() {
        // The whole point of deriving gait from the genome: proportions change the
        // walk without anyone authoring a second animation.
        let mut short = genome(Species::Human, 3);
        let mut tall = short.clone();
        short.proportions.leg_length = 0.42;
        tall.proportions.leg_length = 0.52;
        // Hold stride rate equal so only leg length differs.
        tall.gait.stride_rate = short.gait.stride_rate;

        let distance = 10.0;
        let speed = 2.0;
        let dt = 1.0 / 120.0;
        let steps = (distance / speed / dt) as usize;

        let advance = |g: &CreatureGenome| {
            let mut m = CreatureMotion::new(0.0);
            m.speed = speed;
            let mut total = 0.0;
            for _ in 0..steps {
                let before = m.phase;
                step_phase(g, &mut m, dt);
                total += (m.phase - before).rem_euclid(std::f32::consts::TAU);
            }
            total
        };

        assert!(advance(&short) > advance(&tall));
    }

    #[test]
    fn faster_movement_means_faster_stride() {
        let g = genome(Species::Deer, 4);
        let mut slow = CreatureMotion::new(0.0);
        slow.speed = 1.0;
        let mut fast = CreatureMotion::new(0.0);
        fast.speed = 4.0;

        step_phase(&g, &mut slow, 0.1);
        step_phase(&g, &mut fast, 0.1);
        assert!(fast.phase > slow.phase);
    }

    #[test]
    fn flail_decays_to_zero() {
        let mut m = CreatureMotion::new(0.0);
        m.flail = 1.0;
        for _ in 0..200 {
            m.flail = (m.flail - (1.0 / 60.0) * 1.2).max(0.0);
        }
        assert_eq!(m.flail, 0.0);
    }

    #[test]
    fn idle_offsets_desynchronise_a_crowd() {
        // Identical idle motion across every villager is instantly readable as fake.
        let offsets: Vec<f32> = (0..20).map(|i| i as f32 * 0.31).collect();
        let t = 3.0;
        let values: Vec<f32> = offsets
            .iter()
            .map(|o| ((t * 1.1 + o) * std::f32::consts::TAU * 0.25).sin())
            .collect();
        for i in 1..values.len() {
            assert!((values[i] - values[0]).abs() > 1e-6 || i == 0);
        }
    }
}
