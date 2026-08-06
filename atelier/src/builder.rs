//! The Builder's bench: the game's own parts, snapped together by hand.
//!
//! Nothing here is freeform. The shelf holds walls at the game's true
//! thickness, floors and roof panels at its true proportions, props the
//! god has authored, and the widget blocks that tell the game what a
//! place *does*. You build the toys; the legos come pre-measured.

use bevy::prelude::*;
use bevy::text::FontSize;
use serde::{Deserialize, Serialize};

use crate::Bench;
use crate::look::{Fonts, Palette, theme};
use crate::stage::BuilderFurniture;

/// The pitch a roof arms at, in DEGREES, before anybody pulls on it.
///
/// The unit is in the name because it changed. This was radians until roofs
/// carried their own pitch, and two other readers went on treating it as
/// radians afterwards: a gable took the tangent of thirty RADIANS and came out
/// inverted and eleven times too tall, and a roof panel arrived in the hand
/// tilted two hundred and seventy-nine degrees. Both compiled perfectly, because
/// an angle has no type. A name is the only thing that would have stopped it.
///
/// Thirty was the bench's ONE pitch for every roof of every building - forty-five
/// stood too tall over a village of this scale and the houses read as steeples.
/// It is still the right place to start, and it is no longer the only place: a
/// roof carries its own pitch now and the gold handle at its ridge sets it, which
/// is the difference between a village that reads modern and one that does not.
/// Steep is medieval; shallow is not.
const ROOF_PITCH_DEGREES: f32 = 30.0;

/// What a roof's pitch may be pulled to, in degrees, and the step it moves in.
/// Ten is nearly flat and sixty is a steeple; two and a half is fine enough to
/// tune by eye and coarse enough that two roofs meant to match will match.
const PITCH_LEAST: f32 = 10.0;
const PITCH_MOST: f32 = 60.0;
pub const PITCH_STEP: f32 = 2.5;

/// The pitch a roof arms at has to be one a handle can reach, or a roof would
/// draw itself at an angle its own tool could not return it to. Checked when the
/// bench is BUILT rather than when it is tested, because there is no arrangement
/// of these three numbers worth compiling that fails it.
const _: () = assert!(PITCH_LEAST <= ROOF_PITCH_DEGREES && ROOF_PITCH_DEGREES <= PITCH_MOST);

/// The Atelier's own measurements - the source of truth now; the game
/// conforms to these when its buildings are replaced. A quarter-metre
/// wall on a quarter-metre grid means centrelines always land on snaps.
const WALL_THICK: f32 = 0.25;

/// How far a piece that is MEANT to butt against another is drawn past its own
/// measure, so the two lap instead of meeting exactly. A sixty-fourth at each
/// end. See the wall segment in `body_of`.
const LAP: f32 = 0.03125;
const WALL_HIGH: f32 = 2.5;

/// One piece of a part's body: offset from the part origin, size, ramp,
/// shade, how much of the world shows through it (1.0 = none), and
/// whether it is a wedge rather than a box - a triangular prism, for
/// the honest slopes a gable wants.
struct Slab(Vec3, Vec3, String, f32, f32, Shape, f32);

/// What a piece of a body is cut from.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    /// The plain box, which is most of everything.
    Box,
    /// A gable's prism: the triangle stands across the part's length.
    Wedge,
    /// A ridge cap's prism: the triangle stands ACROSS the part, which
    /// runs lengthwise under it, apex up.
    Ridge,
    /// A right-angle prism: a box with one end cut through at an angle, full
    /// height at -X and falling away to +X. What a saw leaves.
    Mitre,
    /// The same cut the other way about: full height at +X. Which hand is
    /// wanted depends on which end of a beam is being capped.
    MitreBack,
}


/// A right-angle prism: a box with one end cut clean through at an angle.
///
/// The shape a saw makes. A wedge is a GABLE's prism - two slopes meeting at a
/// peak - and there was nothing in the bench for the far commoner cut, so a beam
/// meeting a roof had to stop square and stand off it. Brett: "There has to be a
/// way to cut the end of the beam at an angle. Keeping it square wont work."
///
/// Built in a unit box like every other shape, so its angle is whatever the
/// slab's own proportions make it: a mitre one long and one high is
/// forty-five degrees, and squashing it flatter or steeper is what sizing it
/// does. The full-height end stands at -X and the cut falls away to +X.
fn mitre_mesh(mirrored: bool) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    // Mirrored, the cut faces the other way: full height at +X instead. Which
    // hand a mitre needs depends on which END of a beam it caps, so both exist -
    // a slab may lean about its own X, and a lean can turn the material to the
    // other SIDE but cannot swap which end is full.
    let mut face = |corners: &[[f32; 3]], normal: [f32; 3]| {
        let first = positions.len() as u32;
        let mut corners: Vec<[f32; 3]> = corners.to_vec();
        let mut normal = normal;
        if mirrored {
            for corner in &mut corners {
                corner[0] = -corner[0];
            }
            // Mirroring turns a face inside out; walking it the other way puts
            // it right again.
            corners.reverse();
            normal[0] = -normal[0];
        }
        for corner in &corners {
            positions.push(*corner);
            normals.push(normal);
        }
        for step in 1..(corners.len() as u32 - 1) {
            indices.extend_from_slice(&[first, first + step, first + step + 1]);
        }
    };
    // The two triangles, one at each side: full height at -X, nothing at +X.
    face(
        &[[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [-0.5, 0.5, 0.5]],
        [0.0, 0.0, 1.0],
    );
    face(
        &[[0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5]],
        [0.0, 0.0, -1.0],
    );
    // The floor.
    face(
        &[
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, -0.5, -0.5],
            [-0.5, -0.5, -0.5],
        ],
        [0.0, -1.0, 0.0],
    );
    // The square end it was cut from.
    face(
        &[
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, 0.5, -0.5],
        ],
        [-1.0, 0.0, 0.0],
    );
    // And the cut itself.
    let slant = (1.0f32 / 2.0f32.sqrt(), 1.0 / 2.0f32.sqrt());
    face(
        &[
            [0.5, -0.5, 0.5],
            [0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, 0.5, 0.5],
        ],
        [slant.0, slant.1, 0.0],
    );
    Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}

fn wedge_mesh(lengthwise: bool) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut face = |corners: &[[f32; 3]], normal: [f32; 3]| {
        let first = positions.len() as u32;
        for corner in corners {
            positions.push(*corner);
            normals.push(normal);
        }
        for step in 1..(corners.len() as u32 - 1) {
            indices.extend_from_slice(&[first, first + step, first + step + 1]);
        }
    };
    let slope = (2.0f32 / 5.0f32.sqrt(), 1.0 / 5.0f32.sqrt());
    // The two triangular faces.
    face(
        &[[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.0, 0.5, 0.5]],
        [0.0, 0.0, 1.0],
    );
    face(
        &[[0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [0.0, 0.5, -0.5]],
        [0.0, 0.0, -1.0],
    );
    // The floor.
    face(
        &[
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, -0.5, -0.5],
            [-0.5, -0.5, -0.5],
        ],
        [0.0, -1.0, 0.0],
    );
    // The two slopes.
    face(
        &[
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.0, 0.5, 0.5],
            [0.0, 0.5, -0.5],
        ],
        [-slope.0, slope.1, 0.0],
    );
    face(
        &[
            [0.5, -0.5, 0.5],
            [0.5, -0.5, -0.5],
            [0.0, 0.5, -0.5],
            [0.0, 0.5, 0.5],
        ],
        [slope.0, slope.1, 0.0],
    );
    // A ridge cap is the same prism turned a quarter: the triangle
    // stands across the part and the length runs under the apex.
    if lengthwise {
        for corner in &mut positions {
            *corner = [corner[2], corner[1], corner[0]];
        }
        for normal in &mut normals {
            *normal = [normal[2], normal[1], normal[0]];
        }
        for triangle in indices.chunks_mut(3) {
            triangle.swap(1, 2);
        }
    }
    let uvs: Vec<[f32; 2]> = positions.iter().map(|_| [0.0, 0.0]).collect();
    Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}

