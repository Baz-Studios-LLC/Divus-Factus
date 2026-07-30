//! People markers: press P and every villager wears a slowly turning
//! golden lozenge above their head, readable from any height — the way a
//! god keeps track of small people from a long way up.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::creature::Corpse;
use crate::villager::Villager;

pub struct MarkersPlugin;

impl Plugin for MarkersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MarkerMode>().add_systems(
            Update,
            (
                // Only the played game listens for the key: the title's veil
                // is translucent now, and a stray keystroke on the menu must
                // not redecorate the world behind it.
                toggle_markers.run_if(in_state(crate::GameState::Playing)),
                tend_markers,
                spin_markers,
            )
                .chain(),
        );
    }
}

/// Whether the markers are showing.
#[derive(Resource, Default)]
pub struct MarkerMode(pub bool);

/// One villager's overhead lozenge.
#[derive(Component)]
struct PeopleMarker;

fn toggle_markers(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<MarkerMode>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    if !keys.just_pressed(KeyCode::KeyP) {
        return;
    }
    mode.0 = !mode.0;
    if mode.0 {
        notices.write(crate::ui::Notice::new(
            "The god marks every soul - press P again to look away".to_string(),
        ));
    }
}

/// Keeps every living villager wearing a marker while the mode is on —
/// newborns included — and sweeps them all away when it goes off.
fn tend_markers(
    mut commands: Commands,
    mode: Res<MarkerMode>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shared: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    markers: Query<Entity, With<PeopleMarker>>,
    bare: Query<(Entity, Option<&Children>), (With<Villager>, Without<Corpse>)>,
    is_marker: Query<(), With<PeopleMarker>>,
) {
    if !mode.0 {
        for marker in &markers {
            commands.entity(marker).despawn();
        }
        return;
    }
    let (mesh, material) = shared
        .get_or_insert_with(|| {
            (
                meshes.add(Cuboid::new(0.55, 0.55, 0.55)),
                materials.add(StandardMaterial {
                    base_color: crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.9),
                    emissive: LinearRgba::from(crate::palette::shade(
                        &crate::palette::CLOTH_GOLD,
                        0.9,
                    )) * 3.0,
                    unlit: false,
                    ..default()
                }),
            )
        })
        .clone();
    for (villager, children) in &bare {
        let wearing = children
            .into_iter()
            .flatten()
            .any(|child| is_marker.get(*child).is_ok());
        if wearing {
            continue;
        }
        commands.spawn((
            Name::new("A marker"),
            PeopleMarker,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(0.0, 2.6, 0.0).with_scale(Vec3::new(1.0, 1.45, 1.0)),
            NotShadowCaster,
            ChildOf(villager),
        ));
    }
}

/// The lozenges turn together, slowly - alive, not frantic.
fn spin_markers(time: Res<Time>, mut markers: Query<&mut Transform, With<PeopleMarker>>) {
    let spin = time.elapsed_secs() * 1.1;
    for mut transform in &mut markers {
        transform.rotation = Quat::from_rotation_y(spin) * Quat::from_rotation_z(0.785);
    }
}
