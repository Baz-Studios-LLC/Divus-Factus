//! THE RIG — the body on its pedestal, and the clips it is taught.
//!
//! The bench poses a villager by dragging their limbs, keys the pose at a moment
//! on a timeline, and plays the whole thing back on a loop. What it saves is a
//! CLIP: a handful of moments, each holding a rotation for every joint that
//! moved. Brett's three words for it were "key framing, moving body parts by
//! dragging and looping playback".
//!
//! # Rotations, and nothing else
//!
//! A key holds joint ROTATIONS. Never positions, never lengths. The village is
//! full of bodies that are not this one — a child is two thirds the height of
//! their father and their arms are a different fraction again — and Brett asked
//! for "the ability to test on differnt body types and sizes that appear in the
//! game". A clip of rotations plays correctly on every one of them, because an
//! elbow bent ninety degrees is bent ninety degrees whatever the forearm's
//! length. A clip that stored where the hand ENDED would only ever fit the body
//! it was drawn on.
//!
//! # Whose body it is
//!
//! Not this program's. The bodies come out of the game's own builder, baked to
//! `data/bodies/*.json` by `cargo test bake_the_bodies` over in the game, with
//! the joints named the way a clip names them. The Atelier and the game share no
//! code, so a villager drawn here by hand would be a SECOND villager, and wrong
//! the first time a proportion moved.

use bevy::prelude::*;
use std::collections::BTreeMap;

use crate::Bench;
use crate::look::{Fonts, Palette, theme};

/// The joints a clip may key, in the order a maker meets them.
pub const JOINTS: [&str; 10] = [
    "body",
    "head",
    "arm.l",
    "arm.l.lower",
    "arm.r",
    "arm.r.lower",
    "leg.l",
    "leg.l.lower",
    "leg.r",
    "leg.r.lower",
];

// ------------------------------------------------------------ the body files

/// One joint of a baked body, in its parent's frame.
#[derive(serde::Deserialize, Clone)]
struct JointDef {
    name: String,
    parent: Option<String>,
    at: [f32; 3],
}

/// One drawn box, in the frame of the joint it hangs from.
#[derive(serde::Deserialize, Clone)]
struct BoxDef {
    joint: String,
    at: [f32; 3],
    size: [f32; 3],
    turn: [f32; 4],
    rgb: [u8; 3],
}

/// A whole body as the game baked it.
#[derive(serde::Deserialize, Clone)]
struct BodyFile {
    name: String,
    high: f32,
    joints: Vec<JointDef>,
    boxes: Vec<BoxDef>,
}

/// Every body the bench can stand on the pedestal.
#[derive(Resource, Default)]
struct Bodies(Vec<BodyFile>);

/// Which of them is standing there.
#[derive(Resource, Default)]
struct Wearing(usize);

/// The joints of the body now standing, by name.
#[derive(Resource, Default)]
struct Standing(BTreeMap<String, Entity>);

/// A joint of the body on the pedestal.
#[derive(Component)]
struct RigJoint(String);

/// A box hanging off one, and the joint it answers to.
#[derive(Component)]
struct RigBox(String);

/// Everything belonging to the rig bench, so the whole body can be swept.
///
/// It wears the stage's own `RigFurniture` besides, which is what puts it away
/// when the maker walks to the builder. The pedestal already did that and the
/// body did not, so the villager stood in the middle of the building grid -
/// Brett: "When you are in rig mode and go back to build mode the body is still
/// there."
#[derive(Component)]
struct RigPart;

// ------------------------------------------------------------ the clip

/// One moment of a clip: a time, and the joints that are somewhere at it.
///
/// Only the joints that MOVED. A key that wrote every joint would make every key
/// a whole pose, and two clips could never be laid over one another - which is
/// the whole reason the game can chop with its arms while its legs still walk.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Key {
    at: f32,
    pose: BTreeMap<String, [f32; 4]>,
}

/// The clip on the bench.
#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct Clip {
    #[serde(default)]
    name: Option<String>,
    length: f32,
    #[serde(rename = "loop")]
    looping: bool,
    keys: Vec<Key>,
}

impl Default for Clip {
    fn default() -> Self {
        Clip {
            name: None,
            length: 2.0,
            looping: true,
            keys: Vec::new(),
        }
    }
}

impl Clip {
    /// This joint's first key, and its last, for turning across the seam.
    fn first_key(&self, joint: &str) -> Option<(f32, Quat)> {
        self.keys
            .iter()
            .find_map(|key| key.pose.get(joint).map(|turn| (key.at, Quat::from_array(*turn))))
    }

    fn last_key(&self, joint: &str) -> Option<(f32, Quat)> {
        self.keys
            .iter()
            .rev()
            .find_map(|key| key.pose.get(joint).map(|turn| (key.at, Quat::from_array(*turn))))
    }

    /// The pose at a moment: every joint keyed anywhere in the clip, turned to
    /// where it stands at `t`.
    ///
    /// A joint keyed at some moments and not others holds its nearest key on
    /// either side and turns between them, which is what a maker means by
    /// keying an arm at nought and one: the arm swings for the whole second,
    /// even though the second key is the only other one in the clip.
    fn pose_at(&self, t: f32) -> BTreeMap<String, Quat> {
        let mut posed = BTreeMap::new();
        for joint in JOINTS {
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
            // The seam. A clip keyed at nought and one, two seconds long, holds
            // its last pose for a second and then SNAPS back to the first - which
            // is what Brett saw: "between keyframnes the body should kern from
            // one keyframe to the other not just pop over to it". A looping clip
            // is a circle, so the last key turns toward the first across the end
            // of it, and the loop closes.
            let turn = match (before, after) {
                (Some((a, from)), Some((b, to))) => {
                    if (b - a).abs() < 1e-4 {
                        from
                    } else {
                        from.slerp(to, (t - a) / (b - a))
                    }
                }
                (Some((a, from)), None) if self.looping => match self.first_key(joint) {
                    Some((b, to)) => {
                        let over = (self.length - a + b).max(1e-4);
                        from.slerp(to, ((t - a) / over).clamp(0.0, 1.0))
                    }
                    None => from,
                },
                (None, Some((b, to))) if self.looping => match self.last_key(joint) {
                    Some((a, from)) => {
                        let over = (self.length - a + b).max(1e-4);
                        from.slerp(to, ((t + self.length - a) / over).clamp(0.0, 1.0))
                    }
                    None => to,
                },
                (Some((_, only)), None) | (None, Some((_, only))) => only,
                (None, None) => continue,
            };
            posed.insert(joint.to_string(), turn);
        }
        posed
    }
}