/// What a shelf entry stands for.
#[derive(Clone, Copy, PartialEq)]
pub enum PartKind {
    Wall(f32),
    /// A piece of wall left standing around an opening: the sides of a
    /// doorway, the header above it, the sill strip under a window.
    Seg {
        long: f32,
        high: f32,
        lift: f32,
    },
    Trim {
        long: f32,
        stone: bool,
    },
    /// The stepped triangle that closes a pitched roof's end: courses of
    /// wall narrowing to a peak at the roof's own thirty degrees.
    Gable(f32, f32),
    /// A squared timber laid along its own length: the corner post's section,
    /// on its side and as long as it is drawn — and how far each end is cut
    /// back at an angle, nought for a square end.
    ///
    /// The cut is a RUN, not an angle: how far along the beam the saw travels
    /// while crossing its full height. That is the number the roof hands over —
    /// the difference between where the top corner meets the slope and where the
    /// bottom does — and it needs no trigonometry at either end.
    Beam(f32, f32, f32),
    BeamRun,
    /// The cap that hides the seam where two slopes meet.
    Ridge(f32),
    /// A chimney stack: the number is how far its shaft reaches DOWN
    /// from where it stands, so it can be buried in a roof's slope or
    /// run all the way to the hearth below.
    Chimney(f32),
    /// A ridge pole: a round log along the spine, the older way of
    /// closing a roof.
    /// A whole gable roof: both slopes and the ridge between them, drawn
    /// once over the walls instead of lined up slope by slope.
    GableRoof(f32, f32, f32, f32),
    /// What a whole roof looks like while it is being sized: the ground
    /// it will cover, with a gold line down the way the ridge will run.
    /// It is never placed - the record beneath it names the roof.
    RoofPlan(f32, f32),
    Floor(f32, f32),
    Foundation(f32, f32),
    Roof(f32, f32),
    /// The stretch tools: anchored with one click, drawn to size, set
    /// with the next. They exist only in the hand - what they place are
    /// the plain kinds above at the drawn size.
    WallRun,
    TrimRun {
        stone: bool,
    },
    /// A wall piece drawn at a fixed height and lift: the header that
    /// spans above an opening, the sill that fills below one.
    SegRun {
        high: f32,
        lift: f32,
    },
    GableRun,
    RidgeRun,
    GableRoofRun,
    FloorRun,
    FoundationRun,
    RoofRun,
    Prop(&'static str),
    Widget(&'static str),
}

impl PartKind {
    /// The runs stretch along one axis; the rect runs stretch two.
    pub fn run_axes(&self) -> Option<u8> {
        match self {
            PartKind::WallRun
            | PartKind::TrimRun { .. }
            | PartKind::SegRun { .. }
            | PartKind::GableRun
            | PartKind::RidgeRun
            | PartKind::BeamRun => Some(1),
            PartKind::FloorRun
            | PartKind::FoundationRun
            | PartKind::RoofRun
            | PartKind::GableRoofRun => Some(2),
            _ => None,
        }
    }

    /// What a run becomes at the drawn size.
    pub fn run_made(&self, w: f32, d: f32) -> PartKind {
        match self {
            PartKind::WallRun => PartKind::Wall(w),
            PartKind::TrimRun { stone } => PartKind::Trim {
                long: w,
                stone: *stone,
            },
            PartKind::GableRun => PartKind::Gable(w, ROOF_PITCH_DEGREES),
            PartKind::RidgeRun => PartKind::Ridge(w),
            PartKind::BeamRun => PartKind::Beam(w, 0.0, 0.0),
            // A hand's breadth of overhang to begin with; the gold
            // handles pull it further without moving the gables.
            PartKind::GableRoofRun => PartKind::GableRoof(w, d, 0.25, ROOF_PITCH_DEGREES),
            PartKind::SegRun { high, lift } => PartKind::Seg {
                long: w,
                high: *high,
                lift: *lift,
            },
            PartKind::FloorRun => PartKind::Floor(w, d),
            PartKind::FoundationRun => PartKind::Foundation(w, d),
            PartKind::RoofRun => PartKind::Roof(w, d),
            other => *other,
        }
    }
}

/// A shelf entry: the name it wears, what it places, and the stage the
/// village raises it in.
pub struct CatalogEntry {
    pub label: &'static str,
    pub kind: PartKind,
    pub stage: &'static str,
}

const fn structure(label: &'static str, kind: PartKind, stage: &'static str) -> CatalogEntry {
    CatalogEntry { label, kind, stage }
}

const fn prop(label: &'static str, name: &'static str) -> CatalogEntry {
    CatalogEntry {
        label,
        kind: PartKind::Prop(name),
        stage: "furnishing",
    }
}

/// The shelf's drawers: each section opens and closes on its header.
pub const STRUCTURE: &[CatalogEntry] = &[
    structure("BEAM, STRETCH", PartKind::BeamRun, "frame"),
    structure("DOOR", PartKind::Prop("door"), "walls"),
    structure("DOOR, DOUBLE", PartKind::Prop("door-double"), "walls"),
    structure("DOORWAY", PartKind::Prop("doorway"), "walls"),
    structure("FLOOR, 2M", PartKind::Floor(2.0, 2.0), "footing"),
    structure("FLOOR, STRETCH", PartKind::FloorRun, "footing"),
    structure("FOUNDATION, 2M", PartKind::Foundation(2.0, 2.0), "footing"),
    structure("FOUNDATION, STRETCH", PartKind::FoundationRun, "footing"),
    structure("GABLE, STRETCH", PartKind::GableRun, "roof"),
    structure(
        "HEADER, STRETCH",
        PartKind::SegRun {
            high: 0.375,
            lift: 2.125,
        },
        "walls",
    ),
    structure("POLE, CORNER", PartKind::Prop("pole"), "frame"),
    structure("RIDGE, STRETCH", PartKind::RidgeRun, "roof"),
    structure("ROOF, GABLE, STRETCH", PartKind::GableRoofRun, "roof"),
    structure("ROOF, PANEL", PartKind::Roof(2.2, 2.2), "roof"),
    structure("ROOF, STRETCH", PartKind::RoofRun, "roof"),
    structure(
        "SILL, STRETCH",
        PartKind::SegRun {
            high: 0.75,
            lift: 0.0,
        },
        "walls",
    ),
    structure("STEPS, STONE", PartKind::Prop("steps"), "footing"),
    structure(
        "TRIM, STONE, STRETCH",
        PartKind::TrimRun { stone: true },
        "walls",
    ),
    structure("TRIM, STRETCH", PartKind::TrimRun { stone: false }, "walls"),
    structure("WALL, 2M", PartKind::Wall(2.0), "walls"),
    structure("WALL, STRETCH", PartKind::WallRun, "walls"),
    structure("WINDOW", PartKind::Prop("window"), "walls"),
];

pub const FURNITURE: &[CatalogEntry] = &[
    prop("BED", "bed"),
    prop("BED, DOUBLE", "bed-double"),
    prop("BENCH", "bench"),
    prop("CHAIR", "chair"),
    prop("CHEST", "chest"),
    CatalogEntry {
        label: "CHIMNEY",
        kind: PartKind::Chimney(1.75),
        stage: "roof",
    },
    prop("COUCH", "couch"),
    prop("CRADLE", "cradle"),
    prop("CUPBOARD", "cupboard"),
    prop("HEARTH", "hearth"),
    prop("SHELVES", "shelves"),
    prop("STOOL", "stool"),
    prop("TABLE", "table"),
    prop("TABLE, SIDE", "side-table"),
    prop("WARDROBE", "wardrobe"),
];

pub const DECOR: &[CatalogEntry] = &[
    prop("ANVIL", "anvil"),
    prop("BARREL", "barrel"),
    prop("BASKET", "basket"),
    prop("CRATE", "crate"),
    prop("FENCE", "fence"),
    prop("LADDER", "ladder"),
    prop("LOOM", "loom"),
    prop("MANNEQUIN", "mannequin"),
    prop("PLANTER", "planter"),
    prop("POT, COOKING", "pot"),
    prop("RUG", "rug"),
    prop("SACK", "sack"),
    prop("STAND, CANDLE", "candle"),
    prop("TROUGH", "trough"),
    prop("WOODPILE", "woodpile"),
];

pub const WIDGETS: &[(&str, &str, f32)] = &[
    // name, ramp that colours its block, shade
    ("sleep", "cloth-blue", 0.7),
    ("sit", "cloth-gold", 0.6),
    ("fire", "cloth-red", 0.7),
    ("smoke", "stone", 0.7),
    ("door", "cloth-green", 0.6),
    ("work", "cloth-purple", 0.6),
    ("store", "earth", 0.6),
    ("light", "cloth-gold", 0.95),
];

/// The boxes a part is made of, in its own local space, resting on y = 0.
fn body_of(kind: &PartKind, repaint: Option<(&str, f32)>) -> Vec<Slab> {
    let slab = |x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, ramp: &str, shade: f32| {
        Slab(
            Vec3::new(x, y, z),
            Vec3::new(sx, sy, sz),
            ramp.to_string(),
            shade,
            1.0,
            Shape::Box,
            0.0,
        )
    };
    // A wedge: the gable's own shape.
    let wedge = |x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, ramp: &str, shade: f32| {
        Slab(
            Vec3::new(x, y, z),
            Vec3::new(sx, sy, sz),
            ramp.to_string(),
            shade,
            1.0,
            Shape::Wedge,
            0.0,
        )
    };
    // A piece that leans on its own, about its length: the two slopes of
    // a whole roof, and whatever else wants an angle inside a part.
    #[allow(clippy::too_many_arguments)]
    let leaning =
        |x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, ramp: &str, shade: f32, lean: f32| {
            Slab(
                Vec3::new(x, y, z),
                Vec3::new(sx, sy, sz),
                ramp.to_string(),
                shade,
                1.0,
                Shape::Box,
                lean,
            )
        };
    // A ridge cap: the same triangle, laid along the part's length.
    let ridge = |x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, ramp: &str, shade: f32| {
        Slab(
            Vec3::new(x, y, z),
            Vec3::new(sx, sy, sz),
            ramp.to_string(),
            shade,
            1.0,
            Shape::Ridge,
            0.0,
        )
    };
    // Glass: the world shows through it.
    #[allow(unused_variables)]
    // Kept for whatever wants to be seen through next - a lantern's
    // horn pane, water in a trough.
    #[allow(unused_variables)]
    let glass = |x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, ramp: &str, shade: f32| {
        Slab(
            Vec3::new(x, y, z),
            Vec3::new(sx, sy, sz),
            ramp.to_string(),
            shade,
            0.35,
            Shape::Box,
            0.0,
        )
    };
    let mut slabs = match kind {
        PartKind::WallRun => vec![slab(
            0.0,
            WALL_HIGH * 0.5,
            0.0,
            0.25,
            WALL_HIGH,
            WALL_THICK,
            "wood",
            0.7,
        )],
        PartKind::Wall(length) => vec![slab(
            0.0,
            WALL_HIGH * 0.5,
            0.0,
            *length,
            WALL_HIGH,
            WALL_THICK,
            "wood",
            0.7,
        )],
        PartKind::SegRun { high, lift } => vec![slab(
            0.0,
            lift + high * 0.5,
            0.0,
            0.25,
            *high,
            WALL_THICK,
            "wood",
            0.7,
        )],
        PartKind::Seg { long, high, lift } => vec![slab(
            0.0,
            lift + high * 0.5,
            0.0,
            // Drawn a hair longer than it measures, and it laps at both ends.
            //
            // A segment exists to fill the wall BESIDE something - the pieces a
            // punch leaves either side of a window, a header over it, a sill
            // under it - so both its ends abut. Two boxes that meet exactly
            // share an edge each works out by its own sum, from different
            // centres, and the last bits disagree; the rasteriser then leaves a
            // hairline that neither claims and the dark behind the wall shows
            // through it. Brett photographed one running down from a window, and
            // it survives the bake because the fault is in the geometry.
            //
            // A sixty-fourth at each end: too small to see, too large for any
            // float to lose. Put HERE rather than where a punch works out its
            // leavings, because that would only mend walls punched from today
            // on - and the buildings that have the seam are the ones already
            // drawn. Seventeen of these stand in the longhouse alone.
            *long + LAP,
            *high + LAP,
            WALL_THICK,
            "wood",
            0.7,
        )],
        PartKind::Floor(w, d) => vec![slab(0.0, 0.0625, 0.0, *w, 0.125, *d, "wood", 0.5)],
        PartKind::FloorRun => vec![slab(0.0, 0.0625, 0.0, 0.25, 0.125, 0.25, "wood", 0.5)],
        PartKind::Foundation(w, d) => {
            vec![slab(0.0, 0.1875, 0.0, *w, 0.375, *d, "stone", 0.55)]
        }
        PartKind::FoundationRun => {
            vec![slab(0.0, 0.1875, 0.0, 0.25, 0.375, 0.25, "stone", 0.55)]
        }
        PartKind::Roof(w, d) => vec![slab(0.0, 0.0625, 0.0, *w, 0.125, *d, "earth", 0.4)],
        PartKind::RoofRun => vec![slab(0.0, 0.0625, 0.0, 0.25, 0.125, 0.25, "earth", 0.4)],
        PartKind::Gable(long, degrees) => {
            // One clean slope each way, at the gable's OWN pitch.
            //
            // It used to be the bench's one pitch, with a comment promising that
            // a gable "meets the roof panels exactly" - true while every roof in
            // the world stood at thirty degrees, and false from the moment a
            // roof could be pulled steeper. A steep roof on a thirty degree
            // gable is a wall with daylight over it, which is the opposite of
            // the medieval look the pitch was added for.
            //
            // Snapped to the lattice like everything else, so a gable's peak is
            // somewhere another part can meet it.
            let pitch = degrees.clamp(PITCH_LEAST, PITCH_MOST).to_radians();
            let high = ((long * 0.5 * pitch.tan()) * 16.0).round() / 16.0;
            vec![wedge(
                0.0,
                high * 0.5,
                0.0,
                *long,
                high,
                WALL_THICK,
                "wood",
                0.65,
            )]
        }
        PartKind::GableRun => vec![wedge(
            0.0, 0.0625, 0.0, 0.25, 0.125, WALL_THICK, "wood", 0.65,
        )],
        PartKind::Ridge(long) => {
            // Half a metre across, a quarter tall: the bench's own pitch
            // again, so it sits down onto two 45 degree slopes.
            vec![ridge(0.0, 0.0625, 0.0, *long, 0.125, 0.5, "earth", 0.35)]
        }
        PartKind::RidgeRun => vec![ridge(0.0, 0.0625, 0.0, 0.25, 0.125, 0.5, "earth", 0.35)],
        PartKind::GableRoof(long, span, over, degrees) => {
            // Both slopes at once, meeting over the middle: no lining up
            // two panels and hoping. The eaves rest at y=0, so the part
            // seats straight onto the wall tops.
            let pitch = degrees.clamp(PITCH_LEAST, PITCH_MOST).to_radians();
            let half = span * 0.5;
            let rise = half * pitch.tan();
            let slope = (half * half + rise * rise).sqrt();
            let thick = 0.125;
            // The slopes reach past the building on every side by the
            // overhang; the gables stay where the walls are.
            let mut sides = Vec::new();
            for way in [-1.0_f32, 1.0] {
                sides.push(leaning(
                    0.0,
                    // Down the slope as well as out along it. The panel grows
                    // by the overhang and its middle has to travel half that
                    // distance ALONG its own pitch to keep the ridge end where
                    // it was - the outward half was here and the downward half
                    // was not, so every pull on the eaves lifted the whole roof
                    // off the gable it was sitting on. Brett found it at once.
                    rise * 0.5 + thick * 0.5 - over * 0.5 * pitch.sin(),
                    way * (half * 0.5 + over * 0.5 * pitch.cos()),
                    long + over * 2.0,
                    thick,
                    slope + over,
                    "earth",
                    0.4,
                    way * pitch,
                ));
            }
            // And the two ends closed: a gable apiece, standing just
            // inside the slopes so the roof laps over them.
            let end = 0.25;
            for way in [-1.0_f32, 1.0] {
                sides.push(ridge(
                    way * (long * 0.5 - end * 0.5),
                    rise * 0.5,
                    0.0,
                    end,
                    rise,
                    span * 0.995,
                    "wood",
                    0.65,
                ));
            }
            sides
        }
        PartKind::RoofPlan(w, d) => vec![
            slab(0.0, 0.0625, 0.0, *w, 0.125, *d, "earth", 0.4),
            // The ridge line: gold down the way the ridge will run, so
            // the roof can be turned with R before it is set down.
            slab(0.0, 0.1875, 0.0, *w, 0.125, 0.25, "cloth-gold", 0.85),
            // And the two eaves it will come down to.
            slab(
                0.0,
                0.1875,
                -*d * 0.5 + 0.0625,
                *w,
                0.125,
                0.125,
                "cloth-gold",
                0.5,
            ),
            slab(
                0.0,
                0.1875,
                *d * 0.5 - 0.0625,
                *w,
                0.125,
                0.125,
                "cloth-gold",
                0.5,
            ),
        ],
        PartKind::GableRoofRun => {
            vec![leaning(
                0.0, 0.0625, 0.0, 0.25, 0.125, 0.25, "earth", 0.4, 0.0,
            )]
        }
        PartKind::Beam(long, cut_high, cut_low) => {
            // The corner post's own timber, laid over. It rests ON its origin
            // rather than straddling it, the way the post stands on its foot -
            // so a beam dropped on a wall top sits on the wall rather than half
            // inside it.
            //
            // A cut end is the square box stopping short and a prism finishing
            // the job: full height where it joins, tapering to nothing where the
            // saw came out. The two ends want opposite hands of that prism,
            // since the full face points inward at both.
            let thick = 0.375;
            let (high, low) = (cut_high.max(0.0), cut_low.max(0.0));
            let square = (long - high - low).max(0.0625);
            let middle = (low - high) * 0.5;
            let mut body = vec![slab(
                middle, 0.1875, 0.0, square, thick, thick, "wood", 0.45,
            )];
            if high > 0.0 {
                body.push(Slab(
                    Vec3::new(middle + square * 0.5 + high * 0.5, 0.1875, 0.0),
                    Vec3::new(high, thick, thick),
                    "wood".to_string(),
                    0.45,
                    1.0,
                    Shape::Mitre,
                    0.0,
                ));
            }
            if low > 0.0 {
                body.push(Slab(
                    Vec3::new(middle - square * 0.5 - low * 0.5, 0.1875, 0.0),
                    Vec3::new(low, thick, thick),
                    "wood".to_string(),
                    0.45,
                    1.0,
                    Shape::MitreBack,
                    0.0,
                ));
            }
            body
        }
        PartKind::BeamRun => {
            vec![slab(0.0, 0.1875, 0.0, 0.25, 0.375, 0.375, "wood", 0.45)]
        }
        PartKind::Trim { long, stone } => {
            let (ramp, shade) = if *stone {
                ("stone", 0.55)
            } else {
                ("wood", 0.5)
            };
            vec![slab(0.0, 0.15625, 0.0, *long, 0.3125, 0.125, ramp, shade)]
        }
        PartKind::TrimRun { stone } => {
            let (ramp, shade) = if *stone {
                ("stone", 0.55)
            } else {
                ("wood", 0.5)
            };
            vec![slab(0.0, 0.15625, 0.0, 0.25, 0.3125, 0.125, ramp, shade)]
        }
        PartKind::Prop("bed") => vec![
            // Long enough for the villager who will lie in it: the
            // sleeper is 1.75 head to heel, so the mattress is 1.875 and
            // the frame two metres. The old bed was a metre and a half,
            // and everyone's feet hung off the end of it.
            slab(0.0, 0.25, 0.0, 0.875, 0.25, 2.0, "wood", 0.55),
            slab(0.0, 0.46875, 0.0, 0.75, 0.1875, 1.875, "bone", 0.8),
            slab(0.0, 0.5625, 0.6875, 0.5, 0.125, 0.375, "bone", 0.95),
        ],
        PartKind::Prop("bed-double") => vec![
            // Room for two who each take three-quarters of a metre with
            // their arms at their sides: a mattress of one and three
            // quarters, so nobody sleeps inside their spouse.
            slab(0.0, 0.25, 0.0, 1.875, 0.25, 2.0, "wood", 0.55),
            slab(0.0, 0.46875, 0.0, 1.75, 0.1875, 1.875, "bone", 0.8),
            slab(-0.46875, 0.5625, 0.6875, 0.5625, 0.125, 0.375, "bone", 0.95),
            slab(0.40625, 0.5625, 0.6875, 0.5625, 0.125, 0.375, "bone", 0.95),
        ],
        PartKind::Prop("table") => {
            let mut parts = vec![slab(0.0, 0.75, 0.0, 1.5, 0.125, 0.875, "wood", 0.65)];
            for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                parts.push(slab(
                    sx * 0.625,
                    0.375,
                    sz * 0.3125,
                    0.125,
                    0.75,
                    0.125,
                    "wood",
                    0.5,
                ));
            }
            parts
        }
        PartKind::Prop("stool") => vec![
            // Seven units to the seat, and every edge on the lattice.
            slab(0.0, 0.40625, 0.0, 0.375, 0.0625, 0.375, "wood", 0.6),
            slab(0.0, 0.1875, 0.0, 0.25, 0.375, 0.25, "wood", 0.45),
        ],
        PartKind::Prop("hearth") => vec![
            slab(0.0, 0.40625, 0.0, 0.875, 0.8125, 0.625, "stone", 0.6),
            slab(0.0, 0.5625, 0.09375, 0.625, 0.5, 0.4375, "stone", 0.25),
        ],
        PartKind::Chimney(drop) => vec![
            // A stack of dressed stone with a capped throat, and a shaft
            // that reaches down as far as it is told: set on a roof it
            // buries itself in the slope instead of perching on it, and
            // pulled further it runs to the hearth below.
            slab(
                0.0,
                (2.0 - drop) * 0.5,
                0.0,
                0.875,
                2.0 + drop,
                0.875,
                "stone",
                0.5,
            ),
            slab(0.0, 2.0625, 0.0, 1.0, 0.125, 1.0, "stone", 0.6),
            slab(0.0, 2.25, 0.0, 0.75, 0.25, 0.75, "stone", 0.45),
            slab(0.0, 2.4375, 0.0, 0.875, 0.125, 0.875, "stone", 0.6),
        ],
        PartKind::Prop("chair") => vec![
            slab(
                -0.03125, 0.40625, -0.03125, 0.4375, 0.0625, 0.4375, "wood", 0.6,
            ),
            slab(
                0.03125, 0.1875, 0.03125, 0.3125, 0.375, 0.3125, "wood", 0.45,
            ),
            // The back, standing where a back actually meets a back.
            slab(-0.03125, 0.75, -0.21875, 0.4375, 0.75, 0.0625, "wood", 0.55),
        ],
        PartKind::Prop("bench") => vec![
            // Wide enough for the two bodies it claims to seat, and cut
            // to the same seven units as every other seat.
            slab(0.0, 0.40625, 0.0, 1.75, 0.0625, 0.375, "wood", 0.6),
            slab(-0.75, 0.1875, 0.03125, 0.125, 0.375, 0.3125, "wood", 0.45),
            slab(0.75, 0.1875, 0.03125, 0.125, 0.375, 0.3125, "wood", 0.45),
        ],
        PartKind::Prop("couch") => vec![
            // A padded settle: a timber plinth with cloth laid in it.
            // The plinth stands BACK from the front of the cushion, the
            // way an upholstered seat is built - a sitter's shins have to
            // hang somewhere, and a solid block to the front edge is the
            // one place they cannot.
            slab(0.0, 0.1875, -0.0625, 1.875, 0.375, 0.625, "wood", 0.5),
            slab(
                -0.90625, 0.3125, 0.03125, 0.1875, 0.625, 0.8125, "wood", 0.55,
            ),
            slab(
                0.96875, 0.3125, 0.03125, 0.1875, 0.625, 0.8125, "wood", 0.55,
            ),
            slab(0.0, 0.75, -0.28125, 1.875, 0.75, 0.1875, "wood", 0.55),
            // The cushions: two to sit on, two to lean against.
            slab(
                -0.46875,
                0.4375,
                0.09375,
                0.8125,
                0.125,
                0.6875,
                "cloth-wine",
                0.6,
            ),
            slab(
                0.40625,
                0.4375,
                0.09375,
                0.8125,
                0.125,
                0.6875,
                "cloth-wine",
                0.6,
            ),
            slab(
                -0.46875,
                0.71875,
                -0.1875,
                0.8125,
                0.4375,
                0.125,
                "cloth-wine",
                0.5,
            ),
            slab(
                0.40625,
                0.71875,
                -0.1875,
                0.8125,
                0.4375,
                0.125,
                "cloth-wine",
                0.5,
            ),
        ],
        PartKind::Prop("chest") => vec![
            slab(0.03125, 0.25, 0.0, 0.8125, 0.5, 0.5, "wood", 0.5),
            slab(0.03125, 0.5, 0.03125, 0.8125, 0.125, 0.5625, "wood", 0.35),
            slab(
                0.0,
                0.34375,
                0.28125,
                0.125,
                0.1875,
                0.0625,
                "cloth-gold",
                0.7,
            ),
        ],
        PartKind::Prop("barrel") => vec![
            slab(0.03125, 0.375, 0.03125, 0.5625, 0.75, 0.5625, "wood", 0.55),
            slab(
                0.03125, 0.15625, 0.03125, 0.5625, 0.0625, 0.5625, "stone", 0.45,
            ),
            slab(
                0.03125, 0.53125, 0.03125, 0.5625, 0.0625, 0.5625, "stone", 0.45,
            ),
        ],
        PartKind::Prop("crate") => vec![
            slab(0.0, 0.3125, 0.0, 0.625, 0.625, 0.625, "wood", 0.6),
            slab(0.0, 0.59375, 0.0, 0.5, 0.0625, 0.5, "wood", 0.4),
        ],
        PartKind::Prop("shelves") => vec![
            slab(
                -0.40625, 0.8125, 0.03125, 0.0625, 1.625, 0.3125, "wood", 0.5,
            ),
            slab(0.40625, 0.8125, 0.03125, 0.0625, 1.625, 0.3125, "wood", 0.5),
            slab(0.0, 0.53125, 0.03125, 0.875, 0.0625, 0.3125, "wood", 0.65),
            slab(0.0, 1.03125, 0.03125, 0.875, 0.0625, 0.3125, "wood", 0.65),
            slab(0.0, 1.53125, 0.03125, 0.875, 0.0625, 0.3125, "wood", 0.65),
        ],
        PartKind::Prop("cupboard") => vec![
            slab(0.0, 0.75, -0.03125, 0.875, 1.5, 0.4375, "wood", 0.5),
            slab(
                0.03125, 0.78125, 0.21875, 0.8125, 1.3125, 0.0625, "wood", 0.65,
            ),
            slab(
                0.09375,
                0.71875,
                0.28125,
                0.0625,
                0.1875,
                0.0625,
                "cloth-gold",
                0.6,
            ),
        ],
        PartKind::Prop("pot") => vec![
            slab(0.0, 0.1875, 0.0, 0.375, 0.375, 0.375, "stone", 0.3),
            slab(
                -0.03125, 0.40625, -0.03125, 0.4375, 0.0625, 0.4375, "stone", 0.45,
            ),
        ],
        PartKind::Prop("basket") => vec![
            slab(
                -0.03125, 0.15625, -0.03125, 0.4375, 0.3125, 0.4375, "sand", 0.55,
            ),
            slab(0.0, 0.28125, 0.0, 0.5, 0.0625, 0.5, "sand", 0.4),
        ],
        PartKind::Prop("rug") => vec![
            slab(0.0, 0.03125, 0.0, 1.375, 0.0625, 0.875, "cloth-red", 0.55),
            slab(0.0, 0.03125, 0.0, 1.125, 0.0625, 0.625, "cloth-red", 0.75),
        ],
        PartKind::Prop("woodpile") => vec![
            slab(0.0, 0.125, -0.03125, 1.0, 0.25, 0.6875, "wood", 0.4),
            slab(0.0, 0.34375, 0.0, 1.0, 0.1875, 0.5, "wood", 0.5),
            slab(0.0, 0.46875, 0.03125, 1.0, 0.1875, 0.3125, "wood", 0.6),
        ],
        PartKind::Prop("candle") => vec![
            slab(
                0.03125, 0.03125, 0.03125, 0.3125, 0.0625, 0.3125, "stone", 0.5,
            ),
            slab(0.03125, 0.625, 0.03125, 0.0625, 1.125, 0.0625, "wood", 0.4),
            slab(0.0, 1.1875, 0.0, 0.125, 0.125, 0.125, "bone", 0.95),
            slab(
                0.03125,
                1.3125,
                0.03125,
                0.0625,
                0.125,
                0.0625,
                "cloth-gold",
                0.95,
            ),
        ],
        PartKind::Prop("sack") => vec![
            slab(
                -0.03125, 0.21875, -0.03125, 0.4375, 0.4375, 0.4375, "bone", 0.6,
            ),
            slab(
                -0.03125, 0.4375, -0.03125, 0.1875, 0.125, 0.1875, "bone", 0.45,
            ),
        ],
        PartKind::Prop("trough") => vec![
            slab(
                -0.03125, 0.15625, -0.03125, 1.1875, 0.3125, 0.4375, "wood", 0.45,
            ),
            slab(
                0.03125, 0.28125, 0.03125, 1.0625, 0.0625, 0.3125, "water", 0.7,
            ),
        ],
        PartKind::Prop("pole") => vec![
            // The corner post: shoulders over both wall ends at a meeting,
            // a shade darker so the frame reads against the panels.
            slab(
                0.0,
                WALL_HIGH * 0.5,
                0.0,
                0.375,
                WALL_HIGH,
                0.375,
                "wood",
                0.45,
            ),
        ],
        PartKind::Prop("door") => vec![
            // Jambs, lintel board, and the leaf itself, all on the
            // lattice, with a gold latch.
            slab(-0.5625, 1.0, 0.0, 0.125, 2.0, 0.375, "wood", 0.45),
            slab(0.5625, 1.0, 0.0, 0.125, 2.0, 0.375, "wood", 0.45),
            slab(0.0, 2.0625, 0.0, 1.25, 0.125, 0.375, "wood", 0.45),
            slab(0.0, 1.0, 0.0625, 1.0, 2.0, 0.125, "wood", 0.35),
            slab(0.375, 1.0, 0.125, 0.125, 0.125, 0.125, "cloth-gold", 0.8),
        ],
        PartKind::Prop("door-double") => vec![
            // The hall door: two leaves meeting in the middle, for the
            // buildings a village walks into rather than through - a hall, a
            // barn, a granary taking a cart. Two full metres of clear opening
            // against the single door's one.
            //
            // The single door's own construction, widened: jambs on the
            // lattice, a lintel board across both leaves, and each leaf the
            // same width as the single's, so a double reads as two of the
            // doors already in the world rather than as a different thing.
            slab(-1.0625, 1.0, 0.0, 0.125, 2.0, 0.375, "wood", 0.45),
            slab(1.0625, 1.0, 0.0, 0.125, 2.0, 0.375, "wood", 0.45),
            slab(0.0, 2.0625, 0.0, 2.25, 0.125, 0.375, "wood", 0.45),
            slab(-0.5, 1.0, 0.0625, 1.0, 2.0, 0.125, "wood", 0.35),
            slab(0.5, 1.0, 0.0625, 1.0, 2.0, 0.125, "wood", 0.35),
            // Latches where the leaves meet, an eighth in from each free
            // edge - the same hand's reach as the single door's.
            slab(-0.125, 1.0, 0.125, 0.125, 0.125, 0.125, "cloth-gold", 0.8),
            slab(0.125, 1.0, 0.125, 0.125, 0.125, 0.125, "cloth-gold", 0.8),
        ],
        PartKind::Prop("doorway") => vec![
            // An opening with no leaf: jambs and a lintel, for the ways
            // between rooms that never wanted a door.
            slab(-0.5625, 1.0, 0.0, 0.125, 2.0, 0.375, "wood", 0.45),
            slab(0.5625, 1.0, 0.0, 0.125, 2.0, 0.375, "wood", 0.45),
            slab(0.0, 2.0625, 0.0, 1.25, 0.125, 0.375, "wood", 0.45),
        ],
        PartKind::Prop("window") => vec![
            // An opening in a timber frame with its bars across it, and
            // no glass in it at all: glass is a later century than this
            // village. Jambs, sill, lintel, and the two muntins.
            slab(-0.5625, 1.375, 0.0, 0.125, 1.125, 0.375, "wood", 0.45),
            slab(0.5625, 1.375, 0.0, 0.125, 1.125, 0.375, "wood", 0.45),
            slab(0.0, 0.8125, 0.0, 1.25, 0.125, 0.375, "wood", 0.45),
            slab(0.0, 1.9375, 0.0, 1.25, 0.125, 0.375, "wood", 0.45),
            slab(0.0, 1.375, -0.03125, 0.125, 1.0, 0.1875, "wood", 0.4),
            slab(0.0, 1.375, -0.03125, 1.0, 0.125, 0.1875, "wood", 0.4),
        ],
        PartKind::Prop("steps") => vec![
            // Three treads rising the foundation's 0.375, from +X.
            slab(0.375, 0.0625, 0.0, 0.375, 0.125, 1.25, "stone", 0.6),
            slab(0.0, 0.125, 0.0, 0.375, 0.25, 1.25, "stone", 0.55),
            slab(-0.375, 0.1875, 0.0, 0.375, 0.375, 1.25, "stone", 0.5),
        ],
        PartKind::Prop("cradle") => vec![
            slab(0.03125, 0.28125, 0.0, 0.5625, 0.3125, 0.875, "wood", 0.55),
            slab(-0.03125, 0.4375, 0.3125, 0.4375, 0.125, 0.25, "bone", 0.9),
            slab(0.0, 0.09375, -0.375, 0.625, 0.0625, 0.125, "wood", 0.4),
            slab(0.0, 0.09375, 0.375, 0.625, 0.0625, 0.125, "wood", 0.4),
        ],
        PartKind::Prop("wardrobe") => vec![
            slab(0.0, 0.9375, 0.0, 1.125, 1.875, 0.5, "wood", 0.5),
            slab(-0.25, 0.96875, 0.28125, 0.5, 1.6875, 0.0625, "wood", 0.62),
            slab(0.25, 0.96875, 0.28125, 0.5, 1.6875, 0.0625, "wood", 0.62),
            slab(
                0.03125,
                0.96875,
                0.28125,
                0.0625,
                0.3125,
                0.0625,
                "cloth-gold",
                0.7,
            ),
        ],
        PartKind::Prop("side-table") => vec![
            slab(0.0, 0.53125, 0.0, 0.625, 0.0625, 0.625, "wood", 0.65),
            slab(-0.21875, 0.25, -0.21875, 0.0625, 0.5, 0.0625, "wood", 0.5),
            slab(0.21875, 0.25, -0.21875, 0.0625, 0.5, 0.0625, "wood", 0.5),
            slab(-0.21875, 0.25, 0.21875, 0.0625, 0.5, 0.0625, "wood", 0.5),
            slab(0.21875, 0.25, 0.21875, 0.0625, 0.5, 0.0625, "wood", 0.5),
        ],
        PartKind::Prop("anvil") => vec![
            slab(-0.03125, 0.15625, 0.0, 0.4375, 0.3125, 0.375, "wood", 0.4),
            slab(0.0, 0.40625, 0.0, 0.25, 0.1875, 0.25, "stone", 0.3),
            slab(-0.03125, 0.5, 0.0, 0.6875, 0.125, 0.25, "stone", 0.45),
        ],
        PartKind::Prop("loom") => vec![
            slab(
                -0.46875, 0.6875, 0.03125, 0.0625, 1.375, 0.0625, "wood", 0.5,
            ),
            slab(0.53125, 0.6875, 0.03125, 0.0625, 1.375, 0.0625, "wood", 0.5),
            slab(
                0.03125, 1.34375, 0.03125, 1.0625, 0.0625, 0.0625, "wood", 0.6,
            ),
            slab(
                0.03125, 0.34375, 0.03125, 1.0625, 0.0625, 0.0625, "wood", 0.6,
            ),
            slab(0.0, 0.8125, 0.03125, 0.875, 0.875, 0.0625, "cloth-red", 0.6),
        ],
        PartKind::Prop("planter") => vec![
            slab(0.0, 0.15625, 0.0, 0.875, 0.3125, 0.375, "earth", 0.4),
            slab(
                -0.21875, 0.40625, -0.03125, 0.1875, 0.1875, 0.1875, "grass", 0.6,
            ),
            slab(0.125, 0.4375, 0.0625, 0.25, 0.25, 0.25, "grass", 0.5),
            slab(
                0.34375, 0.375, -0.03125, 0.1875, 0.125, 0.1875, "grass", 0.7,
            ),
        ],
        PartKind::Prop("fence") => vec![
            // The posts stand ON the fence's own ends and are square in
            // plan, so two fences meeting at a corner put a post in
            // exactly the same place and the two become one post rather
            // than two overlapping ones. Inset posts could never do that:
            // whichever way you turned the second fence, its post landed
            // a finger's width off the first.
            slab(-0.75, 0.4375, 0.0, 0.125, 0.875, 0.125, "wood", 0.45),
            slab(0.75, 0.4375, 0.0, 0.125, 0.875, 0.125, "wood", 0.45),
            slab(0.0, 0.6875, 0.0, 1.5, 0.125, 0.125, "wood", 0.55),
            slab(0.0, 0.375, 0.0, 1.5, 0.125, 0.125, "wood", 0.55),
        ],
        PartKind::Prop("ladder") => vec![
            slab(
                -0.15625, 1.1875, 0.03125, 0.0625, 2.375, 0.0625, "wood", 0.5,
            ),
            slab(0.15625, 1.1875, 0.03125, 0.0625, 2.375, 0.0625, "wood", 0.5),
            slab(0.0, 0.40625, 0.03125, 0.375, 0.0625, 0.0625, "wood", 0.6),
            slab(0.0, 0.84375, 0.03125, 0.375, 0.0625, 0.0625, "wood", 0.6),
            slab(0.0, 1.28125, 0.03125, 0.375, 0.0625, 0.0625, "wood", 0.6),
            slab(0.0, 1.78125, 0.03125, 0.375, 0.0625, 0.0625, "wood", 0.6),
            slab(0.0, 2.21875, 0.03125, 0.375, 0.0625, 0.0625, "wood", 0.6),
        ],
        PartKind::Prop("mannequin") => vec![
            // The game's adult, boxed in bone: a measuring stick with a
            // face. Skipped on import - reference, not furniture.
            slab(-0.125, 0.3125, 0.0, 0.125, 0.625, 0.125, "bone", 0.6),
            slab(0.125, 0.3125, 0.0, 0.125, 0.625, 0.125, "bone", 0.6),
            slab(-0.03125, 0.90625, 0.0, 0.4375, 0.5625, 0.25, "bone", 0.75),
            slab(-0.25, 0.875, 0.0, 0.125, 0.5, 0.125, "bone", 0.6),
            slab(0.25, 0.875, 0.0, 0.125, 0.5, 0.125, "bone", 0.6),
            slab(
                -0.03125, 1.40625, -0.03125, 0.4375, 0.4375, 0.4375, "bone", 0.85,
            ),
        ],
        PartKind::Prop(_) => vec![],
        PartKind::Widget(name) => {
            let (_, ramp, shade) = WIDGETS
                .iter()
                .find(|(w, _, _)| w == name)
                .copied()
                .unwrap_or(("", "bone", 0.5));
            // The sleeping place is shaped like the sleeper: a body laid
            // out at the game's own proportions, head toward the widget's
            // own facing. No guessing whether a bed is long enough, or
            // which end the pillow wants to be.
            if *name == "sleep" {
                // The village's median adult, measured from its own
                // genome: 1.75 head to heel, a half-metre head, a torso
                // seven units across. Head at the facing end.
                return vec![
                    slab(0.625, 0.25, 0.0, 0.5, 0.5, 0.5, ramp, shade),
                    slab(0.0625, 0.125, -0.03125, 0.625, 0.25, 0.4375, ramp, shade),
                    // Arms alongside.
                    slab(0.0625, 0.09375, -0.28125, 0.5, 0.1875, 0.1875, ramp, shade),
                    slab(0.0625, 0.09375, 0.34375, 0.5, 0.1875, 0.1875, ramp, shade),
                    // Legs, to the foot of the bed.
                    slab(
                        -0.5625, 0.09375, -0.15625, 0.625, 0.1875, 0.1875, ramp, shade,
                    ),
                    slab(
                        -0.5625, 0.09375, 0.09375, 0.625, 0.1875, 0.1875, ramp, shade,
                    ),
                ];
            }
            // Smoke has one direction and it is not a compass point. Every
            // other widget wears a nose along its own +X to say which way
            // it faces - and that nose CANNOT be tilted up, because tilt
            // turns a part about its own X and the nose lies along it. So
            // this one is drawn rising instead of pointing: a plume
            // thinning as it goes, which is the whole of what the mark
            // means.
            // A candle throws its light every way at once, so it is the
            // third of the directionless three. A taper with a flame on
            // it: slimmer and taller than the hearth's, so the two read
            // apart at a glance across a dark room.
            if *name == "light" {
                return vec![
                    slab(0.0, 0.25, 0.0, 0.125, 0.5, 0.125, ramp, shade),
                    slab(0.0, 0.5625, 0.0, 0.25, 0.125, 0.25, ramp, shade),
                    slab(0.0, 0.6875, 0.0, 0.125, 0.125, 0.125, ramp, shade),
                ];
            }
            // A fire is the same case: people gather at one from every
            // side, so a nose pointing at one of them says something the
            // eye cannot check. Drawn as a flame instead - and the yaw is
            // still written into the mark either way, so if a hearth's
            // open side ever comes to matter the direction is already
            // there in the file, carried by the hearth's own turn.
            if *name == "fire" {
                return vec![
                    slab(0.0, 0.09375, 0.0, 0.375, 0.1875, 0.375, ramp, shade),
                    slab(0.0, 0.3125, 0.0, 0.25, 0.25, 0.25, ramp, shade),
                    slab(0.0, 0.5, 0.0, 0.125, 0.125, 0.125, ramp, shade),
                ];
            }
            if *name == "smoke" {
                return vec![
                    slab(0.0, 0.1875, 0.0, 0.375, 0.375, 0.375, ramp, shade),
                    slab(0.0, 0.5, 0.0, 0.25, 0.25, 0.25, ramp, shade),
                    slab(0.0, 0.6875, 0.0, 0.125, 0.125, 0.125, ramp, shade),
                ];
            }
            // A seat is shaped like the sitter: knees toward the facing,
            // so a stool's height and a table's clearance can be judged
            // by eye instead of by arithmetic.
            if *name == "sit" {
                // The same adult, sat on a stool's own seat: hips at
                // seven units, the crown a shade over a metre and a half.
                // Knees toward the facing.
                // The seat surface is where the THIGHS rest - seven
                // units up, which is what every seat in this catalogue
                // is cut to. The first version hung its hip joint there
                // instead, so the thighs lay a hand's breadth inside
                // every chair, stool and cushion in the house.
                return vec![
                    // The crown stays where it always was: 25 units.
                    slab(0.0, 1.3125, 0.0, 0.5, 0.5, 0.5, ramp, shade),
                    // Torso: hip to shoulder, deep front to back.
                    slab(0.0, 0.84375, -0.03125, 0.25, 0.4375, 0.4375, ramp, shade),
                    // Arms hanging at the sides.
                    slab(
                        -0.03125, 0.84375, -0.28125, 0.1875, 0.4375, 0.1875, ramp, shade,
                    ),
                    slab(
                        -0.03125, 0.84375, 0.34375, 0.1875, 0.4375, 0.1875, ramp, shade,
                    ),
                    // Thighs, resting ON the seat, forward to the knees.
                    slab(
                        0.21875, 0.53125, -0.15625, 0.4375, 0.1875, 0.1875, ramp, shade,
                    ),
                    slab(
                        0.21875, 0.53125, 0.09375, 0.4375, 0.1875, 0.1875, ramp, shade,
                    ),
                    // Shins, from the knee down to the floor.
                    slab(
                        0.46875, 0.21875, -0.15625, 0.1875, 0.4375, 0.1875, ramp, shade,
                    ),
                    slab(
                        0.46875, 0.21875, 0.09375, 0.1875, 0.4375, 0.1875, ramp, shade,
                    ),
                ];
            }
            vec![
                slab(0.0, 0.1875, 0.0, 0.375, 0.375, 0.375, ramp, shade),
                // The nose: which way the widget faces.
                slab(0.28125, 0.1875, 0.0, 0.1875, 0.125, 0.125, ramp, shade),
            ]
        }
    };
    // A repainted part carries its choice into every structural slab.
    if let Some((ramp, shade)) = repaint {
        for piece in &mut slabs {
            if piece.2 == "wood" || piece.2 == "earth" || piece.2.starts_with("cloth") {
                piece.2 = ramp.to_string();
                piece.3 = shade;
            }
        }
    }
    slabs
}

/// A placed part's record: everything the export needs to rebuild it.
#[derive(Component, Clone, Serialize, Deserialize)]
pub struct Placed {
    pub part: String,
    pub at: [f32; 3],
    pub yaw: f32,
    pub tilt: f32,
    pub ramp: Option<String>,
    pub shade: f32,
    /// The stage the village raises this in: footing, frame, walls, roof,
    /// furnishing - or "widget", which never becomes a box at all.
    #[serde(default)]
    pub stage: String,
    /// Mirrored: the body reflected across its own length, and any tilt
    /// leaning the other way - the far half of a gable, the other hand
    /// of an L.
    #[serde(default)]
    pub flip: bool,
    /// Which group this part belongs to, if any.
    ///
    /// A plain number, shared by everything grouped together, and that is the
    /// whole mechanism: parts wearing one move, paint, delete and travel as one.
    /// It replaces a GUESS with a fact. A part used to carry the marks that
    /// happened to sit within a metre of it, nearest owner winning, which works
    /// until two beds are pushed together or a mark stands between a door and a
    /// wall. A group says which pieces belong to which and stops the bench
    /// inferring it.
    ///
    /// Flat, not nested. A group of groups doubles the question a click has to
    /// answer - the part, or the whole? - and nothing yet needs the second
    /// level.
    #[serde(default)]
    pub group: Option<u32>,

    /// A widget that has been cut loose from whatever it was standing in.
    ///
    /// A door arrives with its routing mark, a bed could arrive with its sleep
    /// mark, and Brett's rule is that they travel together: "they should stay
    /// grouped like the stretch gable roofs unless you right click and ungroup".
    /// So a mark belongs to the part it sits inside, and moving or burying that
    /// part takes the mark with it - until somebody says otherwise here.
    ///
    /// Defaulting to false means every mark in every saved work is bundled,
    /// which is what a maker who placed one inside a door meant by it.
    #[serde(default)]
    pub loose: bool,
}

/// Everything that moves when this part moves: itself, and whatever shares its
/// group.
///
/// A click on any one of a group takes them all, because that is what being
/// grouped means. An ungrouped part is its own company.
pub fn kin_of(part: Entity, records: &Query<(Entity, &Placed), Without<Ghost>>) -> Vec<Entity> {
    let Ok((_, record)) = records.get(part) else {
        return vec![part];
    };
    let Some(group) = record.group else {
        return vec![part];
    };
    records
        .iter()
        .filter(|(_, other)| other.group == Some(group))
        .map(|(entity, _)| entity)
        .collect()
}

/// A group number nothing else is using.
///
/// Taken over whatever records the caller has to hand, since the two callers
/// hold different shapes of query and neither should have to reshape itself to
/// ask a question about numbers.
pub fn a_fresh_group<'a>(records: impl Iterator<Item = &'a Placed>) -> u32 {
    records
        .filter_map(|record| record.group)
        .max()
        .map_or(1, |highest| highest + 1)
}

/// How far a mark may sit from a part and still be carried by it.
///
/// A metre: a double door's two marks stand half a metre either side of its
/// middle, and nothing else a maker places sits that close to a part it does
/// not belong to.
const CARRIES_WITHIN: f32 = 1.0;

/// The marks a part carries: its own, bundled with it.
///
/// Nearest owner wins, so a mark between two parts belongs to the one it is
/// actually in rather than to both. Loose marks belong to nobody.
pub(crate) fn carried_marks<'a>(
    owner: Entity,
    owner_at: Vec3,
    everything: impl Iterator<Item = (Entity, Vec3, &'a Placed)> + Clone,
) -> Vec<Entity> {
    let mut carried = Vec::new();
    for (mark, mark_at, record) in everything.clone() {
        if record.loose || record.stage != "widget" {
            continue;
        }
        let reach = mark_at.distance(owner_at);
        if reach > CARRIES_WITHIN {
            continue;
        }
        // Whoever is nearest. A mark inside a door and beside a wall belongs to
        // the door.
        let nearer = everything.clone().any(|(other, other_at, other_record)| {
            other != mark
                && other_record.stage != "widget"
                && other_at.distance(mark_at) < reach - 1e-4
        });
        if !nearer && owner != mark {
            carried.push(mark);
        }
    }
    carried
}

/// A part's turn: yaw, then tilt - which leans the other way when the
/// part is mirrored, so a pitched panel's twin completes the gable.
pub fn pose(yaw: f32, tilt: f32, flip: bool) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(if flip { -tilt } else { tilt })
}

/// The ghost that follows the cursor while the hand is full.
#[derive(Component)]
pub struct Ghost;

/// The maker's hand: what it holds and how it holds it. Filled from the
/// shelf, or by picking a placed part back up with an empty hand.
#[derive(Resource, Default)]
pub struct Hand {
    pub kind: Option<PartKind>,
    /// A stretch wall's anchored start, once the first click lands.
    pub anchor: Option<Vec3>,
    /// Whether the held part is mirrored.
    pub flip: bool,
    pub stage: String,
    pub yaw: f32,
    pub tilt: f32,
    pub lift: f32,
    pub ramp: Option<String>,
    pub shade: f32,
}

impl Hand {
    fn filled(kind: PartKind, stage: String) -> Self {
        // A roof comes to hand already pitched: a flat panel is the
        // exception, not the starting point.
        let tilt = if matches!(kind, PartKind::Roof(..) | PartKind::RoofRun) {
            ROOF_PITCH_DEGREES.to_radians()
        } else {
            0.0
        };
        Hand {
            kind: Some(kind),
            anchor: None,
            flip: false,
            tilt,
            stage,
            shade: 0.7,
            ..default()
        }
    }

    fn record(&self, at: Vec3) -> Option<Placed> {
        let kind = self.kind.as_ref()?;
        Some(Placed {
            part: part_name(kind),
            at: at.into(),
            yaw: self.yaw,
            tilt: self.tilt,
            ramp: self.ramp.clone(),
            shade: self.shade,
            stage: self.stage.clone(),
            flip: self.flip,
            loose: false,
            group: None,
        })
    }
}

