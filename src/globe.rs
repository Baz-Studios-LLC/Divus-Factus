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
/// TWO. Layer 0 is the world, 1 the hand, 3 the deity alcove, 4 the paperdoll
/// - which was on 2 first, and whose studio key relit the whole night sky when
/// this constant moved in beside it. If a new layer is ever needed, count the
/// neighbors first: a light on a layer the god camera renders lights
/// EVERYTHING the god camera sees.
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
/// fight for every pixel. That fight was the gray striping across the whole
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

/// How far under `SPLIT_PX` the error must fall before ground already drawn at
/// a finer level is allowed to coarsen again. This intentionally leaves a
/// broad overlap with the chunk handoff: a small pull on the zoom must not
/// replace the country beneath the player with a visibly coarser planet.
/// See the cut in `tend_the_tree`.
const MERGE_HYSTERESIS: f32 = 0.45;

/// How far the planet's water sits below true level: enough to keep out of the
/// chunks' own sea, and no more than that.
///
/// A twentieth of a unit. It was one whole unit, which sounded like nothing and
/// was not: the ground under the two waters does not meet either - patch relief
/// is sunk by `PATCH_SINK` so it cannot fight the chunks - so at the streamed
/// edge there is a step in the BED, and any gap between the two water surfaces
/// is a window onto it. A staircase of chunk-sized treads ran round the whole
/// loaded region with a brown seabed face under each one.
///
/// Held this close the two seas are one surface to the eye, and whatever the
/// bed does underneath is the water's business. Nothing fights, because within
/// the streamed radius - the only place both are drawn - a twentieth of a unit
/// is many depth values wide.
///
/// See `build_patch_water`.
const WATER_CLEARANCE: f32 = 0.05;

/// Patches built per frame while the tree is chasing the camera. A build is
/// a couple of milliseconds of noise sampling; four a frame chases a fast
/// descent closely without stuttering the simulation, and a patch that
/// arrives a beat late only means the ground is briefly coarser there —
/// the same bargain streaming ground has always made.
const BUILDS_PER_FRAME: usize = 6;

/// The most per frame when there is a lot to do — the same bargain the chunk
/// streamer already makes, and for the same reason: the point of rationing is
/// to HIDE the arriving ground, and six a frame stops hiding anything the
/// moment several hundred patches come due at once. Measured on a descent to
/// the village, eighteen hundred patches fall due together and six a frame
/// takes five seconds of watching the planet sharpen; this brings it under two.
const BUILDS_PER_HURRIED_FRAME: usize = 26;

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

