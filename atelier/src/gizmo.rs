//! The tool modes and the fine hand.
//!
//! A mode bar rides the top of the window, the way the big 3D programs
//! do it: NORMAL places and grabs, MOVE puts translate arrows on what
//! you click, RESIZE puts end-handles on any sized primitive and drags
//! its dimensions in quarter-metre steps. Tab walks the modes.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::Bench;
use crate::builder::{self, Ghost, Hovered, Naming, PartKind, Placed};
use crate::look::Palette;

/// Which tool the mouse is.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ToolMode {
    #[default]
    Normal,
    Move,
    Resize,
    /// Colour what is already standing: clicking a part paints it with the
    /// brush rather than merely selecting it.
    Paint,
}

/// The part currently wearing handles.
#[derive(Resource, Default)]
pub struct Selected(pub Vec<Entity>);

impl Selected {
    /// The one part chosen, when exactly one is.
    ///
    /// Handles ask this rather than "the first of them", and the difference is
    /// the whole rule: a single part wears its OWN handles - a gable roof keeps
    /// its eaves and its ridge - while several together can only be moved, since
    /// stretching six things at once has no meaning to invent.
    pub fn one(&self) -> Option<Entity> {
        (self.0.len() == 1).then(|| self.0[0])
    }

    /// The part a menu or a measure speaks about: the first chosen.
    pub fn lead(&self) -> Option<Entity> {
        self.0.first().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn holds(&self, part: Entity) -> bool {
        self.0.contains(&part)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// In or out - a shift-click on something already chosen lets it go.
    pub fn toggle(&mut self, part: Entity) {
        if let Some(at) = self.0.iter().position(|held| *held == part) {
            self.0.remove(at);
        } else {
            self.0.push(part);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}

/// A drag in progress.
pub struct DragState {
    dir: Vec3,
    t0: f32,
    start_at: Vec3,
    grip: Grip,
}

#[derive(Clone, Copy)]
enum Grip {
    /// Slide the whole part along the handle's direction.
    Slide,
    /// Pull one end of a sized primitive: which local axis, and the
    /// dimensions when the grip closed. The handle's own direction
    /// already points out of the pulled end.
    Size {
        on_x: bool,
        w0: f32,
        d0: f32,
        /// What the part actually MEASURED when the handle was taken hold of.
        was: Vec2,
    },
    /// Pull a whole roof's eaves out past the walls, leaving the gables
    /// where the building is.
    Over { o0: f32 },
    /// Pull a roof's ridge up or down: its pitch, in degrees when the grip
    /// closed. The eaves do not move, so a roof steepens where it stands.
    Pitch { p0: f32 },
    /// Raise or lower a flight's handrail. The treads do not move.
    Rail { h0: f32 },
    /// Raise or lower a pad: how tall the stone stands.
    Rise { h0: f32 },
}

#[derive(Resource, Default)]
pub struct GizmoDrag(Option<DragState>);

/// True while the cursor rides a handle or a drag is live.
#[derive(Resource, Default)]
pub struct GizmoHot(pub bool);

#[derive(Component)]
struct GizmoRoot;

/// A handle: its world direction, its dye, and what gripping it does.
#[derive(Component)]
struct Handle {
    dir: Vec3,
    ramp: &'static str,
    grip: Grip,
}

#[derive(Component)]
struct GizmoCamera;

const ARROW_LAYER: usize = 1;

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ToolMode>()
            .init_resource::<Selected>()
            .init_resource::<GizmoDrag>()
            .init_resource::<GizmoHot>()
            .add_systems(Startup, raise_gizmo_camera)
            .add_systems(
                Update,
                (walk_modes, select_part, dress_gizmo, work_gizmo)
                    .chain()
                    // Moving, resizing and painting are things done to a
                    // BUILDING. Brett, on finding he could stretch a villager's
                    // head: "I shouldnt be able to stretch or resize any part of
                    // them." The rig bench has one tool and it is the hand.
                    .run_if(|bench: Res<crate::Bench>| *bench == crate::Bench::Builder),
            )
            .add_systems(Update, ride_along)
            .add_systems(
                Update,
                // And the modes go back to NORMAL on the way out, so a maker who
                // left the builder in RESIZE does not come back to a bench that
                // has been in RESIZE all the while they were somewhere else.
                |bench: Res<crate::Bench>,
                 mut mode: ResMut<ToolMode>,
                 mut chosen: ResMut<Selected>| {
                    if bench.is_changed() && *bench != crate::Bench::Builder {
                        *mode = ToolMode::Normal;
                        chosen.clear();
                    }
                },
            );
    }
}

/// The overlay camera: same eye as the bench camera, drawing only the
/// arrow layer, after everything, onto a cleared depth buffer. The UI
/// rides it too, so panels stay above the arrows.
fn raise_gizmo_camera(mut commands: Commands) {
    commands.spawn((
        GizmoCamera,
        Camera3d::default(),
        Camera {
            order: 1,
            clear_color: bevy::camera::ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
        RenderLayers::layer(ARROW_LAYER),
        Transform::default(),
    ));
}

fn ride_along(
    bench_camera: Query<&Transform, (With<Camera3d>, Without<GizmoCamera>)>,
    mut overlay: Query<&mut Transform, With<GizmoCamera>>,
) {
    let Ok(eye) = bench_camera.single() else {
        return;
    };
    for mut camera in &mut overlay {
        *camera = *eye;
    }
}

/// Tab walks the modes; returning to NORMAL drops the selection.
fn walk_modes(
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<builder::DimsEntry>,
    mut mode: ResMut<ToolMode>,
    mut selected: ResMut<Selected>,
) {
    if *bench != Bench::Builder || naming.0.is_some() || dims.0.is_some() {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        *mode = match *mode {
            ToolMode::Normal => ToolMode::Move,
            ToolMode::Move => ToolMode::Resize,
            ToolMode::Resize => ToolMode::Paint,
            ToolMode::Paint => ToolMode::Normal,
        };
    }
    if mode.is_changed() && *mode == ToolMode::Normal {
        selected.clear();
    }
}

/// In MOVE and RESIZE, a left click selects what the cursor touches;
/// escape and vanishing deselect. NORMAL keeps no selection at all.
#[allow(clippy::too_many_arguments)]
fn select_part(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mode: Res<ToolMode>,
    hot: Res<GizmoHot>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<builder::DimsEntry>,
    hovered: Res<Hovered>,
    parts: Query<(), With<Placed>>,
    records: Query<(Entity, &Placed), Without<Ghost>>,
    mut selected: ResMut<Selected>,
) {
    if dims.0.is_some() {
        return;
    }
    if *bench != Bench::Builder || naming.0.is_some() {
        selected.clear();
        return;
    }
    // NORMAL is placement: a click there picks a part up, and always did. But
    // SHIFT-click is free in that mode, and grouping is the one thing a maker
    // had to leave the mode to do - Brett: "What about shift clicking in normal
    // mode? Should that work too? Right now it just picks the piece up." So
    // shift gathers here as well, and a plain click lets go of what was gathered
    // before picking anything up.
    let gathering = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if *mode == ToolMode::Normal && !gathering {
        if buttons.just_pressed(MouseButton::Left) {
            selected.clear();
        }
        return;
    }
    // A part that has gone - buried, or left behind on another step - stops
    // being chosen without taking the rest of the choice with it.
    selected.0.retain(|part| parts.contains(*part));
    if buttons.just_pressed(MouseButton::Left)
        && !hot.0
        && let Some(touched) = hovered.grab
    {
        // Shift adds and takes away; a plain click starts afresh. Clicking a
        // part that belongs to a group takes the whole group, which is what
        // being grouped MEANS - see `builder::kin_of`.
        let kin = builder::kin_of(touched, &records);
        if gathering {
            for part in kin {
                selected.toggle(part);
            }
        } else {
            selected.clear();
            for part in kin {
                selected.toggle(part);
            }
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        selected.clear();
    }
}

/// The handles a part deserves in the standing mode: direction, offset
/// of the handle's FOOT from the part's origin, dye, grip.
fn handles_for(mode: ToolMode, record: &Placed) -> Vec<(Vec3, Vec3, &'static str, Grip)> {
    // The part's TRUE pose, tilt and mirror included: a pitched panel's
    // handles must run up its own slope, or dragging one swings the far
    // edge instead of lengthening the roof, and a 45 looks like it bends.
    let spin = builder::pose(record.yaw, record.tilt, record.flip);
    match mode {
        // Painting wears no handles: the part IS the handle, and a shaft in the
        // way would only be something to click by mistake.
        ToolMode::Paint => Vec::new(),
        ToolMode::Move => vec![
            (Vec3::X, Vec3::ZERO, "cloth-red", Grip::Slide),
            (Vec3::Y, Vec3::ZERO, "cloth-gold", Grip::Slide),
            (Vec3::Z, Vec3::ZERO, "cloth-blue", Grip::Slide),
        ],
        ToolMode::Resize => {
            let standing = builder::kind_from_name(&record.part);
            // What the part MEASURES right now, kept with the grip so the mover
            // can tell how much it truly grew.
            let was = standing
                .as_ref()
                .map(builder::extent_of)
                .unwrap_or(Vec2::ZERO);
            let sized = standing.and_then(|kind| match kind {
                PartKind::Wall(long) => Some((long, 0.0, false)),
                // The pieces a punch leaves are walls too, and stretch
                // like them - only their height and lift stay put.
                PartKind::Seg { long, .. } => Some((long, 0.0, false)),
                PartKind::Trim { long, .. } => Some((long, 0.0, false)),
                PartKind::Rail { long, .. } => Some((long, 0.0, false)),
                PartKind::Gable(long, _) => Some((long, 0.0, false)),
                PartKind::Beam(long, ..) => Some((long, 0.0, false)),
                // The chimney sizes its own reach downward.
                PartKind::Chimney(drop) => Some((drop, 0.0, false)),
                // A flight wears both handles. Across is its WIDTH, which is a
                // real measurement of it. Along is its RUN - and a longer run is
                // a taller flight, because the treads are even and the count is
                // what changes, so pulling it out really is climbing higher.
                PartKind::Stairs { rise, wide, .. } => {
                    let (steps, _, tread) = builder::stair_rhythm(rise);
                    Some((wide, steps as f32 * tread, true))
                }
                PartKind::Ridge(long) => Some((long, 0.0, false)),
                PartKind::Floor(w, d) => Some((w, d, true)),
                PartKind::Foundation(w, d, _) => Some((w, d, true)),
                PartKind::Roof(w, d) => Some((w, d, true)),
                PartKind::GableRoof(w, d, _, _) => Some((w, d, true)),
                _ => None,
            });
            let Some((w, d, both)) = sized else {
                return Vec::new();
            };
            let mut handles = Vec::new();
            for end in [-1.0f32, 1.0] {
                let dir = spin * (Vec3::X * end);
                handles.push((
                    dir,
                    dir * (w * 0.5),
                    "cloth-red",
                    Grip::Size {
                        on_x: true,
                        w0: w,
                        d0: d,
                        was,
                    },
                ));
            }
            if both {
                for end in [-1.0f32, 1.0] {
                    let dir = spin * (Vec3::Z * end);
                    handles.push((
                        dir,
                        dir * (d * 0.5),
                        "cloth-blue",
                        Grip::Size {
                            on_x: false,
                            w0: w,
                            d0: d,
                            was,
                        },
                    ));
                }
            }
            // A pad carries one more, in gold: how TALL it stands. Both of the
            // red-and-blue pair are spoken for by its footprint, and a footing
            // that cannot be raised cannot reach the ground on a slope.
            if let Some(PartKind::Foundation(_, _, high)) = builder::kind_from_name(&record.part) {
                handles.push((
                    spin * Vec3::Y,
                    spin * Vec3::new(0.0, high, 0.0),
                    "cloth-gold",
                    Grip::Rise { h0: high },
                ));
            }
            // A flat rail carries the same gold handle a flight's does, and for
            // the same reason: both of the red-and-blue pair are spoken for, and
            // a landing has to meet the flight it continues.
            if let Some(PartKind::Rail { hand, .. }) = builder::kind_from_name(&record.part) {
                handles.push((
                    spin * Vec3::Y,
                    spin * Vec3::new(0.0, hand, 0.0),
                    "cloth-gold",
                    Grip::Rail { h0: hand },
                ));
            }
            // A flight carries one more, in gold: the rail's own height, since
            // both of the red-and-blue pair are spoken for by its width and its
            // run. Brett: "maybe we can add a handle for rail height?"
            if let Some(PartKind::Stairs { rise, hand, .. }) = builder::kind_from_name(&record.part)
            {
                handles.push((
                    spin * Vec3::Y,
                    spin * Vec3::new(0.0, rise + hand, 0.0),
                    "cloth-gold",
                    Grip::Rail { h0: hand },
                ));
            }
            // A whole roof carries two more, in gold: the eaves, which
            // reach out past the walls without taking the gables with
            // them.
            if let Some(PartKind::GableRoof(long, span, over, pitch)) =
                builder::kind_from_name(&record.part)
            {
                // Stood off along the ridge, not straight out past the blue
                // ones. A handle is a shaft one and a third long with a head on
                // the end, and these sat on the SAME LINE as the depth handles
                // with only the overhang between the two feet - so the gold lay
                // inside the blue for three quarters of its length and took
                // every click meant for it. Brett: "these yellow and blue
                // handles overlap so i cant use the blue handles."
                //
                // A quarter of the roof's length to one side, and never less
                // than a stride, so a short roof separates them too.
                let aside = spin * Vec3::X * (long * 0.25).max(0.9);
                for end in [-1.0f32, 1.0] {
                    let dir = spin * (Vec3::Z * end);
                    handles.push((
                        dir,
                        dir * (span * 0.5 + over + 0.35) + aside,
                        "cloth-gold",
                        Grip::Over { o0: over },
                    ));
                }
                // And one at the ridge, straight up: the pitch. Pull the ridge
                // and the roof steepens about its eaves, which stay on the
                // walls where they were set.
                let rise = span * 0.5 * pitch.to_radians().tan();
                handles.push((
                    spin * Vec3::Y,
                    spin * (Vec3::Y * (rise + 0.4)),
                    "cloth-gold",
                    Grip::Pitch { p0: pitch },
                ));
            }
            // A gable is pulled by its peak the same way, so the wall under a
            // steepened roof can be steepened to meet it.
            if let Some(PartKind::Gable(long, pitch)) = builder::kind_from_name(&record.part) {
                let rise = long * 0.5 * pitch.to_radians().tan();
                handles.push((
                    spin * Vec3::Y,
                    spin * (Vec3::Y * (rise + 0.4)),
                    "cloth-gold",
                    Grip::Pitch { p0: pitch },
                ));
            }
            handles
        }
        ToolMode::Normal => Vec::new(),
    }
}

/// Raises, moves and retires the handles as selection, mode and the
/// part's own size change.
#[allow(clippy::too_many_arguments)]
fn dress_gizmo(
    mut commands: Commands,
    selected: Res<Selected>,
    mode: Res<ToolMode>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parts: Query<(&Transform, &Placed), Without<GizmoRoot>>,
    roots: Query<Entity, With<GizmoRoot>>,
    mut stamp: Local<(Option<Entity>, ToolMode, String)>,
) {
    let standing = selected.one().and_then(|part| parts.get(part).ok());
    let fresh = (
        selected.lead(),
        *mode,
        standing
            .map(|(_, record)| record.part.clone())
            .unwrap_or_default(),
    );
    if *stamp == fresh {
        if let Some((at, _)) = standing {
            for root in &roots {
                commands
                    .entity(root)
                    .insert(Transform::from_translation(at.translation));
            }
        }
        return;
    }
    *stamp = fresh;
    for root in &roots {
        commands.entity(root).despawn();
    }
    let Some((at, record)) = standing else {
        return;
    };
    let wanted = handles_for(*mode, record);
    if wanted.is_empty() {
        return;
    }
    let root = commands
        .spawn((
            GizmoRoot,
            Transform::from_translation(at.translation),
            Visibility::default(),
        ))
        .id();
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for (dir, foot, ramp, grip) in wanted {
        let material = materials.add(StandardMaterial {
            base_color: palette.shade(ramp, 0.85),
            unlit: true,
            ..default()
        });
        let handle = commands
            .spawn((
                Handle { dir, ramp, grip },
                Transform::from_translation(foot),
                Visibility::default(),
                ChildOf(root),
            ))
            .id();
        // The shaft runs out of the foot along the direction, the head
        // caps it. Everything on the arrow layer.
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(dir * 0.65)
                .with_scale(Vec3::splat(0.05) + dir.abs() * (1.3 - 0.05)),
            RenderLayers::layer(ARROW_LAYER),
            ChildOf(handle),
        ));
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(dir * 1.35).with_scale(Vec3::splat(0.14)),
            RenderLayers::layer(ARROW_LAYER),
            ChildOf(handle),
        ));
    }
}