/// Asked to stand the body back up, at the next chance.
#[derive(Resource, Default)]
struct CallToRest(bool);

/// Where the playhead stands, and whether it is running.
#[derive(Resource, Default)]
struct Play {
    running: bool,
    t: f32,
}

/// The joint the maker has hold of, and the drag in progress.
#[derive(Resource, Default)]
struct Held {
    joint: Option<String>,
    /// Which way the held box lies FROM its joint, in the joint's own frame.
    ///
    /// Read off the box that was pressed rather than assumed. A limb hangs on
    /// -Y and a head sits on +Y, so a rule that aimed everything's -Y at the
    /// cursor turned the head to look at the floor the instant it was touched -
    /// Brett: "Simply clicking on the head flips it completly upside down."
    aim: Vec3,
    /// The plane the drag runs on: set once, when the drag begins.
    plane: Option<Vec3>,
}

pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bodies>()
            .init_resource::<Wearing>()
            .init_resource::<Standing>()
            .init_resource::<Clip>()
            .init_resource::<Play>()
            .init_resource::<Held>()
            .init_resource::<CallToRest>()
            .init_resource::<Holding>()
            .add_systems(
                Startup,
                (
                    carry_in_the_bodies,
                    hang_the_clip_bar,
                    hang_the_bodies_shelf,
                    hang_the_top_bar,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    show_the_bar,
                    work_the_bar,
                    work_the_bodies,
                    show_the_shelf,
                    keep_or_open_a_clip,
                    raise_the_body,
                    pose_by_dragging,
                    run_the_clip,
                    hang_the_keys,
                    stand_at_rest,
                    show_the_top_bar,
                    stand_the_camera,
                    work_the_props,
                    hold_the_prop,
                )
                    .chain(),
            );
    }
}

/// Reads every baked body beside the bench.
///
/// The same roads the palette takes, and for the same reason: the bodies are
/// DATA the game handed over, not a maker's own work, so they live beside the
/// program rather than in the folder where saved works go. `bench_home` was the
/// wrong question - it answers "where does this maker keep their things", and a
/// bundled bench keeps those in Application Support, where no body was ever
/// written.
fn carry_in_the_bodies(mut bodies: ResMut<Bodies>, mut wearing: ResMut<Wearing>) {
    let mut roads: Vec<std::path::PathBuf> = Vec::new();
    // Beside the program FIRST, which is where a bundle keeps it. A bench
    // launched from the game's title screen has a working directory of `/`, and
    // a source tree that happens to exist on the machine is not the copy that
    // was shipped.
    if let Ok(exe) = std::env::current_exe()
        && let Some(beside) = exe.parent()
    {
        roads.push(beside.join("data/bodies"));
        roads.push(beside.join("../Resources/data/bodies"));
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        roads.push(std::path::PathBuf::from(manifest).join("data/bodies"));
    }
    roads.push("data/bodies".into());
    roads.push("atelier/data/bodies".into());
    roads.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/bodies"));
    let Some((home, entries)) = roads
        .into_iter()
        .find_map(|road| std::fs::read_dir(&road).ok().map(|entries| (road, entries)))
    else {
        warn!("no bodies anywhere the bench looked");
        return;
    };
    info!("bodies from {}", home.display());
    let mut carried: Vec<BodyFile> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "json"))
        .filter_map(|path| {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<BodyFile>(&text).ok())
        })
        .collect();
    // Smallest first, which is the order a maker thinks of them in.
    carried.sort_by(|a, b| a.high.total_cmp(&b.high));
    // But the bench opens on the grown man, because a clip drawn on him is the
    // one a maker means. Brett: "Default is adult man and this position and zoom
    // level." The others are for trying it afterwards.
    if let Some(grown) = carried.iter().position(|body| body.name == "adult-man") {
        wearing.0 = grown;
    }
    for body in &carried {
        info!("carried in {}: {:.2}m", body.name, body.high);
    }
    bodies.0 = carried;
}

