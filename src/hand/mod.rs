//! The Divine Hand.
//!
//! This is the game's primary verb and its primary input device. Everything the
//! player will eventually do to steer what villagers believe — moving a storyteller
//! to where a miracle is about to happen, carrying a witness to the next settlement —
//! runs through picking things up and putting them down. So it has to feel good
//! before anything built on top of it can.
//!
//! Two decisions do most of that work:
//!
//! - Held objects lag behind the cursor on a spring rather than tracking it rigidly.
//!   Rigid tracking feels like dragging an icon; lag feels like carrying something.
//! - Throw velocity comes from the *hand's* recent motion, not the object's, so a
//!   flick throws hard even though the object was still catching up.
//!
//! Picking marches the terrain function and tests bounding spheres, rather than
//! raycasting meshes. The ground is millions of streamed triangles; the terrain
//! function answers the same question analytically and is always available, even
//! where no chunk has been built.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::f32::consts::FRAC_PI_2;

use crate::camera::{CameraRig, CameraSet, GodCamera};
use crate::creature::anim::CreatureMotion;
use crate::creature::{Airborne, Held, MoveTarget};
use crate::palette;
use crate::render::HAND_LAYER;
use crate::terrain::{self, Terrain, WATER_LEVEL};
use crate::ui::PointerContext;
use crate::witness::{DivineEvent, DivineEventKind};

/// How many frames of hand motion feed the throw velocity estimate.
const VELOCITY_SAMPLES: usize = 6;

/// Multiplier turning hand speed into launch speed. Tuned so a deliberate flick
/// sends a villager a satisfying distance without launching them off the slab.
const THROW_STRENGTH: f32 = 1.35;

/// Speed below which a release is a drop rather than a throw.
const THROW_THRESHOLD: f32 = 2.5;

/// Inside this camera distance the hand is not drawn at all. Well below the
/// twelve metres ordinary play is clamped to, so only a first-person view -
/// Avatar - ever reaches it.
const WITHDRAW_WITHIN: f32 = 6.0;

/// How far in front of the camera the hand floats while it is the UI cursor.
///
/// Close enough to clear every piece of world geometry, far enough past the near
/// plane (0.5) that nothing clips.
const UI_CURSOR_DEPTH: f32 = 5.0;

/// Hand scale while over the interface. At [`UI_CURSOR_DEPTH`] this reads as a
/// normal cursor: present, not looming.
const UI_CURSOR_SCALE: f32 = 0.16;

/// How fast a held object converges on the hand. Low enough to see the lag.
const HOLD_SPRING: f32 = 14.0;

/// Which of the palette's ramps the hand is carved from.
///
/// A setting, not a genome: the hand is the player's one embodied choice, so
/// its colour is theirs to pick. Everything still comes off the master
/// palette — a god is of this world's colours like everything else in it.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandStyle {
    pub ramp: &'static crate::palette::Ramp,
}

impl Default for HandStyle {
    fn default() -> Self {
        HandStyle {
            ramp: &crate::palette::BONE,
        }
    }
}

/// The choices offered in settings, in display order.
pub const HAND_STYLES: &[(&str, &crate::palette::Ramp)] = &[
    ("alabaster", &crate::palette::BONE),
    ("gilded", &crate::palette::CLOTH_GOLD),
    ("granite", &crate::palette::STONE),
    ("pale", &crate::palette::SKIN_PALE),
    ("tan", &crate::palette::SKIN_MID),
    ("deep", &crate::palette::SKIN_DEEP),
    ("wrathful", &crate::palette::CLOTH_RED),
    ("verdant", &crate::palette::GRASS),
];

/// Handles to the hand's two materials, for restyling live.
#[derive(Resource)]
pub struct HandMaterials {
    pub skin: Handle<StandardMaterial>,
    pub knuckle: Handle<StandardMaterial>,
}

/// Repaints the hand whenever the style changes — including live on the title
/// screen, where the pointing hand is its own preview.
fn apply_hand_style(
    style: Res<HandStyle>,
    handles: Option<Res<HandMaterials>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(handles) = handles else {
        return;
    };
    if let Some(mut skin) = materials.get_mut(&handles.skin) {
        skin.base_color = palette::shade(style.ramp, 1.0);
        skin.emissive = LinearRgba::from(palette::shade(style.ramp, 0.9)) * 0.25;
    }
    if let Some(mut knuckle) = materials.get_mut(&handles.knuckle) {
        knuckle.base_color = palette::shade(style.ramp, 0.72);
        knuckle.emissive = LinearRgba::from(palette::shade(style.ramp, 0.9)) * 0.16;
    }
}

pub struct HandPlugin;

impl Plugin for HandPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DivineHand>()
            .init_resource::<HandStyle>()
            .add_systems(
                Update,
                apply_hand_style.run_if(resource_changed::<HandStyle>),
            )
            .add_systems(Startup, spawn_hand_cursor)
            .add_systems(Update, breathe_hand_glow)
            .add_systems(
                Update,
                (
                    update_hand_ray,
                    update_hover,
                    toggle_follow,
                    handle_grab_and_release,
                    carry_held_object,
                    fade_divine_mark,
                    animate_hand,
                )
                    .chain()
                    .after(CameraSet),
            );
    }
}

/// Radius used for cursor picking. Set at spawn by whatever creates the object.
#[derive(Component)]
pub struct PickRadius(pub f32);