/// Where the planet's center sits: the sea-level sphere tangent to the flat
/// world at the origin, sunk a few units so the two surfaces — the same
/// field, so they agree exactly at the tangent point — never fight over the
/// same pixels while both are drawn.
pub(crate) fn planet_center() -> Vec3 {
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
    fn center_dir(self) -> Vec3 {
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

    /// Half this patch's own angular reach, center to corner, in radians.
    ///
    /// Generous on purpose. [`cell_arc`](Self::cell_arc) is an AVERAGE over a
    /// cube face and the projection is not even: a patch in the middle of a
    /// face covers about a quarter more arc per cube unit than the average
    /// says, and one out at a corner rather less. Under-reaching here culls
    /// patches with a corner still in view, so the widest case is the one
    /// taken.
    fn half_span(self) -> f32 {
        // Half a diagonal (root two over two), times the face-middle stretch
        // (one over pi-quarters).
        self.cell_arc() * PATCH_CELLS as f32 * 0.901 / PLANET_RADIUS
    }

    /// Whether every part of this patch lies beyond the planet's own limb from
    /// an eye at `eye`, in the planet's frame.
    ///
    /// This is ground the camera cannot see however it turns, because the world
    /// itself is in the way — and the tree was refining it, drawing it and
    /// keeping it in memory anyway. It has no frustum test and deliberately
    /// still has none: turn the camera and ground outside the frame must
    /// already be sharp, or every pan sweeps a wave of coarse ground across
    /// the screen. The limb is different. Nothing brings that ground into view
    /// but MOVING, and moving rebuilds the cut anyway.
    ///
    /// The size of it: from four hundred units up, the visible cap is about six
    /// percent of the sphere. Ninety-four percent of the planet was being
    /// tended for nobody, and because the shallow levels are evergreen (see
    /// `EVERGREEN`) two thousand of those patches never even went away.
    fn over_the_limb(self, eye: Vec3) -> bool {
        let height = eye.length();
        // The horizon of the LOWEST ground there is, not of sea level, so a
        // mountain standing on the far slope still counts as visible. Clamped
        // so an eye somehow inside the world cannot hand `acos` a number over
        // one and cull the entire planet on a NaN.
        let floor = (PLANET_RADIUS - crate::terrain::TERRAIN_HEIGHT).min(height * 0.999);
        let horizon = (floor / height).acos();

        // How far round the world this patch sits from the point under the
        // eye. By `atan2` and not `acos`: near the nadir the two vectors are
        // nearly parallel and `acos` throws away most of the significant
        // figures right where the answer decides the finest patches in the
        // world. The same lesson as `Place::angle_to`.
        let nadir = eye.normalize_or(Vec3::Y);
        let here = self.center_dir();
        let away = here.cross(nadir).length().atan2(here.dot(nadir));

        // Its own half-span back off, so a patch STRADDLING the limb is kept.
        // Without this a root face is culled while a quarter of it is still on
        // screen, and the cull walks a bite out of the world's edge.
        away - self.half_span() > horizon
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
pub(crate) struct PlanetTree {
    root: Option<Entity>,
    built: HashMap<PatchKey, Patch2>,
    /// The seed the tree was grown for, so a new world fells the old tree.
    grown_for: Option<u32>,
    /// Frames since the world began, for the forgetting below.
    beat: u64,
    /// Which painting of the veil the world currently wants. Bumped when the
    /// fog of war TOGGLES - a change with no locality - so every patch
    /// repaints, a few per frame, and the veil SWEEPS rather than hitches.
    /// The known world merely GROWING no longer bumps it: growth stales
    /// only the patches it touches, by zeroing their own `painted`.
    paint_beat: u64,
    /// The veil as the planet last painted it, kept to tell WHAT changed.
    veil_seen: Option<VeilSeen>,
}

/// The veil's shape at one painting: the home circle and the pockets, in
/// the terrain's own `(x, z)`. Two of these answer the only question the
/// planet has - which GROUND would these paint differently?
#[derive(PartialEq)]
struct VeilSeen {
    center: Vec2,
    radius: f32,
    pockets: Vec<(Vec2, f32)>,
}

impl VeilSeen {
    fn of(known: &crate::villager::explore::KnownWorld) -> Self {
        VeilSeen {
            center: Vec2::new(known.center.x, known.center.z),
            radius: known.radius,
            pockets: known
                .pockets
                .iter()
                .map(|pocket| (Vec2::new(pocket.at.x, pocket.at.z), pocket.radius))
                .collect(),
        }
    }

    /// The discs of ground where these two veils could disagree: every
    /// pocket present in exactly one of them, and both home circles when
    /// the home moved or grew. Everywhere else, both paint alike.
    fn changed_discs(&self, fresh: &Self) -> Vec<(Vec2, f32)> {
        let mut discs = Vec::new();
        if self.center != fresh.center || self.radius != fresh.radius {
            discs.push((self.center, self.radius));
            discs.push((fresh.center, fresh.radius));
        }
        // Set difference both ways: `tidy` merges and reorders pockets,
        // so positions are compared, never indices.
        let key = |(at, radius): &(Vec2, f32)| {
            (
                (at.x * 8.0) as i64,
                (at.y * 8.0) as i64,
                (radius * 8.0) as i64,
            )
        };
        let old: std::collections::HashSet<_> = self.pockets.iter().map(key).collect();
        let new: std::collections::HashSet<_> = fresh.pockets.iter().map(key).collect();
        discs.extend(self.pockets.iter().filter(|p| !new.contains(&key(p))));
        discs.extend(fresh.pockets.iter().filter(|p| !old.contains(&key(p))));
        discs
    }
}

/// The direction a terrain `(x, z)` bends to - the inverse of
/// [`ground_coordinates`], minus sign and all.
fn chart_direction(at: Vec2) -> Vec3 {
    let lon = at.x / PLANET_RADIUS;
    let lat = -at.y / PLANET_RADIUS;
    Vec3::new(lon.sin() * lat.cos(), lat.sin(), lon.cos() * lat.cos())
}

impl PlanetTree {
    /// How many patches are built. For the planet bench's readout.
    pub(crate) fn standing(&self) -> usize {
        self.built.len()
    }
}

/// "Seat my ROOT, and my parts follow ME" - for anything assembled from
/// pieces that must stay square to one another.
///
/// The bend seats every entity individually by its own flat position, and
/// near home the pieces of a house disagree by millimeters. Near the POLES
/// they do not: the flat chart's longitude lines converge there, each part
/// of one roof gets a visibly different frame, and a village founded on the
/// far side of the world came out of the ground as cubism - Brett: "my
/// house looks like a picasso painting, lol." A building is rigid the way
/// the camera is a look-at: one part of the picture the bend must not tear.
#[derive(Component)]
pub struct RigidlySeated;

/// Reseats every part of a rigid assembly in ITS ROOT'S bent frame, after
/// the bend has seated the roots. Children were already seated one by one -
/// wrongly, near the poles - and this overwrites them with the root's frame
/// carried down the tree, which is what "rigid" means.
fn seat_the_rigid(
    roots: Query<(Entity, &GlobalTransform), With<RigidlySeated>>,
    children: Query<&Children>,
    mut parts: Query<(&Transform, &mut GlobalTransform), Without<RigidlySeated>>,
) {
    for (root, root_seat) in &roots {
        let mut walk: Vec<(Entity, GlobalTransform)> = children
            .get(root)
            .into_iter()
            .flatten()
            .map(|child| (*child, *root_seat))
            .collect();
        while let Some((part, above)) = walk.pop() {
            let Ok((local, mut seat)) = parts.get_mut(part) else {
                continue;
            };
            let seated = above.mul_transform(*local);
            *seat.bypass_change_detection() = seated;
            for grandchild in children.get(part).into_iter().flatten() {
                walk.push((*grandchild, seated));
            }
        }
    }
}

/// "My vertices are already seated on the sphere — leave my transform alone."
///
/// The bend's whole contract is that it seats FLAT things, and a growing number
/// of meshes in this world are built already-bent, vertex by vertex: the chunks,
/// their rivers, the veil's cloths. Those get their positions from
/// [`bend_frame`] at build time, so bending them again seats a position that was
/// never flat. Chunks and cloths each earned their own exclusion by name as
/// they were caught; this is the concept those names were groping at, so
/// anything built in world space can say so once.
///
/// The river said it in a comment and not in the world, which is how it came to
/// be buried: a river is a CHILD of its chunk, the chunk is excluded but the
/// child was not, and bending an identity transform seats the origin at
/// twenty-eight units below the ground. Every river in the world sank out of
/// sight, leaving its carved channel painted on an empty valley floor.
#[derive(Component)]
pub struct BentInPlace;

/// What the planet is drawn at, right now, for the developer's panel.
///
/// The altitude row answers how high the god is; this answers what that HEIGHT
/// bought — which depths of the tree are on screen, how many patches that is,
/// and whether any of them are still being built. Which is the number to have
/// in front of you while judging how the world sharpens.
#[derive(Resource, Default, Debug)]
pub struct PlanetDetail {
    /// Shallowest and deepest levels currently shown.
    pub coarsest: u8,
    pub finest: u8,
    /// Patches on screen, patches resident, and patches still owed.
    pub shown: usize,
    pub built: usize,
    pub owed: usize,
}

/// A standing patch: its entity, its mesh, the last frame it was on screen —
/// the tree forgets what it has not shown for a while — and which painting
/// of the veil it wears, so the fog of war can sweep across a planet that
/// already stands.
struct Patch2 {
    entity: Entity,
    mesh: Handle<Mesh>,
    /// The beat this patch was grown on.
    ///
    /// A patch enters `built` the instant it is asked for, but its ENTITY is
    /// spawned through `Commands` and does not exist until the schedule
    /// flushes. For the rest of that frame the cut believed the new patch was
    /// standing, handed it the ground, and hid the ancestor that had been
    /// covering it - so there was one frame with nothing drawn there at all.
    /// Moving, that is a ring of holes opening and closing around the camera
    /// as fast as patches are built, which is what flashed.
    ///
    /// So a patch is not STANDING until a beat has passed. See the ancestor
    /// walk in `tend_the_tree`.
    grown_at: u64,
    /// The sea, lakes and rivers standing on this patch, if any stand on it.
    /// A child entity, so it inherits the patch's visibility and is felled
    /// with it; the mesh is held so it can be dropped from the assets too.
    water: Option<Handle<Mesh>>,
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
/// where streaming has always happened. The bake is a couple of seconds
/// inside the loading screen, and the fallback for a still-growing patch
/// always finds an ancestor standing.
///
/// Four rather than three, which is the fix for the tiles Brett watched
/// generate on the way out. They were BUILT at creation already — the bake
/// has always gone one level past the evergreens — but level four was
/// evictable, so half an hour in the village felled the lot of them and the
/// climb out had to make them again. Measured: eight hundred and twenty-four
/// patches felled by a spell at play zoom, and level-four tiles are five
/// hundred and eighty-nine units across, which is exactly the size of thing
/// that was appearing. Now the whole planet at that grain is resident for the
/// life of the world.
///
/// Not five. Level five alone is six thousand patches, ten times the bake and
/// most of a gigabyte of vertex buffers, and its tiles are only wanted below
/// about three thousand units — where the chunk world is drawing the detail
/// anyway. Four is the grain the eye catches on the way out.
const EVERGREEN: u8 = 4;

/// The one material every patch wears; the colors ride the vertices.
#[derive(Resource)]
struct PlanetSkin(Handle<PlanetMaterial>);

/// The same skin, cut for water: it carries the veil the way the ground does,
/// so the sea darkens with the coast instead of glowing through it.
#[derive(Resource)]
struct PlanetSeaSkin(Handle<PlanetMaterial>);

/// Ordinary ground, plus the fog of war mixed in AFTER the lighting.
///
/// The cloths that hide unknown ground at play height are unlit — their shader
/// hands back its tint and that is the pixel. A patch is lit ground, so the
/// same color painted into its vertices came out somewhere else entirely: the
/// sun's diffuse and its specular sheen both add to it, and the veil read half
/// again as far toward white, less blue, and lighter still at a grazing angle
/// near the limb. Three shades of one fog in a single frame. Worse, any tuning
/// of it would only hold for the light it was tuned under.
///
/// So the mix happens in the fragment shader, past the lighting, and the answer
/// is the cloth's color under any sun at all.
pub type PlanetMaterial = ExtendedMaterial<StandardMaterial, VeilExtension>;

/// Which color the veil is, and how much of it fully-veiled ground takes.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct VeilExtension {
    /// rgb the veil's color; a the weight at full veil.
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
pub(crate) struct Patch(PatchKey);

pub struct GlobePlugin;

impl Plugin for GlobePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PlanetMaterial>::default())
            .init_resource::<PlanetTree>()
            .init_resource::<PlanetDetail>()
            .add_systems(
                Update,
                (plant_the_tree, dress_the_patches, dress_the_patch_water)
                    .run_if(resource_exists::<Terrain>),
            )
            .add_systems(
                Update,
                tend_the_tree
                    .after(crate::camera::CameraSet)
                    // The TITLE too, and not only the world afoot. Patches are
                    // spawned hidden and this is what shows them, so while the
                    // condition was `Choosing | Playing` the planet was simply
                    // not drawn on the title screen - which mattered not at all
                    // when the title looked down at a valley from a hundred and
                    // seventy-five units, and matters entirely now that the
                    // title IS the planet. What was on screen was the cloud
                    // shell hanging in empty space.
                    .run_if(crate::world_is_afoot.or_else(in_state(crate::GameState::Title))),
            )
            .add_systems(Update, dress_for_space.after(crate::render::paint_the_sky))
            // The bend runs at the very end of the transform pipeline: after
            // propagation has written every entity's final flat pose, before
            // the renderer reads it. That ordering IS the design - the
            // simulation keeps its flat world, the picture gets the sphere.
            .add_systems(
                PostUpdate,
                (bend_the_world, seat_the_rigid)
                    .chain()
                    .after(bevy::transform::TransformSystems::Propagate),
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
/// planet's center, and the rotation turns "up" into the local outward
/// radial, east into east along the curve. Applied rigidly to a whole chunk
/// the error within its sixty-four units is eight centimeters, which is why
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
    let seat = planet_center() + up * (PLANET_RADIUS + flat.y);
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
    // From the rig's CARRIED frame, not from the focus's longitude and
    // latitude. See `CameraRig::facing`: a frame derived from lat/lon has
    // poles in it however round the planet is, and the camera would inherit
    // them - twisting harder and harder as it neared one, and flipping end for
    // end across it.
    //
    // The eye is a rigid offset inside that frame. It used to be seated on its
    // own, by its own coordinates, which is what forced all the special
    // handling here: at play pitch the eye stands two and a half thousand
    // units from its focus, twenty-five degrees of arc on a six thousand unit
    // world, so seating it by its own place dropped it a quarter-continent
    // away and turned it into THAT local frame, staring at unrelated ground. A
    // rig sitting above one point does not have that problem, because it is
    // one rigid thing over one place.
    let frame = rig.facing;
    let focus_seat = planet_center() + (frame * Vec3::Y) * (PLANET_RADIUS + rig.focus.y);
    let eye_seat = focus_seat + (frame * rig.eye_offset());
    if rig.distance < crate::camera::FIRST_PERSON {
        // The ride, narrated: where the frame thinks up is against where up
        // really is, and how high the eye actually sits. The avatar broke on
        // the round world and three code readings cleared three suspects -
        // the probe rule stands. `DIVUS_FACTUS_AVATAR_PROBE=1`.
        if std::env::var("DIVUS_FACTUS_AVATAR_PROBE").is_ok() {
            let outward = planet_stance() * crate::terrain::direction_at(rig.focus.x, rig.focus.z);
            let up_err = (frame * Vec3::Y).angle_between(outward).to_degrees();
            bevy::log::info!(
                "avatar probe: focus {:?} seat_alt {:.1} up_err {up_err:.2}deg fwd_dot_up {:.2}",
                rig.focus,
                eye_seat.distance(planet_center()) - PLANET_RADIUS,
                (frame * rig.forward()).dot((eye_seat - planet_center()).normalize()),
            );
        }
        // No separation to look along; carry the flat gaze into the seat.
        Transform {
            translation: eye_seat,
            rotation: frame
                * Transform::default()
                    .looking_to(rig.forward(), Vec3::Y)
                    .rotation,
            scale: Vec3::ONE,
        }
    } else {
        let up = frame * Vec3::Y;
        let gaze = (focus_seat - eye_seat).normalize_or(-up);
        // Looking almost STRAIGHT DOWN, the local up is no use as a hint - it is
        // nearly antiparallel to the gaze, `looking_at` has no plane left to
        // work in, and the camera's roll becomes whatever the fallback inside it
        // happens to pick. On a sphere that is invisible, which is how it went
        // unnoticed: nothing in the picture told, until the title screen tried to
        // put a hand UNDER the planet and the hand came out at ten o'clock.
        //
        // From near the vertical, the honest screen-up is the bearing the rig is
        // facing - the convention every top-down map keeps.
        let up = if gaze.dot(up).abs() > 0.985 {
            frame * rig.ground_forward()
        } else {
            up
        };
        let mut pose = Transform::from_translation(eye_seat).looking_at(focus_seat, up);
        // And then turned off its aim, if anything has asked it to — see
        // `CameraRig::aim_offset`. About the camera's own axes, so the offset
        // means the same thing at every attitude.
        if rig.aim_offset != Vec2::ZERO {
            let right = pose.right().as_vec3();
            let above = pose.up().as_vec3();
            pose.rotation = Quat::from_axis_angle(above, rig.aim_offset.x)
                * Quat::from_axis_angle(right, rig.aim_offset.y)
                * pose.rotation;
        }
        pose
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
    let from_center = seat - planet_center();
    let radius = from_center.length().max(1.0);
    let dir = planet_stance().inverse() * (from_center / radius);
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
            // chunks and their rivers.
            Without<crate::terrain::TerrainChunk>,
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
            // The sun and the moon are out in space already; their places are
            // world positions, not flat ground waiting to be wrapped.
            Without<crate::calendar::Celestial>,
            // And the weather is a shell already standing round the planet.
            Without<crate::clouds::CloudShell>,
            // Anything else built already-bent; see `BentInPlace`.
            Without<BentInPlace>,
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
        // Through `bent_camera_pose`, which is the ONE definition of where the
        // camera is. This loop used to compute it again inline - the same
        // arithmetic, written twice - and the copies drifted the moment one of
        // them was fixed: the top-down roll was corrected in the shared version
        // for the title screen's sake while the RENDERER went on using this one,
        // so the hand placed itself in a frame the picture did not share and
        // came out beside the planet instead of under it.
        let bent = crate::globe::bent_camera_pose(rig);
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
    state: Res<State<crate::GameState>>,
) {
    let shroud = veil_of(&veil.0, &veil.1, state.get());
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
            Transform::from_translation(planet_center()).with_rotation(planet_stance()),
            Visibility::Inherited,
        ))
        .id();
    tree.root = Some(root);

    // No sun of its own any more. The planet used to carry a second one,
    // because the world's lights covered only the world's layers and a planet
    // without one hangs in space unlit - but it was fixed at a bearing and
    // never told the hour, so the ball stayed in a permanent mid-morning while
    // the ground ran through its day, and the terminator the round world
    // should have had was nowhere. The real sun covers this layer now; see
    // `main::setup` and `calendar::apply_sky_to_lights`.

    let veil_paint = || VeilExtension {
        // Unknown ground is solid at play height, and the far planet uses
        // the same rule so climbing cannot make the shroud transparent.
        tint: Vec4::new(
            crate::fog::VEIL_TINT[0],
            crate::fog::VEIL_TINT[1],
            crate::fog::VEIL_TINT[2],
            1.0,
        ),
    };
    commands.insert_resource(PlanetSkin(materials.add(PlanetMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 1.0,
            ..default()
        },
        extension: veil_paint(),
    })));

    // THE SEA WEARS THE VEIL TOO. It used to wear a plain `StandardMaterial`,
    // which has no way to read the mark - so every ocean on the planet lay in
    // full daylight while the continents around it went dark, and knowledge
    // was reduced to deciding whether the water was BUILT at all. Same shader,
    // same tint, water's own finish: smooth and glossy where the ground is
    // rough, so the sea still reads as sea.
    commands.insert_resource(PlanetSeaSkin(materials.add(PlanetMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.12,
            ..default()
        },
        extension: veil_paint(),
    })));

    // Exactly the evergreens, and no deeper. The bake used to go one level
    // past them, on the theory that the extra level cost nothing permanent
    // because it could be evicted - and then it WAS evicted, and the climb
    // out rebuilt it in front of the player. That level is evergreen now, so
    // the bake and the resident set are the same set: the whole planet at
    // five-hundred-unit tiles, built once in the loading screen and never
    // built again. One deeper is ten times the patches; see `EVERGREEN`.
    const PREBAKED: u8 = EVERGREEN;
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
        if let Some(water) = old.water {
            meshes.remove(&water);
        }
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
    // And the water standing on it, as a child so it is shown, hidden and
    // felled with the ground it covers. Render layers are not inherited, so
    // it states its own.
    let water = build_patch_water(terrain, veil, key).map(|sheet| {
        let handle = meshes.add(sheet);
        commands.spawn((
            PatchWater,
            Mesh3d(handle.clone()),
            Transform::default(),
            Visibility::Inherited,
            RenderLayers::layer(GLOBE_LAYER),
            NotShadowCaster,
            NotShadowReceiver,
            ChildOf(entity),
        ));
        handle
    });

    let beat = tree.beat;
    let painted = tree.paint_beat;
    tree.built.insert(
        key,
        Patch2 {
            entity,
            mesh,
            water,
            grown_at: beat,
            last_shown: beat,
            painted,
        },
    );
    entity
}

