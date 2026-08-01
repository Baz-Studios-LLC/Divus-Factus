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

/// The bench's standard roof pitch: forty-five degrees, where the rise
/// equals the run and the gable's peak is half its width. Roofs arm
/// already pitched; T walks away from it in fifteens.
const ROOF_PITCH: f32 = std::f32::consts::FRAC_PI_4;

/// The Atelier's own measurements - the source of truth now; the game
/// conforms to these when its buildings are replaced. A quarter-metre
/// wall on a quarter-metre grid means centrelines always land on snaps.
const WALL_THICK: f32 = 0.25;
const WALL_HIGH: f32 = 2.5;

/// One piece of a part's body: offset from the part origin, size, ramp,
/// shade, how much of the world shows through it (1.0 = none), and
/// whether it is a wedge rather than a box - a triangular prism, for
/// the honest slopes a gable wants.
struct Slab(Vec3, Vec3, String, f32, f32, bool);

/// A triangular prism: an isosceles gable end, base to apex, extruded
/// across its thickness. Unit-sized, so a part scales it like any box.
fn wedge_mesh() -> Mesh {
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
    Gable(f32),
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
    GableRun,
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
            PartKind::WallRun | PartKind::TrimRun { .. } | PartKind::GableRun => Some(1),
            PartKind::FloorRun | PartKind::FoundationRun | PartKind::RoofRun => Some(2),
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
            PartKind::GableRun => PartKind::Gable(w),
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
    structure("WALL, STRETCH", PartKind::WallRun, "walls"),
    structure("WALL, 2M", PartKind::Wall(2.0), "walls"),
    structure("CORNER POLE", PartKind::Prop("pole"), "frame"),
    structure("TRIM, STRETCH", PartKind::TrimRun { stone: false }, "walls"),
    structure(
        "TRIM STONE, STRETCH",
        PartKind::TrimRun { stone: true },
        "walls",
    ),
    structure("TRIM CORNER", PartKind::Prop("trim-corner"), "walls"),
    structure(
        "TRIM CORNER, STONE",
        PartKind::Prop("trim-corner-stone"),
        "walls",
    ),
    structure("DOOR", PartKind::Prop("door"), "walls"),
    structure("DOORWAY", PartKind::Prop("doorway"), "walls"),
    structure("WINDOW", PartKind::Prop("window"), "walls"),
    structure("FOUNDATION, STRETCH", PartKind::FoundationRun, "footing"),
    structure("FOUNDATION, 2M", PartKind::Foundation(2.0, 2.0), "footing"),
    structure("STONE STEPS", PartKind::Prop("steps"), "footing"),
    structure("FLOOR, STRETCH", PartKind::FloorRun, "footing"),
    structure("FLOOR, 2M", PartKind::Floor(2.0, 2.0), "footing"),
    structure("GABLE, STRETCH", PartKind::GableRun, "walls"),
    structure("ROOF, STRETCH", PartKind::RoofRun, "roof"),
    structure("ROOF PANEL", PartKind::Roof(2.2, 2.2), "roof"),
];

pub const FURNITURE: &[CatalogEntry] = &[
    prop("BED", "bed"),
    prop("BED, DOUBLE", "bed-double"),
    prop("CRADLE", "cradle"),
    prop("WARDROBE", "wardrobe"),
    prop("SIDE TABLE", "side-table"),
    prop("TABLE", "table"),
    prop("STOOL", "stool"),
    prop("CHAIR", "chair"),
    prop("BENCH", "bench"),
    prop("HEARTH", "hearth"),
    prop("CHEST", "chest"),
    prop("SHELVES", "shelves"),
    prop("CUPBOARD", "cupboard"),
];

