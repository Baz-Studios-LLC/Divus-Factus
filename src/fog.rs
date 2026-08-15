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

/// How high the veil's bank of mist stands.
///
/// The tallest current tree is a shade over eight metres; sixteen gives the
/// shroud headroom for roofs and future scenery without letting silhouettes
/// poke through its top.
///
/// It was set to ZERO, and this comment was left standing over it. A bank of
/// no height is a skin painted on the terrain, which every tree and roof
/// stands clean on top of - and once it could not occlude anything, hiding
/// the scenery became a second, separate job done by testing each mesh's
/// anchor point against the known circle, which is how whole groves came to
/// float in the dark. One mechanism, restored: the veil is tall enough to
/// bury a wood, and `VEIL_TAPER` is what keeps its edge from showing.
const VEIL_HIGH: f32 = 16.0;

/// How many metres the bank takes to climb from the ground to its full
/// height, going outward from the edge of what is known.
///
/// Brett: "keep it at 16m but taper it to the ground, that way it covers
/// everything but looks seamless because it would taper down and slightly
/// clip through the ground." Long enough to read as a slope rather than a
/// wall; short enough that the wood a step beyond the boundary is already
/// buried.
const VEIL_TAPER: f32 = 30.0;

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
                (
                    toggle_fog.run_if(in_state(crate::GameState::Playing)),
                    // NOT state-gated, alone among these. Chunks are born
                    // hidden and this is what reveals them, so on the title
                    // screen and while the god is still choosing ground —
                    // where there is no village, nothing known, and nothing
                    // to hide — it is the only thing standing between the
                    // player and an empty world.
                    drape_the_veil,
                    follow_the_known.run_if(in_state(crate::GameState::Playing)),
                    raise_the_veil.run_if(in_state(crate::GameState::Playing)),
                )
                    .chain()
                    // AFTER the ground streams, or the veil is always one
                    // frame behind it. Twenty chunks arrive in a frame while
                    // the god is moving, and dressing them next frame meant
                    // twenty tiles of unveiled world in every frame of a pan
                    // or a zoom - not a flicker, a permanent scattering of
                    // bright squares for as long as the view kept changing.
                    // Ordered after the spawn, the veil and the ground it
                    // hides reach the renderer in the same frame.
                    .after(crate::terrain::TerrainSet),
            );
    }
}

/// The veil's colour, in linear light: a cold slate blue rather than black,
/// because unknown ground should read as unlit distance and not as a hole cut
/// in the world.
///
/// Public because the PLANET wears the same fog of war — painted into its
/// patches rather than draped in cloths, since from orbit there is no cloth
/// small enough — and the two must be the same colour or the veil visibly
/// changes shade as the god climbs. One fact, one place.
pub(crate) const VEIL_TINT: [f32; 3] = [0.05, 0.07, 0.11];

/// Uniform block handed to the fog shader.
#[derive(Clone, ShaderType, Debug)]
pub struct FogParams {
    /// The veil's colour; alpha is how heavy it gets at its thickest.
    pub tint: Vec4,
    /// The home ground: xyz its centre, w its radius.
    pub home: Vec4,
    /// x how many pockets are live, y how many metres the edge takes.
    pub dials: Vec4,
    /// The planet: xyz its centre, w its radius.
    ///
    /// The veil is drawn on the BENT world, and what it knows is written in
    /// FLAT ground coordinates - so the shader has to undo the bend before it
    /// can ask whether a pixel is known. Near the village the two agree closely
    /// enough that nobody noticed for months; walk to the far side of the world
    /// and they have nothing to do with one another. Brett: "if you go to the
    /// other side of the world to plant your flag it works but the fog of war
    /// doesnt clear around the settlement."
    pub planet: Vec4,
    /// Each known pocket: xyz its centre, w its radius.
    pub pockets: [Vec4; MAX_POCKETS],
}