/// The sea standing on one patch. See `build_patch_water`.
#[derive(Component)]
struct PatchWater;

/// Hands the water shader to any patch sea that lacks it, the same way
/// `dress_the_patches` hands out the skin - so patch growth never has to know
/// what water looks like.
fn dress_the_patch_water(
    mut commands: Commands,
    skin: Option<Res<PlanetSeaSkin>>,
    bare: Query<Entity, (With<PatchWater>, Without<MeshMaterial3d<PlanetMaterial>>)>,
) {
    let Some(skin) = skin else {
        return;
    };
    for sea in &bare {
        // try_insert, not insert: clicking Title tears the old world down
        // in the same frame this queued, and a command applied into a
        // freshly despawned patch panicked the whole game. Dressing a
        // patch that died mid-frame is legitimately skippable.
        commands
            .entity(sea)
            .try_insert(MeshMaterial3d(skin.0.clone()));
    }
}

/// Hands the shared material to any patch that lacks it. Separate from the
/// spawning so patch growth never has to thread the material through.
fn dress_the_patches(
    mut commands: Commands,
    skin: Option<Res<PlanetSkin>>,
    bare: Query<Entity, (With<Patch>, Without<MeshMaterial3d<PlanetMaterial>>)>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("globe: dress_the_patches");
    let Some(skin) = skin else {
        return;
    };
    for patch in &bare {
        // try_insert for the same teardown race as the water above.
        commands
            .entity(patch)
            .try_insert(MeshMaterial3d(skin.0.clone()));
    }
}

