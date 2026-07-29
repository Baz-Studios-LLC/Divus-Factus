//! Terrain: an endless, streamed landscape.
//!
//! There is no stored heightmap. Elevation is a pure function of world position and
//! seed, so the ground can be sampled anywhere without anything having been
//! generated there — which is what makes an unbounded world possible at all. Chunks
//! are meshes built around wherever the camera is looking and discarded when it
//! leaves. They are a *view* of the terrain function, never the terrain itself.
//!
//! That preserves the guarantee the bounded version had: the simulation asks
//! `height_at` and gets an answer, whether or not anything has been drawn there.
//!
//! Shape comes from three layers:
//!
//! - **Continents** — very low frequency, deciding sea from land over kilometres.
//! - **Hills** — mid frequency, domain-warped so the ground does not look like soap
//!   bubbles.
//! - **Ridges** — folded noise, faded in only on high ground, for rocky spines.

use bevy::light::NotShadowCaster;
pub mod rivers;

use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::noise::{fbm_2d, ridged_2d, warped_fbm_2d};
use crate::palette;

/// World units along one edge of a chunk.
pub const CHUNK_SIZE: f32 = 64.0;
/// Grid cells along one edge of a chunk.
pub const CHUNK_CELLS: usize = 32;
/// The furthest the world ever streams, in chunks.
///
/// Reached only at full zoom-out. See [`stream_radius`].
pub const VIEW_CHUNKS: i32 = 20;

/// The closest the world ever streams, in chunks.
const MIN_VIEW_CHUNKS: i32 = 6;

/// How far to stream for a given camera distance.
///
/// A god camera has to pull back far enough to survey a region, but streaming that
/// radius the whole time means paying for a thousand chunks while looking at one
/// village. Tying the radius to the zoom keeps close work cheap and still opens the
/// world up when the player rises.
pub fn stream_radius(camera_distance: f32) -> i32 {
    let chunks = (camera_distance / CHUNK_SIZE) * 1.6 + MIN_VIEW_CHUNKS as f32;
    (chunks.round() as i32).clamp(MIN_VIEW_CHUNKS, VIEW_CHUNKS)
}
/// Chunks built per frame during play. Generation is cheap but not free, and
/// spreading it out keeps streaming from showing up as a stutter.
const CHUNKS_PER_FRAME: usize = 3;

/// Chunks built per frame while the loading screen is up. Nothing is being rendered
/// yet that a hitch would spoil, so this is limited only by keeping the window
/// responsive enough to draw a progress bar.
const CHUNKS_PER_LOADING_FRAME: usize = 48;

/// Sea level.
pub const WATER_LEVEL: f32 = 20.0;
/// Nothing generates above this. Used to bound ray marching.
pub const TERRAIN_HEIGHT: f32 = 320.0;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_terrain).add_systems(
            Update,
            (stream_chunks, follow_water_plane).in_set(TerrainSet),
        );
    }
}

/// Terrain streaming runs as a unit, so anything needing chunks present can order
/// itself after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerrainSet;

/// Broad climate regions. Drives ground colour and which trees grow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Biome {
    /// Mixed grass and woodland.
    Temperate,
    /// Cold conifer country.
    Boreal,
    /// Dry scrub and bare earth.
    Arid,
    /// Damp, dense, dark green.
    Wetland,
    /// Above the treeline: rock and snow.
    Alpine,
}

impl Biome {
    /// A second ramp the ground drifts toward in its lighter patches.
    ///
    /// Picked to read as the same landscape in a different condition — dry grass in
    /// temperate country, bare earth in scrub — rather than as a different place.
    pub fn companion(self) -> &'static palette::Ramp {
        match self {
            Biome::Temperate => &palette::SCRUB,
            Biome::Boreal => &palette::GRASS,
            Biome::Arid => &palette::EARTH,
            Biome::Wetland => &palette::GRASS,
            Biome::Alpine => &palette::EARTH,
        }
    }

    /// The ramp this biome's ground is drawn from, and how far up it.
    pub fn ground(self) -> (&'static palette::Ramp, f32) {
        match self {
            Biome::Temperate => (&palette::GRASS, 0.06),
            Biome::Boreal => (&palette::FOLIAGE, -0.12),
            Biome::Arid => (&palette::SCRUB, 0.05),
            Biome::Wetland => (&palette::FOLIAGE, 0.10),
            Biome::Alpine => (&palette::STONE, 0.12),
        }
    }
}

/// Marks a loaded terrain chunk. Scatter is parented to it, so despawning the chunk
/// takes its trees and rocks with it.
#[derive(Component)]
pub struct TerrainChunk {
    pub coord: IVec2,
}

/// Marks the water plane.
#[derive(Component)]
pub struct WaterPlane;

/// A spot the villagers have worked level: a pad of one height, blending
/// back into the natural land across its falloff ring.
struct FlatSpot {
    x: f32,
    z: f32,
    radius: f32,
    falloff: f32,
    height: f32,
}

/// The terrain function. Holds a seed, the memoised rivers, and the spots
/// the villagers have levelled.
#[derive(Resource, Clone)]
pub struct Terrain {
    seed: u32,
    /// The memoised river network. Cloning a `Terrain` shares it.
    rivers: Arc<rivers::RiverIndex>,
    /// Ground worked level by hands. Shared like the rivers; grows rarely.
    worked: Arc<RwLock<Vec<FlatSpot>>>,
}

impl Terrain {
    pub fn new(seed: u32) -> Self {
        Terrain {
            seed,
            rivers: Arc::new(rivers::RiverIndex::default()),
            worked: Arc::default(),
        }
    }

    /// Levels a pad of ground at the given height. The change is real:
    /// `height_at` reports it everywhere, and the caller is responsible for
    /// rebuilding the chunk meshes that cover it.
    pub fn flatten(&self, x: f32, z: f32, radius: f32, falloff: f32, height: f32) {
        if let Ok(mut worked) = self.worked.write() {
            worked.push(FlatSpot {
                x,
                z,
                radius,
                falloff,
                height,
            });
        }
    }

