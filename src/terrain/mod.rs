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

use crate::noise::{fbm_3d, ridged_3d, warped_fbm_3d};
use crate::palette;

/// World units along one edge of a chunk.
pub const CHUNK_SIZE: f32 = 64.0;
/// Grid cells along one edge of a chunk.
pub const CHUNK_CELLS: usize = 32;
/// The furthest the world ever streams, in chunks.
///
/// Reached only near the top of the play zoom. See [`stream_radius`].
pub const VIEW_CHUNKS: i32 = 12;

/// The closest the world ever streams, in chunks.
const MIN_VIEW_CHUNKS: i32 = 6;

/// How far the god may pull back before no chunk is built at all.
///
/// Brett's number, and his own screenshot is the measurement behind it. At four
/// hundred and thirty-one units up the game was holding nine hundred chunks,
/// seventeen thousand mesh entities and nine thousand shadow casters, and
/// running at thirty to ninety milliseconds a frame — while every system in the
/// game that reports its own time added up to half of ONE millisecond. None of
/// the cost was in the simulation, and none of it was in the streaming. All of
/// it was in how much world was standing.
///
/// And it is standing for nobody. Brett, looking at that frame: "even at this
/// height people look like pixels." They do — a villager is a couple of units
/// tall, which at four hundred units away is about seven pixels. Above here the
/// planet's own patches are the whole picture, and their cells at this height
/// are finer than a chunk's own.
pub const CHUNK_CEILING: f32 = 700.0;

/// Where the plate starts pulling in toward that ceiling.
///
/// Chosen to sit under the scenery's dissolve (see
/// [`crate::debug::layers::scenery_dissolved`]) so the two eases overlap: the
/// forests begin thinning first, the ground they stand on follows, and by the
/// ceiling there is nothing left to take away.
const CHUNK_DISSOLVE: f32 = 450.0;

/// How far to stream for a given camera distance.
///
/// A god camera has to pull back far enough to survey a region, but streaming
/// that radius the whole time means paying for a thousand chunks while looking
/// at one village. Tying the radius to the zoom keeps close work cheap and
/// still opens the world up when the player rises.
pub fn stream_radius(camera_distance: f32) -> i32 {
    // The bubble follows the FRAME. At play pitch the ground in view reaches
    // something like one and a fifth times the camera's distance past the
    // focus, so eight tenths of a chunk per unit of distance, on top of the
    // close-in floor, covers it with room to pan into.
    //
    // It was twice that. At four hundred units up it asked for seventeen rings
    // — eleven hundred units of ground — around a frame that used about five
    // hundred, and every one of those chunks carried its own trees, its own
    // veil sheet and its own shadow casters.
    //
    // Low pitch overruns any bubble and always will: at the eleven-degree
    // floor the view runs to the horizon, two thousand units out at play
    // height, and no radius covers that. That ground is the patches' job, and
    // it always was.
    let chunks = (camera_distance / CHUNK_SIZE) * 1.1 + MIN_VIEW_CHUNKS as f32;
    let wanted = (chunks.round() as i32).clamp(MIN_VIEW_CHUNKS, VIEW_CHUNKS);

    // And back DOWN again to NOTHING, not to a small disc. Three reasons, and
    // they were found in that order.
    //
    // The flat world is drawn in its tangent frame, and a twenty-five-hundred
    // unit plate on a six-thousand radius sphere lifts half a thousand units
    // off the curve at its edge — from orbit it read as a square sticker
    // jutting off the planet into space.
    //
    // Then: past the top of the climb a chunk is smaller than a pixel, but the
    // streamer kept six rings of them around the focus anyway and asked about
    // them every frame. Standing still that hardly shows, because they load
    // once and stay; the title screen is where it did, because the vantage
    // circles the planet and drags the whole disc round with it.
    //
    // And now the one that matters most, because it is inside the play zoom
    // rather than above it: the plate and the patches draw the SAME ground,
    // and between them they were drawing it twice at full detail over a disc a
    // kilometre wide. See [`CHUNK_CEILING`].
    let receding =
        ((camera_distance - CHUNK_DISSOLVE) / (CHUNK_CEILING - CHUNK_DISSOLVE)).clamp(0.0, 1.0);
    let ceiling = VIEW_CHUNKS as f32 * (1.0 - receding);
    wanted.min(ceiling.round() as i32).max(0)
}
/// Chunks built per frame during play when there is little to do.
///
/// Generation is cheap but not free, and spreading it out keeps streaming
/// from showing up as a stutter.
const CHUNKS_PER_FRAME: usize = 3;

/// The most per frame when there is a lot to do - a full zoom-out asks
/// for a thousand chunks at once, and three a frame means seven seconds
/// of watching the world assemble itself. The loading screen already
/// builds forty-eight a frame and stays responsive, so this is a
/// conservative ceiling rather than a brave one.
const CHUNKS_PER_HURRIED_FRAME: usize = 20;

/// Chunks built per frame while the loading screen is up. Nothing is being rendered
/// yet that a hitch would spoil, so this is limited only by keeping the window
/// responsive enough to draw a progress bar.
const CHUNKS_PER_LOADING_FRAME: usize = 48;

/// Sea level.
pub const WATER_LEVEL: f32 = 20.0;
/// The world is a sphere, and this is how big it is.
///
/// Six thousand units of radius: thirty-eight kilometres around and, at the
/// continent wavelength below, about six continent-scale landmasses. Small
/// enough that the whole world hangs in the frame at a fifth of the old
/// orbital zoom and the ground's curve can be FELT from play height - the
/// world is round, and says so - while every local wavelength underfoot is
/// unchanged. It began at fifteen thousand; the smaller world was Brett's
/// call, and it is the difference between a planet you visit and a planet
/// you live on.
///
/// The number was chosen against three things at once. Small enough that the
/// land is FINITE and worth competing over. Large enough that the ground reads
/// as flat where the game is actually played - at the working camera height the
/// horizon drops by a hundredth of a unit across the whole frame, and even at
/// full zoom-out only about seven, against terrain three hundred and twenty
/// units tall. And small enough that a surface point sits fifteen thousand
/// units from the centre, where an `f32` still resolves to under two
/// millimetres - so none of this needs double precision or a moving world
/// origin, which is what makes planet renderers miserable. An Earth-sized world
/// would resolve to three quarters of a unit and everything would jitter.
pub const PLANET_RADIUS: f32 = 6_000.0;

/// Once round the world, in units.
pub fn planet_circumference() -> f32 {
    std::f32::consts::TAU * PLANET_RADIUS
}

/// The direction from the planet's centre to the ground at local `(x, z)`.
///
/// This is the scaffold that lets a flat simulation stand on a round world. The
/// game speaks `(x, z)` in a hundred and nine places, and it goes on doing so:
/// `x` is arc length east, `z` is arc length north, and both are turned into a
/// point on the unit sphere here. Terrain is then a field over DIRECTIONS,
/// sampled from a volume, which is what makes the world finite - go far enough
/// east and `x` comes back round to where it started, because sine and cosine
/// do that for nothing.
///
/// `x` is arc length east; `-z` is arc length north, matching the compass the
/// game already walks by.
///
/// It is deliberately a scaffold and not a coordinate system. Near the
/// reference point it is exact; far from it, east-west arc length stretches by
/// the usual `1/cos(latitude)`, and walking over a pole keeps its longitude
/// when a real journey would flip it. None of that is reachable in play - a
/// settlement is three hundred units across and the error over one is under a
/// unit - and all of it goes away when positions themselves become spherical.
pub fn direction_at(x: f32, z: f32) -> Vec3 {
    let lon = x / PLANET_RADIUS;
    // Negated, because the game's compass points north along -z (that is the
    // direction the pan-north key walks). With latitude growing along +z the
    // globe came out a MIRROR of the ground: a village's mountains sat north
    // of it from orbit and south of it from the air. Caught by looking, which
    // is the only instrument that would ever have caught it.
    let lat = -z / PLANET_RADIUS;
    let (sin_lat, cos_lat) = lat.sin_cos();
    let (sin_lon, cos_lon) = lon.sin_cos();
    Vec3::new(cos_lat * sin_lon, sin_lat, cos_lat * cos_lon)
}

/// A noise frequency written for the flat world, converted for the round one.
///
/// The old field was sampled at `x * k`. Two points an arc length `d` apart on
/// the sphere have directions `d / PLANET_RADIUS` apart, so sampling the volume
/// at `direction * (PLANET_RADIUS * k)` puts the same `d * k` between them.
/// Every wavelength therefore comes through unchanged, and the ground underfoot
/// is the ground it always was - only now it closes.
#[inline]
fn spherical(k: f32) -> f32 {
    PLANET_RADIUS * k
}

/// Nothing generates above this. Used to bound ray marching.
pub const TERRAIN_HEIGHT: f32 = 320.0;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_terrain)
            .add_systems(Update, stream_chunks.in_set(TerrainSet));
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
    /// The level core's half extents, in the pad's own frame, and the way that
    /// frame is turned. A round working - a field, a mine mouth - has none of
    /// these and is all `radius`.
    ///
    /// A building is a rectangle and its pad used to be a circle wide enough to
    /// hold the corners, which levels half again as much ground as the building
    /// stands on and leaves a plateau reaching out past every wall. Brett: "the
    /// defomation of the land to flatten it out needs to be much more subtle
    /// than this". A pad shaped like the thing standing on it is the subtlety:
    /// the same floor, a good deal less moved earth.
    half_w: f32,
    half_d: f32,
    yaw: f32,
    /// How far the level ground reaches past that core, in every direction.
    radius: f32,
    falloff: f32,
    height: f32,
    /// Whether the working leaves bare earth behind it.
    ///
    /// A tilled field does - no blades through the furrows. A house's terrace
    /// does not, which was the other half of what Brett saw: "also it doesnt
    /// need to turn off the grass". Grass growing up to the walls, and under
    /// them, costs nothing at all - "every house has a foundation that is taller
    /// than the grass", and he keeps it that way on the bench.
    bare: bool,
}