/// The veil to paint with, if the fog of war is on, a known world exists, and
/// there is a village whose knowledge it could be.
///
/// That last condition is not pedantry. The fog of war is what a VILLAGE has
/// walked, and before one is founded there is nobody to have walked anything —
/// which is why the cloths over the near ground have always waited for
/// `Playing` (see `fog::drape_the_veil`). The planet's own paint did not, so the
/// title screen showed a world shrouded pole to pole with one small clearing
/// where the village was going to be: the first thing the game said to the
/// player was that they could not see it.
fn veil_of<'k>(
    mode: &Option<Res<crate::fog::FogMode>>,
    known: &'k Option<Res<crate::villager::explore::KnownWorld>>,
    state: &crate::GameState,
) -> Option<&'k crate::villager::explore::KnownWorld> {
    if *state != crate::GameState::Playing {
        return None;
    }
    let veiled = mode.as_ref().is_none_or(|mode| mode.0);
    if !veiled {
        return None;
    }
    known.as_ref().map(|known| &**known)
}

// -------------------------------------------------------------- the building

/// Samples one patch of the world: heights and paint from the same fields
/// the chunks are built from, and a skirt around the edge to curtain the
/// hairline cracks where neighboring patches meet at different depths.
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
            // The ground as the world actually stands - channels cut, village
            // ground leveled - and the water on it, asked once. See
            // `Terrain::ground_and_water_at` for what asking twice cost.
            let (h, wet) = terrain.ground_and_water_at(x, z);
            grid.push(dir * drawn_radial(h));
            ground.push((x, z, h, wet));
        }
    }
    let at = |gi: usize, gj: usize| grid[gj * padded + gi];

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(stride * stride + stride * 4);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(positions.capacity());
    let mut indices: Vec<u32> = Vec::with_capacity(n * n * 6 + n * 24);

    for gj in 0..stride {
        for gi in 0..stride {
            let (pi, pj) = (gi + 1, gj + 1);
            let here = at(pi, pj);
            let dir = here.normalize();

            let across = at(pi + 1, pj) - at(pi - 1, pj);
            let down = at(pi, pj + 1) - at(pi, pj - 1);
            let mut normal = across.cross(down).normalize_or(dir);
            if normal.dot(dir) < 0.0 {
                normal = -normal;
            }

            positions.push(here.to_array());
            normals.push(normal.to_array());
        }
    }

    let colors = paint_patch_colors(terrain, veil, key);

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
    // center, and a wall of triangles joins the two rows. Neighboring
    // patches at different depths do not share vertices, so hairline cracks
    // open along their seams; the skirt stands behind every crack wearing
    // the edge's own color, and the crack shows skirt instead of sky. Both
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
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// Computes vertex colors and veil alpha for one patch of the globe.
fn paint_patch_colors(
    terrain: &Terrain,
    veil: Option<&crate::villager::explore::KnownWorld>,
    key: PatchKey,
) -> Vec<[f32; 4]> {
    let n = PATCH_CELLS;
    let stride = n + 1;
    let padded = n + 3;
    let (u0, v0, side) = key.rect();
    let (outward, along_u, along_v) = face_axes(key.face);
    let step = side / n as f32;

    let mut grid: Vec<Vec3> = Vec::with_capacity(padded * padded);
    let mut ground: Vec<(f32, f32, f32, f32)> = Vec::with_capacity(padded * padded);
    for gj in 0..padded {
        for gi in 0..padded {
            let u = u0 + (gi as f32 - 1.0) * step;
            let v = v0 + (gj as f32 - 1.0) * step;
            let dir = (outward + along_u * u + along_v * v).normalize();
            let (x, z) = ground_coordinates(dir);
            let (h, wet) = terrain.ground_and_water_at(x, z);
            grid.push(dir * drawn_radial(h));
            ground.push((x, z, h, wet));
        }
    }
    let at = |gi: usize, gj: usize| grid[gj * padded + gi];

    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(stride * stride + stride * 4);

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

            let mut color = paint(terrain, x, z, h, wet, slope);
            if let Some(known) = veil
                && !known.knows(Vec3::new(x, h, z))
            {
                color[3] = 0.0;
            }
            colors.push(color);
        }
    }

    let edges: [Vec<(usize, usize)>; 4] = [
        (0..stride).map(|i| (i, 0)).collect(),
        (0..stride).map(|i| (i, n)).collect(),
        (0..stride).map(|j| (0, j)).collect(),
        (0..stride).map(|j| (n, j)).collect(),
    ];
    for edge in edges {
        for (gi, gj) in edge {
            let top_index = gj * stride + gi;
            colors.push(colors[top_index]);
        }
    }

    colors
}