/// Stands the chosen body on the pedestal, once, and again whenever it changes.
#[allow(clippy::too_many_arguments)]
fn raise_the_body(
    mut commands: Commands,
    bench: Res<Bench>,
    bodies: Res<Bodies>,
    wearing: Res<Wearing>,
    mut standing: ResMut<Standing>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    raised: Query<Entity, With<RigPart>>,
    mut hung: Local<Option<usize>>,
) {
    if *bench != Bench::Rig || bodies.0.is_empty() {
        return;
    }
    let which = wearing.0.min(bodies.0.len() - 1);
    if *hung == Some(which) && !raised.is_empty() {
        return;
    }
    *hung = Some(which);
    for part in &raised {
        commands.entity(part).despawn();
    }
    standing.0.clear();

    let body = &bodies.0[which];
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    // The pedestal's own top, so a body stands ON it rather than in it.
    let floor = 0.46;

    // Joints first, parents before children - the file is written in that order
    // and the reader leans on it rather than sorting, since a body with a joint
    // hung on a joint that does not exist yet is not a body.
    for joint in &body.joints {
        let parent = joint
            .parent
            .as_deref()
            .and_then(|name| standing.0.get(name).copied());
        let mut at = Vec3::from(joint.at);
        if parent.is_none() {
            at.y += floor;
        }
        let entity = commands
            .spawn((
                RigPart,
                crate::stage::RigFurniture,
                RigJoint(joint.name.clone()),
                Transform::from_translation(at),
                Visibility::default(),
            ))
            .id();
        if let Some(parent) = parent {
            commands.entity(entity).insert(ChildOf(parent));
        }
        standing.0.insert(joint.name.clone(), entity);
    }

    for slab in &body.boxes {
        let Some(joint) = standing.0.get(&slab.joint).copied() else {
            continue;
        };
        commands.spawn((
            RigPart,
            crate::stage::RigFurniture,
            RigBox(slab.joint.clone()),
            Mesh3d(cube.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb_u8(slab.rgb[0], slab.rgb[1], slab.rgb[2]),
                perceptual_roughness: 0.95,
                reflectance: 0.03,
                ..default()
            })),
            Transform::from_translation(Vec3::from(slab.at))
                .with_rotation(Quat::from_array(slab.turn))
                .with_scale(Vec3::from(slab.size)),
            ChildOf(joint),
        ));
    }
}

// ------------------------------------------------------------ posing

/// Drags a joint round its own hinge.
///
/// The rule is the simplest one that feels like handling a body: the bone AIMS
/// at the cursor. Press on any box, and the joint it hangs from turns so that
/// the box points where the hand is, on the plane through that joint facing the
/// camera. No rings to grab, no axis to choose first - Brett asked for "moving
/// body parts by dragging", and this is that sentence with nothing added.
#[allow(clippy::too_many_arguments)]
fn pose_by_dragging(
    bench: Res<Bench>,
    buttons: Res<ButtonInput<MouseButton>>,
    standing: Res<Standing>,
    mut held: ResMut<Held>,
    mut play: ResMut<Play>,
    windows: Query<&Window>,
    hovers: Query<&Interaction>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    boxes: Query<(&RigBox, &GlobalTransform, &Transform), Without<RigJoint>>,
    mut joints: Query<(&mut Transform, &GlobalTransform), With<RigJoint>>,
) {
    if *bench != Bench::Rig {
        return;
    }
    if buttons.just_released(MouseButton::Left) {
        held.joint = None;
        held.plane = None;
        return;
    }
    let over_ui = hovers
        .iter()
        .any(|interaction| *interaction != Interaction::None);
    let Some(ray) = cursor_ray(&windows, &cameras) else {
        return;
    };

    // Taking hold: the nearest box under the cursor hands over its joint, and
    // the way it lies from that joint.
    if buttons.just_pressed(MouseButton::Left) && !over_ui {
        let mut nearest: Option<(f32, String, Vec3)> = None;
        for (slab, at, local) in &boxes {
            let (scale, turn, middle) = at.to_scale_rotation_translation();
            if let Some(hit) = meets_box(ray.0, ray.1, middle, scale * 0.5, turn)
                && nearest.as_ref().is_none_or(|(had, ..)| hit < *had)
            {
                nearest = Some((hit, slab.0.clone(), local.translation));
            }
        }
        if let Some((_, joint, offset)) = nearest {
            // Posing by hand stops the clock. A body turning under the hand
            // while the maker is trying to put it somewhere is a fight.
            play.running = false;
            held.joint = Some(joint);
            held.aim = offset.normalize_or(Vec3::NEG_Y);
            held.plane = None;
        }
        return;
    }
    if !buttons.pressed(MouseButton::Left) {
        return;
    }
    let Some(joint_name) = held.joint.clone() else {
        return;
    };
    let Some(joint) = standing.0.get(&joint_name).copied() else {
        return;
    };
    let Ok((_, at)) = joints.get(joint) else {
        return;
    };
    let pivot = at.translation();

    // The plane is set ONCE, when the drag starts, and faces the camera. Setting
    // it every frame would let the plane turn with the bone it is turning, and a
    // slow drag would wander round the joint on its own.
    let normal = *held.plane.get_or_insert_with(|| {
        cameras
            .iter()
            .next()
            .map(|(_, eye)| eye.rotation() * Vec3::Z)
            .unwrap_or(Vec3::Z)
    });
    let Some(point) = ray_meets_plane(ray.0, ray.1, pivot, normal) else {
        return;
    };
    // A cursor sitting on the joint itself says nothing about where the bone
    // should point, and asking anyway is how a limb starts to shiver.
    let reach = point - pivot;
    if reach.length() < 0.02 {
        return;
    }

    let Ok((mut local, world)) = joints.get_mut(joint) else {
        return;
    };
    let aim = held.aim;
    let along = (world.rotation() * aim).normalize_or(Vec3::NEG_Y);
    let wanted = reach.normalize();
    // Turning something to face the way it already faces is nothing; turning it
    // to face BACKWARDS has no single answer, and `from_rotation_arc` picks a
    // perpendicular of its own - a different one each frame as the numbers
    // wobble, which is a limb shaking in the maker's hand. Half a turn about the
    // plane the drag is running on is the answer a hand would expect, and it is
    // the same one every frame.
    let facing = along.dot(wanted).clamp(-1.0, 1.0);
    let swing = if facing < -0.9995 {
        Quat::from_axis_angle(normal.normalize_or(Vec3::Z), std::f32::consts::PI)
    } else if facing > 0.9999 {
        return;
    } else {
        Quat::from_rotation_arc(along, wanted)
    };
    // The turn is worked out in the world and worn in the parent's frame, or a
    // shoulder already turned would answer to the wrong axes.
    //
    // And it is NORMALISED before it is worn. A rotation is only a rotation
    // while it is a unit quaternion; drag long enough without saying so and the
    // rounding piles up, the matrix it becomes carries a scale, and the arm the
    // maker is holding swells - "sometimes they shake violently and thats what
    // they warp and change size".
    let parent_turn = world.rotation() * local.rotation.inverse();
    local.rotation = (parent_turn.inverse() * swing * world.rotation()).normalize();
}

