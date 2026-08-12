//! The god camera: an orbiting, panning, zooming view anchored to a point on the ground.
//!
//! The camera never stores its own transform as truth. It stores a focus point plus
//! yaw, pitch and distance, smooths those toward targets, and derives the transform.
//! That keeps orbiting stable — the view rotates around the thing you are looking at
//! rather than drifting the way an incrementally-rotated transform does.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::f32::consts::{FRAC_PI_2, TAU};

use crate::terrain::{Terrain, WATER_LEVEL};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FollowTarget>()
            .add_systems(Startup, spawn_camera.in_set(CameraStartupSet))
            .add_systems(
                Update,
                (
                    // The player's hands stay off the wheel until the game
                    // is actually theirs: the title drift and the opening
                    // descent both own the rig outright. The CHOOSING is
                    // theirs too, though - it is the first thing they do,
                    // and a god who cannot turn the camera cannot pick
                    // anywhere to put a village.
                    read_camera_input.run_if(
                        crate::world_is_afoot
                            .and_then(|dive: Option<Res<CameraDive>>| dive.is_none()),
                    ),
                    apply_follow,
                    fly_the_dive.run_if(crate::world_is_afoot),
                    follow_ground,
                    apply_camera_smoothing,
                    // After everything that could have moved the focus, and
                    // before anything reads the camera's pose from it.
                    carry_the_frame,
                    write_camera_transform,
                    aim_the_near_plane,
                )
                    .chain()
                    .in_set(CameraSet),
            );
    }
}

/// A scripted descent: the camera flies itself from the title vantage down to
/// the village while the player's input is held. Inserted by the title
/// screen's Begin button; removes itself on arrival.
#[derive(Resource)]
pub struct CameraDive {
    /// Progress through the flight, 0 to 1.
    t: f32,
    /// Departure pose — focus, pitch, distance — captured on the first frame
    /// of flight, so the descent leaves from wherever the title drift
    /// happened to be looking.
    from: Option<(Vec3, f32, f32)>,
    to_focus: Vec3,
}

impl CameraDive {
    pub fn descend_to(focus: Vec3) -> Self {
        CameraDive {
            t: 0.0,
            from: None,
            to_focus: focus,
        }
    }
}

/// Seconds the descent takes: long enough to read as flight, short enough
/// that nobody reaches for the mouse wondering whether they have control yet.
const DIVE_SECONDS: f32 = 3.4;
/// Where the descent lands, matching the framing the game has always opened
/// with — see `point_camera_at_settlement`.
const DIVE_PITCH: f32 = 0.85;
const DIVE_DISTANCE: f32 = 80.0;

/// Flies the [`CameraDive`], writing both current and target values so the
/// smoothing pass has nothing left to argue about.
fn fly_the_dive(
    mut commands: Commands,
    time: Res<Time<Real>>,
    dive: Option<ResMut<CameraDive>>,
    mut rigs: Query<&mut CameraRig>,
) {
    let (Some(mut dive), Ok(mut rig)) = (dive, rigs.single_mut()) else {
        return;
    };
    let from = *dive
        .from
        .get_or_insert((rig.focus, rig.pitch, rig.distance));
    dive.t = (dive.t + time.delta_secs() / DIVE_SECONDS).min(1.0);
    // Smoothstep: the flight leaves the drift gently and settles onto the
    // village without a felt seam at either end.
    let s = dive.t * dive.t * (3.0 - 2.0 * dive.t);
    rig.focus = from.0.lerp(dive.to_focus, s);
    rig.pitch = from.1 + (DIVE_PITCH - from.1) * s;
    // The dive lands at the survey height - or wherever the capture
    // dial parked the camera, since a soak cannot reach the mouse wheel
    // and the dive used to stomp the dial three and a half seconds after
    // it was read.
    let landing = std::env::var("DIVUS_FACTUS_DISTANCE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(DIVE_DISTANCE);
    rig.distance = from.2 + (landing - from.2) * s;
    rig.target_focus = rig.focus;
    rig.target_pitch = rig.pitch;
    rig.target_distance = rig.distance;
    rig.zoom_anchor = None;
    // The title's lens shift unwinds as the god descends, so the world swings
    // back square to the frame by the time the village arrives. Cleared
    // outright at the end, since a decay never quite reaches nothing.
    rig.aim_offset *= 1.0 - s;
    if dive.t >= 1.0 {
        rig.aim_offset = Vec2::ZERO;
        commands.remove_resource::<CameraDive>();
    }
}

/// How the camera rides along when following someone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FollowStyle {
    /// The god's view, pinned to them: orbit and zoom stay free.
    #[default]
    Overhead,
    /// Out of their eyes. The rig collapses onto the head and the orbit
    /// becomes the neck - looking about rather than circling them.
    Eyes,
}

/// Whom the camera is following, if anyone.
///
/// Set by right-clicking a creature under the hand. Only right-clicks change
/// it: the next clean click lets go, and clicking a different creature
/// switches to them. Two things release a follow: following someone else,
/// and seizing the land — the left-button grab is the god taking the
/// camera back, and it must always win over a pin.
#[derive(Resource, Default)]
pub struct FollowTarget {
    pub entity: Option<Entity>,
    pub style: FollowStyle,
    /// A one-shot dive: the distance to close to as the follow begins.
    /// Taken (and cleared) on the first frame, so the god's own zoom is
    /// never fought afterwards. The prayer cards use it — press to fly
    /// lands at answering height instead of wherever the camera last was.
    pub close_to: Option<f32>,
}

/// How high the eyes sit above the feet, for a body built from `genome`.
///
/// The head is a box centred a little way down into the torso; the eyes are
/// in its upper third, which is also where the face features are painted on.
/// Kept here beside the camera that needs it rather than in the body module,
/// since nothing else asks the question.
pub fn eye_height(genome: &crate::creature::genome::CreatureGenome) -> f32 {
    let p = &genome.proportions;
    let h = genome.height();
    let head = crate::creature::body::biped_head_size(genome);
    let centre = (p.leg_length + p.torso_length + p.neck_length) * h - head * 0.12;
    centre + head * 0.28
}

/// How far forward of the head's centre the eyes sit, for a body built from
/// `genome`.
///
/// Eyes are on the FACE. Put the camera at the middle of the skull instead —
/// which is where measuring only the height puts it — and looking down looks
/// straight down the inside of the neck. From the face, looking down finds
/// your own chest and boots, which is the entire reason the body is left
/// standing there to be looked at.
///
/// Just inside the front of the head box rather than flush with it, so a
/// turn of the head never swings the near plane out through the cheek.
pub fn eye_forward(genome: &crate::creature::genome::CreatureGenome) -> f32 {
    crate::creature::body::biped_head_size(genome) * 0.42
}

