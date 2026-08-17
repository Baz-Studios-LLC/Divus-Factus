//! The fog of the unknown world.
//!
//! The village knows a home circle and whatever pockets its explorers have
//! walked back from. That knowledge already governs where work may be
//! looked for - a forester will not go to woods nobody has found - but it
//! was invisible, and a rule you cannot see is a rule you cannot judge.
//!
//! THE VEIL IS A COLOR, NOT A THING. Unknown ground is painted the veil's
//! tint - and so is every tree, boulder, bush, ore seam and river standing on
//! it - by `GroundVeil`, an extension on the material all of them already
//! wore. Nothing is stored and nothing is hidden: the shader measures each
//! pixel against the handful of circles the village actually knows, and mixes
//! the tint in after the lighting so the answer is one color under every sky.
//!
//! It used to be an occluder: a copy of each chunk's mesh lifted into a bank
//! tall enough to bury a wood. That is gone, along with everything that
//! existed to manage its height, its taper and its edges. A bank is an object
//! standing in the world, and an object can be seen under, seen past, and
//! disagreed with about where the ground is; a color cannot.

use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::terrain::{TerrainChunk, WaterPlane};
use crate::villager::explore::KnownWorld;

/// The ground's own veil, worn by the terrain and everything growing on it.
const GROUND_SHADER_PATH: &str = "shaders/ground_veil.wgsl";

/// As many pockets as the shader's uniform holds. Explorers bring back far
/// fewer than this; the oldest simply stop being drawn if they ever do not.
const MAX_POCKETS: usize = 128;

/// How heavily the veil paints, given the three things that decide whether
/// there is one at all.
///
/// The cloths answered this by not existing - they were hung only while the
/// fog was up over a world with a village in it. Paint has no such luxury: the
/// material is on every acre from the first frame, so the answer has to be
/// written down, every frame, in the one number the shader multiplies by.
fn veil_weight(fog_on: bool, playing: bool, a_village_exists: bool) -> f32 {
    if fog_on && playing && a_village_exists {
        1.0
    } else {
        0.0
    }
}

/// How many meters the painted veil takes to come on at the edge of what the
/// village knows.
///
/// A shore rather than a cut line. This is the ONLY dial the veil has left:
/// there was a bank height and a taper and a settle distance, and all three
/// belonged to an occluder that no longer exists.
const VEIL_EDGE: f32 = 9.0;

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

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<GroundMaterial>::default())
            .init_resource::<FogMode>()
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
                    // NOT state-gated either, and for the same reason as the
                    // line above: this is what tells the ground whether there
                    // is a veil at all. Gated on `Playing` it simply never ran
                    // before a flag went in, and the ground went on painting
                    // itself from a default uniform nobody had corrected.
                    follow_the_known,
                    raise_the_veil.run_if(in_state(crate::GameState::Playing)),
                )
                    .chain()
                    // AFTER the ground streams. This mattered enormously when
                    // a chunk needed a cloth hung on it and would otherwise
                    // spend a frame bare - twenty bright squares in every
                    // frame of a pan. A painted chunk is born wearing its
                    // veil, so the ordering is now only about the cull below
                    // seeing this frame's chunks rather than last frame's.
                    .after(crate::terrain::TerrainSet),
            );
    }
}

/// The veil's color, in linear light: a cold slate blue rather than black,
/// because unknown ground should read as unlit distance and not as a hole cut
/// in the world.
///
/// Public because the PLANET wears the same fog of war — painted into its
/// patches rather than draped in cloths, since from orbit there is no cloth
/// small enough — and the two must be the same color or the veil visibly
/// changes shade as the god climbs. One fact, one place.
pub(crate) const VEIL_TINT: [f32; 3] = [0.05, 0.07, 0.11];

/// Uniform block handed to the fog shader.
#[derive(Clone, ShaderType, Debug)]
pub struct FogParams {
    /// The veil's color; alpha is how heavy it gets at its thickest.
    pub tint: Vec4,
    /// The home ground: xyz its center, w its radius.
    pub home: Vec4,
    /// x how many pockets are live, y how many meters the edge takes.
    pub dials: Vec4,
    /// The planet: xyz its center, w its radius.
    ///
    /// The veil is drawn on the BENT world, and what it knows is written in
    /// FLAT ground coordinates - so the shader has to undo the bend before it
    /// can ask whether a pixel is known. Near the village the two agree closely
    /// enough that nobody noticed for months; walk to the far side of the world
    /// and they have nothing to do with one another. Brett: "if you go to the
    /// other side of the world to plant your flag it works but the fog of war
    /// doesnt clear around the settlement."
    pub planet: Vec4,
    /// Each known pocket: xyz its center, w its radius.
    pub pockets: [Vec4; MAX_POCKETS],
}