/// The cursor's ray into the world: where it starts and where it heads.
fn cursor_ray(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>,
) -> Option<(Vec3, Vec3)> {
    let window = windows.iter().next()?;
    let at = window.cursor_position()?;
    let (camera, eye) = cameras.iter().next()?;
    let ray = camera.viewport_to_world(eye, at).ok()?;
    Some((ray.origin, *ray.direction))
}

/// Where a ray crosses a plane, if it does at all.
fn ray_meets_plane(from: Vec3, along: Vec3, on: Vec3, normal: Vec3) -> Option<Vec3> {
    let slope = along.dot(normal);
    if slope.abs() < 1e-5 {
        return None;
    }
    let reach = (on - from).dot(normal) / slope;
    (reach > 0.0).then(|| from + along * reach)
}

/// Where a ray first meets an oriented box, in the box's own frame.
fn meets_box(from: Vec3, along: Vec3, at: Vec3, half: Vec3, turn: Quat) -> Option<f32> {
    let inverse = turn.inverse();
    let origin = inverse * (from - at);
    let heading = inverse * along;
    let (mut near, mut far) = (f32::NEG_INFINITY, f32::INFINITY);
    for axis in 0..3 {
        let (o, d, h) = (origin[axis], heading[axis], half[axis]);
        if d.abs() < 1e-6 {
            if o.abs() > h {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((-h - o) / d, (h - o) / d);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        near = near.max(t0);
        far = far.min(t1);
        if near > far {
            return None;
        }
    }
    (far >= 0.0).then_some(near.max(0.0))
}

// ------------------------------------------------------------ playback

/// Runs the clock and puts the body where the clip says.
fn run_the_clip(
    bench: Res<Bench>,
    time: Res<Time>,
    clip: Res<Clip>,
    standing: Res<Standing>,
    held: Res<Held>,
    mut play: ResMut<Play>,
    mut joints: Query<(&RigJoint, &mut Transform)>,
) {
    if *bench != Bench::Rig {
        return;
    }
    if play.running {
        play.t += time.delta_secs();
        if play.t >= clip.length {
            if clip.looping {
                play.t %= clip.length.max(0.01);
            } else {
                play.t = clip.length;
                play.running = false;
            }
        }
    }
    // A hand on a joint owns it: the clock may move every other joint, but the
    // one being posed answers to the hand until it lets go.
    if !play.running && held.joint.is_none() && !play.is_changed() {
        return;
    }
    let posed = clip.pose_at(play.t);
    for (joint, mut at) in &mut joints {
        if held.joint.as_deref() == Some(joint.0.as_str()) {
            continue;
        }
        if let Some(turn) = posed.get(&joint.0) {
            at.rotation = *turn;
        }
    }
    let _ = &standing;
}

// ------------------------------------------------------------ the clip bar

/// The row along the foot: the clock, the keys, and what may be done to them.
#[derive(Component)]
struct ClipBar;

/// The track a key is drawn on, and the playhead runs along.
#[derive(Component)]
struct ClipTrack;

/// A key's own tick on the track.
#[derive(Component)]
struct KeyTick;

/// A mark of the clock along the track, so a maker can see where a moment falls
/// rather than counting pixels. Brett: "The scrub bar needs quarter tick marks."
#[derive(Component)]
struct Grade;

/// How close two marks may stand before the row of them reads as a smear.
const GRADE_ROOM: f32 = 8.0;

/// The track's own width, which the marks are spaced against.
const TRACK_WIDTH: f32 = 420.0;

/// The playhead.
#[derive(Component)]
struct Playhead;

/// One of the bar's buttons.
#[derive(Component, Clone, Copy, PartialEq)]
enum ClipDeed {
    Play,
    Loop,
    SetKey,
    DropKey,
    Shorter,
    Longer,
    /// Everything back where the body was baked standing.
    Rest,
}

/// A button that stands a different body on the pedestal.
#[derive(Component)]
struct BodyButton(usize);

fn hang_the_clip_bar(mut commands: Commands, fonts: Res<Fonts>, palette: Res<Palette>) {
    let bar = commands
        .spawn((
            ClipBar,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect {
                    left: Val::Px(1.0),
                    right: Val::Px(1.0),
                    top: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
            ChildOf(bar),
        ))
        .id();

    let mut button = |deed: ClipDeed, label: &str, wide: f32| {
        let button = commands
            .spawn((
                deed,
                Interaction::default(),
                Node {
                    width: Val::Px(wide),
                    padding: UiRect::vertical(Val::Px(5.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.30)),
                BorderColor::all(theme::panel_border(&palette)),
                ChildOf(panel),
            ))
            .id();
        commands.spawn((
            Text::new(label),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(theme::accent(&palette)),
            ChildOf(button),
        ));
    };
    button(ClipDeed::Play, "PLAY", 72.0);
    button(ClipDeed::Loop, "LOOP", 72.0);

    // The track itself: a strip a maker scrubs, with the keys drawn on it.
    commands.spawn((
        ClipTrack,
        Interaction::default(),
        Node {
            width: Val::Px(TRACK_WIDTH),
            height: Val::Px(26.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.45)),
        BorderColor::all(theme::panel_border(&palette)),
        ChildOf(panel),
    ));

    let mut button = |deed: ClipDeed, label: &str, wide: f32| {
        let button = commands
            .spawn((
                deed,
                Interaction::default(),
                Node {
                    width: Val::Px(wide),
                    padding: UiRect::vertical(Val::Px(5.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.30)),
                BorderColor::all(theme::panel_border(&palette)),
                ChildOf(panel),
            ))
            .id();
        commands.spawn((
            Text::new(label),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(theme::accent(&palette)),
            ChildOf(button),
        ));
    };
    button(ClipDeed::SetKey, "SET KEY", 86.0);
    button(ClipDeed::DropKey, "DROP KEY", 92.0);
    button(ClipDeed::Shorter, "-", 34.0);
    button(ClipDeed::Longer, "+", 34.0);
}

/// The bar belongs to the rig bench, and the builder's belongs to the builder.
fn show_the_bar(
    bench: Res<Bench>,
    mut bars: Query<&mut Visibility, With<ClipBar>>,
    mut stages: Query<&mut Visibility, (With<crate::rail::StageBar>, Without<ClipBar>)>,
) {
    if !bench.is_changed() {
        return;
    }
    for mut showing in &mut bars {
        *showing = if *bench == Bench::Rig {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut showing in &mut stages {
        *showing = if *bench == Bench::Builder {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Carries out whatever the bar was asked for.
#[allow(clippy::too_many_arguments)]
fn work_the_bar(
    bench: Res<Bench>,
    mut rest: ResMut<CallToRest>,
    windows: Query<&Window>,
    mut clip: ResMut<Clip>,
    mut play: ResMut<Play>,
    joints: Query<(&RigJoint, &Transform)>,
    deeds: Query<(&Interaction, &ClipDeed), Changed<Interaction>>,
    track: Query<(&Interaction, &ComputedNode, &GlobalTransform), With<ClipTrack>>,
) {
    if *bench != Bench::Rig {
        return;
    }
    for (touch, deed) in &deeds {
        if *touch != Interaction::Pressed {
            continue;
        }
        match deed {
            ClipDeed::Play => {
                play.running = !play.running;
                if play.running && play.t >= clip.length {
                    play.t = 0.0;
                }
            }
            ClipDeed::Loop => clip.looping = !clip.looping,
            ClipDeed::SetKey => {
                // Every joint, as it stands. A pose is what the maker is
                // looking at, and looking at it is how they decided to key it.
                let mut pose = BTreeMap::new();
                for (joint, at) in &joints {
                    pose.insert(joint.0.clone(), at.rotation.to_array());
                }
                let at = play.t;
                match clip.keys.iter().position(|key| (key.at - at).abs() < 0.02) {
                    Some(standing) => clip.keys[standing].pose = pose,
                    None => {
                        clip.keys.push(Key { at, pose });
                        clip.keys.sort_by(|a, b| a.at.total_cmp(&b.at));
                    }
                }
            }
            ClipDeed::DropKey => {
                let at = play.t;
                if let Some(nearest) = clip
                    .keys
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| (a.at - at).abs().total_cmp(&(b.at - at).abs()))
                    .map(|(index, _)| index)
                {
                    clip.keys.remove(nearest);
                }
            }
            ClipDeed::Shorter => {
                clip.length = (clip.length - 0.25).max(0.25);
                play.t = play.t.min(clip.length);
            }
            ClipDeed::Longer => clip.length = (clip.length + 0.25).min(20.0),
            ClipDeed::Rest => {
                // The pose the body was baked standing in, which for every joint
                // is no turn at all. It does not touch the keys: a maker who has
                // posed themselves into a corner wants the body back, not their
                // work undone.
                play.running = false;
                rest.0 = true;
            }
        }
    }

    // Scrubbing: a press anywhere on the track puts the playhead there.
    let Some(at) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    for (touch, node, place) in &track {
        if *touch != Interaction::Pressed {
            continue;
        }
        // A node measures itself in PHYSICAL pixels and the cursor reports
        // LOGICAL ones, so the node's own inverse scale carries it across.
        // Dividing by it instead - which is what this did - squares the screen's
        // scale into the answer, and on a retina bench every press landed at
        // four times the distance along, which is to say off the end.
        let scale = node.inverse_scale_factor();
        let width = node.size().x * scale;
        let middle = place.translation().x * scale;
        let along = ((at.x - (middle - width * 0.5)) / width.max(1.0)).clamp(0.0, 1.0);
        play.running = false;
        play.t = along * clip.length;
    }
}

/// Draws the keys and the playhead on the track.
fn hang_the_keys(
    mut commands: Commands,
    bench: Res<Bench>,
    palette: Res<Palette>,
    clip: Res<Clip>,
    play: Res<Play>,
    track: Query<Entity, With<ClipTrack>>,
    ticks: Query<Entity, Or<(With<KeyTick>, With<Playhead>, With<Grade>)>>,
    mut heads: Query<&mut Node, With<Playhead>>,
) {
    if *bench != Bench::Rig {
        return;
    }
    let Ok(track) = track.single() else {
        return;
    };
    // The playhead moves every frame; the keys only when they change.
    if !clip.is_changed() && !ticks.is_empty() {
        for mut node in &mut heads {
            node.left = Val::Percent((play.t / clip.length.max(0.01)) * 100.0);
        }
        return;
    }
    for tick in &ticks {
        commands.entity(tick).despawn();
    }

    // The clock first, under everything: quarter seconds while they have room to
    // stand apart, whole seconds when they have not, and failing both the
    // quarters of the clip itself - which is the one spacing that cannot crowd,
    // however long the clip runs.
    let length = clip.length.max(0.01);
    let step = [0.25_f32, 1.0]
        .into_iter()
        .find(|step| (step / length) * TRACK_WIDTH >= GRADE_ROOM)
        .unwrap_or(length * 0.25);
    let mut at = step;
    while at < length - 1e-3 {
        // A mark on the second stands taller than the quarters between them, so
        // the eye can count seconds without reading any of them.
        let whole = (at / 1.0).fract() < 1e-3 || (at / 1.0).fract() > 1.0 - 1e-3;
        let tall = if whole { 0.55 } else { 0.30 };
        commands.spawn((
            Grade,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent((at / length) * 100.0),
                bottom: Val::Px(0.0),
                width: Val::Px(1.0),
                height: Val::Percent(tall * 100.0),
                ..default()
            },
            BackgroundColor(theme::text_dim(&palette).with_alpha(if whole {
                0.55
            } else {
                0.32
            })),
            ChildOf(track),
        ));
        at += step;
    }

    for key in &clip.keys {
        commands.spawn((
            KeyTick,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent((key.at / clip.length.max(0.01)) * 100.0),
                top: Val::Px(0.0),
                width: Val::Px(3.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme::accent(&palette)),
            ChildOf(track),
        ));
    }
    commands.spawn((
        Playhead,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent((play.t / clip.length.max(0.01)) * 100.0),
            top: Val::Px(0.0),
            width: Val::Px(1.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(theme::text_dim(&palette)),
        ChildOf(track),
    ));
}

/// The shelf of bodies, on the rail where the builder keeps its parts.
#[derive(Component)]
struct BodyShelf;

/// Hangs one button per baked body.
fn hang_the_bodies_shelf(
    mut commands: Commands,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
    bodies: Res<Bodies>,
) {
    let shelf = commands
        .spawn((
            BodyShelf,
            // The builder's shelf exactly: same edge, same width, same border on
            // the one side it touches. Brett: "I like the design for he whole app
            // with the shelf on the left and right", and a panel that merely
            // floated near the right edge was a different thing that happened to
            // be over there. What goes on it will grow - the bodies are only the
            // first tenants.
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(212.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::left(Val::Px(1.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
            crate::look::Scrollable,
            bevy::ui::ScrollPosition::default(),
            Visibility::Hidden,
        ))
        .id();
    let bodies_drawer = crate::builder::drawer(&mut commands, &fonts, &palette, shelf, "THE BODY", true);
    for (index, body) in bodies.0.iter().enumerate() {
        let button = commands
            .spawn((
                BodyButton(index),
                Interaction::default(),
                Node {
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.18)),
                BorderColor::all(theme::panel_border(&palette)),
                ChildOf(bodies_drawer),
            ))
            .id();
        commands.spawn((
            Text::new(format!(
                "{}  -  {:.2}M",
                body.name.to_uppercase(),
                body.high
            )),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(theme::text_dim(&palette)),
            ChildOf(button),
        ));
    }

    // What the hands can hold. A pose is read off what the body is DOING, and an
    // arm swinging nothing is an arm swinging nothing - Brett: "props that I can
    // have them hold and attache to their hands. A fishing pole, an axe, a mining
    // pick, sword etc."
    let props = crate::builder::drawer(&mut commands, &fonts, &palette, shelf, "PROPS", true);
    let mut prop_button = |what: &'static str, label: &'static str| {
        let button = commands
            .spawn((
                PropButton(what),
                Interaction::default(),
                Node {
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.18)),
                BorderColor::all(theme::panel_border(&palette)),
                ChildOf(props),
            ))
            .id();
        commands.spawn((
            Text::new(label),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(theme::text_dim(&palette)),
            ChildOf(button),
        ));
        commands.spawn((
            crate::rail::Word("Put it in the hand, or take it out again"),
            ChildOf(button),
        ));
    };
    for (what, label) in PROPS {
        prop_button(what, label);
    }
    let swap = commands
        .spawn((
            SwapHands,
            Interaction::default(),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::accent(&palette).with_alpha(0.5)),
            ChildOf(props),
        ))
        .id();
    commands.spawn((
        Text::new("THE OTHER HAND"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(theme::accent(&palette)),
        ChildOf(swap),
    ));
}

/// The props a hand can hold, and the words on their buttons.
const PROPS: [(&str, &str); 6] = [
    ("axe", "AXE"),
    ("pick", "MINING PICK"),
    ("sword", "SWORD"),
    ("rod", "FISHING POLE"),
    ("hoe", "HOE"),
    ("torch", "TORCH"),
];

/// A prop's boxes, in the grip's own frame: the handle runs down -Y from the
/// hand, the way a tool hangs when an arm is at rest, and the working end is
/// furthest from the palm.
fn prop_body(what: &str) -> Vec<(Vec3, Vec3, &'static str, f32)> {
    // (middle, size, ramp, shade)
    match what {
        "axe" => vec![
            (Vec3::new(0.0, -0.28, 0.0), Vec3::new(0.04, 0.62, 0.04), "wood", 0.45),
            (Vec3::new(0.0, -0.56, 0.0), Vec3::new(0.16, 0.13, 0.04), "stone", 0.75),
            (Vec3::new(0.09, -0.56, 0.0), Vec3::new(0.06, 0.07, 0.05), "stone", 0.9),
        ],
        "pick" => vec![
            (Vec3::new(0.0, -0.30, 0.0), Vec3::new(0.04, 0.66, 0.04), "wood", 0.4),
            (Vec3::new(0.0, -0.60, 0.0), Vec3::new(0.34, 0.05, 0.05), "stone", 0.8),
            (Vec3::new(0.15, -0.57, 0.0), Vec3::new(0.06, 0.05, 0.05), "stone", 0.95),
            (Vec3::new(-0.15, -0.57, 0.0), Vec3::new(0.06, 0.05, 0.05), "stone", 0.95),
        ],
        "sword" => vec![
            (Vec3::new(0.0, -0.09, 0.0), Vec3::new(0.04, 0.18, 0.04), "wood", 0.35),
            (Vec3::new(0.0, -0.19, 0.0), Vec3::new(0.18, 0.04, 0.05), "bone", 0.75),
            (Vec3::new(0.0, -0.55, 0.0), Vec3::new(0.06, 0.70, 0.02), "bone", 0.95),
        ],
        "rod" => vec![
            (Vec3::new(0.0, -0.14, 0.0), Vec3::new(0.04, 0.28, 0.04), "wood", 0.3),
            (Vec3::new(0.0, -0.90, 0.0), Vec3::new(0.025, 1.30, 0.025), "wood", 0.55),
            // The line, hanging from the tip. A pole with no line is a stick.
            (Vec3::new(0.0, -1.85, 0.0), Vec3::new(0.008, 0.62, 0.008), "bone", 0.9),
        ],
        "hoe" => vec![
            (Vec3::new(0.0, -0.34, 0.0), Vec3::new(0.04, 0.72, 0.04), "wood", 0.45),
            (Vec3::new(0.06, -0.70, 0.0), Vec3::new(0.16, 0.05, 0.10), "stone", 0.7),
        ],
        "torch" => vec![
            (Vec3::new(0.0, -0.22, 0.0), Vec3::new(0.05, 0.46, 0.05), "wood", 0.35),
            (Vec3::new(0.0, -0.50, 0.0), Vec3::new(0.09, 0.12, 0.09), "cloth-rust", 0.85),
        ],
        _ => Vec::new(),
    }
}

/// What the body is holding, and in which hand.
#[derive(Resource)]
struct Holding {
    prop: Option<&'static str>,
    /// The forearm the prop hangs from.
    hand: &'static str,
}

impl Default for Holding {
    fn default() -> Self {
        Holding {
            prop: None,
            hand: "arm.r.lower",
        }
    }
}

/// A button that puts a prop in the hand, or takes it out.
#[derive(Component)]
struct PropButton(&'static str);

/// A button that moves whatever is held to the other hand.
#[derive(Component)]
struct SwapHands;

/// A prop's own boxes, so they can be swept when it changes.
#[derive(Component)]
struct PropPart;

/// Puts the held prop in the hand, and takes the old one away.
///
/// It hangs off the FOREARM, at the far end of the forearm's own box, which is
/// where a hand would be if the body had one. The bodies are boxes and the last
/// box of an arm IS the hand as far as anything here is concerned.
fn hold_the_prop(
    mut commands: Commands,
    bench: Res<Bench>,
    palette: Res<Palette>,
    bodies: Res<Bodies>,
    wearing: Res<Wearing>,
    holding: Res<Holding>,
    standing: Res<Standing>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    held: Query<Entity, With<PropPart>>,
) {
    if *bench != Bench::Rig {
        return;
    }
    if !holding.is_changed() && !standing.is_changed() {
        return;
    }
    for part in &held {
        commands.entity(part).despawn();
    }
    let Some(what) = holding.prop else {
        return;
    };
    let Some(hand) = standing.0.get(holding.hand).copied() else {
        return;
    };
    // The far end of the forearm, read off the body that is standing rather
    // than guessed: a child's forearm is not their father's.
    let palm = bodies
        .0
        .get(wearing.0.min(bodies.0.len().saturating_sub(1)))
        .and_then(|body| {
            body.boxes
                .iter()
                .find(|slab| slab.joint == holding.hand)
                .map(|slab| slab.at[1] - slab.size[1] * 0.5)
        })
        .unwrap_or(-0.25);

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for (at, size, ramp, shade) in prop_body(what) {
        commands.spawn((
            PropPart,
            RigPart,
            crate::stage::RigFurniture,
            Mesh3d(cube.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: palette.shade(ramp, shade),
                perceptual_roughness: 0.95,
                reflectance: 0.03,
                ..default()
            })),
            Transform::from_translation(at + Vec3::new(0.0, palm, 0.0)).with_scale(size),
            ChildOf(hand),
        ));
    }
}

/// The prop buttons, and the one that changes hands.
fn work_the_props(
    bench: Res<Bench>,
    mut holding: ResMut<Holding>,
    props: Query<(&Interaction, &PropButton), Changed<Interaction>>,
    swaps: Query<&Interaction, (Changed<Interaction>, With<SwapHands>)>,
) {
    if *bench != Bench::Rig {
        return;
    }
    for (touch, prop) in &props {
        if *touch != Interaction::Pressed {
            continue;
        }
        // The same prop again is the maker putting it down.
        holding.prop = if holding.prop == Some(prop.0) {
            None
        } else {
            Some(prop.0)
        };
    }
    if swaps.iter().any(|touch| *touch == Interaction::Pressed) {
        holding.hand = if holding.hand == "arm.r.lower" {
            "arm.l.lower"
        } else {
            "arm.r.lower"
        };
    }
}

/// The body shelf belongs to the rig bench.
fn show_the_shelf(bench: Res<Bench>, mut shelves: Query<&mut Visibility, With<BodyShelf>>) {
    if !bench.is_changed() {
        return;
    }
    for mut showing in &mut shelves {
        *showing = if *bench == Bench::Rig {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// What a clip looks like on disk.
///
/// The same `.baz` a building wears, with a word inside saying which it is - so
/// one picker opens either and the bench walks to the right place to show it.
#[derive(serde::Serialize, serde::Deserialize)]
struct ClipFile {
    format: u32,
    kind: String,
    name: String,
    length: f32,
    #[serde(rename = "loop")]
    looping: bool,
    keys: Vec<Key>,
}

/// The save and open glyphs, when the maker is standing at this bench.
///
/// The dialog names the clip and finds it a home in one gesture, which is why
/// there is no naming card here: the builder's card was written when the only
/// way to reach the disk was through the bench's own folder.
fn keep_or_open_a_clip(
    _main_thread: bevy::ecs::system::NonSendMarker,
    bench: Res<Bench>,
    mut clip: ResMut<Clip>,
    mut play: ResMut<Play>,
    saves: Query<&Interaction, (Changed<Interaction>, With<crate::builder::SaveButton>)>,
    opens: Query<&Interaction, (Changed<Interaction>, With<crate::builder::OpenWorkButton>)>,
    sweeps: Query<&Interaction, (Changed<Interaction>, With<crate::builder::ClearButton>)>,
) {
    if *bench != Bench::Rig {
        return;
    }
    let pressed = |touch: &Interaction| *touch == Interaction::Pressed;
    let home = crate::builder::bench_home().join("out/clips");

    if sweeps.iter().any(pressed) {
        clip.keys.clear();
        play.t = 0.0;
        play.running = false;
    }

    if saves.iter().any(pressed) {
        let _ = std::fs::create_dir_all(&home);
        let called = clip.name.clone().unwrap_or_else(|| "clip".to_string());
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Keep the clip")
            .add_filter("Divus Factus clips", &["baz"])
            .set_directory(&home)
            .set_file_name(format!("{called}.baz"))
            .save_file()
        {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| called.clone());
            let file = ClipFile {
                format: 1,
                kind: "clip".to_string(),
                name: name.clone(),
                length: clip.length,
                looping: clip.looping,
                keys: clip.keys.clone(),
            };
            match serde_json::to_string_pretty(&file) {
                Ok(text) => match std::fs::write(&path, text) {
                    Ok(()) => {
                        clip.name = Some(name);
                        info!("kept {} keys at {}", clip.keys.len(), path.display());
                    }
                    Err(why) => warn!("could not write {}: {why}", path.display()),
                },
                Err(why) => warn!("could not write the clip: {why}"),
            }
        }
    }

    if opens.iter().any(pressed) {
        let _ = std::fs::create_dir_all(&home);
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open a clip")
            .add_filter("Divus Factus clips", &["baz"])
            .set_directory(&home)
            .pick_file()
        {
            match std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<ClipFile>(&text).ok())
            {
                Some(file) if file.kind == "clip" => {
                    clip.name = Some(file.name);
                    clip.length = file.length.max(0.25);
                    clip.looping = file.looping;
                    clip.keys = file.keys;
                    play.t = 0.0;
                    play.running = false;
                    info!("opened a clip of {} keys", clip.keys.len());
                }
                _ => warn!("{} is not a clip", path.display()),
            }
        }
    }
}

/// Presses on the body buttons stand a different villager on the pedestal.
fn work_the_bodies(
    bench: Res<Bench>,
    mut wearing: ResMut<Wearing>,
    buttons: Query<(&Interaction, &BodyButton), Changed<Interaction>>,
) {
    if *bench != Bench::Rig {
        return;
    }
    for (touch, body) in &buttons {
        if *touch == Interaction::Pressed {
            wearing.0 = body.0;
        }
    }
}

/// The rig bench's own row along the top.
#[derive(Component)]
struct RigBar;

fn hang_the_top_bar(mut commands: Commands, fonts: Res<Fonts>, palette: Res<Palette>) {
    let centring = commands
        .spawn((
            RigBar,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    let bar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect {
                    left: Val::Px(1.0),
                    right: Val::Px(1.0),
                    bottom: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
            ChildOf(centring),
        ))
        .id();
    let button = commands
        .spawn((
            ClipDeed::Rest,
            Interaction::default(),
            Node {
                width: Val::Px(88.0),
                padding: UiRect::vertical(Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::panel_border(&palette)),
            ChildOf(bar),
        ))
        .id();
    commands.spawn((
        Text::new("RESET"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(theme::accent(&palette)),
        ChildOf(button),
    ));
    commands.spawn((
        crate::rail::Word("Reset the body to the pose it was baked in"),
        ChildOf(button),
    ));
}

/// The eye the rig bench opens at: face on, a little above, close enough that a
/// hand can reach a wrist.
///
/// Set every time the maker walks TO the bench rather than once at startup, so
/// coming back from the builder is coming back to the same view - which is what
/// makes it a bench rather than wherever the camera happened to be left.
fn stand_the_camera(bench: Res<Bench>, mut eye: ResMut<crate::camera::OrbitRig>) {
    if !bench.is_changed() || *bench != Bench::Rig {
        return;
    }
    eye.focus = Vec3::new(0.0, 1.0, 0.0);
    // Face on, which is NOT the builder's zero. A body's arms hang off its X, so
    // the eye at zero looks straight down the shoulder line and sees a silhouette
    // - Brett: "The view is starting on the side not the front." The face looks
    // along -Z, which the beard and the eyes both say plainly enough in the
    // game's own builder, so the eye stands there and looks back.
    eye.yaw = -std::f32::consts::FRAC_PI_2;
    eye.pitch = 0.30;
    eye.distance = 4.6;
}

/// Each bench wears its own top bar.
fn show_the_top_bar(
    bench: Res<Bench>,
    mut rigs: Query<&mut Visibility, With<RigBar>>,
    mut modes: Query<&mut Visibility, (With<crate::rail::ModeBar>, Without<RigBar>)>,
) {
    if !bench.is_changed() {
        return;
    }
    for mut showing in &mut rigs {
        *showing = if *bench == Bench::Rig {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut showing in &mut modes {
        *showing = if *bench == Bench::Builder {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Stands every joint back up where it was baked.
fn stand_at_rest(mut rest: ResMut<CallToRest>, mut joints: Query<&mut Transform, With<RigJoint>>) {
    if !rest.0 {
        return;
    }
    rest.0 = false;
    for mut at in &mut joints {
        at.rotation = Quat::IDENTITY;
    }
}
