//! The planet, whole and continuous: the ground is a sphere at every height.
//!
//! The terrain has been a genuine sphere since the field moved into a volume.
//! This module makes the PICTURE true as well. The planet is not one mesh but
//! a tree of patches — six cube faces, each splitting into four children
//! toward the camera — so the same surface carries seven-hundred-unit cells
//! seen from orbit and three-unit cells seen near the ground, sharpening as
//! the god descends and coarsening as they rise, with no threshold at which
//! one picture is swapped for another. Google Earth is the honest comparison,
//! and the deliberate one.
//!
//! There is no orbit MODE. That is the point, arrived at the hard way: every
//! threshold that staged the planet in or out - a curtain for the chunks, a
//! layer swap for the camera, a separate orbital rig - made a seam someone
//! could see, and the last of them made the planet feel like a different
//! screen rather than the same world further away. Now the ordinary camera
//! simply zooms: the flat chunk world (where villagers, trees and buildings
//! live) draws on top of the planet as the final layer of detail, dwindles
//! to a dot as the god rises, and the same wheel comes home again. One
//! camera, one gesture, no handover anywhere in it.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use std::f32::consts::FRAC_PI_2;

use crate::camera::{CameraRig, GodCamera};
use crate::hand::HandPart;
use crate::palette;
use crate::terrain::{PLANET_RADIUS, Terrain, WATER_LEVEL};

/// The globe's own render layer. The planet and its sun live here alone, so
/// what the camera renders is staged with single component writes.
pub const GLOBE_LAYER: usize = 2;

/// Where the climb starts telling: the sky begins turning from horizon to
/// blue here, and the diorama lens retires. The planet itself is beneath the
/// world at EVERY height now — there is no transition to gate — so this is a
/// dial for the atmosphere, not a curtain for the scenery.
pub const ASCENT: f32 = 1_500.0;

/// How far out the wheel will take the god. From here the whole planet sits
/// in the frame with room to spare, like a held apple.
pub const CEILING: f32 = 42_000.0;

/// How far the patch surface is drawn BELOW the true ground.
///
/// The patches and the streamed chunks are now the same surface - both are
/// the same height field seated on the same sphere - and coincident surfaces
/// fight for every pixel. That fight was the grey striping across the whole
/// landscape: the chunks painted green, the patches beneath them painted
/// with the fog of war's slate, trading places pixel by pixel down to the
/// depth buffer's last bit. Before the world was bent the chunks lay on a
/// flat plane hundreds of units clear of the sphere and always won by being
/// nearer; the bend took that accident away and nothing replaced it.
///
/// So the planet is sunk a couple of units, and the chunks win wherever they
/// exist. The cost is a small step down at the streamed rim - the detail seam
/// that is already the known price of two representations - and at the
/// distance the rim is ever seen, two units is nothing.
const PATCH_SINK: f32 = 2.5;

/// Grid cells along each edge of every patch, whatever its depth. Depth is
/// carried by the tree, not the grid: a deeper patch covers a quarter of the
/// ground with the same count of cells, which is what sharpening IS.
const PATCH_CELLS: usize = 32;

/// How deep the tree may split. Level-eight patches are ninety-two units
/// across with cells under three — the same order as the chunk world's own
/// vertices — and they are only asked for near the bottom of the climb,
/// where the flat world is about to take over the detail anyway.
const MAX_LEVEL: u8 = 8;

/// Split while a patch's cells subtend more than this many pixels. The knob
/// that trades sharpness for patch count; nine pixels keeps the low-poly
/// look through every level rather than dissolving it into smoothness.
const SPLIT_PX: f32 = 7.0;

/// Patches built per frame while the tree is chasing the camera. A build is
/// a couple of milliseconds of noise sampling; four a frame chases a fast
/// descent closely without stuttering the simulation, and a patch that
/// arrives a beat late only means the ground is briefly coarser there —
/// the same bargain streaming ground has always made.
const BUILDS_PER_FRAME: usize = 6;

/// The camera never counts as closer to the surface than this when deciding
/// how deep to refine. Low: the patches now stand in open view just past the
/// loaded ground's edge at play zoom, and the sharper they hold that ring,
/// the quieter the one seam this design has left.
const REFINE_FLOOR: f32 = 300.0;

/// The rotation that stands the planet up in the world: the terrain scaffold
/// maps ground onto the unit sphere with the reference point at +Z and the
/// poles at ±Y, and this turns that frame so the reference point is UP,
/// with the game's own north (-z on the ground) kept as north.
pub(crate) fn planet_stance() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2)
}

/// Where the planet's centre sits: the sea-level sphere tangent to the flat
/// world at the origin, sunk a few units so the two surfaces — the same
/// field, so they agree exactly at the tangent point — never fight over the
/// same pixels while both are drawn.
pub(crate) fn planet_centre() -> Vec3 {
    Vec3::new(0.0, -(PLANET_RADIUS + WATER_LEVEL) - 8.0, 0.0)
}

// ------------------------------------------------------------------ the tree

/// One patch of the planet: a face of the cube, a depth, and which cell of
/// that depth's grid. The whole quadtree is names, not pointers.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct PatchKey {
    face: u8,
    level: u8,
    x: u32,
    y: u32,
}

impl PatchKey {
    fn child(self, dx: u32, dy: u32) -> PatchKey {
        PatchKey {
            face: self.face,
            level: self.level + 1,
            x: self.x * 2 + dx,
            y: self.y * 2 + dy,
        }
    }