/// Rides the focus (and, at the shoulder, the whole rig) along with whoever
/// is being followed.
fn apply_follow(
    mut follow: ResMut<FollowTarget>,
    // The FLAT transform, and this line is why the Avatar miracle broke on
    // the round world. The rig's focus lives in sim coordinates; the global
    // is the BENT seat, and feeding its y into the focus put the first-person
    // eye at radius-minus-eighty - inside the planet, sky below the horizon,
    // the cloud deck overhead as a ceiling. The probe read seat_alt -82.7
    // with the frame at 0.00 degrees of error: perfect orientation, seated
    // underground. The bend's SIXTH bite, all six the same species.
    targets: Query<&Transform>,
    bodies: Query<&crate::creature::genome::CreatureGenome>,
    mut rigs: Query<&mut CameraRig>,
) {
    // The rig is fetched before anything else, and `in_a_body` is answered on
    // EVERY path out of here — including the paths where there is nobody to
    // follow. Leaving it stale is what let a released follow strand the camera
    // under the rules of a head it was no longer inside.
    let Ok(mut rig) = rigs.single_mut() else {
        return;
    };
    let Some(entity) = follow.entity else {
        rig.in_a_body = false;
        return;
    };
    let Ok(target) = targets.get(entity) else {
        // Whoever it was is gone from the world.
        follow.entity = None;
        rig.in_a_body = false;
        return;
    };
    rig.in_a_body = follow.style == FollowStyle::Eyes;

    let at = target.translation;
    rig.target_focus.x = at.x;
    rig.target_focus.z = at.z;
    // A followed zoom anchor fights the pin; the pin wins.
    rig.zoom_anchor = None;
    // The one-shot dive, spent the moment it is read.
    if let Some(height) = follow.close_to.take() {
        rig.target_distance = height;
    }

    if follow.style == FollowStyle::Eyes {
        // Behind the eyes: the focus rises to head height and the orbit
        // closes to nothing, so the rig's yaw and pitch become where
        // this person is looking. `MIN_DISTANCE` is twelve metres and
        // would hold the camera out in front of their own face, so the
        // eye distance is written straight past it, and the pitch is let
        // below the overhead floor - a person can look at their boots.
        // TARGETS only: the rig's own smoothing then flies the god down
        // into the body over about a second rather than cutting to it.
        // Writing the immediate values as well - which is what this did
        // first - snapped the camera into their skull in one frame.
        // Read off the body itself, every frame. Asking whoever set this
        // style to also hand over the eye height was the wrong shape: the
        // right-click follow sets the style too and knew nothing about it,
        // so it fell back to a floor value and put the view down around the
        // ankles. The body is right here and can be measured.
        let eyes = bodies.get(entity).map_or(1.3, eye_height);
        rig.target_focus.y = at.y + eyes;
        // And forward, onto the face. At the centre of the head, looking
        // down looked straight down the inside of the neck. The offset
        // follows the LOOK direction rather than the body's own facing, so
        // turning to look at something takes the eyes round with it the way
        // a head does — and a body standing still can still be looked down
        // at from its own face.
        let face = rig.ground_forward() * bodies.get(entity).map_or(0.19, eye_forward);
        rig.target_focus.x = at.x + face.x;
        rig.target_focus.z = at.z + face.z;
        rig.target_distance = 0.0;
        // The pitch is deliberately NOT touched here. Clamping it every
        // frame — which is what this did first — capped the downward look at
        // whatever the clamp was, so you could never find your own feet. It
        // is levelled once, on possession, and after that it is yours.
    }
}

/// Camera systems run as a unit so that anything needing a settled camera — the
/// Hand's raycast, for one — can simply order itself after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSet;

/// Startup set that spawns the camera. The render pipeline orders itself after
/// this so it has a camera to attach the offscreen target to.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraStartupSet;

/// Sets the near plane from how high the god is, every frame.
///
/// This is a DEPTH PRECISION instrument, not a clipping one. The far plane
/// had to grow to seventy thousand so the whole planet fits in the frame,
/// and with a near plane of half a unit against that, the depth buffer can
/// only separate surfaces about a tenth of a unit apart at a thousand units
/// away - which is precisely how high the fog veil's lowest sheet floats
/// over the ground. The whole landscape came out streaked in grey where the
/// two fought for the same pixels.
///
/// Nothing is ever nearer the camera than the ground it is looking at, so
/// the near plane can ride the zoom: a twelfth of the orbit distance, which
/// at village height is some twenty-five units and buys two orders of
/// magnitude of precision. Behind a mortal's eyes it drops to four
/// centimetres, because there the god's own chest is a hand away - and that
/// case owns the plane outright.
fn aim_the_near_plane(
    rigs: Query<&CameraRig>,
    mut lenses: Query<&mut Projection, With<GodCamera>>,
) {
    let Ok(rig) = rigs.single() else {
        return;
    };
    let Ok(mut lens) = lenses.single_mut() else {
        return;
    };
    let Projection::Perspective(lens) = &mut *lens else {
        return;
    };
    let wanted = if rig.in_a_body {
        CLOSE_NEAR
    } else {
        (rig.distance / 12.0).clamp(WIDE_NEAR, 90.0)
    };
    if (lens.near - wanted).abs() > 1.0e-4 {
        lens.near = wanted;
    }
}

/// The near plane for the god's ordinary view of the world.
///
/// Half a metre: overhead the camera is never within twelve of anything, so
/// the plane can sit far out and buy depth precision across a three-kilometre
/// far plane.
pub const WIDE_NEAR: f32 = 0.5;

/// The near plane while the god is wearing a body.
///
/// Their own chest is a hand's width below the eyes and their hands swing
/// closer than that, so the plane has to come in to a few centimetres or the
/// body is clipped away and looking down shows bare ground. The cost is
/// depth precision out at the horizon, which is a fair trade for five
/// minutes and is put back the moment the body is given up.
pub const CLOSE_NEAR: f32 = 0.04;

/// Below this camera distance the rig is taken to be behind somebody's eyes
/// rather than orbiting them, and several rules change: the pitch range
/// opens both ways, the transform is pointed by yaw and pitch rather than at
/// the focus, and the ground-clearance lift is skipped. Well under the
/// twelve metres ordinary play is clamped to, so only Avatar reaches it.
/// The god camera's vertical field of view, in radians.
///
/// Named because the title screen has to aim the planet at a place on the
/// SCREEN, and turning a fraction of the frame into an angle needs the lens
/// that frame was drawn with.
pub const FIELD_OF_VIEW: f32 = 0.62;

pub const FIRST_PERSON: f32 = 0.5;

/// Mouse travel in a single frame, in pixels, past which free look treats the
/// motion as a pointer warp rather than a movement of somebody's hand. Locking
/// the cursor reports the jump to the window centre as relative motion, and at
/// free-look sensitivity that is most of a full turn.
const LOOK_JUMP: f32 = 160.0;

/// How far up and down a worn body may look, in radians — about 77° each
/// way, which finds both the sky and your own boots without letting the
/// neck fold over backwards.
const RIDDEN_PITCH: f32 = 1.35;

/// [`CameraRig::zoom_fraction`] for a bare distance, so callers holding an
/// altitude rather than a rig can ask the same question - the hand pins its
/// screen size to what it would be at a chosen height.
pub(crate) fn zoom_fraction_of(distance: f32) -> f32 {
    ((distance - MIN_DISTANCE) / (MAX_DISTANCE - MIN_DISTANCE)).clamp(0.0, 1.0)
}

const MIN_PITCH: f32 = 0.20;
pub const MAX_PITCH: f32 = FRAC_PI_2 - 0.06;
const MIN_DISTANCE: f32 = 12.0;
const MAX_DISTANCE: f32 = 1400.0;

/// World units of drag-pan per pixel of mouse travel, per unit of camera distance.
///
/// Derived from the projection: the view spans roughly `0.64 * distance` world units
/// vertically, over about 900 logical pixels of window.
const DRAG_PAN_SCALE: f32 = 0.0007;

/// Marks the single god camera.
#[derive(Component)]
pub struct GodCamera;

/// Camera state, split into current and target values so movement can be smoothed.
#[derive(Component)]
pub struct CameraRig {
    /// Point on the ground the camera orbits.
    pub focus: Vec3,
    pub target_focus: Vec3,
    pub yaw: f32,
    pub target_yaw: f32,
    /// Turns the camera off its own aim, in radians of yaw and pitch, AFTER the
    /// look-at is built.
    ///
    /// A lens shift, in effect, and the title screen's reason for existing: the
    /// menu wants the right of the frame and the planet wants the left, and the
    /// rig can only look AT things. Turning the camera a little to the right
    /// slides the world left without moving the focus an inch — and because the
    /// cursor's ray is built from the same pose (`globe::bent_camera_pose`), the
    /// pointer goes on landing where it is pointed.
    pub aim_offset: Vec2,
    pub pitch: f32,
    pub target_pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
    /// Whether the camera is presently behind somebody's eyes.
    ///
    /// Written by [`apply_follow`] every frame, and the authority on the
    /// question. It was inferred from `distance` being near nought before,
    /// which is a CONSEQUENCE of being in a body rather than the fact of it —
    /// and the difference bit hard: releasing the follow (a stray right-click
    /// does it) left the distance at nought with nothing pinning the camera
    /// to anybody, so the rules that only make sense inside a head stayed on.
    /// The ground-following stayed off and the keys still panned, which flew
    /// the god about at a fixed height and straight through hillsides.
    pub in_a_body: bool,
    /// How fast panning moves the focus, in world units per second at mid zoom.
    pub pan_speed: f32,
    pub orbit_sensitivity: f32,
    pub zoom_sensitivity: f32,
    /// Higher converges faster. Tuned by feel; around 12 reads as responsive but
    /// still weighted, which suits something the size of a god.
    pub smoothing: f32,
    /// Which way the camera is over the planet, and which way is up on the
    /// screen: the tangent frame at the focus, in world space.
    ///
    /// CARRIED, never recomputed from the focus's longitude and latitude, and
    /// that is the whole point of it. A ball has no preferred axis. The world
    /// does not spin, so it does not need poles - but the frame used to be
    /// rebuilt every frame from `bend_frame`, whose east is the derivative
    /// along longitude, and THAT has poles whatever the planet thinks. Near
    /// one, east-west arc length collapses and the frame twists faster and
    /// faster for the same movement of the ground; at one, longitude is
    /// undefined and the derivative is a difference of two nearly identical
    /// directions, so it fell back to a hardcoded axis; and crossing one,
    /// longitude jumps by half a turn and east flips end for end.
    ///
    /// Instead the frame is turned by exactly as much as the ground under it
    /// turned, and no more. Over a drag or a pan that is parallel transport
    /// along the great circle: no twist is introduced, so there is nothing to
    /// blow up anywhere, and the planet can be rolled in any direction for as
    /// long as you like. See `carry_the_frame`.
    pub facing: Quat,
    /// Whether the world is being held by the mouse this frame.
    ///
    /// Direct manipulation and smoothing cannot both be in the same loop. The
    /// grab reads the ground under the cursor from where the camera ACTUALLY
    /// is, and sets the target so that ground comes back under the hand - but
    /// a smoothed camera is never where its target says, so the next frame
    /// reads a different piece of ground and corrects again, and the camera's
    /// own catch-up drives the correction. That is the wobble: the rig chasing
    /// a target computed from the fact that it has not arrived yet.
    ///
    /// While the world is held the focus is therefore snapped, not smoothed.
    /// Nothing is lost - the mouse is the smoothing.
    pub held_by_hand: bool,
    /// Ground point the current zoom is closing in on.
    ///
    /// Held for the whole smoothed zoom rather than applied once on the scroll
    /// event. Distance and focus are smoothed independently, so a one-shot
    /// adjustment drifts off the target over the frames that follow — the point
    /// under the cursor has to be re-pinned every frame until the zoom settles.
    pub zoom_anchor: Option<Vec3>,
}

