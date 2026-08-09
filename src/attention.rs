//! What the god is actually looking at.
//!
//! A village of thirty says a great deal and almost none of it is ever read. A
//! bubble over someone off the edge of the frame is built and hidden in the
//! same breath. A bubble over someone two hundred units out is a full-sized
//! box of text hanging above a speck four pixels tall. Both cost what one the
//! player actually reads costs — and if the words were composed rather than
//! written, they cost a second and a half of a language model's time on top.
//!
//! So the village's talk is aimed at the eye watching it. This module owns the
//! single rule for where that eye is, and everything that puts words in a
//! villager's mouth asks here first.
//!
//! Two things it deliberately does NOT gate: the spread of belief, and the
//! knowledge that changes hands when two people meet. Those are simulation.
//! They happen whether or not anyone is looking, and a village whose faith
//! only moved on camera would be a different and much smaller game. Only the
//! WORDS are aimed — never what the words are about.

use bevy::prelude::*;

/// How far a villager can be from the eye and still be worth any words.
///
/// Read off the lens rather than guessed. The projection spans a person's 1.8
/// units across roughly `2500 / distance` pixels of a nine-hundred-pixel
/// window, so at this range they stand about eleven pixels tall — already
/// shorter than one line of the text in the bubble above them. Past it there
/// is nothing for the words to belong to.
const REGARD_RANGE: f32 = 220.0;

/// Within this, a villager is a person rather than a figure: about forty
/// pixels tall, close to the distance the opening descent settles at, and the
/// only range at which it is worth asking the teller to compose anything.
const CLOSE_RANGE: f32 = 110.0;

/// Past this multiplier the world is being hurried, and composing a line takes
/// longer than the moment it was for.
///
/// A composed line is around a second and a half of the model's time. At two
/// and a half times speed that is most of a conversation; at eight it is a
/// day. The words would land on a square that has moved on, so a hurried world
/// keeps to its written lines, which are instant.
const HURRIED: f32 = 2.5;

/// How far past the frame's edge still counts as seen.
///
/// A bubble sits above its speaker's head and hangs out to either side, so
/// someone a little outside the frame still shows up inside it.
const EDGE_SLACK: f32 = 1.2;

/// How much of the god's attention something is receiving.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Regard {
    /// Being looked at, close enough to read. Worth composing words for.
    Close,
    /// In frame, but small. A written line is all anyone could take in.
    Distant,
    /// Not being looked at. Nothing here needs words at all.
    Unseen,
}

impl Regard {
    /// Whether anything should be said aloud here. The one question the
    /// presentation asks; the simulation underneath never asks anything.
    /// (`worth_composing` lived here once — the retired teller's economy,
    /// which composed only for closely watched heads. The corpus picks for
    /// nothing, so the question died with the model.)
    pub fn worth_saying(self) -> bool {
        self != Regard::Unseen
    }
}

/// Where the god's eye is this frame, and how hard the world is being hurried.
///
/// Holds a matrix rather than a camera so that any system can ask about any
/// point without taking a camera query of its own — several of the systems
/// that need this are already at Bevy's parameter ceiling.
#[derive(Resource)]
pub struct Attention {
    /// The eye itself, for judging how large a thing appears.
    eye: Vec3,
    /// World to clip space, for asking whether a point is in frame.
    clip_from_world: Mat4,
    /// The speed dial, with a pause counting as no haste at all: stopped and
    /// zoomed in on one person is exactly when the best words are wanted.
    haste: f32,
    /// Whether a camera was found. Before one exists, nothing is being looked
    /// at — but tests and headless runs have no camera either, and there the
    /// answer must be the generous one or every speech test goes quiet.
    watching: bool,
}

impl Default for Attention {
    fn default() -> Self {
        Attention {
            eye: Vec3::ZERO,
            clip_from_world: Mat4::IDENTITY,
            haste: 1.0,
            watching: false,
        }
    }
}