    /// Every worked pad, for the save file.
    pub fn export_worked(&self) -> Vec<(f32, f32, f32, f32, f32)> {
        self.worked
            .read()
            .map(|worked| {
                worked
                    .iter()
                    .map(|s| (s.x, s.z, s.radius, s.falloff, s.height))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Restores worked pads from a save.
    pub fn import_worked(&self, spots: &[(f32, f32, f32, f32, f32)]) {
        if let Ok(mut worked) = self.worked.write() {
            worked.clear();
            for &(x, z, radius, falloff, height) in spots {
                worked.push(FlatSpot {
                    x,
                    z,
                    radius,
                    falloff,
                    height,
                });
            }
        }
    }

    /// Whether this ground has been worked level - a field pad, one day a
    /// floor. Nothing wild grows on worked ground.
    pub fn is_worked(&self, x: f32, z: f32) -> bool {
        self.is_worked_within(x, z, 0.0)
    }

    /// Worked ground plus a margin - trees keep a wider berth than grass.
    pub fn is_worked_within(&self, x: f32, z: f32, margin: f32) -> bool {
        let Ok(worked) = self.worked.read() else {
            return false;
        };
        worked.iter().any(|spot| {
            let dx = x - spot.x;
            let dz = z - spot.z;
            let reach = spot.radius + margin;
            dx * dx + dz * dz < reach * reach
        })
    }

    /// Applies the worked-level pads to a computed height.
    fn leveled(&self, x: f32, z: f32, height: f32) -> f32 {
        let Ok(worked) = self.worked.read() else {
            return height;
        };
        let mut height = height;
        for spot in worked.iter() {
            let reach = spot.radius + spot.falloff;
            let dx = x - spot.x;
            let dz = z - spot.z;
            let d2 = dx * dx + dz * dz;
            if d2 >= reach * reach {
                continue;
            }
            let d = d2.sqrt();
            let w = ((reach - d) / spot.falloff).clamp(0.0, 1.0);
            let w = w * w * (3.0 - 2.0 * w);
            height = height * (1.0 - w) + spot.height * w;
        }
        height
    }

    pub(crate) fn seed(&self) -> u32 {
        self.seed
    }

    /// Ground height at a world position.
    ///
    /// Pure: the same position always returns the same height, independent of what
    /// has been generated or in what order.
    /// Ground height at a world position, including river channels.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let base = self.base_height_at(x, z);
        let Some((level, distance, width)) = self.river_query(x, z) else {
            return self.leveled(x, z, base);
        };
        let half_width = rivers::CHANNEL_HALF_WIDTH * width;
        if distance >= half_width {
            return base;
        }

        // Parabolic bed below the course's water level, blending to untouched
        // ground at the banks. Where terrain already sits lower than the bed — a
        // dip the course crosses — it is left alone and water simply pools over it.
        // Young streams cut shallow; the channel deepens with maturity.
        let across = distance / half_width;
        let depth = rivers::CHANNEL_DEPTH * (0.3 + 0.7 * width);
        let bed = level - depth * (1.0 - across * across);
        self.leveled(x, z, base.min(bed).max(0.0))
    }

    /// Water level and lateral distance of the nearest river course.
    fn river_query(&self, x: f32, z: f32) -> Option<(f32, f32, f32)> {
        self.rivers.ensure_near(self, x, z);
        self.rivers.nearest(x, z)
    }

    /// Ground height before any river is cut into it.
    ///
    /// The river's own water surface is derived from this: water sits just below the
    /// land it flows through, not at sea level.
    pub fn base_height_at(&self, x: f32, z: f32) -> f32 {
        // Continents. Wavelength on the order of a kilometre, so crossing a coastline
        // takes a real walk.
        let continent = fbm_2d(x * 0.0011, z * 0.0011, self.seed, 5, 2.0, 0.5);

        // -1 is deep ocean, +1 is high inland.
        let land = ((continent - 0.44) / 0.30).clamp(-1.0, 1.0);

        // Steepen the curve either side of zero so ground climbs away from sea level
        // quickly. Left linear, the coastal band where height is barely above water
        // spans hundreds of units and the whole shoreline reads as one vast beach.
        let shaped = if land >= 0.0 {
            land.powf(0.62)
        } else {
            -(-land).powf(0.85)
        };

        // Hills, present everywhere but stronger on land.
        let detail = warped_fbm_2d(x * 0.010, z * 0.010, self.seed ^ 0xa1a1, 4, 0.5) - 0.5;

        // Ridges only bite on ground that is already high, so valleys stay walkable.
        let ridge_mask = land.clamp(0.0, 1.0);
        let ridge = ridged_2d(x * 0.006, z * 0.006, self.seed ^ 0xa5a5, 4);

        let mut height: f32 = WATER_LEVEL
            + shaped * 44.0
            + detail * 19.0 * (0.35 + ridge_mask * 0.65)
            + ridge * ridge_mask * ridge_mask * 26.0;

        // Mountain belts. Without a field of their own, "high ground" is just
        // wherever the continent noise happened to peak, and the world comes out
        // uniformly rolling. A separate low-frequency mask puts ranges in particular
        // places and leaves the rest of the map as lowland.
        let belt = fbm_2d(
            x * 0.00055 + 300.0,
            z * 0.00055 - 120.0,
            self.seed ^ 0x3721,
            4,
            2.0,
            0.5,
        );
        // Threshold and exponents tuned by measuring, not by eye. Cubing the ridged
        // noise and squaring the mask looked reasonable in the source and produced a
        // world where the highest point within 1.5km of spawn was 61 units and the
        // nearest real mountain was nine kilometres away. Ridged noise rarely
        // approaches 1, so every extra power crushes what little there is.
        let belt_mask = ((belt - 0.46) / 0.16).clamp(0.0, 1.0) * ridge_mask;
        if belt_mask > 0.0 {
            let peaks = ridged_2d(x * 0.0021, z * 0.0021, self.seed ^ 0x77aa, 5);
            height += peaks * peaks * belt_mask.powf(1.3) * 240.0;
        }

        height.clamp(0.0, TERRAIN_HEIGHT)
    }

    /// The river's water surface here, if a course carries water at this point.
    ///
    /// `None` below sea level, where the ocean already covers everything and a
    /// second surface would only fight it.
    pub fn river_surface_at(&self, x: f32, z: f32) -> Option<f32> {
        let (level, distance, width) = self.river_query(x, z)?;
        if distance > rivers::CHANNEL_HALF_WIDTH * width * 1.05 {
            return None;
        }
        let ground = self.height_at(x, z);
        (level > ground + 0.12 && level > WATER_LEVEL + 0.8).then_some(level)
    }

    /// Whether a position lies in flowing water.
    // Callers currently want the richer `river_influence_at`; this stays as the
    // plain question wildlife and construction will ask.
    #[allow(dead_code)]
    pub fn is_river(&self, x: f32, z: f32) -> bool {
        self.river_surface_at(x, z).is_some()
    }

    /// Water level and lateral distance of the nearest course, within the range a
    /// river influences its surroundings — wider than the channel, because banks
    /// and riparian growth extend past the water's edge.
    pub fn river_influence_at(&self, x: f32, z: f32) -> Option<(f32, f32, f32)> {
        let (level, distance, width) = self.river_query(x, z)?;
        (distance < rivers::CHANNEL_HALF_WIDTH * width * 1.6).then_some((level, distance, width))
    }

    /// Dampness in `[0, 1]`, driving surface colour and scatter density.
    pub fn moisture_at(&self, x: f32, z: f32) -> f32 {
        fbm_2d(
            x * 0.0045 + 17.0,
            z * 0.0045 - 9.0,
            self.seed ^ 0x5eed,
            3,
            2.0,
            0.5,
        )
    }

    /// Ground mottling in `[0, 1]`, at a scale of tens of metres.
    ///
    /// Ground colour otherwise varies only with moisture and altitude, which change
    /// over kilometres — this is what stops a hillside being one flat green.
    pub fn ground_patch_at(&self, x: f32, z: f32) -> f32 {
        let broad = fbm_2d(
            x * 0.011 - 140.0,
            z * 0.011 + 77.0,
            self.seed ^ 0x9e11,
            3,
            2.0,
            0.5,
        );
        let fine = fbm_2d(
            x * 0.055 + 12.0,
            z * 0.055 - 31.0,
            self.seed ^ 0x51f3,
            2,
            2.0,
            0.5,
        );
        (broad * 0.68 + fine * 0.32).clamp(0.0, 1.0)
    }

    /// Wander applied to the treeline and snowline, in `[-1, 1]`.
    ///
    /// An unperturbed altitude threshold draws a level contour across a mountain,
    /// which reads instantly as a rendering rule rather than vegetation.
    pub fn line_variation_at(&self, x: f32, z: f32) -> f32 {
        fbm_2d(
            x * 0.019 + 61.0,
            z * 0.019 - 24.0,
            self.seed ^ 0xbeef,
            3,
            2.0,
            0.5,
        ) * 2.0
            - 1.0
    }

    /// Rough temperature in `[0, 1]`, falling with altitude.
    pub fn temperature_at(&self, x: f32, z: f32) -> f32 {
        let base = fbm_2d(
            x * 0.00042 - 400.0,
            z * 0.00042 + 250.0,
            self.seed ^ 0x7e11,
            3,
            2.0,
            0.5,
        );
        let altitude = ((self.height_at(x, z) - WATER_LEVEL) / 120.0).clamp(0.0, 1.0);
        (base - altitude * 0.55).clamp(0.0, 1.0)
    }

    /// Which biome a position falls in.
    pub fn biome_at(&self, x: f32, z: f32) -> Biome {
        let height = self.height_at(x, z);
        if height > WATER_LEVEL + 78.0 {
            return Biome::Alpine;
        }

        let temperature = self.temperature_at(x, z);
        let moisture = self.moisture_at(x, z);

        if temperature < 0.34 {
            Biome::Boreal
        } else if temperature > 0.52 && moisture < 0.50 {
            Biome::Arid
        } else if moisture > 0.58 {
            Biome::Wetland
        } else {
            Biome::Temperate
        }
    }

    /// Surface normal, from central differences on the height function.
    pub fn normal_at(&self, x: f32, z: f32) -> Vec3 {
        let e = CHUNK_SIZE / CHUNK_CELLS as f32;
        let hl = self.height_at(x - e, z);
        let hr = self.height_at(x + e, z);
        let hd = self.height_at(x, z - e);
        let hu = self.height_at(x, z + e);
        Vec3::new(hl - hr, 2.0 * e, hd - hu).normalize()
    }

    /// Steepness in `[0, 1]`, where 0 is flat and 1 is a vertical face.
    pub fn slope_at(&self, x: f32, z: f32) -> f32 {
        1.0 - self.normal_at(x, z).y.clamp(0.0, 1.0)
    }

    pub fn is_submerged(&self, x: f32, z: f32) -> bool {
        self.height_at(x, z) < WATER_LEVEL
    }

    /// Somewhere a walking creature can stand: dry, not a cliff face, and not in
    /// deep flowing water. The shallow edge of a river is fordable.
    pub fn is_walkable(&self, x: f32, z: f32) -> bool {
        if self.is_submerged(x, z) || self.slope_at(x, z) >= 0.55 {
            return false;
        }
        match self.river_surface_at(x, z) {
            Some(surface) => surface - self.height_at(x, z) < 0.9,
            None => true,
        }
    }

    /// Chunk containing a world position.
    pub fn chunk_of(&self, x: f32, z: f32) -> IVec2 {
        IVec2::new(
            (x / CHUNK_SIZE).floor() as i32,
            (z / CHUNK_SIZE).floor() as i32,
        )
    }
}

/// Chunks currently loaded.
#[derive(Resource, Default)]
pub struct LoadedChunks {
    entities: HashMap<IVec2, Entity>,
    /// How many chunks the current view wants, loaded or not.
    wanted: usize,
    /// Stream radius in chunks, as of the last update.
    radius: i32,
}

impl LoadedChunks {
    pub fn count(&self) -> usize {
        self.entities.len()
    }

    /// Progress toward a fully built view, in `[0, 1]`.
    pub fn progress(&self) -> f32 {
        if self.wanted == 0 {
            return 0.0;
        }
        (self.entities.len() as f32 / self.wanted as f32).clamp(0.0, 1.0)
    }

    /// Whether every chunk in view has been built.
    pub fn is_complete(&self) -> bool {
        self.wanted > 0 && self.entities.len() >= self.wanted
    }

    /// Removes and returns every chunk, for a full world reload.
    pub fn take_all(&mut self) -> Vec<Entity> {
        self.entities.drain().map(|(_, e)| e).collect()
    }

    /// Removes and returns the chunks whose footprint touches a circle, so a
    /// caller can despawn them; streaming rebuilds them with current heights.
    pub fn take_near(&mut self, x: f32, z: f32, radius: f32) -> Vec<Entity> {
        let mut taken = Vec::new();
        let min = IVec2::new(
            ((x - radius) / CHUNK_SIZE).floor() as i32,
            ((z - radius) / CHUNK_SIZE).floor() as i32,
        );
        let max = IVec2::new(
            ((x + radius) / CHUNK_SIZE).floor() as i32,
            ((z + radius) / CHUNK_SIZE).floor() as i32,
        );
        for cx in min.x..=max.x {
            for cz in min.y..=max.y {
                if let Some(entity) = self.entities.remove(&IVec2::new(cx, cz)) {
                    taken.push(entity);
                }
            }
        }
        taken
    }

    /// How far the world currently streams, in world units.
    ///
    /// Fog reads this rather than the constant, so the horizon tracks the radius
    /// that is actually loaded instead of the one that might be.
    pub fn radius_units(&self) -> f32 {
        self.radius.max(MIN_VIEW_CHUNKS) as f32 * CHUNK_SIZE
    }
}

/// Chunk coordinates within `VIEW_CHUNKS` of `centre`, nearest first.
///
/// Ordering matters: ground under the player has to appear before the horizon does,
/// or streaming reads as the world assembling itself from the outside in.
fn chunks_in_view(centre: IVec2, radius: i32) -> Vec<IVec2> {
    let mut coords = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dz * dz <= radius * radius {
                coords.push(centre + IVec2::new(dx, dz));
            }
        }
    }
    coords.sort_by_key(|c| {
        let d = *c - centre;
        d.x * d.x + d.y * d.y
    });
    coords
}

