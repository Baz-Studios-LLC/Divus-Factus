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
                    read_camera_input,
                    apply_follow,
                    follow_ground,
                    apply_camera_smoothing,
                    write_camera_transform,
                )
                    .chain()
                    .in_set(CameraSet),
            );
    }
}

/// How the camera rides along when following someone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FollowStyle {
    /// The god's view, pinned to them: orbit and zoom stay free.
    #[default]
    Overhead,
}

/// Whom the camera is following, if anyone.
///
/// Set by right-clicking a creature under the hand. Only right-clicks change
/// it: the next clean click lets go, and clicking a different creature
/// switches to them. Nothing else — not panning,
/// not time — releases a follow.
#[derive(Resource, Default)]
pub struct FollowTarget {
    pub entity: Option<Entity>,
    pub style: FollowStyle,
}

/// Rides the focus (and, at the shoulder, the whole rig) along with whoever
/// is being followed.
fn apply_follow(
    mut follow: ResMut<FollowTarget>,
    targets: Query<&GlobalTransform>,
    mut rigs: Query<&mut CameraRig>,
) {
    let Some(entity) = follow.entity else {
        return;
    };
    let (Ok(mut rig), Ok(target)) = (rigs.single_mut(), targets.get(entity)) else {
        // Whoever it was is gone from the world.
        follow.entity = None;
        return;
    };

    let at = target.translation();
    rig.target_focus.x = at.x;
    rig.target_focus.z = at.z;
    // A followed zoom anchor fights the pin; the pin wins.
    rig.zoom_anchor = None;
}

/// Camera systems run as a unit so that anything needing a settled camera — the
/// Hand's raycast, for one — can simply order itself after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSet;

/// Startup set that spawns the camera. The render pipeline orders itself after
/// this so it has a camera to attach the offscreen target to.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraStartupSet;

const MIN_PITCH: f32 = 0.20;
const MAX_PITCH: f32 = FRAC_PI_2 - 0.06;
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
    pub pitch: f32,
    pub target_pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
    /// How fast panning moves the focus, in world units per second at mid zoom.
    pub pan_speed: f32,
    pub orbit_sensitivity: f32,
    pub zoom_sensitivity: f32,
    /// Higher converges faster. Tuned by feel; around 12 reads as responsive but
    /// still weighted, which suits something the size of a god.
    pub smoothing: f32,
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
            pitch: 0.85,
            target_pitch: 0.85,
            distance: 62.0,
            target_distance: 62.0,
            pan_speed: 28.0,
            orbit_sensitivity: 0.005,
            zoom_sensitivity: 0.12,
            smoothing: 12.0,
            zoom_anchor: None,
        }
    }
}

impl CameraRig {
    /// Unit vector the camera looks along, from its eye toward the focus.
    pub fn forward(&self) -> Vec3 {
        -self.eye_offset().normalize()
    }

    /// Offset from focus to eye.
    fn eye_offset(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(sy * cp, sp, cy * cp) * self.distance
    }

    /// World-space eye position.
    pub fn eye(&self) -> Vec3 {
        self.focus + self.eye_offset()
    }

    /// Ground-plane forward direction, for panning relative to the view.
    fn ground_forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(-sy, 0.0, -cy)
    }

    /// Ground-plane right direction.
    fn ground_right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(cy, 0.0, -sy)
    }

    /// Zoom as a 0-to-1 value, 0 being closest.
    pub fn zoom_fraction(&self) -> f32 {
        ((self.distance - MIN_DISTANCE) / (MAX_DISTANCE - MIN_DISTANCE)).clamp(0.0, 1.0)
    }
}

/// Converts a frame's scroll delta into wheel notches.
///
/// A mouse wheel reports discrete lines; a trackpad reports pixels, roughly a
/// hundred times larger for the same gesture. Feeding the raw delta into the zoom
/// meant every trackpad scroll slammed straight into the clamp and the camera
/// jumped its maximum step regardless of how gently it was nudged.
fn normalised_scroll(delta: f32, unit: MouseScrollUnit) -> f32 {
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
    Vec3::new(
        target.x + (focus.x - target.x) * ratio,
        focus.y,
        target.z + (focus.z - target.z) * ratio,
    )
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
            fov: 0.62,
            near: 0.5,
            // Past max zoom (1400) plus the widest fog band, so overlooking the
            // world from the top of the zoom shows a world, not the clip plane.
            far: 3000.0,
            ..default()
        }),
        Transform::from_translation(rig.eye()).looking_at(rig.focus, Vec3::Y),
        GodCamera,
        rig,
    ));
}

