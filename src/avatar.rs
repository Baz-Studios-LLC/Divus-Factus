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
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

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

/// How far ahead of the eyes the body is told to walk.
///
/// It has to clear the arrival crawl. Locomotion eases off as it nears a
/// destination — `distance / (height * 2)`, floored at a quarter speed —
/// which for an adult means anything inside about three and a half metres is
/// walked at less than full pace. At three metres, which this was, the god
/// never got out of that crawl and plodded everywhere. Eight is past the
/// threshold for the tallest villager with room to spare, and costs nothing
/// in responsiveness: the goal is re-aimed from the body's own position
/// every frame, and letting go of the key clears it outright.
const STRIDE: f32 = 8.0;

/// How far the god pulls back out to on letting a body go — the ordinary
/// working zoom, so you are returned to the view you play from.
const LEAVING_HEIGHT: f32 = 80.0;

/// How much faster a sprinted body moves than a walked one.
///
/// Not so much that a villager reads as a vehicle — they are still somebody's
/// legs — but enough that crossing the settlement is a decision rather than a
/// wait.
const SPRINT: f32 = 1.8;

/// Upward speed of a jump, in world units per second.
///
/// Chosen against the landing: the fall home arrives at the same speed it
/// left, and harm begins above about eight, so this clears a low wall and
/// still lands without hurting the body it borrowed. Jump off something tall
/// and that is a different sum, and the fall will be honoured as any other.
const JUMP: f32 = 7.0;

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
    folk: Query<&Transform, (With<Villager>, Without<crate::creature::Corpse>)>,
    worn: Query<Entity, With<Ridden>>,
    mut rigs: Query<&mut CameraRig>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if selected.0 != Some(Miracle::Avatar) {
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) || pointer.over_ui {
        return;
    }
    let Some((who, facing)) = hand
        .hovered
        .and_then(|who| folk.get(who).ok().map(|at| (who, at.rotation)))
    else {
        return;
    };
    if belief.available() < AVATAR_COST {
        return;
    }
    // One body at a time — and give the last one back VISIBLE. Dropping
    // `Ridden` alone left whoever was worn before standing about the village
    // permanently invisible, since the only thing that undraws a body is the
    // ride and the only thing that redraws it is the ride ending properly.
    for already in &worn {
        commands
            .entity(already)
            .remove::<Ridden>()
            .insert(Visibility::Inherited);
    }
    belief.spent += AVATAR_COST;
    // Taken mid-stride, and stopped there: the errand, the destination and
    // the path already found to it are all dropped, or the body would finish
    // walking wherever it was going before the god arrived.
    commands.entity(who).insert((
        Ridden {
            left: RIDE_FOR,
            hidden: false,
        },
        Activity::Idle,
        MoveTarget(None),
        crate::creature::Route::default(),
    ));
    follow.entity = Some(who);
    follow.style = FollowStyle::Eyes;
    // The near plane is not touched here. `aim_the_near_plane` owns it and
    // reads `in_a_body` every frame, which is set the moment this follow
    // style lands - two systems writing one plane is how it ended up at a
    // stale value in the first place.
    // Level the look once, on the way in. Overhead the rig is pitched
    // steeply down at the ground; arriving in a body still pitched that way
    // would put the god's first mortal view at their own feet. A little
    // below level, so the first thing seen is the village.
    if let Ok(mut rig) = rigs.single_mut() {
        rig.target_pitch = ARRIVING_PITCH;
        // And turn to face the way THEY are facing. This is the whole of
        // what read as being moved somewhere else: the rig arrives looking
        // along its own orbit bearing, which points from wherever the camera
        // happened to be, through the body, and onward — so the god landed
        // at ground level staring outward at empty country, with the body
        // invisible and not a landmark in sight. Nothing had moved. Only the
        // heading was wrong, and a wrong heading with no reference is
        // indistinguishable from having been carried off.
        //
        // The target alone, so the descent swings round into their bearing
        // instead of snapping to it. A body's model faces -Z, which is the
        // same convention the rig's own forward uses, so its Y rotation IS
        // the yaw.
        let (theirs, _, _) = facing.to_euler(EulerRot::YXZ);
        rig.target_yaw = theirs;
    }
    hold_the_pointer(&mut windows, true);
    selected.0 = None;
    notices.write(crate::ui::Notice::new(
        "You are looking out of somebody else's eyes".to_string(),
    ));
}