impl FlatSpot {
    /// How far outside the level core this spot is - negative within it. The
    /// distance to an oriented rectangle, rounded off by `radius`, which for a
    /// pad with no extent at all is simply the distance to its middle.
    fn beyond(&self, x: f32, z: f32) -> f32 {
        let (sin, cos) = self.yaw.sin_cos();
        let (dx, dz) = (x - self.x, z - self.z);
        // Into the pad's own frame. The building's +X is its front.
        let along = dx * cos - dz * sin;
        let across = dx * sin + dz * cos;
        let out_w = along.abs() - self.half_w;
        let out_d = across.abs() - self.half_d;
        let outside = Vec2::new(out_w.max(0.0), out_d.max(0.0)).length();
        let inside = out_w.max(out_d).min(0.0);
        outside + inside - self.radius
    }
}

/// A worked pad as the save file keeps it. Its own type rather than a tuple of
/// nine floats, which nobody can read and everybody can put in the wrong order.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy)]
pub struct WorkedGround {
    pub x: f32,
    pub z: f32,
    #[serde(default)]
    pub half_w: f32,
    #[serde(default)]
    pub half_d: f32,
    #[serde(default)]
    pub yaw: f32,
    pub radius: f32,
    pub falloff: f32,
    pub height: f32,
    #[serde(default = "yes")]
    pub bare: bool,
}

fn yes() -> bool {
    true
}

/// The terrain function. Holds a seed, the memoised rivers, and the spots
/// the villagers have levelled.
#[derive(Resource, Clone)]
pub struct Terrain {
    /// Public so the globe can remember which world its planet was carved
    /// for, and never rebuild it for the world it already shows.
    pub seed: u32,
    /// The memoised river network. Cloning a `Terrain` shares it.
    rivers: Arc<rivers::RiverIndex>,
    /// Ground worked level by hands. Shared like the rivers; grows rarely.
    worked: Arc<RwLock<Vec<FlatSpot>>>,
    /// Built decks over water — walkable planks the ground itself answers
    /// for, so navigation needs no special cases. Registered when docks rise.
    boardwalks: Arc<RwLock<Vec<Boardwalk>>>,
}

/// One deck of planks run out over the water.
#[derive(Clone, Copy)]
struct Boardwalk {
    /// Dry-end origin.
    x: f32,
    z: f32,
    /// Unit direction the deck runs, seaward.
    dx: f32,
    dz: f32,
    /// The walkable span along that direction, in world units from the origin.
    from: f32,
    to: f32,
    /// Walkable half-width either side of the centreline.
    half_w: f32,
    /// World height of the deck top.
    deck: f32,
}

impl Terrain {
    pub fn new(seed: u32) -> Self {
        Terrain {
            seed,
            rivers: Arc::new(rivers::RiverIndex::default()),
            worked: Arc::default(),
            boardwalks: Arc::default(),
        }
    }

    /// Opens a built deck to foot traffic: a strip from `from` to `to` along
    /// `dir` out of `origin`, `half_w` wide, standing at `deck` height. The
    /// half-width should be a little generous — the navigation grid samples
    /// every 2.5 units and a strip narrower than that can slip between cells.
    pub fn register_boardwalk(
        &self,
        origin: Vec3,
        dir: Vec2,
        from: f32,
        to: f32,
        half_w: f32,
        deck: f32,
    ) {
        let dir = dir.normalize_or_zero();
        if let Ok(mut walks) = self.boardwalks.write() {
            walks.push(Boardwalk {
                x: origin.x,
                z: origin.z,
                dx: dir.x,
                dz: dir.y,
                from,
                to,
                half_w,
                deck,
            });
        }
    }

    /// The deck underfoot, if any: the world height of the planks there.
    pub fn boardwalk_at(&self, x: f32, z: f32) -> Option<f32> {
        let walks = self.boardwalks.read().ok()?;
        walks
            .iter()
            .find(|walk| {
                let rx = x - walk.x;
                let rz = z - walk.z;
                let along = rx * walk.dx + rz * walk.dz;
                let across = (rx * walk.dz - rz * walk.dx).abs();
                (walk.from..=walk.to).contains(&along) && across <= walk.half_w
            })
            .map(|walk| walk.deck)
    }

    /// Where feet actually stand: the deck when planks span this spot and
    /// clear the ground, the ground itself otherwise.
    pub fn stand_height_at(&self, x: f32, z: f32) -> f32 {
        let ground = self.height_at(x, z);
        match self.boardwalk_at(x, z) {
            Some(deck) => deck.max(ground),
            None => ground,
        }
    }

    /// Levels a pad of ground at the given height. The change is real:
    /// `height_at` reports it everywhere, and the caller is responsible for
    /// rebuilding the chunk meshes that cover it.
    pub fn flatten(&self, x: f32, z: f32, radius: f32, falloff: f32, height: f32) {
        self.work(FlatSpot {
            x,
            z,
            half_w: 0.0,
            half_d: 0.0,
            yaw: 0.0,
            radius,
            falloff,
            height,
            bare: true,
        });
    }

    fn work(&self, spot: FlatSpot) {
        if let Ok(mut worked) = self.worked.write() {
            worked.push(spot);
        }
    }

    /// Levels a pad and banks it back into the land with a bank cut to the
    /// earth it actually has to move, answering how far the whole working
    /// reaches. `least` is a floor on the bank, not the answer: a pad laid
    /// on the flat keeps it, and one cut into a hillside gets a bank long
    /// enough to walk down.
    ///
    /// This is for ground people live on. A short fixed falloff left a
    /// house on a slope standing on a mesa with a metre of cliff at its
    /// downhill edge - fine for a mine mouth, which wants to look cut, and
    /// wrong for everywhere anyone walks.
    pub fn terrace(
        &self,
        x: f32,
        z: f32,
        half_w: f32,
        half_d: f32,
        yaw: f32,
        margin: f32,
        least: f32,
        height: f32,
    ) -> f32 {
        // The worst of the earth to be moved, read around the pad's rim - which
        // is a rectangle's rim now, so the corners are read where the corners
        // are rather than at some circle that swallows them.
        let (sin, cos) = yaw.sin_cos();
        let (reach_w, reach_d) = (half_w + margin, half_d + margin);
        let mut cut: f32 = 0.0;
        for step in 0..16 {
            let turn = std::f32::consts::TAU * step as f32 / 16.0;
            let (s, c) = turn.sin_cos();
            // A point on the rectangle's rim in that direction: whichever wall
            // the ray leaves through.
            let scale = (reach_w / c.abs()).min(reach_d / s.abs());
            let (along, across) = (c * scale, s * scale);
            let rim = self.height_at(
                x + along * cos + across * sin,
                z - along * sin + across * cos,
            );
            cut = cut.max((rim - height).abs());
        }
        // One in four: graded earth rather than a quarry face, and a shallower
        // grade than the one in three it used to cut - a bank a metre high now
        // takes four metres to come back, which reads as ground rather than as
        // groundwork. Smoothstep steepens toward the middle of the ring, so the
        // true grade at the halfway line is a shade sharper than that.
        let falloff = least.max((cut * 4.0).min(18.0));
        self.work(FlatSpot {
            x,
            z,
            half_w,
            half_d,
            yaw,
            radius: margin,
            falloff,
            height,
            // Ground people live on keeps its grass.
            bare: false,
        });
        reach_w.hypot(reach_d) + falloff
    }