/// A shelf button holding one catalog entry.
#[derive(Component)]
struct ShelfButton(&'static CatalogEntry);

/// A shelf button holding one widget.
#[derive(Component)]
struct WidgetButton(&'static str);

/// The button that sweeps the bench bare.
#[derive(Component)]
pub(crate) struct ClearButton;

/// A drawer header: pressing it opens and closes the drawer body.
#[derive(Component)]
struct DrawerHeader {
    body: Entity,
    label: Entity,
    name: &'static str,
    open: bool,
}

/// The shelf panel itself, shown only at the Builder bench.
#[derive(Component)]
struct Shelf;

/// The export/save button.
#[derive(Component)]
pub(crate) struct SaveButton;

/// A button that asks the desktop for a work to open.
///
/// Brett wanted `.baz` files associated with the bench so a double click opened
/// them, and then found the shorter way himself: "what if we just had a open
/// file button?" - which needs nothing of the operating system, works the same
/// on both, and can open a work kept anywhere rather than only the ones in the
/// bench's own folder.
#[derive(Component)]
pub(crate) struct OpenWorkButton;

/// A work the maker has chosen from the desktop's own file window, waiting to
/// be set out on the bench.
#[derive(Resource, Default)]
pub(crate) struct WorkWanted(pub Option<std::path::PathBuf>);

/// The save button's label, so it can say what just happened.
#[derive(Component)]
pub(crate) struct SaveLabel;

/// The name this work goes by, once it has been given one. Saving again
/// updates the same file instead of scattering copies.
#[derive(Resource, Default)]
pub struct WorkName(pub Option<String>);

/// A label speaking a passing word; it returns to its old text at `until`.
#[derive(Component)]
pub(crate) struct PassingWord {
    back: &'static str,
    until: f32,
}

/// The name being typed for an export, while the naming card is up.
/// While this is Some, every other key on the bench holds its tongue.
#[derive(Resource, Default)]
pub struct Naming(pub Option<String>);

/// The naming card's root, for tearing it down.
#[derive(Component)]
struct NamingCard;

/// The text inside the card that shows the name as it is typed.
#[derive(Component)]
struct NameText;

/// The card's own buttons, for those who would rather click.
#[derive(Component)]
struct NamingSave;

#[derive(Component)]
struct NamingCancel;

/// F walks between face snapping and plain ground placement; G cycles
/// the grid interval through the powers of the atom.
fn toggle_snap_mode(
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    mut mode: ResMut<SnapMode>,
    mut grid: ResMut<SnapGrid>,
    mut labels: Query<&mut Text, With<SnapModeText>>,
) {
    if *bench == Bench::Builder && naming.0.is_none() && dims.0.is_none() {
        if keys.just_pressed(KeyCode::KeyF) {
            mode.face = !mode.face;
        }
        if keys.just_pressed(KeyCode::KeyG) {
            grid.0 = match grid.0 {
                1 => 2,
                2 => 4,
                4 => 8,
                8 => 16,
                _ => 1,
            };
        }
    }
    let word = format!(
        "face snap - {} (F) / grid - {} (G)",
        if mode.face { "on" } else { "off" },
        grid.0
    );
    for mut label in &mut labels {
        if label.0 != word {
            *label = Text::new(word.clone());
        }
    }
}

pub struct BuilderPlugin;

impl Plugin for BuilderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hand>()
            .init_resource::<Stages>()
            .init_resource::<StageWish>()
            .init_resource::<StageHeld>()
            .init_resource::<WorkWanted>()
            .init_resource::<Brush>()
            .init_resource::<Naming>()
            .init_resource::<Hovered>()
            .init_resource::<WorkName>()
            .init_resource::<SnapMode>()
            .init_resource::<DimsEntry>()
            .init_resource::<History>()
            .init_resource::<SnapGrid>()
            .init_resource::<Clipboard>()
            .init_resource::<RoofsLifted>()
            .add_systems(
                Startup,
                (raise_shelf, raise_palette).after(crate::rail::raise_rail),
            )
            // Two chains, because a tuple of systems stops at twenty:
            // the hand's work, then the bench's bookkeeping after it.
            .add_systems(
                Update,
                (
                    show_shelf,
                    work_drawers,
                    work_shelf,
                    open_or_clear,
                    steer_hand,
                    // The painting tools and the part menu as one group: this
                    // tuple is at Bevy's own limit for how many systems it will
                    // take in a row, and a nested tuple counts as one.
                    (
                        paint_the_work,
                        work_palette,
                        raise_part_menu,
                        turn_to_stage,
                        bury_the_chosen,
                    ),
                    toggle_snap_mode,
                    disarm_on_mode,
                    turn_part,
                    tilt_part,
                    turn_the_work,
                    reflow_openings,
                    lift_roofs,
                    copy_and_paste,
                    mirror_part,
                    feel_ahead,
                    move_ghost,
                    // The menu acts AFTER the grab has had its look, and takes
                    // the ordering from this chain rather than asking for it: an
                    // `after` inside an already-chained tuple states the
                    // opposite of what the chain states, which Bevy cannot solve
                    // and will not start with.
                    //
                    // The order matters because a click on the menu is the
                    // menu's business, and the grab knows that - it steps aside
                    // for any click landing on interface. But choosing a line
                    // despawns the menu that same frame, so with the menu first
                    // the grab looked for interface under the cursor, found it
                    // already gone, and took the click for the world: it picked
                    // the roof up as it came apart.
                    (place_grab_remove, work_part_menu).chain(),
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    save_workbench,
                    pick_a_work,
                    take_the_name,
                    dims_panel,
                    recall,
                    remember,
                    settle_words,
                )
                    .chain()
                    .after(place_grab_remove),
            );
    }
}

pub fn part_name(kind: &PartKind) -> String {
    match kind {
        PartKind::Wall(len) => format!("wall-{len}"),
        PartKind::Seg { long, high, lift } => format!("wallseg-{long}x{high}@{lift}"),
        PartKind::Trim { long, stone } => {
            if *stone {
                format!("trimstone-{long}")
            } else {
                format!("trim-{long}")
            }
        }
        PartKind::Gable(long, pitch) => format!("gable-{long}x{pitch}"),
        PartKind::Beam(long, high, low) => format!("beam-{long}x{high}x{low}"),
        PartKind::BeamRun => "beamrun".to_string(),
        PartKind::Ridge(long) => format!("ridge-{long}"),
        PartKind::Chimney(drop) => format!("chimney-{drop}"),
        PartKind::GableRoof(long, span, over, pitch) => {
            format!("gableroof-{long}x{span}x{over}x{pitch}")
        }
        PartKind::RoofPlan(w, d) => format!("roofplan-{w}x{d}"),
        PartKind::Floor(w, d) => format!("floor-{w}x{d}"),
        PartKind::Foundation(w, d) => format!("foundation-{w}x{d}"),
        PartKind::Roof(w, d) => format!("roof-{w}x{d}"),
        PartKind::WallRun
        | PartKind::TrimRun { .. }
        | PartKind::SegRun { .. }
        | PartKind::GableRun
        | PartKind::RidgeRun
        | PartKind::GableRoofRun
        | PartKind::FloorRun
        | PartKind::FoundationRun
        | PartKind::RoofRun => "run".to_string(),
        PartKind::Prop(name) => format!("prop:{name}"),
        PartKind::Widget(name) => format!("widget:{name}"),
    }
}

pub fn kind_from_name(name: &str) -> Option<PartKind> {
    if let Some(rest) = name.strip_prefix("wall-") {
        return rest.parse::<f32>().ok().map(PartKind::Wall);
    }
    if let Some(rest) = name.strip_prefix("gableroof-") {
        // Four numbers now: the building it covers, the overhang, and the
        // pitch. Three is a roof from before the pitch could be pulled and two
        // from before the eaves could; both still open, at the pitch every roof
        // in the world had when they were drawn.
        let mut parts = rest.split('x');
        let long = parts.next()?.parse().ok()?;
        let span = parts.next()?.parse().ok()?;
        let over = parts.next().and_then(|o| o.parse().ok()).unwrap_or(0.25);
        let pitch = parts
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(ROOF_PITCH_DEGREES);
        return Some(PartKind::GableRoof(long, span, over, pitch));
    }
    if let Some(rest) = name.strip_prefix("chimney-") {
        return rest.parse::<f32>().ok().map(PartKind::Chimney);
    }
    if name == "prop:chimney" {
        // The first chimneys, from before the shaft could reach.
        return Some(PartKind::Chimney(0.0));
    }
    if let Some(rest) = name.strip_prefix("ridge-") {
        return rest.parse::<f32>().ok().map(PartKind::Ridge);
    }
    if let Some(rest) = name.strip_prefix("beam-") {
        // Three numbers: its length and the cut at each end. One is a beam from
        // before ends could be cut, and opens square at both.
        let mut parts = rest.split('x');
        let long = parts.next()?.parse().ok()?;
        let high = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
        let low = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
        return Some(PartKind::Beam(long, high, low));
    }
    if let Some(rest) = name.strip_prefix("gable-") {
        // Two numbers now, its width and its pitch. One is a gable from before
        // gables had a pitch of their own, and opens at the one they all had.
        let mut parts = rest.split('x');
        let long = parts.next()?.parse().ok()?;
        let pitch = parts
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(ROOF_PITCH_DEGREES);
        return Some(PartKind::Gable(long, pitch));
    }
    if let Some(rest) = name.strip_prefix("trimstone-") {
        return rest
            .parse::<f32>()
            .ok()
            .map(|long| PartKind::Trim { long, stone: true });
    }
    if let Some(rest) = name.strip_prefix("trim-") {
        return rest
            .parse::<f32>()
            .ok()
            .map(|long| PartKind::Trim { long, stone: false });
    }
    if let Some(rest) = name.strip_prefix("floor-") {
        return sides_of(rest).map(|(w, d)| PartKind::Floor(w, d));
    }
    if let Some(rest) = name.strip_prefix("foundation-") {
        return sides_of(rest).map(|(w, d)| PartKind::Foundation(w, d));
    }
    if let Some(rest) = name.strip_prefix("roof-") {
        return sides_of(rest).map(|(w, d)| PartKind::Roof(w, d));
    }
    if let Some(rest) = name.strip_prefix("wallseg-") {
        let (long, rest) = rest.split_once('x')?;
        let (high, lift) = rest.split_once('@')?;
        return Some(PartKind::Seg {
            long: long.parse().ok()?,
            high: high.parse().ok()?,
            lift: lift.parse().ok()?,
        });
    }
    if let Some(wanted) = name.strip_prefix("prop:") {
        return STRUCTURE
            .iter()
            .chain(FURNITURE)
            .chain(DECOR)
            .find_map(|e| match e.kind {
                PartKind::Prop(p) if p == wanted => Some(e.kind),
                _ => None,
            });
    }
    if let Some(widget) = name.strip_prefix("widget:") {
        return WIDGETS
            .iter()
            .find(|(w, _, _)| *w == widget)
            .map(|(w, _, _)| PartKind::Widget(w));
    }
    match name {
        // Legacy names from before the primitives learned their sizes.
        "floor" => Some(PartKind::Floor(2.0, 2.0)),
        "roof" => Some(PartKind::Roof(2.2, 2.2)),
        "prop:foundation" => Some(PartKind::Foundation(2.0, 2.0)),
        "prop:trim" => Some(PartKind::Trim {
            long: 2.0,
            stone: false,
        }),
        "prop:trim-stone" => Some(PartKind::Trim {
            long: 2.0,
            stone: true,
        }),
        _ => None,
    }
}

/// Splits "3x4.5" into its two sides.
fn sides_of(text: &str) -> Option<(f32, f32)> {
    let (w, d) = text.split_once('x')?;
    Some((w.parse().ok()?, d.parse().ok()?))
}

/// Spawns a part's boxes under one root. Widgets go translucent.
fn spawn_part(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
    kind: &PartKind,
    record: &Placed,
    ghostly: bool,
) -> Entity {
    let root = commands
        .spawn((
            record.clone(),
            Transform::from_translation(Vec3::from(record.at)).with_rotation(pose(
                record.yaw,
                record.tilt,
                record.flip,
            )),
            Visibility::default(),
            BuilderFurniture,
        ))
        .id();
    if ghostly {
        commands.entity(root).insert(Ghost);
    }
    dress_part(
        commands, meshes, materials, palette, kind, record, root, ghostly,
    );
    root
}

/// The figures a piece of furniture comes with: where a body will lie
/// or sit on it, and which way round. They arrive as REAL widgets set
/// down beside the furniture, not as decoration - so an unwanted one is
/// picked up and thrown away like anything else, and a bed with no
/// sleeper on it means exactly that.
///
/// Every one of these pieces faces its own +Z - the back of a chair, the
/// pillow of a bed - and a widget's nose is its +X, so the figure always
/// turns a quarter behind the furniture that carries it.
pub fn companions(kind: &PartKind) -> Vec<(&'static str, Vec3)> {
    // Mattress top: where a sleeper's back actually rests.
    const LIE: f32 = 0.53125;
    match kind {
        PartKind::Prop("bed") => vec![("sleep", Vec3::new(0.0, LIE, 0.0))],
        PartKind::Prop("bed-double") => vec![
            ("sleep", Vec3::new(-0.5, LIE, 0.0)),
            ("sleep", Vec3::new(0.5, LIE, 0.0)),
        ],
        PartKind::Prop("chair" | "stool") => vec![("sit", Vec3::ZERO)],
        PartKind::Prop("bench") => vec![
            ("sit", Vec3::new(-0.4375, 0.0, 0.0)),
            ("sit", Vec3::new(0.4375, 0.0, 0.0)),
        ],
        // A cushion sits higher than a plank, so the sitter rides up
        // with it, and forward a touch so the knees clear the plinth.
        PartKind::Prop("couch") => vec![
            ("sit", Vec3::new(-0.4375, 0.09375, 0.0)),
            ("sit", Vec3::new(0.4375, 0.09375, 0.0)),
        ],
        _ => vec![],
    }
}

/// Sets down the figures a piece of furniture implies, alongside it.
#[allow(clippy::too_many_arguments)]
pub fn seat_the_figures(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
    kind: &PartKind,
    record: &Placed,
) {
    let turn = pose(record.yaw, record.tilt, record.flip);
    for (what, offset) in companions(kind) {
        let widget = PartKind::Widget(what);
        let offset = if record.flip {
            Vec3::new(-offset.x, offset.y, offset.z)
        } else {
            offset
        };
        let at = Vec3::from(record.at) + turn * offset;
        let mark = Placed {
            part: part_name(&widget),
            at: at.into(),
            yaw: record.yaw - std::f32::consts::FRAC_PI_2,
            tilt: 0.0,
            ramp: None,
            shade: 0.7,
            stage: "widget".to_string(),
            flip: false,
            loose: false,
            group: None,
        };
        spawn_part(commands, meshes, materials, palette, &widget, &mark, false);
    }
}

/// Dresses an existing root in a part's boxes - the resize handles use
/// this to rebuild a body in place without disturbing the entity.
#[allow(clippy::too_many_arguments)]
pub fn dress_part(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
    kind: &PartKind,
    record: &Placed,
    root: Entity,
    ghostly: bool,
) {
    let translucent = ghostly || matches!(kind, PartKind::Widget(_));
    let repaint = record.ramp.as_deref().map(|r| (r, record.shade));
    for Slab(mut at, size, ramp, shade, clarity, shape, mut lean) in body_of(kind, repaint) {
        // Mirrored: the body reflects across its own length, and any
        // lean of its own leans the other way.
        if record.flip {
            at.x = -at.x;
            lean = -lean;
        }
        let mut color = palette.shade(&ramp, shade);
        let see_through = translucent || clarity < 1.0;
        if see_through {
            color = color.with_alpha(if ghostly {
                0.45
            } else if matches!(kind, PartKind::Widget(_)) {
                0.55
            } else {
                clarity
            });
        }
        commands.spawn((
            Mesh3d(match shape {
                Shape::Wedge => meshes.add(wedge_mesh(false)),
                Shape::Ridge => meshes.add(wedge_mesh(true)),
                Shape::Mitre => meshes.add(mitre_mesh(false)),
                Shape::MitreBack => meshes.add(mitre_mesh(true)),
                Shape::Box => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            }),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.95,
                reflectance: 0.03,
                alpha_mode: if see_through {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Opaque
                },
                ..default()
            })),
            Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_x(lean))
                .with_scale(size),
            ChildOf(root),
        ));
    }
}

/// Rebuilds the ghost from the hand's current state.
fn dress_ghost(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
    hand: &Hand,
    ghosts: &Query<Entity, With<Ghost>>,
) {
    for ghost in ghosts {
        commands.entity(ghost).despawn();
    }
    if let Some(kind) = hand.kind
        && let Some(record) = hand.record(Vec3::new(0.0, hand.lift, 0.0))
    {
        spawn_part(commands, meshes, materials, palette, &kind, &record, true);
    }
}

/// Something a right-click can do to the part under the cursor.
///
/// One entry today. The menu exists as much for the ones after it — Brett:
/// "that way we could add other things to the menu later" — and a menu is the
/// right home for the deeds that are neither a tool nor a key: rare enough not
/// to earn a letter, specific enough to belong to one part.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Deed {
    /// Break a made part into the parts it is made of.
    Ungroup,
    /// Say what a part IS - which decides what comes off with the roof, what
    /// the walls are made of, and what the village can walk through.
    Nature(&'static str),
    /// Bring a part back until it stops at the roof instead of coming through
    /// it.
    TrimToRoof,
    /// Make one thing of everything chosen.
    Group,
}

impl Deed {
    fn label(self) -> &'static str {
        match self {
            Deed::Ungroup => "UNGROUP",
            Deed::Nature("roof") => "PART OF THE ROOF",
            Deed::Nature("walls") => "PART OF THE WALLS",
            Deed::Nature("frame") => "PART OF THE FRAME",
            Deed::Nature("footing") => "PART OF THE FOOTING",
            Deed::Nature(_) => "FURNISHING",
            Deed::TrimToRoof => "TRIM TO THE ROOF",
            Deed::Group => "GROUP",
        }
    }
}

/// Brings a part back along its own length until it stops at the roof.
///
/// Brett wanted the option rather than the rule, and was right to: "Somethings
/// like Chimneys I would want to clip through, thats why I would like an
/// option." A chimney's whole purpose is to come out the other side.
///
/// It moves the GEOMETRY rather than hiding pixels, and that is the whole
/// constraint. The bench's promise is that what is saved is what the village
/// raises, and the bake emits plain boxes - so a beam clipped only in the
/// picture would sit right here and stand proud of the roof out in the world,
/// which is a worse bug than the one being fixed because it cannot be seen from
/// the bench at all.
///
/// Each end is cast at the roof separately and only the end that meets one comes
/// in. The other stays exactly where the maker put it, because a part that
/// shortened from both ends would walk out of the joint it was seated in.
///
/// The end stays SQUARE. A mitre is what a carpenter would cut, and a mitre is
/// not a box - the baked format speaks boxes, wedges and ridges, and nothing
/// that is a box with one corner off. At a steep pitch that leaves a gap about
/// the beam's own thickness, which in a world built of boxes reads as ordinary
/// framing.
fn trim_to_roof(
    kind: &PartKind,
    record: &Placed,
    roofs: &[(Vec3, Vec3, Quat)],
) -> Option<(PartKind, Placed)> {
    let (long, rebuild) = length_of(kind)?;
    let spin = pose(record.yaw, record.tilt, record.flip);
    let along = spin * Vec3::X;
    let at = Vec3::from(record.at);
    let half = long * 0.5;

    // How far each end may reach before it is inside the roof.
    // A box the part's MIDDLE already lies inside is a part it is SEATED IN,
    // not one it is coming through, and trimming to it would be nonsense: a tie
    // beam across a gable end sits in that gable by design. Left in, such a box
    // reports a hit at no distance at all in both directions, the trim comes out
    // as nothing, and the deed silently declines - which is exactly what Brett
    // saw. His beam sits in its gable and pokes out through a slope; the slope
    // was two and three quarter metres along the very ray that had already
    // given up.
    let seated: Vec<&(Vec3, Vec3, Quat)> = roofs
        .iter()
        .filter(|(box_at, box_half, box_turn)| {
            !point_in_box(at, *box_at, *box_half, *box_turn)
        })
        .collect();

    // The part's own cross-section, so its CORNERS are cast and not its centre
    // line. A square beam meeting a slanted roof touches with a corner first, so
    // trimming to where the middle meets the slope leaves that corner standing
    // proud of it - a small stub above the roof, which is what Brett
    // photographed after the first trim worked.
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for Slab(offset, size, ..) in body_of(kind, None) {
        low = low.min(offset - size * 0.5);
        high = high.max(offset + size * 0.5);
    }
    let corners = [
        Vec3::new(0.0, low.y, low.z),
        Vec3::new(0.0, low.y, high.z),
        Vec3::new(0.0, high.y, low.z),
        Vec3::new(0.0, high.y, high.z),
    ];

    // Every corner is cast, and BOTH the nearest and the furthest are kept. The
    // near one is where the saw goes in and the far one is where it comes out -
    // and the difference between them is the cut. A square end can only stop at
    // the near one, which is why it stood off the slope by the beam's own
    // thickness; Brett: "get the angle of the roof and cut the beam at that
    // angle and delete the escess".
    let mut nearest = [half, half];
    let mut furthest = [0.0_f32, 0.0];
    let mut met = [false, false];
    for (end, way) in [(0usize, 1.0_f32), (1, -1.0)] {
        for corner in corners {
            let from = at + spin * corner;
            for (box_at, box_half, box_turn) in &seated {
                if let Some(hit) =
                    ray_meets_box(from, along * way, half, *box_at, *box_half, *box_turn)
                {
                    nearest[end] = nearest[end].min(hit);
                    furthest[end] = furthest[end].max(hit);
                    met[end] = true;
                }
            }
        }
        if !met[end] {
            furthest[end] = half;
        }
    }
    if !met[0] && !met[1] {
        return None;
    }
    // The beam reaches as far as the LAST corner to meet the roof; the cut takes
    // back the wedge between that and the first.
    let reach = furthest;
    let cut = [
        (furthest[0] - nearest[0]).max(0.0),
        (furthest[1] - nearest[1]).max(0.0),
    ];
    let trimmed = reach[0] + reach[1];
    if trimmed < 0.125 {
        return None;
    }
    let middle = at + along * (reach[0] - reach[1]) * 0.5;
    let lattice = |n: f32| (n * 16.0).round() / 16.0;
    // What this end was ALREADY cut back to, if a saw has been here before.
    let (had_high, had_low) = match *kind {
        PartKind::Beam(_, high, low) => (high, low),
        _ => (0.0, 0.0),
    };
    let made = match rebuild(lattice(trimmed).max(0.125)) {
        // A beam takes its cuts; anything else can only be shortened, since
        // nothing else has an end to cut.
        //
        // And it KEEPS the ones it has. The cast reads a beam's square envelope
        // - the box its slabs fill - so a beam already mitred to a slope reads
        // as meeting that slope everywhere at once, the wedge between first
        // corner and last comes out as nothing, and a second trim handed the
        // beam back with square ends at the same length it already had. Which
        // is exactly what a maker sees as the saw work being undone: Brett,
        // after tagging a trimmed beam as part of the roof, "it loses its trim
        // to roof angles". Taking the deeper of the two can only ever cut more.
        PartKind::Beam(long, ..) => PartKind::Beam(
            long,
            lattice(cut[0]).max(had_high),
            lattice(cut[1]).max(had_low),
        ),
        other => other,
    };
    let mut moved = record.clone();
    moved.part = part_name(&made);
    moved.at = middle.into();
    Some((made, moved))
}

/// The length a part is drawn to, if it has one, and the kind it becomes at
/// another length.
///
/// The `long` family: everything a maker draws by pulling it out to size along
/// its own X. Trimming is only meaningful for these — a part with no length has
/// no direction to come back along.
fn length_of(kind: &PartKind) -> Option<(f32, Box<dyn Fn(f32) -> PartKind>)> {
    match *kind {
        PartKind::Beam(long, high, low) => {
            Some((long, Box::new(move |n| PartKind::Beam(n, high, low))))
        }
        PartKind::Wall(long) => Some((long, Box::new(PartKind::Wall))),
        PartKind::Ridge(long) => Some((long, Box::new(PartKind::Ridge))),
        _ => None,
    }
}

/// Whether a point is inside an oriented box.
fn point_in_box(p: Vec3, box_at: Vec3, box_half: Vec3, box_turn: Quat) -> bool {
    let local = box_turn.inverse() * (p - box_at);
    (0..3).all(|axis| local[axis].abs() <= box_half[axis])
}