/// Takes the mouse pointer away, or gives it back.
///
/// Locked and hidden, the mouse stops being a cursor over a map and becomes
/// the neck: it can turn past the edge of the screen without running out of
/// desk, which is the whole reason first person locks it. There is nothing
/// for it to point at in here anyway — the hand is withdrawn at this range
/// and hovering is switched off with it.
fn hold_the_pointer(windows: &mut Query<&mut CursorOptions, With<PrimaryWindow>>, hold: bool) {
    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };
    cursor.grab_mode = if hold {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !hold;
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
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let dt = time.delta_secs();

    // Nothing worn: make certain everything borrowed has been given back.
    //
    // A possessed villager can be killed by a wolf halfway through a ride,
    // and the loop below never runs for a body that no longer exists — which
    // would leave the mouse locked away with no way to ask for it back, the
    // lens focused four centimetres from the eye, and the camera lying on the
    // grass where the body used to be. Every one of those is recovered here
    // rather than at each of the places a ride can end.
    if ridden.is_empty() {
        let held = windows
            .single()
            .is_ok_and(|cursor| cursor.grab_mode != CursorGrabMode::None);
        if held {
            hold_the_pointer(&mut windows, false);
        }
        if let Ok(mut rig) = rigs.single_mut()
            && rig.target_distance < crate::camera::FIRST_PERSON
        {
            rig.target_distance = LEAVING_HEIGHT;
            rig.target_pitch = rig.target_pitch.max(0.7);
        }
        return;
    }

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
            commands
                .entity(who)
                .remove::<Ridden>()
                .remove::<crate::creature::Sprinting>();
            // Given back visible. If the god leaves while the camera is still
            // inside the skull, this is the only thing that puts them back in
            // the world at all.
            commands.entity(who).insert(Visibility::Inherited);
            hold_the_pointer(&mut windows, false);
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
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    rigs: Query<&CameraRig>,
    mut ridden: Query<
        (
            Entity,
            &Transform,
            &mut MoveTarget,
            &mut Activity,
            &crate::creature::genome::CreatureGenome,
            Option<&crate::creature::Airborne>,
            Option<&crate::creature::Sprinting>,
        ),
        With<Ridden>,
    >,
) {
    use crate::keymap::Deed;
    let Ok(rig) = rigs.single() else {
        return;
    };
    for (who, at, mut target, mut activity, genome, aloft, sprinting) in &mut ridden {
        // Whatever errand they were on, they are not on it now. IDLE, and
        // deliberately not `Wandering` — wandering is not a state of doing
        // nothing, it is the state that goes looking for somewhere to be,
        // and a driven body set to it walked off toward the middle of the
        // settlement on its own. Idle is the one that stands still.
        if *activity != Activity::Idle {
            *activity = Activity::Idle;
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
        let going = (push.length_squared() > 0.0).then(|| push.normalize());
        target.0 = going.map(|way| at.translation + way * STRIDE);

        // Sprinting is a component rather than a number written here, so the
        // walking stays the one authority on how fast anything moves.
        let running = keymap.pressed(&keys, Deed::Sprint);
        if running && sprinting.is_none() {
            commands
                .entity(who)
                .insert(crate::creature::Sprinting(SPRINT));
        } else if !running && sprinting.is_some() {
            commands.entity(who).remove::<crate::creature::Sprinting>();
        }

        // A jump hands the body to the same ballistics that carry a villager
        // the god has thrown: gravity, a tumble, and a landing that is
        // reckoned honestly. Whatever pace they were going at leaves with
        // them, so a running jump carries — and while they are aloft the
        // walking lets go of them, which is what makes it a jump rather than
        // a hop straight up out of a stride.
        // Space, read directly rather than through the keymap: it already
        // belongs to Pause, and the keymap holds every deed to one key of its
        // own. These two never want it at the same moment — inside a body
        // there is nothing worth pausing — so `command_time` stands aside
        // while one is worn and the key means jump.
        if aloft.is_none() && keys.just_pressed(KeyCode::Space) {
            let carry = going.unwrap_or(Vec3::ZERO)
                * genome.walk_speed()
                * if running { SPRINT } else { 1.0 };
            commands.entity(who).insert(crate::creature::Airborne {
                velocity: carry + Vec3::Y * JUMP,
            });
        }
    }
}