impl Default for CameraRig {
    fn default() -> Self {
        CameraRig {
            focus: Vec3::ZERO,
            target_focus: Vec3::ZERO,
            yaw: 0.6,
            target_yaw: 0.6,
            aim_offset: Vec2::ZERO,
            pitch: 0.85,
            target_pitch: 0.85,
            distance: 62.0,
            target_distance: 62.0,
            in_a_body: false,
            pan_speed: 28.0,
            orbit_sensitivity: 0.005,
            zoom_sensitivity: 0.12,
            smoothing: 12.0,
            // Seated from the ground it starts on. After this first breath it
            // is only ever carried.
            facing: crate::globe::bend_frame(Vec3::ZERO).1,
            held_by_hand: false,
            zoom_anchor: None,
        }
    }
}

impl CameraRig {
    /// Unit vector the camera looks along, from its eye toward the focus.
    /// Unit direction the camera is looking.
    ///
    /// Derived from yaw and pitch alone rather than from the offset between
    /// eye and focus, because that offset is a ZERO VECTOR when the god is
    /// wearing a body: the eye sits exactly on the focus, and normalising
    /// the nothing between them gave NaN, which reached the transform and
    /// pointed the view nowhere at all.
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        -Vec3::new(sy * cp, sp, cy * cp)
    }

    /// Offset from focus to eye.
    pub(crate) fn eye_offset(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(sy * cp, sp, cy * cp) * self.distance
    }

    /// World-space eye position.
    pub fn eye(&self) -> Vec3 {
        self.focus + self.eye_offset()
    }

    /// Ground-plane forward direction, for panning relative to the view.
    pub(crate) fn ground_forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(-sy, 0.0, -cy)
    }

    /// Ground-plane right direction.
    pub(crate) fn ground_right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(cy, 0.0, -sy)
    }

    /// Zoom as a 0-to-1 value, 0 being closest.
    pub fn zoom_fraction(&self) -> f32 {
        zoom_fraction_of(self.distance)
    }
}

/// Converts a frame's scroll delta into wheel notches.
///
/// A mouse wheel reports discrete lines; a trackpad reports pixels, roughly a
/// hundred times larger for the same gesture. Feeding the raw delta into the zoom
/// meant every trackpad scroll slammed straight into the clamp and the camera
/// jumped its maximum step regardless of how gently it was nudged.
pub(crate) fn normalised_scroll(delta: f32, unit: MouseScrollUnit) -> f32 {
    match unit {
        MouseScrollUnit::Line => delta,
        MouseScrollUnit::Pixel => delta / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
    }
}

/// New focus point after zooming toward `target` by `ratio`.
///
/// `ratio` is the new camera distance over the old one, so zooming in halves it and
/// the focus travels half the way to the cursor. That is what keeps the point under
/// the pointer fixed on screen while the view closes in on it.
/// Only the ground plane is adjusted. Height is owned by `follow_ground`, which
/// pins the focus to the terrain every frame — moving it here as well would just be
/// overwritten, and on a slope the two would fight.
fn zoom_focus(focus: Vec3, target: Vec3, ratio: f32) -> Vec3 {
    // Along the great circle between them, NOT across the coordinate plane.
    //
    // The flat `(x, z)` is longitude and latitude multiplied by the radius, so
    // lerping it is lerping ANGLES. Two failures came out of that. Near a pole
    // east-west arc length collapses, so a few units of actual ground is an
    // enormous step of `x`, and the focus shot round the world - the fast spin
    // on zoom. And across the date line the straight line between two adjacent
    // places is the whole circumference the wrong way.
    //
    // As places, `ratio` means what it always meant: one leaves the focus
    // alone, zero arrives at the anchor, and everything between is that
    // fraction of the way along the ground.
    let here = crate::place::Place::from_flat(focus);
    let anchor = crate::place::Place::from_flat(target);
    if anchor.apart(here) <= f32::EPSILON {
        return focus;
    }
    // A glide and not a step, because zooming OUT has to carry the focus
    // further from the anchor than it began - past the far end of the arc,
    // which anything that refuses to overshoot cannot do.
    canonical_near(focus, anchor.glide(here, ratio).direction())
}

/// Ground point under the mouse cursor, if the cursor is over the terrain.
///
/// Reads the camera's `GlobalTransform`, which is one frame stale — this system runs
/// before the transform is rewritten. At mouse-wheel timescales that is invisible,
/// and it avoids having to re-derive the view matrix here.
fn cursor_ground_point(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<GodCamera>>,
    terrain: Option<&Terrain>,
) -> Option<Vec3> {
    let terrain = terrain?;
    let window = windows.single().ok()?;
    let (camera, camera_transform) = cameras.single().ok()?;
    let cursor = window.cursor_position()?;
    let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
    crate::terrain::raycast(terrain, ray)
}

fn spawn_camera(mut commands: Commands) {
    let rig = CameraRig::default();
    commands.spawn((
        Name::new("God Camera"),
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            // A slightly long lens flattens the scene, which reinforces the
            // look-at-a-model feeling the tilt-shift pass will build on.
            fov: FIELD_OF_VIEW,
            near: WIDE_NEAR,
            // Far enough to see the far limb of the planet from the top of the
            // orbital zoom. Reverse-Z depth keeps the precision honest across
            // a range this wide.
            far: 70_000.0,
            ..default()
        }),
        Transform::from_translation(rig.eye()).looking_at(rig.focus, Vec3::Y),
        // The world's layers and the planet's, always both: the planet is
        // not a view to switch to, it is the far ground of the only view
        // there is.
        bevy::camera::visibility::RenderLayers::from_layers(&[0, crate::globe::GLOBE_LAYER]),
        GodCamera,
        rig,
    ));
}