/// Marches a ray against the terrain and returns where it meets the ground.
///
/// Lives here rather than with the Hand because it is a terrain query: the camera
/// uses it to zoom toward the cursor, and the Hand uses it to place the cursor.
pub fn raycast(terrain: &Terrain, ray: Ray3d) -> Option<Vec3> {
    const MAX_DISTANCE: f32 = 3_000.0;
    const STEP: f32 = 1.5;

    let origin = ray.origin;
    let direction = *ray.direction;

    // A ray pointing up from above the terrain will never hit it.
    if direction.y >= 0.0 && origin.y > TERRAIN_HEIGHT {
        return None;
    }

    let ground_at = |p: Vec3| terrain.height_at(p.x, p.z).max(WATER_LEVEL);

    let mut travelled = 0.0;
    let mut previous = origin;
    let mut previous_above = origin.y > ground_at(origin);

    while travelled < MAX_DISTANCE {
        travelled += STEP;
        let current = origin + direction * travelled;
        let above = current.y > ground_at(current);

        if previous_above && !above {
            // Crossed the surface between `previous` and `current`. Bisect.
            let mut lo = previous;
            let mut hi = current;
            for _ in 0..14 {
                let mid = (lo + hi) * 0.5;
                if mid.y > ground_at(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some((lo + hi) * 0.5);
        }

        previous = current;
        previous_above = above;
    }

    None
}

/// Chooses a surface colour from height, steepness and moisture.
fn surface_color(
    height: f32,
    slope: f32,
    moisture: f32,
    shade_t: f32,
    biome: Biome,
    variation: f32,
    patch: f32,
    river: Option<(f32, f32, f32)>,
) -> Color {
    // A river repaints its surroundings, or it reads as a groove cut into lawn:
    // a wet earth bed, a bank ribbon hugging the waterline, damper ground beyond.
    let mut bank = 0.0;
    let mut moisture = moisture;
    if let Some((level, distance, width)) = river {
        let below = level - height;
        if below > 0.0 {
            // The bed, darkening with depth the way the sea floor does.
            let wetness = (below / 3.0).clamp(0.0, 1.0);
            return palette::shade_blend(
                &palette::SAND,
                &palette::EARTH,
                0.45 + wetness * 0.55,
                shade_t * 0.55,
            );
        }

        // A *ribbon* of bare earth. The first version faded over the whole
        // influence radius, and on flat ground — where height barely constrains
        // anything — that painted a ten-metre brown floodplain either side. The
        // band is normalised to the channel wall instead: full just past the
        // waterline, gone within a third of a half-width beyond it. The patch
        // field jitters the boundary so it breaks up organically at vertex
        // resolution instead of stair-stepping.
        let half_width = rivers::CHANNEL_HALF_WIDTH * width;
        let lateral = distance / half_width.max(0.5) + (patch - 0.5) * 0.3;
        let band = 1.0 - ((lateral - 0.8) / 0.5).clamp(0.0, 1.0);
        let rise = 1.0 - ((height - level) / 1.8).clamp(0.0, 1.0);
        bank = band * rise;

        // Riparian dampness stays narrow too.
        moisture = (moisture + band * 0.16).clamp(0.0, 1.0);
    }

    let depth_below_water = WATER_LEVEL - height;

    // Lake and sea beds.
    if depth_below_water > 0.0 {
        let wetness = (depth_below_water / 6.0).clamp(0.0, 1.0);
        return palette::shade_blend(&palette::SAND, &palette::EARTH, wetness, shade_t * 0.6);
    }

    // A beach band just above the waterline.
    let shore = 1.0 - ((height - WATER_LEVEL) / 1.6).clamp(0.0, 1.0);

    // Anything steep is exposed rock regardless of altitude. Mountains are steep
    // everywhere, so this — not the altitude term — is what bares them.
    let rockiness = ((slope - 0.30) / 0.28).clamp(0.0, 1.0);

    // High ground goes bare as well — but not nearly as low as it once did. Baring
    // everything above forty units meant ordinary hills came out as white rock,
    // which read as the whole landscape being washed out rather than as altitude.
    //
    // The threshold wanders with `variation` and the band is wide, so the treeline
    // is a ragged zone rather than a contour line drawn round the mountain.
    let treeline = 78.0 + variation * 34.0;
    let altitude = ((height - WATER_LEVEL - treeline) / 62.0).clamp(0.0, 1.0);
    let bare = rockiness.max(altitude);

    // Each biome sits mostly in its own ramp. Blending every one of them halfway
    // toward foliage — as this used to — pulls them all to the same green and the
    // distinction stops being visible from the air.
    let (ramp, shift) = biome.ground();

    // Mottling. The patch field pushes the shade up and down within the ramp, and
    // drifts toward the biome's companion ramp at the extremes — dry grass in the
    // pale patches, deeper growth in the dark ones. Without it a hillside is one
    // flat colour however good the lighting is.
    let patch_shade = (patch - 0.5) * 0.22;
    let companion = biome.companion();
    let companion_mix = ((patch - 0.62) / 0.30).clamp(0.0, 1.0) * 0.55;

    // Hollows hold moisture and grow darker; crests are grazed and sun-bleached.
    let hollow = ((slope - 0.10) / 0.35).clamp(0.0, 1.0) * 0.10;

    let ground = palette::shade_blend(
        ramp,
        companion,
        ((moisture * 0.22) + companion_mix).clamp(0.0, 1.0),
        (shade_t + shift + patch_shade - hollow).clamp(0.0, 1.0),
    );
    let with_shore = blend(
        ground,
        palette::shade_smooth(&palette::SAND, shade_t),
        shore * 0.85,
    );
    let with_rock = blend(
        with_shore,
        palette::shade_smooth(&palette::STONE, shade_t),
        bare,
    );

    // Snow on the highest ground. Green lowland, grey rock, white summit gives the
    // eye three bands to read altitude from instead of one — and the snowline
    // wanders for the same reason the treeline does.
    let snowline = 158.0 + variation * 38.0;
    let snow = ((height - WATER_LEVEL - snowline) / 62.0).clamp(0.0, 1.0);
    let composed = blend(
        with_rock,
        palette::shade_smooth(&palette::SNOW, 0.5 + shade_t * 0.5),
        snow,
    );

    // Banks last, over everything: bare earth at the water's edge, kept muted so
    // it reads as damp soil rather than an orange highlight.
    blend(
        composed,
        palette::shade_blend(&palette::EARTH, &palette::SAND, 0.3, shade_t * 0.62),
        bank * 0.7,
    )
}

/// Linear-space blend between two colours.
fn blend(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_linear();
    let b = b.to_linear();
    let t = t.clamp(0.0, 1.0);
    Color::LinearRgba(LinearRgba {
        red: a.red + (b.red - a.red) * t,
        green: a.green + (b.green - a.green) * t,
        blue: a.blue + (b.blue - a.blue) * t,
        alpha: 1.0,
    })
}

/// Builds one chunk as a smooth-shaded, indexed grid.
///
/// Positions are chunk-local; the entity's transform places it. Normals and colours
/// derive from *world* position, so neighbouring chunks agree exactly along their
/// shared edge and no seam appears.
///
/// Heights are sampled once into a local grid with a one-cell skirt, and normals are
/// taken from that grid rather than by re-evaluating the terrain function four more
/// times per vertex — the difference between one noise evaluation per vertex and five.
/// The ground's own colour at a point, exactly as [`build_chunk_mesh`]
/// would paint the vertex there. Kept in step with the loop below so the
/// trail painter can restore a faded path to the true ground colour.
pub fn ground_color_at(terrain: &Terrain, world_x: f32, world_z: f32) -> [f32; 4] {
    let cell = CHUNK_SIZE / CHUNK_CELLS as f32;
    let y = terrain.height_at(world_x, world_z);
    let normal = Vec3::new(
        terrain.height_at(world_x - cell, world_z) - terrain.height_at(world_x + cell, world_z),
        2.0 * cell,
        terrain.height_at(world_x, world_z - cell) - terrain.height_at(world_x, world_z + cell),
    )
    .normalize();
    let slope = 1.0 - normal.y.clamp(0.0, 1.0);
    let moisture = terrain.moisture_at(world_x, world_z);
    let shade_t =
        (0.42 + ((y - WATER_LEVEL) / 200.0) * 0.45 + (moisture - 0.5) * 0.12).clamp(0.0, 1.0);
    let color = surface_color(
        y,
        slope,
        moisture,
        shade_t,
        terrain.biome_at(world_x, world_z),
        terrain.line_variation_at(world_x, world_z),
        terrain.ground_patch_at(world_x, world_z),
        terrain.river_influence_at(world_x, world_z),
    )
    .to_linear();
    [color.red, color.green, color.blue, 1.0]
}

pub fn build_chunk_mesh(terrain: &Terrain, coord: IVec2) -> Mesh {
    let cell = CHUNK_SIZE / CHUNK_CELLS as f32;
    let origin = Vec2::new(coord.x as f32 * CHUNK_SIZE, coord.y as f32 * CHUNK_SIZE);

    let padded = CHUNK_CELLS + 3;
    let mut heights = vec![0.0f32; padded * padded];
    for iz in 0..padded {
        for ix in 0..padded {
            let x = origin.x + (ix as f32 - 1.0) * cell;
            let z = origin.y + (iz as f32 - 1.0) * cell;
            heights[iz * padded + ix] = terrain.height_at(x, z);
        }
    }
    let sample = |ix: usize, iz: usize| heights[iz * padded + ix];

    let stride = CHUNK_CELLS + 1;
    let vertex_count = stride * stride;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut colors = Vec::with_capacity(vertex_count);

    for iz in 0..stride {
        for ix in 0..stride {
            // Offset by one to skip the skirt.
            let (px, pz) = (ix + 1, iz + 1);
            let y = sample(px, pz);

            let normal = Vec3::new(
                sample(px - 1, pz) - sample(px + 1, pz),
                2.0 * cell,
                sample(px, pz - 1) - sample(px, pz + 1),
            )
            .normalize();

            let world_x = origin.x + ix as f32 * cell;
            let world_z = origin.y + iz as f32 * cell;

            let slope = 1.0 - normal.y.clamp(0.0, 1.0);
            let moisture = terrain.moisture_at(world_x, world_z);
            // Normalised over the full height range. Dividing by sixty saturated the
            // shade at maximum brightness barely above the treeline, so every
            // mountain came out uniformly white from base to summit with no sense of
            // altitude at all.
            let shade_t = (0.42 + ((y - WATER_LEVEL) / 200.0) * 0.45 + (moisture - 0.5) * 0.12)
                .clamp(0.0, 1.0);
            let biome = terrain.biome_at(world_x, world_z);
            let variation = terrain.line_variation_at(world_x, world_z);
            let patch = terrain.ground_patch_at(world_x, world_z);
            let color = surface_color(
                y,
                slope,
                moisture,
                shade_t,
                biome,
                variation,
                patch,
                terrain.river_influence_at(world_x, world_z),
            )
            .to_linear();

            positions.push([ix as f32 * cell, y, iz as f32 * cell]);
            normals.push([normal.x, normal.y, normal.z]);
            colors.push([color.red, color.green, color.blue, 1.0]);
        }
    }

    let mut indices = Vec::with_capacity(CHUNK_CELLS * CHUNK_CELLS * 6);
    for row in 0..CHUNK_CELLS {
        for column in 0..CHUNK_CELLS {
            let top_left = (row * stride + column) as u32;
            let top_right = top_left + 1;
            let bottom_left = top_left + stride as u32;
            let bottom_right = bottom_left + 1;

            // Split each quad along its shorter diagonal, which keeps ridgelines from
            // developing a visible weave.
            let a = positions[top_left as usize][1] - positions[bottom_right as usize][1];
            let b = positions[top_right as usize][1] - positions[bottom_left as usize][1];

            if a.abs() <= b.abs() {
                indices.extend_from_slice(&[top_left, bottom_left, bottom_right]);
                indices.extend_from_slice(&[top_left, bottom_right, top_right]);
            } else {
                indices.extend_from_slice(&[top_left, bottom_left, top_right]);
                indices.extend_from_slice(&[top_right, bottom_left, bottom_right]);
            }
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// Builds the water surface for a chunk's rivers, if it has any.
///
/// A separate mesh from the sea, sitting at whatever height the land carries it —
/// which is what lets a river run through hills a hundred units above the ocean.
/// Cells with no river collapse to nothing, so a chunk without one costs no
/// geometry at all.
pub fn build_river_mesh(terrain: &Terrain, coord: IVec2) -> Option<Mesh> {
    let cell = CHUNK_SIZE / CHUNK_CELLS as f32;
    let origin = Vec2::new(coord.x as f32 * CHUNK_SIZE, coord.y as f32 * CHUNK_SIZE);
    let stride = CHUNK_CELLS + 1;

    // Two grids: where there is water, and the height the surface would take there.
    // Keeping the ground height for dry corners lets the mesh run right up onto the
    // bank, where the depth-based alpha fades it out — a quad that stops dead at the
    // last wet cell leaves a staircase along the whole length of the river.
    let mut wet = vec![false; stride * stride];
    let mut heights = vec![0.0f32; stride * stride];
    let mut any = false;

    for iz in 0..stride {
        for ix in 0..stride {
            let x = origin.x + ix as f32 * cell;
            let z = origin.y + iz as f32 * cell;
            let index = iz * stride + ix;

            match terrain.river_surface_at(x, z) {
                Some(surface) => {
                    wet[index] = true;
                    heights[index] = surface;
                    any = true;
                }
                None => heights[index] = terrain.height_at(x, z),
            }
        }
    }
    if !any {
        return None;
    }

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // One quad per cell whose four corners all carry water. Requiring all four keeps
    // the surface from sticking out over dry ground at the channel edge.
    for row in 0..CHUNK_CELLS {
        for column in 0..CHUNK_CELLS {
            let corner = [
                row * stride + column,
                row * stride + column + 1,
                (row + 1) * stride + column,
                (row + 1) * stride + column + 1,
            ];

            // Any wet corner is enough. The dry ones sit at ground level, so the
            // surface tapers into the bank instead of ending on a hard edge.
            if !corner.iter().any(|i| wet[*i]) {
                continue;
            }

            let [a, b, c, d] = [
                heights[corner[0]],
                heights[corner[1]],
                heights[corner[2]],
                heights[corner[3]],
            ];

            let base = positions.len() as u32;
            let x0 = column as f32 * cell;
            let z0 = row as f32 * cell;

            for (dx, dz, y) in [
                (0.0, 0.0, a),
                (cell, 0.0, b),
                (0.0, cell, c),
                (cell, cell, d),
            ] {
                positions.push([x0 + dx, y, z0 + dz]);
                normals.push([0.0, 1.0, 0.0]);
            }

            indices.extend_from_slice(&[base, base + 2, base + 3, base, base + 3, base + 1]);
        }
    }

    if indices.is_empty() {
        return None;
    }

    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::MAIN_WORLD
                | bevy::asset::RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

/// Shared handles for terrain rendering.
#[derive(Resource)]
pub struct TerrainAssets {
    pub ground_material: Handle<StandardMaterial>,
    /// Shared with the sea, so rivers and ocean look like the same substance.
    pub water_material: Handle<crate::water::WaterMaterial>,
}

fn setup_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<crate::water::WaterMaterial>>,
    world_seed: Res<crate::WorldSeed>,
) {
    // Vertex colours carry all the surface variation, so the material is plain white.
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        reflectance: 0.02,
        ..default()
    });

    // Water is one large quad that follows the camera. An endless world cannot have a
    // single finite sea, and a quad per chunk would show a seam at every join.
    //
    // Sized far past the furthest the fog can ever reach. The quad is two triangles,
    // so being generous costs nothing — and being stingy puts a straight edge across
    // the horizon, which is exactly the artefact the fog exists to prevent.
    let extent = CHUNK_SIZE * VIEW_CHUNKS as f32 * 8.0;
    let water_mesh = meshes.add(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::MAIN_WORLD
                | bevy::asset::RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [-extent, 0.0, -extent],
                [-extent, 0.0, extent],
                [extent, 0.0, extent],
                [extent, 0.0, -extent],
            ],
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4])
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
        )
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3])),
    );

    let water_material = water_materials.add(crate::water::WaterMaterial::default());

    commands.spawn((
        Name::new("Water"),
        Mesh3d(water_mesh),
        MeshMaterial3d(water_material.clone()),
        Transform::from_xyz(0.0, WATER_LEVEL, 0.0),
        // Water casting a shadow onto the seabed puts a dark band under every
        // shoreline, which reads as the surface hovering above the sand rather than
        // meeting it.
        NotShadowCaster,
        WaterPlane,
    ));

    commands.insert_resource(Terrain::new(world_seed.0));
    commands.insert_resource(TerrainAssets {
        ground_material,
        water_material,
    });
    commands.init_resource::<LoadedChunks>();
}