impl Default for FogParams {
    fn default() -> Self {
        FogParams {
            // Full unknownness is solid. Alpha drives the veil's rising and
            // dithered discovery edge, not translucency through the world.
            tint: Vec4::new(VEIL_TINT[0], VEIL_TINT[1], VEIL_TINT[2], 1.0),
            home: Vec4::new(0.0, 0.0, 0.0, 170.0),
            // x is how many pockets are live, y how far the edge fades.
            // z and w carried the old bank's height and taper and are
            // spare now that nothing is lifted off the ground.
            dials: Vec4::new(0.0, VEIL_EDGE, 0.0, 0.0),
            planet: crate::globe::planet_center().extend(crate::terrain::PLANET_RADIUS),
            pockets: [Vec4::ZERO; MAX_POCKETS],
        }
    }
}

/// The ground's own veil, and the veil of everything standing on it.
///
/// The same fog, mixed into the LIT surface instead of draped over it in a
/// sheet. Worn by the terrain, the groves, the loose trees, the scenery, the
/// boulders and the bushes - everything that comes out of `TerrainAssets`'s
/// ground material - so that unknown country and the wood growing on it are
/// one color together, with nothing standing proud of anything.
///
/// See `ground_veil.wgsl` for why this is a per-pixel read of the fog uniform
/// rather than the planet's baked vertex mark.
#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
pub struct GroundVeil {
    /// 100, not 0: the base `StandardMaterial` owns the low bindings.
    #[uniform(100)]
    pub params: FogParams,
}

/// Ordinary ground, plus the fog of war mixed in after the lighting.
pub type GroundMaterial = bevy::pbr::ExtendedMaterial<StandardMaterial, GroundVeil>;

