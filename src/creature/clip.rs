//! Authored clips, laid over the animator's own motion.
//!
//! A clip is a handful of moments drawn on the Atelier's rig bench, each holding
//! a rotation for the joints that moved in it. The game reads them out of
//! `assets/clips/*.baz` and plays one on a villager whose activity it is named
//! for: `praying.baz` plays on anyone praying, `chatting.baz` on anyone stopped
//! to trade news.
//!
//! # Over, not instead of
//!
//! The sines in [`super::anim`] keep the walk, the breath, the head-scan and the
//! lean, because those answer to speed and ground and neither belongs in a
//! drawn clip. A clip writes ONLY the joints its keys name, and it writes them
//! after the animator has had its say. So a clip that keys two arms leaves the
//! legs walking, which is Brett's call and the useful one: "Clips for actions,
//! sines for locomotion."
//!
//! The consequence a maker should know: keying a joint at all takes that joint
//! away from the animator for the whole clip. A chop that keys the legs would
//! stand its villager still while they chopped, however fast they were moving.
//!
//! # Rotations only
//!
//! Clips hold joint rotations and nothing else, which is what lets one clip play
//! on every body in the village - a child's arm is not their father's arm, but
//! an elbow bent ninety degrees is bent ninety degrees on both.

use bevy::prelude::*;
use std::collections::BTreeMap;

use super::body::{CreatureRig, name_the_joints};

/// One moment of a clip.
#[derive(serde::Deserialize, Clone)]
struct Key {
    at: f32,
    pose: BTreeMap<String, [f32; 4]>,
}

/// A whole clip as the bench wrote it.
#[derive(serde::Deserialize, Clone)]
pub struct Clip {
    pub name: String,
    length: f32,
    #[serde(rename = "loop")]
    looping: bool,
    keys: Vec<Key>,
    /// The word the bench puts in every file it writes, so one picker can open
    /// a building or a clip. A building carried in here would be nonsense.
    #[serde(default)]
    kind: String,
}

impl Clip {
    /// This joint's first key, or its last.
    fn edge_key(&self, joint: &str, first: bool) -> Option<(f32, Quat)> {
        let read = |key: &Key| {
            key.pose
                .get(joint)
                .map(|turn| (key.at, Quat::from_array(*turn)))
        };
        if first {
            self.keys.iter().find_map(read)
        } else {
            self.keys.iter().rev().find_map(read)
        }
    }

    /// Where each keyed joint stands at `t`, in the joint's own frame.
    fn pose_at(&self, t: f32) -> BTreeMap<&str, Quat> {
        let mut posed = BTreeMap::new();
        let keyed: Vec<&str> = {
            let mut names: Vec<&str> = self
                .keys
                .iter()
                .flat_map(|key| key.pose.keys().map(String::as_str))
                .collect();
            names.sort_unstable();
            names.dedup();
            names
        };
        for joint in keyed {
            let mut before: Option<(f32, Quat)> = None;
            let mut after: Option<(f32, Quat)> = None;
            for key in &self.keys {
                let Some(turn) = key.pose.get(joint) else {
                    continue;
                };
                let turn = Quat::from_array(*turn);
                if key.at <= t && before.is_none_or(|(had, _)| key.at >= had) {
                    before = Some((key.at, turn));
                }
                if key.at >= t && after.is_none_or(|(had, _)| key.at <= had) {
                    after = Some((key.at, turn));
                }
            }
            // Across the seam for a looping clip: the last key turns toward the
            // first through the end of the clip, so the loop closes rather than
            // snapping. The bench does this too - a clip that read one way there
            // and another way here would be worse than either.
            let turn = match (before, after) {
                (Some((a, from)), Some((b, to))) if (b - a).abs() > 1e-4 => {
                    from.slerp(to, ((t - a) / (b - a)).clamp(0.0, 1.0))
                }
                (Some((a, from)), None) if self.looping => match self.edge_key(joint, true) {
                    Some((b, to)) => {
                        let over = (self.length - a + b).max(1e-4);
                        from.slerp(to, ((t - a) / over).clamp(0.0, 1.0))
                    }
                    None => from,
                },
                (None, Some((b, to))) if self.looping => match self.edge_key(joint, false) {
                    Some((a, from)) => {
                        let over = (self.length - a + b).max(1e-4);
                        from.slerp(to, ((t + self.length - a) / over).clamp(0.0, 1.0))
                    }
                    None => to,
                },
                (Some((_, only)), _) | (None, Some((_, only))) => only,
                (None, None) => continue,
            };
            posed.insert(joint, turn);
        }
        posed
    }
}

/// Every clip carried in, read once and held for the life of the run - the same
/// arrangement the baked buildings keep, and for the same reason: the raising
/// happens deep inside systems that would otherwise all have to be handed it.
static CARRIED: std::sync::OnceLock<Vec<Clip>> = std::sync::OnceLock::new();