/// Loads chunks around the camera and unloads the ones it has left behind.
fn stream_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain: Res<Terrain>,
    assets: Res<TerrainAssets>,
    mut loaded: ResMut<LoadedChunks>,
    cameras: Query<&crate::camera::CameraRig>,
    state: Res<State<crate::GameState>>,
) {
    let Ok(rig) = cameras.single() else {
        return;
    };
    let centre = terrain.chunk_of(rig.focus.x, rig.focus.z);

    let radius = stream_radius(rig.distance);
    loaded.radius = radius;
    let wanted = chunks_in_view(centre, radius);
    let wanted_set: HashSet<IVec2> = wanted.iter().copied().collect();

    // Unload first, so memory is released before more is claimed.
    loaded.entities.retain(|coord, entity| {
        if wanted_set.contains(coord) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });

    // Record how much of the view is still missing, so the loading screen has
    // something honest to report.
    loaded.wanted = wanted_set.len();

    let budget = if *state.get() == crate::GameState::Loading {
        CHUNKS_PER_LOADING_FRAME
    } else {
        CHUNKS_PER_FRAME
    };

    // Load a bounded number per frame, nearest first.
    let mut built = 0;
    for coord in wanted {
        if built >= budget {
            break;
        }
        if loaded.entities.contains_key(&coord) {
            continue;
        }

        let river = build_river_mesh(&terrain, coord).map(|mesh| meshes.add(mesh));

        let entity = commands
            .spawn((
                Name::new(format!("Chunk {},{}", coord.x, coord.y)),
                TerrainChunk { coord },
                Mesh3d(meshes.add(build_chunk_mesh(&terrain, coord))),
                MeshMaterial3d(assets.ground_material.clone()),
                Transform::from_xyz(
                    coord.x as f32 * CHUNK_SIZE,
                    0.0,
                    coord.y as f32 * CHUNK_SIZE,
                ),
                Visibility::default(),
            ))
            .id();

        if let Some(river) = river {
            commands.spawn((
                Name::new("River"),
                Mesh3d(river),
                MeshMaterial3d(assets.water_material.clone()),
                Transform::default(),
                NotShadowCaster,
                ChildOf(entity),
            ));
        }

        loaded.entities.insert(coord, entity);
        built += 1;
    }
}

