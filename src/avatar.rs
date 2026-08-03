//! Avatar: the god takes a body and walks about in it.
//!
//! Every other miracle is done TO the world from outside it — lightning
//! on a point, fruit on a bush, the ground thrown like a blanket. This
//! one puts the god inside the world as a person, and NOBODY REMARKS ON
//! IT. That is the whole feature: it is the only way a god ever hears
//! what is said about them by people who believe they are alone.
//!
//! Selected on its key, then left-clicked on a villager rather than on a
//! patch of ground — which is why it lives here and not in the `cast`
//! system, that being about points on the map (and at Bevy's parameter
//! ceiling besides).
//!
//! Five minutes, and the belief drains the whole time. A god who can be
//! a person indefinitely stops being a god.

use bevy::prelude::*;

use crate::camera::{CameraRig, FollowStyle, FollowTarget};
use crate::creature::MoveTarget;
use crate::miracles::{AVATAR_COST, Miracle, SelectedMiracle};
use crate::villager::{Activity, Villager};

/// The body the god is currently wearing, and when it has to be given
/// back.
#[derive(Component)]
pub struct Ridden {
    /// Seconds of riding left.
    pub left: f32,
}

/// How long a body may be worn, before belief runs out anyway.
const RIDE_FOR: f32 = 300.0;

/// Belief spent per second of wearing somebody. The cast is cheap and the
/// staying is not.
const DRAIN: f32 = 0.05;

/// How far ahead of the eyes the body is told to walk. Far enough that
/// the ordinary locomotion has somewhere to go, near enough that letting
/// go of the key stops them promptly.
const STRIDE: f32 = 3.0;

/// How far the god pulls back out to on letting a body go — the ordinary
/// working zoom, so you are returned to the view you play from.
const LEAVING_HEIGHT: f32 = 80.0;

pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (take_a_body, wear_it, drive_the_body)
                .chain()
                .run_if(in_state(crate::GameState::Playing)),
        );
    }
}

/// Left-click a villager with Avatar armed and the god is in them.
fn take_a_body(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<crate::ui::PointerContext>,
    hand: Res<crate::hand::DivineHand>,
    mut selected: ResMut<SelectedMiracle>,
    mut belief: ResMut<crate::villager::belief::Belief>,
    mut follow: ResMut<FollowTarget>,
    mut notices: MessageWriter<crate::ui::Notice>,
    folk: Query<(), (With<Villager>, Without<crate::creature::Corpse>)>,
    worn: Query<Entity, With<Ridden>>,
) {
    if selected.0 != Some(Miracle::Avatar) {
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) || pointer.over_ui {
        return;
    }
    let Some(who) = hand.hovered.filter(|who| folk.contains(*who)) else {
        return;
    };
    if belief.available() < AVATAR_COST {
        return;
    }
    // One body at a time.
    for already in &worn {
        commands.entity(already).remove::<Ridden>();
    }
    belief.spent += AVATAR_COST;
    commands.entity(who).insert(Ridden { left: RIDE_FOR });
    follow.entity = Some(who);
    follow.style = FollowStyle::Eyes;
    selected.0 = None;
    notices.write(crate::ui::Notice::new(
        "You are looking out of somebody else's eyes".to_string(),
    ));
}

/// The ride: it costs belief every second, and it ends.
fn wear_it(
    time: Res<Time>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut belief: ResMut<crate::villager::belief::Belief>,
    mut follow: ResMut<FollowTarget>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut ridden: Query<(Entity, &mut Ridden)>,
    mut rigs: Query<&mut CameraRig>,
) {
    let dt = time.delta_secs();
    for (who, mut ride) in &mut ridden {
        ride.left -= dt;
        belief.spent += DRAIN * dt;
        // Given back willingly, run out of time, or run out of belief.
        let done =
            ride.left <= 0.0 || belief.available() <= 0.0 || keys.just_pressed(KeyCode::Escape);
        if done {
            commands.entity(who).remove::<Ridden>();
            if follow.entity == Some(who) {
                follow.entity = None;
                follow.style = FollowStyle::Overhead;
                // And the god rises back out of them. Targets again, so
                // the same smoothing that flew the camera down into the
                // body carries it back up - letting go of the pin while
                // the distance was still nought left the view lying on
                // the grass where the body had been.
                if let Ok(mut rig) = rigs.single_mut() {
                    rig.target_distance = LEAVING_HEIGHT;
                    rig.target_pitch = rig.target_pitch.max(0.7);
                }
            }
            notices.write(crate::ui::Notice::new("You are yourself again".to_string()));
        }
    }
}

/// Driving. The body is not moved directly — it is told where to go, and
/// walks there on its own legs, so every gait, slope and stumble the
/// village already has applies to the god as much as to anybody.
fn drive_the_body(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    rigs: Query<&CameraRig>,
    mut ridden: Query<(&Transform, &mut MoveTarget, &mut Activity), With<Ridden>>,
) {
    use crate::keymap::Deed;
    let Ok(rig) = rigs.single() else {
        return;
    };
    for (at, mut target, mut activity) in &mut ridden {
        // Whatever errand they were on, they are not on it now.
        if *activity != Activity::Wandering {
            *activity = Activity::Wandering;
        }
        // Where the eyes are pointed, flattened: forward on the ground
        // is the camera's own bearing.
        let (sin, cos) = rig.yaw.sin_cos();
        let ahead = Vec3::new(-sin, 0.0, -cos);
        let beside = Vec3::new(cos, 0.0, -sin);
        let mut push = Vec3::ZERO;
        if keymap.pressed(&keys, Deed::PanNorth) {
            push += ahead;
        }
        if keymap.pressed(&keys, Deed::PanSouth) {
            push -= ahead;
        }
        if keymap.pressed(&keys, Deed::PanWest) {
            push -= beside;
        }
        if keymap.pressed(&keys, Deed::PanEast) {
            push += beside;
        }
        target.0 =
            (push.length_squared() > 0.0).then(|| at.translation + push.normalize() * STRIDE);
    }
}