/// Freshly set down by the hand of god.
///
/// The mark fades in under half a minute. Its whole purpose is answered
/// prayers: food that carries this mark, arriving beside someone mid-prayer,
/// is not luck — it is providence, and the faith systems treat it so.
#[derive(Component, Debug)]
pub struct DivinelyPlaced {
    pub remaining: f32,
}

/// The mark of the hand fades.
pub fn fade_divine_mark(
    mut commands: Commands,
    time: Res<Time>,
    mut marked: Query<(Entity, &mut DivinelyPlaced)>,
) {
    for (entity, mut mark) in &mut marked {
        mark.remaining -= time.delta_secs();
        if mark.remaining <= 0.0 {
            commands.entity(entity).remove::<DivinelyPlaced>();
        }
    }
}

/// Pickable but not liftable: the hand may hover it — the inspector opens, the
/// hand flexes — but never close around it. Town banners are of the earth.
#[derive(Component)]
pub struct Rooted;

/// Marks the visible hand.
#[derive(Component)]
struct HandModel;

/// The hand's joints, and its one piece of animation state.
#[derive(Component)]
struct HandRig {
    /// `[knuckle, mid-joint]` per finger, index to little.
    fingers: Vec<[Entity; 2]>,
    /// `[base, tip]`.
    thumb: [Entity; 2],
    /// How closed the hand is, 0 open to 1 gripping. Eased toward the pose.
    grip: f32,
    /// Smoothed banking from the hand's own motion, `(roll, pitch)`.
    bank: Vec2,
    /// How far into UI-cursor mode the hand is, 0 over the world to 1 over a
    /// panel. Eased, so crossing a panel edge is a gesture rather than a swap.
    point: f32,
    /// Click depression while pointing, 0 raised to 1 pressed. Fast attack, slow
    /// release: the tap lands sharply and lifts like a finger, not a switch.
    tap: f32,
    /// Presence, 0 gone to 1 fully here. The hand withdraws over the shoulder
    /// camera — a god watching through mortal eyes keeps its hands to itself —
    /// and it fades rather than pops.
    fade: f32,
    /// How far into the flag-carrying pose the hand is, 0 to 1. Eased, so the
    /// hand closes around the pole and turns on the way in rather than
    /// snapping into a fist the instant the founding begins.
    carry: f32,
}

/// The roll that turns the flat palm into a fist held sideways, as a hand
/// holds a pole: knuckles to the camera, fingers wrapped round a vertical
/// shaft rather than laid out over the ground.
const CARRY_ROLL: f32 = -FRAC_PI_2;

/// And a little tip forward with it, so the fist reads as bearing weight
/// rather than merely being turned over.
const CARRY_PITCH: f32 = 0.35;

/// Grip closure and hover height for each state of the hand.
///
/// One scalar drives every joint. The poses only need to be distinguishable —
/// open drifting over the world, flexed and ready above something grabbable,
/// closed around a carry.
fn pose(held: bool, hovering: bool) -> (f32, f32) {
    if held {
        (1.0, 1.1)
    } else if hovering {
        (0.45, 1.8)
    } else {
        (0.1, 2.6)
    }
}

/// What the hand is currently holding.
pub struct HeldObject {
    pub entity: Entity,
    /// How far above the ground the object is being carried.
    pub hold_height: f32,
    /// Recent hand positions, for estimating throw velocity on release.
    recent: Vec<Vec3>,
}

/// State of the player's hand in the world.
#[derive(Resource, Default)]
pub struct DivineHand {
    /// The ray from the camera through the cursor, whether or not it hits ground.
    /// This is what places the hand when it is over the interface or open sky.
    pub cursor_ray: Option<Ray3d>,
    /// Where the cursor ray meets the ground, if it does.
    pub cursor_world: Option<Vec3>,
    /// The object the cursor is over, if any.
    pub hovered: Option<Entity>,
    /// The object being carried, if any.
    pub held: Option<HeldObject>,
}

impl DivineHand {
    /// The world position the hand is currently at — the point a held object is
    /// being carried toward.
    pub fn grip_point(&self) -> Option<Vec3> {
        let ground = self.cursor_world?;
        let lift = self.held.as_ref().map_or(0.0, |h| h.hold_height);
        Some(ground + Vec3::Y * lift)
    }

    /// Estimated hand velocity from recent motion.
    fn velocity(&self, dt: f32) -> Vec3 {
        let Some(held) = &self.held else {
            return Vec3::ZERO;
        };
        if held.recent.len() < 2 || dt <= 0.0 {
            return Vec3::ZERO;
        }

        // Average the per-frame deltas rather than taking first-to-last, so a
        // single stuttered frame does not dominate the throw.
        let mut total = Vec3::ZERO;
        for pair in held.recent.windows(2) {
            total += pair[1] - pair[0];
        }
        total / ((held.recent.len() - 1) as f32 * dt)
    }
}