    fn parent(self) -> Option<PatchKey> {
        (self.level > 0).then(|| PatchKey {
            face: self.face,
            level: self.level - 1,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    /// The patch's span on its face, in `[-1, 1]` cube coordinates.
    fn rect(self) -> (f32, f32, f32) {
        let side = 2.0 / (1u32 << self.level) as f32;
        (
            -1.0 + self.x as f32 * side,
            -1.0 + self.y as f32 * side,
            side,
        )
    }

    /// Unit direction, in the planet's own frame, to the middle of the patch.
    fn centre_dir(self) -> Vec3 {
        let (u0, v0, side) = self.rect();
        let (outward, along_u, along_v) = face_axes(self.face);
        (outward + along_u * (u0 + side * 0.5) + along_v * (v0 + side * 0.5)).normalize()
    }

    /// Arc length of one grid cell of this patch, in world units. Approximate
    /// — the cube projection stretches toward face corners — and plenty for a
    /// split heuristic.
    fn cell_arc(self) -> f32 {
        let quarter = std::f32::consts::FRAC_PI_2 * PLANET_RADIUS;
        quarter / (PATCH_CELLS as f32 * (1u32 << self.level) as f32)
    }
}

/// The six faces of the cube, each an outward axis spanned by two others.
fn face_axes(face: u8) -> (Vec3, Vec3, Vec3) {
    match face {
        0 => (Vec3::X, Vec3::Y, Vec3::Z),
        1 => (Vec3::NEG_X, Vec3::Y, Vec3::NEG_Z),
        2 => (Vec3::Y, Vec3::Z, Vec3::X),
        3 => (Vec3::NEG_Y, Vec3::NEG_Z, Vec3::X),
        4 => (Vec3::Z, Vec3::Y, Vec3::NEG_X),
        _ => (Vec3::NEG_Z, Vec3::Y, Vec3::X),
    }
}

/// The living tree: the root everything hangs from, and which patches stand.
#[derive(Resource, Default)]
struct PlanetTree {
    root: Option<Entity>,
    built: HashMap<PatchKey, Patch2>,
    /// The seed the tree was grown for, so a new world fells the old tree.
    grown_for: Option<u32>,
    /// Frames since the world began, for the forgetting below.
    beat: u64,
    /// Which painting of the veil the world currently wants. Bumped when the
    /// fog of war toggles or the known world grows; patches wearing an older
    /// painting are rebuilt a few per frame, so the veil SWEEPS rather than
    /// hitches.
    paint_beat: u64,
    /// A cheap fingerprint of (fog mode, known world) to notice the change.
    veil_print: (bool, u32, usize),
}

/// A standing patch: its entity, its mesh, the last frame it was on screen —
/// the tree forgets what it has not shown for a while — and which painting
/// of the veil it wears, so the fog of war can sweep across a planet that
/// already stands.
struct Patch2 {
    entity: Entity,
    mesh: Handle<Mesh>,
    last_shown: u64,
    painted: u64,
}

/// How long a hidden patch is kept before it is felled, in frames. Long
/// enough that riding the wheel up and down does not rebuild the same
/// ground, short enough that a long flight cannot hoard the planet.
///
/// It has to forget: every patch is a live mesh in memory whether drawn or
/// not, and the first tree never dropped one — a session of flying grew it
/// by four patches a frame until the whole game ran at eight frames a
/// second, and the loudest clue was the quietest number on the overlay.
const KEPT_FOR: u64 = 1_800;

/// Patches at or below this depth are never felled, and all of them are
/// grown at world creation: at this depth the whole planet is sharp from
/// orbit, so the far view is BAKED - zooming out never builds anything, and
/// the streaming-in of finer ground happens only on the way down, close in,
/// where streaming has always happened. Five hundred and ten patches at a
/// six-thousand radius; the bake is a couple of seconds inside the loading
/// screen, and the fallback for a still-growing patch always finds an
/// ancestor standing.
const EVERGREEN: u8 = 3;

/// The one material every patch wears; the colours ride the vertices.
#[derive(Resource)]
struct PlanetSkin(Handle<PlanetMaterial>);

/// Ordinary ground, plus the fog of war mixed in AFTER the lighting.
///
/// The cloths that hide unknown ground at play height are unlit — their shader
/// hands back its tint and that is the pixel. A patch is lit ground, so the
/// same colour painted into its vertices came out somewhere else entirely: the
/// sun's diffuse and its specular sheen both add to it, and the veil read half
/// again as far toward white, less blue, and lighter still at a grazing angle
/// near the limb. Three shades of one fog in a single frame. Worse, any tuning
/// of it would only hold for the light it was tuned under.
///
/// So the mix happens in the fragment shader, past the lighting, and the answer
/// is the cloth's colour under any sun at all.
pub type PlanetMaterial = ExtendedMaterial<StandardMaterial, VeilExtension>;

/// Which colour the veil is, and how much of it fully-veiled ground takes.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct VeilExtension {
    /// rgb the veil's colour; a the weight at full veil.
    #[uniform(100)]
    pub tint: Vec4,
}

impl MaterialExtension for VeilExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/planet_skin.wgsl".into()
    }
}

/// Marks the planet's root entity and its sun.
#[derive(Component)]
struct ThePlanet;

/// Marks one patch, by its name in the tree.
#[derive(Component)]
struct Patch(PatchKey);

pub struct GlobePlugin;

impl Plugin for GlobePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PlanetMaterial>::default())
            .init_resource::<PlanetTree>()
            .add_systems(
                Update,
                (plant_the_tree, dress_the_patches).run_if(resource_exists::<Terrain>),
            )
            .add_systems(
                Update,
                tend_the_tree
                    .after(crate::camera::CameraSet)
                    .run_if(crate::world_is_afoot),
            )
            .add_systems(Update, dress_for_space.after(crate::render::paint_the_sky))
            // The bend runs at the very end of the transform pipeline: after
            // propagation has written every entity's final flat pose, before
            // the renderer reads it. That ordering IS the design - the
            // simulation keeps its flat world, the picture gets the sphere.
            .add_systems(
                PostUpdate,
                bend_the_world.after(bevy::transform::TransformSystems::Propagate),
            );
    }
}

// ----------------------------------------------------------------- the bend

/// A flat-world position's seat on the sphere, and the tangent rotation that
/// stands things upright there.
///
/// This is the whole trick of the round world, in one function. The
/// simulation lives on a flat plane - a hundred and nine call sites speak
/// `(x, z)` and height, and none of them change. The PICTURE lives on the
/// sphere: `x` and `z` become a direction, height becomes distance from the
/// planet's centre, and the rotation turns "up" into the local outward
/// radial, east into east along the curve. Applied rigidly to a whole chunk
/// the error within its sixty-four units is eight centimetres, which is why
/// chunks can tile a planet without knowing they are doing it.
pub(crate) fn bend_frame(flat: Vec3) -> (Vec3, Quat) {
    let stance = planet_stance();
    let up = stance * crate::terrain::direction_at(flat.x, flat.z);
    // East, from the seat's own derivative along +x, straightened against up.
    let ahead = stance * crate::terrain::direction_at(flat.x + 1.0, flat.z);
    let behind = stance * crate::terrain::direction_at(flat.x - 1.0, flat.z);
    let east = (ahead - behind).normalize_or(Vec3::X);
    let east = (east - up * east.dot(up)).normalize_or(Vec3::X);
    let turn = Quat::from_mat3(&Mat3::from_cols(east, up, east.cross(up)));
    let seat = planet_centre() + up * (PLANET_RADIUS + flat.y);
    (seat, turn)
}