pub const DECOR: &[CatalogEntry] = &[
    prop("MANNEQUIN", "mannequin"),
    prop("ANVIL", "anvil"),
    prop("LOOM", "loom"),
    prop("PLANTER", "planter"),
    prop("FENCE", "fence"),
    prop("LADDER", "ladder"),
    prop("BARREL", "barrel"),
    prop("CRATE", "crate"),
    prop("COOKING POT", "pot"),
    prop("BASKET", "basket"),
    prop("RUG", "rug"),
    prop("WOODPILE", "woodpile"),
    prop("CANDLE STAND", "candle"),
    prop("SACK", "sack"),
    prop("TROUGH", "trough"),
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

/// The bench's ready-made starts: authored files under templates/.
pub const TEMPLATES: &[(&str, &str)] = &[("HOUSE", "house"), ("LONGHOUSE", "longhouse")];

/// The boxes a part is made of, in its own local space, resting on y = 0.
fn body_of(kind: &PartKind, repaint: Option<(&str, f32)>) -> Vec<Slab> {
    let slab = |x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, ramp: &str, shade: f32| {
        Slab(
            Vec3::new(x, y, z),
            Vec3::new(sx, sy, sz),
            ramp.to_string(),
            shade,
            1.0,
            false,
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
            true,
        )
    };
    // Glass: the world shows through it.
    let glass = |x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, ramp: &str, shade: f32| {
        Slab(
            Vec3::new(x, y, z),
            Vec3::new(sx, sy, sz),
            ramp.to_string(),
            shade,
            0.35,
            false,
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
        PartKind::Seg { long, high, lift } => vec![slab(
            0.0,
            lift + high * 0.5,
            0.0,
            *long,
            *high,
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
        PartKind::Gable(long) => {
            // One clean slope each way: the peak rises at the bench's own
            // pitch, so a 45 degree gable stands half as tall as it is
            // wide and meets the roof panels exactly.
            let high = ((long * 0.5 * ROOF_PITCH.tan()) * 16.0).round() / 16.0;
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
            // The game's own bed: frame, mattress, pillow at +Z (the head).
            slab(0.0, 0.26, 0.0, 0.76, 0.24, 1.64, "wood", 0.55),
            slab(0.0, 0.44, 0.0, 0.62, 0.18, 1.5, "bone", 0.8),
            slab(0.0, 0.56, 0.55, 0.46, 0.1, 0.32, "bone", 0.95),
        ],
        PartKind::Prop("bed-double") => vec![
            // Room for two: the single's proportions, widened, with a
            // pillow for each head at +Z.
            slab(0.0, 0.26, 0.0, 1.29, 0.24, 1.64, "wood", 0.55),
            slab(0.0, 0.44, 0.0, 1.15, 0.18, 1.5, "bone", 0.8),
            slab(-0.27, 0.56, 0.55, 0.42, 0.1, 0.32, "bone", 0.95),
            slab(0.27, 0.56, 0.55, 0.42, 0.1, 0.32, "bone", 0.95),
        ],
        PartKind::Prop("table") => {
            let mut parts = vec![slab(0.0, 0.72, 0.0, 1.5, 0.1, 0.9, "wood", 0.65)];
            for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                parts.push(slab(
                    sx * 0.62,
                    0.34,
                    sz * 0.32,
                    0.1,
                    0.68,
                    0.1,
                    "wood",
                    0.5,
                ));
            }
            parts
        }
        PartKind::Prop("stool") => vec![
            slab(0.0, 0.4, 0.0, 0.38, 0.07, 0.38, "wood", 0.6),
            slab(0.0, 0.18, 0.0, 0.3, 0.36, 0.3, "wood", 0.45),
        ],
        PartKind::Prop("hearth") => vec![
            slab(0.0, 0.42, 0.0, 0.9, 0.84, 0.6, "stone", 0.6),
            slab(0.0, 0.55, 0.12, 0.62, 0.5, 0.44, "stone", 0.25),
        ],
        PartKind::Prop("chair") => vec![
            slab(0.0, 0.4, 0.0, 0.4, 0.07, 0.4, "wood", 0.6),
            slab(0.0, 0.18, 0.0, 0.32, 0.36, 0.32, "wood", 0.45),
            slab(0.0, 0.72, -0.17, 0.4, 0.72, 0.07, "wood", 0.55),
        ],
        PartKind::Prop("bench") => vec![
            slab(0.0, 0.4, 0.0, 1.2, 0.08, 0.36, "wood", 0.6),
            slab(-0.5, 0.18, 0.0, 0.09, 0.36, 0.3, "wood", 0.45),
            slab(0.5, 0.18, 0.0, 0.09, 0.36, 0.3, "wood", 0.45),
        ],
        PartKind::Prop("chest") => vec![
            slab(0.0, 0.25, 0.0, 0.8, 0.5, 0.5, "wood", 0.5),
            slab(0.0, 0.52, 0.0, 0.84, 0.1, 0.54, "wood", 0.35),
            slab(0.0, 0.33, 0.26, 0.1, 0.16, 0.04, "cloth-gold", 0.7),
        ],
        PartKind::Prop("barrel") => vec![
            slab(0.0, 0.36, 0.0, 0.55, 0.72, 0.55, "wood", 0.55),
            slab(0.0, 0.16, 0.0, 0.59, 0.07, 0.59, "stone", 0.45),
            slab(0.0, 0.56, 0.0, 0.59, 0.07, 0.59, "stone", 0.45),
        ],
        PartKind::Prop("crate") => vec![
            slab(0.0, 0.3, 0.0, 0.6, 0.6, 0.6, "wood", 0.6),
            slab(0.0, 0.61, 0.0, 0.52, 0.03, 0.52, "wood", 0.4),
        ],
        PartKind::Prop("shelves") => vec![
            slab(-0.42, 0.8, 0.0, 0.06, 1.6, 0.3, "wood", 0.5),
            slab(0.42, 0.8, 0.0, 0.06, 1.6, 0.3, "wood", 0.5),
            slab(0.0, 0.5, 0.0, 0.9, 0.05, 0.3, "wood", 0.65),
            slab(0.0, 1.0, 0.0, 0.9, 0.05, 0.3, "wood", 0.65),
            slab(0.0, 1.5, 0.0, 0.9, 0.05, 0.3, "wood", 0.65),
        ],
        PartKind::Prop("cupboard") => vec![
            slab(0.0, 0.75, 0.0, 0.9, 1.5, 0.45, "wood", 0.5),
            slab(0.0, 0.75, 0.24, 0.82, 1.34, 0.04, "wood", 0.65),
            slab(0.12, 0.75, 0.27, 0.05, 0.16, 0.03, "cloth-gold", 0.6),
        ],
        PartKind::Prop("pot") => vec![
            slab(0.0, 0.2, 0.0, 0.4, 0.4, 0.4, "stone", 0.3),
            slab(0.0, 0.42, 0.0, 0.46, 0.06, 0.46, "stone", 0.45),
        ],
        PartKind::Prop("basket") => vec![
            slab(0.0, 0.15, 0.0, 0.45, 0.3, 0.45, "sand", 0.55),
            slab(0.0, 0.31, 0.0, 0.5, 0.05, 0.5, "sand", 0.4),
        ],
        PartKind::Prop("rug") => vec![
            slab(0.0, 0.015, 0.0, 1.4, 0.03, 0.9, "cloth-red", 0.55),
            slab(0.0, 0.032, 0.0, 1.1, 0.01, 0.62, "cloth-red", 0.75),
        ],
        PartKind::Prop("woodpile") => vec![
            slab(0.0, 0.11, 0.0, 1.0, 0.22, 0.66, "wood", 0.4),
            slab(0.0, 0.32, 0.0, 1.0, 0.2, 0.5, "wood", 0.5),
            slab(0.0, 0.5, 0.0, 1.0, 0.18, 0.32, "wood", 0.6),
        ],
        PartKind::Prop("candle") => vec![
            slab(0.0, 0.02, 0.0, 0.3, 0.05, 0.3, "stone", 0.5),
            slab(0.0, 0.6, 0.0, 0.07, 1.1, 0.07, "wood", 0.4),
            slab(0.0, 1.18, 0.0, 0.12, 0.14, 0.12, "bone", 0.95),
            slab(0.0, 1.3, 0.0, 0.07, 0.1, 0.07, "cloth-gold", 0.95),
        ],
        PartKind::Prop("sack") => vec![
            slab(0.0, 0.21, 0.0, 0.42, 0.42, 0.42, "bone", 0.6),
            slab(0.0, 0.46, 0.0, 0.18, 0.12, 0.18, "bone", 0.45),
        ],
        PartKind::Prop("trough") => vec![
            slab(0.0, 0.15, 0.0, 1.2, 0.3, 0.45, "wood", 0.45),
            slab(0.0, 0.27, 0.0, 1.08, 0.04, 0.33, "water", 0.7),
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
            slab(0.0, 1.0, 0.03125, 1.0, 2.0, 0.0625, "wood", 0.35),
            slab(
                0.375,
                1.0,
                0.0625,
                0.0625,
                0.0625,
                0.0625,
                "cloth-gold",
                0.8,
            ),
        ],
        PartKind::Prop("doorway") => vec![
            // An opening with no leaf: jambs and a lintel, for the ways
            // between rooms that never wanted a door.
            slab(-0.5625, 1.0, 0.0, 0.125, 2.0, 0.375, "wood", 0.45),
            slab(0.5625, 1.0, 0.0, 0.125, 2.0, 0.375, "wood", 0.45),
            slab(0.0, 2.0625, 0.0, 1.25, 0.125, 0.375, "wood", 0.45),
        ],
        PartKind::Prop("window") => vec![
            // Frame boards around the opening and a pale pane within it,
            // all on the lattice.
            slab(-0.5625, 1.375, 0.0, 0.125, 1.125, 0.375, "wood", 0.45),
            slab(0.5625, 1.375, 0.0, 0.125, 1.125, 0.375, "wood", 0.45),
            slab(0.0, 0.8125, 0.0, 1.25, 0.125, 0.375, "wood", 0.45),
            slab(0.0, 1.9375, 0.0, 1.25, 0.125, 0.375, "wood", 0.45),
            glass(0.0, 1.375, 0.0, 1.0, 1.0, 0.0625, "sky", 0.8),
            slab(0.0, 1.375, 0.03125, 0.0625, 1.0, 0.0625, "wood", 0.5),
            slab(0.0, 1.375, 0.03125, 1.0, 0.0625, 0.0625, "wood", 0.5),
        ],
        PartKind::Prop("trim-corner") => vec![
            // An L that wraps an outside corner: two legs meeting at the
            // origin, turned to face with R. Runs of straight trim meet
            // its ends on the grid.
            slab(0.1875, 0.15625, 0.0, 0.375, 0.3125, 0.125, "wood", 0.5),
            slab(0.0, 0.15625, 0.1875, 0.125, 0.3125, 0.375, "wood", 0.5),
        ],
        PartKind::Prop("trim-corner-stone") => vec![
            slab(0.1875, 0.15625, 0.0, 0.375, 0.3125, 0.125, "stone", 0.55),
            slab(0.0, 0.15625, 0.1875, 0.125, 0.3125, 0.375, "stone", 0.55),
        ],
        PartKind::Prop("steps") => vec![
            // Three treads rising the foundation's 0.375, from +X.
            slab(0.375, 0.0625, 0.0, 0.375, 0.125, 1.25, "stone", 0.6),
            slab(0.0, 0.125, 0.0, 0.375, 0.25, 1.25, "stone", 0.55),
            slab(-0.375, 0.1875, 0.0, 0.375, 0.375, 1.25, "stone", 0.5),
        ],
        PartKind::Prop("cradle") => vec![
            slab(0.0, 0.3, 0.0, 0.55, 0.3, 0.9, "wood", 0.55),
            slab(0.0, 0.42, 0.3, 0.45, 0.1, 0.25, "bone", 0.9),
            slab(0.0, 0.08, -0.38, 0.6, 0.06, 0.12, "wood", 0.4),
            slab(0.0, 0.08, 0.38, 0.6, 0.06, 0.12, "wood", 0.4),
        ],
        PartKind::Prop("wardrobe") => vec![
            slab(0.0, 0.95, 0.0, 1.1, 1.9, 0.5, "wood", 0.5),
            slab(-0.26, 0.95, 0.26, 0.48, 1.7, 0.04, "wood", 0.62),
            slab(0.26, 0.95, 0.26, 0.48, 1.7, 0.04, "wood", 0.62),
            slab(0.0, 0.95, 0.28, 0.04, 0.3, 0.04, "cloth-gold", 0.7),
        ],
        PartKind::Prop("side-table") => vec![
            slab(0.0, 0.55, 0.0, 0.6, 0.08, 0.6, "wood", 0.65),
            slab(-0.24, 0.26, -0.24, 0.08, 0.52, 0.08, "wood", 0.5),
            slab(0.24, 0.26, -0.24, 0.08, 0.52, 0.08, "wood", 0.5),
            slab(-0.24, 0.26, 0.24, 0.08, 0.52, 0.08, "wood", 0.5),
            slab(0.24, 0.26, 0.24, 0.08, 0.52, 0.08, "wood", 0.5),
        ],
        PartKind::Prop("anvil") => vec![
            slab(0.0, 0.15, 0.0, 0.45, 0.3, 0.35, "wood", 0.4),
            slab(0.0, 0.38, 0.0, 0.22, 0.16, 0.22, "stone", 0.3),
            slab(0.0, 0.53, 0.0, 0.7, 0.14, 0.26, "stone", 0.45),
        ],
        PartKind::Prop("loom") => vec![
            slab(-0.5, 0.7, 0.0, 0.08, 1.4, 0.08, "wood", 0.5),
            slab(0.5, 0.7, 0.0, 0.08, 1.4, 0.08, "wood", 0.5),
            slab(0.0, 1.32, 0.0, 1.08, 0.08, 0.08, "wood", 0.6),
            slab(0.0, 0.35, 0.0, 1.08, 0.08, 0.08, "wood", 0.6),
            slab(0.0, 0.82, 0.0, 0.9, 0.9, 0.03, "cloth-red", 0.6),
        ],
        PartKind::Prop("planter") => vec![
            slab(0.0, 0.15, 0.0, 0.9, 0.3, 0.35, "earth", 0.4),
            slab(-0.22, 0.38, 0.0, 0.2, 0.18, 0.2, "grass", 0.6),
            slab(0.1, 0.42, 0.05, 0.24, 0.26, 0.22, "grass", 0.5),
            slab(0.32, 0.36, -0.04, 0.16, 0.14, 0.16, "grass", 0.7),
        ],
        PartKind::Prop("fence") => vec![
            slab(-0.65, 0.45, 0.0, 0.09, 0.9, 0.09, "wood", 0.45),
            slab(0.65, 0.45, 0.0, 0.09, 0.9, 0.09, "wood", 0.45),
            slab(0.0, 0.7, 0.0, 1.5, 0.08, 0.06, "wood", 0.55),
            slab(0.0, 0.38, 0.0, 1.5, 0.08, 0.06, "wood", 0.55),
        ],
        PartKind::Prop("ladder") => vec![
            slab(-0.16, 1.2, 0.0, 0.07, 2.4, 0.07, "wood", 0.5),
            slab(0.16, 1.2, 0.0, 0.07, 2.4, 0.07, "wood", 0.5),
            slab(0.0, 0.4, 0.0, 0.36, 0.06, 0.06, "wood", 0.6),
            slab(0.0, 0.85, 0.0, 0.36, 0.06, 0.06, "wood", 0.6),
            slab(0.0, 1.3, 0.0, 0.36, 0.06, 0.06, "wood", 0.6),
            slab(0.0, 1.75, 0.0, 0.36, 0.06, 0.06, "wood", 0.6),
            slab(0.0, 2.2, 0.0, 0.36, 0.06, 0.06, "wood", 0.6),
        ],
        PartKind::Prop("mannequin") => vec![
            // The game's adult, boxed in bone: a measuring stick with a
            // face. Skipped on import - reference, not furniture.
            slab(-0.11, 0.31, 0.0, 0.14, 0.62, 0.14, "bone", 0.6),
            slab(0.11, 0.31, 0.0, 0.14, 0.62, 0.14, "bone", 0.6),
            slab(0.0, 0.9, 0.0, 0.43, 0.55, 0.25, "bone", 0.75),
            slab(-0.27, 0.88, 0.0, 0.1, 0.52, 0.1, "bone", 0.6),
            slab(0.27, 0.88, 0.0, 0.1, 0.52, 0.1, "bone", 0.6),
            slab(0.0, 1.42, 0.0, 0.46, 0.46, 0.46, "bone", 0.85),
        ],
        PartKind::Prop(_) => vec![],
        PartKind::Widget(name) => {
            let (_, ramp, shade) = WIDGETS
                .iter()
                .find(|(w, _, _)| w == name)
                .copied()
                .unwrap_or(("", "bone", 0.5));
            vec![
                slab(0.0, 0.2, 0.0, 0.4, 0.4, 0.4, ramp, shade),
                // The nose: which way the widget faces.
                slab(0.3, 0.2, 0.0, 0.2, 0.12, 0.12, ramp, shade),
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
}

/// A part's turn: yaw, then tilt - which leans the other way when the
/// part is mirrored, so a pitched panel's twin completes the gable.
fn pose(yaw: f32, tilt: f32, flip: bool) -> Quat {
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
            ROOF_PITCH
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
        })
    }
}

/// A shelf button holding one catalog entry.
#[derive(Component)]
struct ShelfButton(&'static CatalogEntry);

/// A shelf button holding one widget.
#[derive(Component)]
struct WidgetButton(&'static str);

/// A button that loads a ready-made start onto a cleared bench.
#[derive(Component)]
struct TemplateButton(&'static str);

/// A button that loads a saved work file back onto a cleared bench.
#[derive(Component)]
struct LoadFileButton(std::path::PathBuf);

/// The small x beside a saved work: pressed once it asks, pressed again
/// while asking it deletes the file and the row.
#[derive(Component)]
struct DeleteFileButton {
    path: std::path::PathBuf,
    row: Entity,
    /// Pressed once, the button stays armed until this moment passes.
    armed_until: f32,
}

/// The button that sweeps the bench bare.
#[derive(Component)]
struct ClearButton;

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
struct SaveButton;

/// The save button's label, so it can say what just happened.
#[derive(Component)]
struct SaveLabel;

/// The name this work goes by, once it has been given one. Saving again
/// updates the same file instead of scattering copies.
#[derive(Resource, Default)]
pub struct WorkName(pub Option<String>);

/// A label speaking a passing word; it returns to its old text at `until`.
#[derive(Component)]
struct PassingWord {
    back: &'static str,
    until: f32,
}

/// The SAVED WORK drawer's body, so a fresh export can join it live.
#[derive(Resource)]
struct SavedWorkDrawer(Entity);

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
            .init_resource::<Naming>()
            .init_resource::<Hovered>()
            .init_resource::<WorkName>()
            .init_resource::<SnapMode>()
            .init_resource::<DimsEntry>()
            .init_resource::<History>()
            .init_resource::<SnapGrid>()
            .init_resource::<Clipboard>()
            .add_systems(Startup, raise_shelf.after(crate::rail::raise_rail))
            .add_systems(
                Update,
                (
                    show_shelf,
                    work_drawers,
                    work_shelf,
                    work_templates,
                    steer_hand,
                    toggle_snap_mode,
                    disarm_on_mode,
                    copy_and_paste,
                    mirror_part,
                    feel_ahead,
                    move_ghost,
                    place_grab_remove,
                    save_workbench,
                    take_the_name,
                    dims_panel,
                    recall,
                    remember,
                    bury_saved_work,
                    settle_words,
                )
                    .chain(),
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
        PartKind::Gable(long) => format!("gable-{long}"),
        PartKind::Floor(w, d) => format!("floor-{w}x{d}"),
        PartKind::Foundation(w, d) => format!("foundation-{w}x{d}"),
        PartKind::Roof(w, d) => format!("roof-{w}x{d}"),
        PartKind::WallRun
        | PartKind::TrimRun { .. }
        | PartKind::GableRun
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
    if let Some(rest) = name.strip_prefix("gable-") {
        return rest.parse::<f32>().ok().map(PartKind::Gable);
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
    for Slab(mut at, size, ramp, shade, clarity, is_wedge) in body_of(kind, repaint) {
        // Mirrored: the body reflects across its own length.
        if record.flip {
            at.x = -at.x;
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
            Mesh3d(if is_wedge {
                meshes.add(wedge_mesh())
            } else {
                meshes.add(Cuboid::new(1.0, 1.0, 1.0))
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
            Transform::from_translation(at).with_scale(size),
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

// ---------------------------------------------------------------- the shelf

fn raise_shelf(
    mut commands: Commands,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
    file_home: Option<Res<crate::rail::FileHome>>,
) {
    let shelf = commands
        .spawn((
            Shelf,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(176.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(3.0),
                border: UiRect::left(Val::Px(1.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
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

    // The file work lives on the left rail when it offers a home; the
    // shelf keeps only parts and widgets.
    let files = file_home.map(|home| home.0).unwrap_or(shelf);

    // READY-MADE: templates and the broom.
    let ready = drawer(&mut commands, &fonts, &palette, files, "READY-MADE", true);
    for (label, name) in TEMPLATES {
        let button = plain_button(&mut commands, &palette, ready);
        commands.entity(button).insert(TemplateButton(name));
        button_label(&mut commands, &fonts, &palette, button, label);
    }
    let clear = plain_button(&mut commands, &palette, ready);
    commands.entity(clear).insert(ClearButton);
    button_label(&mut commands, &fonts, &palette, clear, "CLEAR THE BENCH");

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
    // SAVED WORK: whatever exports already stand in out/buildings/.
    let saved = drawer(&mut commands, &fonts, &palette, files, "SAVED WORK", false);
    commands.insert_resource(SavedWorkDrawer(saved));
    if let Some(dir) = bench_path().parent()
        && let Ok(entries) = std::fs::read_dir(dir)
    {
        let mut names: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "json"))
            .collect();
        names.sort();
        for path in names {
            let label = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_uppercase())
                .unwrap_or_default();
            saved_work_row(
                &mut commands,
                &fonts,
                &palette,
                saved,
                path,
                Box::leak(label.into_boxed_str()),
            );
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

    // The save at the shelf's foot.
    let save = commands
        .spawn((
            SaveButton,
            Interaction::default(),
            Node {
                margin: UiRect::top(Val::Px(12.0)),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::accent(&palette).with_alpha(0.7)),
            ChildOf(files),
        ))
        .id();
    commands.spawn((
        SaveLabel,
        Text::new("SAVE THE WORK"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(theme::accent(&palette)),
        ChildOf(save),
    ));
    commands.spawn((
        Text::new(
            "build anywhere: the door\nwidget decides the front.\nthe gold marks +X if you\nlike to work oriented.",
        ),
        TextFont {
            font: fonts.text.clone().into(),
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(theme::text_dim(&palette).with_alpha(0.75)),
        Node {
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        },
        ChildOf(shelf),
    ));
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
fn saved_work_row(
    commands: &mut Commands,
    fonts: &Fonts,
    palette: &Palette,
    drawer_body: Entity,
    path: std::path::PathBuf,
    label: &'static str,
) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(3.0),
                align_items: AlignItems::Stretch,
                ..default()
            },
            ChildOf(drawer_body),
        ))
        .id();
    let load = commands
        .spawn((
            LoadFileButton(path.clone()),
            Interaction::default(),
            Node {
                flex_grow: 1.0,
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::panel_border(palette)),
            ChildOf(row),
        ))
        .id();
    button_label(commands, fonts, palette, load, label);
    let bury = commands
        .spawn((
            DeleteFileButton {
                path,
                row,
                armed_until: 0.0,
            },
            Interaction::default(),
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::panel_border(palette).with_alpha(0.25)),
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        Text::new("x"),
        TextFont {
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(theme::text_dim(palette).with_alpha(0.7)),
        ChildOf(bury),
    ));
}

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

fn button_label(
    commands: &mut Commands,
    fonts: &Fonts,
    palette: &Palette,
    button: Entity,
    label: &'static str,
) {
    commands.spawn((
        Text::new(label),
        TextFont {
            font: fonts.text.clone().into(),
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(theme::text_dim(palette)),
        ChildOf(button),
    ));
}

/// The shelf belongs to the Builder bench alone.
fn show_shelf(bench: Res<Bench>, mut shelves: Query<&mut Visibility, With<Shelf>>) {
    if !bench.is_changed() {
        return;
    }
    for mut visibility in &mut shelves {
        *visibility = if *bench == Bench::Builder {
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
fn work_templates(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
    templates: Query<(&Interaction, &TemplateButton), Changed<Interaction>>,
    files: Query<(&Interaction, &LoadFileButton), Changed<Interaction>>,
    clears: Query<&Interaction, (Changed<Interaction>, With<ClearButton>)>,
    standing: Query<Entity, (With<Placed>, Without<Ghost>)>,
    mut work_name: ResMut<WorkName>,
) {
    let base = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let mut from_file = false;
    let wanted = templates
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, template)| {
            std::path::PathBuf::from(&base).join(format!("templates/{}.json", template.0))
        })
        .or_else(|| {
            files
                .iter()
                .find(|(interaction, _)| **interaction == Interaction::Pressed)
                .map(|(_, file)| {
                    from_file = true;
                    file.0.clone()
                })
        });
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
    // A loaded work carries its name; a template starts nameless.
    work_name.0 = if from_file {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
    } else {
        None
    };
    match std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Workbench>(&text).ok())
    {
        Some(bench) => {
            let count = bench.parts.len();
            for record in bench.parts {
                if let Some(kind) = kind_from_name(&record.part) {
                    spawn_part(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &palette,
                        &kind,
                        &record,
                        false,
                    );
                }
            }
            info!("set out {}: {count} parts", path.display());
        }
        None => warn!("nothing readable at {}", path.display()),
    }
}

// ---------------------------------------------------------------- the hand

/// Keys that steer what the hand holds. Esc empties it.
#[allow(clippy::too_many_arguments)]
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
    if keys.just_pressed(KeyCode::Escape) {
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
        hand.tilt = (hand.tilt + 15f32.to_radians()).rem_euclid(std::f32::consts::FRAC_PI_2);
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
            | PartKind::Gable(..)
            | PartKind::GableRun
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
    placed: &Query<(Entity, &Transform, &Placed), Without<Ghost>>,
    samples: &[Vec3],
    carrying_structure: bool,
    except: Option<Entity>,
) -> f32 {
    let mut top = 0.0f32;
    for (entity, transform, record) in placed {
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
        // Tilted panels make poor tables; let them pass.
        if record.tilt.abs() > 0.01 {
            continue;
        }
        let unspin = Quat::from_rotation_y(-record.yaw);
        let repaint = record.ramp.as_deref().map(|r| (r, record.shade));
        for Slab(at, size, ..) in body_of(&kind, repaint) {
            for sample in samples {
                let local = unspin
                    * (Vec3::new(sample.x, 0.0, sample.z)
                        - Vec3::new(transform.translation.x, 0.0, transform.translation.z));
                if (local.x - at.x).abs() <= size.x * 0.5 && (local.z - at.z).abs() <= size.z * 0.5
                {
                    top = top.max(transform.translation.y + at.y + size.y * 0.5);
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
    placed: &Query<(Entity, &Transform, &Placed), Without<Ghost>>,
) -> Vec<PlatformRect> {
    let mut rects = Vec::new();
    for (_, transform, record) in placed {
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
fn wall_ends(placed: &Query<(Entity, &Transform, &Placed), Without<Ghost>>) -> Vec<(Vec3, Vec3)> {
    let mut ends = Vec::new();
    for (_, transform, record) in placed {
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
    placed: Query<(Entity, &Transform, &Placed), Without<Ghost>>,
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
    let tuning = selected.0.is_some();
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
            (
                kind_now.run_made(w, d),
                anchor + Vec3::new(w * 0.5 * reach.x.signum(), 0.0, d * 0.5 * reach.z.signum()),
                0.0,
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
                &made,
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
    placed: &Query<(Entity, &Transform, &Placed), Without<Ghost>>,
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
    for (entity, transform, record) in placed {
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
    placed: Query<(Entity, &Transform, &Placed), Without<Ghost>>,
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
    placed: Query<(Entity, &Transform, &Placed), Without<Ghost>>,
    hovers: Query<&Interaction>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (gizmo_hot, selected, tool) = gizmo;
    if *bench != Bench::Builder
        || naming.0.is_some()
        || gizmo_hot.0
        || selected.0.is_some()
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
            let opening = match kind {
                PartKind::Prop("door") => Some((1.25, 2.125, 0.0_f32, true)),
                // A bare doorway needs no widget: the gap itself is the
                // portal, and a widget would only say it twice.
                PartKind::Prop("doorway") => Some((1.25, 2.125, 0.0, false)),
                PartKind::Prop("window") => Some((1.25, 2.0, 0.75, false)),
                _ => None,
            };
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
            }
        } else if let Some(grabbed) = hovered.grab
            && let Ok((_, transform, record)) = placed.get(grabbed)
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

    if keys.just_pressed(KeyCode::KeyX)
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
    placed: &Query<(Entity, &Transform, &Placed), Without<Ghost>>,
    frame: Entity,
) -> bool {
    let Ok((_, frame_at, frame_record)) = placed.get(frame) else {
        return false;
    };
    let width = match kind_from_name(&frame_record.part) {
        Some(PartKind::Prop("door" | "doorway" | "window")) => 1.25,
        _ => return false,
    };
    let along = Quat::from_rotation_y(frame_record.yaw) * Vec3::X;
    let base = frame_at.translation;

    // Everything standing on this wall's own line, measured along it.
    let mut doomed: Vec<Entity> = Vec::new();
    let mut low = -width * 0.5;
    let mut high = width * 0.5;
    let mut cloth: Option<Placed> = None;
    for (entity, transform, record) in placed {
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
    placed: &Query<(Entity, &Transform, &Placed), Without<Ghost>>,
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
        && let Ok((entity, transform, record)) = placed.get(touched)
        && let Some(length) = punchable_length(record)
    {
        let along = Quat::from_rotation_y(record.yaw) * Vec3::X;
        let t = (point - transform.translation).dot(along);
        if t.abs() <= length * 0.5 {
            best = Some((entity, 0.0, along, t, length, record.clone()));
        }
    }
    if best.is_none() {
        for (entity, transform, record) in placed {
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

    // The opening, clamped so it never spills past the wall's ends.
    let half = length * 0.5;
    let middle = t.clamp(-half + wide * 0.5, half - wide * 0.5);
    let centre_of = |offset: f32| {
        let base = placed
            .get(wall)
            .map(|(_, tf, _)| tf.translation)
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
        .map(|(_, tf, _)| tf.translation)
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
        let mark = Placed {
            part: part_name(&widget),
            at: [frame_at.x, base.y, frame_at.z],
            yaw: facing,
            tilt: 0.0,
            ramp: None,
            shade: 0.7,
            stage: "widget".to_string(),
            flip: false,
        };
        spawn_part(commands, meshes, materials, palette, &widget, &mark, false);
    }
    true
}

// ---------------------------------------------------------------- the file

#[derive(Serialize, Deserialize, Default)]
struct Workbench {
    format: u32,
    name: String,
    parts: Vec<Placed>,
}

fn bench_path() -> std::path::PathBuf {
    let base = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(base).join("out/buildings/workbench.json")
}

/// The save button asks the work its name; the writing happens when the
/// name is given, in [`take_the_name`].
fn save_workbench(
    mut commands: Commands,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
    work_name: Res<WorkName>,
    mut naming: ResMut<Naming>,
    saves: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
) {
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
    mut keystrokes: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut naming: ResMut<Naming>,
    time: Res<Time>,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
    saved_drawer: Option<Res<SavedWorkDrawer>>,
    mut work_name: ResMut<WorkName>,
    placed: Query<&Placed, Without<Ghost>>,
    cards: Query<Entity, With<NamingCard>>,
    rows: Query<&LoadFileButton>,
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
            let mut path = dir.join(format!("{stem}.json"));
            if !ours {
                let mut n = 2;
                while path.exists() {
                    stem = format!("{written}-{n}");
                    path = dir.join(format!("{stem}.json"));
                    n += 1;
                }
            }
            let bench = Workbench {
                format: 1,
                name: stem.clone(),
                parts: placed.iter().cloned().collect(),
            };
            if let Ok(json) = serde_json::to_string_pretty(&bench) {
                let count = bench.parts.len();
                let _ = std::fs::write(&path, json);
                info!("saved {count} parts to {}", path.display());
                work_name.0 = Some(stem.clone());
                for (entity, mut text) in &mut save_labels {
                    *text = Text::new(format!("SAVED {} - {count} PARTS", stem.to_uppercase()));
                    commands.entity(entity).insert(PassingWord {
                        back: "SAVE THE WORK",
                        until: time.elapsed_secs() + 2.5,
                    });
                }
                // The row appears at once - unless it already stands.
                let standing = rows.iter().any(|row| row.0 == path);
                if !standing && let Some(saved_drawer) = saved_drawer.as_deref() {
                    saved_work_row(
                        &mut commands,
                        &fonts,
                        &palette,
                        saved_drawer.0,
                        path,
                        Box::leak(stem.to_uppercase().into_boxed_str()),
                    );
                }
            }
        }
    }
    naming.0 = None;
    for card in &cards {
        commands.entity(card).despawn();
    }
}

/// The x beside a saved work: the first press asks, the second buries.
/// An unanswered question calms back down on its own.
fn bury_saved_work(
    mut commands: Commands,
    time: Res<Time>,
    palette: Res<Palette>,
    mut buttons: Query<(
        &mut DeleteFileButton,
        &Interaction,
        &mut BorderColor,
        &Children,
    )>,
    mut labels: Query<&mut Text>,
) {
    let now = time.elapsed_secs();
    for (mut bury, interaction, mut border, children) in &mut buttons {
        let armed = now < bury.armed_until;
        let pressed = *interaction == Interaction::Pressed;
        if pressed && armed {
            let _ = std::fs::remove_file(&bury.path);
            info!("buried {}", bury.path.display());
            commands.entity(bury.row).despawn();
            continue;
        }
        if pressed && !armed {
            bury.armed_until = now + 2.5;
        }
        let asking = now < bury.armed_until;
        let (word, dress) = if asking {
            ("sure?", theme::accent(&palette))
        } else {
            ("x", theme::panel_border(&palette).with_alpha(0.25))
        };
        let fresh = BorderColor::all(dress);
        if *border != fresh {
            *border = fresh;
        }
        for &child in children {
            if let Ok(mut text) = labels.get_mut(child)
                && text.0 != word
            {
                *text = Text::new(word);
            }
        }
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
                    bottom: Val::Px(10.0),
                    margin: UiRect::left(Val::Px(-110.0)),
                    width: Val::Px(220.0),
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
    let sized = selected.0.and_then(|part| {
        let (_, record) = parts.get(part).ok()?;
        let kind = kind_from_name(&record.part)?;
        match kind {
            PartKind::Wall(long) => Some((part, long, None)),
            PartKind::Trim { long, .. } => Some((part, long, None)),
            PartKind::Floor(w, d) | PartKind::Foundation(w, d) | PartKind::Roof(w, d) => {
                Some((part, w, Some(d)))
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
                && let Some((part, _, had_d)) = sized
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
                        PartKind::Trim { stone, .. } => Some(PartKind::Trim { long: w, stone }),
                        PartKind::Floor(_, old) => Some(PartKind::Floor(w, d.unwrap_or(old))),
                        PartKind::Foundation(_, old) => {
                            Some(PartKind::Foundation(w, d.unwrap_or(old)))
                        }
                        PartKind::Roof(_, old) => Some(PartKind::Roof(w, d.unwrap_or(old))),
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
        && let Some((_, w, d)) = sized
    {
        let units = |value: f32| format!("{}", (value * 16.0).round() as i64);
        said = Some(match d {
            Some(d) => format!("{} x {} - D to type", units(w), units(d)),
            None => format!("{} - D to type", units(w)),
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
        && let Some(source) = selected.0.or(hovered.grab)
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
    if let Some(part) = selected.0
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
