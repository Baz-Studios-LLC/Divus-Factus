//! The bench camera: orbit, zoom and pan, tuned to the game's hand-feel.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

#[derive(Resource)]
pub struct OrbitRig {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for OrbitRig {
    fn default() -> Self {
        OrbitRig {
            focus: Vec3::ZERO,
            yaw: 0.6,
            pitch: 0.7,
            distance: 16.0,
        }
    }
}

impl OrbitRig {
    fn eye(&self) -> Vec3 {
        let flat = Vec2::from_angle(self.yaw) * self.pitch.cos();
        self.focus + Vec3::new(flat.x, self.pitch.sin(), flat.y) * self.distance
    }

    fn ground_forward(&self) -> Vec3 {
        let eye_to_focus = self.focus - self.eye();
        Vec3::new(eye_to_focus.x, 0.0, eye_to_focus.z).normalize_or_zero()
    }

    fn ground_right(&self) -> Vec3 {
        self.ground_forward().cross(Vec3::Y) * -1.0
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitRig>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (steer, place).chain());
    }
}

fn spawn_camera(mut commands: Commands, rig: Res<OrbitRig>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(rig.eye()).looking_at(rig.focus, Vec3::Y),
    ));
}

fn steer(
    keys: Res<ButtonInput<KeyCode>>,
    naming: Res<crate::builder::Naming>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    mut rig: ResMut<OrbitRig>,
) {
    // Orbit on right mouse, the game's way.
    if buttons.pressed(MouseButton::Right) {
        rig.yaw += motion.delta.x * 0.008;
        rig.pitch = (rig.pitch + motion.delta.y * 0.008).clamp(0.12, 1.55);
    }

    // While a name is being typed, the keyboard belongs to the pen.
    if naming.0.is_some() {
        return;
    }

    // Snapped views on the number row: the drafting angles. Front is the
    // door side - the gold sill - and overhead is the plan view.
    let pi = std::f32::consts::PI;
    for (key, yaw, pitch) in [
        (KeyCode::Digit1, 0.0, 0.32),       // front, the door side
        (KeyCode::Digit2, pi * 0.5, 0.32),  // right side
        (KeyCode::Digit3, pi, 0.32),        // back
        (KeyCode::Digit4, -pi * 0.5, 0.32), // left side
        (KeyCode::Digit5, 0.0, 1.55),       // overhead, the plan
        (KeyCode::Digit6, 0.6, 0.7),        // the working perch
    ] {
        if keys.just_pressed(key) {
            rig.yaw = yaw;
            rig.pitch = pitch;
        }
    }

    // Pan on middle mouse: the bench moves with the pointer.
    if buttons.pressed(MouseButton::Middle) && motion.delta != Vec2::ZERO {
        let scale = rig.distance * 0.0016;
        let right = rig.ground_right();
        let forward = rig.ground_forward();
        rig.focus -= right * motion.delta.x * scale;
        rig.focus += forward * motion.delta.y * scale;
    }

    // Keyboard glide, WASD and arrows both.
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
        let speed = (0.35 + rig.distance * 0.12) * 8.0 * time.delta_secs();
        rig.focus += pan.normalize() * speed;
    }

    // The wheel draws near and pulls away.
    if scroll.delta.y != 0.0 {
        rig.distance = (rig.distance * (1.0 - scroll.delta.y * 0.08)).clamp(3.0, 60.0);
    }
}

fn place(rig: Res<OrbitRig>, mut cameras: Query<&mut Transform, With<Camera3d>>) {
    for mut transform in &mut cameras {
        *transform = Transform::from_translation(rig.eye()).looking_at(rig.focus, Vec3::Y);
    }
}