/// Closest object whose bounding sphere the ray passes through.
fn pick_object(
    ray: Ray3d,
    candidates: &Query<(Entity, &GlobalTransform, &PickRadius), Without<Held>>,
    max_distance: f32,
) -> Option<Entity> {
    let mut best: Option<(f32, Entity)> = None;

    for (entity, transform, radius) in candidates.iter() {
        let centre = transform.translation();
        let to_centre = centre - ray.origin;
        let along = to_centre.dot(*ray.direction);

        if along < 0.0 || along > max_distance {
            continue;
        }

        // Perpendicular distance from the ray to the sphere centre.
        let closest = ray.origin + *ray.direction * along;
        let miss = closest.distance(centre);

        // A little forgiveness for small targets, but not a magnet: the halo
        // used to be wide enough that pointing *between* two villagers was
        // impossible. What wins is whatever the cursor is most centred on,
        // not whatever is nearest the camera.
        let score = miss / radius.0;
        if score <= 1.15 && best.is_none_or(|(s, _)| score < s) {
            best = Some((score, entity));
        }
    }

    best.map(|(_, e)| e)
}

fn update_hand_ray(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &CameraRig), With<GodCamera>>,
    terrain: Option<Res<Terrain>>,
    mut hand: ResMut<DivineHand>,
) {
    let (Ok(window), Ok((camera, rig)), Some(terrain)) =
        (windows.single(), cameras.single(), terrain)
    else {
        return;
    };

    // The ray is cast from the rig's pose THIS frame, not from the camera's
    // GlobalTransform — that syncs in PostUpdate and is a frame stale. A
    // static camera hides the difference; a moving one (the title drift, the
    // opening descent, a follow) turns it into a per-frame sawtooth that made
    // the pointing hand visibly jitter.
    let camera_transform = GlobalTransform::from(
        Transform::from_translation(rig.eye()).looking_at(rig.focus, Vec3::Y),
    );

    let cursor = match window.cursor_position() {
        Some(cursor) => cursor,
        // Unattended capture runs never have a pointer over the window; aim through
        // mid-screen instead so the hand appears in what the capture is *for*.
        None if crate::capture_path().is_some() => {
            Vec2::new(window.width() * 0.5, window.height() * 0.62)
        }
        // Cursor left the window. Keep the last known point so a held object does
        // not snap to the origin.
        None => return,
    };

    let Ok(ray) = camera.viewport_to_world(&camera_transform, cursor) else {
        return;
    };

    hand.cursor_ray = Some(ray);
    hand.cursor_world = terrain::raycast(&terrain, ray);
}