fn read_camera_input(
    mut grabbed: Local<Option<Vec3>>,
    // Whether the drag in progress is the world's. A press that begins on a
    // panel belongs to the panel for its whole life, however far it wanders.
    mut ours: Local<bool>,
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse: Res<crate::keymap::MouseScheme>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    pointer: Res<crate::ui::PointerContext>,
    // Real time, for the same reason as the smoothing below: panning and
    // turning are looking, and looking survives a pause.
    time: Res<Time<Real>>,
    terrain: Option<Res<Terrain>>,
    mut follow: ResMut<FollowTarget>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<GodCamera>>,
    mut rigs: Query<&mut CameraRig>,
) {
    let Ok(mut rig) = rigs.single_mut() else {
        return;
    };

    use crate::keymap::Deed;

    // Panning. Speed scales with zoom so that crossing the screen takes about the
    // same time whether you are looking at one villager or the whole valley.
    //
    // None of it inside a body: those four keys are the LEGS then, and the
    // camera has no business also sliding itself sideways when they are
    // pressed. It got away with it only because the follow overwrote the
    // focus again a moment later, which is not a rule so much as a
    // coincidence — and the moment the follow was released the coincidence
    // ended and the keys flew the god through the landscape.
    let mut pan = Vec3::ZERO;
    if !rig.in_a_body {
        if keymap.pressed(&keys, Deed::PanNorth) || keys.pressed(KeyCode::ArrowUp) {
            pan += rig.ground_forward();
        }
        if keymap.pressed(&keys, Deed::PanSouth) || keys.pressed(KeyCode::ArrowDown) {
            pan -= rig.ground_forward();
        }
        if keymap.pressed(&keys, Deed::PanWest) || keys.pressed(KeyCode::ArrowLeft) {
            pan -= rig.ground_right();
        }
        if keymap.pressed(&keys, Deed::PanEast) || keys.pressed(KeyCode::ArrowRight) {
            pan += rig.ground_right();
        }
    }

    if pan != Vec3::ZERO {
        // Within the play zoom the pan speed rides the zoom fraction, as it
        // always has. Past it the speed grows with the DISTANCE itself, the
        // way a map's does, because the focus is now how the god travels the
        // planet: from twenty thousand up, a pan sweeps continents in
        // seconds, the streaming follows the focus wherever it lands, and
        // zooming back in arrives on whatever ground is under the view -
        // the far side of the world included. At the old capped speed that
        // journey took five minutes of held keys.
        let zoom_scale = 0.35
            + rig.zoom_fraction() * 1.6
            + (rig.target_distance - MAX_DISTANCE).max(0.0) * 0.012;
        let speed = rig.pan_speed * zoom_scale * time.delta_secs();
        // Walked along the ground, as a rotation in the rig's own carried
        // frame - not added to flat `(x, z)`, which is longitude and latitude
        // multiplied by the radius. Adding to those walks a COORDINATE, and a
        // coordinate's relation to the ground depends where you are standing:
        // near a pole the same step of `x` is a fraction of the ground it is
        // at the equator, so the same key held for the same second flew the
        // camera further and further the higher it got. The axis comes from
        // the frame rather than from a compass bearing, so there is no north
        // to be undefined at the top of the world.
        let heading = rig.facing * pan.normalize();
        let up_here = rig.facing * Vec3::Y;
        if let Some(axis) = heading.cross(up_here).try_normalize() {
            let stance = crate::globe::planet_stance();
            let was = stance * crate::terrain::direction_at(rig.target_focus.x, rig.target_focus.z);
            let turn = Quat::from_axis_angle(axis, speed / crate::terrain::PLANET_RADIUS);
            let now = stance.inverse() * (turn * was);
            rig.target_focus = canonical_near(rig.target_focus, now);
        }
        // And then folded back onto the sphere. Panning walks the flat
        // scaffold, and the scaffold runs off the end of the world: keep going
        // north and `z` grows past the pole for ever, which the terrain field
        // survives but an f32 does not. Sent through the sphere and back, the
        // coordinates come home canonical — and going over the top of the
        // world comes out on the far side, the way it should. It used to stop
        // dead a few degrees short of the pole, which zoomed out reads as a
        // wall in the middle of the sky.
        rig.target_focus = fold_onto_the_sphere(rig.target_focus);
    }

    // The mouse GRABS THE WORLD. Not a camera pan: the ground under the cursor
    // is seized on the press, and while the button is held the whole planet is
    // turned so that piece of ground stays under the hand - the gesture Black
    // and White taught, and the only one that makes sense once the world is a
    // ball. At village zoom the turn is microscopic and it feels exactly like
    // the drag-pan it replaces; from altitude the same pull spins continents.
    // One mechanism, every height.
    //
    // On the LEFT button and nothing else. Black and White's own table: left
    // is Grab Land, right is Action, middle is Rotate and Pitch. Middle used to
    // do this too, from before the gesture had a name, and it cannot any more -
    // it is the rotate now.
    //
    // Worth writing down because I got this wrong out loud: the Black and White
    // TWO manual has them the other way about, left for Action and right to
    // move, and I quoted it at Brett as though he had misremembered his own
    // game. He had not. The sequel flipped them, and this world follows the
    // first one.
    let taking_hold = buttons.just_pressed(mouse.land());
    let holding_on = buttons.pressed(mouse.land());
    if taking_hold {
        // A press that lands on a panel is the panel's. Judged ONCE, at the
        // press: a drag that starts on a roster and slides off it must not
        // suddenly seize the planet halfway through, and one that starts on
        // the world must go on turning it even when the cursor crosses a panel.
        *ours = !pointer.over_ui;
        // Grabbing the land takes the camera back. A follow re-pins the
        // focus every frame, so a pinned view ate every drag — Brett,
        // after pressing a prayer card: "it disables LMB dragging to
        // move." Seizing the world is the god saying "mine again"; only
        // the Avatar's behind-the-eyes ride is exempt, since it owns the
        // pointer outright.
        if *ours && follow.entity.is_some() && follow.style != FollowStyle::Eyes {
            follow.entity = None;
        }
        *grabbed = if *ours {
            cursor_sphere_direction(&windows, &cameras)
        } else {
            None
        };
    }
    if !holding_on {
        *ours = false;
        *grabbed = None;
    }
    // Told to the smoothing pass, which must not chase a target the grab is
    // deriving from where the camera already is. See `CameraRig::held_by_hand`.
    rig.held_by_hand = holding_on && *ours && grabbed.is_some();
    if holding_on && *ours {
        if let Some(held) = *grabbed {
            if let Some(now) = cursor_sphere_direction(&windows, &cameras) {
                rig.target_focus = turn_the_world(rig.target_focus, held, now);
            }
        } else {
            // The grab began on the sky: fall back to the old drag-pan so
            // the gesture still does something sensible.
            let delta = mouse_motion.delta;
            if delta != Vec2::ZERO {
                let scale = rig.distance * DRAG_PAN_SCALE;
                let right = rig.ground_right();
                let forward = rig.ground_forward();
                rig.target_focus -= right * delta.x * scale;
                rig.target_focus += forward * delta.y * scale;
            }
        }
    }

    // Orbiting, on the MIDDLE button — "Rotate; Pitch" in Black and White's
    // own table — or freely, with no button at all, when the
    // god is behind somebody's eyes. In a body there is no orbit to speak of
    // and nothing else for the mouse to do: the pointer is locked away and
    // the hand withdrawn, so the mouse simply IS the neck, the way it is in
    // every game that has ever put you inside a head.
    // Free look only while the window actually has the player's attention.
    // Grabbing the pointer WARPS it to the middle of the window, and the warp
    // comes back as one enormous relative motion — which, with no button
    // needed to turn the head any more, slammed the view round the instant a
    // body was taken. An unfocused window delivering stray motion did the
    // same. A soak with nobody at the keyboard was enough to see it: the
    // pitch wandered from 0.85 to -1.25 and back with no input at all.
    let attended = windows.single().is_ok_and(|window| window.focused);
    let looking_about = rig.in_a_body && attended;
    if buttons.pressed(MouseButton::Middle) || looking_about {
        let delta = mouse_motion.delta;
        // A jump this big in one frame is a warp or a focus change, never a
        // wrist.
        let warped = looking_about && delta.length_squared() > LOOK_JUMP * LOOK_JUMP;
        if delta != Vec2::ZERO && !warped {
            let sensitivity = rig.orbit_sensitivity;
            rig.target_yaw -= delta.x * sensitivity;
            // The overhead camera is never allowed to look level or upward,
            // because it orbits a point on the ground and would end up
            // underneath it. A person looking out of their own eyes has no
            // such trouble, and needs the range both ways: up at the sky
            // they pray to, and down far enough to find their own boots.
            let (floor, ceiling) = if rig.in_a_body {
                (-RIDDEN_PITCH, RIDDEN_PITCH)
            } else {
                (MIN_PITCH, MAX_PITCH)
            };
            rig.target_pitch = (rig.target_pitch + delta.y * sensitivity).clamp(floor, ceiling);
        }
    }

    // Keyboard orbit, so the camera is fully usable without a mouse button held.
    let mut keyboard_yaw = 0.0;
    if keymap.pressed(&keys, Deed::TurnLeft) {
        keyboard_yaw += 1.0;
    }
    if keymap.pressed(&keys, Deed::TurnRight) {
        keyboard_yaw -= 1.0;
    }
    if keyboard_yaw != 0.0 {
        rig.target_yaw += keyboard_yaw * 1.4 * time.delta_secs();
    }

    // Zooming. Multiplicative so each notch feels the same at every distance.
    // Over a window, the wheel belongs to the window — a scroll meant for a
    // roster must never yank the world's camera.
    let scroll = if pointer.over_ui {
        0.0
    } else {
        normalised_scroll(mouse_scroll.delta.y, mouse_scroll.unit)
    };
    if scroll != 0.0 {
        let factor = (1.0 - scroll * rig.zoom_sensitivity).clamp(0.5, 2.0);
        // The wheel runs past the play ceiling and on out to orbit: the globe
        // takes over at its curtain, and `zoom_fraction` — which sizes the
        // hand, the fog band and the pan speed — stays normalised to the play
        // range so none of them stretch on the way up.
        rig.target_distance =
            (rig.target_distance * factor).clamp(MIN_DISTANCE, crate::globe::CEILING);

        // Zoom toward whatever is under the cursor rather than the centre of the
        // screen, so zooming doubles as aiming: you can drop onto one villager
        // without panning there first.
        // The ground under the cursor when there IS loaded ground under it,
        // and the planet's own surface when there is not. Zoomed out at the
        // globe the terrain raycast has nothing to hit, so the anchor came
        // back None and the zoom fell to the centre of the screen - which is
        // precisely the altitude at which aiming the zoom matters most.
        let focus = rig.focus;
        rig.zoom_anchor =
            cursor_ground_point(&windows, &cameras, terrain.as_deref()).or_else(|| {
                cursor_sphere_direction(&windows, &cameras).map(|dir| canonical_near(focus, dir))
            });
    }

    // Leaving for the sky. Past the play ceiling the view steepens toward
    // straight down over a few thousand units of climb, so the whole planet
    // arrives framed the way a world in space reads best — looked AT, not
    // along. Gentle, and one-way per zoom: it never fights a pitch the god
    // steers on the way back down.
    if rig.target_distance > MAX_DISTANCE {
        let leaving = ((rig.target_distance - MAX_DISTANCE) / (MAX_DISTANCE * 3.0)).clamp(0.0, 1.0);
        rig.target_pitch = rig
            .target_pitch
            .max(MIN_PITCH + (MAX_PITCH - MIN_PITCH) * leaving);
    }
}