/// Where a ray first meets an oriented box, if it does within `reach`.
///
/// The slab method, in the box's own frame - which is why the box may be turned
/// any way at all, and a roof's slopes are turned every way there is.
fn ray_meets_box(
    from: Vec3,
    along: Vec3,
    reach: f32,
    box_at: Vec3,
    box_half: Vec3,
    box_turn: Quat,
) -> Option<f32> {
    let inverse = box_turn.inverse();
    let origin = inverse * (from - box_at);
    let heading = inverse * along;
    let (mut near, mut far) = (0.0_f32, reach);
    for axis in 0..3 {
        let (o, d, h) = (origin[axis], heading[axis], box_half[axis]);
        if d.abs() < 1e-6 {
            if o.abs() > h {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((-h - o) / d, (h - o) / d);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        near = near.max(t0);
        far = far.min(t1);
        if near > far {
            return None;
        }
    }
    (near <= far && near >= 0.0).then_some(near)
}

/// The natures a maker can hand a part, in the order they are offered.
///
/// Brett's idea, and it takes a rule the bench was enforcing by TYPE and hands
/// it to the maker: "we could tag an item as a roof peice... then I dont need
/// special peices for the roof...any peice could be a roof piece." A plank laid
/// as a canopy is a roof, and until now only the parts the bench thought of as
/// roofs came off when H was pressed.
///
/// Nothing new had to be stored for it. A part already carries what it is — the
/// cutaway reads it, the game reads it to learn a building's wall and roof
/// cloth, and the walkable shell is what stands on `walls` or `frame`. It was
/// simply never something a maker could change after placing.
const NATURES: [&str; 5] = ["footing", "frame", "walls", "roof", "furnishing"];

/// What may be done to this part.
///
/// Ungrouping is offered only where the pieces have somewhere to GO. A gable
/// roof is two slopes and two ends, and the bench already has a part for each -
/// a roof panel and a gable - so breaking one up leaves four parts a maker can
/// go on working with. A door is jambs and a leaf and a latch, and the bench has
/// no part for a jamb, so breaking one up would leave nothing but a hole where a
/// door used to be.
fn deeds_for(kind: &PartKind) -> Vec<Deed> {
    // Only what the KIND can answer. Whether a part is also carrying marks is a
    // question about this bench rather than about the kind, so the menu asks
    // that one itself and adds the entry if the answer is yes - an UNGROUP that
    // would do nothing is worse than no UNGROUP, because it has to be tried
    // before a maker learns it is nothing.
    let mut deeds = match kind {
        PartKind::GableRoof(..) => vec![Deed::Ungroup],
        _ => Vec::new(),
    };
    // Every part can be told what it is; only some can be broken up.
    deeds.extend(NATURES.iter().map(|nature| Deed::Nature(nature)));
    deeds
}

/// The menu itself. Which part raised it rides on each LINE, since that is
/// where it is read - keeping a second copy up here only invited the two to
/// disagree.
#[derive(Component)]
struct PartMenu;

/// One line of it.
#[derive(Component)]
struct MenuLine {
    deed: Deed,
    part: Entity,
}

/// Right-click a part to raise its menu.
///
/// On the RELEASE, and only if the mouse barely moved: the right button already
/// orbits the camera, so a right DRAG has to stay an orbit and only a right
/// CLICK is a menu. Every three-dimensional tool resolves it this way and a
/// maker will not notice the rule at all, which is the point of it.
#[allow(clippy::too_many_arguments)]
fn raise_part_menu(
    mut commands: Commands,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<bevy::input::mouse::MouseMotion>,
    windows: Query<&Window>,
    hovered: Res<Hovered>,
    naming: Res<Naming>,
    hand: Res<Hand>,
    selected: Res<crate::gizmo::Selected>,
    placed: Query<(Entity, &Transform, &Placed), Without<Ghost>>,
    menus: Query<Entity, With<PartMenu>>,
    mut drift: Local<f32>,
) {
    let stirred: f32 = motion.read().map(|moved| moved.delta.length()).sum();
    if buttons.pressed(MouseButton::Right) {
        *drift += stirred;
    }
    if buttons.just_pressed(MouseButton::Right) {
        *drift = 0.0;
    }
    if !buttons.just_released(MouseButton::Right) {
        return;
    }
    let travelled = std::mem::take(&mut *drift);
    for menu in &menus {
        commands.entity(menu).despawn();
    }
    // Four pixels of slop: a hand resting on a mouse is never quite still, and
    // an orbit that happens to end where it started is still an orbit.
    if travelled > 4.0 || naming.0.is_some() || hand.kind.is_some() {
        return;
    }
    let Some(part) = hovered.grab else {
        return;
    };
    let Ok((_, _, record)) = placed.get(part) else {
        return;
    };
    let Some(kind) = kind_from_name(&record.part) else {
        return;
    };
    let mut deeds = deeds_for(&kind);
    // Grouping needs company: one part is already as gathered as it gets.
    if selected.0.len() > 1 {
        deeds.insert(0, Deed::Group);
    }
    // Only where there is a roof to trim to, and a length to trim.
    if length_of(&kind).is_some()
        && placed
            .iter()
            .any(|(_, _, other)| other.stage == "roof" && other.part != record.part)
    {
        deeds.insert(0, Deed::TrimToRoof);
    }
    // Carrying a mark is reason enough to offer it, whatever the part is.
    if !deeds.contains(&Deed::Ungroup)
        && placed.get(part).is_ok_and(|(_, _, record)| record.group.is_some())
    {
        deeds.insert(0, Deed::Ungroup);
    }
    if !deeds.contains(&Deed::Ungroup) {
        let held: Vec<(Entity, Vec3, &Placed)> = placed
            .iter()
            .map(|(entity, at, record)| (entity, at.translation, record))
            .collect();
        let carrying = placed.get(part).is_ok_and(|(_, at, _)| {
            !carried_marks(
                part,
                at.translation,
                held.iter().map(|(e, at, record)| (*e, *at, *record)),
            )
            .is_empty()
        });
        if carrying {
            deeds.insert(0, Deed::Ungroup);
        }
    }
    if deeds.is_empty() {
        return;
    }
    let Some(at) = windows.iter().next().and_then(|window| window.cursor_position()) else {
        return;
    };

    let menu = commands
        .spawn((
            PartMenu,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(at.x),
                top: Val::Px(at.y),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
            GlobalZIndex(60),
        ))
        .id();
    let wearing = record.stage.clone();
    for deed in deeds {
        // The nature it already has is marked, so the menu answers "what is
        // this?" as well as offering to change it.
        let standing = matches!(deed, Deed::Nature(nature) if nature == wearing);
        let line = commands
            .spawn((
                MenuLine { deed, part },
                Interaction::default(),
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                ChildOf(menu),
            ))
            .id();
        commands.spawn((
            Text::new(if standing {
                format!("- {}", deed.label())
            } else {
                format!("  {}", deed.label())
            }),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(if standing {
                theme::accent(&palette)
            } else {
                theme::text_dim(&palette)
            }),
            ChildOf(line),
        ));
    }
}

/// Carries a chosen deed out, and shuts the menu on anything else.
#[allow(clippy::too_many_arguments)]
fn work_part_menu(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    palette: Res<Palette>,
    selected: Res<crate::gizmo::Selected>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lines: Query<(&MenuLine, &Interaction, &mut BackgroundColor)>,
    menus: Query<Entity, With<PartMenu>>,
    mut placed: Query<(Entity, &mut Transform, &mut Placed), Without<Ghost>>,
) {
    if menus.is_empty() {
        return;
    }
    let mut chosen = None;
    let mut over = false;
    for (line, interaction, mut fill) in &mut lines {
        if *interaction != Interaction::None {
            over = true;
        }
        if *interaction == Interaction::Pressed {
            chosen = Some((line.deed, line.part));
        }
        let wanted = BackgroundColor(if *interaction == Interaction::None {
            Color::NONE
        } else {
            theme::accent(&palette).with_alpha(0.18)
        });
        if fill.0 != wanted.0 {
            *fill = wanted;
        }
    }
    // A click anywhere but on the menu closes it, which is what a menu does.
    let dismissed =
        keys.just_pressed(KeyCode::Escape) || (buttons.just_pressed(MouseButton::Left) && !over);
    if chosen.is_none() && !dismissed {
        return;
    }
    match chosen {
        Some((Deed::Ungroup, part)) => {
            // A group first: the commonest thing UNGROUP is asked to undo, now
            // that grouping exists at all.
            let holding = placed.get(part).ok().and_then(|(_, _, record)| record.group);
            if let Some(group) = holding {
                let kin: Vec<Entity> = placed
                    .iter()
                    .filter(|(_, _, record)| record.group == Some(group))
                    .map(|(entity, _, _)| entity)
                    .collect();
                for part in kin {
                    if let Ok((_, _, mut record)) = placed.get_mut(part) {
                        record.group = None;
                    }
                }
            }
            // Then the marks it carries, which any part may have; then the
            // part's own pieces, which only some parts can be broken into.
            let held: Vec<(Entity, Vec3, Placed)> = placed
                .iter()
                .map(|(entity, at, record)| (entity, at.translation, record.clone()))
                .collect();
            if let Some((_, owner_at, record)) = held.iter().find(|(e, ..)| *e == part) {
                let carried = carried_marks(
                    part,
                    *owner_at,
                    held.iter().map(|(e, at, record)| (*e, *at, record)),
                );
                for mark in &carried {
                    if let Ok((_, _, mut loosed)) = placed.get_mut(*mark) {
                        loosed.loose = true;
                    }
                }
                if let Some(kind) = kind_from_name(&record.part) {
                    let record = record.clone();
                    let together =
                        a_fresh_group(held.iter().map(|(_, _, record)| record));
                    ungroup(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &palette,
                        &kind,
                        &record,
                        part,
                        together,
                    );
                }
            }
        }
        Some((Deed::TrimToRoof, part)) => {
            // Every box the roof is made of, in world space, so the part's own
            // ends can be cast at them.
            let roofs: Vec<(Vec3, Vec3, Quat)> = placed
                .iter()
                .filter(|(other, _, record)| *other != part && record.stage == "roof")
                .filter_map(|(_, at, record)| {
                    kind_from_name(&record.part).map(|kind| (at, record, kind))
                })
                .flat_map(|(at, record, kind)| {
                    let spin = pose(record.yaw, record.tilt, record.flip);
                    body_of(&kind, None)
                        .into_iter()
                        .map(move |Slab(offset, size, _, _, _, _, lean)| {
                            // A slab may lean inside its own part - a roof's
                            // slopes do nothing else - so its turn is the part's
                            // and then its own.
                            let turn = spin * Quat::from_rotation_x(lean);
                            (at.translation + spin * offset, size * 0.5, turn)
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            let trimmed = placed.get(part).ok().and_then(|(_, _, record)| {
                kind_from_name(&record.part)
                    .and_then(|kind| trim_to_roof(&kind, record, &roofs))
            });
            if let Some((made, moved)) = trimmed
                && let Ok((_, mut transform, mut record)) = placed.get_mut(part)
            {
                transform.translation = Vec3::from(moved.at);
                *record = moved.clone();
                commands.entity(part).despawn_related::<Children>();
                dress_part(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &palette,
                    &made,
                    &moved,
                    part,
                    false,
                );
            }
        }
        Some((Deed::Group, _)) => {
            let together = a_fresh_group(placed.iter().map(|(_, _, record)| record));
            let chosen: Vec<Entity> = selected.iter().collect();
            for part in chosen {
                if let Ok((_, _, mut record)) = placed.get_mut(part) {
                    record.group = Some(together);
                }
            }
        }
        Some((Deed::Nature(nature), part)) => {
            // Nothing to redraw: a part's nature changes what the bench and the
            // village make of it, not what it looks like.
            if let Ok((_, _, mut record)) = placed.get_mut(part) {
                record.stage = nature.to_string();
            }
        }
        None => {}
    }
    for menu in &menus {
        commands.entity(menu).despawn();
    }
}

/// Breaks a made part into the parts it is made of, standing exactly where its
/// own pieces stood — and GROUPED, so it is still one thing to move.
///
/// Which is where the two kinds of togetherness meet, and Brett asked how they
/// fit: a gable roof is a MADE part, parametric, a span and an overhang and a
/// pitch with handles that change those numbers and re-derive every piece from
/// them. A group is an ASSEMBLY: separate parts, each with its own record, moved
/// as one and edited freely. Neither is a lesser version of the other.
///
/// Ungrouping is the door between them, and it only opens one way. What was a
/// roof becomes four parts that can each be pulled, pitched, painted and
/// deleted - and can no longer be pitched TOGETHER, because the number they
/// shared is gone. Trading parameters for freedom is exactly what a maker is
/// asking for when they break something up, so the trade is the feature. Coming
/// back is what undo is for.
///
/// A gable roof is four things the bench can already draw: two slopes, which are
/// roof panels, and two ends, which are gables. So it comes apart into four
/// parts a maker can pull, pitch, paint and delete one at a time — which is the
/// whole point, and the reason a door cannot do this (see [`deeds_for`]).
///
/// The offsets are read off the same arithmetic that draws the roof, because a
/// piece that lands even slightly off is worse than no ungroup at all: a maker
/// would have to notice, and then correct something they did not move.
///
/// Undo needs no help here. The bench remembers whole states rather than deeds,
/// so four parts appearing and one leaving is one step back like anything else.
#[allow(clippy::too_many_arguments)]
fn ungroup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
    kind: &PartKind,
    record: &Placed,
    part: Entity,
    together: u32,
) {
    let PartKind::GableRoof(long, span, over, degrees) = *kind else {
        return;
    };
    let pitch = degrees.clamp(PITCH_LEAST, PITCH_MOST).to_radians();
    let half = span * 0.5;
    let rise = half * pitch.tan();
    let slope = (half * half + rise * rise).sqrt();
    let thick = 0.125;
    let at = Vec3::from(record.at);
    let spin = pose(record.yaw, record.tilt, record.flip);

    let mut born: Vec<(PartKind, Placed)> = Vec::new();
    for way in [-1.0_f32, 1.0] {
        // Where the slope's own slab sits inside the roof.
        let middle = Vec3::new(
            0.0,
            rise * 0.5 + thick * 0.5 - over * 0.5 * pitch.sin(),
            way * (half * 0.5 + over * 0.5 * pitch.cos()),
        );
        // A roof panel is drawn standing ON its origin - its slab is half a
        // thickness up - while the slope's offset is the slab's MIDDLE. So the
        // panel is seated half a thickness back down its own face, which is not
        // straight down once it is pitched.
        let lean = record.tilt + way * pitch;
        let face = pose(record.yaw, lean, record.flip) * Vec3::Y;
        let panel = PartKind::Roof(long + over * 2.0, slope + over);
        born.push((
            panel,
            Placed {
                part: part_name(&panel),
                at: (at + spin * middle - face * (thick * 0.5)).into(),
                yaw: record.yaw,
                tilt: lean,
                ramp: record.ramp.clone(),
                shade: record.shade,
                stage: record.stage.clone(),
                flip: record.flip,
                loose: false,
                group: None,
            },
        ));

        // The ends. A gable is drawn with its width along its own X and the
        // roof's ends stand across the span, so each one is turned a quarter
        // circle to face down the building.
        let gable = PartKind::Gable(span, degrees);
        born.push((
            gable,
            Placed {
                part: part_name(&gable),
                at: (at + spin * Vec3::new(way * (long * 0.5 - 0.125), 0.0, 0.0)).into(),
                yaw: record.yaw + std::f32::consts::FRAC_PI_2,
                tilt: record.tilt,
                ramp: record.ramp.clone(),
                shade: record.shade,
                stage: record.stage.clone(),
                flip: record.flip,
                loose: false,
                group: None,
            },
        ));
    }

    // One number across the pieces, so the roof that WAS one thing to move is
    // still one thing to move.
    for (kind, mut record) in born {
        record.group = Some(together);
        spawn_part(commands, meshes, materials, palette, &kind, &record, false);
    }
    commands.entity(part).despawn();
}

// -------------------------------------------------------------- the palette

/// The palette panel, shown only while painting.
#[derive(Component)]
struct PalettePanel;

/// The big square at the head of the palette: the colour now armed.
#[derive(Component)]
struct BrushFace;

/// One colour a maker can arm. `ramp` empty is the bare swatch: painting with
/// it strips a part back to its own colours.
#[derive(Component, Clone)]
struct Swatch {
    ramp: Option<String>,
    shade: f32,
}

/// The shades a swatch row offers, which are the shades the keys step through —
/// so a colour picked with the mouse can be nudged with `-` and `=` and land on
/// another swatch rather than between two of them.
const SWATCHES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// Builds the palette once, standing hidden until the paint tool is chosen.
///
/// Brett: "a palet comes up on the screen. You have an armed color and click a
/// part to paint it." Which is the right shape and better than what the keys
/// alone could do — walking `[` and `]` through twenty-four ramps is guessing at
/// a colour, and a palette is looking at one. The keys stay for nudging a shade
/// once the eye is close.
fn raise_palette(mut commands: Commands, fonts: Res<Fonts>, palette: Res<Palette>) {
    let panel = commands
        .spawn((
            PalettePanel,
            // In the SHELF's place, not beside it. The shelf holds what a
            // building is made of and this holds what it is coloured with, and a
            // maker who is painting is not placing - so one panel stands at a
            // time and neither has to be squeezed to make room for the other.
            // (The left edge is spoken for: the key rail lives there.)
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                // The shelf's own width: the two share an edge and only one
                // stands at a time, so a differing width would make the panel
                // jump as a maker changes tool.
                width: Val::Px(212.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(2.0),
                border: UiRect::left(Val::Px(1.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
            Visibility::Hidden,
        ))
        .id();
    commands.spawn((
        Text::new("THE PALETTE"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(theme::accent(&palette)),
        Node {
            margin: UiRect::bottom(Val::Px(6.0)),
            ..default()
        },
        ChildOf(panel),
    ));

    // The armed colour, large, at the head of the panel. A swatch ringed in gold
    // says which one is armed but says it in the size of a swatch; this says
    // what is actually on the brush, at a size worth glancing at.
    commands.spawn((
        BrushFace,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(44.0),
            border: UiRect::all(Val::Px(1.0)),
            margin: UiRect::bottom(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(palette.shade("wood", 0.5)),
        BorderColor::all(theme::accent(&palette)),
        ChildOf(panel),
    ));

    // The bare swatch first, because stripping a part is the one stroke a maker
    // cannot reach any other way once a part has been painted.
    let bare = commands
        .spawn((
            Swatch {
                ramp: None,
                shade: 0.5,
            },
            Interaction::default(),
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::panel_border(&palette)),
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        Text::new("BARE - its own colours"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(theme::text_dim(&palette)),
        ChildOf(bare),
    ));

    let names: Vec<String> = palette.names().map(|n| n.to_string()).collect();
    for name in names {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(2.0),
                    ..default()
                },
                ChildOf(panel),
            ))
            .id();
        commands.spawn((
            Text::new(name.clone()),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(theme::text_dim(&palette)),
            Node {
                width: Val::Px(74.0),
                ..default()
            },
            ChildOf(row),
        ));
        for shade in SWATCHES {
            commands.spawn((
                Swatch {
                    ramp: Some(name.clone()),
                    shade,
                },
                Interaction::default(),
                Node {
                    width: Val::Px(20.0),
                    height: Val::Px(18.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(palette.shade(&name, shade)),
                BorderColor::all(Color::BLACK.with_alpha(0.35)),
                ChildOf(row),
            ));
        }
    }
}

/// Shows the palette while painting, arms whatever is clicked, and rings the
/// armed colour in gold.
#[allow(clippy::too_many_arguments)]
fn work_palette(
    palette: Res<Palette>,
    mode: Res<crate::gizmo::ToolMode>,
    hovered: Res<Hovered>,
    placed: Query<&Placed, Without<Ghost>>,
    mut brush: ResMut<Brush>,
    mut panels: Query<&mut Visibility, With<PalettePanel>>,
    mut face: Query<&mut BackgroundColor, With<BrushFace>>,
    mut swatches: Query<(&Swatch, &Interaction, &mut BorderColor)>,
) {
    let painting = *mode == crate::gizmo::ToolMode::Paint;
    for mut visibility in &mut panels {
        let wanted = if painting {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
    if !painting {
        return;
    }
    for (swatch, interaction, _) in &swatches {
        if *interaction == Interaction::Pressed {
            brush.ramp = swatch.ramp.clone();
            brush.shade = swatch.shade;
        }
    }
    // The brush's own face. An empty brush strips rather than paints, and shows
    // as the panel's own dark rather than as a colour it does not have.
    let showing = match brush.ramp.as_deref() {
        Some(ramp) => palette.shade(ramp, brush.shade),
        None => Color::BLACK.with_alpha(0.30),
    };
    for mut fill in &mut face {
        if fill.0 != showing {
            *fill = BackgroundColor(showing);
        }
    }

    // What the part under the cursor is wearing, so the palette can point at
    // it. Brett's idea, and better than the eyedropper he first reached for:
    // there is no tool to arm and no modifier to hold, and once the swatch has
    // lit up, clicking it is the whole of picking the colour up. It also tells a
    // maker where in the ramps a colour they liked actually lives, which an
    // eyedropper never would.
    let worn = hovered
        .grab
        .and_then(|part| placed.get(part).ok())
        .map(|record| (record.ramp.clone(), record.shade));

    for (swatch, _, mut border) in &mut swatches {
        let same_as = |ramp: &Option<String>, shade: f32| {
            swatch.ramp.as_deref() == ramp.as_deref()
                // The bare swatch stands for a part with no paint at all, and a
                // part like that has a shade the maker never chose.
                && (swatch.ramp.is_none()
                    // Half a step, because a shade set before the swatches
                    // existed - or nudged from an odd starting point - can sit
                    // between two of them, and the nearer one is the honest
                    // answer.
                    || (swatch.shade - shade).abs() < 0.13)
        };
        let armed = same_as(&brush.ramp, brush.shade);
        let shown = worn
            .as_ref()
            .is_some_and(|(ramp, shade)| same_as(ramp, *shade));
        let dress = BorderColor::all(match (armed, shown) {
            // Armed wins the gold: what the next click will lay down matters
            // more than what the cursor happens to be over.
            (true, _) => theme::accent(&palette),
            (false, true) => Color::WHITE.with_alpha(0.85),
            (false, false) => Color::BLACK.with_alpha(0.35),
        });
        if *border != dress {
            *border = dress;
        }
    }
}

// ---------------------------------------------------------------- the shelf

fn raise_shelf(
    mut commands: Commands,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
) {
    let shelf = commands
        .spawn((
            Shelf,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                // Wide enough for the longest thing on it - ROOF, GABLE,
                // STRETCH - on one line. A shelf that wraps its own names is a
                // shelf a maker reads twice.
                width: Val::Px(212.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(3.0),
                border: UiRect::left(Val::Px(1.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
            crate::look::Scrollable,
            bevy::ui::ScrollPosition::default(),
        ))
        .id();

    commands.spawn((
        SnapModeText,
        Text::new("face snap - on (F)"),
        TextFont {
            font: fonts.text.clone().into(),
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(theme::text_dim(&palette)),
        ChildOf(shelf),
    ));


    // The drawers of parts.
    for (name, entries, open) in [
        ("STRUCTURE", STRUCTURE, true),
        ("FURNITURE", FURNITURE, false),
        ("DECOR", DECOR, false),
    ] {
        let body = drawer(&mut commands, &fonts, &palette, shelf, name, open);
        for entry in entries {
            let button = plain_button(&mut commands, &palette, body);
            commands.entity(button).insert(ShelfButton(entry));
            button_label(&mut commands, &fonts, &palette, button, entry.label);
        }
    }
    let widgets = drawer(&mut commands, &fonts, &palette, shelf, "WIDGETS", false);
    for (name, _, _) in WIDGETS {
        let button = plain_button(&mut commands, &palette, widgets);
        commands.entity(button).insert(WidgetButton(name));
        button_label(
            &mut commands,
            &fonts,
            &palette,
            button,
            Box::leak(name.to_uppercase().into_boxed_str()),
        );
    }

}

/// A drawer: a header that opens and closes, and the body under it.
/// Returns the body, ready for buttons.
fn drawer(
    commands: &mut Commands,
    fonts: &Fonts,
    palette: &Palette,
    shelf: Entity,
    name: &'static str,
    open: bool,
) -> Entity {
    let header = commands
        .spawn((
            Interaction::default(),
            Node {
                margin: UiRect::top(Val::Px(8.0)),
                padding: UiRect::axes(Val::Px(2.0), Val::Px(2.0)),
                ..default()
            },
            ChildOf(shelf),
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(format!("{} {}", name, if open { "-" } else { "+" })),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(theme::accent(palette)),
            ChildOf(header),
        ))
        .id();
    let body = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                display: if open { Display::Flex } else { Display::None },
                ..default()
            },
            ChildOf(shelf),
        ))
        .id();
    commands.entity(header).insert(DrawerHeader {
        body,
        label,
        name,
        open,
    });
    body
}

/// A saved-work row: the load button, and the small x that buries it.
fn plain_button(commands: &mut Commands, palette: &Palette, parent: Entity) -> Entity {
    commands
        .spawn((
            Interaction::default(),
            Node {
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::panel_border(palette)),
            ChildOf(parent),
        ))
        .id()
}

/// A word on a button. Returns the text so a caller can say more about how it
/// should break — most should not, since breaking at spaces is what reading is.
fn button_label(
    commands: &mut Commands,
    fonts: &Fonts,
    palette: &Palette,
    button: Entity,
    label: &'static str,
) -> Entity {
    commands.spawn((
        Text::new(label),
        TextFont {
            font: fonts.text.clone().into(),
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(theme::text_dim(palette)),
        ChildOf(button),
    ))
    .id()
}

/// The shelf belongs to the Builder bench alone.
/// Which of the two right-hand panels is standing.
///
/// The shelf and the palette share an edge, and this is the ONE place that says
/// which is on it: a second system reaching for either one would be two writers
/// on one value, and they would take turns hiding each other's panel.
fn show_shelf(
    bench: Res<Bench>,
    mode: Res<crate::gizmo::ToolMode>,
    mut shelves: Query<&mut Visibility, With<Shelf>>,
) {
    if !bench.is_changed() && !mode.is_changed() {
        return;
    }
    // Painting is not placing: the parts go away while the colours are out.
    let standing = *bench == Bench::Builder && *mode != crate::gizmo::ToolMode::Paint;
    for mut visibility in &mut shelves {
        *visibility = if standing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Drawer headers open and close their bodies.
fn work_drawers(
    mut headers: Query<(&mut DrawerHeader, &Interaction), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
    mut labels: Query<&mut Text>,
) {
    for (mut header, interaction) in &mut headers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        header.open = !header.open;
        if let Ok(mut node) = nodes.get_mut(header.body) {
            node.display = if header.open {
                Display::Flex
            } else {
                Display::None
            };
        }
        if let Ok(mut text) = labels.get_mut(header.label) {
            *text = Text::new(format!(
                "{} {}",
                header.name,
                if header.open { "-" } else { "+" }
            ));
        }
    }
}

/// Shelf presses fill the hand; the armed entry wears the gold.
#[allow(clippy::too_many_arguments)]
fn work_shelf(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
    mut hand: ResMut<Hand>,
    mut tool: ResMut<crate::gizmo::ToolMode>,
    mut parts: Query<(&Interaction, &ShelfButton, &mut BorderColor), Without<WidgetButton>>,
    mut widgets: Query<(&Interaction, &WidgetButton, &mut BorderColor), Without<ShelfButton>>,
    ghosts: Query<Entity, With<Ghost>>,
) {
    let mut rearmed = false;
    for (interaction, button, _) in &parts {
        if *interaction == Interaction::Pressed && hand.kind != Some(button.0.kind) {
            *hand = Hand::filled(button.0.kind, button.0.stage.to_string());
            rearmed = true;
        }
    }
    for (interaction, button, _) in &widgets {
        let kind = PartKind::Widget(button.0);
        if *interaction == Interaction::Pressed && hand.kind != Some(kind) {
            *hand = Hand::filled(kind, "widget".to_string());
            rearmed = true;
        }
    }
    if rearmed {
        *tool = crate::gizmo::ToolMode::Normal;
        dress_ghost(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &hand,
            &ghosts,
        );
    }
    for (_, button, mut border) in &mut parts {
        dress_shelf_border(&palette, hand.kind == Some(button.0.kind), &mut border);
    }
    for (_, button, mut border) in &mut widgets {
        dress_shelf_border(
            &palette,
            hand.kind == Some(PartKind::Widget(button.0)),
            &mut border,
        );
    }
}

fn dress_shelf_border(palette: &Palette, standing: bool, border: &mut BorderColor) {
    let dress = BorderColor::all(if standing {
        theme::accent(palette)
    } else {
        theme::panel_border(palette)
    });
    if *border != dress {
        *border = dress;
    }
}

/// Template presses sweep the bench and set out the ready-made start; the
/// clear button just sweeps.
#[allow(clippy::too_many_arguments)]
/// Asks the desktop for a work to open.
///
/// The dialog is the system's own, and it stops the world while it is up - which
/// is what a modal dialog is. `NonSendMarker` is what keeps this system on the
/// main thread, where a Mac insists its panels be raised; without it Bevy is
/// free to run it on a worker and the panel is a crash rather than a window.
fn pick_a_work(
    _main_thread: bevy::ecs::system::NonSendMarker,
    bench: Res<Bench>,
    mut wanted: ResMut<WorkWanted>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<OpenWorkButton>)>,
) {
    if *bench != Bench::Builder {
        return;
    }
    if !buttons
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        return;
    }
    let home = works_home();
    let _ = std::fs::create_dir_all(&home);
    if let Some(path) = rfd::FileDialog::new()
        .set_title("Open a work")
        .add_filter("Divus Factus works", &[WORK_KIND])
        .set_directory(&home)
        .pick_file()
    {
        wanted.0 = Some(path);
    }
}

/// Opens a saved work onto a cleared bench, or simply clears it.
fn open_or_clear(
    mut commands: Commands,
    mut stages: ResMut<Stages>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
    bench: Res<Bench>,
    mut chosen: ResMut<WorkWanted>,
    clears: Query<&Interaction, (Changed<Interaction>, With<ClearButton>)>,
    standing: Query<Entity, (With<Placed>, Without<Ghost>)>,
    mut work_name: ResMut<WorkName>,
) {
    if *bench != Bench::Builder {
        return;
    }
    let wanted = chosen.0.take();
    let sweeping = clears
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed);
    if wanted.is_none() && !sweeping {
        return;
    }
    for part in &standing {
        commands.entity(part).despawn();
    }
    let Some(path) = wanted else {
        work_name.0 = None;
        return;
    };
    // A loaded work carries its own name into the bench.
    work_name.0 = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string());
    match std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Workbench>(&text).ok())
    {
        Some(bench) => {
            // A work from before stages becomes stages on the way in, by the
            // rule the game used to infer them with - so an old building opens
            // with exactly the steps the village was already raising it in.
            let drawings = if bench.stages.is_empty() {
                stages_from_flat(&bench.parts)
            } else {
                bench.stages
            };
            // The LAST step: a maker opening a building wants the building,
            // not its footings.
            let showing = drawings.len().saturating_sub(1);
            let count = drawings[showing].len();
            for record in &drawings[showing] {
                if let Some(kind) = kind_from_name(&record.part) {
                    spawn_part(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &palette,
                        &kind,
                        record,
                        false,
                    );
                }
            }
            let steps = drawings.len();
            stages.drawings = drawings;
            stages.showing = showing;
            info!(
                "set out {}: {count} parts, step {} of {steps}",
                path.display(),
                showing + 1
            );
        }
        None => warn!("nothing readable at {}", path.display()),
    }
}

// ---------------------------------------------------------------- the hand

/// Keys that steer what the hand holds. Esc empties it.
#[allow(clippy::too_many_arguments)]
/// The colour the paint tool lays down.
///
/// `None` is not a colour but the absence of one: painting with an empty brush
/// strips a part back to the colours its own body was drawn with, which is the
/// only way back once a part has been painted.
#[derive(Resource)]
pub struct Brush {
    pub ramp: Option<String>,
    pub shade: f32,
}

impl Default for Brush {
    fn default() -> Self {
        Brush {
            ramp: Some("wood".to_string()),
            shade: 0.5,
        }
    }
}

/// Paints what is already standing.
///
/// The colour keys only ever spoke to the HAND: a part took its ramp and shade
/// from whatever was held when it went down, and after that the only way to
/// change a wall's colour was to delete the wall. Brett asked whether a building
/// could be painted, and it could not.
///
/// A mode rather than a modifier, on Brett's suggestion — PAINT sits with MOVE
/// and RESIZE on the bar, so the tool is somewhere you can see rather than a key
/// you have to know. In it, clicking a part paints it: the bench's own picking
/// already turns a click into a selection, and in this mode a selection IS the
/// stroke, so there is no second way of pointing at a part to keep in step with
/// the first.
///
/// The brush takes the same four keys the hand uses for the same job — `[` and
/// `]` through the ramps, `-` and `=` darker and brighter — because a maker
/// should not have to learn the colour keys twice. `\` empties the brush, and
/// painting with an empty brush gives a part its own colours back.
///
/// Shift paints the whole building instead of the one part: a shade at a time is
/// how a colour gets found, all at once is what happens when it has been.
#[allow(clippy::too_many_arguments)]
fn paint_the_work(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    palette: Res<Palette>,
    naming: Res<Naming>,
    mode: Res<crate::gizmo::ToolMode>,
    selected: Res<crate::gizmo::Selected>,
    mut brush: ResMut<Brush>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut placed: Query<(Entity, &mut Placed), Without<Ghost>>,
) {
    if *mode != crate::gizmo::ToolMode::Paint || naming.0.is_some() {
        return;
    }

    // The brush first, so a key and a click on the same frame paint the colour
    // the maker just chose rather than the one before it.
    let ramps: Vec<&str> = palette.names().collect();
    if !ramps.is_empty() {
        let step = i32::from(keys.just_pressed(KeyCode::BracketRight))
            - i32::from(keys.just_pressed(KeyCode::BracketLeft));
        if step != 0 {
            let here = brush
                .ramp
                .as_deref()
                .and_then(|r| ramps.iter().position(|n| *n == r))
                .unwrap_or(0);
            let next = (here as i32 + step).rem_euclid(ramps.len() as i32) as usize;
            brush.ramp = Some(ramps[next].to_string());
        }
    }
    let by = f32::from(keys.just_pressed(KeyCode::Equal))
        - f32::from(keys.just_pressed(KeyCode::Minus));
    if by != 0.0 {
        brush.shade = (brush.shade + by * 0.25).clamp(0.0, 1.0);
        if brush.ramp.is_none() {
            brush.ramp = Some(ramps.first().copied().unwrap_or("wood").to_string());
        }
    }
    if keys.just_pressed(KeyCode::Backslash) {
        brush.ramp = None;
    }

    // A stroke is a CLICK ON A PART, and nothing else. Changing the brush used
    // to repaint whatever was standing selected - meant as a way to hold a wall
    // and walk the ramps watching it change, and wrong: arming a colour is
    // choosing, not doing, and a maker choosing a colour has not said where they
    // want it yet. Brett: "Arming a color shouldnt paint."
    if !selected.is_changed() {
        return;
    }
    if selected.is_empty() {
        return;
    }
    let whole = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    for (part, mut record) in &mut placed {
        if !whole && !selected.holds(part) {
            continue;
        }
        let Some(kind) = kind_from_name(&record.part) else {
            continue;
        };
        if record.ramp.as_deref() == brush.ramp.as_deref()
            && (record.shade - brush.shade).abs() < 1e-4
        {
            continue;
        }
        record.ramp = brush.ramp.clone();
        record.shade = brush.shade;
        let copy = record.clone();
        commands.entity(part).despawn_related::<Children>();
        dress_part(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &kind,
            &copy,
            part,
            false,
        );
    }
}

fn steer_hand(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    palette: Res<Palette>,
    naming: Res<Naming>,
    mut hand: ResMut<Hand>,
    ghosts: Query<Entity, With<Ghost>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if hand.kind.is_none() || naming.0.is_some() {
        return;
    }
    // Backspace as well, which is what the key marked "delete" reports on a
    // Mac. Without it, the one key a maker would try to empty their hand with
    // did nothing at all, and escape was the only way - which is exactly the
    // roundabout Brett described.
    if keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::Delete)
        || keys.just_pressed(KeyCode::Backspace)
    {
        if hand.anchor.is_some() {
            hand.anchor = None;
        } else {
            *hand = Hand::default();
            for ghost in &ghosts {
                commands.entity(ghost).despawn();
            }
        }
        return;
    }
    let mut redress = false;
    if keys.just_pressed(KeyCode::KeyR) {
        hand.yaw += std::f32::consts::FRAC_PI_2;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        // A whole turn, not a quarter. Stopping at ninety meant the tilt
        // wrapped back to flat just as it reached upright, so nothing
        // could ever be stood on its end - and shift walks it back, since
        // twenty-three presses to undo one is not a control.
        let step = if held_shift(&keys) {
            -15f32.to_radians()
        } else {
            15f32.to_radians()
        };
        hand.tilt = (hand.tilt + step).rem_euclid(std::f32::consts::TAU);
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        hand.lift = (hand.lift + 0.25).min(8.0);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        hand.lift = (hand.lift - 0.25).max(0.0);
    }
    let ramps: Vec<&str> = palette.names().collect();
    if keys.just_pressed(KeyCode::BracketRight) && !ramps.is_empty() {
        let here = hand
            .ramp
            .as_deref()
            .and_then(|r| ramps.iter().position(|n| *n == r))
            .unwrap_or(0);
        hand.ramp = Some(ramps[(here + 1) % ramps.len()].to_string());
        redress = true;
    }
    if keys.just_pressed(KeyCode::BracketLeft) && !ramps.is_empty() {
        let here = hand
            .ramp
            .as_deref()
            .and_then(|r| ramps.iter().position(|n| *n == r))
            .unwrap_or(0);
        hand.ramp = Some(ramps[(here + ramps.len() - 1) % ramps.len()].to_string());
        redress = true;
    }
    if keys.just_pressed(KeyCode::Minus) {
        hand.shade = (hand.shade - 0.25).max(0.0);
        redress = true;
    }
    if keys.just_pressed(KeyCode::Equal) {
        hand.shade = (hand.shade + 0.25).min(1.0);
        redress = true;
    }
    if redress {
        dress_ghost(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &hand,
            &ghosts,
        );
    }
}

/// Where the cursor's ray meets the working plane (the grid, lifted).
fn cursor_point(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    lift: f32,
) -> Option<Vec3> {
    let window = windows.iter().next()?;
    let cursor = window.cursor_position()?;
    let (camera, camera_at) = cameras.iter().next()?;
    let ray = camera.viewport_to_world(camera_at, cursor).ok()?;
    let reach = ray.intersect_plane(Vec3::Y * lift, InfinitePlane3d::new(Vec3::Y))?;
    Some(ray.get_point(reach))
}

/// Whether this kind counts as structure - walls, their leavings, floors,
/// roofs, foundations and steps. Structure rests only on structure; props
/// rest on anything.
fn is_structure(kind: &PartKind) -> bool {
    matches!(
        kind,
        PartKind::Wall(_)
            | PartKind::Seg { .. }
            | PartKind::Floor(..)
            | PartKind::FloorRun
            | PartKind::Foundation(..)
            | PartKind::FoundationRun
            | PartKind::Roof(..)
            | PartKind::RoofRun
            | PartKind::Trim { .. }
            | PartKind::TrimRun { .. }
            | PartKind::SegRun { .. }
            | PartKind::Gable(..)
            | PartKind::GableRun
            | PartKind::Ridge(..)
            | PartKind::Chimney(..)
            | PartKind::RidgeRun
            | PartKind::GableRoof(..)
            | PartKind::GableRoofRun
            | PartKind::Prop("steps")
            | PartKind::Prop("pole")
    )
}

/// The carried part's footprint, spoken as sample points: its centre and
/// four corners, drawn in slightly so edge-kisses do not flicker.
fn footprint_samples(kind: &PartKind, at: Vec3, yaw: f32) -> Vec<Vec3> {
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for Slab(slab_at, size, ..) in body_of(kind, None) {
        low = low.min(slab_at - size * 0.5);
        high = high.max(slab_at + size * 0.5);
    }
    if !low.x.is_finite() {
        return vec![at];
    }
    let spin = Quat::from_rotation_y(yaw);
    let mut samples = vec![at];
    for (cx, cz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = Vec3::new(
            if cx < 0.0 { low.x } else { high.x } * 0.9,
            0.0,
            if cz < 0.0 { low.z } else { high.z } * 0.9,
        );
        samples.push(at + spin * corner);
    }
    samples
}

/// The height of whatever stands beneath a point: the highest slab top
/// whose footprint holds it. Widgets hold nothing up; structure is picky
/// about what it stands on.
fn support_height(
    placed: &Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
    samples: &[Vec3],
    carrying_structure: bool,
    except: Option<Entity>,
) -> f32 {
    let mut top = 0.0f32;
    for (entity, transform, record, showing) in placed {
        // A wall the cutaway has taken away holds nothing up and
        // catches nothing: what you cannot see, you cannot build on.
        if *showing == Visibility::Hidden {
            continue;
        }
        if Some(entity) == except {
            continue;
        }
        let Some(kind) = kind_from_name(&record.part) else {
            continue;
        };
        if matches!(kind, PartKind::Widget(_)) {
            continue;
        }
        if carrying_structure && !is_structure(&kind) {
            continue;
        }
        let turn = pose(record.yaw, record.tilt, record.flip);
        let repaint = record.ramp.as_deref().map(|r| (r, record.shade));
        for Slab(mut at, size, ..) in body_of(&kind, repaint) {
            if record.flip {
                at.x = -at.x;
            }
            let face_y = at.y + size.y * 0.5;
            for sample in samples {
                // Where the sample's own column meets this piece's top
                // face. The turn carries (lx, face_y, lz) into the world,
                // so its x and z rows are two equations in lx and lz -
                // which is how a ridge finds the top of a SLOPING roof
                // instead of sliding off to the wall beneath it.
                let base = Vec3::new(sample.x, 0.0, sample.z) - transform.translation;
                let cx = turn * Vec3::X;
                let cy = turn * Vec3::Y;
                let cz = turn * Vec3::Z;
                let det = cx.x * cz.z - cz.x * cx.z;
                if det.abs() < 1e-5 {
                    continue;
                }
                let rx = base.x - cy.x * face_y;
                let rz = base.z - cy.z * face_y;
                let lx = (rx * cz.z - cz.x * rz) / det;
                let lz = (cx.x * rz - rx * cx.z) / det;
                if (lx - at.x).abs() <= size.x * 0.5 && (lz - at.z).abs() <= size.z * 0.5 {
                    let world_y = transform.translation.y + (turn * Vec3::new(lx, face_y, lz)).y;
                    top = top.max(world_y);
                    break;
                }
            }
        }
    }
    top
}

/// A platform's top rectangle: foundations and floors, the things walls
/// stand on and line up against.
struct PlatformRect {
    at: Vec3,
    yaw: f32,
    half: Vec2,
}

fn platform_rects(
    placed: &Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
) -> Vec<PlatformRect> {
    let mut rects = Vec::new();
    for (_, transform, record, showing) in placed {
        if *showing == Visibility::Hidden {
            continue;
        }
        let Some(kind) = kind_from_name(&record.part) else {
            continue;
        };
        if !matches!(kind, PartKind::Floor(..) | PartKind::Foundation(..)) {
            continue;
        }
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for Slab(at, size, ..) in body_of(&kind, None) {
            low = low.min(at - size * 0.5);
            high = high.max(at + size * 0.5);
        }
        rects.push(PlatformRect {
            at: transform.translation,
            yaw: record.yaw,
            half: Vec2::new((high.x - low.x) * 0.5, (high.z - low.z) * 0.5),
        });
    }
    rects
}

/// Wall centrelines sit half a wall inside the platform edge, which
/// puts the timber's OUTER FACE flush with the stone's: fully seated,
/// no gap, no overhang. Corners still meet cleanly because platform
/// corners pull walls only along their own line - the flush snap owns
/// the sideways part - and the pole caps the centreline crossing.
const PLINTH_REVEAL: f32 = WALL_THICK * 0.5;

/// The ends of every standing full-height wall piece, for the magnets.
/// Every standing wall end, with the direction it points out of its own
/// wall - the joint math needs to know which way a tip faces.
fn wall_ends(
    placed: &Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
) -> Vec<(Vec3, Vec3)> {
    let mut ends = Vec::new();
    for (_, transform, record, showing) in placed {
        if *showing == Visibility::Hidden {
            continue;
        }
        let long = match kind_from_name(&record.part) {
            Some(PartKind::Wall(long)) => long,
            Some(PartKind::Seg { long, lift, .. }) if lift == 0.0 => long,
            _ => continue,
        };
        let along = Quat::from_rotation_y(record.yaw) * Vec3::X;
        ends.push((transform.translation + along * (long * 0.5), along));
        ends.push((transform.translation - along * (long * 0.5), -along));
    }
    ends
}

#[allow(clippy::too_many_arguments)]
fn move_ghost(
    mut commands: Commands,
    bench: Res<Bench>,
    hand: Res<Hand>,
    mode: Res<SnapMode>,
    snap_grid: Res<SnapGrid>,
    hovered: Res<Hovered>,
    selected: Res<crate::gizmo::Selected>,
    mut ghost_shapes: Query<&mut Visibility, With<Ghost>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    placed: Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
    mut ghosts: Query<(Entity, &mut Transform, &Placed), With<Ghost>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
) {
    if *bench != Bench::Builder {
        return;
    }
    // While the arrows are out, the ghost stands aside entirely - fine
    // tuning wants a clear view and no accidental placements.
    let tuning = !selected.is_empty();
    for mut visibility in &mut ghost_shapes {
        let wanted = if tuning {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
    if tuning {
        return;
    }
    let Some(kind_now) = hand.kind else {
        return;
    };

    // A stretch tool with its anchor down draws itself from the anchor
    // to the cursor and listens to nothing else.
    if let Some(axes) = kind_now.run_axes()
        && let Some(anchor) = hand.anchor
    {
        let Some(point) = cursor_point(&windows, &cameras, anchor.y) else {
            return;
        };
        let grid = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            16.0
        } else {
            16.0 / snap_grid.0 as f32
        };
        let mut to = Vec3::new(
            (point.x * grid).round() / grid,
            anchor.y,
            (point.z * grid).round() / grid,
        );
        // The drawn end answers the same magnets as any wall end: joint
        // crossings, wall ends and platform corners pull it off the
        // plain grid, so a stretched wall can actually MEET a seated
        // one instead of stopping a half-thickness short.
        let half_thick = WALL_THICK * 0.5;
        let mut stops: Vec<Vec3> = Vec::new();
        for (end, out) in wall_ends(&placed) {
            stops.push(end);
            stops.push(end - out * half_thick);
        }
        for platform in platform_rects(&placed) {
            let spin = Quat::from_rotation_y(platform.yaw);
            for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                stops.push(
                    platform.at + spin * Vec3::new(sx * platform.half.x, 0.0, sz * platform.half.y),
                );
            }
        }
        let mut best: Option<(f32, Vec3)> = None;
        for stop in stops {
            let gap = Vec2::new(stop.x - to.x, stop.z - to.z).length();
            if gap < 0.4 && best.as_ref().is_none_or(|(b, _)| gap < *b) {
                best = Some((gap, stop));
            }
        }
        if let Some((_, stop)) = best {
            to.x = stop.x;
            to.z = stop.z;
        }
        let reach = to - anchor;
        let (made, centre, yaw) = if axes == 1 {
            let on_x = reach.x.abs() >= reach.z.abs();
            let signed = if on_x { reach.x } else { reach.z };
            let long = signed.abs().max(0.25);
            let dir = if on_x {
                Vec3::X * signed.signum()
            } else {
                Vec3::Z * signed.signum()
            };
            (
                kind_now.run_made(long, 0.0),
                anchor + dir * (long * 0.5),
                if on_x {
                    0.0
                } else {
                    std::f32::consts::FRAC_PI_2
                },
            )
        } else {
            let w = reach.x.abs().max(0.25);
            let d = reach.z.abs().max(0.25);
            // R turns a whole roof a quarter, so the ridge can run the
            // other way over the same rectangle: the part is laid
            // crosswise and its two sides swap.
            let crossed = hand.yaw.rem_euclid(std::f32::consts::PI) > 0.7;
            let made = if crossed {
                kind_now.run_made(d, w)
            } else {
                kind_now.run_made(w, d)
            };
            (
                made,
                anchor + Vec3::new(w * 0.5 * reach.x.signum(), 0.0, d * 0.5 * reach.z.signum()),
                if crossed {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                },
            )
        };
        let record = Placed {
            part: part_name(&made),
            at: centre.into(),
            yaw,
            tilt: 0.0,
            ramp: hand.ramp.clone(),
            shade: hand.shade,
            stage: hand.stage.clone(),
            flip: hand.flip,
            loose: false,
            group: None,
        };
        // A whole roof draws as a flat plane while it is being sized -
        // the footprint it will cover - and becomes a roof when the
        // second click lands. Far easier to judge than two slopes
        // swinging about in the air.
        let shown = match made {
            PartKind::GableRoof(w, d, _, _) => PartKind::RoofPlan(w, d),
            other => other,
        };
        // Redraw only when the drawn size changed; otherwise carry the
        // ghost along.
        let stale = ghosts
            .iter()
            .next()
            .map(|(_, _, held)| held.part != record.part)
            .unwrap_or(true);
        if stale {
            for (ghost, _, _) in &ghosts {
                commands.entity(ghost).despawn();
            }
            spawn_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &shown,
                &record,
                true,
            );
        } else {
            for (_, mut transform, _) in &mut ghosts {
                transform.translation = centre;
                transform.rotation = Quat::from_rotation_y(yaw);
            }
        }
        return;
    }

    // Face-aware placement: the part clings to the face the cursor
    // points at. A side is clung to flush at the aimed course and is
    // final; the top of a thing seeds the position and then passes
    // through the magnets like any other placement, so a wall set down
    // on a foundation's top still seats flush to its edges.
    // A door or a window does not stand ON a wall, it stands IN one - and
    // it belongs to walls alone. Shown any other way the ghost clings to
    // whatever the cursor finds, roofs included, and leaps a metre the
    // instant the aim slips off the timber. It seats itself here with the
    // punch's own arithmetic, so what you see is where it goes.
    if let Some((wide, ..)) = opening_of(&kind_now) {
        let seat = hovered
            .build
            .filter(|hit| hit.normal.y.abs() < 0.3)
            .and_then(|hit| {
                let (_, wall_at, record, _) = placed.get(hit.entity).ok()?;
                let length = punchable_length(record)?;
                let along = Quat::from_rotation_y(record.yaw) * Vec3::X;
                let middle = opening_seat(wall_at.translation, along, length, wide, hit.point);
                Some((wall_at.translation + along * middle, record.yaw))
            });
        // Nothing punchable under the cursor: hold still. A ghost that
        // jumps to the ground whenever the aim wanders is worse than one
        // that waits where it was last wanted.
        let Some((seat, wall_yaw)) = seat else {
            return;
        };
        for (_, mut transform, _) in &mut ghosts {
            transform.translation = seat;
            transform.rotation = pose(wall_yaw, hand.tilt, hand.flip);
        }
        return;
    }

    let mut seeded: Option<Vec3> = None;
    if mode.face
        && let Some(hit) = hovered.build
    {
        if hit.normal.y > 0.7 {
            let per = 16.0 / snap_grid.0 as f32;
            seeded = Some(Vec3::new(
                (hit.point.x * per).round() / per,
                0.0,
                (hit.point.z * per).round() / per,
            ));
        } else if hit.normal.y.abs() < 0.3 {
            // My reach along the face's normal: how far my centre must
            // stand off so my body kisses the face.
            let mut low = Vec3::splat(f32::INFINITY);
            let mut high = Vec3::splat(f32::NEG_INFINITY);
            for Slab(at, size, ..) in body_of(&kind_now, None) {
                low = low.min(at - size * 0.5);
                high = high.max(at + size * 0.5);
            }
            let spin = Quat::from_rotation_y(hand.yaw);
            let mut reach = 0.0f32;
            for (cx, cz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = spin
                    * Vec3::new(
                        if cx < 0.0 { low.x } else { high.x },
                        0.0,
                        if cz < 0.0 { low.z } else { high.z },
                    );
                reach = reach.max(corner.dot(hit.normal).abs());
            }
            // Along the face: quarter-metre order. Up the face: courses
            // measured from the part's own base, so trim stacks in rings.
            let per = 16.0 / snap_grid.0 as f32;
            let tangent = Vec3::Y.cross(hit.normal).normalize_or_zero();
            let along = (hit.point.dot(tangent) * per).round() / per;
            let course = ((hit.point.y - hit.base_y).max(0.0) * per).round() / per + hit.base_y;
            let anchor = hit.point - tangent * hit.point.dot(tangent) + tangent * along;
            let snapped = Vec3::new(
                (anchor + hit.normal * reach).x,
                course + hand.lift,
                (anchor + hit.normal * reach).z,
            );
            for (_, mut transform, _) in &mut ghosts {
                transform.translation = snapped;
                transform.rotation = pose(hand.yaw, hand.tilt, hand.flip);
            }
            return;
        }
    }

    let mut snapped = match seeded {
        Some(seed) => seed,
        None => {
            let Some(point) = cursor_point(&windows, &cameras, hand.lift) else {
                return;
            };
            Vec3::ZERO + point
        }
    };
    // Quarter-metre snap by default; holding shift tightens the grid to
    // five centimetres for the odd exact nestling.
    if seeded.is_none() {
        let grid = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            16.0
        } else {
            16.0 / snap_grid.0 as f32
        };
        snapped = Vec3::new(
            (snapped.x * grid).round() / grid,
            0.0,
            (snapped.z * grid).round() / grid,
        );
    }

    let kind = kind_now;

    // Walls click to wall ends - a butt joint or a square corner - and
    // the corner pole magnetizes to the same points it exists to cover.
    let magnetic = matches!(kind, PartKind::Wall(_))
        || kind == PartKind::Prop("pole")
        || kind.run_axes() == Some(1);
    if magnetic {
        let mut ends = wall_ends(&placed);
        let platforms = platform_rects(&placed);
        // The pole magnetizes to centreline crossings at platform corners
        // - the exact point two flush walls meet - alongside wall ends.
        if kind == PartKind::Prop("pole") || kind.run_axes() == Some(1) {
            for platform in &platforms {
                let spin = Quat::from_rotation_y(platform.yaw);
                for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    ends.push((
                        platform.at
                            + spin
                                * Vec3::new(
                                    sx * (platform.half.x - PLINTH_REVEAL),
                                    0.0,
                                    sz * (platform.half.y - PLINTH_REVEAL),
                                ),
                        Vec3::ZERO,
                    ));
                }
            }
        }
        let my_dir = Quat::from_rotation_y(hand.yaw) * Vec3::X;
        let my_ends: Vec<(Vec3, Vec3)> = match kind {
            PartKind::Wall(long) => {
                vec![
                    (snapped + my_dir * (long * 0.5), my_dir),
                    (snapped - my_dir * (long * 0.5), -my_dir),
                ]
            }
            _ => vec![(snapped, Vec3::ZERO)],
        };
        let half_thick = WALL_THICK * 0.5;
        let mut pull: Option<(f32, Vec3)> = None;
        for (mine, my_out) in &my_ends {
            for (theirs, their_out) in &ends {
                // The joint decides the target. Perpendicular tips overlap
                // into a full corner block, outer faces flush both ways; a
                // continuation meets end to end; a pole takes the
                // centreline crossing itself.
                let target = if *my_out == Vec3::ZERO {
                    *theirs - *their_out * half_thick
                } else if my_out.dot(*their_out).abs() < 0.35 {
                    *theirs - *their_out * half_thick + *my_out * half_thick
                } else {
                    *theirs
                };
                let gap = Vec3::new(target.x - mine.x, 0.0, target.z - mine.z);
                let reach = gap.length();
                if reach < 0.4 && pull.as_ref().is_none_or(|(best, _)| reach < *best) {
                    pull = Some((reach, gap));
                }
            }
        }
        if let Some((_, gap)) = pull {
            snapped += gap;
        } else if let PartKind::Wall(my_len) = kind {
            // No wall end took hold. A wall running parallel to a platform
            // edge seats flush onto it - outer face to the stone's face -
            // and platform corners then slide it ALONG its line only, so
            // the flush seat is never yanked sideways.
            let mut best: Option<(f32, Vec3)> = None;
            for platform in &platforms {
                let spin = Quat::from_rotation_y(platform.yaw);
                let faces = [
                    (
                        spin * Vec3::X,
                        spin * Vec3::Z,
                        platform.half.y,
                        platform.half.x,
                    ),
                    (
                        spin * Vec3::Z,
                        spin * Vec3::X,
                        platform.half.x,
                        platform.half.y,
                    ),
                ];
                for (along_edge, outward, half_out, half_along) in faces {
                    if my_dir.dot(along_edge).abs() < 0.92 {
                        continue;
                    }
                    for side in [-1.0f32, 1.0] {
                        let line = platform.at + outward * side * (half_out - PLINTH_REVEAL);
                        let offset = snapped - line;
                        let across = offset.dot(outward);
                        let along = offset.dot(along_edge);
                        if across.abs() < 0.45
                            && along.abs() < half_along + 0.3
                            && best.as_ref().is_none_or(|(b, _)| across.abs() < *b)
                        {
                            best = Some((across.abs(), outward * -across));
                        }
                    }
                }
            }
            if let Some((_, shift)) = best {
                snapped += shift;
                // Corner slide: my nearest end walks along my line to the
                // platform corner's projection, and no further than that.
                let mut slide: Option<(f32, f32)> = None;
                for platform in &platforms {
                    let spin = Quat::from_rotation_y(platform.yaw);
                    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                        let corner = platform.at
                            + spin * Vec3::new(sx * platform.half.x, 0.0, sz * platform.half.y);
                        for end_sign in [-1.0f32, 1.0] {
                            let my_end = snapped + my_dir * (end_sign * my_len * 0.5);
                            let to_corner = corner - my_end;
                            let along = to_corner.dot(my_dir);
                            let sideways = (to_corner - my_dir * along).length();
                            if along.abs() < 0.4
                                && sideways < 0.45
                                && slide.as_ref().is_none_or(|(b, _)| along.abs() < *b)
                            {
                                slide = Some((along.abs(), along));
                            }
                        }
                    }
                }
                if let Some((_, along)) = slide {
                    snapped += my_dir * along;
                }
            }
        }
    }

    // Whatever lies beneath carries the part; Q and E add height on top
    // of that, so a roof panel rides the wall tops on its own.
    let samples = footprint_samples(&kind, snapped, hand.yaw);
    let support = support_height(&placed, &samples, is_structure(&kind), None);
    // A tilted part rests its DOWNHILL EDGE on what carries it and
    // rises from there - a pitched panel's eave sits on the wall plate
    // instead of half the slope swinging down into the room.
    let mut eave = 0.0;
    if hand.tilt.abs() > 0.001 {
        let mut deep = 0.0f32;
        for Slab(at, size, ..) in body_of(&kind, None) {
            deep = deep.max((at.z.abs() + size.z * 0.5) * 2.0);
        }
        eave = deep * 0.5 * hand.tilt.abs().sin();
    }
    snapped.y = support + hand.lift + eave;

    for (_, mut transform, _) in &mut ghosts {
        transform.translation = snapped;
        transform.rotation = pose(hand.yaw, hand.tilt, hand.flip);
    }
}

/// What the cursor's ray touched first: the part, where, and through
/// which face - the face is what placement clings to.
#[derive(Clone, Copy)]
pub struct Hit {
    /// Which part was struck - unread today, but the grab and future
    /// tools (paint-by-face, measure) will want to know.
    #[allow(dead_code)]
    pub entity: Entity,
    pub point: Vec3,
    pub normal: Vec3,
    pub base_y: f32,
}

/// The cursor's findings, shared by the glow, the grab and the ghost:
/// `grab` is the first thing touched (widgets included), `build` the
/// first solid face a part could cling to.
#[derive(Resource, Default)]
pub struct Hovered {
    pub grab: Option<Entity>,
    pub build: Option<Hit>,
}

/// Whether placement clings to the face under the cursor, or ignores
/// faces and works the ground plane alone. F walks between them.
#[derive(Resource)]
pub struct SnapMode {
    pub face: bool,
}

impl Default for SnapMode {
    fn default() -> Self {
        SnapMode { face: true }
    }
}

/// The placement grid's step, in atoms. G cycles it; shift always
/// drops to a single atom while held.
#[derive(Resource)]
pub struct SnapGrid(pub i32);

impl Default for SnapGrid {
    fn default() -> Self {
        SnapGrid(4)
    }
}

/// The shelf line that says which mode the hand is in.
#[derive(Component)]
struct SnapModeText;

/// Exact dimensions being typed for the selected part, while the card
/// is up. Every other key on the bench holds its tongue.
#[derive(Resource, Default)]
pub struct DimsEntry(pub Option<String>);

/// The dimensions card at the window's foot and the text inside it.
#[derive(Component)]
pub(crate) struct DimsCard;

#[derive(Component)]
pub(crate) struct DimsText;

#[allow(clippy::type_complexity)]
fn ray_scan(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    placed: &Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
) -> (Option<Entity>, Option<Hit>) {
    let Some(ray) = windows
        .iter()
        .next()
        .and_then(|window| window.cursor_position())
        .and_then(|cursor| {
            let (camera, camera_at) = cameras.iter().next()?;
            camera.viewport_to_world(camera_at, cursor).ok()
        })
    else {
        return (None, None);
    };

    // First thing touched at all (the grab), and first SOLID face (the
    // build target) - widgets are markers, not masonry.
    let mut first_any: Option<(Entity, f32)> = None;
    let mut first_solid: Option<(f32, Hit)> = None;
    for (entity, transform, record, showing) in placed {
        // A wall the cutaway has taken away holds nothing up and
        // catches nothing: what you cannot see, you cannot build on.
        if *showing == Visibility::Hidden {
            continue;
        }
        let Some(kind) = kind_from_name(&record.part) else {
            continue;
        };
        let spin = Quat::from_rotation_y(record.yaw) * Quat::from_rotation_x(record.tilt);
        let inverse = spin.inverse();
        let origin = inverse * (ray.origin - transform.translation);
        let toward = inverse * Vec3::from(ray.direction);
        let repaint = record.ramp.as_deref().map(|r| (r, record.shade));
        for Slab(at, size, ..) in body_of(&kind, repaint) {
            let low = at - size * 0.5;
            let high = at + size * 0.5;
            let mut enter = f32::NEG_INFINITY;
            let mut leave = f32::INFINITY;
            let mut face = Vec3::Y;
            let mut missed = false;
            for axis in 0..3 {
                let (o, d, lo, hi) = (origin[axis], toward[axis], low[axis], high[axis]);
                if d.abs() < 1e-6 {
                    if o < lo || o > hi {
                        missed = true;
                        break;
                    }
                    continue;
                }
                let a = (lo - o) / d;
                let b = (hi - o) / d;
                let near = a.min(b);
                if near > enter {
                    enter = near;
                    let mut normal = Vec3::ZERO;
                    normal[axis] = -toward[axis].signum();
                    face = normal;
                }
                leave = leave.min(a.max(b));
            }
            if missed || enter > leave || leave < 0.0 {
                continue;
            }
            let reach = enter.max(0.0);
            if first_any.is_none_or(|(_, t)| reach < t) {
                first_any = Some((entity, reach));
            }
            if !matches!(kind, PartKind::Widget(_))
                && first_solid.as_ref().is_none_or(|(t, _)| reach < *t)
            {
                first_solid = Some((
                    reach,
                    Hit {
                        entity,
                        point: ray.get_point(reach),
                        normal: (spin * face).normalize_or_zero(),
                        base_y: transform.translation.y,
                    },
                ));
            }
        }
    }
    (
        first_any.map(|(entity, _)| entity),
        first_solid.map(|(_, hit)| hit),
    )
}

/// Keeps the hovered part known, and lights it softly gold while the
/// hand is empty - what glows is what a click will take.
#[allow(clippy::too_many_arguments)]
fn feel_ahead(
    bench: Res<Bench>,
    naming: Res<Naming>,
    tool: Res<crate::gizmo::ToolMode>,
    hand: Res<Hand>,
    mut hovered: ResMut<Hovered>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    placed: Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
    hovers: Query<&Interaction>,
    children: Query<&Children>,
    slabs: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let over_ui = hovers
        .iter()
        .any(|interaction| *interaction != Interaction::None);
    let (fresh, build) = if *bench == Bench::Builder && naming.0.is_none() && !over_ui {
        ray_scan(&windows, &cameras, &placed)
    } else {
        (None, None)
    };
    hovered.build = build;
    if fresh == hovered.grab {
        return;
    }
    // The old glow goes out; the new one comes on only for an empty hand.
    let glow = |materials: &mut Assets<StandardMaterial>,
                children: &Query<&Children>,
                slabs: &Query<&MeshMaterial3d<StandardMaterial>>,
                part: Entity,
                lit: bool| {
        let Ok(kids) = children.get(part) else {
            return;
        };
        for &kid in kids {
            if let Ok(handle) = slabs.get(kid)
                && let Some(mut material) = materials.get_mut(&handle.0)
            {
                material.emissive = if lit {
                    LinearRgba::new(0.14, 0.11, 0.04, 1.0)
                } else {
                    LinearRgba::BLACK
                };
            }
        }
    };
    if let Some(old) = hovered.grab
        && placed.contains(old)
    {
        glow(&mut materials, &children, &slabs, old, false);
    }
    if let Some(new) = fresh
        && (hand.kind.is_none() || *tool != crate::gizmo::ToolMode::Normal)
    {
        glow(&mut materials, &children, &slabs, new, true);
    }
    hovered.grab = fresh;
}

/// A full hand places on click. An empty hand picks a placed part back up.
/// X removes what the cursor touches either way.
#[allow(clippy::too_many_arguments)]
fn place_grab_remove(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    hovered: Res<Hovered>,
    // Bundled: Bevy's parameter ceiling is sixteen, and this system
    // presses it.
    gizmo: (
        Res<crate::gizmo::GizmoHot>,
        Res<crate::gizmo::Selected>,
        Res<crate::gizmo::ToolMode>,
    ),
    mut hand: ResMut<Hand>,
    palette: Res<Palette>,
    ghosts: Query<Entity, With<Ghost>>,
    ghost_spots: Query<(&Transform, &Placed), With<Ghost>>,
    placed: Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
    hovers: Query<&Interaction>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (gizmo_hot, selected, tool) = gizmo;
    if *bench != Bench::Builder
        || naming.0.is_some()
        || gizmo_hot.0
        || !selected.is_empty()
        || *tool != crate::gizmo::ToolMode::Normal
    {
        return;
    }
    // A click that lands on UI is the UI's business.
    let over_ui = hovers
        .iter()
        .any(|interaction| *interaction != Interaction::None);

    if buttons.just_pressed(MouseButton::Left) && !over_ui {
        if let Some(kind) = hand.kind {
            // A stretch tool: the first click sets the anchor where the
            // stub stands; the next makes the drawn part real. A wall run
            // chains - the far end becomes the next anchor - while rects
            // rest after each one.
            if kind.run_axes().is_some() {
                if hand.anchor.is_none() {
                    if let Some((ghost_at, _)) = ghost_spots.iter().next() {
                        hand.anchor = Some(ghost_at.translation);
                    }
                } else if let Some((ghost_at, drawn)) = ghost_spots.iter().next()
                    && let Some(made) = kind_from_name(&drawn.part)
                {
                    spawn_part(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &palette,
                        &made,
                        drawn,
                        false,
                    );
                    hand.anchor = if kind.run_axes() == Some(1) {
                        hand.anchor
                            .map(|anchor| ghost_at.translation * 2.0 - anchor)
                    } else {
                        None
                    };
                }
                return;
            }
            // Doors and windows would rather punch through a wall than
            // stand alone: if one lands on a wall, the wall parts around
            // the opening and the frame settles in.
            let opening = opening_of(&kind);
            let punched = if let Some((wide, head, sill, is_door)) = opening
                && let Some((ghost_at, _)) = ghost_spots.iter().next()
            {
                punch_wall(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &palette,
                    &placed,
                    hovered.build.map(|hit| (hit.entity, hit.point, hit.normal)),
                    ghost_at.translation,
                    wide,
                    head,
                    sill,
                    is_door,
                    &hand,
                )
            } else {
                false
            };
            // Setting down (a punch already set the frame itself).
            if !punched
                && let Some((ghost_at, _)) = ghost_spots.iter().next()
                && let Some(record) = hand.record(ghost_at.translation)
            {
                spawn_part(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &palette,
                    &kind,
                    &record,
                    false,
                );
                seat_the_figures(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &palette,
                    &kind,
                    &record,
                );
            }
        } else if let Some(grabbed) = hovered.grab
            && let Ok((_, transform, record, _)) = placed.get(grabbed)
            && let Some(kind) = kind_from_name(&record.part)
        {
            // An opening picked up closes the wall behind it.
            heal_wall(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &placed,
                grabbed,
            );
            // Picking back up: the part leaves the floor and rides the
            // cursor again with its paint and turn intact. Only the height
            // ABOVE its old support comes along - the new resting place
            // supplies its own.
            let beneath = support_height(
                &placed,
                &footprint_samples(&kind, transform.translation, record.yaw),
                is_structure(&kind),
                Some(grabbed),
            );
            *hand = Hand {
                kind: Some(kind),
                anchor: None,
                flip: record.flip,
                stage: record.stage.clone(),
                yaw: record.yaw,
                tilt: record.tilt,
                lift: (transform.translation.y - beneath).max(0.0),
                ramp: record.ramp.clone(),
                shade: record.shade,
            };
            commands.entity(grabbed).despawn();
            dress_ghost(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &hand,
                &ghosts,
            );
        }
    }

    if (keys.just_pressed(KeyCode::KeyX)
        || keys.just_pressed(KeyCode::Delete)
        || keys.just_pressed(KeyCode::Backspace))
        && let Some(doomed) = hovered.grab
        && placed.contains(doomed)
    {
        // A removed opening leaves the wall whole again.
        heal_wall(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &placed,
            doomed,
        );
        commands.entity(doomed).despawn();
    }
}

/// Taking an opening out of a wall closes the wall back up: the pieces
/// the punch left - the sides, the header, a window's sill - merge into
/// one whole wall again, and a door's routing widget goes with it.
fn heal_wall(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
    placed: &Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
    frame: Entity,
) -> bool {
    let Ok((_, frame_at, _, _)) = placed.get(frame) else {
        return false;
    };
    let spot = frame_at.translation;
    heal_wall_at(commands, meshes, materials, palette, placed, frame, spot)
}

/// The same closing, at a spot the frame may since have left - a door
/// dragged along its wall heals the hole it came from.
#[allow(clippy::too_many_arguments)]
fn heal_wall_at(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
    placed: &Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
    frame: Entity,
    spot: Vec3,
) -> bool {
    let Ok((_, _, frame_record, _)) = placed.get(frame) else {
        return false;
    };
    // Through `opening_of`, not a literal beside it. This was written as a
    // bare 1.25 - correct while every opening in the shelf happened to be one
    // and a quarter wide, and wrong the moment a double door existed.
    let Some(width) = kind_from_name(&frame_record.part)
        .and_then(|kind| opening_of(&kind))
        .map(|(wide, ..)| wide)
    else {
        return false;
    };
    let along = Quat::from_rotation_y(frame_record.yaw) * Vec3::X;
    let base = spot;

    // Everything standing on this wall's own line, measured along it.
    let mut doomed: Vec<Entity> = Vec::new();
    let mut low = -width * 0.5;
    let mut high = width * 0.5;
    let mut cloth: Option<Placed> = None;
    for (entity, transform, record, showing) in placed {
        // A wall the cutaway has taken away holds nothing up and
        // catches nothing: what you cannot see, you cannot build on.
        if *showing == Visibility::Hidden {
            continue;
        }
        if entity == frame {
            continue;
        }
        let offset = transform.translation - base;
        if (offset.y).abs() > 0.05 {
            continue;
        }
        let reach = offset.dot(along);
        if (offset - along * reach).length() > 0.2 {
            continue;
        }
        // The door's own widget rides along.
        if matches!(kind_from_name(&record.part), Some(PartKind::Widget("door")))
            && reach.abs() < 0.2
        {
            doomed.push(entity);
            continue;
        }
        let Some(kind) = kind_from_name(&record.part) else {
            continue;
        };
        let facing = Quat::from_rotation_y(record.yaw) * Vec3::X;
        if facing.dot(along).abs() < 0.99 {
            continue;
        }
        let (long, full) = match kind {
            PartKind::Wall(long) => (long, true),
            PartKind::Seg { long, high, lift } => {
                (long, lift.abs() < 0.01 && (high - WALL_HIGH).abs() < 0.05)
            }
            _ => continue,
        };
        let (piece_low, piece_high) = (reach - long * 0.5, reach + long * 0.5);
        let fills_opening = reach.abs() < 0.1 && !full;
        let touches_left = (piece_high - low).abs() < 0.1 && full;
        let touches_right = (piece_low - high).abs() < 0.1 && full;
        if !(fills_opening || touches_left || touches_right) {
            continue;
        }
        doomed.push(entity);
        low = low.min(piece_low);
        high = high.max(piece_high);
        if full {
            cloth = Some(record.clone());
        }
    }

    let dressed = cloth.unwrap_or_else(|| frame_record.clone());
    let made = PartKind::Wall(((high - low) * 16.0).round() / 16.0);
    let centre = base + along * ((low + high) * 0.5);
    let whole = Placed {
        part: part_name(&made),
        at: centre.into(),
        yaw: frame_record.yaw,
        tilt: 0.0,
        ramp: dressed.ramp.clone(),
        shade: dressed.shade,
        stage: "walls".to_string(),
        flip: false,
        loose: false,
        group: None,
    };
    for piece in doomed {
        commands.entity(piece).despawn();
    }
    spawn_part(commands, meshes, materials, palette, &made, &whole, false);
    true
}

/// Splits the nearest wall around an opening and sets the frame in it.
/// Returns false when no wall stands close enough to take the punch.
#[allow(clippy::too_many_arguments)]
/// A wall the punch may part: pristine, or a full-height leaving from
/// an earlier punch - a second window in the same run is honest work.
/// The hole a part cuts in a wall: how wide, how high its head, how far
/// its sill stands off the floor, and whether a routing widget comes with
/// it. One table, read both by the ghost that SHOWS the placement and by
/// the punch that makes it - they each had their own copy once, and the
/// ghost drifted off onto the roof while the door went into the wall.
pub fn opening_of(kind: &PartKind) -> Option<(f32, f32, f32, bool)> {
    match kind {
        PartKind::Prop("door") => Some((1.25, 2.125, 0.0, true)),
        // Twice the leaf, so twice the hole.
        PartKind::Prop("door-double") => Some((2.25, 2.125, 0.0, true)),
        // A bare doorway needs no widget: the gap itself is the portal,
        // and a widget would only say it twice.
        PartKind::Prop("doorway") => Some((1.25, 2.125, 0.0, false)),
        PartKind::Prop("window") => Some((1.25, 2.0, 0.75, false)),
        _ => None,
    }
}

/// Where the routing widgets stand in an opening, measured along the wall from
/// its middle: one lane per leaf.
///
/// Brett's idea, and it needs nothing new anywhere else: the game reads EVERY
/// mark called "door" into a building's list of doorways and steers each walker
/// to the nearest one, so two marks a metre apart in one opening are two lanes,
/// and two villagers meeting at a double door take one each instead of queueing
/// through the same point. The part that knows it has two leaves is the part that
/// should say where they are.
pub fn door_lanes(kind: &PartKind) -> &'static [f32] {
    match kind {
        // One lane per leaf, each on its own leaf's centre.
        PartKind::Prop("door-double") => &[-0.5, 0.5],
        _ => &[0.0],
    }
}

/// Where along a wall an opening aimed at `point` actually lands: on the
/// lattice, and never spilling past either end.
pub fn opening_seat(wall_at: Vec3, along: Vec3, length: f32, wide: f32, point: Vec3) -> f32 {
    let half = length * 0.5;
    let reach = (half - wide * 0.5).max(0.0);
    let t = (point - wall_at).dot(along).clamp(-reach, reach);
    ((t * 16.0).round() / 16.0).clamp(-reach, reach)
}

fn punchable_length(record: &Placed) -> Option<f32> {
    match kind_from_name(&record.part)? {
        PartKind::Wall(long) => Some(long),
        PartKind::Seg { long, high, lift }
            if lift.abs() < 0.01 && (high - WALL_HIGH).abs() < 0.05 =>
        {
            Some(long)
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn punch_wall(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &Palette,
    placed: &Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
    aimed: Option<(Entity, Vec3, Vec3)>,
    at: Vec3,
    wide: f32,
    head: f32,
    sill: f32,
    is_door: bool,
    hand: &Hand,
) -> bool {
    // The wall the cursor's own ray touches wins outright; the search
    // by proximity is the fallback for a blind click.
    let mut best: Option<(Entity, f32, Vec3, f32, f32, Placed)> = None;
    if let Some((touched, point, _)) = aimed
        && let Ok((entity, transform, record, _)) = placed.get(touched)
        && let Some(length) = punchable_length(record)
    {
        let along = Quat::from_rotation_y(record.yaw) * Vec3::X;
        let t = (point - transform.translation).dot(along);
        if t.abs() <= length * 0.5 {
            best = Some((entity, 0.0, along, t, length, record.clone()));
        }
    }
    if best.is_none() {
        for (entity, transform, record, showing) in placed {
            // A wall the cutaway has taken away holds nothing up and
            // catches nothing: what you cannot see, you cannot build on.
            if *showing == Visibility::Hidden {
                continue;
            }
            let Some(length) = punchable_length(record) else {
                continue;
            };
            let along = Quat::from_rotation_y(record.yaw) * Vec3::X;
            let from_centre = at - transform.translation;
            let t = from_centre.dot(along);
            let sideways = (from_centre - along * t).length();
            if sideways > 0.5 || t.abs() > length * 0.5 {
                continue;
            }
            if best.as_ref().is_none_or(|(_, s, ..)| sideways < *s) {
                best = Some((entity, sideways, along, t, length, record.clone()));
            }
        }
    }
    let Some((wall, _, along, t, length, record)) = best else {
        return false;
    };

    // The opening, on the lattice and clamped so it never spills past the
    // wall's ends. The ghost seats itself with this same arithmetic.
    let half = length * 0.5;
    let wall_at = placed
        .get(wall)
        .map(|(_, tf, _, _)| tf.translation)
        .unwrap_or(at);
    let middle = opening_seat(wall_at, along, length, wide, wall_at + along * t);
    let centre_of = |offset: f32| {
        let base = placed
            .get(wall)
            .map(|(_, tf, _, _)| tf.translation)
            .unwrap_or(at);
        base + along * offset
    };

    let mut leavings: Vec<(PartKind, Vec3)> = Vec::new();
    let left = middle - wide * 0.5 + half;
    if left > 0.06 {
        leavings.push((
            PartKind::Seg {
                long: left,
                high: WALL_HIGH,
                lift: 0.0,
            },
            centre_of(-half + left * 0.5),
        ));
    }
    let right = half - (middle + wide * 0.5);
    if right > 0.06 {
        leavings.push((
            PartKind::Seg {
                long: right,
                high: WALL_HIGH,
                lift: 0.0,
            },
            centre_of(half - right * 0.5),
        ));
    }
    if WALL_HIGH - head > 0.06 {
        leavings.push((
            PartKind::Seg {
                long: wide,
                high: WALL_HIGH - head,
                lift: head,
            },
            centre_of(middle),
        ));
    }
    if sill > 0.06 {
        leavings.push((
            PartKind::Seg {
                long: wide,
                high: sill,
                lift: 0.0,
            },
            centre_of(middle),
        ));
    }

    let base = placed
        .get(wall)
        .map(|(_, tf, _, _)| tf.translation)
        .unwrap_or(at);
    commands.entity(wall).despawn();
    for (kind, spot) in leavings {
        let piece = Placed {
            part: part_name(&kind),
            at: spot.into(),
            yaw: record.yaw,
            tilt: 0.0,
            ramp: record.ramp.clone(),
            shade: record.shade,
            stage: record.stage.clone(),
            flip: false,
            loose: false,
            group: None,
        };
        spawn_part(commands, meshes, materials, palette, &kind, &piece, false);
    }

    // The frame takes the wall's own line and turn.
    // The hand knows which opening it holds; `is_door` only decides
    // whether a routing widget rides along.
    let frame_kind = hand.kind.unwrap_or(PartKind::Prop("window"));
    // The frame keeps the wall's own footing - a door in a wall on a
    // foundation stands on the foundation, not sunk to the ground.
    let frame_at = base + along * middle;
    let frame = Placed {
        part: part_name(&frame_kind),
        at: [frame_at.x, base.y, frame_at.z],
        yaw: record.yaw,
        tilt: 0.0,
        ramp: hand.ramp.clone(),
        shade: hand.shade,
        stage: "walls".to_string(),
        flip: hand.flip,
        loose: false,
        group: None,
    };
    spawn_part(
        commands,
        meshes,
        materials,
        palette,
        &frame_kind,
        &frame,
        false,
    );

    // A door is a doorway: the routing widget arrives with it, its nose
    // pointing OUT through the opening - the way you were looking when
    // you punched it, since that is the side you were standing on.
    if is_door {
        let widget = PartKind::Widget("door");
        let outward = aimed
            .map(|(_, _, normal)| Vec3::new(normal.x, 0.0, normal.z))
            .filter(|flat| flat.length() > 0.1)
            .map(|flat| flat.normalize())
            .unwrap_or_else(|| Vec3::Y.cross(along).normalize_or_zero());
        // A widget's nose is its local +X.
        let facing = (-outward.z).atan2(outward.x);
        // One per leaf: a double door gets two, so two people can use it at
        // once. See [`door_lanes`].
        for lane in door_lanes(&frame_kind) {
            let stands = frame_at + along * *lane;
            let mark = Placed {
                part: part_name(&widget),
                at: [stands.x, base.y, stands.z],
                yaw: facing,
                tilt: 0.0,
                ramp: None,
                shade: 0.7,
                stage: "widget".to_string(),
                flip: false,
                loose: false,
                group: None,
            };
            spawn_part(commands, meshes, materials, palette, &widget, &mark, false);
        }
    }
    true
}

// ---------------------------------------------------------------- the file

#[derive(Serialize, Deserialize, Default)]
struct Workbench {
    format: u32,
    name: String,
    /// A work from before stages: one flat list, which is the finished
    /// building. Kept readable forever - a maker's work is not something to
    /// lose to a format change - and turned into stages on the way in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    parts: Vec<Placed>,
    /// One COMPLETE drawing per step of the build.
    ///
    /// Complete, and not a set of additions: raising step two clears step one
    /// off the ground and puts step two there instead. Brett's call, and the
    /// reason is authoring rather than rendering — "replacing the building each
    /// stage allows me to be more creative during the stages". A frame drawn at
    /// step one is a PICTURE of a frame, and by the time the walls are up it
    /// should be gone, because the walls are solid boxes that never needed it.
    /// Stages that accumulate would make that the awkward case; stages that
    /// replace make it the ordinary one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stages: Vec<Vec<Placed>>,
}

/// Every step of the work, and which one the bench is showing.
///
/// Only the shown step exists as entities; the rest are records waiting their
/// turn. Switching gathers the standing parts back into their step and sets the
/// next one out, which is why a stage IS the bench rather than a filter over it.
#[derive(Resource)]
pub struct Stages {
    drawings: Vec<Vec<Placed>>,
    showing: usize,
}

impl Default for Stages {
    fn default() -> Self {
        Stages {
            drawings: vec![Vec::new()],
            showing: 0,
        }
    }
}

impl Stages {
    pub fn count(&self) -> usize {
        self.drawings.len()
    }

    pub fn showing(&self) -> usize {
        self.showing
    }
}

/// Turns a work from before stages into stages, by the rule the game used to
/// infer them with.
///
/// Which makes the change invisible where it should be: an old building comes
/// back with exactly the steps the village was already raising it in. The rule
/// is `step_of`, and the last stage is the whole finished work.
fn stages_from_flat(parts: &[Placed]) -> Vec<Vec<Placed>> {
    if parts.is_empty() {
        return vec![Vec::new()];
    }
    let framed = parts.iter().any(|record| record.stage == "frame");
    let steps = if framed { 4 } else { 3 };
    (0..steps)
        .map(|step| {
            parts
                .iter()
                .filter(|record| {
                    // The maker's own marks belong to the finished work, and
                    // stand throughout it besides.
                    record.stage == "widget" || step_of(&record.stage, framed) <= step as u8
                })
                .cloned()
                .collect()
        })
        .collect()
}

/// What a saved building is called on disk.
///
/// `.baz`, for the studio whose bench this is. `.json` said only what these are
/// written IN, which is true of a great many files and tells a maker looking at
/// a folder nothing at all; this says whose they are and what made them.
///
/// The contents are still JSON and always were - this is a name, not a format.
pub const WORK_KIND: &str = "baz";

/// Whether a file is a saved work: the name it wears now, or the one it wore
/// before the name changed.
///
/// Both, forever. A maker's buildings are not something to lose to a rename, and
/// the ones already on disk are the only two that exist.
#[cfg_attr(not(test), allow(dead_code))]
fn is_a_work(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|kind| kind == WORK_KIND || kind == "json")
}

/// Where the bench keeps everything: its own folder in a source tree, and the
/// maker's own Application Support beside the game's saves otherwise.
///
/// It used to be `CARGO_MANIFEST_DIR` or, failing that, the working directory —
/// which was right while the only way to run the bench was `cargo run` from its
/// own crate. It is opened from the game's title screen now, and a bundled bench
/// would have taken "the working directory" to mean INSIDE the `.app`: a place
/// that is read-only where it is installed properly, and that breaks the
/// signature where it is not.
///
/// A maker working in the tree still writes to `atelier/out/buildings`, which is
/// where their buildings already are and where git can see them.
pub(crate) fn bench_home() -> std::path::PathBuf {
    if let Ok(tree) = std::env::var("CARGO_MANIFEST_DIR") {
        return std::path::PathBuf::from(tree);
    }
    // Beside the game's saves, under the same roof the launcher already uses
    // for this game's things.
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let base = if cfg!(target_os = "macos") {
        format!("{home}/Library/Application Support/Divus Factus/atelier")
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(|roaming| format!("{roaming}/Divus Factus/atelier"))
            .unwrap_or_else(|_| "atelier".into())
    } else {
        format!("{home}/.local/share/divus-factus/atelier")
    };
    std::path::PathBuf::from(base)
}

fn bench_path() -> std::path::PathBuf {
    bench_home().join(format!("out/buildings/workbench.{WORK_KIND}"))
}

/// Where the works are kept: the folder the bench saves into and loads from.
fn works_home() -> std::path::PathBuf {
    bench_path()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(bench_home)
}

/// The save button asks the work its name; the writing happens when the
/// name is given, in [`take_the_name`].
fn save_workbench(
    mut commands: Commands,
    bench: Res<Bench>,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
    work_name: Res<WorkName>,
    mut naming: ResMut<Naming>,
    saves: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
) {
    // The glyphs are one row for both benches; what they save is not.
    if *bench != Bench::Builder {
        return;
    }
    let pressed = saves
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed);
    if pressed && naming.0.is_none() {
        naming.0 = Some(work_name.0.clone().unwrap_or_default());
        raise_naming_card(&mut commands, &fonts, &palette);
    }
}

/// The card that asks for the work's name.
fn raise_naming_card(commands: &mut Commands, fonts: &Fonts, palette: &Palette) {
    let card = commands
        .spawn((
            NamingCard,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(40.0),
                margin: UiRect {
                    left: Val::Px(-170.0),
                    ..default()
                },
                width: Val::Px(340.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::accent(palette).with_alpha(0.7)),
            GlobalZIndex(50),
        ))
        .id();
    commands.spawn((
        Text::new("NAME THE WORK"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(theme::accent(palette)),
        ChildOf(card),
    ));
    commands.spawn((
        NameText,
        Text::new("_"),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(theme::text(palette)),
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            min_width: Val::Px(220.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.35)),
        BorderColor::all(theme::panel_border(palette)),
        ChildOf(card),
    ));
    commands.spawn((
        Text::new("enter saves - esc thinks better of it"),
        TextFont {
            font: fonts.text.clone().into(),
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(theme::text_dim(palette).with_alpha(0.8)),
        ChildOf(card),
    ));
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
            ChildOf(card),
        ))
        .id();
    for (label, accent) in [("SAVE", true), ("CANCEL", false)] {
        let button = commands
            .spawn((
                Interaction::default(),
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.18)),
                BorderColor::all(if accent {
                    theme::accent(palette).with_alpha(0.7)
                } else {
                    theme::panel_border(palette)
                }),
                ChildOf(row),
            ))
            .id();
        if accent {
            commands.entity(button).insert(NamingSave);
        } else {
            commands.entity(button).insert(NamingCancel);
        }
        commands.spawn((
            Text::new(label),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(if accent {
                theme::accent(palette)
            } else {
                theme::text_dim(palette)
            }),
            ChildOf(button),
        ));
    }
}

/// Typing while the card is up: letters, digits and dashes build the name,
/// enter writes the file, escape puts the pen down.
#[allow(clippy::too_many_arguments)]
fn take_the_name(
    mut commands: Commands,
    stages: Res<Stages>,
    mut keystrokes: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut naming: ResMut<Naming>,
    time: Res<Time>,
    mut work_name: ResMut<WorkName>,
    placed: Query<&Placed, Without<Ghost>>,
    cards: Query<Entity, With<NamingCard>>,
    saves_click: Query<&Interaction, (Changed<Interaction>, With<NamingSave>)>,
    cancels_click: Query<&Interaction, (Changed<Interaction>, With<NamingCancel>)>,
    mut shown: Query<&mut Text, With<NameText>>,
    mut save_labels: Query<(Entity, &mut Text), (With<SaveLabel>, Without<NameText>)>,
) {
    let Some(name) = naming.0.as_mut() else {
        return;
    };
    use bevy::input::keyboard::Key;
    let mut done: Option<bool> = None;
    for stroke in keystrokes.read() {
        if !stroke.state.is_pressed() {
            continue;
        }
        match &stroke.logical_key {
            Key::Character(text) => {
                for letter in text.chars() {
                    let letter = letter.to_ascii_lowercase();
                    if (letter.is_ascii_alphanumeric() || letter == '-') && name.len() < 24 {
                        name.push(letter);
                    }
                }
            }
            Key::Space => {
                if name.len() < 24 && !name.is_empty() {
                    name.push('-');
                }
            }
            Key::Backspace => {
                name.pop();
            }
            Key::Enter => done = Some(true),
            Key::Escape => done = Some(false),
            _ => {}
        }
    }
    if saves_click
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        done = Some(true);
    }
    if cancels_click
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        done = Some(false);
    }
    for mut text in &mut shown {
        let fresh = format!("{name}_");
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
    let Some(saving) = done else {
        return;
    };
    if saving {
        let written = if name.is_empty() {
            "untitled"
        } else {
            name.as_str()
        };
        if let Some(dir) = bench_path().parent().map(|d| d.to_path_buf()) {
            let _ = std::fs::create_dir_all(&dir);
            // The work's own name is overwritten freely - that is what
            // saving means - but a name some OTHER work holds steps aside
            // rather than clobbering it in silence.
            let ours = work_name.0.as_deref() == Some(written);
            let mut stem = written.to_string();
            let mut path = dir.join(format!("{stem}.{WORK_KIND}"));
            if !ours {
                let mut n = 2;
                while path.exists() || dir.join(format!("{stem}.json")).exists() {
                    stem = format!("{written}-{n}");
                    path = dir.join(format!("{stem}.{WORK_KIND}"));
                    n += 1;
                }
            }
            // A work saved under a name it already held as a `.json` leaves no
            // twin behind: the drawer lists what is in the folder, and two rows
            // reading LONGHOUSE would be two rows a maker has to tell apart by
            // opening them.
            let elder = dir.join(format!("{stem}.json"));
            if ours && elder.exists() && elder != path {
                let _ = std::fs::remove_file(&elder);
            }
            // The shown step is standing as entities; the rest are already
            // records. Gather the one before writing, or a maker's last few
            // minutes go missing from whichever step they were on.
            let mut drawings = stages.drawings.clone();
            let showing = stages.showing.min(drawings.len().saturating_sub(1));
            if let Some(slot) = drawings.get_mut(showing) {
                *slot = placed.iter().cloned().collect();
            }
            let bench = Workbench {
                format: 2,
                name: stem.clone(),
                parts: Vec::new(),
                stages: drawings,
            };
            if let Ok(json) = serde_json::to_string_pretty(&bench) {
                let count = bench.stages.get(showing).map_or(0, Vec::len);
                let _ = std::fs::write(&path, json);
                info!("saved {count} parts to {}", path.display());
                work_name.0 = Some(stem.clone());
                for (entity, mut text) in &mut save_labels {
                    *text = Text::new(format!("SAVED {} - {count} PARTS", stem.to_uppercase()));
                    commands.entity(entity).insert(PassingWord {
                        back: crate::rail::FOOT_SAYING,
                        until: time.elapsed_secs() + 2.5,
                    });
                }
            }
        }
    }
    naming.0 = None;
    for card in &cards {
        commands.entity(card).despawn();
    }
}