/// Warms up and repaints the veil on an existing patch mesh immediately in-place
/// without expensive terrain noise re-sampling.
fn repaint_patch_veil(
    mesh: &mut Mesh,
    veil: Option<&crate::villager::explore::KnownWorld>,
    key: PatchKey,
) {
    let Some(bevy::render::mesh::VertexAttributeValues::Float32x4(colors)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
    else {
        return;
    };

    let Some(known) = veil else {
        for c in colors.iter_mut() {
            c[3] = 1.0;
        }
        return;
    };

    let n = PATCH_CELLS;
    let stride = n + 1;
    let (u0, v0, side) = key.rect();
    let (outward, along_u, along_v) = face_axes(key.face);
    let step = side / n as f32;

    // Fast reject if the whole patch is far from the known world:
    let patch_center = key.center_dir();
    let patch_reach = key.cell_arc() * (n as f32) * 0.9;
    let home_dir = chart_direction(known.center.xz());
    let home_reach = (known.radius + 64.0) / PLANET_RADIUS;
    let angle_to_home = patch_center.dot(home_dir).clamp(-1.0, 1.0).acos();

    let touches_home = angle_to_home <= (patch_reach + home_reach);
    let touches_any_pocket = known.pockets.iter().any(|pocket| {
        let p_dir = chart_direction(pocket.at.xz());
        let p_reach = (pocket.radius + 64.0) / PLANET_RADIUS;
        let angle = patch_center.dot(p_dir).clamp(-1.0, 1.0).acos();
        angle <= (patch_reach + p_reach)
    });

    if !touches_home && !touches_any_pocket {
        for c in colors.iter_mut() {
            c[3] = 0.0;
        }
        return;
    }

    for gj in 0..stride {
        for gi in 0..stride {
            let u = u0 + gi as f32 * step;
            let v = v0 + gj as f32 * step;
            let dir = (outward + along_u * u + along_v * v).normalize();
            let (x, z) = ground_coordinates(dir);
            let idx = gj * stride + gi;
            if idx < colors.len() {
                colors[idx][3] = if known.knows(Vec3::new(x, 0.0, z)) {
                    1.0
                } else {
                    0.0
                };
            }
        }
    }

    let edges: [Vec<(usize, usize)>; 4] = [
        (0..stride).map(|i| (i, 0)).collect(),
        (0..stride).map(|i| (i, n)).collect(),
        (0..stride).map(|j| (0, j)).collect(),
        (0..stride).map(|j| (n, j)).collect(),
    ];
    let mut skirt_idx = stride * stride;
    for edge in edges {
        for (gi, gj) in edge {
            let top_index = gj * stride + gi;
            if skirt_idx < colors.len() && top_index < colors.len() {
                colors[skirt_idx][3] = colors[top_index][3];
            }
            skirt_idx += 1;
        }
    }
}

/// The `(x, z)` the terrain speaks, for a direction in the scaffold's frame.
/// The minus mirrors `direction_at`'s: the game's north is -z.
pub(crate) fn ground_coordinates(dir: Vec3) -> (f32, f32) {
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    let lon = dir.x.atan2(dir.z);
    (lon * PLANET_RADIUS, -lat * PLANET_RADIUS)
}

/// Radial distance the GROUND is drawn at. True relief everywhere now, beds
/// included: the planet sits underneath the flat world whenever the god is
/// high enough to see past the loaded ground, and an exaggerated mountain
/// would tower up through the real ground above it.
///
/// It used to lift every drowned vertex to the water's surface and paint it
/// blue, because there was nothing else to draw the sea with up here. That is
/// what made the ocean a picture of water rather than water: the flat sheet
/// over the loaded ground had the whole water shader - waves, sky reflection,
/// fresnel, foam - and the planet had a colored triangle, and where the two
/// met you could see the join. They can be given the same color and never the
/// same light.
///
/// So the bed is drawn where the bed is, and `build_patch_water` lays the
/// actual water over it. The water shader measures its own thickness from
/// whatever is drawn behind it, which is exactly what a real bed gives it -
/// shallows clear enough to show the bottom, depths opaque, foam along the
/// shore - and none of that was reachable while the bed WAS the surface.
fn drawn_radial(h: f32) -> f32 {
    PLANET_RADIUS + h - PATCH_SINK
}

/// The water standing over one patch, if any: the sea, and the lakes and
/// rivers the courses know, all of them the same question asked once.
///
/// Drawn at the water's own surface, with the sphere's own outward for a
/// normal - a level surface on a ball IS the sphere, so there is nothing to
/// derive. No skirt and no neighbor sampling either: the sheet is level, so
/// two patches meeting at different depths still meet exactly.
///
/// Sunk by `WATER_CLEARANCE` - just enough to keep out of the chunks' way,
/// and no more.
///
/// The DEPTH has to be honest. The shader reads thickness as the distance
/// between this surface and whatever is drawn behind it, so sinking the bed
/// without sinking the water would add the whole sink to every reading: a
/// shoreline would come out two and a half units deep, which is past the foam
/// band, and the sea would run up the beach with no edge on it. Sunk together,
/// the difference is `wet - h` exactly - the number the terrain actually knows.
///
/// It cannot sit at true sea level, because the chunks' own sea does and two
/// water surfaces in one plane is a z-fight. It cannot drop the whole
/// `PATCH_SINK` either, which is what it did at first: the chunks' sea ends at
/// the streamed radius and this carries on from there, so a two and a half
/// unit sink put a STEP in the ocean at that edge, seen end-on from any low
/// camera. A single unit clears the chunks and is nothing to look at from the
/// thousand-odd units away that boundary always is.
///
/// The cost is that the depth read here runs `PATCH_SINK - WATER_CLEARANCE`
/// too deep, so a shore drawn by a patch will not foam. Shores near enough to
/// look at are drawn by chunks, which measure honestly.
fn build_patch_water(
    terrain: &Terrain,
    veil: Option<&crate::villager::explore::KnownWorld>,
    key: PatchKey,
) -> Option<Mesh> {
    let n = PATCH_CELLS;
    let stride = n + 1;
    let (u0, v0, side) = key.rect();
    let (outward, along_u, along_v) = face_axes(key.face);
    let step = side / n as f32;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(stride * stride);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(stride * stride);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(stride * stride);
    let mut drowned = vec![false; stride * stride];
    let mut any = false;

    for gj in 0..stride {
        for gi in 0..stride {
            let u = u0 + gi as f32 * step;
            let v = v0 + gj as f32 * step;
            let dir = (outward + along_u * u + along_v * v).normalize();
            let (x, z) = ground_coordinates(dir);
            // The same question the ground asked, so the two agree vertex for
            // vertex. See `Terrain::ground_and_water_at`.
            let (h, wet) = terrain.ground_and_water_at(x, z);
            // WHETHER THERE IS SEA HERE IS A QUESTION ABOUT THE WORLD, NOT
            // ABOUT WHO HAS SEEN IT. Knowledge used to decide the water's
            // EXISTENCE - `h < wet && known_here` - which left the ocean only
            // two possible readings, full daylight or absent, and never the
            // one the land gets. So every sea on the planet lay lit while the
            // continents around it went dark, which is precisely backwards:
            // unwalked water is the least known ground there is.
            let under = h < wet;
            any |= under;
            drowned[gj * stride + gi] = under;
            positions.push((dir * (PLANET_RADIUS + wet - WATER_CLEARANCE)).to_array());
            normals.push(dir.to_array());
            // Depth against the bed, exactly as the chunks' own sea reads it,
            // so the two agree where they meet. See `terrain::water_color`.
            let mut color = crate::terrain::water_color(wet - h);
            // And the veil is worn the way the land wears it: alpha zero is
            // the mark `paint_patch_colors` makes on unknown ground, and the
            // planet's skin reads that one mark for both. One veil, one
            // color - see `paint_patch_colors`.
            if veil.is_some_and(|known| !known.knows(Vec3::new(x, 0.0, z))) {
                color[3] = 0.0;
            }
            colors.push(color);
        }
    }
    if !any {
        return None;
    }

    // Winding read off the geometry, the same way `build_patch` reads it, so
    // the sea faces outward on all six faces of the cube without any of them
    // having to know which face they are.
    let pa = Vec3::from(positions[0]);
    let pb = Vec3::from(positions[1]);
    let pc = Vec3::from(positions[stride]);
    let outward_as_abc = (pb - pa).cross(pc - pa).dot(pa) > 0.0;

    let mut indices: Vec<u32> = Vec::new();
    for row in 0..n {
        for column in 0..n {
            let corner = [
                row * stride + column,
                row * stride + column + 1,
                (row + 1) * stride + column,
                (row + 1) * stride + column + 1,
            ];
            // Any drowned corner is enough. The dry ones are already at the
            // water's level, so the sheet runs up onto the shore and the
            // shader's own depth fade thins it away there - rather than
            // stopping dead at the last wet vertex and leaving a staircase
            // along every coast on the planet.
            if !corner.iter().any(|i| drowned[*i]) {
                continue;
            }
            let [a, b, c, d] = corner.map(|i| i as u32);
            if outward_as_abc {
                indices.extend_from_slice(&[a, b, c, b, d, c]);
            } else {
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
    if indices.is_empty() {
        return None;
    }

    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

/// One vertex's color: any standing water by depth in the water's own
/// ramp — the sea, and the lakes and rivers the courses know — the land by
/// the chunk painter itself, darkened under deep woods the way the ground
/// disappears under canopy from the air. The aim is that the planet at any
/// height is recognizably the ground the game is played on.
fn paint(terrain: &Terrain, x: f32, z: f32, h: f32, wet: f32, slope: f32) -> [f32; 4] {
    if h < wet {
        let depth = ((wet - h) / 8.0).clamp(0.0, 1.0);
        // The same two shades the water itself is drawn in. See
        // `terrain::SEA_SHALLOW`.
        let shallow = palette::shade(&palette::WATER, crate::terrain::SEA_SHALLOW).to_linear();
        let deep = palette::shade(&palette::WATER, crate::terrain::SEA_DEEP).to_linear();
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
        crate::terrain::polarity_at(z),
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
    mut detail: ResMut<PlanetDetail>,
    mut meshes: ResMut<Assets<Mesh>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&GlobalTransform, &CameraRig), With<GodCamera>>,
    mut patches: Query<(&Patch, &mut Visibility)>,
    state: Res<State<crate::GameState>>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("globe: tend_the_tree");
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
    let cam_mesh = planet_stance().inverse() * (camera.translation() - planet_center());
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
        // Ground the planet itself hides is neither split nor shown. A whole
        // root face going this way takes its entire subtree with it, which is
        // the point. See `PatchKey::over_the_limb`.
        if key.over_the_limb(cam_mesh) {
            continue;
        }
        let center = key.center_dir() * (PLANET_RADIUS + WATER_LEVEL);
        let reach = key.cell_arc() * PATCH_CELLS as f32 * 0.75;
        let distance = (cam_mesh.distance(center) - reach).max(REFINE_FLOOR);
        let sharp_px = key.cell_arc() / distance * px_per_radian;
        // Once a patch HAS been split, the error must fall well under the
        // threshold before its children are given up again - otherwise an
        // altitude parked on the boundary splits and merges on alternate
        // frames, rebuilding that ground every time. With eight levels there
        // is a patch sitting on a threshold in almost every frame.
        //
        // Straight out of the Flat Earth Simulator, which took this quadtree
        // from here and then learned this the hard way on the way back.
        let already = key.level < MAX_LEVEL
            && [(0, 0), (1, 0), (0, 1), (1, 1)]
                .iter()
                .any(|(dx, dy)| tree.built.contains_key(&key.child(*dx, *dy)));
        let threshold = if already {
            SPLIT_PX * MERGE_HYSTERESIS
        } else {
            SPLIT_PX
        };
        if sharp_px > threshold && key.level < MAX_LEVEL {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                walk.push(key.child(dx, dy));
            }
        } else {
            wanted.push(key);
        }
    }

    // Notice the veil changing. A TOGGLE - the F key, leaving the title -
    // changes every patch at once, so it bumps the beat and the planet
    // repaints whole. The known world GROWING is a different animal: one
    // walker learning one meadow used to repaint the entire planet a few
    // patches a frame - seconds of full-price terrain sampling for a
    // thirty-pace circle - so growth now stales only the patches whose
    // ground the change actually touches.
    let shroud = veil_of(&veil.0, &veil.1, state.get());
    let fresh = shroud.map(VeilSeen::of);
    if fresh != tree.veil_seen {
        let discs = match (&tree.veil_seen, &fresh) {
            (Some(seen), Some(now)) => Some(seen.changed_discs(now)),
            // On or off is everywhere at once.
            _ => None,
        };
        match discs {
            Some(discs) => {
                let toward: Vec<(Vec3, f32)> = discs
                    .iter()
                    .map(|(at, radius)| (chart_direction(*at), (radius + 48.0) / PLANET_RADIUS))
                    .collect();
                let paint_beat = tree.paint_beat;
                for (key, patch) in tree.built.iter_mut() {
                    let reach = key.cell_arc() * PATCH_CELLS as f32 * 0.8;
                    let dir = key.center_dir();
                    let touched = toward.iter().any(|(disc, arc)| {
                        let limit = (reach + arc).min(std::f32::consts::PI);
                        dir.dot(*disc) > limit.cos()
                    });
                    if touched {
                        if let Some(mut mesh) = meshes.get_mut(&patch.mesh) {
                            repaint_patch_veil(&mut *mesh, shroud, *key);
                            patch.painted = paint_beat;
                        } else {
                            patch.painted = 0;
                        }
                    }
                }
            }
            None => {
                // When toggling or first starting the game / entering playing mode:
                // WARM UP IMMEDIATELY on all resident layers in ~0.3ms!
                tree.paint_beat += 1;
                let beat = tree.paint_beat;
                for (key, patch) in tree.built.iter_mut() {
                    if let Some(mut mesh) = meshes.get_mut(&patch.mesh) {
                        repaint_patch_veil(&mut *mesh, shroud, *key);
                        patch.painted = beat;
                    } else {
                        patch.painted = 0;
                    }
                }
            }
        }
        tree.veil_seen = fresh;
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
        let da = cam_mesh.distance(a.center_dir() * PLANET_RADIUS);
        let db = cam_mesh.distance(b.center_dir() * PLANET_RADIUS);
        da.total_cmp(&db)
    });
    // A big backlog earns a bigger budget.
    let budget = (BUILDS_PER_FRAME + missing.len() / 24).min(BUILDS_PER_HURRIED_FRAME);
    for key in missing.iter().take(budget) {
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
    let now = tree.beat;
    for key in &wanted {
        let mut candidate = *key;
        loop {
            // Standing means its entity is really there, not merely promised.
            // See `Patch2::grown_at`.
            if tree
                .built
                .get(&candidate)
                .is_some_and(|patch| patch.grown_at < now)
            {
                on_screen.insert(candidate);
                break;
            }
            match candidate.parent() {
                Some(up) => candidate = up,
                None => break,
            }
        }
    }
    // What all that resolved to, for the panel.
    detail.coarsest = on_screen.iter().map(|k| k.level).min().unwrap_or(0);
    detail.finest = on_screen.iter().map(|k| k.level).max().unwrap_or(0);
    detail.shown = on_screen.len();
    detail.built = tree.built.len();
    detail.owed = missing.len();

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
/// The sky painter writes the horizon color into the clear color every
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

    // The sky becomes a sky, and then becomes space. The horizon color it
    // starts from is a neutral gray from the era when terrain had to
    // dissolve into it; above a genuinely curving horizon that gray read as
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
    // Gray to blue across the first stretch of the climb...
    let bluing = ((height - ASCENT) / 4_000.0).clamp(0.0, 1.0);
    let skyed = mix(horizon, sky_blue, bluing);
    // ...then blue to black, and DONE by eleven thousand. It used to ramp to
    // twenty thousand and square the result to hold the blue longer, which meant
    // seventeen thousand units up — twice as high as any aircraft flies, with the
    // whole planet in frame and stars out — the sky was still plainly blue.
    // Smoothstepped rather than squared, so it eases at both ends instead of
    // clinging to one.
    let t = ((height - 3_500.0) / 7_500.0).clamp(0.0, 1.0);
    let thinning = t * t * (3.0 - 2.0 * t);
    clear.0 = Color::LinearRgba(mix(skyed, space, thinning));
}

#[cfg(test)]
mod tests {

    /// A learned pocket must stale the patch it lands on and leave the far
    /// side of the planet alone - the growth that used to repaint the world.
    #[test]
    fn growth_changes_only_the_ground_it_touched() {
        use super::VeilSeen;
        let before = VeilSeen {
            center: Vec2::new(1000.0, 200.0),
            radius: 170.0,
            pockets: vec![(Vec2::new(1400.0, 250.0), 30.0)],
        };
        let mut after = VeilSeen {
            pockets: vec![
                // The old pocket, reordered - tidy does that - plus a new one.
                (Vec2::new(1650.0, -90.0), 32.0),
                (Vec2::new(1400.0, 250.0), 30.0),
            ],
            ..VeilSeen {
                center: before.center,
                radius: before.radius,
                pockets: Vec::new(),
            }
        };
        let discs = before.changed_discs(&after);
        assert_eq!(discs, vec![(Vec2::new(1650.0, -90.0), 32.0)]);

        // The home circle growing names both paintings of it.
        after.radius = 240.0;
        let discs = before.changed_discs(&after);
        assert!(discs.contains(&(before.center, 170.0)));
        assert!(discs.contains(&(before.center, 240.0)));
    }

    /// The staleness test lives on the sphere: a chart position must bend
    /// to the same direction `ground_coordinates` unbends.
    #[test]
    fn the_chart_direction_inverts_the_ground() {
        for at in [
            Vec2::new(0.0, 0.0),
            Vec2::new(1070.0, 139.0),
            Vec2::new(-4200.0, 2600.0),
            Vec2::new(9000.0, -5000.0),
        ] {
            let dir = super::chart_direction(at);
            let (x, z) = super::ground_coordinates(dir);
            assert!(
                (x - at.x).abs() < 0.5 && (z - at.y).abs() < 0.5,
                "{at:?} came back as ({x}, {z})"
            );
        }
    }

    /// The fog shader undoes the bend to ask what ground a pixel stands on, and
    /// it has to come back to the ground the simulation is actually using.
    ///
    /// The formula lives twice - here and in `fog.wgsl` - because a shader
    /// cannot be called from a test. Written out the same way in both, so that
    /// this failing is a warning the other is wrong.
    #[test]
    fn the_bend_can_be_undone() {
        use crate::terrain::PLANET_RADIUS;
        for flat in [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(120.0, 3.0, -80.0),
            Vec3::new(-758.0, 0.0, -329.0),
            // The far side of the world, which is where the fault showed.
            Vec3::new(PLANET_RADIUS * 2.0, 0.0, 400.0),
            Vec3::new(-PLANET_RADIUS * 1.5, 12.0, PLANET_RADIUS * 0.4),
        ] {
            let (seat, _) = bend_frame(flat);
            // What the shader does, in the same order.
            let from_center = seat - planet_center();
            let unturned = Vec3::new(from_center.x, -from_center.z, from_center.y);
            let dir = unturned.normalize();
            let lat = dir.y.clamp(-1.0, 1.0).asin();
            let lon = dir.x.atan2(dir.z);
            let ground = Vec2::new(lon * PLANET_RADIUS, -lat * PLANET_RADIUS);

            // Longitude wraps, so compare the ANGLE rather than the arc: a
            // village at x = 2R and one at x = 2R - 2*pi*R stand on the same
            // ground and the fog must treat them as one place.
            let wrap = std::f32::consts::TAU * PLANET_RADIUS;
            let mut dx = ground.x - flat.x;
            dx -= (dx / wrap).round() * wrap;
            assert!(
                dx.abs() < 0.5,
                "a pixel at {flat} came back {} out along x",
                dx.abs()
            );
            assert!(
                (ground.y - flat.z).abs() < 0.5,
                "a pixel at {flat} came back {} out along z",
                (ground.y - flat.z).abs()
            );
        }
    }
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

    /// Anything built already-bent keeps the transform it was given.
    ///
    /// A river is a child of its chunk. The chunk is excluded from the bend
    /// because its vertices are seated one by one; the river's are too, and it
    /// said so in a comment instead of in the world — so the bend seated its
    /// transform as well, and an identity transform seats the origin at
    /// twenty-eight units BELOW the ground. Every river in the world sank out of
    /// sight and left its carved channel painted on a dry valley floor.
    #[test]
    fn the_bend_leaves_already_bent_geometry_alone() {
        let mut app = App::new();
        app.add_systems(Update, bend_the_world);

        let placed = GlobalTransform::IDENTITY;
        let river = app.world_mut().spawn((BentInPlace, placed)).id();
        app.update();

        let after = *app.world().get::<GlobalTransform>(river).unwrap();
        assert_eq!(after, placed, "the river was seated and buried");
        // And the burial was real: this is where an identity transform lands.
        let sunk = bend_frame(Vec3::ZERO).0;
        assert!(
            sunk.y < -20.0,
            "the bend used to put the origin at {sunk}, which was the whole bug"
        );
    }

    /// A house on the far side of the world is still a house.
    ///
    /// The bend seats every entity by its own flat position, and near the
    /// poles the chart's longitudes converge - each part of one roof got a
    /// visibly different frame, and a far village rose as cubism: "my house
    /// looks like a picasso painting." Rigid assemblies are reseated in
    /// their ROOT'S frame after the bend; this holds the whole claim.
    #[test]
    fn a_rigid_house_stays_square_at_the_far_latitudes() {
        let mut app = App::new();
        app.add_systems(Update, (bend_the_world, seat_the_rigid).chain());

        // High latitude: two thirds of the way to the pole.
        let site = Vec3::new(700.0, 40.0, -6_300.0);
        let ridge = Transform::from_xyz(-3.0, 4.0, 0.0);
        let slab = Transform::from_xyz(3.0, 0.2, 0.0);
        let root = app
            .world_mut()
            .spawn((
                RigidlySeated,
                Transform::from_translation(site),
                GlobalTransform::from(Transform::from_translation(site)),
            ))
            .id();
        let parts = [ridge, slab].map(|local| {
            let flat_world = Transform::from_translation(site + local.translation);
            let part = app
                .world_mut()
                .spawn((local, GlobalTransform::from(flat_world)))
                .id();
            app.world_mut().entity_mut(part).insert(ChildOf(root));
            part
        });

        app.update();

        let world = app.world();
        let seat = *world.get::<GlobalTransform>(root).unwrap();
        for (part, local) in parts.iter().zip([ridge, slab]) {
            let got = *world.get::<GlobalTransform>(*part).unwrap();
            let want = seat.mul_transform(local);
            assert!(
                got.translation().distance(want.translation()) < 1e-3,
                "a part sits {} from its place in the root's frame",
                got.translation().distance(want.translation()),
            );
        }
        // And the claim is not vacuous: seated individually, the ridge lands
        // measurably elsewhere at this latitude.
        let individually = bend_frame(site + ridge.translation).0;
        let rigidly = seat.mul_transform(ridge).translation();
        assert!(
            individually.distance(rigidly) > 0.05,
            "at this latitude the two seatings agree ({}) - the fixture \
             proves nothing",
            individually.distance(rigidly),
        );
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
    /// version was tried first on the strength of an eight-centimeter
    /// estimate that turned out to be for a bigger planet and a nearer point.
    /// At this radius a chunk's corner bows a fifth of a unit from its
    /// center's frame and two thirds from its ORIGIN's, so neighbors seated
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
    /// world the far edge drops well below the tangent plane, which is the
    /// difference between a horizon and a plate.
    ///
    /// Measured against the world's own relief rather than in bare units,
    /// because the plate's radius is a tuning knob and this claim is not. It
    /// was a couple of hundred units when the plate reached twenty rings; at
    /// twelve it is nearer sixty, and sixty against three hundred and twenty
    /// units of relief is still a horizon and not a table top.
    #[test]
    fn the_streamed_world_visibly_curves() {
        let rim = crate::terrain::CHUNK_SIZE * crate::terrain::VIEW_CHUNKS as f32;
        let far = bend_frame(Vec3::new(rim, WATER_LEVEL, 0.0)).0;
        let drop = -far.y;
        let relief = crate::terrain::TERRAIN_HEIGHT;
        assert!(
            drop > relief * 0.1,
            "the streamed rim drops {drop} units against {relief} of relief - \
             the world is flat"
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

    /// A patch mesh holds together: grid plus skirt, a color for every
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

    /// An eye that height above the ground under `face`'s center.
    fn eye_over(face: u8, height: f32) -> Vec3 {
        let root = PatchKey {
            face,
            level: 0,
            x: 0,
            y: 0,
        };
        root.center_dir() * (PLANET_RADIUS + height)
    }

    #[test]
    fn the_far_side_of_the_world_is_not_tended() {
        // The tree had no cull at all, so from four hundred units up it was
        // refining, drawing and keeping in memory a whole planet of ground the
        // planet itself was standing in front of.
        let eye = eye_over(0, 431.0);
        let behind = PatchKey {
            face: 1,
            level: 0,
            x: 0,
            y: 0,
        };
        assert!(
            behind.over_the_limb(eye),
            "the opposite face of the cube is being tended from 431 units up"
        );
        // Whatever it is made of. A deep patch out there is no more visible
        // than the root it hangs from.
        let deep = PatchKey {
            face: 1,
            level: 5,
            x: 16,
            y: 16,
        };
        assert!(deep.over_the_limb(eye), "and neither is anything inside it");
    }

    #[test]
    fn the_ground_underfoot_is_always_tended() {
        // The cull must never touch what the god is looking at, at any height
        // from the treetops to the top of the climb.
        for height in [50.0, 431.0, 1_500.0, crate::globe::CEILING] {
            let eye = eye_over(2, height);
            for level in 0..=MAX_LEVEL {
                let middle = 1u32 << level.saturating_sub(1);
                let under = PatchKey {
                    face: 2,
                    level,
                    x: middle,
                    y: middle,
                };
                assert!(
                    !under.over_the_limb(eye),
                    "the ground under the god was culled at L{level}, {height} up"
                );
            }
        }
    }

    #[test]
    fn nothing_with_a_corner_in_view_is_ever_culled() {
        // What the half span is FOR. The cull reads a patch's center, and a
        // patch whose middle is over the horizon can still have a near corner
        // in plain sight; culling by the middle alone walks a visible bite out
        // of the edge of the world.
        //
        // Checked against each patch's own four corners, worked out from its
        // rect rather than from the half span the cull uses - so this is a
        // second opinion and not a restatement. The middle column of a face is
        // swept deliberately: that is where the cube projection stretches most
        // and where an average-based span under-reaches worst.
        for height in [120.0, 431.0, 2_000.0, CEILING] {
            let eye = eye_over(0, height);
            let nadir = eye.normalize();
            let horizon = ((PLANET_RADIUS - crate::terrain::TERRAIN_HEIGHT) / eye.length()).acos();
            for level in [3u8, 5, 6, 8] {
                let span = 1u32 << level;
                for y in 0..span {
                    let key = PatchKey {
                        face: 0,
                        level,
                        x: span / 2,
                        y,
                    };
                    if !key.over_the_limb(eye) {
                        continue;
                    }
                    let (u0, v0, side) = key.rect();
                    let (outward, along_u, along_v) = face_axes(key.face);
                    let nearest = [(0.0, 0.0), (side, 0.0), (0.0, side), (side, side)]
                        .into_iter()
                        .map(|(du, dv)| {
                            let dir =
                                (outward + along_u * (u0 + du) + along_v * (v0 + dv)).normalize();
                            dir.cross(nadir).length().atan2(dir.dot(nadir))
                        })
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        nearest > horizon,
                        "an L{level} patch was culled from {height} up with a \
                         corner {nearest} from the nadir, inside the horizon at \
                         {horizon}"
                    );
                }
            }
        }
    }
}