impl Attention {
    /// How much attention a point in the world is receiving. `at` is a
    /// FLAT sim position — the bend happens here, once.
    pub fn on(&self, at: Vec3) -> Regard {
        // With no camera there is no frame to be outside of. Headless runs and
        // unit tests land here, and they must behave as they always have.
        if !self.watching {
            return Regard::Close;
        }
        // The eye and the clip matrix live in the BENT render world; the
        // callers all hold flat sim positions. Bend the point once, here —
        // or every "is this on screen" answer is wrong away from the
        // origin. The bend's EIGHTH bite: `worth_saying` read every far
        // speaker as off-frame, and whole villages talked with no words
        // over any head.
        let (seat, _) = crate::globe::bend_frame(at);
        judge(self.eye.distance(seat), self.in_frame(seat), self.haste)
    }

    /// An eye that is turned away from everything.
    ///
    /// For the tests that have to prove the village goes on living off camera:
    /// a zero matrix sends every point behind the lens, so nothing anywhere is
    /// in frame. Only reachable from a test build, because a blind [`Attention`]
    /// is never a state the running game should be able to enter.
    #[cfg(test)]
    pub fn blind() -> Attention {
        Attention {
            eye: Vec3::ZERO,
            clip_from_world: Mat4::ZERO,
            haste: 1.0,
            watching: true,
        }
    }

    /// Whether a point falls inside the frame, generously.
    fn in_frame(&self, at: Vec3) -> bool {
        let clip = self.clip_from_world * at.extend(1.0);
        // Behind the eye. Dividing through would fold it back into frame.
        if clip.w <= 0.0 {
            return false;
        }
        let x = clip.x / clip.w;
        let y = clip.y / clip.w;
        x.abs() <= EDGE_SLACK && y.abs() <= EDGE_SLACK
    }
}

/// How much attention a point is receiving, for a caller that may not have the
/// resource at all.
///
/// Every consumer of this goes through here rather than reaching for the
/// resource directly, because a good half of them run in worlds that have no
/// camera, no window and no [`AttentionPlugin`] — the unit tests, which
/// assemble three villagers and a clock. There the answer must be the generous
/// one, or the suite that proves a village talks would quietly prove it
/// silent instead.
pub fn regard(attention: Option<&Attention>, at: Vec3) -> Regard {
    attention.map_or(Regard::Close, |attention| attention.on(at))
}

/// The rule itself, apart from any camera.
///
/// Kept a free function so the thing that actually decides whether a village
/// speaks can be tested without a window, a projection or a render pass.
fn judge(distance: f32, in_frame: bool, haste: f32) -> Regard {
    if !in_frame || distance > REGARD_RANGE {
        return Regard::Unseen;
    }
    if distance <= CLOSE_RANGE && haste <= HURRIED {
        return Regard::Close;
    }
    Regard::Distant
}

/// Reads the camera into [`Attention`], once a frame, before anything asks.
///
/// The camera's `GlobalTransform` is a frame stale here, which is what every
/// other consumer of it in the game already lives with: at the speed a camera
/// moves it is invisible, and the alternative is ordering this after transform
/// propagation and therefore after most of the systems that want to read it.
fn watch(
    mut attention: ResMut<Attention>,
    speed: Option<Res<crate::speed::SimSpeed>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
) {
    let Some((camera, at)) = cameras
        .iter()
        .find(|(camera, _)| camera.order == 0 && camera.is_active)
    else {
        attention.watching = false;
        return;
    };
    attention.eye = at.translation();
    attention.clip_from_world = camera.clip_from_view() * at.to_matrix().inverse();
    attention.haste = match speed.as_deref() {
        Some(speed) if speed.paused => 0.0,
        Some(speed) => speed.speed,
        None => 1.0,
    };
    attention.watching = true;
}

/// Reports what the eye is and is not seeing.
///
/// `DIVUS_FACTUS_ATTENTION=1`. Two things it is for. The first is tuning: the
/// two ranges above are the difference between a talkative village and a
/// silent one, and they want setting by feel rather than by arithmetic. The
/// second is proving the frustum test discriminates at all — a matrix built
/// wrong here would quietly report the whole world in frame, the gate would
/// pass everything, and nothing would look broken.
fn report(
    time: Res<Time<Real>>,
    mut since: Local<f32>,
    attention: Res<Attention>,
    people: Query<&Transform, With<crate::villager::Villager>>,
) {
    *since += time.delta_secs();
    if *since < 3.0 {
        return;
    }
    *since = 0.0;
    let mut tally = [0usize; 3];
    let mut nearest = f32::INFINITY;
    for at in &people {
        tally[match attention.on(at.translation) {
            Regard::Close => 0,
            Regard::Distant => 1,
            Regard::Unseen => 2,
        }] += 1;
        nearest = nearest.min(attention.eye.distance(at.translation));
    }
    // The eye and the nearest person, because the counts alone cannot tell a
    // village that has wandered off from an eye that is looking the wrong way.
    info!(
        "attention: {} close, {} distant, {} unseen — eye at {:.0},{:.0},{:.0}, nearest {:.0}",
        tally[0], tally[1], tally[2], attention.eye.x, attention.eye.y, attention.eye.z, nearest,
    );
}