/// The focus, turned so the piece of ground the hand is holding comes back
/// under the cursor.
///
/// `held` is the direction the grab seized; `now` is where the cursor points at
/// this instant. Rotating the focus by the arc from `now` back to `held` puts
/// that ground under the hand again — the whole world-grab, in one rotation.
///
/// Pure, and separately tested, because the axes of this are easy to get wrong
/// and impossible to see wrong from a screenshot.
pub(crate) fn turn_the_world(focus: Vec3, held: Vec3, now: Vec3) -> Vec3 {
    // Built from an explicit axis and a CLAMPED angle, not from
    // `from_rotation_arc`, which documents that it picks an arbitrary axis
    // when its two directions are opposite - and near the planet's silhouette
    // they can be, because a hit that skids over the limb lands on the far
    // side of the world. An arbitrary axis through the middle is a half turn,
    // which is the planet flipping end over end under the hand.
    //
    // The angle is capped so no single reading can lurch the world either.
    // `held` is fixed for the whole gesture, so a capped step is not a step
    // lost: the next frame sees the same gap and takes another bite of it,
    // and a fast drag converges over two or three frames instead of snapping.
    let axis = now.cross(held);
    let Some(axis) = axis.try_normalize() else {
        // Parallel: already aligned, nothing to turn. Or antiparallel, where
        // there is no honest answer and leaving the world alone is the only
        // safe one.
        return focus;
    };
    let angle = axis
        .dot(now.cross(held))
        .atan2(now.dot(held))
        .min(MOST_TURN_IN_A_FRAME);
    let turn = Quat::from_axis_angle(axis, angle);
    let stance = crate::globe::planet_stance();
    let focus_dir = stance * crate::terrain::direction_at(focus.x, focus.z);
    let turned = stance.inverse() * (turn * focus_dir);
    canonical_near(focus, turned)
}

/// A flat position, sent out to the sphere and brought back as the canonical
/// coordinates for the place it names.
///
/// The scaffold is a plane wrapped round a ball, so it has more names than it
/// has places: `x` repeats every circumference, and every `x` at all meets at
/// each pole. Walking in a straight line eventually leaves the canonical range
/// entirely. This brings a position home without moving it an inch.
pub(crate) fn fold_onto_the_sphere(focus: Vec3) -> Vec3 {
    let direction = crate::terrain::direction_at(focus.x, focus.z);
    canonical_near(focus, direction)
}

/// The canonical `(x, z)` for a direction, with longitude unwrapped toward the
/// position it came from — so a step never teleports the coordinates across the
/// date line, even though the place either side of it is the same place.
fn canonical_near(was: Vec3, direction: Vec3) -> Vec3 {
    let (mut x, z) = crate::globe::ground_coordinates(direction);
    let round = crate::terrain::planet_circumference();
    while x - was.x > round * 0.5 {
        x -= round;
    }
    while was.x - x > round * 0.5 {
        x += round;
    }
    Vec3::new(x, was.y, z)
}

/// Where the cursor's ray meets the planet's sea-level sphere, as a unit
/// direction from the planet's centre — the handle the world-grab holds.
/// `None` when the cursor is off the ball entirely.
/// The most the world may turn from one reading, in radians.
const MOST_TURN_IN_A_FRAME: f32 = 0.35;

/// How far inside the silhouette a hit must land to be worth trusting, as a
/// fraction of the radius.
///
/// At the limb the sphere runs edgewise to the eye: the near and far
/// intersections meet, and the ground under the cursor sweeps arbitrarily fast
/// for an arbitrarily small movement of the mouse. Readings from there are not
/// slightly noisy, they are meaningless - and they are what sent the grab to
/// the far side of the world. Past this the cursor is treated as being off the
/// planet, and a grab in progress simply holds still until it comes back.
const STEADY_GROUND: f32 = 0.12;

fn cursor_sphere_direction(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<GodCamera>>,
) -> Option<Vec3> {
    let window = windows.single().ok()?;
    let (camera, camera_transform) = cameras.single().ok()?;
    let cursor = window.cursor_position()?;
    let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
    let centre = crate::globe::planet_centre();
    let radius = crate::terrain::PLANET_RADIUS + WATER_LEVEL;
    let to_centre = centre - ray.origin;
    let along = to_centre.dot(*ray.direction);
    let closest = ray.origin + *ray.direction * along - centre;
    let off_axis = closest.length_squared();
    if off_axis > radius * radius {
        return None;
    }
    let depth = (radius * radius - off_axis).sqrt();
    if depth < radius * STEADY_GROUND {
        return None;
    }
    let hit = ray.origin + *ray.direction * (along - depth);
    Some((hit - centre).normalize())
}

/// Turns the carried frame by exactly as much as the ground beneath it turned.
///
/// Everything that moves the camera moves `focus`, and knows nothing about
/// frames - a drag, a keyboard pan, a zoom glide, a jump to a village. This
/// takes the one rotation that carries the frame's own up onto the focus's new
/// up, and applies it. That single rule covers all of them:
///
/// - For a drag or a pan, where the movement is small and continuous, the
///   minimal rotation IS parallel transport along the great circle. The frame
///   arrives with no twist that the journey did not put there, which is what
///   lets the world be rolled in any direction indefinitely.
/// - For a jump there is no continuity to preserve, and the minimal rotation
///   is as good an answer as any - it keeps the screen's up as close to what
///   it was as the sphere allows.
///
/// The one case with no answer is a jump to the exact antipode, where every
/// rotation is minimal. Nothing is continuous across that, so the frame is
/// simply re-seated from the ground.
fn carry_the_frame(mut rigs: Query<&mut CameraRig>) {
    let Ok(mut rig) = rigs.single_mut() else {
        return;
    };
    rig.facing = carried(rig.facing, rig.focus);
}