/// The camera's seated pose, derived the same way the bend derives it.
///
/// Shared, because two other things need the camera in bent space and must
/// agree with it exactly: the cursor ray, which is walked through the seated
/// world, and the god's hand, which floats in front of the camera as the UI
/// pointer. Each computed it their own way once, and each was wrong in its
/// own direction.
pub(crate) fn bent_camera_pose(rig: &CameraRig) -> Transform {
    let (eye_seat, eye_turn) = bend_frame(rig.eye());
    if rig.distance < crate::camera::FIRST_PERSON {
        // No separation to look along; carry the flat gaze into the seat.
        Transform {
            translation: eye_seat,
            rotation: eye_turn
                * Transform::default()
                    .looking_to(rig.forward(), Vec3::Y)
                    .rotation,
            scale: Vec3::ONE,
        }
    } else {
        let (focus_seat, focus_turn) = bend_frame(rig.focus);
        Transform::from_translation(eye_seat).looking_at(focus_seat, focus_turn * Vec3::Y)
    }
}

/// The bend, undone: a seated world position back into the flat coordinates
/// the simulation speaks.
///
/// Needed wherever a bent answer has to re-enter flat reasoning - the god's
/// hand reads hovered entities' `GlobalTransform`, which is already seated,
/// and would otherwise be bent a second time and thrown a quarter of a world
/// away from the thing it was reaching for.
pub(crate) fn unbend(seat: Vec3) -> Vec3 {
    let from_centre = seat - planet_centre();
    let radius = from_centre.length().max(1.0);
    let dir = planet_stance().inverse() * (from_centre / radius);
    let (x, z) = ground_coordinates(dir);
    Vec3::new(x, radius - PLANET_RADIUS, z)
}

/// Seats every world entity's RENDER transform on the sphere, once per
/// propagation write.
///
/// The simulation is never touched: it owns `Transform` and keeps thinking
/// in the flat frame. This rewrites `GlobalTransform` - what the renderer,
/// the culling and the camera actually consume - after propagation has
/// finished with it, through `bypass_change_detection` so the write is
/// invisible to change tracking and each pose is bent exactly once, when
/// propagation produces it. Chunks, trees, buildings, villagers, the fire,
/// the flag, the god's own camera: one map, everything on the ball.
///
/// Excluded: UI nodes (their transforms are pixels), the planet's own
/// patches (already spherical), directional lights (a direction has no
/// seat), and the water plane, whose vertices are bent individually because
/// a single rigid seat cannot serve a sheet thousands of units wide.
fn bend_the_world(
    mut bendable: Query<
        &mut GlobalTransform,
        (
            Changed<GlobalTransform>,
            Without<Node>,
            Without<Patch>,
            Without<ThePlanet>,
            Without<crate::terrain::WaterPlane>,
            Without<DirectionalLight>,
            Without<CameraRig>,
            // Geometry already seated on the sphere by its own vertices:
            // chunks, their rivers, and the veil sheets that copy them.
            Without<crate::terrain::TerrainChunk>,
            Without<crate::fog::Veil>,
            // The god's hand places itself in seated space already - it has
            // to, because as the UI pointer it floats a few units in front of
            // the camera, and there is no flat position that bends to "just
            // in front of the camera". Bent again here it was flung to the
            // seat of a point beside the god's eye, thousands of units away:
            // no cursor on any panel, and none on the title screen. Every
            // part, not just the root - see `HandPart`.
            Without<HandPart>,
            // And the camera that draws it, which is a child of the god's own
            // and must share its pose exactly; it gets that below.
            Without<crate::render::HandCamera>,
        ),
    >,
    mut eyes: Query<(&mut GlobalTransform, &CameraRig), Without<crate::render::HandCamera>>,
    mut overlay: Query<&mut GlobalTransform, With<crate::render::HandCamera>>,
) {
    for mut global in &mut bendable {
        let (scale, rotation, translation) = global.to_scale_rotation_translation();
        let (seat, turn) = bend_frame(translation);
        *global.bypass_change_detection() = GlobalTransform::from(Transform {
            translation: seat,
            rotation: turn * rotation,
            scale,
        });
    }

    // The camera cannot be bent rigidly, and this is the one place the naive
    // map fails outright. Its eye stands thousands of units from its focus -
    // at play pitch, two and a half thousand, which on a six-thousand radius
    // world is twenty-five degrees of arc - so seating the eye by its own
    // (x, z) drops it a quarter-continent away and turns it into THAT local
    // frame, where it stares at unrelated ground. What must be preserved is
    // the RELATIONSHIP: the eye seated where it belongs, looking at the
    // focus where the focus now is, with the focus's own outward as up.
    let mut seated_eye = None;
    for (mut global, rig) in &mut eyes {
        let (eye_seat, eye_turn) = bend_frame(global.translation());
        let (focus_seat, focus_turn) = bend_frame(rig.focus);
        let up = focus_turn * Vec3::Y;
        let bent = if rig.distance < crate::camera::FIRST_PERSON {
            // Behind a mortal's eyes there is no separation to preserve, and
            // `looking_at` has no direction to work from: carry the flat
            // forward into the seat's own frame instead.
            let (_, rotation, _) = global.to_scale_rotation_translation();
            Transform {
                translation: eye_seat,
                rotation: eye_turn * rotation,
                scale: Vec3::ONE,
            }
        } else {
            Transform::from_translation(eye_seat).looking_at(focus_seat, up)
        };
        *global.bypass_change_detection() = GlobalTransform::from(bent);
        seated_eye = Some(bent);
    }

    // The hand's camera is an identity child of the god's, so that it always
    // sees exactly what the god sees and the cursor lands where it looks. Its
    // global was computed by propagation from the FLAT parent, before the loop
    // above bent it, and the ordinary rule would then bend it by a different
    // law than the look-at - leaving the overlay pass pointing degrees away
    // from the pass beneath it. Hand it the same pose instead.
    if let Some(bent) = seated_eye {
        for mut global in &mut overlay {
            *global.bypass_change_detection() = GlobalTransform::from(bent);
        }
    }
}