fn update_hover(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<GodCamera>>,
    rigs: Query<&CameraRig>,
    candidates: Query<(Entity, &GlobalTransform, &PickRadius), Without<Held>>,
    pointer: Res<PointerContext>,
    mut hand: ResMut<DivineHand>,
) {
    // A panel between the cursor and the world blocks the world. Hovering a
    // villager through the HUD and snatching them with a misclick is exactly the
    // kind of accident the interface layer exists to prevent. Likewise a body
    // being worn: the hand is withdrawn at that range, and a withdrawn hand
    // touches nothing. Naming what is "beneath your hand" while the god has
    // no hand to speak of gives the whole thing away.
    let riding = rigs.single().is_ok_and(|rig| rig.in_a_body);
    if hand.held.is_some() || pointer.over_ui || riding {
        hand.hovered = None;
        return;
    }

    let (Ok(window), Ok((camera, camera_transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
        return;
    };

    hand.hovered = pick_object(ray, &candidates, 600.0);
}

/// A clean right-click on a creature pins the camera to them; on the same
/// creature again, drops to their shoulder; once more — or on empty ground —
/// lets go. A right-*drag* is an orbit and is left alone.
fn toggle_follow(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    pointer: Res<PointerContext>,
    hand: Res<DivineHand>,
    creatures: Query<(), With<crate::creature::Creature>>,
    rigs: Query<&crate::camera::CameraRig>,
    mut follow: ResMut<crate::camera::FollowTarget>,
    mut press_at: Local<Option<Vec2>>,
) {
    use crate::camera::FollowStyle;

    // A worn body is not a follow the player may click out of. This cycle
    // releases the pin on any clean right-click that lands on nothing new —
    // sensible when the pin is a choice about whom to watch, ruinous when it
    // is a possession, and the mouse is locked away and every stray click
    // lands on nothing. It let go of the body while the god was still inside
    // it: nothing pinned the camera any more, so it kept the eye height it
    // had and the movement keys flew it through hillsides. The ride is ended
    // by the miracle or by Escape, not by a click.
    if rigs.single().is_ok_and(|rig| rig.in_a_body) {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    if buttons.just_pressed(MouseButton::Right) {
        *press_at = window.cursor_position();
    }
    if !buttons.just_released(MouseButton::Right) {
        return;
    }
    let (Some(down), Some(up)) = (press_at.take(), window.cursor_position()) else {
        return;
    };
    if down.distance(up) > 6.0 || pointer.over_ui {
        return;
    }

    let hovered_creature = hand.hovered.filter(|e| creatures.get(*e).is_ok());
    match (follow.entity, hovered_creature) {
        // A different creature: switch the follow to them.
        (Some(current), Some(other)) if other != current => {
            follow.entity = Some(other);
            follow.style = FollowStyle::Overhead;
        }
        // Already following: any clean right-click on nothing new lets go.
        (Some(_), _) => {
            follow.entity = None;
            follow.style = FollowStyle::Overhead;
        }
        // Not following and clicked someone: begin.
        (None, Some(creature)) => {
            follow.entity = Some(creature);
            follow.style = FollowStyle::Overhead;
        }
        (None, None) => {}
    }
}

fn handle_grab_and_release(
    mut commands: Commands,
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    terrain: Option<Res<Terrain>>,
    mut hand: ResMut<DivineHand>,
    mut motions: Query<&mut CreatureMotion>,
    transforms: Query<&GlobalTransform>,
    parents: Query<(), With<ChildOf>>,
    rooted: Query<(), With<Rooted>>,
    trees: Query<(
        &crate::scatter::FellableTree,
        &crate::scatter::TreeBody,
        &crate::scatter::InGrove,
    )>,
    mut grove_kit: (
        ResMut<Assets<Mesh>>,
        Res<crate::terrain::TerrainAssets>,
        ResMut<crate::scatter::DirtyGroves>,
        ResMut<crate::scatter::StrippedGround>,
    ),
    matters: Query<&crate::matter::Matter>,
    pointer: Res<PointerContext>,
    armed: Res<crate::miracles::SelectedMiracle>,
    mut witnessed: MessageWriter<DivineEvent>,
) {
    let Some(terrain) = terrain else {
        return;
    };

    // Grab — but never through a panel. Releases are still honoured over the
    // interface, so carrying a villager across the HUD cannot trap them in
    // the hand. The rooted cannot be grabbed at all.
    if buttons.just_pressed(MouseButton::Left)
        && !pointer.over_ui
        && armed.0.is_none()
        && hand.held.is_none()
        && let Some(entity) = hand.hovered
        && rooted.get(entity).is_err()
        && let Ok(transform) = transforms.get(entity)
    {
        let position = transform.translation();
        let ground = terrain.height_at(position.x, position.z).max(WATER_LEVEL);

        // Start the carry at whatever height it was already at, plus a lift, so
        // grabbing does not yank the object to a fixed altitude.
        let hold_height = (position.y - ground + 2.2).clamp(1.2, 8.0);

        commands
            .entity(entity)
            .insert(Held)
            .remove::<Airborne>()
            .remove::<crate::matter::Rolling>()
            .remove::<crate::matter::Floating>()
            .insert(MoveTarget(None));

        // Anything living in a chunk's coordinate space leaves it the
        // moment the god's hand closes: held transforms are written in
        // world coordinates, and a boulder still parented to its chunk
        // would teleport by the chunk's whole offset at the first tug —
        // a stone that vanishes from the hand.
        if parents.get(entity).is_ok() {
            commands
                .entity(entity)
                .remove::<ChildOf>()
                .insert(transform.compute_transform());
        }

        // Grabbing a living tree is an uprooting: it leaves the ground's
        // ownership (and its chunk's coordinate space), becomes loose wood in
        // the world, and everyone nearby sees it happen.
        if let Ok((tree, body, home)) = trees.get(entity) {
            // Torn from the ground AND from its grove: the tree takes its
            // own body with it, and the grove closes over the gap.
            let (meshes, terrain_assets, dirty_groves, stripped) = &mut grove_kit;
            crate::scatter::stand_alone(
                &mut commands,
                meshes,
                terrain_assets.ground_material.clone(),
                entity,
                body,
                home,
                dirty_groves,
            );
            // The ground remembers the uprooting: no chunk rebuild quietly
            // replants what the god tore out.
            stripped.strip(position.x, position.z);
            commands
                .entity(entity)
                .remove::<ChildOf>()
                .remove::<crate::scatter::FellableTree>()
                .insert((
                    Name::new("A torn-up tree"),
                    crate::matter::Matter::felled_tree(tree.maturity),
                    Transform::from_translation(position),
                ));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Uprooted,
                position,
                subject: None,
                intensity: 0.7,
            });
        }

        if let Ok(mut motion) = motions.get_mut(entity) {
            motion.flail = 1.0;
            motion.speed = 0.0;
        }

        witnessed.write(DivineEvent {
            kind: DivineEventKind::Lifted,
            position,
            subject: Some(entity),
            intensity: 0.5,
        });

        hand.held = Some(HeldObject {
            entity,
            hold_height,
            recent: Vec::with_capacity(VELOCITY_SAMPLES),
        });
        hand.hovered = None;
    }

    // Release. Velocity is read before the hand lets go, since it is derived from
    // the hand's own recorded path.
    if buttons.just_released(MouseButton::Left) && hand.held.is_some() {
        let hand_velocity = hand.velocity(time.delta_secs());
        let held = hand.held.take().expect("checked above");

        let speed = hand_velocity.length();
        // Mass tells in the arc: a bush sails, a boulder barely clears the
        // fingers. Creatures carry no Matter and fly as they always did.
        let heft = matters
            .get(held.entity)
            .map(|m| m.throw_factor())
            .unwrap_or(1.0);
        let launch = if speed > THROW_THRESHOLD {
            // Add lift so a flat sideways flick still arcs. A throw that slides
            // along the ground reads as a shove, not a throw.
            (hand_velocity * THROW_STRENGTH + Vec3::Y * (speed * 0.22).min(6.0)) * heft
        } else {
            // A drop still falls; it just falls straight down.
            Vec3::ZERO
        };

        // A hurl and a careful placement are different acts, and the people watching
        // need to be able to tell them apart.
        let position = transforms
            .get(held.entity)
            .map(|t| t.translation())
            .unwrap_or_default();

        witnessed.write(if speed > THROW_THRESHOLD {
            DivineEvent {
                kind: DivineEventKind::Thrown,
                position,
                subject: Some(held.entity),
                intensity: (speed / 20.0).clamp(0.3, 1.0),
            }
        } else {
            DivineEvent {
                kind: DivineEventKind::SetDown,
                position,
                subject: Some(held.entity),
                intensity: 0.2,
            }
        });

        commands
            .entity(held.entity)
            .remove::<Held>()
            .insert(Airborne { velocity: launch })
            .insert(DivinelyPlaced { remaining: 25.0 });

        if let Ok(mut motion) = motions.get_mut(held.entity) {
            motion.flail = 1.0;
        }
    }
}

fn carry_held_object(
    time: Res<Time>,
    mut hand: ResMut<DivineHand>,
    mut transforms: Query<&mut Transform, With<Held>>,
    mut motions: Query<&mut CreatureMotion, With<Held>>,
) {
    let dt = time.delta_secs();
    let Some(grip) = hand.grip_point() else {
        return;
    };

    // Record the hand's own path for the throw estimate.
    if let Some(held) = &mut hand.held {
        held.recent.push(grip);
        if held.recent.len() > VELOCITY_SAMPLES {
            held.recent.remove(0);
        }
    }

    let Some(held) = &hand.held else {
        return;
    };
    let Ok(mut transform) = transforms.get_mut(held.entity) else {
        return;
    };

    // Spring toward the grip point. The lag is the whole point: it is what makes
    // a carried villager feel like a carried villager rather than a cursor sprite.
    let t = 1.0 - (-HOLD_SPRING * dt).exp();
    let previous = transform.translation;
    transform.translation = previous.lerp(grip, t);

    // Dangle: lean away from the direction of travel, proportional to how far
    // behind the grip the object is trailing.
    let trail = grip - transform.translation;
    let lean_x = (trail.z * 0.22).clamp(-0.6, 0.6);
    let lean_z = (-trail.x * 0.22).clamp(-0.6, 0.6);

    let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
    let target_rotation =
        Quat::from_rotation_y(yaw) * Quat::from_rotation_x(lean_x) * Quat::from_rotation_z(lean_z);
    transform.rotation = transform.rotation.slerp(target_rotation, t * 0.6);

    // Keep held creatures struggling the whole time they are off the ground.
    if let Ok(mut motion) = motions.get_mut(held.entity) {
        motion.flail = motion.flail.max(0.45);
        motion.speed = 0.0;
    }
}

/// The light the hand sheds on the world at night.
#[derive(Component)]
struct HandGlow;

/// Swells the hand's radiance as the light goes, and breathes it.
///
/// The glow is for SEEING at night — the god's lantern — so it follows the
/// dark: nothing at noon, full past dusk. The slow breath is what makes it
/// read as presence rather than a lamp; the hand's own emissive skin swells
/// on the same rhythm, so the glow visibly comes FROM it.
fn breathe_hand_glow(
    time: Res<Time<Real>>,
    clock: Option<Res<crate::calendar::WorldClock>>,
    hand_materials: Option<Res<HandMaterials>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut glows: Query<&mut PointLight, With<HandGlow>>,
) {
    let Some(clock) = clock else {
        return;
    };
    let t = clock.time_of_day();
    // Dusk ramps in just before true night; the wrap back to 0.0 is dawn.
    let nightness = ((t - 0.68) / 0.08).clamp(0.0, 1.0);
    // A slow breath, about six seconds to the cycle: presence, not strobe.
    let breath = 1.0 + 0.18 * (time.elapsed_secs() * 1.05).sin();
    for mut light in &mut glows {
        light.intensity = 900_000.0 * nightness * breath;
    }
    // The skin itself brightens with the light it sheds.
    if let Some(hands) = hand_materials {
        let glow = 0.25 + nightness * breath * 1.3;
        if let Some(mut skin) = materials.get_mut(&hands.skin) {
            skin.emissive = LinearRgba::from(palette::shade(&palette::CLOTH_GOLD, 0.9)) * glow;
        }
        if let Some(mut knuckle) = materials.get_mut(&hands.knuckle) {
            knuckle.emissive =
                LinearRgba::from(palette::shade(&palette::CLOTH_GOLD, 0.9)) * (glow * 0.6);
        }
    }
}

fn spawn_hand_cursor(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // Pale and faintly warm, with just enough emissive to stay readable over dark
    // ground without blooming into a blob. The hand is the player; it must never
    // be lost against the world.
    let skin = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::BONE, 1.0),
        emissive: LinearRgba::from(palette::shade(&palette::CLOTH_GOLD, 0.9)) * 0.25,
        perceptual_roughness: 0.65,
        reflectance: 0.08,
        ..default()
    });
    let knuckle = materials.add(StandardMaterial {
        base_color: palette::shade(&palette::BONE, 0.72),
        emissive: LinearRgba::from(palette::shade(&palette::CLOTH_GOLD, 0.9)) * 0.16,
        perceptual_roughness: 0.7,
        reflectance: 0.06,
        ..default()
    });
    commands.insert_resource(HandMaterials {
        skin: skin.clone(),
        knuckle: knuckle.clone(),
    });

    let root = commands
        .spawn((
            Name::new("Divine Hand"),
            HandModel,
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();

    // The god's own radiance: a soft pale-gold light the hand sheds on the
    // WORLD (not the overlay — no hand layer, so it lights the ground and
    // the sleepers, never the cursor model itself). By day it is nothing;
    // by night it swells and breathes, and the hand becomes a lantern.
    commands.spawn((
        HandGlow,
        PointLight {
            color: Color::srgb(1.0, 0.93, 0.72),
            intensity: 0.0,
            range: 30.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 1.4, 0.0),
        ChildOf(root),
    ));

    // Palm. Fingers hang from its leading edge; forward is -Z, the same convention
    // as every creature. Every mesh goes on the hand's own render layer, drawn by
    // the overlay camera above both the world and the interface: a cursor must
    // never be occluded. `RenderLayers` does not inherit, so each mesh carries it.
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(skin.clone()),
        Transform::from_xyz(0.0, 0.0, 0.12).with_scale(Vec3::new(1.0, 0.26, 1.08)),
        RenderLayers::layer(HAND_LAYER),
        ChildOf(root),
    ));

    // A finger segment: a joint entity at the knuckle with the bone hanging off it,
    // exactly how creature limbs are built — rotating the joint curls the finger.
    let mut segment = |parent: Entity,
                       joint: Vec3,
                       yaw: f32,
                       length: f32,
                       girth: f32,
                       material: &Handle<StandardMaterial>|
     -> Entity {
        let joint_entity = commands
            .spawn((
                Transform::from_translation(joint).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(0.0, 0.0, -length * 0.5).with_scale(Vec3::new(
                girth,
                girth * 1.05,
                length,
            )),
            RenderLayers::layer(HAND_LAYER),
            ChildOf(joint_entity),
        ));
        joint_entity
    };

    // Four fingers of different lengths — even this blocky, equal fingers read as
    // a rake rather than a hand.
    let mut fingers = Vec::with_capacity(4);
    for (x, length) in [(-0.36, 0.52), (-0.12, 0.62), (0.12, 0.57), (0.36, 0.42)] {
        let proximal = segment(root, Vec3::new(x, 0.0, -0.42), 0.0, length, 0.19, &skin);
        let distal = segment(
            proximal,
            Vec3::new(0.0, 0.0, -length),
            0.0,
            length * 0.8,
            0.17,
            &knuckle,
        );
        fingers.push([proximal, distal]);
    }

    // Thumb: set back on the side, splayed outward, opposing the fingers.
    let thumb_base = segment(root, Vec3::new(-0.55, -0.02, 0.18), 0.85, 0.46, 0.2, &skin);
    let thumb_tip = segment(
        thumb_base,
        Vec3::new(0.0, 0.0, -0.46),
        0.0,
        0.36,
        0.18,
        &knuckle,
    );

    commands.entity(root).insert(HandRig {
        fingers,
        thumb: [thumb_base, thumb_tip],
        carry: 0.0,
        grip: 0.1,
        bank: Vec2::ZERO,
        point: 0.0,
        tap: 0.0,
        fade: 1.0,
    });
}

