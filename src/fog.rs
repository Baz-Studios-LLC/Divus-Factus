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

/// The sea takes ONE sheet, at full weight.
///
/// A stack works on land because every sheet drapes the ground's own
/// shape and hangs parallel to it. Over water the sheets are FLAT, and a
/// flat sheet cut by a shelving seabed draws a contour line - six of them
/// drew six, and a low-poly seabed turned the six into a staircase
/// marching out from every shore. Nothing floats on the sea that needs
/// twelve metres of hiding, so the sea gets a single sheet carrying the
/// whole weight, and the only line it can draw is the waterline itself -
/// where the land's own veil takes over anyway.
const SEA_SHEET: f32 = 0.35;

/// Past this camera distance the land's stack collapses to one solid lid
/// at the top of the bank. The stack exists to give the veil's EDGE depth,
/// and from a quarter mile up that depth is a single pixel - but the six
/// sheets still cost six draws of every chunk in a view that is at its
/// widest exactly then. The soak put the whole difference at five and a
/// half milliseconds a frame: 48fps with the stack, 64 with the lid.
const LID_BEYOND: f32 = 350.0;

/// How far the lowest sheet floats over the ground it veils.
///
/// It was a tenth of a unit - "a hand's breadth", enough to win the depth
/// test against the ground while looking like it lay on it. That was true of
/// a world whose vertices were chunk-local, a few tens of units from their
/// own origin. On a round world every vertex is a world-space seat some six
/// thousand units from the planet's centre, and at that magnitude an `f32`
/// resolves to about seven ten-thousandths - so the ground and the sheet
/// stopped being reliably ordered and the whole landscape came out striped
/// where they traded places pixel by pixel. A unit and a half is still
/// nothing beside a tree, and it is two thousand times the precision floor.
const LOWEST_SHEET: f32 = 1.5;

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
pub struct Veil;

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<FogMaterial>::default())
            .init_resource::<FogMode>()
            // Only over a world that has a village in it. While the god
            // is still choosing where to plant the flag there is nothing
            // known and nothing to hide - and a player cannot pick
            // ground they are not allowed to look at.
            .init_resource::<VeilRising>()
            .add_systems(OnEnter(crate::GameState::Playing), the_veil_is_down)
            .add_systems(
                Update,
                (toggle_fog, drape_the_veil, follow_the_known, raise_the_veil)
                    .chain()
                    .run_if(in_state(crate::GameState::Playing)),
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

    /// The veil is drawn LAST of the see-through things, always.
    ///
    /// Blended meshes are sorted by how far their origins sit from the
    /// camera, and the sea's veil hangs a hand's breadth over a sea whose
    /// origin is the world's. Two origins that close sort by rounding
    /// error: turn the camera and the order flipped, and the water came
    /// out on top of the fog that was supposed to be hiding it. A bias
    /// this large is not a nudge, it is a statement - nothing transparent
    /// in this world is ever meant to be in front of the veil.
    fn depth_bias(&self) -> f32 {
        10_000.0
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
    mut cloth: Local<Option<(Handle<FogMaterial>, Handle<FogMaterial>)>>,
    mut was_lidded: Local<Option<bool>>,
    rising: Res<VeilRising>,
    rigs: Query<&crate::camera::CameraRig>,
    chunks: Query<
        (Entity, &Mesh3d, Option<&Children>, Has<WaterPlane>),
        Or<(With<TerrainChunk>, With<WaterPlane>)>,
    >,
    veils: Query<Entity, With<Veil>>,
) {
    if !mode.0 {
        if !veils.is_empty() {
            for veil in &veils {
                commands.entity(veil).despawn();
            }
        }
        *was_lidded = None;
        return;
    }
    // Stack or lid, by zoom - and when the answer changes, every veil
    // comes off so the right dress goes on. A frame of bare world at the
    // moment of switching would show; despawn and respawn land the same
    // frame, so nothing flickers.
    let lidded = rigs
        .iter()
        .next()
        .is_some_and(|rig| rig.distance > LID_BEYOND);
    if *was_lidded != Some(lidded) {
        for veil in &veils {
            commands.entity(veil).despawn();
        }
    }
    let fresh = *was_lidded != Some(lidded);
    *was_lidded = Some(lidded);
    // Two cloths, and both describe the whole world: one sheer, for
    // stacking over the land, and one at full weight for the sea's single
    // sheet. The holes in them are the same holes.
    let (cloth, deep) = cloth.get_or_insert_with(|| {
        let sheer = FogMaterial::default();
        let mut solid = FogMaterial::default();
        // What six sheer sheets come to, in one.
        solid.params.tint.w = 1.0 - (1.0 - sheer.params.tint.w).powi(SHEETS as i32);
        (materials.add(sheer), materials.add(solid))
    });
    // The rise is applied HERE, where both cloths and their own full
    // weights are known. Doing it in a system that walked every fog
    // material and wrote the default weight into each of them dragged
    // the solid cloth from nine tenths down to a third and left it
    // there - so the sea, and the single lid a zoomed-out world wears,
    // both went three times too thin and stayed that way.
    {
        let sheer_full = FogParams::default().tint.w;
        let solid_full = 1.0 - (1.0 - sheer_full).powi(SHEETS as i32);
        // The veil thins with altitude and is gone before the world becomes
        // a ball. It is a play-height instrument - what the village knows,
        // seen from where the village is watched - and the planet's own
        // patches have never honoured it, so carried aloft it stopped being
        // information and became scenery: its sea-sheet borrows the water
        // quad's mesh, and from three thousand up it read as an inexplicable
        // translucent square lying on the world.
        let aloft = rigs.iter().next().map_or(0.0, |rig| {
            ((rig.distance - 1_500.0) / 2_500.0).clamp(0.0, 1.0)
        });
        let standing = rising.risen * (1.0 - aloft);
        let (cloth, deep) = (cloth.clone(), deep.clone());
        if let Some(mut stuff) = materials.get_mut(&cloth) {
            stuff.params.tint.w = sheer_full * standing;
        }
        if let Some(mut stuff) = materials.get_mut(&deep) {
            stuff.params.tint.w = solid_full * standing;
        }
    }
    for (chunk, mesh, children, sea) in &chunks {
        let dressed = if fresh {
            // The children still hold this frame's despawned veils;
            // trusting them would dress nothing until next frame.
            0
        } else {
            children.map_or(0, |kids| {
                kids.iter().filter(|kid| veils.contains(*kid)).count()
            })
        };
        let wanted = if sea || lidded { 1 } else { SHEETS };
        if dressed >= wanted {
            continue;
        }
        for sheet in dressed..wanted {
            // The lowest sits a hand's breadth up, so it never fights the
            // ground it covers for the same pixel; the rest climb to the
            // top of the bank - and the lid goes straight to the top,
            // draping the ground's own shape so it draws no contours and
            // still stands over the trees.
            let lift = if sea {
                SEA_SHEET
            } else if lidded {
                LOWEST_SHEET + VEIL_HIGH
            } else {
                LOWEST_SHEET + VEIL_HIGH * (sheet as f32 / (SHEETS - 1) as f32)
            };
            commands.spawn((
                Veil,
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(if sea || lidded {
                    deep.clone()
                } else {
                    cloth.clone()
                }),
                // Lifted RADIALLY, not along world Y. The sheets copy the
                // chunk's own mesh, and those vertices are seated on the
                // sphere - so "up" is away from the planet's centre, and it
                // differs everywhere. A uniform scale about that centre
                // raises every vertex by `lift` along its own radial at
                // once: p' = centre + k(p - centre), which is exactly a
                // scale of k plus a translation of centre(1 - k). Lifting
                // along Y instead only worked at the reference point and
                // sheared the veil off the world everywhere else.
                {
                    let centre = crate::globe::planet_centre();
                    let k = (crate::terrain::PLANET_RADIUS + lift) / crate::terrain::PLANET_RADIUS;
                    Transform {
                        translation: centre * (1.0 - k),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::splat(k),
                    }
                },
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

/// How far the veil has risen since the world began, 0 to 1.
///
/// The fog is off while the god is choosing ground - a player cannot pick
/// somewhere they are not allowed to look at - so without this it would
/// simply snap on the instant the flag went in. It comes up instead, over
/// the same breath the village does: the world closing in around a place
/// that now has people in it and an edge to their knowledge.
#[derive(Resource)]
pub struct VeilRising {
    risen: f32,
}

impl Default for VeilRising {
    fn default() -> Self {
        // Fully up by default, so a restored world does not fade in every
        // time it loads and the veil is simply there, as it was.
        VeilRising { risen: 1.0 }
    }
}

/// Seconds for the veil to come all the way up behind a new village.
const VEIL_RISES_OVER: f32 = 4.0;

/// The veil starts down over ground nobody has settled yet.
fn the_veil_is_down(mut rising: ResMut<VeilRising>, chosen: Res<crate::villager::ChosenGround>) {
    // Only for a world founded this session. A load already has its fog.
    if chosen.0.is_some() {
        rising.risen = 0.0;
    }
}

/// And comes up behind them. Only the FRACTION lives here; the weight it
/// is a fraction of belongs to each cloth, and `drape_the_veil` applies
/// it where both are known.
fn raise_the_veil(time: Res<Time>, mut rising: ResMut<VeilRising>) {
    if rising.risen >= 1.0 {
        return;
    }
    rising.risen = (rising.risen + time.delta_secs() / VEIL_RISES_OVER).min(1.0);
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