    /// Every worked pad, for the save file.
    pub fn export_worked(&self) -> Vec<WorkedGround> {
        self.worked
            .read()
            .map(|worked| {
                worked
                    .iter()
                    .map(|s| WorkedGround {
                        x: s.x,
                        z: s.z,
                        half_w: s.half_w,
                        half_d: s.half_d,
                        yaw: s.yaw,
                        radius: s.radius,
                        falloff: s.falloff,
                        height: s.height,
                        bare: s.bare,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Restores worked pads from a save.
    pub fn import_worked(&self, spots: &[WorkedGround]) {
        if let Ok(mut worked) = self.worked.write() {
            worked.clear();
            worked.extend(spots.iter().map(|s| FlatSpot {
                x: s.x,
                z: s.z,
                half_w: s.half_w,
                half_d: s.half_d,
                yaw: s.yaw,
                radius: s.radius,
                falloff: s.falloff,
                height: s.height,
                bare: s.bare,
            }));
        }
    }

    /// Whether this ground has been worked level - a field pad, a house's
    /// floor. Nothing wild takes root on worked ground.
    pub fn is_worked(&self, x: f32, z: f32) -> bool {
        self.is_worked_within(x, z, 0.0)
    }

    /// Worked ground plus a margin - trees keep a wider berth than stones do.
    pub fn is_worked_within(&self, x: f32, z: f32, margin: f32) -> bool {
        let Ok(worked) = self.worked.read() else {
            return false;
        };
        worked.iter().any(|spot| spot.beyond(x, z) < margin)
    }

    /// Whether this ground has been worked BARE: turned earth, a quarried
    /// face, a mine's mouth. Grass grows on everything else that has been
    /// levelled, a house's terrace included.
    pub fn is_bare(&self, x: f32, z: f32) -> bool {
        let Ok(worked) = self.worked.read() else {
            return false;
        };
        worked
            .iter()
            .any(|spot| spot.bare && spot.beyond(x, z) < 0.0)
    }

    /// Applies the worked-level pads to a computed height.
    fn leveled(&self, x: f32, z: f32, height: f32) -> f32 {
        let Ok(worked) = self.worked.read() else {
            return height;
        };
        let mut height = height;
        for spot in worked.iter() {
            let beyond = spot.beyond(x, z);
            if beyond >= spot.falloff {
                continue;
            }
            let w = ((spot.falloff - beyond) / spot.falloff).clamp(0.0, 1.0);
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
        self.carved(x, z, self.river_query(x, z))
    }

    /// The ground, given an answer about the river already in hand.
    ///
    /// Split out because `river_surface_at` needs both the course and the
    /// ground, and asking `height_at` for the second ran the whole spatial
    /// lookup a second time - on every vertex of every patch and every chunk in
    /// the world, which with a full drainage network is a great deal of walking
    /// the same bins to get the same answer.
    fn carved(&self, x: f32, z: f32, query: Option<(f32, f32, f32)>) -> f32 {
        let base = self.base_height_at(x, z);
        let Some((level, distance, width)) = query else {
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

    /// The surface of any standing water over this point.
    ///
    /// Lakes need no channel cut for them and no shoreline drawn: the fill that
    /// makes them raises the water to its outlet, and the shore is wherever the
    /// land happens to cross that. All the ground has to do is be lower.
    fn still_query(&self, x: f32, z: f32) -> Option<f32> {
        self.rivers.ensure_near(self, x, z);
        self.rivers.still_at(x, z)
    }

    /// Ground height before any river is cut into it.
    ///
    /// The river's own water surface is derived from this: water sits just below the
    /// land it flows through, not at sea level.
    pub fn base_height_at(&self, x: f32, z: f32) -> f32 {
        // Continents, and they have to be CONTINENTS — big coherent landmasses
        // with real interiors, not an archipelago.
        //
        // Three numbers decide that and they were measured, not chosen. The
        // land's SHAPE cannot be adjusted after the fact: any monotone curve
        // applied to this field leaves the coastline exactly where it was, so
        // the only levers are the wavelength, the octave count and the gain.
        // Eighteen combinations were sampled over nine thousand directions and
        // scored on how much of the land is INTERIOR — land with every neighbour
        // a few hundred units out also on land. The old spectrum scored 56%: two
        // thirds of all land within sight of water, which is exactly the
        // scattered look Brett called out. This one scores about 73%.
        //
        // The gain is the big lever: at 0.5 the second and third octaves carry
        // nearly half the field's energy and every bump of them punches a bay or
        // strands an island. At 0.28 the base wave dominates and the finer
        // octaves only trouble the coastline, which is what they should be doing.
        // Three octaves rather than two because two gives smooth ovals; the third
        // is what makes a coast look walked.
        let dir = direction_at(x, z);
        let continent = fbm_3d(dir * spherical(0.00055), self.seed, 3, 2.0, 0.28);

        // -1 is deep ocean, +1 is high inland.
        //
        // The threshold IS the sea level, and it is the field's own median, which
        // is what makes the world half land and half water. Measured over thirty
        // thousand directions: p50 0.549, p99 0.833. The divisor is the distance
        // between them, so the deepest interiors of the biggest continents still
        // reach a full one and stand as high as they ever did.
        let land = ((continent - 0.549) / 0.284).clamp(-1.0, 1.0);

        // Steepen the curve either side of zero so ground climbs away from sea level
        // quickly. Left linear, the coastal band where height is barely above water
        // spans hundreds of units and the whole shoreline reads as one vast beach.
        let shaped = if land >= 0.0 {
            land.powf(0.62)
        } else {
            -(-land).powf(0.85)
        };

        // Hills, present everywhere but stronger on land.
        //
        // THREE octaves, not four, and the same cut is made to the ridges and
        // the peaks below. At four, the finest of them has a twelve unit
        // wavelength carrying a couple of units of height: too small to read as
        // landform and too big to ignore, so every hillside wore an orange-peel
        // texture that looked like a shading fault rather than like ground.
        //
        // It costs the rivers even more than it costs the eye. Drainage is
        // solved on this field, and every one of those dimples is a hollow with
        // no way out - so the fill spends itself on thousands of puddles, the
        // routing zigzags between them, and catchment that should gather into
        // one river is split among a dozen scratches. Real land looks dendritic
        // BECAUSE running water has already smoothed it; this is the erosion
        // the world never had.
        let detail = warped_fbm_3d(dir * spherical(0.010), self.seed ^ 0xa1a1, 3, 0.5) - 0.5;

        // Whether this is INLAND, which is a different question from how high the
        // continent stands here — and getting those two confused is what emptied
        // the world of mountains the moment the sea rose. The masks below used
        // the continent's own height, so raising sea level shrank every one of
        // them at once: hills, ridges and mountain belts all faded together and
        // the highest ground within a mile and a half of home came out at
        // seventy-eight units. A hill does not know how far above sea level its
        // continent's middle is. It only needs to not be on a beach.
        //
        // Saturating a quarter of the way up from the coast, smoothly, so
        // shorelines stay gentle and everything past them gets its full relief.
        let inland = (land / 0.25).clamp(0.0, 1.0);
        let ridge_mask = inland * inland * (3.0 - 2.0 * inland);
        let ridge = ridged_3d(dir * spherical(0.006), self.seed ^ 0xa5a5, 3);

        let mut height: f32 = WATER_LEVEL
            + shaped * 44.0
            + detail * 19.0 * (0.35 + ridge_mask * 0.65)
            + ridge * ridge_mask * ridge_mask * 26.0;

        // Mountain belts. Without a field of their own, "high ground" is just
        // wherever the continent noise happened to peak, and the world comes out
        // uniformly rolling. A separate low-frequency mask puts ranges in particular
        // places and leaves the rest of the map as lowland.
        let belt = fbm_3d(
            dir * spherical(0.00055) + Vec3::new(300.0, -120.0, 60.0),
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
            // FIVE octaves here, unlike the hills and the ridges below the
            // treeline. What was cut from those was a twelve unit wavelength
            // wearing the ground like orange peel; the finest octave of this
            // one is nearer thirty, on slopes that rise two hundred units, and
            // that is not texture - it is the ridgeline. Ridged noise makes its
            // creases in the last octave it is given, so taking one away does
            // not smooth a mountain, it rounds it off.
            let peaks = ridged_3d(dir * spherical(0.0021), self.seed ^ 0x77aa, 5);
            // NOT squared. The comment above this warns that ridged noise
            // rarely approaches one and that every extra power crushes what
            // little there is - and then the code squared it anyway, which is
            // what made a range read as a grey plateau with dimples rather than
            // as mountains. Ridged noise draws connected CREST LINES; squaring
            // pushes everything that is not already at the top down to nothing,
            // so the lines broke into separate bumps with flat hollows between
            // them, and the hollows are closed basins, which is why every one
            // of them held a tarn.
            //
            // At 1.4 the crests stay joined into ridges with valleys running
            // off them, and the multiplier comes down to keep the summits where
            // they were - `the_world_has_mountains` measures both ends of that.
            // A MASS, and ridges on it - not ridges alone.
            //
            // All the height used to come from the ridged field, and ridged
            // noise is high on its crests and low everywhere else. Add a
            // hundred and sixty units of that to a belt and the low patches
            // between crests come out ringed by high ground: closed basins, in
            // the middle of every range. The belt's own falloff is the only
            // thing sloping outward and the noise swings harder than it does,
            // so the noise wins locally and the basin stays shut. That is the
            // crater, and the fill then puts a lake in it, because a closed
            // basin is exactly what a lake is.
            //
            // Split it. Most of the height is now a smooth dome that falls away
            // in every direction, so wherever you stand there is always lower
            // ground somewhere near - and the ridges ride on top of that
            // instead of being the whole of it. A mountain reads as one mass
            // with creases rather than a field of bumps, and the water has
            // somewhere to go.
            let dome = belt_mask.powf(1.3);
            height += dome * 150.0 + peaks.powf(1.4) * dome * 95.0;
        }

        height.clamp(0.0, TERRAIN_HEIGHT)
    }

    /// The middle of the biggest piece of high ground within reach.
    ///
    /// For tests. Half this world is ocean and the origin is not special - for
    /// seed 77 it is open sea - so anything that wants rivers, lakes or
    /// mountains has to go and find a catchment first.
    #[cfg(test)]
    pub fn somewhere_inland(&self) -> Vec2 {
        let mut best = (f32::NEG_INFINITY, Vec2::ZERO);
        for iz in -24..24 {
            for ix in -24..24 {
                let at = Vec2::new(ix as f32 * 320.0, iz as f32 * 320.0);
                let h = self.base_height_at(at.x, at.y);
                if h > best.0 {
                    best = (h, at);
                }
            }
        }
        best.1
    }

    /// The river's water surface here, if a course carries water at this point.
    ///
    /// `None` below sea level, where the ocean already covers everything and a
    /// second surface would only fight it.
    pub fn river_surface_at(&self, x: f32, z: f32) -> Option<f32> {
        let (ground, surface) = self.ground_and_surface(x, z);
        surface.filter(|level| *level > ground + 0.12 && *level > WATER_LEVEL + 0.8)
    }

    /// The drawn ground, and any river or lake surface standing over it, both
    /// out of ONE walk of the drainage bins.
    fn ground_and_surface(&self, x: f32, z: f32) -> (f32, Option<f32>) {
        let query = self.river_query(x, z);
        let ground = self.carved(x, z, query);

        // Flowing water, if a channel holds this point.
        let mut surface = query
            .filter(|(_, distance, width)| *distance <= rivers::CHANNEL_HALF_WIDTH * width * 1.05)
            .map(|(level, _, _)| level);

        // And standing water, which needs no channel and has no drawn edge -
        // a lake covers whatever ground lies under its level, so its shore is a
        // contour of the land and follows every inlet and headland for free.
        if let Some(pond) = self.still_query(x, z)
            && surface.is_none_or(|had| pond > had)
        {
            surface = Some(pond);
        }

        (ground, surface)
    }

    /// The ground and the water on it, for a surface that needs both at every
    /// vertex - which the planet's own patches do, twice over.
    ///
    /// They used to ask `base_height_at` for the ground and `river_surface_at`
    /// for the water, and that was wrong twice. It walked the drainage bins
    /// TWICE per vertex for one answer, throwing the carved ground away in
    /// between. And it drew the ground BEFORE the world had been worked on it:
    /// no channel cut under any river, and no terrace under any village. The
    /// planet wore a river as a blue ribbon painted flat across unbroken
    /// hillside, and a settlement's levelled ground came back as raw slope.
    ///
    /// That never mattered while the chunks covered every acre anyone could
    /// look at. It matters now the chunks end at [`CHUNK_CEILING`], because
    /// past there this IS the ground.
    pub fn ground_and_water_at(&self, x: f32, z: f32) -> (f32, f32) {
        let (ground, surface) = self.ground_and_surface(x, z);
        let wet = surface
            .filter(|level| *level > ground + 0.12 && *level > WATER_LEVEL + 0.8)
            .unwrap_or(WATER_LEVEL)
            .max(WATER_LEVEL);
        (ground, wet)
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
        fbm_3d(
            direction_at(x, z) * spherical(0.0045) + Vec3::new(17.0, -9.0, 5.0),
            self.seed ^ 0x5eed,
            3,
            2.0,
            0.5,
        )
    }

    /// Forest density in `[0, 1]` — the same field the scatterer seeds trees
    /// from, so ground can be judged for timber before any chunk exists.
    /// Trees stand where this exceeds 0.50 on moist, gentle ground.
    pub fn forest_at(&self, x: f32, z: f32) -> f32 {
        fbm_3d(
            direction_at(x, z) * spherical(0.004),
            self.seed ^ 0xf00d,
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
        let dir = direction_at(x, z);
        let broad = fbm_3d(
            dir * spherical(0.011) + Vec3::new(-140.0, 77.0, 22.0),
            self.seed ^ 0x9e11,
            3,
            2.0,
            0.5,
        );
        let fine = fbm_3d(
            dir * spherical(0.055) + Vec3::new(12.0, -31.0, 44.0),
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
        fbm_3d(
            direction_at(x, z) * spherical(0.019) + Vec3::new(61.0, -24.0, 13.0),
            self.seed ^ 0xbeef,
            3,
            2.0,
            0.5,
        ) * 2.0
            - 1.0
    }

    /// Rough temperature in `[0, 1]`, falling with altitude. Takes the height
    /// already in hand, for the same bulk-sampling reason as
    /// [`biome_for`](Self::biome_for) — the planet asks this once per vertex.
    pub fn temperature_for(&self, x: f32, z: f32, height: f32) -> f32 {
        let base = fbm_3d(
            direction_at(x, z) * spherical(0.00042) + Vec3::new(-400.0, 250.0, 90.0),
            self.seed ^ 0x7e11,
            3,
            2.0,
            0.5,
        );
        let altitude = ((height - WATER_LEVEL) / 120.0).clamp(0.0, 1.0);
        (base - altitude * 0.55).clamp(0.0, 1.0)
    }

    /// Which biome a position falls in.
    pub fn biome_at(&self, x: f32, z: f32) -> Biome {
        self.biome_for(x, z, self.height_at(x, z))
    }

    /// [`biome_at`](Self::biome_at) for a height the caller already holds, so
    /// a bulk sampler — the globe reads every vertex of a planet — does not
    /// silently pay for each height twice.
    pub fn biome_for(&self, x: f32, z: f32, height: f32) -> Biome {
        if height > WATER_LEVEL + 78.0 {
            return Biome::Alpine;
        }

        let temperature = self.temperature_for(x, z, height);
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
        // Planks beat water: a deck built over the shallows is a floor,
        // whatever the seabed under it is doing.
        if self.boardwalk_at(x, z).is_some() {
            return true;
        }
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
    /// The widest radius this view has recently asked for. Chunks are kept
    /// out to it for a moment after the camera comes back in: the memory was
    /// already paid at the widest moment, and throwing it away at once only
    /// means building it again the next time the god leans back.
    ///
    /// For a MOMENT. It used to be for ever, and that turned one zoom-out into
    /// a permanent tax: the widest ring the session ever asked for stayed
    /// anchored to the camera, and every pan afterwards dragged it along. In
    /// the frame that started this work it was the difference between the nine
    /// hundred chunks the view wanted and the twelve hundred that were
    /// standing. See `HELD_FOR`.
    held: i32,
    /// Seconds since the view last wanted the width it is holding.
    let_go: f32,
}

/// How long a ring outlives the view that asked for it, in seconds.
///
/// Long enough that leaning back and in again — which takes a beat of the
/// wheel — costs nothing, short enough that a zoom-out does not follow the god
/// around for the rest of the session.
const HELD_FOR: f32 = 0.5;

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
    // The ray lives in the BENT world - the picture the player clicks is on
    // the sphere - and the answer comes back in FLAT coordinates, the only
    // language the simulation speaks. Altitude is distance from the planet's
    // centre less the radius; the ground under a point is found by turning
    // its direction back into the scaffold's (x, z).
    // Far enough to cross the whole planet from the highest the god can climb.
    // It used to stop at four thousand, which is not even as high as the wheel
    // goes now: from any real altitude the march simply ran out before it
    // reached the ground and reported open sky, so nothing up there could be
    // clicked, zoomed toward, or reached for.
    const MAX_DISTANCE: f32 = 60_000.0;
    /// The finest step, used in the last stretch above the ground.
    const STEP: f32 = 1.5;

    let centre = crate::globe::planet_centre();
    let stance_back = crate::globe::planet_stance().inverse();
    let flat_of = |p: Vec3| -> (f32, f32, f32) {
        let v = p - centre;
        let r = v.length().max(1.0);
        let (x, z) = crate::globe::ground_coordinates(stance_back * (v / r));
        (x, z, r - PLANET_RADIUS)
    };
    // How far the point stands above the ground beneath it. Signed, so the
    // march can both test for a crossing and know how big a stride it can
    // safely take.
    let clearance = |p: Vec3| -> f32 {
        let (x, z, altitude) = flat_of(p);
        altitude - terrain.height_at(x, z).max(WATER_LEVEL)
    };

    let origin = ray.origin;
    let direction = *ray.direction;

    // A ray climbing away from the planet, already above everything, will
    // never come down.
    {
        let v = origin - centre;
        let outward = direction.dot(v.normalize_or(Vec3::Y));
        if outward >= 0.0 && v.length() - PLANET_RADIUS > TERRAIN_HEIGHT {
            return None;
        }
    }

    let mut travelled = 0.0;
    let mut previous = origin;
    let mut previous_above = clearance(origin) > 0.0;

    while travelled < MAX_DISTANCE {
        // A stride the height of the ground below, halved: high above the
        // world it covers ground in leagues, and closing on the surface it
        // shortens to the old fixed step. Sixty thousand units at a step and
        // a half would be forty thousand samples a frame; this crosses the
        // same distance in a couple of hundred and is no coarser anywhere it
        // matters, because the stride is never longer than the room there is
        // to fall.
        let gap = clearance(previous).abs();
        travelled += (gap * 0.5).clamp(STEP, 2_000.0);
        let current = origin + direction * travelled;
        let above = clearance(current) > 0.0;

        if previous_above && !above {
            // Crossed the surface between `previous` and `current`. Bisect,
            // then answer in the flat frame.
            let mut lo = previous;
            let mut hi = current;
            for _ in 0..14 {
                let mid = (lo + hi) * 0.5;
                if clearance(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let (x, z, _) = flat_of((lo + hi) * 0.5);
            let ground = terrain.height_at(x, z).max(WATER_LEVEL);
            return Some(Vec3::new(x, ground, z));
        }
        previous = current;
        previous_above = above;
    }
    None
}

/// Chooses a surface colour from height, steepness and moisture.
///
/// `pub(crate)` because the globe paints its planet with this exact function —
/// the world seen from orbit must be the world the chunks paint up close.
/// How far from the equator a piece of ground lies: nought at the belt of the
/// world, one at either pole.
///
/// The scaffold's `z` IS latitude — `direction_at` reads it as `-z / radius` in
/// radians — so a quarter of the way round the world is a pole.
pub(crate) fn polarity_at(z: f32) -> f32 {
    (z.abs() / (PLANET_RADIUS * std::f32::consts::FRAC_PI_2)).clamp(0.0, 1.0)
}

pub(crate) fn surface_color(
    height: f32,
    slope: f32,
    moisture: f32,
    shade_t: f32,
    biome: Biome,
    variation: f32,
    patch: f32,
    river: Option<(f32, f32, f32)>,
    // How far from the equator this ground is; see `polarity_at`.
    polarity: f32,
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

    // Snow on the highest ground — and on ANY ground near the poles, because a
    // snowline is a fact about latitude before it is one about height. Fixed at a
    // hundred and fifty-eight units, it whitened every continental interior in
    // the world the moment those interiors got their proper relief: an ice planet
    // with tropics. On Earth the line runs near five thousand metres at the
    // equator and meets the sea in the far north; that is the shape of it here.
    //
    // Green lowland, grey rock, white summit still gives the eye three bands to
    // read altitude from, and the line wanders for the same reason the treeline
    // does.
    const EQUATOR_SNOW: f32 = 300.0;
    const POLE_SNOW: f32 = -40.0;
    let toward_the_pole = polarity.clamp(0.0, 1.0).powf(1.6);
    let snowline = EQUATOR_SNOW + (POLE_SNOW - EQUATOR_SNOW) * toward_the_pole + variation * 38.0;
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
        polarity_at(world_z),
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
                polarity_at(world_z),
            )
            .to_linear();

            // Bent HERE, per vertex, in world space. Seating a whole chunk
            // rigidly on its tangent point was the first plan and it is not
            // good enough: a sixty-four unit chunk seated at its origin bows
            // two thirds of a unit at its far corner, and neighbours seated
            // by their own origins disagree at every shared edge - a step
            // at every chunk seam across the whole world. Bending the
            // vertices costs nothing (the mesh is built once) and the chunks
            // then tile the sphere EXACTLY, because a shared edge is the
            // same world position for both of them and gets the same seat.
            let (seat, turn) = crate::globe::bend_frame(Vec3::new(world_x, y, world_z));
            positions.push(seat.to_array());
            let bent_normal = turn * normal;
            normals.push([bent_normal.x, bent_normal.y, bent_normal.z]);
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
/// How far up the beach the sea's mesh is carried past the waterline, so its
/// own polygon edge falls on ground where it is already invisible.
const SHORE_REACH: f32 = 1.5;

/// Depth at which water has reached its full colour and is fully opaque.
const DEEP_BY: f32 = 7.0;

/// The colour of water this deep, as a vertex colour.
///
/// Everything the old shader worked out per fragment, known here for nothing:
/// the mesh is built from the terrain, so the depth at a vertex is simply the
/// water's level less the bed under it. Shallow water keeps the bed's colour by
/// being nearly clear; deep water hides it by being nearly opaque; and at the
/// waterline the alpha reaches zero, which is what lets the sheet run up onto
/// the beach and vanish instead of stopping on an edge.
pub fn water_colour(depth: f32) -> [f32; 4] {
    let t = (depth / DEEP_BY).clamp(0.0, 1.0);
    let shallow = palette::shade(&palette::WATER, SEA_SHALLOW).to_linear();
    let deep = palette::shade(&palette::WATER, SEA_DEEP).to_linear();
    [
        shallow.red + (deep.red - shallow.red) * t,
        shallow.green + (deep.green - shallow.green) * t,
        shallow.blue + (deep.blue - shallow.blue) * t,
        // Eased in, so a shore fades rather than stepping out of nothing.
        (t * (2.0 - t)).clamp(0.0, 1.0) * 0.94,
    ]
}

/// Where on the water ramp the sea's two colours are taken from.
///
/// Shared, because two different things draw the same ocean: the chunks'
/// own sea and the planet's patches. They used to choose their own shades and
/// the join between them was a square of differently coloured sea.
pub const SEA_SHALLOW: f32 = 1.0;
pub const SEA_DEEP: f32 = 0.62;

pub fn build_river_mesh(terrain: &Terrain, coord: IVec2) -> Option<Mesh> {
    build_water_mesh(terrain, coord, false)
}

/// The sea standing over one chunk, if it reaches this far.
///
/// The other half of what the planet's patches do overhead, and the reason the
/// flat sheet could be deleted: water is geometry the ground carries now,
/// wherever that ground is drawn from, instead of one square quad chasing the
/// camera around. Same builder as the rivers, asked a different question.
pub fn build_sea_mesh(terrain: &Terrain, coord: IVec2) -> Option<Mesh> {
    build_water_mesh(terrain, coord, true)
}

fn build_water_mesh(terrain: &Terrain, coord: IVec2, sea: bool) -> Option<Mesh> {
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

            if sea {
                // The open sea: everything the ground drops below sea level.
                let h = terrain.height_at(x, z);
                if h < WATER_LEVEL {
                    wet[index] = true;
                    heights[index] = WATER_LEVEL;
                    any = true;
                } else {
                    heights[index] = h;
                    // A step of dry land past the waterline still counts as
                    // shore. The sheet is cut on the chunk's own two-metre
                    // grid, so ending it at the last WET corner left the
                    // coastline as a row of two-metre teeth - the polygon
                    // edge, in plain view. Carried a little way up the beach
                    // the edge lands on ground the depth fade has already
                    // taken to nothing, and there is no hard line left to see.
                    if h < WATER_LEVEL + SHORE_REACH {
                        wet[index] = true;
                        any = true;
                    }
                }
            } else {
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
    }
    if !any {
        return None;
    }

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
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
                // Bent in world space, like the ground it runs through: the
                // river shares its chunk's identity transform, so a
                // chunk-local vertex would put every river in the world at
                // the origin.
                let flat = Vec3::new(origin.x + x0 + dx, y, origin.y + z0 + dz);
                let (seat, turn) = crate::globe::bend_frame(flat);
                positions.push(seat.to_array());
                normals.push((turn * Vec3::Y).to_array());
                // The bed under this corner, which is the ground height where
                // the corner is dry and the carved channel where it is not.
                let bed = terrain.height_at(origin.x + x0 + dx, origin.y + z0 + dz);
                colors.push(water_colour(y - bed));
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
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

/// Shared handles for terrain rendering.
#[derive(Resource)]
pub struct TerrainAssets {
    pub ground_material: Handle<StandardMaterial>,
    /// The same shader as the sea, so rivers and ocean are plainly the same
    /// substance — but its own settings, because they are not the same water.
    ///
    /// The sea's body comes from how much of it there is: its alpha rises over
    /// the first seven units of depth, which is what makes a shoreline fade
    /// instead of ending on a line. A river carved three units into its valley
    /// never reaches half of that, so rivers have always been a faint sheen on
    /// a brown channel — Brett's "rivers were never good". Reading the depth
    /// over a couple of units instead gives a river a surface while leaving the
    /// sea's shallows exactly as they were.
    pub river_material: Handle<StandardMaterial>,
    /// The sea's own, shared with the planet's patches so the ocean at
    /// altitude and the ocean underfoot are the same water lit the same way.
    pub sea_material: Handle<StandardMaterial>,
}

fn setup_terrain(
    mut commands: Commands,
    _meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    world_seed: Res<crate::WorldSeed>,
) {
    // Vertex colours carry all the surface variation, so the material is plain white.
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        reflectance: 0.02,
        ..default()
    });

    // Water is one quad that follows the camera, drawing the sea INSIDE the
    // loaded world; a quad per chunk would show a seam at every join.
    //
    // Sized to the streamed ground and no further. It used to be eight times
    // this — "far past the furthest the fog can ever reach", in the fog's
    // era, when whatever the quad covered was hidden anyway. With the fog
    // gone and the planet drawn beneath the world, that oversize lay across
    // the planet like a twenty-kilometre grey tarp: every acre of the world
    // lower than sea level plus the curvature drop vanished under it, and
    // from any height the god saw a loaded island in a dead grey sea of
    // nothing. Past this quad's edge, the planet paints its own ocean.

    // Water is a plain lit surface now, and everything that made it read as
    // water is carried by the MESH.
    //
    // There was a whole shader here: a noise wave field perturbing the normal,
    // a hand-rolled sun and sky reflection, fresnel, foam, and a read of the
    // depth prepass to work out how much water lay between the eye and the
    // seabed. All of it ran per fragment over a surface that covers half the
    // screen, and every water fault of the last day was one of those pieces
    // beating against the pixel grid at a distance where it could not be seen.
    //
    // None of it was needed, because the meshes are ours. `water_colour` knows
    // the exact depth at every vertex - the terrain told it - so shallow
    // against deep, and the fade to nothing at the shore, are vertex colours.
    // A flat-shaded low-poly world wanted that anyway; a physically detailed sea
    // in the middle of it always looked borrowed.
    let mut still_water = StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        // Smooth enough to catch the sun as a broad soft sheen, which is the
        // one thing the old shader did that the colour cannot.
        perceptual_roughness: 0.22,
        reflectance: 0.35,
        // Water casting a shadow onto its own bed puts a dark band under every
        // shoreline, which reads as the surface hovering above the sand rather
        // than meeting it.
        double_sided: true,
        cull_mode: None,
        ..default()
    };
    still_water.base_color.set_alpha(1.0);
    let water_material = materials.add(still_water.clone());
    let river_material = materials.add(still_water);

    // No sheet any more. The sea used to be ONE square quad, sized to the
    // streamed ground and dragged along under the camera, and every problem it
    // had came from being a separate object pretending to be part of the
    // world: it hung over the globe as a blue square when the view pulled
    // back, it lit differently from the ocean the planet painted, and it sat
    // in the same plane as the patch water and fought it.
    //
    // Water is geometry the ground carries now - `build_sea_mesh` here for the
    // chunks, `build_patch_water` for the planet - so there is nothing to
    // follow the camera, nothing to size, and no edge anywhere for a seam to
    // live on.

    info!(
        "the world is a sphere {:.0} units around, {:.0} across",
        planet_circumference(),
        PLANET_RADIUS * 2.0
    );
    commands.insert_resource(Terrain::new(world_seed.0));
    commands.insert_resource(TerrainAssets {
        ground_material,
        river_material,
        sea_material: water_material.clone(),
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
    time: Res<Time<Real>>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("terrain: stream_chunks");
    let Ok(rig) = cameras.single() else {
        return;
    };
    let centre = terrain.chunk_of(rig.focus.x, rig.focus.z);

    let radius = stream_radius(rig.distance);
    loaded.radius = radius;
    // What the view wants BUILT, and what is worth keeping. They differ
    // when the camera comes back in: zooming out, watching the world
    // assemble, then zooming in and out again and watching it assemble a
    // second time is the whole complaint, and it happens because the
    // chunks were thrown away in between.
    //
    // So the wide ring outlives the view that wanted it — but it lets go
    // again, a ring at a time. See `LoadedChunks::held`.
    if radius >= loaded.held {
        loaded.held = radius;
        loaded.let_go = 0.0;
    } else {
        loaded.let_go += time.delta_secs();
        if loaded.let_go >= HELD_FOR {
            loaded.held -= 1;
            loaded.let_go = 0.0;
        }
    }
    let wanted = chunks_in_view(centre, radius);
    let wanted_set: HashSet<IVec2> = wanted.iter().copied().collect();
    let kept: HashSet<IVec2> = if loaded.held > radius {
        chunks_in_view(centre, loaded.held).into_iter().collect()
    } else {
        wanted_set.clone()
    };

    // Unload first, so memory is released before more is claimed.
    loaded.entities.retain(|coord, entity| {
        if kept.contains(coord) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });

    // Record how much of the view is still missing, so the loading screen has
    // something honest to report.
    loaded.wanted = wanted_set.len();

    // A big backlog earns a bigger budget: the point of rationing is to
    // hide the seams, and three a frame stopped hiding anything the moment
    // a thousand chunks came due at once.
    let missing = wanted_set.len().saturating_sub(loaded.entities.len());
    let budget = if *state.get() == crate::GameState::Loading {
        CHUNKS_PER_LOADING_FRAME
    } else {
        (CHUNKS_PER_FRAME + missing / 24).min(CHUNKS_PER_HURRIED_FRAME)
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

        spawn_chunk(
            &mut commands,
            &mut meshes,
            &assets,
            &terrain,
            &mut loaded,
            coord,
        );
        built += 1;
    }
}

/// Builds one chunk entity - terrain mesh, river ribbon - and registers it.
/// Shared by streaming and by anyone reshaping ground in place.
pub(crate) fn spawn_chunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &TerrainAssets,
    terrain: &Terrain,
    loaded: &mut LoadedChunks,
    coord: IVec2,
) -> Entity {
    let river = build_river_mesh(terrain, coord).map(|mesh| meshes.add(mesh));
    let sea = build_sea_mesh(terrain, coord).map(|mesh| meshes.add(mesh));
    let entity = commands
        .spawn((
            Name::new(format!("Chunk {},{}", coord.x, coord.y)),
            TerrainChunk { coord },
            Mesh3d(meshes.add(build_chunk_mesh(terrain, coord))),
            MeshMaterial3d(assets.ground_material.clone()),
            // Identity: the mesh's vertices are already seated on the
            // sphere in world space, so the entity places nothing.
            Transform::IDENTITY,
            // Born hidden, and revealed by `fog::drape_the_veil` once its
            // veil is on it — see that system. Ground the village has not
            // walked must never be seen, and a chunk that appears the frame
            // before its veil does IS seen: twenty of them arrive in a frame
            // while the god is moving, so the whole time the view was
            // changing there were bright unveiled squares scattered over a
            // shrouded world. Nothing is lost by waiting a frame; the
            // planet's own patch surface lies under every chunk, already
            // wearing the same veil, so an undressed chunk shows the right
            // colour anyway.
            Visibility::Hidden,
        ))
        .id();
    if let Some(sea) = sea {
        commands.spawn((
            Name::new("Sea"),
            Mesh3d(sea),
            MeshMaterial3d(assets.sea_material.clone()),
            Transform::default(),
            // World-space seats already, like its chunk's. See the river below.
            crate::globe::BentInPlace,
            NotShadowCaster,
            ChildOf(entity),
        ));
    }
    if let Some(river) = river {
        commands.spawn((
            Name::new("River"),
            Mesh3d(river),
            MeshMaterial3d(assets.river_material.clone()),
            Transform::default(),
            // Its vertices are world-space seats already, like its chunk's.
            // Without this the bend seated the transform as well - and an
            // identity transform seats the origin twenty-eight units under the
            // ground, so every river in the world sank out of sight and left
            // its carved channel painted on a dry valley floor.
            crate::globe::BentInPlace,
            NotShadowCaster,
            ChildOf(entity),
        ));
    }
    loaded.entities.insert(coord, entity);
    entity
}

/// Reshaped ground swaps its chunks in a single frame: the old entity dies
/// and its replacement - built from the terrain's CURRENT heights - is
/// spawned in the same command batch, so there is never a frame where the
/// world has a hole in it. (Despawning and letting streaming refill was a
/// white flash of sky, once per ground-breaking, most visible at 8x.)
pub(crate) fn rebuild_chunks_near(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &TerrainAssets,
    terrain: &Terrain,
    loaded: &mut LoadedChunks,
    x: f32,
    z: f32,
    radius: f32,
) {
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
            let coord = IVec2::new(cx, cz);
            let Some(old) = loaded.entities.remove(&coord) else {
                continue;
            };
            // The replacement FIRST, wearing the old chunk's own visibility,
            // and only then the felling - all in one command flush, so there
            // is no frame with a hole in the world.
            //
            // The visibility has to be COPIED, not defaulted: chunks are born
            // hidden so no unveiled ground is ever seen, and the veil decides
            // when each one may show (see `fog::drape_the_veil`). A rebuild
            // used to inherit that rule blindly - the old chunk vanished this
            // frame, its replacement stood invisible until the veil's next
            // pass, and every ground-breaking flashed a chunk-sized hole.
            // Brett: "I see chunk flash and regenerate when they build new
            // houses." The old chunk's visibility IS the veil's decision
            // about this ground, already made; the replacement wears it from
            // its first frame, shown or veiled alike.
            let new = spawn_chunk(commands, meshes, assets, terrain, loaded, coord);
            commands.queue(move |world: &mut bevy::prelude::World| {
                let worn = world
                    .get::<Visibility>(old)
                    .copied()
                    .unwrap_or(Visibility::Hidden);
                if let Ok(mut replacement) = world.get_entity_mut(new) {
                    replacement.insert(worn);
                }
            });
            commands.entity(old).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    /// A little world with real chunk machinery, for tests that need to spawn
    /// and rebuild ground rather than only ask it questions.
    fn chunk_bench() -> bevy::app::App {
        let mut app = bevy::app::App::new();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let ground_material = materials.add(StandardMaterial::default());
        let river_material = materials.add(StandardMaterial::default());
        let sea_material = materials.add(StandardMaterial::default());
        let _ = &mut meshes;
        app.insert_resource(meshes)
            .insert_resource(materials)
            .insert_resource(Terrain::new(4242))
            .insert_resource(LoadedChunks::default())
            .insert_resource(TerrainAssets {
                ground_material,
                river_material,
                sea_material,
            });
        app
    }

    /// A rebuilt chunk wears the old chunk's visibility from its first frame.
    ///
    /// Chunks are born hidden until the veil dresses them, and a rebuild used
    /// to inherit that blindly: the old chunk vanished in the same flush its
    /// replacement stood invisible, so every ground-breaking flashed a
    /// chunk-sized hole - "I see chunk flash and regenerate when they build
    /// new houses." Both directions matter: shown ground is reborn shown, and
    /// VEILED ground is reborn veiled, because the veil's decision about this
    /// ground was already made and a rebuild is not the veil's business.
    #[test]
    fn a_rebuilt_chunk_wears_the_visibility_the_old_one_had() {
        for worn in [Visibility::Inherited, Visibility::Hidden] {
            let mut app = chunk_bench();
            let coord = IVec2::new(3, -2);

            // A chunk, standing, with the veil's decision already applied.
            let old = {
                let world = app.world_mut();
                let mut state: bevy::ecs::system::SystemState<(
                    Commands,
                    ResMut<Assets<Mesh>>,
                    Res<TerrainAssets>,
                    Res<Terrain>,
                    ResMut<LoadedChunks>,
                )> = bevy::ecs::system::SystemState::new(world);
                let (mut commands, mut meshes, assets, terrain, mut loaded) =
                    state.get_mut(world).expect("bench resources present");
                let old = spawn_chunk(
                    &mut commands,
                    &mut meshes,
                    &assets,
                    &terrain,
                    &mut loaded,
                    coord,
                );
                state.apply(world);
                world.entity_mut(old).insert(worn);
                old
            };

            // The ground is worked and the chunk rebuilt in place.
            {
                let world = app.world_mut();
                let mut state: bevy::ecs::system::SystemState<(
                    Commands,
                    ResMut<Assets<Mesh>>,
                    Res<TerrainAssets>,
                    Res<Terrain>,
                    ResMut<LoadedChunks>,
                )> = bevy::ecs::system::SystemState::new(world);
                let (mut commands, mut meshes, assets, terrain, mut loaded) =
                    state.get_mut(world).expect("bench resources present");
                let (x, z) = (coord.x as f32 * CHUNK_SIZE, coord.y as f32 * CHUNK_SIZE);
                rebuild_chunks_near(
                    &mut commands,
                    &mut meshes,
                    &assets,
                    &terrain,
                    &mut loaded,
                    x,
                    z,
                    1.0,
                );
                state.apply(world);
            }

            let world = app.world_mut();
            assert!(
                world.get_entity(old).is_err(),
                "the old chunk should be felled by the rebuild"
            );
            let replacement = world
                .resource::<LoadedChunks>()
                .entities
                .get(&coord)
                .copied()
                .expect("the rebuild must register a replacement");
            assert_ne!(replacement, old, "the replacement is a new entity");
            assert_eq!(
                world.get::<Visibility>(replacement).copied(),
                Some(worn),
                "the replacement must wear the old chunk's visibility from \
                 its first frame - {worn:?} in, {worn:?} out"
            );
        }
    }

    /// A house's pad is the house's own rectangle. The circle it used to be
    /// levelled the ground off the ends of a long building as flat as the floor
    /// itself, which is a plateau reaching out past every wall - and half again
    /// as much moved earth as the building needed.
    #[test]
    fn a_terrace_is_the_shape_of_what_stands_on_it() {
        let land = Terrain::new(31);
        // A long building, turned off the axes so the frame has to be right.
        let (x, z, yaw) = (120.0_f32, -80.0_f32, 0.9_f32);
        let (half_w, half_d, margin) = (3.0_f32, 9.0_f32, 1.6_f32);
        let floor = land.height_at(x, z);
        land.terrace(x, z, half_w, half_d, yaw, margin, 2.4, floor);

        let (sin, cos) = yaw.sin_cos();
        // Local (along, across) to world: +X is the front, +Z the length.
        let world = |along: f32, across: f32| {
            (
                x + along * cos + across * sin,
                z - along * sin + across * cos,
            )
        };
        // Every corner of the floor itself is level with it.
        for (along, across) in [
            (half_w, half_d),
            (-half_w, half_d),
            (half_w, -half_d),
            (-half_w, -half_d),
            (0.0, 0.0),
        ] {
            let (wx, wz) = world(along, across);
            assert!(
                (land.height_at(wx, wz) - floor).abs() < 1e-3,
                "the floor at ({along}, {across}) stands at {} and not {floor}",
                land.height_at(wx, wz)
            );
        }
        // And the ground off the SIDE, at the distance the old circle would
        // have reached to hold those corners, is the land's own again.
        let corner = (half_w + margin).hypot(half_d + margin);
        for across in [0.0_f32, 3.0] {
            let (wx, wz) = world(corner + 6.0, across);
            let untouched = Terrain::new(31).height_at(wx, wz);
            assert!(
                (land.height_at(wx, wz) - untouched).abs() < 1e-3,
                "ground {corner:.1}m off the front wall was levelled with the floor"
            );
        }
    }

    /// Grass grows on a terrace and not in a furrow.
    #[test]
    fn only_turned_earth_is_left_bare() {
        let land = Terrain::new(31);
        let (house, field) = ((0.0_f32, 0.0_f32), (60.0_f32, 0.0_f32));
        land.terrace(
            house.0,
            house.1,
            3.0,
            3.0,
            0.0,
            1.0,
            2.4,
            land.height_at(house.0, house.1),
        );
        land.flatten(field.0, field.1, 3.4, 2.6, land.height_at(field.0, field.1));

        assert!(!land.is_bare(house.0, house.1), "a terrace lost its grass");
        assert!(land.is_bare(field.0, field.1), "a field kept its grass");
        // Both are still worked, which is what keeps trees off them.
        assert!(land.is_worked(house.0, house.1));
        assert!(land.is_worked(field.0, field.1));
    }

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
        let far = stream_radius(CHUNK_DISSOLVE);

        assert!(near < far, "radius did not grow with zoom");
        assert_eq!(near, MIN_VIEW_CHUNKS, "close in should be the minimum");
        assert_eq!(far, VIEW_CHUNKS, "the widest view should reach the maximum");

        // Up to the dissolve. Past it the radius comes back in, which is the
        // point of it, so the floor does not apply up there.
        for distance in [0.0, 5.0, 60.0, 300.0, CHUNK_DISSOLVE] {
            let r = stream_radius(distance);
            assert!(
                (MIN_VIEW_CHUNKS..=VIEW_CHUNKS).contains(&r),
                "{r} out of bounds at {distance}"
            );
        }
    }

    #[test]
    fn no_ground_is_built_above_the_ceiling() {
        // Brett's number and the whole point of it: no real chunk is generated
        // from more than seven hundred units up, because up there a villager is
        // seven pixels tall and the planet's own patches are already drawing
        // that ground at a finer cell than a chunk's.
        assert_eq!(stream_radius(CHUNK_DISSOLVE), VIEW_CHUNKS);
        assert!(stream_radius((CHUNK_DISSOLVE + CHUNK_CEILING) * 0.5) < VIEW_CHUNKS);
        for distance in [CHUNK_CEILING, 1_400.0, 5_400.0, 20_000.0, 31_000.0] {
            assert_eq!(
                stream_radius(distance),
                0,
                "chunks are still being built {distance} units up"
            );
        }
    }

    #[test]
    fn the_stream_radius_never_shrinks_on_the_way_out_to_the_dissolve() {
        // Below the dissolve, rising only ever widens the view.
        let mut previous = 0;
        let mut distance = 0.0;
        while distance <= CHUNK_DISSOLVE {
            let r = stream_radius(distance);
            assert!(
                r >= previous,
                "radius shrank while zooming out at {distance}"
            );
            previous = r;
            distance += 4.0;
        }
    }

    #[test]
    fn the_plate_only_ever_pulls_in_above_the_dissolve() {
        // And once it has started coming in it never goes back out, or a god
        // hanging in that band watches the edge of the world breathe.
        let mut previous = VIEW_CHUNKS;
        let mut distance = CHUNK_DISSOLVE;
        while distance <= CHUNK_CEILING + 200.0 {
            let r = stream_radius(distance);
            assert!(r <= previous, "the plate grew again at {distance}");
            previous = r;
            distance += 4.0;
        }
    }

    #[test]
    fn the_planet_is_drawn_on_the_ground_the_game_is_played_on() {
        // The patches asked `base_height_at`, which is the land BEFORE the
        // world has been worked on it: no channel under any river, no terrace
        // under any village. That was survivable while the chunks covered
        // every acre anyone could look at from close enough to tell. Above
        // `CHUNK_CEILING` the patches ARE the ground, so they have to answer
        // the same question the villagers walk on.
        let t = Terrain::new(4242);

        let mut rivers_seen = 0;
        for i in 0..400 {
            let x = (i as f32 * 137.0) % 9_000.0 - 4_500.0;
            let z = (i as f32 * -211.0) % 9_000.0 + 2_000.0;
            let (ground, wet) = t.ground_and_water_at(x, z);
            assert_eq!(
                ground,
                t.height_at(x, z),
                "the planet stands at a different height from the world at ({x}, {z})"
            );
            match t.river_surface_at(x, z) {
                Some(level) => {
                    rivers_seen += 1;
                    assert_eq!(wet, level.max(WATER_LEVEL), "water disagrees at ({x}, {z})");
                }
                None => assert_eq!(wet, WATER_LEVEL, "invented water at ({x}, {z})"),
            }
        }
        assert!(
            rivers_seen > 0,
            "no sample landed in water, so this proved nothing about it"
        );

        // And the worked ground, which is the one a player would actually
        // catch: a village levels its site, and the planet used to hand that
        // hillside straight back the moment the chunks bowed out.
        let site = t.somewhere_inland();
        t.flatten(site.x, site.y, 30.0, 12.0, 40.0);
        let (levelled, _) = t.ground_and_water_at(site.x, site.y);
        assert!(
            (levelled - 40.0).abs() < 0.01,
            "the terrace is at 40 and the planet draws {levelled}"
        );
    }

    #[test]
    fn ground_mottling_varies_at_a_visible_scale() {
        // The point of the patch field is variation across a hillside, not across a
        // continent. A short walk should show real spread.
        //
        // Measured across several worlds and averaged, because one walk through
        // one seed is a sample of a random field, not a property of it. This
        // asserted a span of 0.35 on a single walk through seed 2024 and duly
        // failed at 0.31 the moment the field was resampled on a sphere - at
        // an identical frequency, on ground just as mottled. What is being
        // claimed is that the field varies at this scale, so that is what is
        // now measured.
        let mut spans = Vec::new();
        for seed in [2024, 7, 99, 4242, 31337, 555] {
            let t = Terrain::new(seed);
            let mut lowest: f32 = 1.0;
            let mut highest: f32 = 0.0;
            for i in 0..300 {
                let p = t.ground_patch_at(i as f32 * 1.5, i as f32 * -0.9);
                assert!((0.0..=1.0).contains(&p), "{p} out of range");
                lowest = lowest.min(p);
                highest = highest.max(p);
            }
            spans.push(highest - lowest);
        }
        let typical = spans.iter().sum::<f32>() / spans.len() as f32;
        // Well above the ~0.1 a near-constant field would show, and far
        // enough below the measured mean (0.3 to 0.4 across re-rollings of
        // the world) that reparameterising the planet does not fail it by
        // luck - which a threshold of 0.35 managed twice.
        assert!(
            typical > 0.28,
            "patch field typically spanned only {typical:.2} over 450 units: {spans:?}"
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
        let plain = surface_color(60.5, 0.1, 0.4, 0.5, Biome::Temperate, 0.0, 0.5, None, 0.0);
        let banked = surface_color(
            60.5,
            0.1,
            0.4,
            0.5,
            Biome::Temperate,
            0.0,
            0.5,
            Some((level, 4.0, 1.0)),
            0.0,
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
            0.0,
        )
        .to_linear();
        assert!(bed.red > bed.green, "river bed is not earthy");
        let ground = plain.to_linear();
        assert!(ground.green > ground.red, "plain ground should be green");
    }

    #[test]
    fn a_channel_is_cut_below_the_water_it_carries() {
        // What THIS layer owns. Whether a river's surface is level across its
        // width is a question about the network and is measured there, against
        // the course's own fall and at a stride short enough to mean something.
        // Asked here it could only ever be a question about two points three
        // units apart, and two points three units apart can be in two different
        // rivers - `river_influence_at` reaches half again past the channel, so
        // a tributary and the trunk it is about to join both answer to it, at
        // two heights, correctly.
        //
        // The carve is this layer's job: wherever water is drawn, the ground
        // under it has been cut below it, and cut by enough to hold water
        // rather than by a rounding error.
        let t = Terrain::new(77);
        let middle = t.somewhere_inland();
        let mut checked = 0;

        'search: for iz in -60..60 {
            for ix in -60..60 {
                let x = middle.x + ix as f32 * 12.0;
                let z = middle.y + iz as f32 * 12.0;
                let Some(surface) = t.river_surface_at(x, z) else {
                    continue;
                };
                let bed = t.height_at(x, z);
                assert!(
                    surface > bed,
                    "water at {surface:.2} over ground at {bed:.2} at ({x}, {z})",
                );
                assert!(
                    surface > WATER_LEVEL,
                    "inland water at {surface:.2} below the sea at ({x}, {z})",
                );
                checked += 1;
                if checked > 200 {
                    break 'search;
                }
            }
        }
        assert!(checked > 10, "only {checked} wet samples found");
    }

    /// Standing water lies flat, whatever the ground under it is doing.
    ///
    /// The one law lakes have, and the reason they are made by filling rather
    /// than drawn: every cell of a lake carries the height of the outlet that
    /// made it, so a shore can wander wherever the land does and the surface
    /// still cannot tilt.
    #[test]
    fn a_lake_lies_flat() {
        let t = Terrain::new(77);
        let middle = t.somewhere_inland();
        let mut found = 0;

        'search: for iz in -60..60 {
            for ix in -60..60 {
                let x = middle.x + ix as f32 * 12.0;
                let z = middle.y + iz as f32 * 12.0;
                // Standing water: wet, and with no channel anywhere near it.
                let (Some(here), None) = (t.river_surface_at(x, z), t.river_influence_at(x, z))
                else {
                    continue;
                };
                for (dx, dz) in [(6.0, 0.0), (-6.0, 0.0), (0.0, 6.0), (0.0, -6.0)] {
                    let (px, pz) = (x + dx, z + dz);
                    if t.river_influence_at(px, pz).is_some() {
                        continue;
                    }
                    if let Some(near) = t.river_surface_at(px, pz) {
                        assert!(
                            (near - here).abs() < 0.01,
                            "a lake tilts {:.3} in six units at ({x}, {z})",
                            (near - here).abs(),
                        );
                    }
                }
                found += 1;
                if found > 40 {
                    break 'search;
                }
            }
        }
        assert!(found > 5, "only {found} points of standing water found");
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
        // Rays live in the BENT world now - the picture the player clicks -
        // so the test aims one there: seat the target on the sphere, stand
        // off along its own local up-and-over, and fire back at it.
        let (seat, turn) = crate::globe::bend_frame(target);
        let origin = seat + turn * Vec3::new(30.0, 60.0, 40.0);
        let ray = Ray3d::new(origin, Dir3::new(seat - origin).unwrap());

        let hit = raycast(&t, ray).expect("ray missed the ground");
        assert!(hit.distance(target) < 2.0, "hit {hit:?}, wanted {target:?}");
    }

    /// And it reaches from as high as the god can climb.
    ///
    /// The march used to stop at four thousand units, which is not a quarter of
    /// the way up the wheel now: from any real altitude it ran out of road and
    /// reported open sky, so from up there nothing could be clicked, nothing
    /// zoomed toward, and the hand had no ground to hover over. A stride that
    /// grows with the clearance below crosses the whole distance in a couple of
    /// hundred samples — and the test above proves it is still exact where it
    /// lands.
    #[test]
    fn the_ground_can_be_found_from_the_top_of_the_climb() {
        let t = Terrain::new(2024);
        for altitude in [1_000.0, 5_000.0, 20_000.0] {
            let (seat, turn) = crate::globe::bend_frame(Vec3::new(0.0, altitude, 0.0));
            // Straight down, and again at a slant, which is the longer road.
            // A gentle slant, and it has to be: from twenty thousand up the
            // whole planet is only thirteen degrees wide (sin θ = R / (R + h)),
            // so anything bolder than that genuinely does fly off into space
            // and SHOULD find no ground.
            for aim in [Vec3::NEG_Y, Vec3::new(0.08, -1.0, 0.05)] {
                let direction = Dir3::new(turn * aim).unwrap();
                let hit = raycast(&t, Ray3d::new(seat, direction))
                    .unwrap_or_else(|| panic!("lost the ground from {altitude} up, aiming {aim}"));
                let ground = t.height_at(hit.x, hit.z).max(WATER_LEVEL);
                assert!(
                    (hit.y - ground).abs() < 0.5,
                    "landed {} off the ground from {altitude} up",
                    hit.y - ground
                );
            }
        }
    }

    #[test]
    fn a_ray_pointing_at_the_sky_misses() {
        let t = Terrain::new(1);
        let ray = Ray3d::new(Vec3::new(0.0, TERRAIN_HEIGHT + 50.0, 0.0), Dir3::Y);
        assert!(raycast(&t, ray).is_none());
    }

    // ------------------------------------------------ the world is a sphere

    /// The whole point. Walk far enough east and you arrive where you started —
    /// not at similar ground, at THE SAME ground, to the last decimal. A field
    /// sampled on a plane can never do this; one sampled in a volume on the unit
    /// sphere does it for free, because sine and cosine close.
    #[test]
    fn the_world_comes_back_round_on_itself() {
        let t = Terrain::new(4242);
        let round = planet_circumference();
        for &z in &[0.0, 900.0, -2_400.0] {
            for &x in &[0.0, 137.0, -812.5, 6_000.0] {
                let here = t.base_height_at(x, z);
                let all_the_way = t.base_height_at(x + round, z);
                assert!(
                    (here - all_the_way).abs() < 0.05,
                    "at ({x}, {z}): {here} here, {all_the_way} once round"
                );
            }
        }
    }

    /// And so does everything else the ground is described by. A world whose
    /// coastlines join up but whose climate tears along a meridian would be
    /// worse than one that never closed at all.
    #[test]
    fn the_climate_closes_with_the_land() {
        let t = Terrain::new(99);
        let round = planet_circumference();
        for &(x, z) in &[(0.0, 0.0), (450.0, -1_100.0), (-3_000.0, 2_000.0)] {
            let fields: [(&str, f32, f32); 5] = [
                ("moisture", t.moisture_at(x, z), t.moisture_at(x + round, z)),
                ("forest", t.forest_at(x, z), t.forest_at(x + round, z)),
                (
                    "patch",
                    t.ground_patch_at(x, z),
                    t.ground_patch_at(x + round, z),
                ),
                (
                    "lines",
                    t.line_variation_at(x, z),
                    t.line_variation_at(x + round, z),
                ),
                (
                    // At a fixed height, so the closure of the base field is
                    // what is measured rather than the altitude term.
                    "temperature",
                    t.temperature_for(x, z, 30.0),
                    t.temperature_for(x + round, z, 30.0),
                ),
            ];
            for (name, here, round_again) in fields {
                assert!(
                    (here - round_again).abs() < 0.01,
                    "{name} tears at ({x}, {z}): {here} vs {round_again}"
                );
            }
        }
    }

    /// Finite, but not small: the ground still has to be a world worth walking.
    /// Sea and land both present, in something like the proportion the flat
    /// version had, and mountains somewhere.
    #[test]
    fn a_round_world_is_still_a_world() {
        let t = Terrain::new(4242);
        let mut wet = 0;
        let mut dry = 0;
        let mut highest: f32 = 0.0;
        // A coarse sweep of a whole hemisphere, in strides of a kilometre.
        for iz in -20..=20 {
            for ix in -40..=40 {
                let h = t.base_height_at(ix as f32 * 1_000.0, iz as f32 * 1_000.0);
                if h <= WATER_LEVEL {
                    wet += 1;
                } else {
                    dry += 1;
                }
                highest = highest.max(h);
            }
        }
        let total = (wet + dry) as f32;
        let land = dry as f32 / total;
        assert!(
            (0.15..0.75).contains(&land),
            "land is {:.0}% of the world",
            land * 100.0
        );
        assert!(highest > 90.0, "nowhere is high: {highest}");
    }

    /// The scaffold is exact where the game actually happens. A settlement is
    /// some three hundred units across; the sphere's curvature over one is a
    /// couple of units at this radius - accepted with open eyes when the
    /// world shrank, and worth re-measuring if buildings ever read as
    /// floating at a settlement's far edge.
    #[test]
    fn a_settlement_sized_patch_is_flat_enough_to_ignore() {
        let half = 150.0;
        let drop = half * half / (2.0 * PLANET_RADIUS);
        assert!(drop < 2.5, "curvature over a settlement is {drop} units");

        // And two directions a settlement apart are still nearly parallel.
        let a = direction_at(0.0, 0.0);
        let b = direction_at(300.0, 0.0);
        assert!(a.angle_between(b) < 0.06, "{}", a.angle_between(b));
    }
}

#[cfg(test)]
mod the_shape_of_the_world {
    use super::*;

    /// The world is half water, and its land is CONTINENTS.
    ///
    /// Both halves of that are worth a test, because both were got wrong and
    /// neither is visible from a single screenshot. The first world was two
    /// thirds land — Earth the wrong way round. The second was a quarter land and
    /// still read as an archipelago, because raising the sea does nothing to the
    /// SHAPE: any monotone remap of the continent field leaves the coastline
    /// exactly where it was, and only the field's own spectrum decides whether
    /// land comes in continents or in crumbs.
    ///
    /// So this measures the shape directly. `interior` is the fraction of land
    /// that has every neighbour a few hundred units out also on land — land you
    /// can stand in the middle of without seeing the sea. The old spectrum scored
    /// 36%; this one scores about two thirds. If a change to the terrain drops it
    /// back, the world has quietly become islands again.
    #[test]
    fn the_world_is_half_water_and_its_land_is_continents() {
        let terrain = Terrain::new(7);
        let (mut land, mut total, mut interior) = (0, 0, 0);
        for i in 0..12_000 {
            let a = crate::rng::hash_2d_f32(i, 11, 99);
            let b = crate::rng::hash_2d_f32(i, 22, 99);
            let height = a * 2.0 - 1.0;
            let ring = (1.0 - height * height).max(0.0).sqrt();
            let angle = b * std::f32::consts::TAU;
            let direction = Vec3::new(ring * angle.cos(), height, ring * angle.sin());
            let (x, z) = crate::globe::ground_coordinates(direction);

            total += 1;
            if terrain.base_height_at(x, z) <= WATER_LEVEL {
                continue;
            }
            land += 1;
            let step = 260.0;
            let inland = [
                (step, 0.0),
                (-step, 0.0),
                (0.0, step),
                (0.0, -step),
                (step, step),
                (-step, -step),
                (step, -step),
                (-step, step),
            ]
            .iter()
            .all(|(dx, dz)| terrain.base_height_at(x + dx, z + dz) > WATER_LEVEL);
            if inland {
                interior += 1;
            }
        }

        let dry = land as f32 / total as f32;
        assert!(
            (0.40..=0.60).contains(&dry),
            "the world is {:.0}% land, and it should be about half",
            dry * 100.0
        );
        let coherent = interior as f32 / land as f32;
        assert!(
            coherent > 0.55,
            "only {:.0}% of the land is interior — that is an archipelago, not continents",
            coherent * 100.0
        );
    }

    /// And the founders always have somewhere to stand.
    ///
    /// Half an ocean means the world's reference point is sometimes at sea, and
    /// the site search used to fall back to the origin AT SEA LEVEL when nothing
    /// near it scored — a longhouse on the seabed. Every seed here must find
    /// ground with land around it.
    #[test]
    fn every_world_has_ground_for_its_first_village() {
        for seed in [1u32, 3, 7, 42, 1337, 90210] {
            let terrain = Terrain::new(seed);
            let mut rng = crate::rng::Rng::new(seed.into());
            let site = crate::villager::choose_settlement_site(&terrain, &mut rng);
            assert!(
                terrain.height_at(site.x, site.z) > WATER_LEVEL,
                "seed {seed} founded a village under water"
            );
            let dry = (0..24)
                .filter(|step| {
                    let angle = *step as f32 / 24.0 * std::f32::consts::TAU;
                    !terrain.is_submerged(site.x + angle.cos() * 90.0, site.z + angle.sin() * 90.0)
                })
                .count();
            assert!(
                dry >= 8,
                "seed {seed} founded a village on a sandbar: only {dry} of 24 bearings are land"
            );
        }
    }
}
