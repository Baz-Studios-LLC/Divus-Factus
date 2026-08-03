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
//! The flat chunk world still exists below [`ASCENT`]: it is where villagers,
//! trees and buildings live, and it draws ON TOP of the planet as the final
//! layer of detail. From the moment the god rises past [`ASCENT`] the planet
//! is always underneath — every ray that misses loaded ground lands on more
//! world, never on void — and past [`CURTAIN`] the chunks bow out entirely,
//! by which point the patches beneath them have refined to the same order of
//! detail and their retirement changes almost nothing on screen.
//!
//! Orbit controls: grab the world with the left mouse and drag to turn it;
//! the wheel rides from a doorstep to the whole planet and back; Escape is
//! the other way home. Leaving orbit lands the camera on the ground that was
//! under view.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::FRAC_PI_2;

use crate::camera::{CameraRig, GodCamera};
use crate::palette;
use crate::terrain::{PLANET_RADIUS, Terrain, WATER_LEVEL};

/// The globe's own render layer. The planet and its sun live here alone, so
/// what the camera renders is staged with single component writes.
pub const GLOBE_LAYER: usize = 2;

/// Past this camera distance the flat chunk world bows out and the patch
/// tree carries the frame alone.
pub const CURTAIN: f32 = 5_200.0;

/// Coming back in through this, the chunks return. Inside the curtain so the
/// two thresholds cannot chatter when the wheel hesitates between them.
const RESURFACE: f32 = 4_600.0;

/// Where the climb starts telling: the sky begins turning from horizon to
/// blue here, and the diorama lens retires. The planet itself is beneath the
/// world at EVERY height now — there is no transition to gate — so this is a
/// dial for the atmosphere, not a curtain for the scenery.
pub const ASCENT: f32 = 1_500.0;

/// How far out the wheel will take the god. From here the whole planet sits
/// in the frame with room to spare, like a held apple.
pub const CEILING: f32 = 42_000.0;

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
const SPLIT_PX: f32 = 9.0;

/// Patches built per frame while the tree is chasing the camera. A build is
/// a couple of milliseconds of noise sampling; four a frame chases a fast
/// descent closely without stuttering the simulation, and a patch that
/// arrives a beat late only means the ground is briefly coarser there —
/// the same bargain streaming ground has always made.
const BUILDS_PER_FRAME: usize = 4;

/// The camera never counts as closer to the surface than this when deciding
/// how deep to refine — below it the flat world owns the detail, and a tree
/// refining to centimetres under a world that hides it is pure waste.
const REFINE_FLOOR: f32 = 900.0;

/// The rotation that stands the planet up in the world: the terrain scaffold
/// maps ground onto the unit sphere with the reference point at +Z and the
/// poles at ±Y, and this turns that frame so the reference point is UP,
/// with the game's own north (-z on the ground) kept as north.
fn planet_stance() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2)
}

/// Where the planet's centre sits: the sea-level sphere tangent to the flat
/// world at the origin, sunk a few units so the two surfaces — the same
/// field, so they agree exactly at the tangent point — never fight over the
/// same pixels while both are drawn.
fn planet_centre() -> Vec3 {
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
    built: HashMap<PatchKey, Entity>,
    /// The seed the tree was grown for, so a new world fells the old tree.
    grown_for: Option<u32>,
}

/// The one material every patch wears; the colours ride the vertices.
#[derive(Resource)]
struct PlanetSkin(Handle<StandardMaterial>);

/// The state of the orbital view.
#[derive(Resource)]
pub struct GlobeView {
    /// Whether the planet owns the frame outright (past the curtain).
    pub shown: bool,
    /// Unit direction from the planet's centre to the ground under view, in
    /// world space. Dragging rotates this; leaving orbit turns it back into
    /// the `(x, z)` the flat world speaks.
    look: Vec3,
    /// Present height above the sea-level sphere, smoothed toward `sought`.
    gaze: f32,
    /// Where the wheel has asked the height to go.
    sought: f32,
}

impl GlobeView {
    /// The height of the climb, whichever camera owns it: the orbit's own
    /// height when the planet has the frame, the rig's distance otherwise.
    /// One number for the overlay and the sky, continuous through the
    /// handover.
    pub fn height(&self, rig_distance: f32) -> f32 {
        if self.shown { self.gaze } else { rig_distance }
    }
}

