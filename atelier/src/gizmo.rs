//! The fine hand: three axis arrows for the last centimetre.
//!
//! The magnets and faces place a part well; this places it exactly. V
//! selects what the cursor touches, the arrows appear, and dragging one
//! slides the part along that axis alone in clean five-centimetre steps -
//! every snap deliberately silent while the arrows are in charge.

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;

use crate::Bench;
use crate::builder::{Hovered, Naming, Placed};
use crate::look::Palette;

/// The part currently wearing the arrows.
#[derive(Resource, Default)]
pub struct Selected(pub Option<Entity>);

/// A drag in progress: the axis, where along it the grip began, and the
/// part's position when it did.
#[derive(Resource, Default)]
pub struct GizmoDrag(pub Option<(Vec3, f32, Vec3)>);

/// True while the cursor is on an arrow or a drag is live - the bench's
/// ordinary click work stands aside for it.
#[derive(Resource, Default)]
pub struct GizmoHot(pub bool);

/// The arrows' root, parked at the selected part.
#[derive(Component)]
struct GizmoRoot;

/// One arrow, knowing its world axis.
#[derive(Component)]
struct GizmoArrow(Vec3);

/// The camera that draws the arrows over everything: it runs after the
/// main camera with a fresh depth buffer, so no wall can bury them.
#[derive(Component)]
struct GizmoCamera;

/// The render layer the arrows live on, seen only by their own camera.
const ARROW_LAYER: usize = 1;

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selected>()
            .init_resource::<GizmoDrag>()
            .init_resource::<GizmoHot>()
            .add_systems(Startup, raise_gizmo_camera)
            .add_systems(
                Update,
                (select_part, dress_gizmo, work_gizmo, ride_along).chain(),
            );
    }
}

/// The overlay camera: same eye as the bench camera, drawing only the
/// arrow layer, after everything, onto a cleared depth buffer.
fn raise_gizmo_camera(mut commands: Commands) {
    commands.spawn((
        GizmoCamera,
        Camera3d::default(),
        Camera {
            order: 1,
            clear_color: bevy::camera::ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(ARROW_LAYER),
        Transform::default(),
    ));
}

/// The overlay camera wears the bench camera's exact pose every frame.
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

/// V or a clean right CLICK (a right DRAG stays the camera's) selects
/// what the cursor touches; escape and vanishing both deselect.
#[allow(clippy::too_many_arguments)]
fn select_part(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mut swept: Local<f32>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    hovered: Res<Hovered>,
    parts: Query<(), With<Placed>>,
    mut selected: ResMut<Selected>,
) {
    if *bench != Bench::Builder || naming.0.is_some() {
        selected.0 = None;
        return;
    }
    if let Some(chosen) = selected.0
        && !parts.contains(chosen)
    {
        selected.0 = None;
    }
    // A right press starts a tally of how far the mouse swept; releasing
    // with the tally still tiny was a click, not an orbit.
    if buttons.just_pressed(MouseButton::Right) {
        *swept = 0.0;
    }
    if buttons.pressed(MouseButton::Right) {
        *swept += motion.delta.length();
    }
    let right_clicked = buttons.just_released(MouseButton::Right) && *swept < 4.0;
    if keys.just_pressed(KeyCode::KeyV) || right_clicked {
        selected.0 = match (selected.0, hovered.grab) {
            // Asking again on the selected part puts the arrows away.
            (Some(standing), Some(touched)) if standing == touched => None,
            (_, Some(touched)) => Some(touched),
            (_, None) => None,
        };
    }
    if keys.just_pressed(KeyCode::Escape) {
        selected.0 = None;
    }
}

/// Raises and parks the arrows whenever the selection changes, and keeps
/// them riding the part they belong to.
fn dress_gizmo(
    mut commands: Commands,
    selected: Res<Selected>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parts: Query<&Transform, (With<Placed>, Without<GizmoRoot>)>,
    mut roots: Query<(Entity, &mut Transform), With<GizmoRoot>>,
) {
    let at = selected.0.and_then(|part| parts.get(part).ok());
    match (at, roots.iter_mut().next()) {
        (Some(part_at), Some((_, mut root_at))) => {
            root_at.translation = part_at.translation;
        }
        (Some(part_at), None) => {
            let root = commands
                .spawn((
                    GizmoRoot,
                    Transform::from_translation(part_at.translation),
                    Visibility::default(),
                ))
                .id();
            let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
            for (axis, ramp) in [
                (Vec3::X, "cloth-red"),
                (Vec3::Y, "cloth-gold"),
                (Vec3::Z, "cloth-blue"),
            ] {
                let material = materials.add(StandardMaterial {
                    base_color: palette.shade(ramp, 0.85),
                    unlit: true,
                    ..default()
                });
                let arrow = commands
                    .spawn((
                        GizmoArrow(axis),
                        Transform::from_translation(axis * 0.7),
                        Visibility::default(),
                        ChildOf(root),
                    ))
                    .id();
                // The shaft, then the head at its far end - each stamped
                // onto the arrow layer, since layers do not inherit.
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_scale(Vec3::splat(0.05) + axis.abs() * (1.3 - 0.05)),
                    RenderLayers::layer(ARROW_LAYER),
                    ChildOf(arrow),
                ));
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(axis * 0.72).with_scale(Vec3::splat(0.14)),
                    RenderLayers::layer(ARROW_LAYER),
                    ChildOf(arrow),
                ));
            }
        }
        (None, Some((root, _))) => {
            commands.entity(root).despawn();
        }
        (None, None) => {}
    }
}