fn carried() -> &'static Vec<Clip> {
    CARRIED.get_or_init(|| {
        let mut clips: Vec<Clip> = Vec::new();
        for dir in [
            crate::carried::folder("assets/clips"),
            crate::carried::made_by_hand("clips"),
        ]
        .into_iter()
        .flatten()
        {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|kind| kind != "baz") {
                    continue;
                }
                match std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Clip>(&text).ok())
                {
                    // A maker's clip of the same name replaces the shipped one.
                    Some(clip) if clip.kind == "clip" && !clip.keys.is_empty() => {
                        info!(
                            "carried in the clip {}: {} keys over {:.2}s",
                            clip.name,
                            clip.keys.len(),
                            clip.length
                        );
                        match clips.iter().position(|had| had.name == clip.name) {
                            Some(standing) => clips[standing] = clip,
                            None => clips.push(clip),
                        }
                    }
                    Some(_) => warn!("{} is not a clip", path.display()),
                    None => warn!("could not read the clip at {}", path.display()),
                }
            }
        }
        clips.sort_by(|a, b| a.name.cmp(&b.name));
        clips
    })
}

/// The clip of a given name, if a maker has drawn one.
pub fn clip_called(name: &str) -> Option<&'static Clip> {
    carried().iter().find(|clip| clip.name == name)
}

/// A clip running on one body.
#[derive(Component)]
pub struct Playing {
    clip: &'static Clip,
    /// Where this body stands in it. Its own, so two villagers praying side by
    /// side are not two copies of the same frame.
    t: f32,
}

impl Playing {
    pub fn of(clip: &'static Clip) -> Self {
        Playing { clip, t: 0.0 }
    }

    pub fn is(&self, name: &str) -> bool {
        self.clip.name == name
    }
}

/// Runs every clip that is playing, over whatever the animator just wrote.
///
/// After `animate_creatures` in the schedule, and that ordering is the whole
/// design: the sines write every joint, and the clip writes back over the few it
/// was drawn with.
pub fn play_clips(
    time: Res<Time>,
    mut commands: Commands,
    mut bodies: Query<(Entity, &CreatureRig, &mut Playing)>,
    mut joints: Query<&mut Transform>,
) {
    for (entity, rig, mut playing) in &mut bodies {
        playing.t += time.delta_secs();
        if playing.t >= playing.clip.length {
            if playing.clip.looping {
                playing.t %= playing.clip.length.max(0.01);
            } else {
                // A clip that does not loop is done, and the body goes back to
                // the animator's own motion entirely.
                commands.entity(entity).remove::<Playing>();
                continue;
            }
        }
        let posed = playing.clip.pose_at(playing.t);
        let named = name_the_joints(rig, |joint| {
            joints
                .get(joint)
                .map(|at| at.translation.x)
                .unwrap_or_default()
        });
        for (joint, name) in named {
            let Some(turn) = posed.get(name) else {
                continue;
            };
            if let Ok(mut at) = joints.get_mut(joint) {
                at.rotation = *turn;
            }
        }
    }
}

/// Starts and stops clips as villagers go about their day.
///
/// The hook is deliberately the plainest one there is: a clip is played on
/// anyone whose activity it is NAMED for. Draw `praying.baz` and every villager
/// on their knees plays it; draw nothing and the village animates exactly as it
/// did before. No table to keep in step with the bench, and no clip that has to
/// be registered somewhere before it does anything.
pub fn clips_follow_the_day(
    mut commands: Commands,
    bodies: Query<
        (Entity, &crate::villager::Activity, Option<&Playing>),
        (With<crate::villager::Villager>, With<CreatureRig>),
    >,
) {
    for (entity, doing, playing) in &bodies {
        let wanted = match doing {
            crate::villager::Activity::Idle => "idle",
            crate::villager::Activity::Wandering => "wandering",
            crate::villager::Activity::SeekingFood(_) => "seeking-food",
            crate::villager::Activity::Eating(_) => "eating",
            crate::villager::Activity::Working => "working",
            crate::villager::Activity::VisitingStore => "visiting-store",
            crate::villager::Activity::TendingFire => "tending-fire",
            crate::villager::Activity::Hauling => "hauling",
            crate::villager::Activity::Mourning => "mourning",
            crate::villager::Activity::Chatting => "chatting",
            crate::villager::Activity::Sheltering => "sheltering",
            crate::villager::Activity::Bearing => "bearing",
            crate::villager::Activity::Praying => "praying",
            crate::villager::Activity::Sleeping => "sleeping",
        };
        match (clip_called(wanted), playing) {
            // Already playing the right one: leave it be, or it restarts every
            // frame and never reaches its second key.
            (Some(_), Some(playing)) if playing.is(wanted) => {}
            (Some(clip), _) => {
                commands.entity(entity).insert(Playing::of(clip));
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<Playing>();
            }
            (None, None) => {}
        }
    }
}