pub struct AttentionPlugin;

impl Plugin for AttentionPlugin {
    fn build(&self, app: &mut App) {
        // PreUpdate, so the transforms it reads are the settled ones from last
        // frame's propagation and every Update system sees the same answer.
        app.init_resource::<Attention>()
            .add_systems(PreUpdate, watch);
        if std::env::var("DIVUS_FACTUS_ATTENTION").is_ok_and(|dial| dial == "1") {
            app.add_systems(Update, report);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_off_screen_is_worth_words() {
        // The cheapest saving there is: a bubble behind the camera or past the
        // edge of the frame is built and hidden in the same frame.
        assert_eq!(judge(5.0, false, 1.0), Regard::Unseen);
        assert_eq!(judge(0.0, false, 0.0), Regard::Unseen);
    }

    #[test]
    fn a_speck_across_the_valley_is_worth_words_from_nobody() {
        assert_eq!(judge(REGARD_RANGE + 1.0, true, 1.0), Regard::Unseen);
        assert_eq!(judge(1400.0, true, 1.0), Regard::Unseen);
    }

    #[test]
    fn someone_being_watched_up_close_is_regarded_closely() {
        assert_eq!(judge(20.0, true, 1.0), Regard::Close);
        assert_eq!(judge(CLOSE_RANGE, true, 1.0), Regard::Close);
    }

    #[test]
    fn the_middle_distance_still_speaks() {
        let far = judge(CLOSE_RANGE + 1.0, true, 1.0);
        assert_eq!(far, Regard::Distant);
        assert!(far.worth_saying());
    }

    #[test]
    fn a_hurried_world_keeps_to_its_written_lines() {
        // Close and in frame, but the composed line would land a day late.
        assert_eq!(judge(10.0, true, HURRIED), Regard::Close);
        assert_eq!(judge(10.0, true, HURRIED + 0.1), Regard::Distant);
        assert_eq!(judge(10.0, true, 8.0), Regard::Distant);
        // A pause is not haste: stopped on one person is when the best words
        // are wanted most.
        assert_eq!(judge(10.0, true, 0.0), Regard::Close);
    }

    #[test]
    fn a_world_with_no_camera_is_a_world_that_still_talks() {
        // Every headless soak and every speech test runs without a camera. If
        // the absence of one read as "unseen", the whole village would fall
        // silent in exactly the runs used to verify that it does not.
        let attention = Attention::default();
        assert_eq!(attention.on(Vec3::new(9000.0, 0.0, 9000.0)), Regard::Close);
    }

    #[test]
    fn the_frame_test_agrees_with_a_real_projection() {
        // Built the way the plugin builds it, so a change to either the lens
        // or the matrix order is caught here rather than in a silent village.
        let eye = Vec3::new(0.0, 40.0, 60.0);
        let view =
            GlobalTransform::from(Transform::from_translation(eye).looking_at(Vec3::ZERO, Vec3::Y));
        let projection = Mat4::perspective_rh(0.62, 16.0 / 9.0, 0.5, 3000.0);
        let attention = Attention {
            eye,
            clip_from_world: projection * view.to_matrix().inverse(),
            haste: 1.0,
            watching: true,
        };

        // What the camera is pointed at.
        assert_eq!(attention.on(Vec3::ZERO), Regard::Close);
        // Directly behind it.
        assert_eq!(
            attention.on(eye + Vec3::new(0.0, 0.0, 40.0)),
            Regard::Unseen
        );
        // Far out to the side, well outside a 0.62-radian lens.
        assert_eq!(attention.on(Vec3::new(400.0, 0.0, 0.0)), Regard::Unseen);
        // In front, in frame, but across the valley.
        assert_eq!(attention.on(Vec3::new(0.0, 0.0, -300.0)), Regard::Unseen);
    }
}