/// Passing words return to their old text when their moment ends.
fn settle_words(
    mut commands: Commands,
    time: Res<Time>,
    mut words: Query<(Entity, &PassingWord, &mut Text)>,
) {
    for (entity, word, mut text) in &mut words {
        if time.elapsed_secs() >= word.until {
            *text = Text::new(word.back.to_string());
            commands.entity(entity).remove::<PassingWord>();
        }
    }
}

/// The dimensions card: raised once, shown when it has something to say.
/// While stretch-drawing it reads the live size; with a sized part
/// selected in RESIZE it shows the measure and D opens typed entry -
/// "3.5" for a length, "3.5x6" for a slab - enter applies on the
/// lattice, escape thinks better of it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dims_panel(
    mut commands: Commands,
    mut keystrokes: MessageReader<bevy::input::keyboard::KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
    tool: Res<crate::gizmo::ToolMode>,
    selected: Res<crate::gizmo::Selected>,
    mut entry: ResMut<DimsEntry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cards: Query<&mut Visibility, With<DimsCard>>,
    mut readouts: Query<&mut Text, With<DimsText>>,
    ghosts: Query<&Placed, With<Ghost>>,
    mut parts: Query<(&mut Transform, &mut Placed), Without<Ghost>>,
    mut raised: Local<bool>,
) {
    if !*raised {
        *raised = true;
        let card = commands
            .spawn((
                DimsCard,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    // Clear of the step row, which took the floor when the
                    // stages arrived and hid this underneath itself.
                    bottom: Val::Px(52.0),
                    margin: UiRect::left(Val::Px(-130.0)),
                    width: Val::Px(260.0),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme::panel_bg()),
                BorderColor::all(theme::panel_border(&palette)),
                Visibility::Hidden,
            ))
            .id();
        commands.spawn((
            DimsText,
            Text::new(""),
            TextFont {
                font: fonts.text.clone().into(),
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(theme::text(&palette)),
            ChildOf(card),
        ));
        return;
    }

    // The selected sized part, if any, and its measure.
    let sized = selected.one().and_then(|part| {
        let (_, record) = parts.get(part).ok()?;
        let kind = kind_from_name(&record.part)?;
        match kind {
            PartKind::Wall(long) => Some((part, long, None, None)),
            PartKind::Seg { long, .. } => Some((part, long, None, None)),
            PartKind::Trim { long, .. } => Some((part, long, None, None)),
            PartKind::Beam(long, ..) => Some((part, long, None, None)),
            PartKind::Ridge(long) => Some((part, long, None, None)),
            // A roof's PITCH rides along with its size. Brett: "incase I want
            // to make multiple buildings with the same sized roof peak" - and a
            // number you can read off one roof is a number you can pull another
            // to, where an angle you can only judge by eye is not.
            PartKind::GableRoof(w, d, _, pitch) => Some((part, w, Some(d), Some(pitch))),
            PartKind::Gable(long, pitch) => Some((part, long, None, Some(pitch))),
            PartKind::Chimney(drop) => Some((part, drop, None, None)),
            PartKind::Floor(w, d) | PartKind::Foundation(w, d) | PartKind::Roof(w, d) => {
                Some((part, w, Some(d), None))
            }
            _ => None,
        }
    });

    // Typing takes precedence; then the live stretch; then the measure.
    let mut said: Option<String> = None;
    if let Some(text) = entry.0.as_mut() {
        use bevy::input::keyboard::Key;
        let mut done: Option<bool> = None;
        for stroke in keystrokes.read() {
            if !stroke.state.is_pressed() {
                continue;
            }
            match &stroke.logical_key {
                Key::Character(typed) => {
                    for letter in typed.chars() {
                        let letter = letter.to_ascii_lowercase();
                        if (letter.is_ascii_digit() || letter == '.' || letter == 'x')
                            && text.len() < 12
                        {
                            text.push(letter);
                        }
                    }
                }
                Key::Backspace => {
                    text.pop();
                }
                Key::Enter => done = Some(true),
                Key::Escape => done = Some(false),
                _ => {}
            }
        }
        said = Some(format!("{text}_"));
        if let Some(saving) = done {
            if saving
                && let Some((part, _, had_d, _)) = sized
                && let Ok((mut transform, mut record)) = parts.get_mut(part)
                && let Some(kind) = kind_from_name(&record.part)
            {
                // "3.5" or "3.5x6", snapped onto the lattice, no smaller
                // than one coarse cell, resized around the centre.
                let lattice = |value: f32| ((value * 16.0).round() / 16.0).max(0.25);
                let (w_in, d_in) = match text.split_once('x') {
                    Some((a, b)) => (a.parse::<f32>().ok(), b.parse::<f32>().ok()),
                    None => (text.parse::<f32>().ok(), None),
                };
                // Typed numbers are UNITS - sixteenths of a metre - so a
                // wall is 40 tall and a room 48 wide, no decimals needed.
                let units = |value: f32| lattice(value / 16.0);
                let _ = &lattice;
                if let Some(w) = w_in.map(units) {
                    let d = d_in.map(lattice);
                    let made = match kind {
                        PartKind::Wall(_) => Some(PartKind::Wall(w)),
                        PartKind::Seg { high, lift, .. } => Some(PartKind::Seg {
                            long: w,
                            high,
                            lift,
                        }),
                        PartKind::Trim { stone, .. } => Some(PartKind::Trim { long: w, stone }),
                        PartKind::Floor(_, old) => Some(PartKind::Floor(w, d.unwrap_or(old))),
                        PartKind::Foundation(_, old) => {
                            Some(PartKind::Foundation(w, d.unwrap_or(old)))
                        }
                        PartKind::Roof(_, old) => Some(PartKind::Roof(w, d.unwrap_or(old))),
                        // The pitch rides through a resize: a roof HAS a pitch,
                        // so a wider building wants a taller roof and not a
                        // flatter one.
                        PartKind::GableRoof(_, old, over, pitch) => {
                            Some(PartKind::GableRoof(w, d.unwrap_or(old), over, pitch))
                        }
                        PartKind::Chimney(_) => Some(PartKind::Chimney(w)),
                        _ => None,
                    };
                    if let Some(made) = made {
                        record.part = part_name(&made);
                        record.at = transform.translation.into();
                        let _ = &mut transform;
                        commands.entity(part).despawn_related::<Children>();
                        dress_part(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            &palette,
                            &made,
                            &record,
                            part,
                            false,
                        );
                        let _ = had_d;
                    }
                }
            }
            entry.0 = None;
            said = None;
        }
    } else if let Some(drawn) = ghosts.iter().next().and_then(|g| kind_from_name(&g.part)) {
        // Live measure while stretch-drawing.
        let units = |value: f32| format!("{}", (value * 16.0).round() as i64);
        said = match drawn {
            PartKind::Wall(long) => Some(format!("wall - {}", units(long))),
            PartKind::Trim { long, .. } => Some(format!("trim - {}", units(long))),
            PartKind::Floor(w, d) => Some(format!("floor - {} x {}", units(w), units(d))),
            PartKind::Foundation(w, d) => Some(format!("foundation - {} x {}", units(w), units(d))),
            PartKind::Roof(w, d) => Some(format!("roof - {} x {}", units(w), units(d))),
            _ => None,
        };
    } else if *tool == crate::gizmo::ToolMode::Resize
        && let Some((_, w, d, pitch)) = sized
    {
        let units = |value: f32| format!("{}", (value * 16.0).round() as i64);
        let angle = pitch.map_or(String::new(), |degrees| format!("  {degrees:.1}°"));
        said = Some(match d {
            Some(d) => format!("{} x {}{angle} - D to type", units(w), units(d)),
            None => format!("{}{angle} - D to type", units(w)),
        });
        if keys.just_pressed(KeyCode::KeyD) {
            entry.0 = Some(String::new());
        }
    }

    for mut visibility in &mut cards {
        let wanted = if said.is_some() || entry.0.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
    if let Some(word) = said {
        for mut text in &mut readouts {
            if text.0 != word {
                *text = Text::new(word.clone());
            }
        }
    }
}

/// The bench's memory: whole-state snapshots, since a build is nothing
/// but its list of records. Undo therefore covers everything the same
/// way - placements, punches, stretches, moves, typed sizes, even a
/// template load or a cleared bench.
#[derive(Resource, Default)]
pub struct History {
    past: Vec<Vec<Placed>>,
    future: Vec<Vec<Placed>>,
    current: Vec<Placed>,
    primed: bool,
}

impl History {
    /// Forgets everything, for a change no hand could have made — setting out
    /// another step of the build swaps every part on the bench at once.
    fn forget(&mut self) {
        self.past.clear();
        self.future.clear();
        self.current.clear();
        self.primed = false;
    }
}

fn state_signature(list: &[Placed]) -> String {
    let mut lines: Vec<String> = list
        .iter()
        .map(|record| serde_json::to_string(record).unwrap_or_default())
        .collect();
    lines.sort();
    lines.join("|")
}

/// Notices settled changes and remembers the state they replaced. While
/// the mouse button is down nothing commits, so a whole drag lands as
/// one step.
fn remember(
    buttons: Res<ButtonInput<MouseButton>>,
    mut history: ResMut<History>,
    placed: Query<&Placed, Without<Ghost>>,
) {
    if buttons.pressed(MouseButton::Left) {
        return;
    }
    let now: Vec<Placed> = placed.iter().cloned().collect();
    if !history.primed {
        history.current = now;
        history.primed = true;
        return;
    }
    if state_signature(&now) != state_signature(&history.current) {
        let old = std::mem::replace(&mut history.current, now);
        history.past.push(old);
        if history.past.len() > 50 {
            history.past.remove(0);
        }
        history.future.clear();
    }
}

/// Ctrl or cmd with Z walks back; with Y - or shift-Z - walks forward.
#[allow(clippy::too_many_arguments)]
fn recall(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    mut history: ResMut<History>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
    standing: Query<Entity, (With<Placed>, Without<Ghost>)>,
) {
    if *bench != Bench::Builder || naming.0.is_some() || dims.0.is_some() {
        return;
    }
    let held = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    if !held {
        return;
    }
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let back = keys.just_pressed(KeyCode::KeyZ) && !shifted;
    let forward = keys.just_pressed(KeyCode::KeyY) || (keys.just_pressed(KeyCode::KeyZ) && shifted);
    if !back && !forward {
        return;
    }
    let restored = if back {
        let Some(older) = history.past.pop() else {
            return;
        };
        let now = std::mem::replace(&mut history.current, older.clone());
        history.future.push(now);
        older
    } else {
        let Some(newer) = history.future.pop() else {
            return;
        };
        let now = std::mem::replace(&mut history.current, newer.clone());
        history.past.push(now);
        newer
    };
    for part in &standing {
        commands.entity(part).despawn();
    }
    for record in &restored {
        if let Some(kind) = kind_from_name(&record.part) {
            spawn_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &kind,
                record,
                false,
            );
        }
    }
    info!(
        "{} to a bench of {} parts",
        if back {
            "walked back"
        } else {
            "walked forward"
        },
        restored.len()
    );
}

