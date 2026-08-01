//! Carrying: logs on shoulders, stone in arms, sacks on backs.

use bevy::prelude::*;

/// Timber in someone's arms, on its way somewhere.
///
/// The whole economy is visible now: a log exists in the world from the moment
/// the tree falls to the moment it becomes wall — on a shoulder, on the pile,
/// or nailed into a frame. Nothing teleports.
#[derive(Component, Debug)]
pub struct CarryingWood {
    pub amount: f32,
}

/// The visible log in a carrier's arms.
#[derive(Component)]
pub struct WoodLoad;

/// Stone in someone's arms, on its way to a foundation.
#[derive(Component, Debug)]
pub struct CarryingStone {
    /// Clay brick rather than stone: the refund must go back to the
    /// right pile if the errand is abandoned.
    pub clay: bool,
}

/// Puts a stone block in someone's arms.
pub(crate) fn shoulder_stone(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    carrier: Entity,
) {
    commands.spawn((
        WoodLoad,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::STONE, 0.5),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.05, -0.38).with_scale(Vec3::new(0.45, 0.32, 0.34)),
        ChildOf(carrier),
    ));
    commands.entity(carrier).insert(crate::creature::Laden);
}

/// Puts a log in someone's arms.
pub(crate) fn shoulder_wood(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    carrier: Entity,
) {
    commands.spawn((
        WoodLoad,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::WOOD, 0.4),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.05, -0.38).with_scale(Vec3::new(0.95, 0.2, 0.2)),
        ChildOf(carrier),
    ));
    commands.entity(carrier).insert(crate::creature::Laden);
}

/// Takes the log back out of their arms.
pub(crate) fn shed_wood(
    commands: &mut Commands,
    carrier: Entity,
    children: &Query<&Children>,
    loads: &Query<Entity, With<WoodLoad>>,
) {
    if let Ok(kids) = children.get(carrier) {
        for &child in kids {
            if loads.contains(child) {
                commands.entity(child).despawn();
            }
        }
    }
    commands
        .entity(carrier)
        .try_remove::<crate::creature::Laden>();
}

/// Burdens survive a body rebuild. Coming of age and growing old tear down
/// every child - the carried prop included - while `Laden` and the carrying
/// components live on the root and keep the ledger true. This system
/// notices a laden carrier with empty hands and puts the prop back,
/// whoever despawned it and for whatever reason.
pub(crate) fn redress_carriers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    carriers: Query<
        (Entity, Option<&Children>, Has<CarryingStone>),
        (
            With<crate::creature::Laden>,
            Or<(With<CarryingWood>, With<CarryingStone>)>,
            Without<crate::creature::Corpse>,
        ),
    >,
    loads: Query<(), With<WoodLoad>>,
) {
    for (carrier, children, stone) in &carriers {
        let empty_handed = !children.is_some_and(|kids| kids.iter().any(|kid| loads.contains(kid)));
        if !empty_handed {
            continue;
        }
        if stone {
            shoulder_stone(&mut commands, &mut meshes, &mut materials, carrier);
        } else {
            shoulder_wood(&mut commands, &mut meshes, &mut materials, carrier);
        }
    }
}