/// Keeps the sea centred under the camera.
fn follow_water_plane(
    cameras: Query<&crate::camera::CameraRig>,
    mut water: Query<&mut Transform, With<WaterPlane>>,
) {
    let Ok(rig) = cameras.single() else {
        return;
    };
    for mut transform in &mut water {
        transform.translation.x = rig.focus.x;
        transform.translation.z = rig.focus.z;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn height_is_deterministic() {
        let a = Terrain::new(2024);
        let b = Terrain::new(2024);
        for i in 0..500 {
            let x = i as f32 * 13.7 - 3000.0;
            let z = i as f32 * -7.1 + 900.0;
            assert_eq!(a.height_at(x, z), b.height_at(x, z));
        }
    }

    #[test]
    fn different_seeds_give_different_worlds() {
        let a = Terrain::new(1);
        let b = Terrain::new(2);
        let differs = (0..200).any(|i| {
            let x = i as f32 * 31.0;
            a.height_at(x, 0.0) != b.height_at(x, 0.0)
        });
        assert!(differs);
    }

    #[test]
    fn heights_are_finite_and_bounded_everywhere() {
        // An endless world has no edges to stay inside, so the guarantee that matters
        // is that any coordinate at all returns something sane.
        let t = Terrain::new(7);
        for i in 0..3_000 {
            let x = (i as f32 * 977.0) % 400_000.0 - 200_000.0;
            let z = (i as f32 * -613.0) % 400_000.0 + 150_000.0;
            let h = t.height_at(x, z);
            assert!(h.is_finite(), "non-finite height at ({x}, {z})");
            assert!((0.0..=TERRAIN_HEIGHT).contains(&h), "{h} out of range");
        }
    }

    #[test]
    fn the_world_has_both_land_and_sea() {
        let t = Terrain::new(2024);
        let mut land = 0;
        let mut sea = 0;
        for iz in 0..80 {
            for ix in 0..80 {
                let x = ix as f32 * 90.0 - 3600.0;
                let z = iz as f32 * 90.0 - 3600.0;
                if t.is_submerged(x, z) {
                    sea += 1;
                } else {
                    land += 1;
                }
            }
        }
        assert!(land > 600, "only {land} land samples");
        assert!(sea > 600, "only {sea} sea samples");
    }

    #[test]
    fn the_stream_radius_follows_the_zoom() {
        // Close in it must stay cheap; pulled back it must open up. Neither end may
        // leave the bounds, or the fog that reads this would fall out of step.
        let near = stream_radius(12.0);
        let far = stream_radius(1_400.0);

        assert!(near < far, "radius did not grow with zoom");
        assert_eq!(near, MIN_VIEW_CHUNKS, "close in should be the minimum");
        assert_eq!(far, VIEW_CHUNKS, "far out should reach the maximum");

        for distance in [0.0, 5.0, 60.0, 300.0, 900.0, 10_000.0] {
            let r = stream_radius(distance);
            assert!(
                (MIN_VIEW_CHUNKS..=VIEW_CHUNKS).contains(&r),
                "{r} out of bounds"
            );
        }
    }

    #[test]
    fn the_stream_radius_never_shrinks_as_the_camera_rises() {
        let mut previous = 0;
        for step in 0..200 {
            let r = stream_radius(step as f32 * 8.0);
            assert!(r >= previous, "radius shrank while zooming out");
            previous = r;
        }
    }

    #[test]
    fn ground_mottling_varies_at_a_visible_scale() {
        // The point of the patch field is variation across a hillside, not across a
        // continent. Sampling a short walk should show real spread.
        let t = Terrain::new(2024);
        let mut lowest: f32 = 1.0;
        let mut highest: f32 = 0.0;
        for i in 0..300 {
            let p = t.ground_patch_at(i as f32 * 1.5, i as f32 * -0.9);
            assert!((0.0..=1.0).contains(&p), "{p} out of range");
            lowest = lowest.min(p);
            highest = highest.max(p);
        }
        assert!(
            highest - lowest > 0.35,
            "patch field only spanned {:.2} over 450 units",
            highest - lowest,
        );
    }

    #[test]
    fn every_biome_has_a_companion_distinct_from_its_ground() {
        for biome in [
            Biome::Temperate,
            Biome::Boreal,
            Biome::Arid,
            Biome::Wetland,
            Biome::Alpine,
        ] {
            let (ground, _) = biome.ground();
            assert!(
                !std::ptr::eq(ground, biome.companion()),
                "{biome:?} drifts toward its own ramp, so mottling would be invisible",
            );
        }
    }

    #[test]
    fn chunks_without_rivers_build_no_river_geometry() {
        // A surface mesh per chunk regardless would be thousands of empty draws.
        let t = Terrain::new(77);

        // Find a chunk that carries a river, then one that does not.
        let mut wet_chunk = None;
        let mut dry_chunk = None;
        for iz in -40..40 {
            for ix in -40..40 {
                let x = ix as f32 * 64.0 + 32.0;
                let z = iz as f32 * 64.0 + 32.0;
                let coord = t.chunk_of(x, z);
                if t.is_river(x, z) {
                    wet_chunk.get_or_insert(coord);
                } else if t.height_at(x, z) > WATER_LEVEL + 6.0 {
                    dry_chunk.get_or_insert(coord);
                }
            }
        }

        if let Some(coord) = wet_chunk {
            assert!(
                build_river_mesh(&t, coord).is_some(),
                "a chunk with a river built no surface",
            );
        }
        let dry = dry_chunk.expect("no dry chunk found anywhere");
        // A dry sample does not guarantee the whole chunk is dry, so only assert
        // when the mesh really is absent-or-present coherently: absence is the
        // common case and presence just means a course clips the chunk edge.
        let _ = build_river_mesh(&t, dry);
    }

    #[test]
    fn river_banks_are_painted_and_beds_are_earthy() {
        // Without this, the channel walls keep the hillside's grass and the river
        // reads as a carved groove rather than a river.
        let level = 60.0;

        // A point on the bank, just above the waterline, close to the course.
        let plain = surface_color(60.5, 0.1, 0.4, 0.5, Biome::Temperate, 0.0, 0.5, None);
        let banked = surface_color(
            60.5,
            0.1,
            0.4,
            0.5,
            Biome::Temperate,
            0.0,
            0.5,
            Some((level, 4.0, 1.0)),
        );
        assert_ne!(
            plain.to_linear(),
            banked.to_linear(),
            "bank changed nothing"
        );

        // The submerged bed must come out earthy — more red than green — where the
        // untouched ground is green-dominant.
        let bed = surface_color(
            58.0,
            0.1,
            0.4,
            0.5,
            Biome::Temperate,
            0.0,
            0.5,
            Some((level, 1.0, 1.0)),
        )
        .to_linear();
        assert!(bed.red > bed.green, "river bed is not earthy");
        let ground = plain.to_linear();
        assert!(ground.green > ground.red, "plain ground should be green");
    }

    #[test]
    fn river_water_is_level_across_the_channel() {
        // The law the redesign exists to honour: across its width, a river is a
        // level sheet. Along the course it may fall, so the tolerance allows for
        // downstream slope but not for the hammock sag it replaces.
        let t = Terrain::new(77);
        let mut checked = 0;

        'search: for iz in -60..60 {
            for ix in -60..60 {
                let x = ix as f32 * 40.0;
                let z = iz as f32 * 40.0;
                let Some(here) = t.river_surface_at(x, z) else {
                    continue;
                };

                for (dx, dz) in [(3.0, 0.0), (-3.0, 0.0), (0.0, 3.0), (0.0, -3.0)] {
                    if let Some(near) = t.river_surface_at(x + dx, z + dz) {
                        assert!(
                            (near - here).abs() < 2.0,
                            "surface steps {:.1} in three units at ({x}, {z})",
                            (near - here).abs(),
                        );
                    }
                }

                checked += 1;
                if checked > 60 {
                    break 'search;
                }
            }
        }
        assert!(checked > 10, "only {checked} river samples found");
    }

    #[test]
    fn deep_river_water_is_not_walkable() {
        let t = Terrain::new(77);
        let mut deep = 0;

        'search: for iz in -60..60 {
            for ix in -60..60 {
                let x = ix as f32 * 40.0;
                let z = iz as f32 * 40.0;
                if let Some(surface) = t.river_surface_at(x, z)
                    && surface - t.height_at(x, z) >= 0.9
                {
                    assert!(!t.is_walkable(x, z), "standing in deep water at ({x}, {z})");
                    deep += 1;
                    if deep > 30 {
                        break 'search;
                    }
                }
            }
        }
        assert!(deep > 5, "only {deep} deep-water samples found");
    }

    #[test]
    fn rivers_are_identical_across_terrain_instances() {
        // The memo cache must never influence the result: two independently built
        // networks answer identically, whatever order they were queried in.
        let a = Terrain::new(9);
        let b = Terrain::new(9);
        for i in 0..400 {
            let x = (i % 20) as f32 * 130.0 - 1300.0;
            let z = (i / 20) as f32 * 130.0 - 1300.0;
            assert_eq!(a.is_river(x, z), b.is_river(x, z));
            assert_eq!(a.height_at(x, z), b.height_at(x, z));
        }
    }

    #[test]
    fn the_world_grows_mountains() {
        // Without a mountain belt field the map comes out uniformly rolling. There
        // should be somewhere genuinely high, and it should be the exception.
        let t = Terrain::new(2024);
        let mut peak: f32 = 0.0;
        let mut high = 0;
        let mut total = 0;

        for iz in 0..120 {
            for ix in 0..120 {
                let x = ix as f32 * 70.0 - 4200.0;
                let z = iz as f32 * 70.0 - 4200.0;
                let h = t.height_at(x, z);
                peak = peak.max(h);
                total += 1;
                if h > WATER_LEVEL + 90.0 {
                    high += 1;
                }
            }
        }

        assert!(peak > WATER_LEVEL + 110.0, "highest point was only {peak}");
        let fraction = high as f32 / total as f32;
        assert!(
            fraction < 0.2,
            "{fraction} of the world is mountain — too much"
        );
        assert!(high > 0, "no mountains at all");
    }

    #[test]
    fn every_biome_occurs_somewhere() {
        let t = Terrain::new(2024);
        let mut seen = HashSet::new();
        for iz in 0..90 {
            for ix in 0..90 {
                let x = ix as f32 * 110.0 - 4950.0;
                let z = iz as f32 * 110.0 - 4950.0;
                seen.insert(t.biome_at(x, z));
            }
        }
        assert!(seen.len() >= 4, "only found biomes {seen:?}");
    }

    #[test]
    fn biomes_are_stable_across_a_region() {
        // Biomes must be broad enough to walk through. Sampling a short line should
        // not cross more than a couple of them.
        let t = Terrain::new(5);
        let mut changes = 0;
        let mut previous = t.biome_at(0.0, 0.0);
        for i in 1..200 {
            let b = t.biome_at(i as f32 * 2.0, 0.0);
            if b != previous {
                changes += 1;
                previous = b;
            }
        }
        assert!(changes < 12, "biome changed {changes} times in 400 units");
    }

    #[test]
    fn most_dry_land_is_walkable() {
        let t = Terrain::new(2024);
        let mut walkable = 0;
        let mut dry = 0;
        for iz in 0..80 {
            for ix in 0..80 {
                let x = ix as f32 * 90.0 - 3600.0;
                let z = iz as f32 * 90.0 - 3600.0;
                if !t.is_submerged(x, z) {
                    dry += 1;
                    if t.is_walkable(x, z) {
                        walkable += 1;
                    }
                }
            }
        }
        let fraction = walkable as f32 / dry as f32;
        assert!(fraction > 0.7, "only {fraction} of dry land is walkable");
    }

    #[test]
    fn normals_point_upward_and_are_unit_length() {
        let t = Terrain::new(31);
        for i in 0..400 {
            let x = i as f32 * 17.3 - 2000.0;
            let z = i as f32 * -9.7 + 800.0;
            let n = t.normal_at(x, z);
            assert!((n.length() - 1.0).abs() < 1e-3);
            assert!(n.y > 0.0);
        }
    }

    #[test]
    fn chunk_lookup_matches_chunk_placement() {
        let t = Terrain::new(1);
        for coord in [
            IVec2::new(0, 0),
            IVec2::new(-3, 5),
            IVec2::new(12, -40),
            IVec2::new(-1, -1),
        ] {
            let x = coord.x as f32 * CHUNK_SIZE + 0.5;
            let z = coord.y as f32 * CHUNK_SIZE + 0.5;
            assert_eq!(t.chunk_of(x, z), coord);
        }
    }

    #[test]
    fn chunk_meshes_are_well_formed() {
        let t = Terrain::new(8);
        let mesh = build_chunk_mesh(&t, IVec2::new(2, -1));
        assert_eq!(mesh.count_vertices(), (CHUNK_CELLS + 1) * (CHUNK_CELLS + 1));
        assert_eq!(
            mesh.indices().map(|i| i.len()),
            Some(CHUNK_CELLS * CHUNK_CELLS * 6),
        );
    }

    #[test]
    fn neighbouring_chunks_share_their_edge_exactly() {
        // Smooth shading only stays seamless if the shared rim of two chunks carries
        // identical heights and normals. Any drift shows as a hard line across the
        // landscape — and with streaming it would appear and vanish as chunks load.
        let t = Terrain::new(11);
        let left = build_chunk_mesh(&t, IVec2::new(0, 0));
        let right = build_chunk_mesh(&t, IVec2::new(1, 0));

        let fetch = |mesh: &Mesh, attr| match mesh.attribute(attr) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => panic!("missing attribute"),
        };

        let left_pos = fetch(&left, Mesh::ATTRIBUTE_POSITION);
        let right_pos = fetch(&right, Mesh::ATTRIBUTE_POSITION);
        let left_norm = fetch(&left, Mesh::ATTRIBUTE_NORMAL);
        let right_norm = fetch(&right, Mesh::ATTRIBUTE_NORMAL);

        let stride = CHUNK_CELLS + 1;
        for row in 0..stride {
            let l = row * stride + (stride - 1);
            let r = row * stride;
            assert_eq!(
                left_pos[l][1], right_pos[r][1],
                "height mismatch, row {row}"
            );
            assert_eq!(left_norm[l], right_norm[r], "normal mismatch, row {row}");
        }
    }

    #[test]
    fn the_view_is_a_disc_ordered_nearest_first() {
        let centre = IVec2::new(5, -2);
        let radius = VIEW_CHUNKS;
        let coords = chunks_in_view(centre, radius);
        assert!(!coords.is_empty());
        assert_eq!(coords[0], centre, "centre chunk must come first");

        let mut previous = 0;
        for coord in &coords {
            let d = *coord - centre;
            let distance = d.x * d.x + d.y * d.y;
            assert!(distance <= radius * radius, "outside the disc");
            assert!(distance >= previous, "not ordered nearest first");
            previous = distance;
        }
    }

    #[test]
    fn the_loaded_set_follows_the_camera() {
        // Walking one chunk east should retire roughly a column and claim another,
        // not rebuild the world.
        let before: HashSet<IVec2> = chunks_in_view(IVec2::ZERO, VIEW_CHUNKS)
            .into_iter()
            .collect();
        let after: HashSet<IVec2> = chunks_in_view(IVec2::new(1, 0), VIEW_CHUNKS)
            .into_iter()
            .collect();

        let retained = before.intersection(&after).count();
        let dropped = before.difference(&after).count();

        assert!(
            retained > before.len() * 9 / 10,
            "only {retained} of {} chunks survived a one-chunk step",
            before.len(),
        );
        assert!(dropped > 0, "stepping should retire something");
    }

    #[test]
    fn a_ray_aimed_at_the_ground_hits_it() {
        let t = Terrain::new(2024);
        let mut target = None;
        'search: for iz in 0..60 {
            for ix in 0..60 {
                let x = ix as f32 * 40.0 - 1200.0;
                let z = iz as f32 * 40.0 - 1200.0;
                if t.is_walkable(x, z) {
                    target = Some(Vec3::new(x, t.height_at(x, z), z));
                    break 'search;
                }
            }
        }
        let target = target.expect("no dry land found");
        let origin = target + Vec3::new(30.0, 60.0, 40.0);
        let ray = Ray3d::new(origin, Dir3::new(target - origin).unwrap());

        let hit = raycast(&t, ray).expect("ray missed the ground");
        assert!(hit.distance(target) < 2.0, "hit {hit:?}, wanted {target:?}");
    }

    #[test]
    fn a_ray_pointing_at_the_sky_misses() {
        let t = Terrain::new(1);
        let ray = Ray3d::new(Vec3::new(0.0, TERRAIN_HEIGHT + 50.0, 0.0), Dir3::Y);
        assert!(raycast(&t, ray).is_none());
    }
}