/// Walking into MOVE or RESIZE empties the hand: the ghost belongs to
/// placement, and those modes are for what already stands.
fn disarm_on_mode(
    mut commands: Commands,
    tool: Res<crate::gizmo::ToolMode>,
    mut hand: ResMut<Hand>,
    ghosts: Query<Entity, With<Ghost>>,
) {
    if tool.is_changed() && *tool != crate::gizmo::ToolMode::Normal && hand.kind.is_some() {
        *hand = Hand::default();
        for ghost in &ghosts {
            commands.entity(ghost).despawn();
        }
    }
}

/// The last part copied, kept whole - its kind, size, turn and paint.
#[derive(Resource, Default)]
pub struct Clipboard(pub Option<Placed>);

/// Cmd or ctrl with C copies what the cursor touches (or what is
/// selected); with V it loads that copy into the hand, ghost and all,
/// so it lands with every snap the bench offers and can be stamped as
/// often as you like.
#[allow(clippy::too_many_arguments)]
fn copy_and_paste(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    hovered: Res<Hovered>,
    gizmo: (Res<crate::gizmo::Selected>, ResMut<crate::gizmo::ToolMode>),
    mut clipboard: ResMut<Clipboard>,
    mut hand: ResMut<Hand>,
    placed: Query<&Placed, Without<Ghost>>,
    ghosts: Query<Entity, With<Ghost>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
) {
    let (selected, mut tool) = gizmo;
    if *bench != Bench::Builder || naming.0.is_some() || dims.0.is_some() {
        return;
    }
    let held = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    if !held {
        return;
    }
    if keys.just_pressed(KeyCode::KeyC)
        && let Some(source) = selected.lead().or(hovered.grab)
        && let Ok(record) = placed.get(source)
    {
        clipboard.0 = Some(record.clone());
        info!("copied {}", record.part);
    }
    if keys.just_pressed(KeyCode::KeyV)
        && let Some(record) = clipboard.0.clone()
        && let Some(kind) = kind_from_name(&record.part)
    {
        // Pasting is placing: the hand takes the copy and the modes step
        // back to NORMAL, where placement lives.
        *tool = crate::gizmo::ToolMode::Normal;
        *hand = Hand {
            kind: Some(kind),
            anchor: None,
            flip: record.flip,
            stage: record.stage.clone(),
            yaw: record.yaw,
            tilt: record.tilt,
            lift: 0.0,
            ramp: record.ramp.clone(),
            shade: record.shade,
        };
        dress_ghost(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &hand,
            &ghosts,
        );
    }
}