impl Default for GlobeView {
    fn default() -> Self {
        GlobeView {
            shown: false,
            look: Vec3::Y,
            gaze: CURTAIN,
            sought: CURTAIN,
        }
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
        app.init_resource::<GlobeView>()
            .init_resource::<PlanetTree>()
            .add_systems(
                Update,
                (plant_the_tree, dress_the_patches).run_if(resource_exists::<Terrain>),
            )
            .add_systems(
                Update,
                (behold_the_world, stage_the_ascent, tend_the_tree)
                    .chain()
                    .after(crate::camera::CameraSet)
                    .run_if(crate::world_is_afoot),
            )
            .add_systems(Update, dress_for_space.after(crate::render::paint_the_sky));
    }
}

// -------------------------------------------------------------- the planting

/// Raises the planet's root and sun for a new world, and pre-grows the tree
/// two levels deep — the orbital view's own resolution — in the loading
/// screen's shadow, so the first climb finds a planet already standing.
fn plant_the_tree(
    mut commands: Commands,
    terrain: Res<Terrain>,
    mut tree: ResMut<PlanetTree>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
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
            Visibility::Hidden,
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

    commands.insert_resource(PlanetSkin(materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        ..default()
    })));

    let mut grown = 0;
    for face in 0..6u8 {
        for level in 0..=2u8 {
            let across = 1u32 << level;
            for y in 0..across {
                for x in 0..across {
                    let key = PatchKey { face, level, x, y };
                    grow_patch(&mut commands, &terrain, &mut meshes, &mut tree, root, key);
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
    meshes: &mut Assets<Mesh>,
    tree: &mut PlanetTree,
    root: Entity,
    key: PatchKey,
) -> Entity {
    let mesh = build_patch(terrain, key);
    let entity = commands
        .spawn((
            Patch(key),
            Mesh3d(meshes.add(mesh)),
            Transform::default(),
            Visibility::Hidden,
            RenderLayers::layer(GLOBE_LAYER),
            NotShadowCaster,
            NotShadowReceiver,
            ChildOf(root),
        ))
        .id();
    tree.built.insert(key, entity);
    entity
}

/// Hands the shared material to any patch that lacks it. Separate from the
/// spawning so patch growth never has to thread the material through.
fn dress_the_patches(
    mut commands: Commands,
    skin: Option<Res<PlanetSkin>>,
    bare: Query<Entity, (With<Patch>, Without<MeshMaterial3d<StandardMaterial>>)>,
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

// -------------------------------------------------------------- the building

/// Samples one patch of the world: heights and paint from the same fields
/// the chunks are built from, and a skirt around the edge to curtain the
/// hairline cracks where neighbouring patches meet at different depths.
fn build_patch(terrain: &Terrain, key: PatchKey) -> Mesh {
    let n = PATCH_CELLS;
    let stride = n + 1;
    let padded = n + 3;
    let (u0, v0, side) = key.rect();
    let (outward, along_u, along_v) = face_axes(key.face);
    let step = side / n as f32;

    // Padded grids of drawn positions and ground answers, one cell beyond
    // each edge for slopes and normals.
    let mut grid: Vec<Vec3> = Vec::with_capacity(padded * padded);
    let mut ground: Vec<(f32, f32, f32)> = Vec::with_capacity(padded * padded);
    for gj in 0..padded {
        for gi in 0..padded {
            let u = u0 + (gi as f32 - 1.0) * step;
            let v = v0 + (gj as f32 - 1.0) * step;
            let dir = (outward + along_u * u + along_v * v).normalize();
            let (x, z) = ground_coordinates(dir);
            let h = terrain.base_height_at(x, z);
            grid.push(dir * drawn_radial(h));
            ground.push((x, z, h));
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
            let (x, z, h) = ground[pj * padded + pi];

            let across = at(pi + 1, pj) - at(pi - 1, pj);
            let down = at(pi, pj + 1) - at(pi, pj - 1);
            let mut normal = across.cross(down).normalize_or(dir);
            if normal.dot(dir) < 0.0 {
                normal = -normal;
            }
            let slope = (1.0 - normal.dot(dir)).clamp(0.0, 1.0);

            positions.push(here.to_array());
            normals.push(normal.to_array());
            colors.push(paint(terrain, x, z, h, slope));
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
fn ground_coordinates(dir: Vec3) -> (f32, f32) {
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    let lon = dir.x.atan2(dir.z);
    (lon * PLANET_RADIUS, -lat * PLANET_RADIUS)
}

/// Radial distance the surface is drawn at. The sea is a smooth ball at
/// water level — a globe shows its oceans, not its seabeds — and the land is
/// TRUE relief: the planet sits underneath the flat world whenever the god
/// is high enough to see past the loaded ground, and an exaggerated mountain
/// would tower up through the real ground above it.
fn drawn_radial(h: f32) -> f32 {
    PLANET_RADIUS + WATER_LEVEL + (h - WATER_LEVEL).max(0.0)
}

/// One vertex's colour: the sea by depth in the water's own ramp, the land
/// by the chunk painter itself, so the planet at any height is recognisably
/// the ground the game is played on.
fn paint(terrain: &Terrain, x: f32, z: f32, h: f32, slope: f32) -> [f32; 4] {
    if h <= WATER_LEVEL {
        let depth = ((WATER_LEVEL - h) / 8.0).clamp(0.0, 1.0);
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

    // Grow the missing, nearest first, a few per frame.
    let mut missing: Vec<PatchKey> = wanted
        .iter()
        .copied()
        .filter(|key| !tree.built.contains_key(key))
        .collect();
    missing.sort_by(|a, b| {
        let da = cam_mesh.distance(a.centre_dir() * PLANET_RADIUS);
        let db = cam_mesh.distance(b.centre_dir() * PLANET_RADIUS);
        da.total_cmp(&db)
    });
    for key in missing.iter().take(BUILDS_PER_FRAME) {
        grow_patch(&mut commands, &terrain, &mut meshes, &mut tree, root, *key);
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
}

// -------------------------------------------------------------- the staging

/// One owner for what the camera renders and whether the planet is drawn.
/// Below [`ASCENT`], ordinary play: the world's layers, no planet. Between
/// [`ASCENT`] and orbit, BOTH — the chunks in front, the planet beneath, so
/// the ground past the streaming edge is more world rather than void. In
/// orbit, the planet alone.
fn stage_the_ascent(
    mut commands: Commands,
    view: Res<GlobeView>,
    tree: Res<PlanetTree>,
    mut roots: Query<&mut Visibility, With<ThePlanet>>,
    cameras: Query<(Entity, &CameraRig), With<GodCamera>>,
) {
    let Ok((camera, _rig)) = cameras.single() else {
        return;
    };
    // The planet is simply always there. It was staged in and out by height
    // for a while, and every threshold made a seam of one kind or another;
    // the world behind the world is not an effect to be cued, it is the
    // ground. The only staging left is orbit, where the flat world's layer
    // bows out and the planet carries the frame alone.
    if let Some(root) = tree.root
        && let Ok(mut showing) = roots.get_mut(root)
        && *showing != Visibility::Inherited
    {
        *showing = Visibility::Inherited;
    }
    if view.shown {
        commands
            .entity(camera)
            .insert(RenderLayers::layer(GLOBE_LAYER));
    } else {
        commands
            .entity(camera)
            .insert(RenderLayers::from_layers(&[0, GLOBE_LAYER]));
    }
}

// --------------------------------------------------------------- the orbit

/// Opens the orbital view when the zoom leaves the world, drives it while it
/// is open — grab and drag to turn the planet, wheel to climb or come home —
/// and lands the camera back on the ground that was under view.
#[allow(clippy::too_many_arguments)]
fn behold_the_world(
    time: Res<Time<Real>>,
    dive: Option<Res<crate::camera::CameraDive>>,
    terrain: Option<Res<Terrain>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mouse_scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    pointer: Res<crate::ui::PointerContext>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut view: ResMut<GlobeView>,
    mut cameras: Query<(&mut CameraRig, &mut Transform), With<GodCamera>>,
) {
    let Ok((mut rig, mut transform)) = cameras.single_mut() else {
        return;
    };

    if !view.shown {
        // The opening descent owns the rig; the handover waits for it.
        if rig.distance > CURTAIN && dive.is_none() {
            view.shown = true;
            view.look = planet_stance() * crate::terrain::direction_at(rig.focus.x, rig.focus.z);
            view.gaze = rig.distance;
            view.sought = rig.distance;
        } else {
            return;
        }
    }

    // The wheel, shaped like the ground's own zoom.
    let scroll = if pointer.over_ui {
        0.0
    } else {
        crate::camera::normalised_scroll(mouse_scroll.delta.y, mouse_scroll.unit)
    };
    if scroll != 0.0 {
        let factor = (1.0 - scroll * rig.zoom_sensitivity).clamp(0.5, 2.0);
        view.sought = (view.sought * factor).clamp(RESURFACE * 0.9, CEILING);
    }
    // Escape is the other way home.
    if keys.just_pressed(KeyCode::Escape) {
        view.sought = RESURFACE * 0.9;
    }
    let ease = 1.0 - (-6.0 * time.delta_secs()).exp();
    view.gaze += (view.sought - view.gaze) * ease;

    // The grab: dragging turns the planet under the hand, scaled so a pixel
    // of mouse is about a pixel of ground.
    let pole = planet_stance() * Vec3::Y;
    let up_hint = (pole - view.look * pole.dot(view.look)).normalize_or(Vec3::X);
    if buttons.pressed(MouseButton::Left) && !pointer.over_ui {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            let height_px = windows
                .single()
                .map(|w| w.resolution.height())
                .unwrap_or(1000.0)
                .max(1.0);
            let per_pixel = 2.0 * (0.31f32).tan() * view.gaze / height_px / PLANET_RADIUS;
            let forward = -view.look;
            let right = forward.cross(up_hint).normalize_or(Vec3::X);
            let turned = Quat::from_axis_angle(up_hint, -delta.x * per_pixel)
                * Quat::from_axis_angle(right, -delta.y * per_pixel)
                * view.look;
            // Short of the poles: a camera exactly over a pole has no north
            // to hang its frame on.
            let leaning = planet_stance().inverse() * turned;
            if leaning.y.abs() < 0.985 {
                view.look = turned.normalize();
            }
        }
    }

    // Coming home: the direction under view, in the ground's own words.
    if view.gaze < RESURFACE {
        let ground_dir = planet_stance().inverse() * view.look;
        let (x, z) = ground_coordinates(ground_dir);
        let y = terrain.as_ref().map_or(WATER_LEVEL, |t| t.height_at(x, z));
        rig.focus = Vec3::new(x, y, z);
        rig.target_focus = rig.focus;
        rig.distance = view.gaze;
        rig.target_distance = view.gaze * 0.96;
        // Straight down and north-up — exactly how the planet was being
        // looked at, so the handover changes nothing about the gaze.
        rig.pitch = crate::camera::MAX_PITCH;
        rig.target_pitch = rig.pitch;
        rig.yaw = 0.0;
        rig.target_yaw = 0.0;
        rig.zoom_anchor = None;
        view.shown = false;
        *transform = Transform::from_translation(rig.eye()).looking_at(rig.focus, Vec3::Y);
        return;
    }

    // The orbital camera: straight down at the point under view, north up.
    let sea = PLANET_RADIUS + WATER_LEVEL;
    let centre = planet_centre();
    let eye = centre + view.look * (sea + view.gaze);
    *transform = Transform::from_translation(eye).looking_at(centre + view.look * sea, up_hint);
}

// ----------------------------------------------------------------- the sky

/// Space is dark, and the climb to it starts before the chunks bow out.
///
/// The sky painter writes the horizon colour into the clear colour every
/// frame; this runs after it and takes the sky through what a climb should
/// look like — blue first, black only with real altitude.
fn dress_for_space(
    view: Option<Res<GlobeView>>,
    mut clear: ResMut<ClearColor>,
    cameras: Query<&CameraRig, With<GodCamera>>,
) {
    let Some(view) = view else {
        return;
    };
    let Ok(rig) = cameras.single() else {
        return;
    };

    // One number for the height of the climb, continuous through the
    // handover: the rig's distance on the way up, the orbit's beyond it.
    let height = if view.shown { view.gaze } else { rig.distance };

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
            let c = paint(&t, x, z, h, 0.05);
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