// -------------------------------------------------------------- the planting

/// Raises the planet's root and sun for a new world, and pre-grows the tree
/// two levels deep — the orbital view's own resolution — in the loading
/// screen's shadow, so the first climb finds a planet already standing.
fn plant_the_tree(
    mut commands: Commands,
    terrain: Res<Terrain>,
    veil: (
        Option<Res<crate::fog::FogMode>>,
        Option<Res<crate::villager::explore::KnownWorld>>,
    ),
    mut tree: ResMut<PlanetTree>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
) {
    let shroud = veil_of(&veil.0, &veil.1);
    if tree.grown_for == Some(terrain.seed) {
        return;
    }
    tree.grown_for = Some(terrain.seed);
    if let Some(old) = tree.root.take() {
        commands.entity(old).despawn();
    }
    tree.built.clear();

    let root = commands
        .spawn((
            Name::new("The Planet"),
            ThePlanet,
            Transform::from_translation(planet_centre()).with_rotation(planet_stance()),
            Visibility::Inherited,
        ))
        .id();
    tree.root = Some(root);

    // The planet's own sun: the world's lights are confined to the world's
    // layers, and a planet without one hangs in space unlit. Same colour and
    // strength as the sun the ground knows, from the same direction.
    commands.spawn((
        Name::new("The Planet's Sun"),
        ThePlanet,
        DirectionalLight {
            color: palette::shade(&palette::BONE, 1.0),
            illuminance: 17_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(crate::SUN_DIRECTION * 140.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(GLOBE_LAYER),
        ChildOf(root),
    ));

    commands.insert_resource(PlanetSkin(materials.add(PlanetMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 1.0,
            ..default()
        },
        extension: VeilExtension {
            // Nine parts in ten, the weight the cloths stack to at play
            // height; the tenth that shows through keeps a coast reading as a
            // coast under the shroud.
            tint: Vec4::new(
                crate::fog::VEIL_TINT[0],
                crate::fog::VEIL_TINT[1],
                crate::fog::VEIL_TINT[2],
                0.9,
            ),
        },
    })));

    // One level past the evergreens: those patches may be evicted later if
    // long unseen, but growing them here means the first descent anywhere
    // on the planet finds its middle distances already standing - Brett's
    // "preload the chunks at creation", scoped to what memory affords.
    const PREBAKED: u8 = EVERGREEN + 1;
    let mut grown = 0;
    for face in 0..6u8 {
        for level in 0..=PREBAKED {
            let across = 1u32 << level;
            for y in 0..across {
                for x in 0..across {
                    let key = PatchKey { face, level, x, y };
                    grow_patch(
                        &mut commands,
                        &terrain,
                        shroud,
                        &mut meshes,
                        &mut tree,
                        root,
                        key,
                    );
                    grown += 1;
                }
            }
        }
    }
    info!(
        "the planet took root: {grown} patches pre-grown, a world {:.0} units around",
        crate::terrain::planet_circumference()
    );
}

/// Builds one patch and hangs it on the root, hidden until the tree shows it.
fn grow_patch(
    commands: &mut Commands,
    terrain: &Terrain,
    veil: Option<&crate::villager::explore::KnownWorld>,
    meshes: &mut Assets<Mesh>,
    tree: &mut PlanetTree,
    root: Entity,
    key: PatchKey,
) -> Entity {
    // A regrowth replaces the old stand outright.
    if let Some(old) = tree.built.remove(&key) {
        commands.entity(old.entity).despawn();
        meshes.remove(&old.mesh);
    }
    let mesh = meshes.add(build_patch(terrain, veil, key));
    let entity = commands
        .spawn((
            Patch(key),
            Mesh3d(mesh.clone()),
            Transform::default(),
            Visibility::Hidden,
            RenderLayers::layer(GLOBE_LAYER),
            NotShadowCaster,
            NotShadowReceiver,
            ChildOf(root),
        ))
        .id();
    let beat = tree.beat;
    let painted = tree.paint_beat;
    tree.built.insert(
        key,
        Patch2 {
            entity,
            mesh,
            last_shown: beat,
            painted,
        },
    );
    entity
}

/// Hands the shared material to any patch that lacks it. Separate from the
/// spawning so patch growth never has to thread the material through.
fn dress_the_patches(
    mut commands: Commands,
    skin: Option<Res<PlanetSkin>>,
    bare: Query<Entity, (With<Patch>, Without<MeshMaterial3d<PlanetMaterial>>)>,
) {
    let Some(skin) = skin else {
        return;
    };
    for patch in &bare {
        commands
            .entity(patch)
            .insert(MeshMaterial3d(skin.0.clone()));
    }
}

/// The veil to paint with, if the fog of war is on and a known world exists.
fn veil_of<'k>(
    mode: &Option<Res<crate::fog::FogMode>>,
    known: &'k Option<Res<crate::villager::explore::KnownWorld>>,
) -> Option<&'k crate::villager::explore::KnownWorld> {
    let veiled = mode.as_ref().is_none_or(|mode| mode.0);
    if !veiled {
        return None;
    }
    known.as_ref().map(|known| &**known)
}

// -------------------------------------------------------------- the building