/// M mirrors what the hand holds, or what the arrows hold: the body
/// reflects across its own length and any tilt leans the other way, so
/// a pitched panel's twin completes the gable and an L-corner becomes
/// the other hand.
#[allow(clippy::too_many_arguments)]
fn mirror_part(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    selected: Res<crate::gizmo::Selected>,
    mut hand: ResMut<Hand>,
    ghosts: Query<Entity, With<Ghost>>,
    mut parts: Query<(&mut Transform, &mut Placed), Without<Ghost>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
) {
    if *bench != Bench::Builder
        || naming.0.is_some()
        || dims.0.is_some()
        || !keys.just_pressed(KeyCode::KeyM)
    {
        return;
    }
    // A standing part first: mirroring what you can see beats mirroring
    // what you are about to place.
    if let Some(part) = selected.lead()
        && let Ok((mut transform, mut record)) = parts.get_mut(part)
        && let Some(kind) = kind_from_name(&record.part)
    {
        record.flip = !record.flip;
        transform.rotation = pose(record.yaw, record.tilt, record.flip);
        commands.entity(part).despawn_related::<Children>();
        dress_part(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &kind,
            &record,
            part,
            false,
        );
        return;
    }
    if hand.kind.is_some() {
        hand.flip = !hand.flip;
        dress_ghost(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &hand,
            &ghosts,
        );
    }
}

/// The opening a frame stands in follows it. Sliding a door or window
/// with the arrows closes the wall it came from and parts the wall it
/// lands in, when the drag lets go.
#[allow(clippy::too_many_arguments)]
fn reflow_openings(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    selected: Res<crate::gizmo::Selected>,
    mut came_from: Local<Option<(Entity, Vec3)>>,
    placed: Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
) {
    // The one table, reached through the one function. This was a SECOND copy
    // of it, with the same three rows written out again - so a new opening added
    // to the shelf punched its hole correctly when placed and reverted to a
    // door's dimensions the moment the wall under it was reflowed.
    let opening_for =
        |record: &Placed| kind_from_name(&record.part).and_then(|kind| opening_of(&kind));

    if buttons.just_pressed(MouseButton::Left) {
        *came_from = selected
            .lead()
            .and_then(|part| placed.get(part).ok())
            .filter(|(_, _, record, _)| opening_for(record).is_some())
            .map(|(entity, at, _, _)| (entity, at.translation));
        return;
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let Some((frame, old_spot)) = came_from.take() else {
        return;
    };
    let Ok((_, at, record, _)) = placed.get(frame) else {
        return;
    };
    let now = at.translation;
    if now.distance(old_spot) < 0.03 {
        return;
    }
    let Some((wide, head, sill, is_door)) = opening_for(record) else {
        return;
    };

    // Close the wall it came from, then part the one it landed in. If
    // nothing stands where it landed, the frame simply keeps its place.
    heal_wall_at(
        &mut commands,
        &mut meshes,
        &mut materials,
        &palette,
        &placed,
        frame,
        old_spot,
    );
    let carried = Hand {
        kind: kind_from_name(&record.part),
        anchor: None,
        flip: record.flip,
        stage: record.stage.clone(),
        yaw: record.yaw,
        tilt: 0.0,
        lift: 0.0,
        ramp: record.ramp.clone(),
        shade: record.shade,
    };
    let punched = punch_wall(
        &mut commands,
        &mut meshes,
        &mut materials,
        &palette,
        &placed,
        None,
        now,
        wide,
        head,
        sill,
        is_door,
        &carried,
    );
    if punched {
        // The punch set a fresh frame of its own in the new opening.
        commands.entity(frame).despawn();
    }
}

/// How much of the work is standing: all of it, the roof lifted off,
/// or the walls down as well - the dollhouse view, for furnishing.
#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub enum Cutaway {
    #[default]
    Whole,
    RoofOff,
    WallsDown,
}

/// Kept as a resource of its own so the rail's button can read it.
#[derive(Resource, Default)]
pub struct RoofsLifted(pub Cutaway);

/// H lifts the roof off and sets it back - everything raised at the
/// roof stage goes with it, panels and ridge caps alike.
/// Delete takes away whatever is chosen, in whichever tool is in hand.
///
/// Being rid of a part used to mean going back to NORMAL, picking the part up,
/// and pressing escape to throw away what you were holding - three steps, one of
/// which is a mode change, to undo one placement. Brett asked for the obvious
/// thing instead: choose it and press delete.
///
/// BACKSPACE as well as Delete, and on this bench that is the important half:
/// the key labelled "delete" on a Mac keyboard IS backspace, and the forward
/// Delete these keyboards do not have is the one Bevy calls `Delete`.
///
/// It stands aside while anything is being TYPED. Backspace belongs to whoever
/// is taking letters - the name card, the dimensions box - and a part quietly
/// vanishing while a maker corrects a typo would be a bad way to learn that.
///
/// Nothing to do about undo: the bench remembers whole states, so a part
/// removed is one step back like anything else.
fn bury_the_chosen(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    hand: Res<Hand>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut selected: ResMut<crate::gizmo::Selected>,
    parts: Query<(Entity, &Transform, &Placed, &Visibility), Without<Ghost>>,
) {
    if *bench != Bench::Builder || naming.0.is_some() || dims.0.is_some() {
        return;
    }
    // A full hand answers these keys already, by throwing away what it holds.
    if hand.kind.is_some() {
        return;
    }
    if !keys.just_pressed(KeyCode::Delete) && !keys.just_pressed(KeyCode::Backspace) {
        return;
    }
    // Everything chosen, not merely the first of them.
    let doomed: Vec<Entity> = selected.iter().collect();
    if doomed.is_empty() {
        return;
    }
    for chosen in doomed {
    if let Ok((_, chosen_at, _, _)) = parts.get(chosen) {
        // The marks it carries go with it. A door's routing mark left standing
        // in a wall with no door is worse than either: the village reads it and
        // sends people to walk through masonry.
        let carried = carried_marks(
            chosen,
            chosen_at.translation,
            parts.iter().map(|(e, at, record, _)| (e, at.translation, record)),
        );
        for mark in carried {
            commands.entity(mark).despawn();
        }
        // A door taken out leaves the wall whole again. The older path through
        // X has always done this; a second way to remove a part that did not
        // would leave holes nobody could account for.
        heal_wall(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &parts,
            chosen,
        );
        commands.entity(chosen).despawn();
    }
    }
    selected.clear();
}

/// A request to show another step, or to add or drop one.
///
/// Held as a resource rather than done where it is asked for, because setting a
/// step out means despawning and respawning the whole work, and exactly one
/// place should be allowed to do that.
#[derive(Resource, Default)]
pub struct StageWish(pub Option<StageDeed>);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StageDeed {
    /// Show this step.
    Show(usize),
    /// A new step after the last, holding a copy of it.
    ///
    /// A copy rather than bare ground: most steps are the one before with
    /// something changed, and a maker who wanted bare ground can clear it in
    /// one press. Starting empty would make the common case the laborious one.
    AddCopying,
    /// A new empty step after the last.
    AddBare,
    /// Drop the step being shown.
    Drop,
    /// Remember this step, to put on another.
    Take,
    /// Put the remembered step here, in place of what stands.
    Put,
}

/// A step held aside, waiting to be put on another.
///
/// Brett: "I need a way to copy a stage and paste it on anotehr stage." `+ COPY`
/// only ever made a NEW step from the one showing, which is the wrong shape for
/// "make step three look like step two again" - there is no new step wanted, and
/// the one that needs changing already exists.
#[derive(Resource, Default)]
pub struct StageHeld(Option<Vec<Placed>>);

/// Sets out another step of the work.
///
/// The bench holds one step as entities and the rest as records, so this is the
/// only place a step changes hands: gather what is standing back into the step
/// it belongs to, then set the wanted one out. Doing it anywhere else would lose
/// whichever step the maker was on.
#[allow(clippy::too_many_arguments)]
fn turn_to_stage(
    mut commands: Commands,
    mut wish: ResMut<StageWish>,
    mut held: ResMut<StageHeld>,
    mut stages: ResMut<Stages>,
    mut history: ResMut<History>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut selected: ResMut<crate::gizmo::Selected>,
    standing: Query<(Entity, &Placed), Without<Ghost>>,
) {
    let Some(deed) = wish.0.take() else {
        return;
    };
    // Whatever is on the bench belongs to the step it was drawn on.
    let showing = stages.showing.min(stages.drawings.len().saturating_sub(1));
    let gathered: Vec<Placed> = standing.iter().map(|(_, record)| record.clone()).collect();
    if let Some(slot) = stages.drawings.get_mut(showing) {
        *slot = gathered;
    }

    let wanted = match deed {
        StageDeed::Show(step) => step.min(stages.drawings.len() - 1),
        StageDeed::Take => {
            // Nothing moves: the step is remembered exactly as it was gathered a
            // moment ago, and the bench goes on showing it.
            held.0 = Some(stages.drawings[showing].clone());
            return;
        }
        StageDeed::Put => {
            let Some(kept) = held.0.clone() else {
                return;
            };
            // In PLACE of what stands, not beside it. Two steps merged would be
            // a step nobody drew, and the way to add to a step is to draw on it.
            stages.drawings[showing] = kept;
            showing
        }
        StageDeed::AddCopying => {
            let copy = stages.drawings[showing].clone();
            stages.drawings.push(copy);
            stages.drawings.len() - 1
        }
        StageDeed::AddBare => {
            stages.drawings.push(Vec::new());
            stages.drawings.len() - 1
        }
        StageDeed::Drop => {
            // Never the last one standing: a building with no steps is not a
            // building anybody can raise.
            if stages.drawings.len() <= 1 {
                return;
            }
            stages.drawings.remove(showing);
            showing.min(stages.drawings.len() - 1)
        }
    };

    for (part, _) in &standing {
        commands.entity(part).despawn();
    }
    selected.clear();
    for record in &stages.drawings[wanted] {
        if let Some(kind) = kind_from_name(&record.part) {
            spawn_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &kind,
                record,
                false,
            );
        }
    }
    let travelled = wanted != showing;
    stages.showing = wanted;
    // Undo does not reach ACROSS a step. Every part on the bench has just been
    // swapped for another step's, and a history that let someone undo into that
    // would put one step's parts down on another - not a thing a maker could
    // have done by hand, and so not a thing undo should be able to do.
    //
    // But PUTTING a step over the one showing never leaves it, and replaces
    // what was there - which is exactly the kind of large, destructive, ordinary
    // edit undo exists for. Forgetting there would make a mis-aimed PUT the one
    // thing in this bench that cannot be taken back.
    if travelled {
        history.forget();
    }
}

/// How many steps a work rises in, and which step a part belongs to.
///
/// This is the GAME's rule, written out again here because the bench and the
/// game share no code — see FORMATS.md. It has to match exactly or the playback
/// is a lie, and a lie in a preview is worse than no preview: the maker would
/// trust it. The rule, including the awkward part:
///
///   footing 0, frame 1, walls 2, everything else 3 — unless the work has no
///   frame at all, in which case walls and the rest each move DOWN a step,
///   because a build with nothing to raise at step 1 never reaches step 3.
fn step_of(stage: &str, framed: bool) -> u8 {
    match (stage, framed) {
        ("footing", _) => 0,
        ("frame", _) => 1,
        ("walls", true) => 2,
        ("walls", false) => 1,
        (_, true) => 3,
        (_, false) => 2,
    }
}