fn cursor_ray(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), (With<Camera3d>, Without<GizmoCamera>)>,
) -> Option<Ray3d> {
    let window = windows.iter().next()?;
    let cursor = window.cursor_position()?;
    let (camera, camera_at) = cameras.iter().next()?;
    camera.viewport_to_world(camera_at, cursor).ok()
}

/// The parameter along `axis` (through `origin`) closest to the ray:
/// t = (e - b·f)/(1 - b²).
fn along_axis(ray: &Ray3d, origin: Vec3, axis: Vec3) -> Option<f32> {
    let toward = Vec3::from(ray.direction);
    let b = axis.dot(toward);
    let denominator = 1.0 - b * b;
    if denominator.abs() < 1e-4 {
        return None;
    }
    let w = ray.origin - origin;
    Some((w.dot(axis) - b * w.dot(toward)) / denominator)
}

fn ray_reach(ray: &Ray3d, point: Vec3) -> f32 {
    (point - ray.origin).dot(Vec3::from(ray.direction))
}

/// Dragging a handle: slides in MOVE (5cm steps), re-dimensions in
/// RESIZE (25cm steps, the far end standing still). Every change lands
/// in the part's record, and a resized body is rebuilt on the spot.
#[allow(clippy::too_many_arguments)]
fn work_gizmo(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    selected: Res<Selected>,
    mut drag: ResMut<GizmoDrag>,
    mut hot: ResMut<GizmoHot>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), (With<Camera3d>, Without<GizmoCamera>)>,
    handles: Query<(Entity, &Handle, &GlobalTransform)>,
    children: Query<&Children>,
    dyes: Query<&MeshMaterial3d<StandardMaterial>>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut parts: Query<(Entity, &mut Transform, &mut Placed), Without<Handle>>,
) {
    // Handles hang on ONE part; a set of parts is moved by its own handles at
    // the middle, which are the move handles and no others.
    let Some(part) = selected.one().or_else(|| selected.lead()) else {
        drag.0 = None;
        hot.0 = false;
        return;
    };
    let Some(ray) = cursor_ray(&windows, &cameras) else {
        return;
    };

    // Which handle the cursor rides.
    let mut touched: Option<(Vec3, Grip)> = None;
    for (_, handle, at) in &handles {
        let origin = at.translation();
        let Some(t) = along_axis(&ray, origin, handle.dir) else {
            continue;
        };
        if !(-0.1..=1.5).contains(&t) {
            continue;
        }
        let on_axis = origin + handle.dir * t;
        let miss =
            (ray.origin + Vec3::from(ray.direction) * ray_reach(&ray, on_axis) - on_axis).length();
        if miss < 0.18 {
            touched = Some((handle.dir, handle.grip));
        }
    }
    hot.0 = touched.is_some() || drag.0.is_some();

    // The ridden handle brightens; a live drag keeps its own lit.
    let lit_dir = drag
        .0
        .as_ref()
        .map(|state| state.dir)
        .or(touched.map(|(dir, _)| dir));
    for (entity, handle, _) in &handles {
        let lit = lit_dir == Some(handle.dir);
        if let Ok(kids) = children.get(entity) {
            for &kid in kids {
                if let Ok(dye) = dyes.get(kid)
                    && let Some(mut material) = materials.get_mut(&dye.0)
                {
                    let wanted = palette.shade(handle.ramp, if lit { 1.0 } else { 0.85 });
                    if material.base_color != wanted {
                        material.base_color = wanted;
                    }
                }
            }
        }
    }

    if buttons.just_pressed(MouseButton::Left)
        && let Some((dir, grip)) = touched
        && let Ok((_, transform, _)) = parts.get_mut(part)
        && let Some(t0) = along_axis(&ray, transform.translation, dir)
    {
        drag.0 = Some(DragState {
            dir,
            t0,
            start_at: transform.translation,
            grip,
        });
    }
    if !buttons.pressed(MouseButton::Left) {
        drag.0 = None;
        return;
    }
    let Some(state) = drag.0.as_ref() else {
        return;
    };
    let Ok((_, mut transform, mut record)) = parts.get_mut(part) else {
        return;
    };
    let Some(t) = along_axis(&ray, state.start_at, state.dir) else {
        return;
    };
    match state.grip {
        Grip::Slide => {
            // A sixteenth of a metre: the universal lattice's own step,
            // so a fine move can always be undone by a coarse snap.
            let step = ((t - state.t0) * 16.0).round() / 16.0;
            let was = transform.translation;
            transform.translation = state.start_at + state.dir * step;
            record.at = transform.translation.into();
            let moved = transform.translation - was;

            // Everything else chosen goes the same distance. The handle hangs on
            // one part, but a choice of several moves as one thing - that is
            // what choosing several is FOR.
            if moved.length_squared() > 0.0 {
                let others: Vec<Entity> = selected.iter().filter(|held| *held != part).collect();
                for other in others {
                    if let Ok((_, mut at, mut record)) = parts.get_mut(other) {
                        at.translation += moved;
                        record.at = at.translation.into();
                    }
                }
            }

            // And the marks it carries travel with it. A door slid along a wall
            // that left its routing mark behind would put the village's doorway
            // where the door used to be.
            if moved.length_squared() > 0.0 {
                let held: Vec<(Entity, Vec3, builder::Placed)> = parts
                    .iter()
                    .map(|(entity, at, record)| (entity, at.translation, record.clone()))
                    .collect();
                let carried = builder::carried_marks(
                    part,
                    was,
                    held.iter().map(|(e, at, record)| (*e, *at, record)),
                );
                for mark in carried {
                    if let Ok((_, mut mark_at, mut mark_record)) = parts.get_mut(mark) {
                        mark_at.translation += moved;
                        mark_record.at = mark_at.translation.into();
                    }
                }
            }
        }
        Grip::Pitch { p0 } => {
            // The drag is a ridge HEIGHT and the stored number is an angle:
            // pulling a ridge is what a roof looks like being made steeper,
            // while degrees are what a builder means by a pitch, so the handle
            // speaks the first and records the second.
            // A roof is pitched about its span and a gable about its width;
            // beyond that the gesture is the same, so the arithmetic is written
            // once and only the part it rebuilds differs.
            let (across, rebuild): (f32, &dyn Fn(f32) -> PartKind) =
                match builder::kind_from_name(&record.part) {
                    Some(PartKind::GableRoof(long, span, over, _)) => (span, &move |pitch| {
                        PartKind::GableRoof(long, span, over, pitch)
                    }),
                    Some(PartKind::Gable(long, _)) => {
                        (long, &move |pitch| PartKind::Gable(long, pitch))
                    }
                    _ => return,
                };
            let was = p0;
            let half = (across * 0.5).max(0.125);
            let rise = (half * p0.to_radians().tan() + (t - state.t0)).max(0.02);
            // Snapped in degrees rather than at the ridge, so two roofs meant
            // to match match exactly however differently they were dragged.
            let pitch = ((rise / half).atan().to_degrees() / builder::PITCH_STEP).round()
                * builder::PITCH_STEP;
            let pitch = pitch.clamp(10.0, 60.0);
            if (pitch - was).abs() < 1e-4 {
                return;
            }
            let made = rebuild(pitch);
            record.part = builder::part_name(&made);
            commands.entity(part).despawn_related::<Children>();
            builder::dress_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &made,
                &record,
                part,
                false,
            );
        }
        Grip::Rise { h0 } => {
            // Whole sixteenths, like every other pull, and never thinner than
            // one: a pad of no height is a pad nobody can see or click.
            let pull = ((t - state.t0) * 16.0).round() / 16.0;
            let high = (h0 + pull).clamp(0.0625, 8.0);
            let Some(PartKind::Foundation(w, d, was)) = builder::kind_from_name(&record.part)
            else {
                return;
            };
            if (high - was).abs() < 1e-4 {
                return;
            }
            let made = PartKind::Foundation(w, d, high);
            record.part = builder::part_name(&made);
            // A pad grows UPWARD from where it sits: its underside is the thing
            // resting on the ground, and a footing that sank as it grew would
            // have to be put back every time.
            transform.translation.y = state.start_at.y + (high - was) * 0.5;
            record.at = transform.translation.into();
            commands.entity(part).despawn_related::<Children>();
            builder::dress_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &made,
                &record,
                part,
                false,
            );
        }
        Grip::Rail { h0 } => {
            // In whole sixteenths, like everything else a hand pulls, and never
            // below a step's own height or above a chest.
            let pull = ((t - state.t0) * 16.0).round() / 16.0;
            let hand = (h0 + pull).clamp(0.375, 2.0);
            // The same handle serves a flight's rail and a flat one's.
            let made = match builder::kind_from_name(&record.part) {
                Some(PartKind::Stairs {
                    rise,
                    wide,
                    stone,
                    rail_stone,
                    hand: was,
                }) => {
                    if (hand - was).abs() < 1e-4 {
                        return;
                    }
                    PartKind::Stairs {
                        rise,
                        wide,
                        stone,
                        rail_stone,
                        hand,
                    }
                }
                Some(PartKind::Rail {
                    long,
                    hand: was,
                    stone,
                }) => {
                    if (hand - was).abs() < 1e-4 {
                        return;
                    }
                    PartKind::Rail { long, hand, stone }
                }
                _ => return,
            };
            record.part = builder::part_name(&made);
            commands.entity(part).despawn_related::<Children>();
            builder::dress_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &made,
                &record,
                part,
                false,
            );
        }
        Grip::Over { o0 } => {
            // The eaves reach out in whole units; the walls beneath and
            // the gables at the ends do not move at all.
            let pull = ((t - state.t0) * 16.0).round() / 16.0;
            let over = (o0 + pull).clamp(0.0, 3.0);
            let Some(PartKind::GableRoof(long, span, was, pitch)) =
                builder::kind_from_name(&record.part)
            else {
                return;
            };
            if (over - was).abs() < 1e-4 {
                return;
            }
            let made = PartKind::GableRoof(long, span, over, pitch);
            record.part = builder::part_name(&made);
            commands.entity(part).despawn_related::<Children>();
            builder::dress_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &made,
                &record,
                part,
                false,
            );
        }
        Grip::Size { on_x, w0, d0, was } => {
            // Pulling outward along the handle grows the dimension; the
            // far end stands still, so the centre walks half the growth.
            // One atom per step, always: resizing is fine work by nature.
            let pull = ((t - state.t0) * 16.0).round() / 16.0;
            let Some(kind) = builder::kind_from_name(&record.part) else {
                return;
            };
            let (w, d) = if on_x {
                ((w0 + pull).max(0.25), d0)
            } else {
                (w0, (d0 + pull).max(0.25))
            };
            // Measured rather than asked for: see `builder::extent_of`.
            let grown = 0.0;
            let made = match kind {
                PartKind::Wall(_) => PartKind::Wall(w),
                PartKind::Seg { high, lift, .. } => PartKind::Seg {
                    long: w,
                    high,
                    lift,
                },
                PartKind::Trim { stone, .. } => PartKind::Trim { long: w, stone },
                PartKind::Rail { hand, stone, .. } => PartKind::Rail {
                    long: w,
                    hand,
                    stone,
                },
                PartKind::Gable(_, pitch) => PartKind::Gable(w, pitch),
                PartKind::Beam(_, high, low) => PartKind::Beam(w, high, low),
                PartKind::Chimney(_) => PartKind::Chimney(w.max(0.0)),
                PartKind::Stairs {
                    stone,
                    rail_stone,
                    hand,
                    ..
                } => {
                    let (_, riser, tread) = builder::stair_rhythm(0.0);
                    let steps = (d / tread).round().clamp(2.0, 24.0);
                    PartKind::Stairs {
                        rise: steps * riser,
                        wide: w.max(0.375),
                        stone,
                        rail_stone,
                        hand,
                    }
                }
                PartKind::Ridge(_) => PartKind::Ridge(w),
                PartKind::Floor(..) => PartKind::Floor(w, d),
                PartKind::Foundation(_, _, high) => PartKind::Foundation(w, d, high),
                PartKind::Roof(..) => PartKind::Roof(w, d),
                PartKind::GableRoof(_, _, over, pitch) => PartKind::GableRoof(w, d, over, pitch),
                _ => return,
            };
            let fresh = builder::part_name(&made);
            if fresh == record.part {
                return;
            }
            // The part keeps the end the maker is NOT pulling: it moves by half
            // of however much it truly grew, which for a part whose handle asks
            // for something other than its own width - a chimney's drop, a
            // flight's rise - is nothing at all.
            let now = builder::extent_of(&made);
            let truly = if on_x { now.x - was.x } else { now.y - was.y };
            let _ = grown;
            transform.translation = state.start_at + state.dir * (truly * 0.5);
            record.at = transform.translation.into();
            record.part = fresh;
            // The body is rebuilt in place; the entity, and with it the
            // selection, stands.
            commands.entity(part).despawn_related::<Children>();
            builder::dress_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &made,
                &record,
                part,
                false,
            );
        }
    }
}