/// Where the hand sits and how it is oriented while acting as the UI cursor:
/// floating just in front of the camera on the cursor ray, back of the hand to
/// the viewer, index finger reaching up-screen with its tip on the cursor point.
fn ui_cursor_placement(camera: &CameraRig, ray: Ray3d, tap: f32) -> (Vec3, Quat) {
    let camera_rotation = Transform::from_translation(camera.eye())
        .looking_at(camera.focus, Vec3::Y)
        .rotation;

    // Rotating local +Y onto camera +Z turns the palm to face the screen with the
    // fingers running up it; the slight yaw and pitch tip the index finger toward
    // upper-left, the way every cursor since the first has leaned.
    let rotation = camera_rotation
        * Quat::from_rotation_x(FRAC_PI_2)
        * Quat::from_rotation_y(0.25)
        * Quat::from_rotation_x(-0.15);

    // Park the *fingertip* on the cursor, not the palm — computed from the
    // finger's real joint chain, including however far the current tap has
    // curled it, so the tip stays glued to the click point mid-press.
    let proximal = 0.04 + tap * 0.42;
    let distal = proximal + 0.03 + tap * 0.5;
    let fingertip_local = Vec3::new(
        -0.36,
        -(0.52 * proximal.sin() + 0.416 * distal.sin()),
        -(0.42 + 0.52 * proximal.cos() + 0.416 * distal.cos()),
    ) * UI_CURSOR_SCALE;
    let position = ray.origin + *ray.direction * UI_CURSOR_DEPTH - rotation * fingertip_local;

    (position, rotation)
}

