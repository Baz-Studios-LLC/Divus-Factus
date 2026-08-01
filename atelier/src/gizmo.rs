//! The tool modes and the fine hand.
//!
//! A mode bar rides the top of the window, the way the big 3D programs
//! do it: NORMAL places and grabs, MOVE puts translate arrows on what
//! you click, RESIZE puts end-handles on any sized primitive and drags
//! its dimensions in quarter-metre steps. Tab walks the modes.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::Bench;
use crate::builder::{self, Hovered, Naming, PartKind, Placed};
use crate::look::Palette;

/// Which tool the mouse is.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ToolMode {
    #[default]
    Normal,
    Move,
    Resize,
}

/// The part currently wearing handles.
#[derive(Resource, Default)]
pub struct Selected(pub Option<Entity>);

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
    Size { on_x: bool, w0: f32, d0: f32 },
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
                (walk_modes, select_part, dress_gizmo, work_gizmo, ride_along).chain(),
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
            ToolMode::Resize => ToolMode::Normal,
        };
    }
    if mode.is_changed() && *mode == ToolMode::Normal {
        selected.0 = None;
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
    mut selected: ResMut<Selected>,
) {
    if dims.0.is_some() {
        return;
    }
    if *bench != Bench::Builder || naming.0.is_some() || *mode == ToolMode::Normal {
        selected.0 = None;
        return;
    }
    if let Some(chosen) = selected.0
        && !parts.contains(chosen)
    {
        selected.0 = None;
    }
    if buttons.just_pressed(MouseButton::Left)
        && !hot.0
        && let Some(touched) = hovered.grab
    {
        selected.0 = Some(touched);
    }
    if keys.just_pressed(KeyCode::Escape) {
        selected.0 = None;
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
        ToolMode::Move => vec![
            (Vec3::X, Vec3::ZERO, "cloth-red", Grip::Slide),
            (Vec3::Y, Vec3::ZERO, "cloth-gold", Grip::Slide),
            (Vec3::Z, Vec3::ZERO, "cloth-blue", Grip::Slide),
        ],
        ToolMode::Resize => {
            let sized = builder::kind_from_name(&record.part).and_then(|kind| match kind {
                PartKind::Wall(long) => Some((long, 0.0, false)),
                // The pieces a punch leaves are walls too, and stretch
                // like them - only their height and lift stay put.
                PartKind::Seg { long, .. } => Some((long, 0.0, false)),
                PartKind::Trim { long, .. } => Some((long, 0.0, false)),
                PartKind::Gable(long) => Some((long, 0.0, false)),
                PartKind::Ridge(long) => Some((long, 0.0, false)),
                PartKind::Floor(w, d) => Some((w, d, true)),
                PartKind::Foundation(w, d) => Some((w, d, true)),
                PartKind::Roof(w, d) => Some((w, d, true)),
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
                        },
                    ));
                }
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
    let standing = selected.0.and_then(|part| parts.get(part).ok());
    let fresh = (
        selected.0,
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
    mut parts: Query<(&mut Transform, &mut Placed), Without<Handle>>,
) {
    let Some(part) = selected.0 else {
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
        && let Ok((transform, _)) = parts.get_mut(part)
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
    let Ok((mut transform, mut record)) = parts.get_mut(part) else {
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
            transform.translation = state.start_at + state.dir * step;
            record.at = transform.translation.into();
        }
        Grip::Size { on_x, w0, d0 } => {
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
            let grown = if on_x { w - w0 } else { d - d0 };
            let made = match kind {
                PartKind::Wall(_) => PartKind::Wall(w),
                PartKind::Seg { high, lift, .. } => PartKind::Seg {
                    long: w,
                    high,
                    lift,
                },
                PartKind::Trim { stone, .. } => PartKind::Trim { long: w, stone },
                PartKind::Gable(_) => PartKind::Gable(w),
                PartKind::Ridge(_) => PartKind::Ridge(w),
                PartKind::Floor(..) => PartKind::Floor(w, d),
                PartKind::Foundation(..) => PartKind::Foundation(w, d),
                PartKind::Roof(..) => PartKind::Roof(w, d),
                _ => return,
            };
            let fresh = builder::part_name(&made);
            if fresh == record.part {
                return;
            }
            transform.translation = state.start_at + state.dir * (grown * 0.5);
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
