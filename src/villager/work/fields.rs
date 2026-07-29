//! Fields and their crops.

use bevy::prelude::*;

use crate::rng::Rng;

/// A tilled field: crops rise from bare soil to harvest.
#[derive(Component, Debug)]
pub struct Field {
    pub growth: f32,
    pub farmer: Entity,
}

/// One row of crops, scaled up as the field grows.
#[derive(Component)]
pub struct CropRow {
    /// This stalk's full height at harvest.
    pub height: f32,
}

/// Raises a field's visible body - soil bed, furrow ridges, and stalks -
/// and returns the field entity. Used by the plough and by the save loader.
#[allow(clippy::too_many_arguments)]
pub fn raise_field(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rng: &mut Rng,
    at: Vec3,
    rotation: Quat,
    growth: f32,
    farmer: Entity,
) -> Entity {
    let field = commands
        .spawn((
            Name::new("A field"),
            Field { growth, farmer },
            Transform::from_translation(at).with_rotation(rotation),
            Visibility::default(),
            crate::hand::PickRadius(2.2),
            crate::hand::Rooted,
        ))
        .id();
    let bed = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::EARTH, 0.2),
        perceptual_roughness: 1.0,
        ..default()
    });
    let ridge = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::EARTH, 0.32),
        perceptual_roughness: 1.0,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(bed),
        Transform::from_xyz(0.0, 0.03, 0.0).with_scale(Vec3::new(3.8, 0.1, 3.1)),
        ChildOf(field),
    ));
    for lane in 0..4 {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(ridge.clone()),
            Transform::from_xyz(0.0, 0.1, lane as f32 * 0.8 - 1.2)
                .with_scale(Vec3::new(3.5, 0.09, 0.34)),
            ChildOf(field),
        ));
    }
    for lane in 0..4 {
        for slot in 0..6 {
            let shade = 0.5 + rng.range(0.0, 0.3);
            let crop = materials.add(StandardMaterial {
                base_color: crate::palette::shade(&crate::palette::GRASS, shade),
                perceptual_roughness: 0.9,
                ..default()
            });
            commands.spawn((
                CropRow {
                    height: rng.range(0.35, 0.62),
                },
                Mesh3d(cube.clone()),
                MeshMaterial3d(crop),
                Transform::from_xyz(
                    slot as f32 * 0.56 - 1.4 + rng.range(-0.1, 0.1),
                    0.2,
                    lane as f32 * 0.8 - 1.2 + rng.range(-0.06, 0.06),
                )
                .with_rotation(Quat::from_rotation_z(rng.range(-0.09, 0.09)))
                .with_scale(Vec3::new(0.07, 0.05, 0.07)),
                ChildOf(field),
            ));
        }
    }
    field
}

/// Crops grow on their own; a farmer's tending hurries them greatly.
pub(crate) fn grow_crops(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    mut fields: Query<(&mut Field, &Children)>,
    mut rows: Query<(&mut Transform, &CropRow)>,
) {
    let dt = time.delta_secs();
    // Rain is the farmer's other pair of hands - and the season is the
    // hand over both: winter fields sleep, and the village lives on what
    // the granary holds.
    let watered = weather.map_or(1.0, |w| 1.0 + w.intensity * 1.5);
    let seasonal = clock.season().growth();
    for (mut field, children) in &mut fields {
        field.growth = (field.growth + dt * watered * seasonal / 600.0).min(1.0);
        for &child in children {
            if let Ok((mut stalk, crop)) = rows.get_mut(child) {
                let height = crop.height * (0.1 + field.growth * 0.9);
                stalk.scale.y = height;
                stalk.translation.y = 0.14 + height * 0.5;
            }
        }
    }
}
