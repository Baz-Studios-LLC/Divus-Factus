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
    /// Whether the body is currently hidden because the camera has arrived
    /// inside it. Remembered so the visibility is written on the two frames
    /// it changes rather than on every frame.
    hidden: bool,
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

/// Where the god's gaze is set as it arrives in a body: a touch below
/// level, the way somebody walking looks at the ground ahead of them.
const ARRIVING_PITCH: f32 = 0.12;

/// Inside this camera distance the worn body stops being drawn at all.
///
/// The head first, on the theory that looking down and finding your own
/// chest and boots would sell the possession — but these bodies are built
/// for the middle distance, and from eight inches away a villager is a
/// stack of boxes. So the whole of it goes.
///
/// Not on possession, though: hiding it the moment the body is taken would
/// vanish a villager the god is still forty metres above, which reads as
/// them dropping dead. By the time the camera is this close it is already
/// inside the skull, so both the vanishing and the return happen where they
/// cannot be seen.
const OUT_OF_SIGHT: f32 = 2.5;

pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (take_a_body, wear_it)
                .chain()
                .run_if(in_state(crate::GameState::Playing)),
        )
        // Driving runs in `Last`, and that is not tidiness. Better than
        // twenty villager systems write `MoveTarget` — work, errands,
        // doorways, gossip, wandering — and none of them are ordered
        // against each other or against this one, so writing the god's
        // step anywhere in `Update` is a coin toss with the villager's own
        // errand. They kept winning: the body wandered off on its own
        // business with the god aboard. Written here it is the last word
        // of the frame, and `plan_routes` at the head of the next one reads
        // it. Nothing else in the schedule can outvote it, including
        // whatever behaviour gets added next year.
        .add_systems(
            Last,
            drive_the_body.run_if(in_state(crate::GameState::Playing)),
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
    mut lens: Query<&mut Projection, With<crate::camera::GodCamera>>,
    mut rigs: Query<&mut CameraRig>,
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
    commands.entity(who).insert(Ridden {
        left: RIDE_FOR,
        hidden: false,
    });
    follow.entity = Some(who);
    follow.style = FollowStyle::Eyes;
    // The lens has to come in. Overhead, the near plane sits at half a
    // metre because nothing is ever nearer than that; behind a mortal's
    // eyes their own chest is a hand's width away, and at half a metre the
    // whole body is clipped out of the frame. Looking down and finding
    // yourself standing there is most of what sells this, so the plane
    // comes in to a few centimetres for as long as the ride lasts.
    if let Ok(mut lens) = lens.single_mut()
        && let Projection::Perspective(lens) = &mut *lens
    {
        lens.near = crate::camera::CLOSE_NEAR;
    }
    // Level the look once, on the way in. Overhead the rig is pitched
    // steeply down at the ground; arriving in a body still pitched that way
    // would put the god's first mortal view at their own feet. A little
    // below level, so the first thing seen is the village.
    if let Ok(mut rig) = rigs.single_mut() {
        rig.target_pitch = ARRIVING_PITCH;
    }
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
    mut selected: ResMut<SelectedMiracle>,
    mut belief: ResMut<crate::villager::belief::Belief>,
    mut follow: ResMut<FollowTarget>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut ridden: Query<(Entity, &mut Ridden)>,
    mut rigs: Query<&mut CameraRig>,
    mut lens: Query<&mut Projection, With<crate::camera::GodCamera>>,
) {
    let dt = time.delta_secs();
    // How far the dive has got. Read before the loop, since the same rig
    // serves whoever is worn.
    let arrived = rigs.single().is_ok_and(|rig| rig.distance < OUT_OF_SIGHT);
    for (who, mut ride) in &mut ridden {
        ride.left -= dt;
        // Take what there is to take. Spending past the end of the pool
        // would run `available` negative, and the readout with it.
        belief.spent += (DRAIN * dt).min(belief.available().max(0.0));
        if arrived != ride.hidden {
            ride.hidden = arrived;
            // The root, so every part of them goes with it.
            commands.entity(who).insert(if arrived {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
        }
        // Reaching for the miracle a second time is how you put the body
        // down. Arming costs nothing, so leaving costs nothing — the god
        // has already paid to be here. The selection is cleared with it,
        // or the next click on the ground would try to cast at a person.
        let recast = selected.0 == Some(Miracle::Avatar);
        if recast {
            selected.0 = None;
        }
        // Given back willingly, or run out of time.
        //
        // NOT on an empty pool, which is what threw the god out of a body
        // one frame after entering it: the cast costs three and an early
        // village only believes three, so `available` was nought before the
        // first step was taken. Time is the bound that matters — five
        // minutes — and the drain above still empties the pool, so a long
        // ride is paid for by having no miracles left when it ends.
        let done = recast || ride.left <= 0.0 || keys.just_pressed(KeyCode::Escape);
        if done {
            commands.entity(who).remove::<Ridden>();
            // Given back visible. If the god leaves while the camera is still
            // inside the skull, this is the only thing that puts them back in
            // the world at all.
            commands.entity(who).insert(Visibility::Inherited);
            if let Ok(mut lens) = lens.single_mut()
                && let Projection::Perspective(lens) = &mut *lens
            {
                lens.near = crate::camera::WIDE_NEAR;
            }
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