impl Default for FogParams {
    fn default() -> Self {
        FogParams {
            // Full unknownness is solid. Alpha drives the veil's rising and
            // dithered discovery edge, not translucency through the world.
            tint: Vec4::new(VEIL_TINT[0], VEIL_TINT[1], VEIL_TINT[2], 1.0),
            home: Vec4::new(0.0, 0.0, 0.0, 170.0),
            // z is the bank's physical height above its terrain mesh, and w
            // is how far it takes to climb to it - the taper that keeps the
            // inner edge under the ground instead of standing on it.
            dials: Vec4::new(0.0, 9.0, VEIL_HIGH, VEIL_TAPER),
            planet: crate::globe::planet_centre().extend(crate::terrain::PLANET_RADIUS),
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
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // The shader supplies full alpha for unknown ground, so this still
        // hides everything behind the shroud. Keeping it in the transparent
        // pass matters at god-height: the opaque pass turned the planet into a
        // pale solid disc instead of letting the sky's finishing pass treat the
        // veil like the atmosphere it is.
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        0.0
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
    state: Res<State<crate::GameState>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut cloth: Local<Option<Handle<FogMaterial>>>,
    rising: Res<VeilRising>,
    known: Option<Res<crate::villager::explore::KnownWorld>>,
    chunks: Query<
        (
            Entity,
            &Mesh3d,
            Option<&Children>,
            Has<WaterPlane>,
            &Visibility,
            Has<crate::terrain::ParkedChunk>,
            Option<&TerrainChunk>,
        ),
        Or<(With<TerrainChunk>, With<WaterPlane>)>,
    >,
    veils: Query<(Entity, &Visibility), With<Veil>>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("fog: drape_the_veil");
    // Chunks are born hidden so that no unveiled ground is ever seen. Whoever
    // decides they may be seen has to be whoever knows the veil is on them —
    // which is this system, and, when there is no veil to wait for, this
    // branch. Without it a lifted fog would leave a world of hidden chunks.
    let reveal = |commands: &mut Commands, chunk: Entity, showing: &Visibility| {
        if *showing == Visibility::Hidden {
            // try_insert: the Title click tears chunks down mid-frame, and
            // dressing a chunk that died is skippable, not fatal.
            commands.entity(chunk).try_insert(Visibility::Inherited);
        }
    };

    // No veil wanted — the key is off, or there is no village yet to have a
    // knowledge to draw. Take any cloths down and let the world be seen.
    if !mode.0 || *state.get() != crate::GameState::Playing {
        if !veils.is_empty() {
            for (veil, _) in &veils {
                commands.entity(veil).try_despawn();
            }
        }
        for (chunk, _, _, _, showing, parked, _) in &chunks {
            if parked {
                continue;
            }
            reveal(&mut commands, chunk, showing);
        }
        return;
    }
    // ONE cloth, at the top of the bank, whatever the zoom. There used to be
    // two - a sheer one stacked six deep over the land, and a full-weight one
    // for the sea and for the lid a zoomed-out world wore - plus the machinery
    // to swap between them without a bare frame showing. All of it is gone.
    // The stack existed to give the veil's edge some depth, and it did, but
    // looked along from a low camera you could COUNT the sheets: six pale
    // contour lines lying across the distance. The flat world's distance fog
    // used to dissolve them before they could be resolved; the round world does
    // not have that fog and does not want it.
    //
    // So the sheet is the top of a slab now, and the shader takes its weight
    // from how far the ray travels through that slab - squarely from above, a
    // long way at a graze. The far edge thickens into a wall by itself, which
    // is what the stack was faking, and it costs one draw per chunk instead of
    // six.
    let cloth = cloth.get_or_insert_with(|| materials.add(FogMaterial::default()));
    {
        // The veil does NOT thin with altitude. It used to, back when the
        // planet's own patches painted no veil at all: carried aloft the
        // cloths stopped being information and became scenery, a translucent
        // slate quadrilateral lying over an otherwise open world with the
        // loaded ring's own straight edge for a border. Now the patches wear
        // the same fog in the same colour (`globe::PlanetMaterial`), so the
        // cloth over the near ground and the paint beyond it are one
        // continuous shroud, and fading either of them would put the seam
        // back. What the village has not walked is hidden from every height.
        let full = FogParams::default().tint.w;
        let cloth = cloth.clone();
        if let Some(mut stuff) = materials.get_mut(&cloth) {
            stuff.params.tint.w = full * rising.risen;
        }
    }
    for (chunk, mesh, children, sea, showing, parked, ground) in &chunks {
        if parked {
            continue;
        }
        // A CHUNK NOBODY HAS SEEN ANY PART OF IS NOT DRAWN AT ALL.
        //
        // Brett: "The fully rendered area is higher then the LOD and it makes
        // it so you can see under the veil at a distance." Chunks and patches
        // ask the same height function but not at the same resolution - a
        // patch interpolates thirty-two cells across ground the chunk renders
        // in full - so the fine surface rides above the coarse one, and from a
        // low angle you look straight under the raised edge of a chunk that is
        // wholly unknown anyway. They were slabs of daylight ground floating
        // over the veiled planet.
        //
        // Nothing is lost by dropping them: the planet's own patch is standing
        // under every one, wearing the same veil in the same colour, so the
        // ground still reads as hidden country. It is also the cull Brett
        // asked for - "we sholod still try to cull trees and whatnot if under
        // the veil to save resouces" - done at the one granularity that takes
        // the trees, the grass, the boulders and the terrain together, since
        // every one of them is a child of this entity.
        // NOT WHILE THE VEIL IS STILL COMING UP. The fog rises over the first
        // moments of a founding rather than snapping on, and during that rise
        // the bank is too thin to draw at all - so culling then does not hide
        // a chunk UNDER something, it just takes a bite out of an open world.
        // The founding shot came out as a ragged island with chunk corners
        // for a coastline. A chunk may only vanish once the thing that would
        // have covered it is actually standing.
        if rising.risen >= 0.999
            && let (Some(known), Some(ground)) = (known.as_ref(), ground)
            && !known.knows_flat(
                (ground.coord.x as f32 + 0.5) * crate::terrain::CHUNK_SIZE,
                (ground.coord.y as f32 + 0.5) * crate::terrain::CHUNK_SIZE,
                // Half the chunk's diagonal: ANY corner being known keeps the
                // whole chunk, so this never hides ground somebody can see.
                crate::terrain::CHUNK_SIZE * std::f32::consts::SQRT_2 * 0.5,
            )
        {
            if *showing != Visibility::Hidden {
                commands.entity(chunk).try_insert(Visibility::Hidden);
            }
            // BUT ITS VEIL STAYS UP. Brett: "What if the veil renders farther
            // out then the land by a chunk or two, then the rendered veil will
            // over lap the non rendered veil." Hiding the chunk would take its
            // veil down with it - the cloth is a child of the ground it hides -
            // and the bank would stop dead at the frontier, exactly where it is
            // most needed. `Visibility::Visible` shows a child whatever its
            // parent is doing, so the land goes and the shroud over it stays,
            // running on across every loaded chunk until it settles onto the
            // patches' own painted veil. The two overlap instead of meeting.
            if let Some(kids) = children {
                for kid in kids.iter() {
                    if let Ok((veil, showing)) = veils.get(kid)
                        && *showing != Visibility::Visible
                    {
                        commands.entity(veil).try_insert(Visibility::Visible);
                    }
                }
            }
            continue;
        }
        let dressed = children.map_or(0, |kids| {
            kids.iter().filter(|kid| veils.contains(*kid)).count()
        });
        // Decent, and may be seen.
        if dressed > 0 {
            reveal(&mut commands, chunk, showing);
            // And its cloth goes back under the chunk's own command. A veil
            // forced `Visible` while its ground was culled would otherwise
            // stay forced for the rest of the world's life, outliving the
            // reason - and would hang there alone the next time this chunk
            // was parked for a trip to globe view.
            if let Some(kids) = children {
                for kid in kids.iter() {
                    if let Ok((veil, showing)) = veils.get(kid)
                        && *showing == Visibility::Visible
                    {
                        commands.entity(veil).try_insert(Visibility::Inherited);
                    }
                }
            }
            continue;
        }
        {
            let _ = sea;
            commands.spawn((
                Veil,
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(cloth.clone()),
                // The vertex shader lifts this exact terrain copy according
                // to its explored-state distance field. Known ground stays at
                // ground level, the frontier rises smoothly, and unknown land
                // reaches the full bank height - one uninterrupted surface.
                Transform::IDENTITY,
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

/// The veil is fully opaque from the first frame.
fn the_veil_is_down(mut rising: ResMut<VeilRising>) {
    rising.risen = 1.0;
}

fn raise_the_veil(_time: Res<Time>, mut rising: ResMut<VeilRising>) {
    rising.risen = 1.0;
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
        material.params.planet =
            crate::globe::planet_centre().extend(crate::terrain::PLANET_RADIUS);
        material.params.dials.x = live as f32;
        for (slot, pocket) in known.pockets.iter().take(live).enumerate() {
            material.params.pockets[slot] = pocket.at.extend(pocket.radius);
        }
    }
}
