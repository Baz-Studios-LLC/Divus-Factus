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
        Has<super::Held>,
        Has<super::Airborne>,
    )>,
    mut transforms: Query<&mut Transform>,
) {
    let t = time.elapsed_secs();

    for (rig, genome, motion, global, held, airborne) in &creatures {
        let gait = &genome.gait;

        // Walking blend: 0 standing still, 1 at full walking speed. Everything
        // scales off this so the transition into and out of motion is continuous.
        let walk = (motion.speed / genome.walk_speed().max(0.01)).clamp(0.0, 1.4);
        let idle = 1.0 - walk.min(1.0);
        let breath = ((t * 1.1 + motion.idle_offset) * std::f32::consts::TAU * 0.25).sin();

        let (sin_phase, cos_phase) = motion.phase.sin_cos();

        // Body: bob at twice stride frequency (one dip per footfall), sway at
        // stride frequency, lean forward proportional to speed.
        if let Ok(mut transform) = transforms.get_mut(rig.body) {
            let bob = -(motion.phase * 2.0).cos() * gait.bounce * rig.height * walk;
            let breathe = breath * 0.004 * rig.height * idle;
            let sway = sin_phase * gait.sway * walk;
            let lean = gait.lean + walk * 0.09 + motion.flail * 0.25;

            transform.translation = Vec3::new(0.0, bob + breathe, 0.0);
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
            let pitch = -walk * 0.06 + breath * 0.02 * idle;
            let counter_sway = -sin_phase * gait.sway * 0.6 * walk;

            // Composed onto the rest pose rather than replacing the transform. A
            // quadruped's head is counter-rotated at build time to sit level on its
            // angled neck; overwriting that left every animal staring at the sky.
            transform.rotation = rig.head_rest
                * Quat::from_rotation_y(yaw)
                * Quat::from_rotation_x(pitch)
                * Quat::from_rotation_z(counter_sway);
        }

        // Limbs: one sine each, offset by the limb's phase.
        for limb in &rig.limbs {
            let Ok(mut transform) = transforms.get_mut(limb.entity) else {
                continue;
            };

            let amplitude = if limb.is_arm {
                gait.stride_swing * 0.55
            } else {
                gait.stride_swing
            };

            let swing = (motion.phase + limb.phase).sin() * amplitude * walk;

            // Flailing is two different things. Off the ground it is panic,
            // and every limb kicks. On the ground it is *work* — hammering,
            // reaping, tending — arms raised out in front and pumping, while
            // the legs are stood on and stay put.
            let off_ground = held || airborne;
            let working = motion.flail > 0.0 && !off_ground;
            let flail = if motion.flail > 0.0 && (limb.is_arm || off_ground) {
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

            transform.rotation = Quat::from_rotation_x(swing + flail + drift);
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