impl bevy::pbr::MaterialExtension for GroundVeil {
    fn fragment_shader() -> ShaderRef {
        GROUND_SHADER_PATH.into()
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

/// Hides whole chunks that nobody has seen any part of.
///
/// THIS USED TO HANG CLOTHS. The veil was an occluder: a copy of every chunk's
/// terrain, lifted into a bank tall enough to bury a wood, hiding what stood
/// under it. A bank is a solid object in the world, and a solid object has a
/// side and an edge - so it floated over ground the planet drew at a coarser
/// height, ended in a cliff where the chunks ran out, let trees stand clean on
/// top of it, and left the ground beneath it wearing full daylight so the
/// shroud read as a sheet laid over a readable world.
///
/// Brett cut the knot: "What if we got rid of the veil on the rendered bits
/// entirely and just kept the veil on the LOD and we just painted the veiled
/// rendered chunks?" So unknown ground is PAINTED the veil's color - by
/// `fog::GroundVeil`, worn by the terrain and by every tree, boulder and bush
/// standing on it - and the planet's patches go on carrying the same color in
/// their vertices. Nothing is hidden, so there is nothing to see under, no edge
/// to meet the paint at, and no height for the two halves to disagree about:
/// the veil is now exactly as tall as the world it covers.
///
/// What is left is a saving. A chunk nobody has seen any part of is painted
/// flat veil color from edge to edge, and the planet patch beneath it is
/// painted the same - so drawing the chunk, its groves, its grass and its
/// boulders buys nothing at all. It is not drawn.
fn drape_the_veil(
    mut commands: Commands,
    mode: Res<FogMode>,
    state: Res<State<crate::GameState>>,
    known: Option<Res<crate::villager::explore::KnownWorld>>,
    chunks: Query<
        (
            Entity,
            &Visibility,
            Has<crate::terrain::ParkedChunk>,
            Option<&TerrainChunk>,
        ),
        Or<(With<TerrainChunk>, With<WaterPlane>)>,
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("fog: drape_the_veil");
    // Chunks are born hidden so that no unpainted ground is ever seen before
    // the material knows where the village has walked. Whoever decides they
    // may be seen is this system.
    let reveal = |commands: &mut Commands, chunk: Entity, showing: &Visibility| {
        if *showing == Visibility::Hidden {
            // try_insert: the Title click tears chunks down mid-frame, and
            // dressing a chunk that died is skippable, not fatal.
            commands.entity(chunk).try_insert(Visibility::Inherited);
        }
    };

    let veiled = mode.0 && *state.get() == crate::GameState::Playing;
    for (chunk, showing, parked, ground) in &chunks {
        if parked {
            continue;
        }
        // Whole chunks only, and only where every corner is unknown - the test
        // carries half the chunk's diagonal as its margin, so this can never
        // hide ground somebody is looking at. A chunk with one known corner
        // stays, and its unknown acres are simply painted.
        let unseen = veiled
            && known.as_ref().zip(ground).is_some_and(|(known, ground)| {
                !known.knows_flat(
                    (ground.coord.x as f32 + 0.5) * crate::terrain::CHUNK_SIZE,
                    (ground.coord.y as f32 + 0.5) * crate::terrain::CHUNK_SIZE,
                    crate::terrain::CHUNK_SIZE * std::f32::consts::SQRT_2 * 0.5,
                )
            });
        if unseen {
            if *showing != Visibility::Hidden {
                commands.entity(chunk).try_insert(Visibility::Hidden);
            }
            continue;
        }
        reveal(&mut commands, chunk, showing);
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
    mut commands: Commands,
    cameras: Query<Entity, With<crate::camera::GodCamera>>,
    mode: Res<FogMode>,
    state: Res<State<crate::GameState>>,
    known: Option<Res<KnownWorld>>,
    mut ground: ResMut<Assets<GroundMaterial>>,
) {
    // IS THERE A VEIL AT ALL. The cloths answered this by not existing: they
    // were only ever hung while the fog was up over a world with a village in
    // it. A painted veil has no such luxury - the material is on every acre of
    // ground from the first frame, and if nobody tells it otherwise it paints
    // from whatever the uniform happens to hold. Which, before a village
    // exists, is `FogParams::default()`: a home circle of a hundred and
    // seventy meters at the WORLD ORIGIN. Brett, looking at a continent gone
    // slate blue around one lit patch of nowhere: "I havent even placed the
    // flag yet."
    //
    // So the weight is the switch, and it is written every frame whatever the
    // state. While the god is still choosing ground there is nothing known and
    // nothing to hide, and a player cannot pick somewhere they are not allowed
    // to look at.
    let weight = veil_weight(
        mode.0,
        *state.get() == crate::GameState::Playing,
        known.is_some(),
    );
    let live = known
        .as_ref()
        .map_or(0, |k| k.pockets.len().min(MAX_POCKETS));
    let tell = |params: &mut FogParams| {
        params.tint.w = weight;
        if let Some(known) = known.as_ref() {
            params.home = known.center.extend(known.radius);
            params.planet = crate::globe::planet_center().extend(crate::terrain::PLANET_RADIUS);
            params.dials.x = live as f32;
            for (slot, pocket) in known.pockets.iter().take(live).enumerate() {
                params.pockets[slot] = pocket.at.extend(pocket.radius);
            }
        }
    };
    for (_, material) in ground.iter_mut() {
        tell(&mut material.extension.params);
    }
    // AND ASPECTUS'S PASS, which veils everything the materials cannot reach -
    // the villagers, the animals, the roofs. Told from the same knowledge in
    // the same breath, so the painted ground and the pass over it can never
    // disagree about where the village has walked.
    for camera in &cameras {
        let mut carried = crate::render::aspectus::VeilView::default();
        tell(&mut carried.params);
        // DIVUS_FACTUS_VEIL_DEBUG=1 paints the pass's own working instead of
        // the world: green where it believes the village has walked, red
        // where not, a hundred-meter checker to judge the reconstruction by.
        if std::env::var("DIVUS_FACTUS_VEIL_DEBUG").is_ok_and(|dial| dial == "1") {
            carried.params.dials.z = 1.0;
        }
        commands.entity(camera).insert(carried);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No village, no veil.
    ///
    /// The cloths enforced this by not existing - nothing was hung until the
    /// fog was up over a founded world. Paint is on every acre from the first
    /// frame instead, so the rule has to be stated, and when it was not the
    /// whole continent went slate blue around one lit circle of nowhere: the
    /// uniform still held `FogParams::default()`, whose home is a hundred and
    /// seventy meters at the WORLD ORIGIN. Brett: "I havent even placed the
    /// flag yet."
    #[test]
    fn there_is_no_veil_before_there_is_a_village() {
        assert_eq!(
            veil_weight(true, true, true),
            1.0,
            "a founded world under a raised fog is veiled",
        );
        assert_eq!(
            veil_weight(true, true, false),
            0.0,
            "no village means nothing is known and nothing is hidden - this is \
             the god choosing ground, and they may look wherever they like",
        );
        assert_eq!(
            veil_weight(true, false, true),
            0.0,
            "and not while the world is still being made or chosen over",
        );
        assert_eq!(
            veil_weight(false, true, true),
            0.0,
            "DIVUS_FACTUS_FOG=0 lifts it entirely, for photographing the world",
        );
    }
}