/// The frame, turned onto the ground the focus now stands on. See
/// `carry_the_frame`.
pub(crate) fn carried(facing: Quat, focus: Vec3) -> Quat {
    let want = crate::globe::planet_stance() * crate::terrain::direction_at(focus.x, focus.z);
    let have = facing * Vec3::Y;
    let across = have.cross(want);
    match across.try_normalize() {
        Some(axis) => {
            // Through `atan2`, not `acos`: the angle here is usually a
            // fraction of a degree, which is exactly where `acos` throws away
            // its significant figures.
            let angle = across.length().atan2(have.dot(want));
            (Quat::from_axis_angle(axis, angle) * facing).normalize()
        }
        None if have.dot(want) < 0.0 => crate::globe::bend_frame(focus).1,
        // Already pointing at it. Nothing to carry.
        None => facing,
    }
}

/// How fast the focus may climb or fall while the world is being dragged, in
/// units a second.
///
/// The ground-following below is one arm of a FEEDBACK LOOP, and the world-grab
/// closes it: the focus moves, the ground under it decides the focus's height,
/// the height moves the eye, the eye recasts the cursor's ray onto the planet,
/// and the new reading moves the focus again. Its gain is the slope of the
/// land, so on the flat it is nothing and on a mountainside it runs away —
/// Brett: "when I click to drag the ground it spazzes out on mountains".
///
/// Worse under the grab than anywhere else, because a held world is SNAPPED
/// rather than smoothed (see `held_by_hand`), so the loop runs with no damping
/// at all and takes a full swing every frame.
///
/// Rate-limiting the height is what opens the loop. The focus still rides the
/// land — a long drag up a mountain still climbs it, which a hard freeze would
/// not — but it can no longer answer a sideways step with a hundred-unit lurch,
/// so there is nothing left for the ray to amplify.
///
/// Squeezed from both sides, and the suite holds both. Slow enough to damp: at
/// sixty frames this is under three units of eye movement in a frame, against
/// the hundred-and-more a cliff edge was handing it. Fast enough not to lag:
/// the whole world is `TERRAIN_HEIGHT` units of relief, so even dragging from
/// the sea to the highest summit the focus is never more than about two and a
/// half seconds behind the ground — and when the relief grows, this must grow
/// with it, or the second squeeze fails. The first number this was written
/// with failed that test — seven seconds to climb a mountain — which is the
/// sort of thing that reads as the camera being broken in the other direction.
const HELD_CLIMB: f32 = 145.0;

/// The focus height for this frame: the ground, or as much of the way there as
/// a held world is allowed to travel.
fn focus_climb(was: f32, ground: f32, held: bool, delta: f32) -> f32 {
    if !held {
        // Free of the hand it snaps, and the smoothing pass eases the picture.
        // Snapping matters: a jump to a village or the founding itself moves
        // the focus across the world, and a rate limit would spend seconds
        // crawling up to the new ground.
        return ground;
    }
    let most = HELD_CLIMB * delta;
    was + (ground - was).clamp(-most, most)
}

/// Keeps the focus point riding the ground, so orbiting over a hill does not
/// bury the camera inside it.
fn follow_ground(
    time: Res<Time<Real>>,
    terrain: Option<Res<Terrain>>,
    mut rigs: Query<&mut CameraRig>,
) {
    let (Some(terrain), Ok(mut rig)) = (terrain, rigs.single_mut()) else {
        return;
    };
    // Not when the focus IS somebody's eyes. This runs after `apply_follow`
    // in the chain and used to overwrite its work every single frame: the
    // eye height went in, the ground height came straight back out, and the
    // god ended up looking out of the villager's boots no matter how
    // carefully their eyes had been measured. The orbit is what needs its
    // focus pinned to the land; a head is not an orbit.
    //
    // Asked of `in_a_body` and not of the distance: with nobody followed the
    // distance stays where it was left, and a camera skipping this while
    // pinned to nothing is a camera at a fixed height that walks through
    // hills.
    if rig.in_a_body {
        return;
    }
    let ground = terrain
        .height_at(rig.target_focus.x, rig.target_focus.z)
        .max(WATER_LEVEL);
    // On REAL time, not the world's. The clamp is about how fast a hand is
    // dragging, and the god can run the day at eight times speed.
    rig.target_focus.y = focus_climb(
        rig.target_focus.y,
        ground,
        rig.held_by_hand,
        time.delta_secs(),
    );
}