/// Where the cursor's ray runs, if it runs anywhere.
fn cursor_ray(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>,
) -> Option<Ray3d> {
    let window = windows.iter().next()?;
    let cursor = window.cursor_position()?;
    let (camera, camera_at) = cameras.iter().next()?;
    camera.viewport_to_world(camera_at, cursor).ok()
}

/// The parameter along `axis` (through `origin`) closest to the ray.
fn along_axis(ray: &Ray3d, origin: Vec3, axis: Vec3) -> Option<f32> {
    let toward = Vec3::from(ray.direction);
    let b = axis.dot(toward);
    let denominator = 1.0 - b * b;
    if denominator.abs() < 1e-4 {
        return None;
    }
    let w = ray.origin - origin;
    Some((w.dot(axis) - b * w.dot(toward)) / -denominator)
}

/// Dragging an arrow slides the part along that axis in 0.05 steps, and
/// writes the move into the part's record so the export tells the truth.
#[allow(clippy::too_many_arguments)]
fn work_gizmo(
    buttons: Res<ButtonInput<MouseButton>>,
    selected: Res<Selected>,
    mut drag: ResMut<GizmoDrag>,
    mut hot: ResMut<GizmoHot>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    arrows: Query<(&GizmoArrow, &GlobalTransform)>,
    mut parts: Query<(&mut Transform, &mut Placed)>,
) {
    let Some(part) = selected.0 else {
        drag.0 = None;
        hot.0 = false;
        return;
    };
    let Some(ray) = cursor_ray(&windows, &cameras) else {
        return;
    };

    // Which arrow the cursor rides, tested against a generous sleeve.
    let mut touched: Option<Vec3> = None;
    for (arrow, at) in &arrows {
        let origin = at.translation() - arrow.0 * 0.7;
        let Some(t) = along_axis(&ray, origin, arrow.0) else {
            continue;
        };
        if !(0.0..=1.45).contains(&t) {
            continue;
        }
        let on_axis = origin + arrow.0 * t;
        let miss =
            (ray.origin + Vec3::from(ray.direction) * ray_reach(&ray, on_axis) - on_axis).length();
        if miss < 0.14 {
            touched = Some(arrow.0);
        }
    }
    hot.0 = touched.is_some() || drag.0.is_some();

    if buttons.just_pressed(MouseButton::Left)
        && let Some(axis) = touched
        && let Ok((transform, _)) = parts.get_mut(part)
    {
        if let Some(t0) = along_axis(&ray, transform.translation, axis) {
            drag.0 = Some((axis, t0, transform.translation));
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        drag.0 = None;
        return;
    }
    if let Some((axis, t0, start)) = drag.0
        && let Ok((mut transform, mut record)) = parts.get_mut(part)
        && let Some(t) = along_axis(&ray, start, axis)
    {
        // Clean five-centimetre steps: fine, but never float dust.
        let step = ((t - t0) * 20.0).round() / 20.0;
        transform.translation = start + axis * step;
        record.at = transform.translation.into();
    }
}

/// How far along the ray the given point sits.
fn ray_reach(ray: &Ray3d, point: Vec3) -> f32 {
    (point - ray.origin).dot(Vec3::from(ray.direction))
}