/// Poses and places the hand every frame.
fn animate_hand(
    time: Res<Time<Real>>,
    hand: Res<DivineHand>,
    pointer: Res<PointerContext>,
    buttons: Res<ButtonInput<MouseButton>>,
    state: Res<State<crate::GameState>>,
    cameras: Query<&CameraRig>,
    anchors: Query<&GlobalTransform>,
    mut roots: Query<(&mut Transform, &mut Visibility, &mut HandRig), With<HandModel>>,
    mut joints: Query<&mut Transform, Without<HandModel>>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let Ok((mut transform, mut visibility, mut rig)) = roots.single_mut() else {
        return;
    };
    // The splash belongs to the studio card alone: no hand hangs over it.
    if matches!(state.get(), crate::GameState::Splash) {
        *visibility = Visibility::Hidden;
        return;
    }

    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    let held = hand.held.is_some();
    let hovered = hand
        .hovered
        .and_then(|entity| anchors.get(entity).ok())
        .map(|global| global.translation());

    // How far into UI-cursor mode the hand is. Eased, so crossing a panel edge
    // reads as the hand lifting off the world rather than teleporting.
    let ease = 1.0 - (-14.0 * dt).exp();
    let pointing = pointer.over_ui && !held;
    rig.point += ((pointing as u32 as f32) - rig.point) * ease;

    // Behind a mortal's eyes the hand withdraws entirely — at that range it
    // would fill the whole frame, and a god wearing a body has no business
    // waving its own hand in front of the face. Ordinary play never comes
    // closer than `MIN_DISTANCE`, twelve metres, so nothing but Avatar can
    // trip this. It fades away and back rather than popping.
    let withdrawn = camera.distance < WITHDRAW_WITHIN && !held;
    let fade_ease = 1.0 - (-6.0 * dt).exp();
    rig.fade += ((!withdrawn as u32 as f32) - rig.fade) * fade_ease;

    // The click, made visible: while pointing, a press dips the index finger
    // into whatever it is over. Attack much faster than release so the tap
    // *lands*, then lifts.
    let tap_target = if pointing && buttons.pressed(MouseButton::Left) {
        1.0
    } else {
        0.0
    };
    let tap_rate = if tap_target > rig.tap { 34.0 } else { 11.0 };
    let tap_ease = 1.0 - (-tap_rate * dt).exp();
    rig.tap += (tap_target - rig.tap) * tap_ease;

    // Where the hand belongs in the world: over the carry, over what it is about
    // to grab, or over the ground.
    let anchor = if held {
        hand.grip_point()
    } else if let Some(position) = hovered {
        // Lean toward the hovered thing rather than locking onto it: the
        // cursor still steers, the hand just shows interest.
        match hand.cursor_world {
            Some(cursor) => Some(cursor.lerp(position + Vec3::Y * 1.0, 0.5)),
            None => Some(position + Vec3::Y * 1.0),
        }
    } else {
        hand.cursor_world
    };

    // The interface placement, available whenever there is a cursor at all.
    let ui_placement = hand
        .cursor_ray
        .map(|ray| ui_cursor_placement(camera, ray, rig.tap));

    // With no ground under the cursor the hand rides at UI depth regardless of
    // rig.point — over open sky it stays a cursor instead of vanishing.
    let blend = match (anchor, ui_placement) {
        (Some(_), Some(_)) => rig.point,
        (None, Some(_)) => 1.0,
        (Some(_), None) => 0.0,
        (None, None) => {
            *visibility = Visibility::Hidden;
            return;
        }
    };
    *visibility = if rig.fade < 0.03 {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };

    // Carrying the flag: the whole of the choosing is one gesture, somebody
    // walking the country with a standard in their fist. An open palm laid
    // flat over the ground is the pose for picking things up, and it read as
    // the god hovering a hand near a pole rather than holding it.
    let carrying = matches!(state.get(), crate::GameState::Choosing);
    rig.carry += ((carrying as u32 as f32) - rig.carry) * fade_ease;

    let (open_grip, hover) = pose(held, hovered.is_some());
    // A fist, and it wins outright over whatever the hover was doing.
    let target_grip = open_grip.max(rig.carry);
    rig.grip += (target_grip - rig.grip) * ease;

    // Mild growth with zoom, so the hand neither vanishes from altitude nor
    // swallows the village up close.
    let world_scale = 1.6 + camera.zoom_fraction() * 2.8;
    let scale = world_scale + (UI_CURSOR_SCALE - world_scale) * blend;

    // Two incommensurate sines so the drift never reads as a loop. Gripping
    // steadies it — a hand carrying something concentrates — and so does
    // pointing: a cursor that bobs is a cursor that misses.
    let calm = (1.0 - rig.grip * 0.7) * (1.0 - blend);
    let bob = ((t * 1.1).sin() * 0.16 + (t * 1.9 + 2.0).sin() * 0.07) * calm;

    let world_position = anchor.map(|anchor| anchor + Vec3::Y * (hover * world_scale * 0.5 + bob));
    let position = match (world_position, ui_placement) {
        (Some(world), Some((ui, _))) => world.lerp(ui, blend),
        (Some(world), None) => world,
        (None, Some((ui, _))) => ui,
        (None, None) => unreachable!("handled above"),
    };
    // A tap presses the whole hand slightly into the interface, along the view
    // ray — the finger does most of the acting, this sells the follow-through.
    let position = position + camera.forward() * (rig.tap * blend * 0.7 * scale);

    let previous = transform.translation;
    // Over the world the hand glides; as a cursor it snaps, because a
    // pointer that trails the mouse reads as a pointer that misses.
    let follow = 1.0 - (-14.0 * dt).exp();
    let follow = follow + (1.0 - follow) * blend;
    transform.translation = transform.translation.lerp(position, follow);

    // Bank into travel, the way anything moving through air does. This, more than
    // any idle loop, is what makes the hand feel suspended rather than pinned:
    // fling it across the map and it heels over; stop and it settles level.
    // Suppressed while pointing — an interface cursor holds its attitude.
    let velocity = (transform.translation - previous) / dt.max(1e-5);
    let local = Quat::from_rotation_y(-camera.yaw) * velocity;
    let target_bank = Vec2::new(
        (local.x * 0.035).clamp(-0.55, 0.55),
        (local.z * 0.03).clamp(-0.45, 0.45),
    ) * (1.0 - blend);
    let settle = 1.0 - (-7.0 * dt).exp();
    let bank_delta = (target_bank - rig.bank) * settle;
    rig.bank += bank_delta;

    // Idle sway on top, so even a motionless hand breathes.
    let sway_roll = (t * 0.9).sin() * 0.05 * calm;
    let sway_pitch = (t * 0.7 + 1.3).sin() * 0.04 * calm;

    // Back of the hand to the camera, fingers up-screen, palm laid nearly flat —
    // tipped just enough toward the ground to show intent.
    let world_rotation = Quat::from_rotation_y(camera.yaw)
        * Quat::from_rotation_x(-0.12 + sway_pitch + rig.bank.y + CARRY_PITCH * rig.carry)
        * Quat::from_rotation_z(sway_roll - rig.bank.x + CARRY_ROLL * rig.carry);
    transform.rotation = match ui_placement {
        Some((_, ui_rotation)) => world_rotation.slerp(ui_rotation, blend),
        None => world_rotation,
    };
    transform.scale = Vec3::splat(scale * rig.fade.max(0.001));

    // Fingers: knuckle takes the curl, the mid-joint slightly more, and an idle
    // ripple runs through them while the hand is open. Pointing overrides all of
    // it — index straight, the rest folded — blended by the same scalar as the
    // flight, so pose and position arrive together.
    let point = rig.point;
    for (index, [proximal, distal]) in rig.fingers.iter().enumerate() {
        let ripple = (t * 1.1 + index as f32 * 1.5).sin() * 0.09 * (1.0 - rig.grip);
        // The index stays straight while pointing — until a click taps it down
        // like it is pressing the thing under it.
        let (point_proximal, point_distal) = if index == 0 {
            (0.04 + rig.tap * 0.42, 0.03 + rig.tap * 0.5)
        } else {
            (1.02 + index as f32 * 0.05, 1.1)
        };

        let proximal_curl =
            (0.28 + rig.grip * 0.95 + ripple) * (1.0 - point) + point_proximal * point;
        let distal_curl =
            (0.22 + rig.grip * 1.1 + ripple * 0.6) * (1.0 - point) + point_distal * point;

        if let Ok(mut joint) = joints.get_mut(*proximal) {
            joint.rotation = Quat::from_rotation_x(-proximal_curl);
        }
        if let Ok(mut joint) = joints.get_mut(*distal) {
            joint.rotation = Quat::from_rotation_x(-distal_curl);
        }
    }

    let [thumb_base, thumb_tip] = rig.thumb;
    let thumb_curl = (0.1 + rig.grip * 0.8) * (1.0 - point) + 0.8 * point;
    if let Ok(mut joint) = joints.get_mut(thumb_base) {
        joint.rotation = Quat::from_rotation_y(0.85) * Quat::from_rotation_x(-thumb_curl);
    }
    if let Ok(mut joint) = joints.get_mut(thumb_tip) {
        joint.rotation = Quat::from_rotation_x(
            -(0.1 * (1.0 - point) + 0.85 * point + rig.grip * 0.9 * (1.0 - point)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hand_closes_progressively() {
        // Open drifting, flexed over a target, shut around a carry — and it sinks
        // lower the more engaged it is.
        let (open, open_hover) = pose(false, false);
        let (ready, ready_hover) = pose(false, true);
        let (grip, grip_hover) = pose(true, false);

        assert!(open < ready && ready < grip);
        assert!(open_hover > ready_hover && ready_hover > grip_hover);
    }

    #[test]
    fn throw_velocity_averages_recent_hand_motion() {
        let mut hand = DivineHand::default();
        hand.held = Some(HeldObject {
            entity: Entity::from_raw_u32(0).unwrap(),
            hold_height: 2.0,
            // Moving +2 units on X per frame.
            recent: (0..VELOCITY_SAMPLES)
                .map(|i| Vec3::new(i as f32 * 2.0, 0.0, 0.0))
                .collect(),
        });

        let dt = 1.0 / 60.0;
        let velocity = hand.velocity(dt);
        assert!((velocity.x - 2.0 / dt).abs() < 1e-2, "got {velocity:?}");
        assert!(velocity.y.abs() < 1e-5 && velocity.z.abs() < 1e-5);
    }

    #[test]
    fn a_stationary_hand_throws_nothing() {
        let mut hand = DivineHand::default();
        hand.held = Some(HeldObject {
            entity: Entity::from_raw_u32(0).unwrap(),
            hold_height: 2.0,
            recent: vec![Vec3::new(3.0, 1.0, 2.0); VELOCITY_SAMPLES],
        });
        assert!(hand.velocity(1.0 / 60.0).length() < 1e-5);
    }

    #[test]
    fn grip_point_sits_above_the_cursor_while_carrying() {
        let mut hand = DivineHand::default();
        hand.cursor_world = Some(Vec3::new(1.0, 5.0, 2.0));
        assert_eq!(hand.grip_point(), Some(Vec3::new(1.0, 5.0, 2.0)));

        hand.held = Some(HeldObject {
            entity: Entity::from_raw_u32(0).unwrap(),
            hold_height: 3.0,
            recent: Vec::new(),
        });
        assert_eq!(hand.grip_point(), Some(Vec3::new(1.0, 8.0, 2.0)));
    }
}