/// Eases the camera toward wherever it has been asked to go.
///
/// REAL time, not the world's. The camera is the player's own eye, and
/// pausing pauses the WORLD - not the ability to look at it. On the virtual
/// clock every one of these deltas is zero while the game is paused, so the
/// distance never travelled toward its target and the zoom simply did
/// nothing until time started again. Brett: "I can't zoom while the speed is
/// paused." Looking around a stopped world is most of what a pause is for.
fn apply_camera_smoothing(time: Res<Time<Real>>, mut rigs: Query<&mut CameraRig>) {
    let Ok(mut rig) = rigs.single_mut() else {
        return;
    };

    // Exponential convergence. Framerate-independent, unlike a plain lerp by dt.
    let t = 1.0 - (-rig.smoothing * time.delta_secs()).exp();

    let previous_distance = rig.distance;
    rig.distance += (rig.target_distance - rig.distance) * t;

    // Re-pin the zoom anchor against the distance actually travelled this frame.
    // Both the live focus and its target are moved by the same ratio, so the point
    // under the cursor stays put while panning still works normally afterwards.
    if let Some(anchor) = rig.zoom_anchor {
        let ratio = rig.distance / previous_distance.max(f32::EPSILON);
        rig.focus = zoom_focus(rig.focus, anchor, ratio);
        rig.target_focus = zoom_focus(rig.target_focus, anchor, ratio);

        if (rig.target_distance - rig.distance).abs() < 0.02 {
            rig.zoom_anchor = None;
        }
    }

    if rig.held_by_hand {
        // Snapped. See `held_by_hand`.
        rig.focus = rig.target_focus;
    } else {
        rig.focus = rig.focus.lerp(rig.target_focus, t);
    }
    rig.pitch += (rig.target_pitch - rig.pitch) * t;

    // Take the short way round the circle so crossing the seam does not spin the
    // camera the long way.
    let mut yaw_delta = rig.target_yaw - rig.yaw;
    yaw_delta = (yaw_delta + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI;
    rig.yaw += yaw_delta * t;
}

/// How far the eye keeps off the ground at full standoff, in metres. The
/// clearance tapers to nothing at the focus, which is ON the ground and is
/// supposed to be.
const EYE_CLEARANCE: f32 = 1.2;

fn write_camera_transform(
    terrain: Option<Res<Terrain>>,
    mut cameras: Query<(&CameraRig, &mut Transform), With<GodCamera>>,
) {
    for (rig, mut transform) in &mut cameras {
        // Behind a mortal's eyes there is no orbit to speak of, and both of
        // the things this function does next are wrong there. `looking_at`
        // has no direction to work from when the eye is already at the
        // focus, and the sightline lift below reads the ground under the
        // focus as an obstacle to clear — which, for anybody whose eyes are
        // lower than `EYE_CLEARANCE`, a child especially, cranes the view up
        // out of the top of their head. So: point it by yaw and pitch, and
        // trust the body to be standing somewhere it can stand.
        if rig.distance < FIRST_PERSON {
            *transform = Transform::from_translation(rig.focus).looking_to(rig.forward(), Vec3::Y);
            continue;
        }
        let mut eye = rig.eye();
        // The focus rides the ground, but the EYE swings out behind it -
        // and on a hillside that put it under the slope. Orbiting buried
        // the camera in the hill it was looking across.
        //
        // The whole line from focus to eye has to clear the land, not
        // just the eye's own footing: a ridge between the two is just as
        // solid. Each sample says how high the eye would have to be for
        // the sightline to pass over THAT point, and the eye takes the
        // highest answer. Lifting rather than pushing back keeps the
        // framing: the camera cranes up over the brow instead of
        // retreating from it.
        if let Some(terrain) = &terrain {
            let mut lift = eye.y;
            for step in 1..=10 {
                let t = step as f32 / 10.0;
                let along = rig.focus.lerp(eye, t);
                let ground =
                    terrain.height_at(along.x, along.z).max(WATER_LEVEL) + EYE_CLEARANCE * t;
                lift = lift.max(rig.focus.y + (ground - rig.focus.y) / t);
            }
            eye.y = lift;
        }
        *transform = Transform::from_translation(eye).looking_at(rig.focus, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a cursor at `ndc` lands on the planet, for a rig — the grab's
    /// whole input, computed the way the renderer computes it, so the test is
    /// driving the real geometry rather than a story about it.
    fn cursor_on_the_ball(rig: &CameraRig, ndc: Vec2) -> Option<Vec3> {
        let pose = crate::globe::bent_camera_pose(rig);
        let half = (0.62f32 * 0.5).tan();
        let aspect = 16.0 / 9.0;
        let direction = (pose.forward().as_vec3()
            + pose.right().as_vec3() * ndc.x * half * aspect
            + pose.up().as_vec3() * ndc.y * half)
            .normalize();

        let centre = crate::globe::planet_centre();
        let radius = crate::terrain::PLANET_RADIUS + WATER_LEVEL;
        let to_centre = centre - pose.translation;
        let along = to_centre.dot(direction);
        let closest = pose.translation + direction * along - centre;
        let off_axis = closest.length_squared();
        if off_axis > radius * radius {
            return None;
        }
        let depth = (radius * radius - off_axis).sqrt();
        Some((pose.translation + direction * (along - depth) - centre).normalize())
    }

    #[test]
    fn a_held_world_cannot_lurch_up_a_mountain() {
        // The loop that made the grab unusable on high ground, run as a loop.
        //
        // The focus is dragged onto a mountainside, so the ground under it
        // jumps by most of the world's relief in one frame. Free of the hand
        // that snap is correct and wanted. HELD, it moves the eye, which
        // recasts the cursor's ray, which moves the focus again - and Brett
        // got a camera that "spazzes out on mountains".
        let relief = crate::terrain::TERRAIN_HEIGHT;
        let frame = 1.0 / 60.0;

        let free = focus_climb(20.0, relief, false, frame);
        assert_eq!(free, relief, "a free focus must still snap to its ground");

        let held = focus_climb(20.0, relief, true, frame);
        let step = held - 20.0;
        assert!(
            step <= HELD_CLIMB * frame + 1e-3,
            "a held focus climbed {step} units in one frame - the eye moves \
             with it and the grab reads a different world every frame"
        );

        // The other side of the squeeze, and the one the first attempt at this
        // failed: a limit slow enough to damp is worthless if it is so slow
        // that the camera visibly trails the land under it. Dragging from the
        // sea to the highest ground in the world is the worst case there is.
        let worst = relief / HELD_CLIMB;
        assert!(
            worst < 3.0,
            "climbing the world's whole relief takes {worst:.1}s of dragging - \
             the focus trails the ground, which reads as broken in the other \
             direction"
        );

        // And it does get there: rate-limited is not frozen.
        let mut y = 20.0;
        for _ in 0..((worst + 0.5) * 60.0) as i32 {
            y = focus_climb(y, relief, true, frame);
        }
        assert!(
            (y - relief).abs() < 0.5,
            "the focus settled at {y} and the ground is at {relief}"
        );

        // Downhill the same, or coming off a peak lurches instead.
        let down = focus_climb(relief, 20.0, true, frame);
        assert!(
            relief - down <= HELD_CLIMB * frame + 1e-3,
            "the focus fell {} units in one frame",
            relief - down
        );
    }

    /// The world-grab has to work in BOTH screen axes, at every height, and it
    /// did not: dragging north or south stopped dead a few degrees short of the
    /// pole, because the focus's `z` was clamped to a brim inside it. Zoomed
    /// out far enough to see the whole planet, a single short drag crosses that
    /// brim, so the vertical axis simply stopped — which is exactly what Brett
    /// reported, and what this measures.
    #[test]
    fn the_world_grab_turns_in_both_axes_from_any_height() {
        for distance in [80.0, 1_400.0, 20_000.0] {
            let rig = CameraRig {
                distance,
                target_distance: distance,
                ..default()
            };
            let held = cursor_on_the_ball(&rig, Vec2::ZERO).unwrap_or_else(|| {
                panic!("the middle of the screen missed the planet at {distance}")
            });

            for (axis, drag) in [
                ("sideways", Vec2::new(0.35, 0.0)),
                ("up-screen", Vec2::new(0.0, 0.35)),
                ("down-screen", Vec2::new(0.0, -0.35)),
            ] {
                let Some(now) = cursor_on_the_ball(&rig, drag) else {
                    continue;
                };
                let moved = turn_the_world(rig.target_focus, held, now);
                let step = (moved - rig.target_focus).length();
                assert!(
                    step > 0.5,
                    "a {axis} grab at {distance} up moved the focus {step}"
                );
            }
        }
    }

    /// And it keeps turning over the top of the world. The flat scaffold has a
    /// genuine singularity at the pole — every longitude meets there — so the
    /// coordinates jump half a circumference as the focus crosses it, but the
    /// PLACE is continuous, and that is what the camera is actually pointing
    /// at. A clamp short of the pole was a wall in the middle of the sky.
    #[test]
    fn the_grab_carries_on_over_the_pole() {
        let quarter = crate::terrain::planet_circumference() * 0.25;
        // A focus a whisker short of the north pole, and a nudge further north.
        let near_pole = Vec3::new(0.0, 0.0, -quarter + 60.0);
        let stance = crate::globe::planet_stance();
        let held = stance * crate::terrain::direction_at(0.0, near_pole.z);
        let now = stance * crate::terrain::direction_at(0.0, near_pole.z + 200.0);

        let moved = turn_the_world(near_pole, held, now);
        let seat_before = crate::globe::bend_frame(near_pole).0;
        let seat_after = crate::globe::bend_frame(moved).0;
        // The grab turns the world by the arc it was dragged, so the focus
        // travels that arc: two hundred units, sixty of them to the pole and
        // the rest down the far side. Asserted as a LENGTH and not merely as
        // "it moved", because the clamp this replaced moved it too — three
        // hundred units backwards, away from the pole it would not cross.
        let travelled = (seat_after - seat_before).length();
        assert!(
            (travelled - 200.0).abs() < 40.0,
            "the focus travelled {travelled} where the drag was 200"
        );
        // And the signature of having crossed: every longitude meets at a
        // pole, so coming down the other side is half a world away in `x`.
        let round = crate::terrain::planet_circumference();
        assert!(
            ((moved.x - near_pole.x).abs() - round * 0.5).abs() < round * 0.02,
            "the focus did not come out on the far side: x moved {}",
            moved.x - near_pole.x
        );
    }

    /// Walk the focus straight over the north pole and watch the two frames.
    ///
    /// This is the whole argument for carrying one. The planet does not spin,
    /// so it has no axis and no poles - but a frame derived from longitude and
    /// latitude has them regardless, and the camera inherited them.
    #[test]
    fn the_carried_frame_crosses_the_pole_without_flipping() {
        let quarter = crate::terrain::PLANET_RADIUS * std::f32::consts::FRAC_PI_2;
        let mut focus = Vec3::new(0.0, 0.0, -quarter + 300.0);

        let mut facing = crate::globe::bend_frame(focus).1;
        let mut was_facing = facing;
        let mut was_bent = crate::globe::bend_frame(focus).1;
        let mut worst_carried = 0.0f32;
        let mut worst_derived = 0.0f32;

        // Five units a step, straight north, right over the top and down the
        // far side.
        for _ in 0..120 {
            focus.z -= 5.0;
            focus = fold_onto_the_sphere(focus);

            facing = carried(facing, focus);
            worst_carried =
                worst_carried.max((facing * Vec3::X).angle_between(was_facing * Vec3::X));
            was_facing = facing;

            let bent = crate::globe::bend_frame(focus).1;
            worst_derived = worst_derived.max((bent * Vec3::X).angle_between(was_bent * Vec3::X));
            was_bent = bent;
        }

        // Five units of ground on a six thousand unit world is under a tenth
        // of a degree of arc, and the carried frame never turns more than the
        // ground under it did.
        assert!(
            worst_carried < 0.01,
            "the carried frame lurched by {worst_carried} radians in one step",
        );
        // While the derived one turns end for end, because every longitude
        // meets at the pole and east is the derivative along longitude.
        assert!(
            worst_derived > 1.0,
            "the lat/lon frame was expected to flip at the pole; worst step was \
             only {worst_derived} radians - has `bend_frame` changed?",
        );
    }

    /// And the frame stays a frame: rolled a long way in one direction it must
    /// not drift out of square or stop being a rotation.
    #[test]
    fn the_carried_frame_stays_square_however_far_it_rolls() {
        let mut focus = Vec3::new(0.0, 0.0, 0.0);
        let mut facing = crate::globe::bend_frame(focus).1;
        // Three times round the world, diagonally, so it crosses both poles
        // and the date line many times over.
        for _ in 0..4_000 {
            focus.x += 21.0;
            focus.z -= 13.0;
            focus = fold_onto_the_sphere(focus);
            facing = carried(facing, focus);
        }
        assert!(
            (facing.length() - 1.0).abs() < 1e-3,
            "drifted off unit length"
        );
        let east = facing * Vec3::X;
        let up = facing * Vec3::Y;
        assert!(east.dot(up).abs() < 1e-3, "east and up came out of square");
        // And it is still standing on the ground it says it is.
        let want = crate::globe::planet_stance() * crate::terrain::direction_at(focus.x, focus.z);
        assert!(up.distance(want) < 1e-2, "the frame drifted off the focus");
    }

    #[test]
    fn eye_sits_at_the_requested_distance() {
        let rig = CameraRig::default();
        assert!((rig.eye().distance(rig.focus) - rig.distance).abs() < 1e-3);
    }

    #[test]
    fn eye_stays_above_the_focus() {
        // Pitch is clamped positive, so the camera must always look downward.
        for pitch in [MIN_PITCH, 0.5, 1.0, MAX_PITCH] {
            let rig = CameraRig { pitch, ..default() };
            assert!(rig.eye().y > rig.focus.y);
        }
    }

    #[test]
    fn forward_points_from_eye_toward_focus() {
        let rig = CameraRig::default();
        let expected = (rig.focus - rig.eye()).normalize();
        assert!(rig.forward().distance(expected) < 1e-4);
    }

    #[test]
    fn ground_directions_are_horizontal_and_perpendicular() {
        for yaw in [0.0, 0.7, 2.5, -1.9] {
            let rig = CameraRig { yaw, ..default() };
            let f = rig.ground_forward();
            let r = rig.ground_right();
            assert!(f.y.abs() < 1e-6 && r.y.abs() < 1e-6);
            assert!(f.dot(r).abs() < 1e-5);
        }
    }

    #[test]
    fn zooming_all_the_way_in_arrives_at_the_cursor() {
        let focus = Vec3::new(10.0, 2.0, -4.0);
        let target = Vec3::new(-3.0, 5.0, 8.0);
        // Ratio 0 means the camera collapsed onto the target point. Only the ground
        // plane is compared: height belongs to `follow_ground`.
        let result = zoom_focus(focus, target, 0.0);
        assert!((result.x - target.x).abs() < 1e-5);
        assert!((result.z - target.z).abs() < 1e-5);
        assert_eq!(result.y, focus.y, "zoom must not touch height");
    }

    #[test]
    fn trackpad_and_wheel_scrolls_agree() {
        // A trackpad reports pixels, a wheel reports lines, and the pixel figure is
        // ~100x larger for the same gesture. Left unnormalised, every trackpad
        // scroll saturated the zoom clamp.
        let wheel = normalised_scroll(1.0, MouseScrollUnit::Line);
        let trackpad = normalised_scroll(
            MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
            MouseScrollUnit::Pixel,
        );
        assert!((wheel - trackpad).abs() < 1e-5);
    }

    #[test]
    fn a_gentle_trackpad_scroll_stays_gentle() {
        // The failure this guards: any nudge at all producing a full-step zoom.
        let rig = CameraRig::default();
        let scroll = normalised_scroll(6.0, MouseScrollUnit::Pixel);
        let factor = (1.0 - scroll * rig.zoom_sensitivity).clamp(0.5, 2.0);
        assert!(factor > 0.99 && factor < 1.0, "factor was {factor}");
    }

    #[test]
    fn zooming_not_at_all_leaves_the_focus_alone() {
        let focus = Vec3::new(10.0, 2.0, -4.0);
        let target = Vec3::new(-3.0, 5.0, 8.0);
        assert!(zoom_focus(focus, target, 1.0).distance(focus) < 1e-5);
    }

    #[test]
    fn zooming_in_moves_the_focus_toward_the_cursor() {
        let focus = Vec3::new(20.0, 0.0, 0.0);
        let target = Vec3::ZERO;

        // Each step in closes the gap; each step out widens it.
        let closer = zoom_focus(focus, target, 0.5);
        assert!(closer.distance(target) < focus.distance(target));

        let further = zoom_focus(focus, target, 2.0);
        assert!(further.distance(target) > focus.distance(target));
    }

    #[test]
    fn repeated_zoom_steps_converge_on_the_cursor() {
        // Scrolling in repeatedly with the pointer held still should walk the view
        // onto that spot rather than drifting past or stalling short of it.
        let target = Vec3::new(7.0, 1.0, -2.0);
        let mut focus = Vec3::new(-30.0, 1.0, 40.0);
        for _ in 0..40 {
            focus = zoom_focus(focus, target, 0.88);
        }
        assert!(focus.distance(target) < 0.5, "ended {focus:?}");
    }

    #[test]
    fn zoom_fraction_spans_the_full_range() {
        let near = CameraRig {
            distance: MIN_DISTANCE,
            ..default()
        };
        let far = CameraRig {
            distance: MAX_DISTANCE,
            ..default()
        };
        assert!(near.zoom_fraction() < 1e-5);
        assert!((far.zoom_fraction() - 1.0).abs() < 1e-5);
    }

    /// The whole of Avatar rests on this. At zero distance the eye sits on
    /// the focus, and `forward` used to be the normalised offset between
    /// them — which is a zero vector, and normalising it gave NaN. A NaN
    /// look direction does not crash: it silently points the view nowhere,
    /// so the god turns their head and nothing moves.
    #[test]
    fn the_look_direction_survives_zero_distance() {
        for pitch in [-RIDDEN_PITCH, -0.4, 0.0, ARRIVING_ISH, RIDDEN_PITCH] {
            for yaw in [0.0, 1.0, 3.0, -2.5] {
                let rig = CameraRig {
                    distance: 0.0,
                    yaw,
                    pitch,
                    ..default()
                };
                let dir = rig.forward();
                assert!(dir.is_finite(), "pitch {pitch} yaw {yaw} gave {dir:?}");
                assert!((dir.length() - 1.0).abs() < 1e-4, "not a unit: {dir:?}");
            }
        }
    }

    /// A worn body has to be able to look up as well as down — the overhead
    /// floor of `MIN_PITCH` faces permanently at the ground.
    #[test]
    fn a_worn_body_can_look_up_and_at_its_own_boots() {
        let up = CameraRig {
            distance: 0.0,
            pitch: -RIDDEN_PITCH,
            ..default()
        };
        let down = CameraRig {
            distance: 0.0,
            pitch: RIDDEN_PITCH,
            ..default()
        };
        assert!(
            up.forward().y > 0.5,
            "cannot see the sky: {:?}",
            up.forward()
        );
        assert!(
            down.forward().y < -0.5,
            "cannot see its feet: {:?}",
            down.forward()
        );
        // And the overhead rig still cannot, which is what keeps the orbit
        // from going under the ground it orbits.
        assert!(MIN_PITCH > 0.0);
    }

    const ARRIVING_ISH: f32 = 0.12;

    /// Eyes belong in the head: above the shoulders, below the crown. Get
    /// this wrong and the god either looks out of somebody's chest or floats
    /// above their hair.
    #[test]
    fn the_eyes_sit_within_the_head() {
        use crate::creature::genome::{CreatureGenome, Species};
        let mut rng = crate::rng::Rng::new(0x_EEE5);
        for _ in 0..200 {
            let genome = CreatureGenome::random(Species::Human, &mut rng);
            let p = &genome.proportions;
            let h = genome.height();
            let head = crate::creature::body::biped_head_size(&genome);
            let centre = (p.leg_length + p.torso_length + p.neck_length) * h - head * 0.12;
            let eyes = eye_height(&genome);
            assert!(
                eyes > centre - head * 0.5 && eyes < centre + head * 0.5,
                "eyes at {eyes} are outside a head of {head} centred at {centre}"
            );
            // And above the shoulders, so looking down finds a chest.
            assert!(eyes > (p.leg_length + p.torso_length) * h);

            // Forward of the head's centre, but not out past the face. At
            // the centre, looking down looks down the neck hole.
            let ahead = eye_forward(&genome);
            assert!(
                ahead > head * 0.25 && ahead < head * 0.5,
                "eyes {ahead} forward of centre in a head of {head}"
            );
        }
    }
}
