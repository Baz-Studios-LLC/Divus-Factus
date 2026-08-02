//! The fog of the unknown world.
//!
//! The village knows a home circle and whatever pockets its explorers have
//! walked back from. That knowledge already governs where work may be
//! looked for - a forester will not go to woods nobody has found - but it
//! was invisible, and a rule you cannot see is a rule you cannot judge.
//!
//! The veil is drawn on a COPY of each terrain chunk's own mesh, lifted a
//! hand's breadth above it. A flat plane would be cut open by every hill;
//! the terrain's own shape is already the right shape. Nothing is stored
//! and nothing is painted: the shader measures each pixel against the
//! handful of circles the village actually knows.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::terrain::{TerrainChunk, WaterPlane};
use crate::villager::explore::KnownWorld;

const SHADER_PATH: &str = "shaders/fog.wgsl";

/// As many pockets as the shader's uniform holds. Explorers bring back far
/// fewer than this; the oldest simply stop being drawn if they ever do not.
const MAX_POCKETS: usize = 128;

/// How high the veil stands, and in how many sheets. The tallest tree in
/// the world is a shade over eight metres, so a bank that reaches twelve
/// swallows the woods whole.
///
/// A single sheet on the ground was the first try and the trees stood
/// straight through it. One sheet at treetop height would hide them, but a
/// flat lid twelve metres up reads as a floating pane and slides against
/// the ground it belongs to as the camera turns. A stack has real depth:
/// its edge is a wall of mist, and each sheet drapes the terrain's own
/// shape, so the bank rises and falls with the land under it.
const SHEETS: usize = 6;
const VEIL_HIGH: f32 = 12.0;

/// Whether the veil is up. It is, from the first frame: the world a
/// village has actually walked is the world the game is about, and the
/// rest of the map is scenery the player was never meant to be reading.
/// `DIVUS_FACTUS_FOG=0` lifts it at startup, for photographing the whole
/// world without reaching for the key.
#[derive(Resource)]
pub struct FogMode(pub bool);

impl Default for FogMode {
    fn default() -> Self {
        FogMode(!std::env::var("DIVUS_FACTUS_FOG").is_ok_and(|dial| dial == "0"))
    }
}

/// The veil laid over one terrain chunk.
#[derive(Component)]
struct Veil;

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<FogMaterial>::default())
            .init_resource::<FogMode>()
            .add_systems(
                Update,
                (toggle_fog, drape_the_veil, follow_the_known).chain(),
            );
    }
}

/// Uniform block handed to the fog shader.
#[derive(Clone, ShaderType, Debug)]
pub struct FogParams {
    /// The veil's colour; alpha is how heavy it gets at its thickest.
    pub tint: Vec4,
    /// The home ground: xyz its centre, w its radius.
    pub home: Vec4,
    /// x how many pockets are live, y how many metres the edge takes.
    pub dials: Vec4,
    /// Each known pocket: xyz its centre, w its radius.
    pub pockets: [Vec4; MAX_POCKETS],
}

impl Default for FogParams {
    fn default() -> Self {
        FogParams {
            // A cold slate blue rather than black: unknown ground should
            // read as unlit distance, not as a hole cut in the world.
            // Each sheet is thin; six of them stacked come to nine parts
            // in ten, which is dark enough to hide a wood and light enough
            // that the bank's own depth still shows at its edge.
            tint: Vec4::new(0.05, 0.07, 0.11, 0.32),
            home: Vec4::new(0.0, 0.0, 0.0, 170.0),
            dials: Vec4::new(0.0, 9.0, 0.0, 0.0),
            pockets: [Vec4::ZERO; MAX_POCKETS],
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct FogMaterial {
    #[uniform(0)]
    pub params: FogParams,
}

impl Material for FogMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

fn toggle_fog(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    mut mode: ResMut<FogMode>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    if !keymap.just_pressed(&keys, crate::keymap::Deed::Fog) {
        return;
    }
    mode.0 = !mode.0;
    let cap = crate::keymap::key_name(keymap.key(crate::keymap::Deed::Fog)).unwrap_or("the key");
    notices.write(crate::ui::Notice::new(if mode.0 {
        format!("Only the ground your people know - press {cap} to lift the veil")
    } else {
        "The whole world again".to_string()
    }));
}

/// Gives every loaded chunk a veil while the fog is up, and takes them all
/// away when it comes down. A veil is a CHILD of its chunk, so a chunk
/// rebuilt by a levelled plot takes its veil with it and this puts a fresh
/// one on the replacement.
///
/// The sea gets one too. The lakebed's sheets hang UNDER the water, which
/// is drawn straight over them - so an unexplored lake sat there in plain
/// daylight with the fog stacked politely on the mud beneath it.
fn drape_the_veil(
    mut commands: Commands,
    mode: Res<FogMode>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut cloth: Local<Option<Handle<FogMaterial>>>,
    chunks: Query<(Entity, &Mesh3d, Option<&Children>), Or<(With<TerrainChunk>, With<WaterPlane>)>>,
    veils: Query<Entity, With<Veil>>,
) {
    if !mode.0 {
        if !veils.is_empty() {
            for veil in &veils {
                commands.entity(veil).despawn();
            }
        }
        return;
    }
    // One material for every chunk: the uniform describes the whole world,
    // not any one piece of it.
    let cloth = cloth.get_or_insert_with(|| materials.add(FogMaterial::default()));
    for (chunk, mesh, children) in &chunks {
        let dressed = children.map_or(0, |kids| {
            kids.iter().filter(|kid| veils.contains(*kid)).count()
        });
        if dressed >= SHEETS {
            continue;
        }
        for sheet in dressed..SHEETS {
            // The lowest sits a hand's breadth up, so it never fights the
            // ground it covers for the same pixel; the rest climb to the
            // top of the bank.
            let lift = 0.12 + VEIL_HIGH * (sheet as f32 / (SHEETS - 1) as f32);
            commands.spawn((
                Veil,
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(cloth.clone()),
                Transform::from_translation(Vec3::Y * lift),
                // The veil is not a THING. Six sheets hanging over the
                // world threw six sheets of shadow onto the ground they
                // were meant to be hiding, and the known island - the one
                // place the veil never covers - went darkest of all,
                // because it lay under all six.
                NotShadowCaster,
                NotShadowReceiver,
                ChildOf(chunk),
            ));
        }
    }
}

/// Keeps the veil's holes where the village's knowledge actually is.
fn follow_the_known(
    mode: Res<FogMode>,
    known: Option<Res<KnownWorld>>,
    mut materials: ResMut<Assets<FogMaterial>>,
) {
    if !mode.0 {
        return;
    }
    let Some(known) = known else {
        return;
    };
    // The knowledge changes when an expedition comes home, which is rare -
    // but the toggle can come up at any time, so the first frame after it
    // does has to write the uniform whatever the resource says.
    if !known.is_changed() && !mode.is_changed() {
        return;
    }
    let live = known.pockets.len().min(MAX_POCKETS);
    for (_, material) in materials.iter_mut() {
        material.params.home = known.centre.extend(known.radius);
        material.params.dials.x = live as f32;
        for (slot, pocket) in known.pockets.iter().take(live).enumerate() {
            material.params.pockets[slot] = pocket.at.extend(pocket.radius);
        }
    }
}