/// Samples one patch of the world: heights and paint from the same fields
/// the chunks are built from, and a skirt around the edge to curtain the
/// hairline cracks where neighbouring patches meet at different depths.
fn build_patch(
    terrain: &Terrain,
    veil: Option<&crate::villager::explore::KnownWorld>,
    key: PatchKey,
) -> Mesh {
    let n = PATCH_CELLS;
    let stride = n + 1;
    let padded = n + 3;
    let (u0, v0, side) = key.rect();
    let (outward, along_u, along_v) = face_axes(key.face);
    let step = side / n as f32;

    // Padded grids of drawn positions and ground answers, one cell beyond
    // each edge for slopes and normals. Water is asked about per vertex —
    // the game's lakes sit in basins ABOVE sea level, and a planet that only
    // wore its ocean painted every one of them as green land, which is most
    // of what made the ball read as a different world from the ground.
    let mut grid: Vec<Vec3> = Vec::with_capacity(padded * padded);
    let mut ground: Vec<(f32, f32, f32, f32)> = Vec::with_capacity(padded * padded);
    for gj in 0..padded {
        for gi in 0..padded {
            let u = u0 + (gi as f32 - 1.0) * step;
            let v = v0 + (gj as f32 - 1.0) * step;
            let dir = (outward + along_u * u + along_v * v).normalize();
            let (x, z) = ground_coordinates(dir);
            let h = terrain.base_height_at(x, z);
            // The water surface standing over this ground, if any: the sea,
            // or a river or lake the courses know about.
            let surface = match terrain.river_surface_at(x, z) {
                Some(level) if level > h => level,
                _ => WATER_LEVEL,
            };
            let wet = surface.max(WATER_LEVEL);
            grid.push(dir * drawn_radial(h, wet));
            ground.push((x, z, h, wet));
        }
    }
    let at = |gi: usize, gj: usize| grid[gj * padded + gi];

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(stride * stride + stride * 4);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(positions.capacity());
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(positions.capacity());
    let mut indices: Vec<u32> = Vec::with_capacity(n * n * 6 + n * 24);

    for gj in 0..stride {
        for gi in 0..stride {
            let (pi, pj) = (gi + 1, gj + 1);
            let here = at(pi, pj);
            let dir = here.normalize();
            let (x, z, h, wet) = ground[pj * padded + pi];

            let across = at(pi + 1, pj) - at(pi - 1, pj);
            let down = at(pi, pj + 1) - at(pi, pj - 1);
            let mut normal = across.cross(down).normalize_or(dir);
            if normal.dot(dir) < 0.0 {
                normal = -normal;
            }
            let slope = (1.0 - normal.dot(dir)).clamp(0.0, 1.0);

            positions.push(here.to_array());
            normals.push(normal.to_array());
            let mut color = paint(terrain, x, z, h, wet, slope);
            // The fog of war wraps the whole planet: ground the village has not
            // walked takes the veil, sea and land alike. The god does not see
            // round the world; the god sees what has been SHOWN, and from orbit
            // that is a handful of clearings on a shrouded ball.
            //
            // Carried in the ALPHA channel, and applied by the skin's shader
            // after the lighting - see `PlanetMaterial`. Blending the veil's
            // colour into the rgb here instead put a LIT version of it on the
            // world, which is a different colour from the unlit cloths that
              // hide the same ground at play height, and a different colour
            // again where the sun grazes the limb.
            if let Some(known) = veil
                && !known.knows(Vec3::new(x, h, z))
            {
                color[3] = 0.0;
            }
            colors.push(color);
        }
    }

    // Winding, tested on geometry: `(b - a) × (c - a) · outward > 0` means
    // `[a, b, c]` runs counter-clockwise seen from outside the sphere, which
    // is the front face. The first globe had the branches of this swapped
    // and the whole world rendered inside-out — the near half culled away,
    // the view landing on the interior of the far side, and every distance
    // a planet wrong.
    let pa = Vec3::from(positions[0]);
    let pb = Vec3::from(positions[1]);
    let pc = Vec3::from(positions[stride]);
    let outward_as_abc = (pb - pa).cross(pc - pa).dot(pa) > 0.0;
    for row in 0..n as u32 {
        for column in 0..n as u32 {
            let top_left = row * stride as u32 + column;
            let top_right = top_left + 1;
            let bottom_left = top_left + stride as u32;
            let bottom_right = bottom_left + 1;
            if outward_as_abc {
                indices.extend([top_left, top_right, bottom_left]);
                indices.extend([top_right, bottom_right, bottom_left]);
            } else {
                indices.extend([top_left, bottom_left, top_right]);
                indices.extend([top_right, bottom_left, bottom_right]);
            }
        }
    }

    // The skirt: every edge vertex gets a twin pulled toward the planet's
    // centre, and a wall of triangles joins the two rows. Neighbouring
    // patches at different depths do not share vertices, so hairline cracks
    // open along their seams; the skirt stands behind every crack wearing
    // the edge's own colour, and the crack shows skirt instead of sky. Both
    // windings are emitted — the wall must read from either side, and a
    // skirt's worth of overdraw is nothing.
    let drop = key.cell_arc() * 2.5;
    let edges: [Vec<(usize, usize)>; 4] = [
        (0..stride).map(|i| (i, 0)).collect(),
        (0..stride).map(|i| (i, n)).collect(),
        (0..stride).map(|j| (0, j)).collect(),
        (0..stride).map(|j| (n, j)).collect(),
    ];
    for edge in edges {
        let mut previous: Option<(u32, u32)> = None;
        for (gi, gj) in edge {
            let top_index = (gj * stride + gi) as u32;
            let here = Vec3::from(positions[top_index as usize]);
            let dir = here.normalize();
            let low_index = positions.len() as u32;
            positions.push((here - dir * drop).to_array());
            normals.push(normals[top_index as usize]);
            colors.push(colors[top_index as usize]);
            if let Some((last_top, last_low)) = previous {
                for triangle in [
                    [last_top, top_index, last_low],
                    [top_index, low_index, last_low],
                    [last_top, last_low, top_index],
                    [top_index, last_low, low_index],
                ] {
                    indices.extend(triangle);
                }
            }
            previous = Some((top_index, low_index));
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// The `(x, z)` the terrain speaks, for a direction in the scaffold's frame.
/// The minus mirrors `direction_at`'s: the game's north is -z.
pub(crate) fn ground_coordinates(dir: Vec3) -> (f32, f32) {
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    let lon = dir.x.atan2(dir.z);
    (lon * PLANET_RADIUS, -lat * PLANET_RADIUS)
}

/// Radial distance the surface is drawn at. Water is drawn AT its surface —
/// the sea at sea level, a lake at the lake's own level — because a globe
/// shows its waters, not its beds. Land is TRUE relief: the planet sits
/// underneath the flat world whenever the god is high enough to see past
/// the loaded ground, and an exaggerated mountain would tower up through
/// the real ground above it.
fn drawn_radial(h: f32, wet: f32) -> f32 {
    PLANET_RADIUS + WATER_LEVEL + (h.max(wet) - WATER_LEVEL).max(0.0) - PATCH_SINK
}

/// One vertex's colour: any standing water by depth in the water's own
/// ramp — the sea, and the lakes and rivers the courses know — the land by
/// the chunk painter itself, darkened under deep woods the way the ground
/// disappears under canopy from the air. The aim is that the planet at any
/// height is recognisably the ground the game is played on.
fn paint(terrain: &Terrain, x: f32, z: f32, h: f32, wet: f32, slope: f32) -> [f32; 4] {
    if h < wet {
        let depth = ((wet - h) / 8.0).clamp(0.0, 1.0);
        let shallow = palette::shade(&palette::WATER, 0.9).to_linear();
        let deep = palette::shade(&palette::WATER, 0.45).to_linear();
        return [
            shallow.red + (deep.red - shallow.red) * depth,
            shallow.green + (deep.green - shallow.green) * depth,
            shallow.blue + (deep.blue - shallow.blue) * depth,
            1.0,
        ];
    }

    let moisture = terrain.moisture_at(x, z);
    let shade_t =
        (0.42 + ((h - WATER_LEVEL) / 200.0) * 0.45 + (moisture - 0.5) * 0.12).clamp(0.0, 1.0);
    let color = crate::terrain::surface_color(
        h,
        slope,
        moisture,
        shade_t,
        terrain.biome_for(x, z, h),
        terrain.line_variation_at(x, z),
        terrain.ground_patch_at(x, z),
        None,
    )
    .to_linear();

    // Under dense woods the ground is canopy from above. The chunks carry
    // actual trees; the planet carries their shade, from the same forest
    // field the scatterer plants by, so a wooded valley is the same dark
    // green at every height.
    let woods = ((terrain.forest_at(x, z) - 0.5) * 2.0).clamp(0.0, 1.0);
    if woods > 0.0 && h > WATER_LEVEL + 0.5 && h < WATER_LEVEL + 78.0 {
        let canopy = palette::shade(&palette::GRASS, 0.34).to_linear();
        let t = woods * 0.55;
        return [
            color.red + (canopy.red - color.red) * t,
            color.green + (canopy.green - color.green) * t,
            color.blue + (canopy.blue - color.blue) * t,
            1.0,
        ];
    }
    [color.red, color.green, color.blue, 1.0]
}

// -------------------------------------------------------------- the tending

/// Grows and prunes the tree toward the camera, every frame.
///
/// The wanted cut is found by walking from the roots: a patch splits while
/// its cells subtend more than [`SPLIT_PX`] on screen. Wanted patches that
/// stand are shown; wanted patches still growing are queued nearest-first,
/// and their PARENT holds the frame until they arrive — the planet is never
/// allowed a hole, only allowed to be briefly coarse.
fn tend_the_tree(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    veil: (
        Option<Res<crate::fog::FogMode>>,
        Option<Res<crate::villager::explore::KnownWorld>>,
    ),
    mut tree: ResMut<PlanetTree>,
    mut meshes: ResMut<Assets<Mesh>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&GlobalTransform, &CameraRig), With<GodCamera>>,
    mut patches: Query<(&Patch, &mut Visibility)>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let Some(root) = tree.root else {
        return;
    };
    let Ok((camera, _)) = cameras.single() else {
        return;
    };
    // The camera, brought into the planet's own frame.
    let cam_mesh = planet_stance().inverse() * (camera.translation() - planet_centre());
    let px_per_radian = windows
        .single()
        .map(|w| w.resolution.height())
        .unwrap_or(1000.0)
        / 0.62;

    // Walk from the roots to the cut this frame wants.
    let mut wanted: Vec<PatchKey> = Vec::with_capacity(256);
    let mut walk: Vec<PatchKey> = (0..6u8)
        .map(|face| PatchKey {
            face,
            level: 0,
            x: 0,
            y: 0,
        })
        .collect();
    while let Some(key) = walk.pop() {
        let centre = key.centre_dir() * (PLANET_RADIUS + WATER_LEVEL);
        let reach = key.cell_arc() * PATCH_CELLS as f32 * 0.75;
        let distance = (cam_mesh.distance(centre) - reach).max(REFINE_FLOOR);
        let sharp_px = key.cell_arc() / distance * px_per_radian;
        if sharp_px > SPLIT_PX && key.level < MAX_LEVEL {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                walk.push(key.child(dx, dy));
            }
        } else {
            wanted.push(key);
        }
    }

    // Notice the veil changing — the F key, or the known world growing —
    // and mark every standing patch's painting stale by bumping the beat.
    let shroud = veil_of(&veil.0, &veil.1);
    let print = (
        shroud.is_some(),
        shroud.map_or(0, |k| k.radius as u32),
        shroud.map_or(0, |k| k.pockets.len()),
    );
    if tree.veil_print != print {
        tree.veil_print = print;
        tree.paint_beat += 1;
    }

    // Grow the missing and repaint the stale, nearest first, a few per
    // frame. A stale patch keeps its old painting on screen until its
    // replacement lands, so the veil sweeps across the world instead of
    // blinking it empty.
    let paint_beat = tree.paint_beat;
    let mut missing: Vec<PatchKey> = wanted
        .iter()
        .copied()
        .filter(|key| {
            tree.built
                .get(key)
                .is_none_or(|patch| patch.painted < paint_beat)
        })
        .collect();
    missing.sort_by(|a, b| {
        let da = cam_mesh.distance(a.centre_dir() * PLANET_RADIUS);
        let db = cam_mesh.distance(b.centre_dir() * PLANET_RADIUS);
        da.total_cmp(&db)
    });
    for key in missing.iter().take(BUILDS_PER_FRAME) {
        grow_patch(
            &mut commands,
            &terrain,
            shroud,
            &mut meshes,
            &mut tree,
            root,
            *key,
        );
    }

    // Resolve the cut to what can be shown today: a wanted patch that
    // stands shows itself, one still growing hands its ground to the
    // nearest standing ancestor. Then one pass settles every visibility.
    let mut on_screen: std::collections::HashSet<PatchKey> =
        std::collections::HashSet::with_capacity(wanted.len());
    for key in &wanted {
        let mut candidate = *key;
        loop {
            if tree.built.contains_key(&candidate) {
                on_screen.insert(candidate);
                break;
            }
            match candidate.parent() {
                Some(up) => candidate = up,
                None => break,
            }
        }
    }
    tree.beat += 1;
    let beat = tree.beat;
    for shown in &on_screen {
        if let Some(patch) = tree.built.get_mut(shown) {
            patch.last_shown = beat;
        }
    }
    for (patch, mut visibility) in &mut patches {
        let wanted = if on_screen.contains(&patch.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }

    // The forgetting. Every standing patch is a mesh in memory whether drawn
    // or not; ones the view has not wanted for a while are felled, entity and
    // mesh both, and the shallow levels are kept for ever as the floor the
    // fallback can always land on.
    let felled: Vec<PatchKey> = tree
        .built
        .iter()
        .filter(|(key, patch)| {
            key.level > EVERGREEN && beat.saturating_sub(patch.last_shown) > KEPT_FOR
        })
        .map(|(key, _)| *key)
        .collect();
    for key in felled {
        if let Some(patch) = tree.built.remove(&key) {
            commands.entity(patch.entity).despawn();
            meshes.remove(&patch.mesh);
        }
    }
}

// ----------------------------------------------------------------- the sky

/// Space is dark, and the climb to it starts before the chunks bow out.
///
/// The sky painter writes the horizon colour into the clear colour every
/// frame; this runs after it and takes the sky through what a climb should
/// look like — blue first, black only with real altitude.
fn dress_for_space(mut clear: ResMut<ClearColor>, cameras: Query<&CameraRig, With<GodCamera>>) {
    let Ok(rig) = cameras.single() else {
        return;
    };
    let height = rig.distance;

    if height <= ASCENT {
        return;
    }

    // The sky becomes a sky, and then becomes space. The horizon colour it
    // starts from is a neutral grey from the era when terrain had to
    // dissolve into it; above a genuinely curving horizon that grey read as
    // a dead band lying on the world, so the climb takes it to blue first
    // and only then thins it out to black.
    let mix =
        |a: bevy::color::LinearRgba, b: bevy::color::LinearRgba, t: f32| bevy::color::LinearRgba {
            red: a.red + (b.red - a.red) * t,
            green: a.green + (b.green - a.green) * t,
            blue: a.blue + (b.blue - a.blue) * t,
            alpha: 1.0,
        };
    let horizon = clear.0.to_linear();
    let sky_blue = palette::shade(&palette::SKY, 0.58).to_linear();
    let space = Color::srgb(0.004, 0.005, 0.012).to_linear();
    // Grey to blue across the first stretch of the climb...
    let bluing = ((height - ASCENT) / 4_000.0).clamp(0.0, 1.0);
    let skyed = mix(horizon, sky_blue, bluing);
    // ...then blue to black with real altitude, eased so the deep black is
    // kept for genuine orbit.
    let thinning = ((height - 6_000.0) / 14_000.0).clamp(0.0, 1.0);
    clear.0 = Color::LinearRgba(mix(skyed, space, thinning * thinning));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bend is a RENDER map, and its first duty is to leave the ground
    /// where the simulation put it. A flat position at the reference point
    /// seats at the tangent point, and the frame there stands up the same way
    /// the flat world did — so a village that has always been at the origin
    /// is drawn exactly where it was.
    #[test]
    fn the_bend_leaves_home_where_it_was() {
        let (seat, turn) = bend_frame(Vec3::new(0.0, WATER_LEVEL, 0.0));
        assert!(seat.length() < 9.0, "home moved to {seat}");
        let up = turn * Vec3::Y;
        assert!(up.dot(Vec3::Y) > 0.9999, "up leans to {up}");
    }

    /// The god's hand, and everything hanging off it, is left exactly where it
    /// put itself.
    ///
    /// It places itself in SEATED space — it has to, because as the UI pointer
    /// it floats five units in front of the camera and no flat position bends
    /// to "just in front of the camera" — so a bend applied to it is a bend
    /// applied twice. That is what hid the cursor on every panel and over the
    /// whole title screen, which is one big interface: the hand was flung to
    /// the seat of a point beside the god's eye, six thousand units out.
    ///
    /// Marking the root was not enough, and that is the point of this test.
    /// The bend rewrites `GlobalTransform` PER ENTITY, so each finger joint
    /// was still being seated from a global that was already seated — sending
    /// the fingers out to a radius of six thousand plus their own altitude
    /// while the palm stayed put. Every part carries `HandPart`.
    #[test]
    fn the_bend_never_touches_the_gods_hand() {
        let mut app = App::new();
        app.add_systems(Update, bend_the_world);

        let palm = GlobalTransform::from(Transform::from_xyz(20.0, 6_040.0, 5.0));
        let finger = GlobalTransform::from(Transform::from_xyz(20.3, 6_040.2, 4.6));
        let hand = app.world_mut().spawn((HandPart, palm)).id();
        let joint = app.world_mut().spawn((HandPart, finger)).id();
        // And an ordinary thing beside them, to prove the bend ran at all.
        let ordinary = app
            .world_mut()
            .spawn(GlobalTransform::from(Transform::from_xyz(20.0, 5.0, 5.0)))
            .id();

        app.update();

        let at = |entity| *app.world().get::<GlobalTransform>(entity).unwrap();
        assert_eq!(at(hand), palm, "the palm was bent");
        assert_eq!(at(joint), finger, "a finger was bent");
        assert_ne!(
            at(ordinary).translation(),
            Vec3::new(20.0, 5.0, 5.0),
            "the bend did not run, so this test proves nothing"
        );
    }

    /// Height is preserved exactly: a thing standing ten units above the
    /// ground stands ten units above the ground, everywhere on the planet.
    /// If this drifts, villagers sink or float.
    #[test]
    fn the_bend_keeps_every_height() {
        for &(x, z) in &[(0.0, 0.0), (900.0, -400.0), (-5_000.0, 3_000.0)] {
            let low = bend_frame(Vec3::new(x, WATER_LEVEL, z)).0;
            let high = bend_frame(Vec3::new(x, WATER_LEVEL + 10.0, z)).0;
            assert!(
                ((high - low).length() - 10.0).abs() < 0.01,
                "ten units became {}",
                (high - low).length()
            );
        }
    }

    /// Why chunk GEOMETRY is bent per vertex rather than each chunk being
    /// seated rigidly on its own tangent point — measured, because the rigid
    /// version was tried first on the strength of an eight-centimetre
    /// estimate that turned out to be for a bigger planet and a nearer point.
    /// At this radius a chunk's corner bows a fifth of a unit from its
    /// centre's frame and two thirds from its ORIGIN's, so neighbours seated
    /// by their own origins would disagree at every shared edge: a step at
    /// every seam in the world. Bent per vertex, a shared edge is the same
    /// world position for both chunks and gets the same seat, exactly.
    #[test]
    fn a_rigidly_seated_chunk_would_not_have_tiled() {
        let cell = crate::terrain::CHUNK_SIZE;
        let (seat, turn) = bend_frame(Vec3::new(0.0, WATER_LEVEL, 0.0));
        let corner = Vec3::new(cell, 0.0, cell);
        let rigid = seat + turn * corner;
        let honest = bend_frame(Vec3::new(corner.x, WATER_LEVEL, corner.z)).0;
        let bow = rigid.distance(honest);
        assert!(
            bow > 0.3,
            "a rigid chunk only bows {bow} - the per-vertex bend would be needless"
        );
    }

    /// The bend and its inverse are the same map read both ways. Every
    /// crossing between the flat simulation and the seated picture goes
    /// through one of them, so a drift here is a hand that misses, a click
    /// that lands elsewhere, a villager grabbed from empty air.
    #[test]
    fn the_bend_undoes_itself() {
        for &(x, y, z) in &[
            (0.0, WATER_LEVEL, 0.0),
            (420.0, 60.0, -180.0),
            (-3_000.0, 25.0, 2_400.0),
            (9_000.0, 140.0, -6_500.0),
        ] {
            let flat = Vec3::new(x, y, z);
            let there_and_back = unbend(bend_frame(flat).0);
            assert!(
                there_and_back.distance(flat) < 0.05,
                "{flat} came back as {there_and_back}"
            );
        }
    }

    /// And the curve is REAL at the scale the eye sees: across the streamed
    /// world the far edge drops a couple of hundred units below the tangent
    /// plane, which is the difference between a horizon and a plate.
    #[test]
    fn the_streamed_world_visibly_curves() {
        let rim = crate::terrain::CHUNK_SIZE * crate::terrain::VIEW_CHUNKS as f32;
        let far = bend_frame(Vec3::new(rim, WATER_LEVEL, 0.0)).0;
        let drop = -far.y;
        assert!(
            drop > 100.0,
            "the streamed rim only drops {drop} units - the world is flat"
        );
    }

    /// Leaving orbit must land where the orbit was looking: direction to
    /// ground words to direction is the identity, sign conventions and all.
    /// The mirror bug lived precisely here.
    #[test]
    fn the_way_down_lands_where_you_looked() {
        for i in 0..500 {
            let golden = 2.399963_f32;
            let y = 0.995 - 1.99 * (i as f32 + 0.5) / 500.0;
            let r = (1.0 - y * y).sqrt();
            let a = golden * i as f32;
            let dir = Vec3::new(r * a.cos(), y, r * a.sin());
            let (x, z) = ground_coordinates(dir);
            let back = crate::terrain::direction_at(x, z);
            assert!(
                dir.dot(back) > 0.999_99,
                "direction {dir} came back as {back}"
            );
        }
    }

    /// The compass agrees between ground and globe: a point the game calls
    /// north of another (pan-north walks along -z) sits at higher latitude.
    #[test]
    fn north_from_orbit_is_north_on_the_ground() {
        let here = crate::terrain::direction_at(0.0, 0.0);
        let north_of_here = crate::terrain::direction_at(0.0, -600.0);
        assert!(
            north_of_here.y > here.y,
            "walking north lowered the latitude: the globe is a mirror"
        );
    }

    /// Sea paints blue, lowland paints green, at every vertex a patch would
    /// ask about. When this fails the planet has quietly gone blank again.
    #[test]
    fn the_planet_is_blue_where_it_is_wet_and_green_where_it_is_not() {
        let t = Terrain::new(4242);
        let mut sea = (0.0f32, 0);
        let mut land = (0.0f32, 0);
        for i in 0..2000 {
            let golden = 2.399963_f32;
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / 2000.0;
            let r = (1.0 - y * y).sqrt();
            let a = golden * i as f32;
            let dir = Vec3::new(r * a.cos(), y, r * a.sin());
            let (x, z) = ground_coordinates(dir);
            let h = t.base_height_at(x, z);
            let c = paint(&t, x, z, h, WATER_LEVEL, 0.05);
            if h <= WATER_LEVEL {
                sea = (sea.0 + c[2] - c[1].max(c[0]), sea.1 + 1);
            } else if h < WATER_LEVEL + 70.0 {
                land = (land.0 + c[1] - c[0].max(c[2]), land.1 + 1);
            }
        }
        assert!(sea.1 > 100 && land.1 > 100, "world too uniform to judge");
        assert!(sea.0 / sea.1 as f32 > 0.0, "the sea is not blue");
        assert!(land.0 / land.1 as f32 > 0.0, "the lowland is not green");
    }

    /// Children tile their parent exactly and name it as their parent: the
    /// tree is names, and the names have to be sound.
    #[test]
    fn children_tile_their_parent() {
        let parent = PatchKey {
            face: 3,
            level: 4,
            x: 9,
            y: 2,
        };
        let (u0, v0, side) = parent.rect();
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let child = parent.child(dx, dy);
            assert_eq!(child.parent(), Some(parent));
            let (cu, cv, cside) = child.rect();
            assert!((cside - side / 2.0).abs() < 1e-6);
            assert!(cu >= u0 - 1e-6 && cu + cside <= u0 + side + 1e-6);
            assert!(cv >= v0 - 1e-6 && cv + cside <= v0 + side + 1e-6);
        }
    }

    /// A deeper patch has finer cells, halving cleanly — the whole point of
    /// the tree, stated as arithmetic — and the deepest level reaches the
    /// chunk world's own order of detail.
    #[test]
    fn depth_means_detail() {
        let coarse = PatchKey {
            face: 0,
            level: 0,
            x: 0,
            y: 0,
        };
        let fine = PatchKey {
            face: 0,
            level: MAX_LEVEL,
            x: 100,
            y: 40,
        };
        let halvings = (1u32 << MAX_LEVEL) as f32;
        assert!((coarse.cell_arc() / fine.cell_arc() - halvings).abs() < 1e-3);
        assert!(fine.cell_arc() < 3.0, "{}", fine.cell_arc());
    }

    /// A patch mesh holds together: grid plus skirt, a colour for every
    /// vertex.
    #[test]
    fn a_patch_stands_scrutiny() {
        let t = Terrain::new(7);
        let mesh = build_patch(
            &t,
            None,
            PatchKey {
                face: 2,
                level: 3,
                x: 5,
                y: 1,
            },
        );
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len();
        let expected_grid = (PATCH_CELLS + 1) * (PATCH_CELLS + 1);
        let expected_skirt = 4 * (PATCH_CELLS + 1);
        assert_eq!(positions, expected_grid + expected_skirt);
        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap().len();
        assert_eq!(colors, positions);
    }
}