pub(crate) fn lift_roofs(
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    mut lifted: ResMut<RoofsLifted>,
    mut parts: Query<(&Placed, &mut Visibility), Without<Ghost>>,
) {
    if *bench == Bench::Builder
        && naming.0.is_none()
        && dims.0.is_none()
        && keys.just_pressed(KeyCode::KeyH)
    {
        lifted.0 = match lifted.0 {
            Cutaway::Whole => Cutaway::RoofOff,
            Cutaway::RoofOff => Cutaway::WallsDown,
            Cutaway::WallsDown => Cutaway::Whole,
        };
    }
    for (record, mut visibility) in &mut parts {
        // What a part IS, and nothing else. This used to ask the part's KIND as
        // well - a gable roof was roof-ish whatever it had been told, a wall was
        // wall-ish - which was a sensible net while nobody could say otherwise
        // and became an override the moment they could. Brett: "any peice could
        // be a roof piece", and the other way about for walls. So the tag is the
        // only word: a plank tagged as roof comes off with the roof, and a roof
        // panel tagged as walls stays until the walls come down.
        //
        // The frame comes down with the walls. What WallsDown means is "show me
        // the ground it stands on", and a hall's posts left standing over bare
        // footings is not that.
        let roofish = record.stage == "roof";
        let wallish = record.stage == "walls" || record.stage == "frame";
        let cut = match lifted.0 {
            Cutaway::Whole => true,
            Cutaway::RoofOff => !roofish,
            Cutaway::WallsDown => !roofish && !wallish,
        };
        let showing = cut;
        let wanted = if showing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

#[cfg(test)]
mod bake {
    use super::*;

    /// Bakes every saved work into what the game can eat: plain boxes
    /// with resolved colours, and the marks that say what the place is
    /// FOR. Run by hand when a building is ready to be carried in:
    /// `cargo test bake_the_works -- --ignored --nocapture`
    ///
    /// The game shares no code with the bench, so the bench resolves its
    /// own catalogue and palette here and hands over the result.
    #[test]
    #[ignore = "a hand-run export, not a check"]
    fn bake_the_works() {
        let palette = crate::look::load_palette_for_bake();
        let dir = bench_path().parent().unwrap().to_path_buf();
        let baked_dir = dir.parent().unwrap().join("baked");
        std::fs::create_dir_all(&baked_dir).expect("baked dir");

        for entry in std::fs::read_dir(&dir).expect("out/buildings") {
            let path = entry.expect("entry").path();
            if !is_a_work(&path) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(work) = serde_json::from_str::<Workbench>(&text) else {
                continue;
            };
            let name = path.file_stem().unwrap().to_string_lossy().to_string();

            // The FINISHED building, which is the last step - steps replace one
            // another rather than adding up, so the last one is the whole thing
            // and no other one is. A work drawn before there were steps keeps
            // its one flat list, which is the same thing said the older way.
            let parts: &[Placed] = work.stages.last().map_or(&work.parts[..], |last| &last[..]);

            // The bounds of everything that is not a scale reference, so
            // the building can be recentred on its own footprint.
            let mut low = Vec3::splat(f32::INFINITY);
            let mut high = Vec3::splat(f32::NEG_INFINITY);
            for record in parts {
                if record.part == "prop:mannequin" {
                    continue;
                }
                let Some(kind) = kind_from_name(&record.part) else {
                    continue;
                };
                let turn = pose(record.yaw, record.tilt, record.flip);
                for Slab(mut at, size, ..) in body_of(&kind, None) {
                    if record.flip {
                        at.x = -at.x;
                    }
                    let centre = Vec3::from(record.at) + turn * at;
                    let reach = (turn * (size * 0.5)).abs();
                    low = low.min(centre - reach);
                    high = high.max(centre + reach);
                }
            }
            let middle = Vec3::new((low.x + high.x) * 0.5, 0.0, (low.z + high.z) * 0.5);

            let mut boxes: Vec<String> = Vec::new();
            // What, where, which way - and whether a hand put it there.
            let mut marks: Vec<(String, Vec3, f32, bool)> = Vec::new();
            let say = |v: Vec3| format!("[{:.4}, {:.4}, {:.4}]", v.x, v.y, v.z);

            for record in parts {
                if record.part == "prop:mannequin" {
                    continue;
                }
                let Some(kind) = kind_from_name(&record.part) else {
                    continue;
                };
                let turn = pose(record.yaw, record.tilt, record.flip);
                let anchor = Vec3::from(record.at) - middle;

                // What the place is for, read from the widgets that say
                // so and from the furniture that means it.
                let mark = |what: &str, at: Vec3, yaw: f32| (what.to_string(), at, yaw, false);
                match kind {
                    PartKind::Widget(what) => {
                        marks.push((what.to_string(), anchor, record.yaw, true));
                        continue;
                    }
                    // Beds and seats say nothing on their own: their
                    // figures are set down WITH them and can be taken
                    // away, so a chair with no sitter on it is a chair
                    // nobody sits in. Only furniture with no figure to
                    // show still speaks for itself.
                    PartKind::Prop("cradle") => marks.push(mark("sleep", anchor, record.yaw)),
                    PartKind::Prop("hearth") => {
                        marks.push(mark("fire", anchor, record.yaw));
                        marks.push(mark("smoke", anchor, record.yaw));
                    }
                    PartKind::Prop("table") => marks.push(mark("table", anchor, record.yaw)),
                    PartKind::Prop("chest" | "cupboard" | "wardrobe" | "shelves") => {
                        marks.push(mark("store", anchor, record.yaw))
                    }
                    PartKind::Prop("anvil" | "loom") => {
                        marks.push(mark("work", anchor, record.yaw))
                    }
                    PartKind::Prop("candle") => marks.push(mark("light", anchor, record.yaw)),
                    _ => {}
                }

                // The body itself, as boxes the game can simply draw.
                let repaint = record.ramp.as_deref().map(|r| (r, record.shade));
                for Slab(mut at, size, ramp, shade, clarity, shape, mut lean) in
                    body_of(&kind, repaint)
                {
                    if record.flip {
                        at.x = -at.x;
                        lean = -lean;
                    }
                    let centre = anchor + turn * at;
                    // A piece that leans carries its own angle into the
                    // turn the game will draw it with.
                    let turn = turn * Quat::from_rotation_x(lean);
                    let colour = palette.shade(&ramp, shade).to_srgba();
                    let form = match shape {
                        Shape::Box => "box",
                        Shape::Wedge => "wedge",
                        Shape::Ridge => "ridge",
                        Shape::Mitre => "mitre",
                        Shape::MitreBack => "mitre-back",
                    };
                    let stage = match kind {
                        PartKind::Gable(..)
                        | PartKind::Ridge(..)
                        | PartKind::GableRoof(..)
                        | PartKind::Roof(..)
                        | PartKind::Chimney(..) => "roof",
                        _ => record.stage.as_str(),
                    };
                    // The cloth is named as well as resolved: the game
                    // re-dyes a house's own walls and roof per building,
                    // the way it always rolled its own, and leaves every
                    // other piece exactly as it was painted.
                    boxes.push(format!(
                        "    {{\"at\": {}, \"size\": {}, \"turn\": [{:.5}, {:.5}, {:.5}, {:.5}], \
                         \"rgb\": [{}, {}, {}], \"alpha\": {:.2}, \"form\": \"{form}\", \
                         \"cloth\": \"{ramp}:{shade}\", \"stage\": \"{}\"}}",
                        say(centre),
                        say(size),
                        turn.x,
                        turn.y,
                        turn.z,
                        turn.w,
                        (colour.red * 255.0).round() as u8,
                        (colour.green * 255.0).round() as u8,
                        (colour.blue * 255.0).round() as u8,
                        clarity,
                        stage,
                    ));
                }
            }

            // A widget laid by hand overrules the same meaning derived
            // from the furniture under it: a sleeping figure set on a bed
            // to check the fit is that bed's sleeping place, not a second
            // one beside it.
            let by_hand: Vec<(String, Vec3)> = marks
                .iter()
                .filter(|(.., hand)| *hand)
                .map(|(what, at, ..)| (what.clone(), *at))
                .collect();
            marks.retain(|(what, at, _, hand)| {
                *hand
                    || !by_hand.iter().any(|(other, spot)| {
                        other == what && (spot.x - at.x).hypot(spot.z - at.z) < 0.8
                    })
            });
            let marks: Vec<String> = marks
                .iter()
                .map(|(what, at, yaw, _)| {
                    format!(
                        "    {{\"mark\": \"{what}\", \"at\": {}, \"yaw\": {yaw:.4}}}",
                        say(*at)
                    )
                })
                .collect();

            let span = high - low;
            let json = format!(
                "{{\n  \"format\": 1,\n  \"name\": \"{name}\",\n  \
                 \"half_w\": {:.4},\n  \"half_d\": {:.4},\n  \"high\": {:.4},\n  \
                 \"boxes\": [\n{}\n  ],\n  \"marks\": [\n{}\n  ]\n}}\n",
                span.x * 0.5,
                span.z * 0.5,
                high.y,
                boxes.join(",\n"),
                marks.join(",\n"),
            );
            let out = baked_dir.join(format!("{name}.json"));
            std::fs::write(&out, json).expect("write baked");
            println!(
                "baked {name}: {} boxes, {} marks, {:.2} x {:.2} x {:.2}",
                boxes.len(),
                marks.len(),
                span.x,
                span.z,
                high.y
            );
        }
    }
}

/// R turns whatever is selected, in whichever mode: the hand's own
/// quarter-turn belongs to placing, but a part already standing should
/// answer the same key.
#[allow(clippy::too_many_arguments)]
fn held_shift(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

/// Shift-R turns the WHOLE work a quarter, about its own middle.
///
/// A house is drawn facing whichever way the maker happened to start, and
/// finding out at the end that it wants to face the other way should not
/// mean rebuilding it. The middle is snapped to the lattice before
/// anything turns, so a quarter turn about it lands every part back on
/// the lattice exactly - no drift, however many times it is spun.
fn turn_the_work(
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    mut parts: Query<(&mut Transform, &mut Placed), Without<Ghost>>,
) {
    if *bench != Bench::Builder
        || naming.0.is_some()
        || dims.0.is_some()
        || !keys.just_pressed(KeyCode::KeyR)
        || !held_shift(&keys)
    {
        return;
    }
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for (_, record) in &parts {
        low = low.min(Vec3::from(record.at));
        high = high.max(Vec3::from(record.at));
    }
    if !low.x.is_finite() {
        return;
    }
    let middle = (low + high) * 0.5;
    let onto = |v: f32| (v * 16.0).round() / 16.0;
    let (mx, mz) = (onto(middle.x), onto(middle.z));

    for (mut transform, mut record) in &mut parts {
        let at = Vec3::from(record.at);
        // A quarter turn about Y sends (x, z) to (z, -x).
        let (dx, dz) = (at.x - mx, at.z - mz);
        record.at = [mx + dz, at.y, mz - dx];
        record.yaw = (record.yaw + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
        transform.translation = Vec3::from(record.at);
        transform.rotation = pose(record.yaw, record.tilt, record.flip);
    }
}

/// T tilts the selected part a notch, the way R turns it. Tilt was the
/// hand's alone until now: a piece already set down could be turned but
/// never leaned, so getting it wrong meant picking it up and starting the
/// approach again.
fn tilt_part(
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    selected: Res<crate::gizmo::Selected>,
    mut parts: Query<(&mut Transform, &mut Placed), Without<Ghost>>,
) {
    if *bench != Bench::Builder
        || naming.0.is_some()
        || dims.0.is_some()
        || !keys.just_pressed(KeyCode::KeyT)
    {
        return;
    }
    let Some(part) = selected.lead() else {
        return;
    };
    let Ok((mut transform, mut record)) = parts.get_mut(part) else {
        return;
    };
    let step = if held_shift(&keys) {
        -15f32.to_radians()
    } else {
        15f32.to_radians()
    };
    record.tilt = (record.tilt + step).rem_euclid(std::f32::consts::TAU);
    transform.rotation = pose(record.yaw, record.tilt, record.flip);
}

fn turn_part(
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    naming: Res<Naming>,
    dims: Res<DimsEntry>,
    selected: Res<crate::gizmo::Selected>,
    mut parts: Query<(&mut Transform, &mut Placed), Without<Ghost>>,
) {
    if *bench != Bench::Builder
        || naming.0.is_some()
        || dims.0.is_some()
        || !keys.just_pressed(KeyCode::KeyR)
        || held_shift(&keys)
    {
        return;
    }
    let Some(part) = selected.lead() else {
        return;
    };
    let Ok((mut transform, mut record)) = parts.get_mut(part) else {
        return;
    };
    record.yaw = (record.yaw + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
    transform.rotation = pose(record.yaw, record.tilt, record.flip);
}

#[cfg(test)]
mod roof_tests {
    use super::*;

    /// The folder the button opens is the folder the works are in - the same one
    /// the bench saves into and lists from, not merely near it.
    #[test]
    fn the_folder_button_opens_where_the_works_are() {
        let home = works_home();
        assert_eq!(
            bench_path().parent().expect("the bench sits in a folder"),
            home,
            "the button and the save would open two different places"
        );
        assert!(
            home.ends_with("out/buildings"),
            "the works live at {}",
            home.display()
        );
    }

    /// Trimming a beam that has ALREADY been trimmed must not undo the saw.
    #[test]
    fn a_second_trim_keeps_the_cuts_the_first_one_made() {
        let roof = a_record(
            part_name(&PartKind::GableRoof(8.0, 6.0, 0.25, 45.0)),
            [0.0, 3.0, 0.0],
            0.0,
        );
        let boxes = roof_boxes(&roof);
        let kind = PartKind::Beam(8.0, 0.0, 0.0);
        let beam = a_record("beam-8".to_string(), [0.0, 4.0, 0.0], std::f32::consts::FRAC_PI_2);
        let (once, moved) = trim_to_roof(&kind, &beam, &boxes).expect("the first trim");
        let PartKind::Beam(_, first_high, first_low) = once else {
            panic!("a trimmed beam is a beam");
        };
        assert!(first_high > 0.0 || first_low > 0.0, "the first trim cut nothing");
        match trim_to_roof(&once, &moved, &boxes) {
            None => {}
            Some((PartKind::Beam(_, high, low), _)) => {
                assert!(
                    high >= first_high - 1e-4 && low >= first_low - 1e-4,
                    "a second trim gave the beam square ends again: \
                     {first_high}/{first_low} became {high}/{low}"
                );
            }
            Some(_) => panic!("a trimmed beam came back as something else"),
        }
    }

    /// Naming a part's nature must not touch its body. A trimmed beam wears its
    /// cuts in its NAME, so anything that rewrites that name loses the saw work.
    #[test]
    fn naming_a_nature_leaves_the_body_alone() {
        use bevy::asset::AssetPlugin;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>().init_asset::<StandardMaterial>();
        app.insert_resource(crate::look::load_palette_for_bake());
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.init_resource::<crate::gizmo::Selected>();
        app.add_systems(Update, work_part_menu);

        let trimmed = "beam-6.375x0.375x0.375";
        let beam = app
            .world_mut()
            .spawn((
                Placed {
                    part: trimmed.to_string(),
                    at: [0.0, 2.0, 0.0],
                    yaw: 0.0,
                    tilt: 0.0,
                    ramp: None,
                    shade: 0.5,
                    stage: "frame".to_string(),
                    flip: false,
                    group: None,
                    loose: false,
                },
                Transform::default(),
            ))
            .id();
        let menu = app.world_mut().spawn(PartMenu).id();
        app.world_mut().spawn((
            MenuLine {
                deed: Deed::Nature("roof"),
                part: beam,
            },
            Interaction::Pressed,
            BackgroundColor(Color::NONE),
            ChildOf(menu),
        ));
        app.update();

        let record = app.world().get::<Placed>(beam).expect("the beam stands");
        assert_eq!(record.stage, "roof", "the nature did not take");
        assert_eq!(record.part, trimmed, "the trim was lost with the naming");
    }

    /// Roofs saved before the pitch could be pulled, taken from the buildings
    /// actually on disk when it was added.
    const ELDERS: [&str; 3] = [
        "gableroof-7.625x7.875x0.25",
        "gableroof-10.797664x7.625x0.5625",
        "gableroof-9x9x0.3125",
    ];

    #[test]
    fn a_roof_drawn_before_pitch_existed_still_opens() {
        // The whole risk of adding a number to a part's name: every building
        // already saved carries the old spelling, and a maker's work is not
        // something to lose to a format change.
        for name in ELDERS {
            let Some(PartKind::GableRoof(long, span, over, pitch)) = kind_from_name(name) else {
                panic!("{name} no longer opens at all");
            };
            assert!(
                long > 0.0 && span > 0.0 && over >= 0.0,
                "{name} came back wrong"
            );
            assert_eq!(
                pitch, ROOF_PITCH_DEGREES,
                "{name} must open at the pitch every roof in the world had when \
                 it was drawn, or every saved building changes shape"
            );
        }
    }

    #[test]
    fn an_angle_is_in_the_unit_its_name_says() {
        // Both halves of a units bug that compiled perfectly. A gable's peak is
        // the tangent of the pitch, and thirty RADIANS has a negative tangent -
        // the wedge came out upside down and eleven times too tall. A roof panel
        // arms at the same pitch, and thirty radians is two hundred and
        // seventy-nine degrees.
        let peak = 0.5 * ROOF_PITCH_DEGREES.to_radians().tan();
        assert!(
            (peak - 0.2887).abs() < 1e-3,
            "a gable half as wide should rise 0.2887 of its width, not {peak}"
        );
        // Against the constant itself: thirty degrees IS a sixth of pi, and
        // writing 0.5236 here would be writing it out by hand.
        assert!(
            (ROOF_PITCH_DEGREES.to_radians() - std::f32::consts::FRAC_PI_6).abs() < 1e-4,
            "the arming tilt is not thirty degrees in radians"
        );
    }

    /// How tall a gable of this width stands at this pitch.
    fn gable_peak(long: f32, pitch: f32) -> f32 {
        body_of(&PartKind::Gable(long, pitch), None)
            .iter()
            .map(|Slab(_, size, ..)| size.y)
            .fold(f32::MIN, f32::max)
    }

    fn a_record(part: String, at: [f32; 3], yaw: f32) -> Placed {
        Placed {
            part,
            at,
            yaw,
            tilt: 0.0,
            ramp: None,
            shade: 0.5,
            stage: "frame".to_string(),
            flip: false,
            loose: false,
            group: None,
        }
    }

    #[test]
    fn a_trimmed_beam_stops_at_the_roof_and_keeps_its_far_end() {
        // A four metre beam running along X through its own middle, and a wall
        // of roof standing across it at x = 1. The beam must come back to that
        // wall, and its far end must not budge - a part that shortened from both
        // ends would walk out of whatever joint it was seated in.
        let kind = PartKind::Beam(4.0, 0.0, 0.0);
        let record = a_record("beam-4".to_string(), [0.0, 0.0, 0.0], 0.0);
        let roofs = vec![(
            Vec3::new(1.5, 0.1875, 0.0),
            Vec3::new(0.5, 2.0, 2.0),
            Quat::IDENTITY,
        )];
        let (made, moved) = trim_to_roof(&kind, &record, &roofs).expect("it should trim");
        let PartKind::Beam(long, ..) = made else {
            panic!("a beam trims to a beam");
        };
        // The far end was at -2 and stays there; the near end stops at +1.
        let far = moved.at[0] - long * 0.5;
        let near = moved.at[0] + long * 0.5;
        assert!((far + 2.0).abs() < 0.07, "the far end moved to {far}");
        assert!((near - 1.0).abs() < 0.07, "the near end stopped at {near}");
        assert!(long < 4.0, "it did not come back at all: {long}");
    }

    /// The roof's boxes in world space, gathered exactly as the menu gathers
    /// them - so a fault in that gathering shows up here rather than in a
    /// maker's hands.
    fn roof_boxes(record: &Placed) -> Vec<(Vec3, Vec3, Quat)> {
        let kind = kind_from_name(&record.part).expect("a roof");
        let spin = pose(record.yaw, record.tilt, record.flip);
        body_of(&kind, None)
            .into_iter()
            .map(|Slab(offset, size, _, _, _, _, lean)| {
                let turn = spin * Quat::from_rotation_x(lean);
                (Vec3::from(record.at) + spin * offset, size * 0.5, turn)
            })
            .collect()
    }

    #[test]
    fn a_beam_is_trimmed_by_a_real_gable_roof() {
        // The case Brett actually has: a beam running up through the slope of a
        // gable roof, not through a box somebody wrote out by hand.
        let roof = Placed {
            part: part_name(&PartKind::GableRoof(8.0, 6.0, 0.25, 45.0)),
            at: [0.0, 2.5, 0.0],
            yaw: 0.0,
            tilt: 0.0,
            ramp: None,
            shade: 0.5,
            stage: "roof".to_string(),
            flip: false,
            loose: false,
            group: None,
        };
        let boxes = roof_boxes(&roof);
        assert!(!boxes.is_empty(), "the roof gathered no boxes at all");

        // A beam laid ACROSS the building, high enough to be inside the roof:
        // it runs out through the near slope, which is the thing to trim.
        //
        // Turned by yaw, not tilt. Tilt is a rotation ABOUT the part's own
        // length, so tilting a beam spins it on its axis and leaves it pointing
        // exactly where it was - which is how the first draft of this test came
        // to run a beam along the ground and conclude the roof was unreachable.
        let kind = PartKind::Beam(8.0, 0.0, 0.0);
        let beam = a_record(
            "beam-8".to_string(),
            [0.0, 4.0, 0.0],
            std::f32::consts::FRAC_PI_2,
        );
        let trimmed = trim_to_roof(&kind, &beam, &boxes);
        assert!(
            trimmed.is_some(),
            "a beam run up through the slope was not trimmed: the roof has \
             {} boxes and none of them were met",
            boxes.len()
        );
    }

    #[test]
    fn a_tie_beam_trims_to_the_slope_and_not_to_the_gable_it_sits_in() {
        // Brett's longhouse, by its own numbers. A tie beam laid across the
        // gable end, running out through the slope on one side. It SITS in the
        // gable - that is what a tie beam does - and the gable must not be
        // mistaken for something it is coming through.
        let kind = PartKind::Beam(6.4375, 0.0, 0.0);
        let beam = a_record("beam-6.4375".to_string(), [0.72, 2.88, 6.06], 0.0);
        // The file says 3.142; that is pi, and clippy would rather it be said so.
        let gable_turn = Quat::from_rotation_y(std::f32::consts::PI);
        let slope = |x: f32, lean: f32| {
            (
                Vec3::new(x, 4.3, 0.0),
                Vec3::new(13.375, 0.125, 5.1145) * 0.5,
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) * Quat::from_rotation_x(lean),
            )
        };
        let roofs = vec![
            (
                Vec3::new(1.03, 2.88, 6.0),
                Vec3::new(6.4375, 3.21875, 0.25) * 0.5,
                gable_turn,
            ),
            (
                Vec3::new(1.03, 2.88, -6.0),
                Vec3::new(6.4375, 3.21875, 0.25) * 0.5,
                gable_turn,
            ),
            slope(-0.73, -0.785),
            slope(2.8, 0.785),
        ];
        let (made, moved) = trim_to_roof(&kind, &beam, &roofs)
            .expect("the beam runs out through a slope and must come back to it");
        let PartKind::Beam(long, ..) = made else {
            panic!("a beam trims to a beam");
        };
        assert!(
            long < 6.4375,
            "it kept its whole length: the gable it sits in swallowed the answer"
        );

        // The property rather than the number: no corner of the trimmed beam may
        // stand ABOVE the roof's outer face. Not "inside the roof" - that was
        // the first version of this check and it passed while the bug was
        // present, because a stub poking through a panel an eighth of a metre
        // thick is on the far SIDE of it rather than within it. A green test
        // that cannot see the reported fault is worse than no test.
        let spin = pose(moved.yaw, moved.tilt, moved.flip);
        let middle = Vec3::from(moved.at);
        // Walked out of the beam's OWN BODY, corner by real corner. Assuming a
        // rectangle would fail now that a beam's ends are cut, because a mitre's
        // box takes in the very wood the saw removed.
        let mut points: Vec<Vec3> = Vec::new();
        for Slab(offset, size, _, _, _, shape, _) in body_of(&made, None) {
            let corners: Vec<Vec3> = match shape {
                // The prism's own six, not the eight of the box it came from.
                Shape::Mitre | Shape::MitreBack => {
                    let full = if shape == Shape::Mitre { -0.5 } else { 0.5 };
                    vec![
                        Vec3::new(full, -0.5, -0.5),
                        Vec3::new(full, -0.5, 0.5),
                        Vec3::new(full, 0.5, -0.5),
                        Vec3::new(full, 0.5, 0.5),
                        Vec3::new(-full, -0.5, -0.5),
                        Vec3::new(-full, -0.5, 0.5),
                    ]
                }
                _ => (0..8)
                    .map(|n| {
                        Vec3::new(
                            if n & 1 == 0 { -0.5 } else { 0.5 },
                            if n & 2 == 0 { -0.5 } else { 0.5 },
                            if n & 4 == 0 { -0.5 } else { 0.5 },
                        )
                    })
                    .collect(),
            };
            points.extend(
                corners
                    .into_iter()
                    .map(|corner| middle + spin * (offset + corner * size)),
            );
        }
        assert!(!points.is_empty(), "the beam has no body at all");
        for corner in points {
            {
                {
                    for (box_at, box_half, box_turn) in &roofs {
                        // The gable it sits in is not a fault; the slopes are.
                        if point_in_box(middle, *box_at, *box_half, *box_turn) {
                            continue;
                        }
                        let out = *box_turn * Vec3::Y;
                        let face = *box_at + out * box_half.y;
                        let above = (corner - face).dot(out);
                        assert!(
                            above <= 0.001,
                            "a corner stands {above:.3} above the roof: that is \
                             the stub on the outside of the slope"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_beam_that_reaches_no_roof_is_left_alone() {
        // Nothing to trim to is not a trim of nothing: the menu should have
        // offered it, and offering a deed that does nothing is the fault.
        let kind = PartKind::Beam(4.0, 0.0, 0.0);
        let record = a_record("beam-4".to_string(), [0.0, 0.0, 0.0], 0.0);
        let far_off = vec![(
            Vec3::new(40.0, 0.0, 0.0),
            Vec3::splat(1.0),
            Quat::IDENTITY,
        )];
        assert!(trim_to_roof(&kind, &record, &far_off).is_none());
        assert!(trim_to_roof(&kind, &record, &[]).is_none());
    }

    #[test]
    fn only_parts_with_a_length_can_be_trimmed() {
        // A chimney is the case Brett named: it is meant to come through, and
        // it has no length to come back along either.
        assert!(length_of(&PartKind::Beam(3.0, 0.0, 0.0)).is_some());
        assert!(length_of(&PartKind::Wall(2.0)).is_some());
        assert!(length_of(&PartKind::Chimney(1.75)).is_none());
        assert!(length_of(&PartKind::Prop("pole")).is_none());
    }

    #[test]
    fn a_work_is_known_by_either_name() {
        // The rename must never cost a maker a building: everything already on
        // disk wears `.json`, and the drawer has to go on finding it.
        assert!(is_a_work(std::path::Path::new("longhouse1-10people.baz")));
        assert!(is_a_work(std::path::Path::new("longhouse1-10people.json")));
        assert!(!is_a_work(std::path::Path::new("palette.png")));
        assert!(!is_a_work(std::path::Path::new("notes.txt")));
        assert_eq!(WORK_KIND, "baz", "the studio's own mark");
    }

    #[test]
    fn the_shelf_reads_in_order_and_says_the_noun_first() {
        // Two rules, and a test rather than good intentions: a shelf drifts one
        // well-meant entry at a time, and nobody notices until it is a jumble.
        for (drawer, entries) in [
            ("STRUCTURE", STRUCTURE),
            ("FURNITURE", FURNITURE),
            ("DECOR", DECOR),
        ] {
            let labels: Vec<&str> = entries.iter().map(|entry| entry.label).collect();
            let mut sorted = labels.clone();
            sorted.sort_unstable();
            assert_eq!(labels, sorted, "{drawer} is out of order");

            for label in labels {
                // The noun first, its qualifiers after commas: TRIM, STONE,
                // STRETCH - never STONE TRIM. So a family sorts together, which
                // is the whole reason for the order.
                let head = label.split(',').next().unwrap_or_default();
                assert!(
                    !head.contains(' '),
                    "{drawer}: \"{label}\" leads with something other than its \
                     noun; qualifiers go after a comma"
                );
            }
        }
    }

    #[test]
    fn a_work_from_before_stages_becomes_the_steps_it_already_rose_in() {
        // The migration is the whole risk of this format change: every building
        // on disk is a flat list, and it has to come back as exactly the steps
        // the village was already raising it in - not one more, not one fewer,
        // and with the finished work last.
        let flat: Vec<Placed> = [
            ("prop:foundation", "footing"),
            ("prop:pole", "frame"),
            ("wall-2", "walls"),
            ("gableroof-6x4x0.25x30", "roof"),
            ("widget:door", "widget"),
        ]
        .iter()
        .map(|(part, stage)| Placed {
            part: part.to_string(),
            at: [0.0, 0.0, 0.0],
            yaw: 0.0,
            tilt: 0.0,
            ramp: None,
            shade: 0.5,
            stage: stage.to_string(),
            flip: false,
            loose: false,
            group: None,
        })
        .collect();

        let stages = stages_from_flat(&flat);
        assert_eq!(stages.len(), 4, "a framed work rises in four steps");
        // Each step holds everything raised so far, and the marks throughout.
        let counts: Vec<usize> = stages.iter().map(Vec::len).collect();
        assert_eq!(counts, vec![2, 3, 4, 5], "steps: {counts:?}");
        assert!(
            stages.last().unwrap().len() == flat.len(),
            "the last step must be the whole finished work"
        );
        for (step, drawing) in stages.iter().enumerate() {
            assert!(
                drawing.iter().any(|record| record.stage == "widget"),
                "the maker's marks are missing from step {step}"
            );
        }

        // And with no frame drawn, three steps rather than four - the case the
        // game had to special-case, and the reason this rule is copied exactly.
        let unframed: Vec<Placed> = flat
            .iter()
            .filter(|record| record.stage != "frame")
            .cloned()
            .collect();
        assert_eq!(
            stages_from_flat(&unframed).len(),
            3,
            "a work with no frame rises in three"
        );
    }

    #[test]
    fn a_roof_comes_apart_into_parts_that_exist() {
        // Ungroup is only offered where the pieces have somewhere to go. If a
        // kind is ever added here without a part to become, this is where it
        // shows up rather than in a maker's hands.
        let comes_apart =
            |kind: &PartKind| deeds_for(kind).iter().any(|deed| *deed == Deed::Ungroup);
        assert!(
            comes_apart(&PartKind::GableRoof(6.0, 4.0, 0.25, 45.0)),
            "a gable roof is two roof panels and two gables, so it comes apart"
        );
        assert!(
            !comes_apart(&PartKind::Prop("door")),
            "a door is jambs and a leaf and the bench has a part for neither: \
             breaking one up would leave a hole where a door used to be"
        );
        assert!(!comes_apart(&PartKind::Wall(2.0)), "a wall is a wall");

        // And every part can be told what it is, whether or not it comes apart.
        for kind in [PartKind::Wall(2.0), PartKind::GableRoof(6.0, 4.0, 0.25, 30.0)] {
            for nature in NATURES {
                assert!(
                    deeds_for(&kind).contains(&Deed::Nature(nature)),
                    "{nature} is not on offer for every part"
                );
            }
        }
    }

    #[test]
    fn a_gable_meets_the_roof_it_stands_under() {
        // The promise the old comment made and could no longer keep. A gable's
        // peak has to rise as far as the roof's ridge over the same width, or a
        // steepened roof leaves daylight over the wall beneath it.
        for pitch in [20.0f32, 30.0, 45.0, 60.0] {
            for long in [4.0f32, 7.0, 9.5] {
                let roof = long * 0.5 * pitch.to_radians().tan();
                let gable = gable_peak(long, pitch);
                assert!(
                    (gable - roof).abs() < 0.07,
                    "a {long} metre gable at {pitch} degrees stands {gable} \
                     against a ridge at {roof}"
                );
            }
        }
    }

    #[test]
    fn a_gable_drawn_before_pitch_existed_still_opens() {
        let Some(PartKind::Gable(long, pitch)) = kind_from_name("gable-7.5") else {
            panic!("the older gables no longer open");
        };
        assert_eq!((long, pitch), (7.5, ROOF_PITCH_DEGREES));
        let name = part_name(&PartKind::Gable(7.5, 45.0));
        let Some(PartKind::Gable(back, degrees)) = kind_from_name(&name) else {
            panic!("{name} did not come back");
        };
        assert_eq!((back, degrees), (7.5, 45.0));
    }

    #[test]
    fn a_gable_stands_the_right_way_up() {
        // Straight from the body, so it catches the sign as well as the size.
        let tall = body_of(&PartKind::Gable(4.0, ROOF_PITCH_DEGREES), None)
            .iter()
            .map(|Slab(_, size, ..)| size.y)
            .fold(f32::MIN, f32::max);
        assert!(
            tall > 0.0,
            "a gable of four metres has a height of {tall}: it is inside out"
        );
        assert!(
            (tall - 1.125).abs() < 0.1,
            "a four metre gable at thirty degrees should stand about 1.15 tall, \
             not {tall}"
        );
    }

    #[test]
    fn a_roof_with_a_pitch_survives_the_round_trip() {
        for degrees in [10.0, 22.5, 30.0, 45.0, 60.0] {
            let made = PartKind::GableRoof(8.0, 6.0, 0.375, degrees);
            let name = part_name(&made);
            let Some(PartKind::GableRoof(long, span, over, back)) = kind_from_name(&name) else {
                panic!("{name} did not come back");
            };
            assert_eq!(
                (long, span, over, back),
                (8.0, 6.0, 0.375, degrees),
                "{name}"
            );
        }
    }

    #[test]
    fn an_older_roof_still_opens_without_its_eaves() {
        // Two numbers: from before the eaves could be pulled either.
        let Some(PartKind::GableRoof(long, span, over, pitch)) = kind_from_name("gableroof-6x4")
        else {
            panic!("the oldest roofs no longer open");
        };
        assert_eq!((long, span, over, pitch), (6.0, 4.0, 0.25, ROOF_PITCH_DEGREES));
    }

    /// The highest point anything in a roof reaches: the ridge.
    ///
    /// Measured through each piece's own lean, because the slopes are tilted
    /// boxes and half their height is not their top.
    fn ridge_top(span: f32, over: f32, pitch: f32) -> f32 {
        body_of(&PartKind::GableRoof(6.0, span, over, pitch), None)
            .iter()
            .map(|Slab(at, size, _, _, _, _, lean)| {
                let reach = (size.y * lean.cos()).abs() + (size.z * lean.sin()).abs();
                at.y + reach * 0.5
            })
            .fold(f32::MIN, f32::max)
    }

    #[test]
    fn pulling_the_eaves_does_not_lift_the_roof() {
        // The overhang reaches further DOWN the slope; it does not raise the
        // ridge. A roof that climbed as its eaves were pulled left daylight
        // between itself and the gable it sits on.
        for pitch in [20.0, 30.0, 45.0, 60.0] {
            let seated = ridge_top(7.0, 0.0, pitch);
            for over in [0.25, 0.5, 1.0, 2.0] {
                let lifted = ridge_top(7.0, over, pitch);
                assert!(
                    (lifted - seated).abs() < 1e-3,
                    "at {pitch} degrees an overhang of {over} moved the ridge \
                     from {seated} to {lifted}"
                );
            }
        }
    }

    #[test]
    fn the_pitch_a_handle_can_reach_is_the_pitch_a_roof_can_hold() {
        // That the arming pitch is reachable is checked at compile time, beside
        // the constants. This is the other half: that the reachable pitches are
        // a whole number of steps apart.
        assert!(
            ((PITCH_MOST - PITCH_LEAST) / PITCH_STEP).fract() < 1e-6,
            "the range is not a whole number of steps, so the steepest pitch \
             cannot be reached by stepping from the shallowest"
        );
    }
}