fn read_camera_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    pointer: Res<crate::ui::PointerContext>,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<GodCamera>>,
    mut rigs: Query<&mut CameraRig>,
) {
    let Ok(mut rig) = rigs.single_mut() else {
        return;
    };

    // Panning. Speed scales with zoom so that crossing the screen takes about the
    // same time whether you are looking at one villager or the whole valley.
    let mut pan = Vec3::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        pan += rig.ground_forward();
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        pan -= rig.ground_forward();
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        pan -= rig.ground_right();
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        pan += rig.ground_right();
    }

    if pan != Vec3::ZERO {
        let zoom_scale = 0.35 + rig.zoom_fraction() * 1.6;
        let speed = rig.pan_speed * zoom_scale * time.delta_secs();
        rig.target_focus += pan.normalize() * speed;
    }

    // Drag-panning on middle mouse. Moves the world with the pointer rather than
    // moving the camera with it, which is the direction people expect from having
    // dragged every map they have ever used.
    if buttons.pressed(MouseButton::Middle) {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            // Scale with distance so a pixel of mouse travel covers roughly the same
            // amount of ground at every zoom level, and the grabbed point stays
            // under the cursor.
            let scale = rig.distance * DRAG_PAN_SCALE;
            let right = rig.ground_right();
            let forward = rig.ground_forward();
            rig.target_focus -= right * delta.x * scale;
            rig.target_focus += forward * delta.y * scale;
        }
    }

    // Orbiting, on right mouse.
    if buttons.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            let sensitivity = rig.orbit_sensitivity;
            rig.target_yaw -= delta.x * sensitivity;
            rig.target_pitch =
                (rig.target_pitch + delta.y * sensitivity).clamp(MIN_PITCH, MAX_PITCH);
        }
    }

    // Keyboard orbit, so the camera is fully usable without a mouse button held.
    let mut keyboard_yaw = 0.0;
    if keys.pressed(KeyCode::KeyQ) {
        keyboard_yaw += 1.0;
    }
    if keys.pressed(KeyCode::KeyE) {
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
        rig.target_distance = (rig.target_distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);

        // Zoom toward whatever is under the cursor rather than the centre of the
        // screen, so zooming doubles as aiming: you can drop onto one villager
        // without panning there first.
        rig.zoom_anchor = cursor_ground_point(&windows, &cameras, terrain.as_deref());
    }
}

/// Keeps the focus point riding the ground, so orbiting over a hill does not
/// bury the camera inside it.
fn follow_ground(terrain: Option<Res<Terrain>>, mut rigs: Query<&mut CameraRig>) {
    let (Some(terrain), Ok(mut rig)) = (terrain, rigs.single_mut()) else {
        return;
    };
    let ground = terrain
        .height_at(rig.target_focus.x, rig.target_focus.z)
        .max(WATER_LEVEL);
    rig.target_focus.y = ground;
}

fn apply_camera_smoothing(time: Res<Time>, mut rigs: Query<&mut CameraRig>) {
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

    rig.focus = rig.focus.lerp(rig.target_focus, t);
    rig.pitch += (rig.target_pitch - rig.pitch) * t;

    // Take the short way round the circle so crossing the seam does not spin the
    // camera the long way.
    let mut yaw_delta = rig.target_yaw - rig.yaw;
    yaw_delta = (yaw_delta + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI;
    rig.yaw += yaw_delta * t;
}

fn write_camera_transform(mut cameras: Query<(&CameraRig, &mut Transform), With<GodCamera>>) {
    for (rig, mut transform) in &mut cameras {
        *transform = Transform::from_translation(rig.eye()).looking_at(rig.focus, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
