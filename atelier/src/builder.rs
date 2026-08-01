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

/// The game's wall cross-section, from buildings.rs.
const WALL_THICK: f32 = 0.24;
const WALL_HIGH: f32 = 2.4;

/// One box of a part's body: offset from the part origin, size, ramp, shade.
struct Slab(Vec3, Vec3, String, f32);

/// What a shelf entry stands for.
#[derive(Clone, Copy, PartialEq)]
pub enum PartKind {
    Wall(f32),
    Floor,
    Roof,
    Prop(&'static str),
    Widget(&'static str),
}

/// A shelf entry: the name it wears, what it places, and the stage the
/// village raises it in.
pub struct CatalogEntry {
    pub label: &'static str,
    pub kind: PartKind,
    pub stage: &'static str,
}

pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        label: "WALL, 1M",
        kind: PartKind::Wall(1.0),
        stage: "walls",
    },
    CatalogEntry {
        label: "WALL, 2M",
        kind: PartKind::Wall(2.0),
        stage: "walls",
    },
    CatalogEntry {
        label: "WALL, 4M",
        kind: PartKind::Wall(4.0),
        stage: "walls",
    },
    CatalogEntry {
        label: "FLOOR, 2M",
        kind: PartKind::Floor,
        stage: "footing",
    },
    CatalogEntry {
        label: "ROOF PANEL",
        kind: PartKind::Roof,
        stage: "roof",
    },
    CatalogEntry {
        label: "BED",
        kind: PartKind::Prop("bed"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "TABLE",
        kind: PartKind::Prop("table"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "STOOL",
        kind: PartKind::Prop("stool"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "HEARTH",
        kind: PartKind::Prop("hearth"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "CHAIR",
        kind: PartKind::Prop("chair"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "BENCH",
        kind: PartKind::Prop("bench"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "CHEST",
        kind: PartKind::Prop("chest"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "BARREL",
        kind: PartKind::Prop("barrel"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "CRATE",
        kind: PartKind::Prop("crate"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "SHELVES",
        kind: PartKind::Prop("shelves"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "CUPBOARD",
        kind: PartKind::Prop("cupboard"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "COOKING POT",
        kind: PartKind::Prop("pot"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "BASKET",
        kind: PartKind::Prop("basket"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "RUG",
        kind: PartKind::Prop("rug"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "WOODPILE",
        kind: PartKind::Prop("woodpile"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "CANDLE STAND",
        kind: PartKind::Prop("candle"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "SACK",
        kind: PartKind::Prop("sack"),
        stage: "furnishing",
    },
    CatalogEntry {
        label: "TROUGH",
        kind: PartKind::Prop("trough"),
        stage: "furnishing",
    },
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
fn body_of(kind: &PartKind, palette_shift: Option<(&str, f32)>) -> Vec<Slab> {
    let mut slabs = match kind {
        PartKind::Wall(length) => vec![Slab(
            Vec3::new(0.0, WALL_HIGH * 0.5, 0.0),
            Vec3::new(*length, WALL_HIGH, WALL_THICK),
            "wood".to_string(),
            0.7,
        )],
        PartKind::Floor => vec![Slab(
            Vec3::new(0.0, 0.06, 0.0),
            Vec3::new(2.0, 0.12, 2.0),
            "wood".to_string(),
            0.5,
        )],
        PartKind::Roof => vec![Slab(
            Vec3::new(0.0, 0.07, 0.0),
            Vec3::new(2.2, 0.14, 2.2),
            "earth".to_string(),
            0.4,
        )],
        PartKind::Prop("bed") => vec![
            // The game's own bed: frame, mattress, pillow at +Z (the head).
            Slab(
                Vec3::new(0.0, 0.26, 0.0),
                Vec3::new(0.76, 0.24, 1.64),
                "wood".to_string(),
                0.55,
            ),
            Slab(
                Vec3::new(0.0, 0.44, 0.0),
                Vec3::new(0.62, 0.18, 1.5),
                "bone".to_string(),
                0.8,
            ),
            Slab(
                Vec3::new(0.0, 0.56, 0.55),
                Vec3::new(0.46, 0.1, 0.32),
                "bone".to_string(),
                0.95,
            ),
        ],
        PartKind::Prop("table") => {
            let mut parts = vec![Slab(
                Vec3::new(0.0, 0.72, 0.0),
                Vec3::new(1.5, 0.1, 0.9),
                "wood".to_string(),
                0.65,
            )];
            for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                parts.push(Slab(
                    Vec3::new(sx * 0.62, 0.34, sz * 0.32),
                    Vec3::new(0.1, 0.68, 0.1),
                    "wood".to_string(),
                    0.5,
                ));
            }
            parts
        }
        PartKind::Prop("stool") => vec![
            Slab(
                Vec3::new(0.0, 0.4, 0.0),
                Vec3::new(0.38, 0.07, 0.38),
                "wood".to_string(),
                0.6,
            ),
            Slab(
                Vec3::new(0.0, 0.18, 0.0),
                Vec3::new(0.3, 0.36, 0.3),
                "wood".to_string(),
                0.45,
            ),
        ],
        PartKind::Prop("hearth") => vec![
            Slab(
                Vec3::new(0.0, 0.42, 0.0),
                Vec3::new(0.9, 0.84, 0.6),
                "stone".to_string(),
                0.6,
            ),
            Slab(
                Vec3::new(0.0, 0.55, 0.12),
                Vec3::new(0.62, 0.5, 0.44),
                "stone".to_string(),
                0.25,
            ),
        ],
        PartKind::Prop("chair") => vec![
            Slab(
                Vec3::new(0.0, 0.4, 0.0),
                Vec3::new(0.4, 0.07, 0.4),
                "wood".to_string(),
                0.6,
            ),
            Slab(
                Vec3::new(0.0, 0.18, 0.0),
                Vec3::new(0.32, 0.36, 0.32),
                "wood".to_string(),
                0.45,
            ),
            Slab(
                Vec3::new(0.0, 0.72, -0.17),
                Vec3::new(0.4, 0.72, 0.07),
                "wood".to_string(),
                0.55,
            ),
        ],
        PartKind::Prop("bench") => vec![
            Slab(
                Vec3::new(0.0, 0.4, 0.0),
                Vec3::new(1.2, 0.08, 0.36),
                "wood".to_string(),
                0.6,
            ),
            Slab(
                Vec3::new(-0.5, 0.18, 0.0),
                Vec3::new(0.09, 0.36, 0.3),
                "wood".to_string(),
                0.45,
            ),
            Slab(
                Vec3::new(0.5, 0.18, 0.0),
                Vec3::new(0.09, 0.36, 0.3),
                "wood".to_string(),
                0.45,
            ),
        ],
        PartKind::Prop("chest") => vec![
            Slab(
                Vec3::new(0.0, 0.25, 0.0),
                Vec3::new(0.8, 0.5, 0.5),
                "wood".to_string(),
                0.5,
            ),
            Slab(
                Vec3::new(0.0, 0.52, 0.0),
                Vec3::new(0.84, 0.1, 0.54),
                "wood".to_string(),
                0.35,
            ),
            Slab(
                Vec3::new(0.0, 0.33, 0.26),
                Vec3::new(0.1, 0.16, 0.04),
                "cloth-gold".to_string(),
                0.7,
            ),
        ],
        PartKind::Prop("barrel") => vec![
            Slab(
                Vec3::new(0.0, 0.36, 0.0),
                Vec3::new(0.55, 0.72, 0.55),
                "wood".to_string(),
                0.55,
            ),
            Slab(
                Vec3::new(0.0, 0.16, 0.0),
                Vec3::new(0.59, 0.07, 0.59),
                "stone".to_string(),
                0.45,
            ),
            Slab(
                Vec3::new(0.0, 0.56, 0.0),
                Vec3::new(0.59, 0.07, 0.59),
                "stone".to_string(),
                0.45,
            ),
        ],
        PartKind::Prop("crate") => vec![
            Slab(
                Vec3::new(0.0, 0.3, 0.0),
                Vec3::new(0.6, 0.6, 0.6),
                "wood".to_string(),
                0.6,
            ),
            Slab(
                Vec3::new(0.0, 0.61, 0.0),
                Vec3::new(0.52, 0.03, 0.52),
                "wood".to_string(),
                0.4,
            ),
        ],
        PartKind::Prop("shelves") => vec![
            Slab(
                Vec3::new(-0.42, 0.8, 0.0),
                Vec3::new(0.06, 1.6, 0.3),
                "wood".to_string(),
                0.5,
            ),
            Slab(
                Vec3::new(0.42, 0.8, 0.0),
                Vec3::new(0.06, 1.6, 0.3),
                "wood".to_string(),
                0.5,
            ),
            Slab(
                Vec3::new(0.0, 0.5, 0.0),
                Vec3::new(0.9, 0.05, 0.3),
                "wood".to_string(),
                0.65,
            ),
            Slab(
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.9, 0.05, 0.3),
                "wood".to_string(),
                0.65,
            ),
            Slab(
                Vec3::new(0.0, 1.5, 0.0),
                Vec3::new(0.9, 0.05, 0.3),
                "wood".to_string(),
                0.65,
            ),
        ],
        PartKind::Prop("cupboard") => vec![
            Slab(
                Vec3::new(0.0, 0.75, 0.0),
                Vec3::new(0.9, 1.5, 0.45),
                "wood".to_string(),
                0.5,
            ),
            Slab(
                Vec3::new(0.0, 0.75, 0.24),
                Vec3::new(0.82, 1.34, 0.04),
                "wood".to_string(),
                0.65,
            ),
            Slab(
                Vec3::new(0.12, 0.75, 0.27),
                Vec3::new(0.05, 0.16, 0.03),
                "cloth-gold".to_string(),
                0.6,
            ),
        ],
        PartKind::Prop("pot") => vec![
            Slab(
                Vec3::new(0.0, 0.2, 0.0),
                Vec3::new(0.4, 0.4, 0.4),
                "stone".to_string(),
                0.3,
            ),
            Slab(
                Vec3::new(0.0, 0.42, 0.0),
                Vec3::new(0.46, 0.06, 0.46),
                "stone".to_string(),
                0.45,
            ),
        ],
        PartKind::Prop("basket") => vec![
            Slab(
                Vec3::new(0.0, 0.15, 0.0),
                Vec3::new(0.45, 0.3, 0.45),
                "sand".to_string(),
                0.55,
            ),
            Slab(
                Vec3::new(0.0, 0.31, 0.0),
                Vec3::new(0.5, 0.05, 0.5),
                "sand".to_string(),
                0.4,
            ),
        ],
        PartKind::Prop("rug") => vec![
            Slab(
                Vec3::new(0.0, 0.015, 0.0),
                Vec3::new(1.4, 0.03, 0.9),
                "cloth-red".to_string(),
                0.55,
            ),
            Slab(
                Vec3::new(0.0, 0.032, 0.0),
                Vec3::new(1.1, 0.01, 0.62),
                "cloth-red".to_string(),
                0.75,
            ),
        ],
        PartKind::Prop("woodpile") => vec![
            Slab(
                Vec3::new(0.0, 0.11, 0.0),
                Vec3::new(1.0, 0.22, 0.66),
                "wood".to_string(),
                0.4,
            ),
            Slab(
                Vec3::new(0.0, 0.32, 0.0),
                Vec3::new(1.0, 0.2, 0.5),
                "wood".to_string(),
                0.5,
            ),
            Slab(
                Vec3::new(0.0, 0.5, 0.0),
                Vec3::new(1.0, 0.18, 0.32),
                "wood".to_string(),
                0.6,
            ),
        ],
        PartKind::Prop("candle") => vec![
            Slab(
                Vec3::new(0.0, 0.02, 0.0),
                Vec3::new(0.3, 0.05, 0.3),
                "stone".to_string(),
                0.5,
            ),
            Slab(
                Vec3::new(0.0, 0.6, 0.0),
                Vec3::new(0.07, 1.1, 0.07),
                "wood".to_string(),
                0.4,
            ),
            Slab(
                Vec3::new(0.0, 1.18, 0.0),
                Vec3::new(0.12, 0.14, 0.12),
                "bone".to_string(),
                0.95,
            ),
            Slab(
                Vec3::new(0.0, 1.3, 0.0),
                Vec3::new(0.07, 0.1, 0.07),
                "cloth-gold".to_string(),
                0.95,
            ),
        ],
        PartKind::Prop("sack") => vec![
            Slab(
                Vec3::new(0.0, 0.21, 0.0),
                Vec3::new(0.42, 0.42, 0.42),
                "bone".to_string(),
                0.6,
            ),
            Slab(
                Vec3::new(0.0, 0.46, 0.0),
                Vec3::new(0.18, 0.12, 0.18),
                "bone".to_string(),
                0.45,
            ),
        ],
        PartKind::Prop("trough") => vec![
            Slab(
                Vec3::new(0.0, 0.15, 0.0),
                Vec3::new(1.2, 0.3, 0.45),
                "wood".to_string(),
                0.45,
            ),
            Slab(
                Vec3::new(0.0, 0.27, 0.0),
                Vec3::new(1.08, 0.04, 0.33),
                "water".to_string(),
                0.7,
            ),
        ],
        PartKind::Prop(_) => vec![],
        PartKind::Widget(name) => {
            let (_, ramp, shade) = WIDGETS
                .iter()
                .find(|(w, _, _)| w == name)
                .copied()
                .unwrap_or(("", "bone", 0.5));
            vec![
                Slab(
                    Vec3::new(0.0, 0.2, 0.0),
                    Vec3::splat(0.4),
                    ramp.to_string(),
                    shade,
                ),
                // The nose: which way the widget faces.
                Slab(
                    Vec3::new(0.3, 0.2, 0.0),
                    Vec3::new(0.2, 0.12, 0.12),
                    ramp.to_string(),
                    shade,
                ),
            ]
        }
    };
    // A repainted part carries its choice into every wooden slab.
    if let Some((ramp, shade)) = palette_shift {
        for slab in &mut slabs {
            if slab.2 == "wood" || slab.2 == "earth" || slab.2.starts_with("cloth") {
                slab.2 = ramp.to_string();
                slab.3 = shade;
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
}

/// What the hand holds, as an index into [`CATALOG`] + widgets beyond it.
#[derive(Resource, Default)]
pub struct Armed(pub Option<usize>);

/// The ghost that follows the cursor while something is armed.
#[derive(Component)]
pub struct Ghost;

/// The armed part's working state: turn, tilt, lift and paint.
#[derive(Resource)]
pub struct Hand {
    pub yaw: f32,
    pub tilt: f32,
    pub lift: f32,
    pub ramp: Option<String>,
    pub shade: f32,
}

impl Default for Hand {
    fn default() -> Self {
        Hand {
            yaw: 0.0,
            tilt: 0.0,
            lift: 0.0,
            ramp: None,
            shade: 0.7,
        }
    }
}

/// A shelf button arming catalog entry N.
#[derive(Component)]
struct ShelfButton(usize);

/// The shelf panel itself, shown only at the Builder bench.
#[derive(Component)]
struct Shelf;

/// The export/save button.
#[derive(Component)]
struct SaveButton;

pub struct BuilderPlugin;

impl Plugin for BuilderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Armed>()
            .init_resource::<Hand>()
            .add_systems(Startup, (raise_shelf, load_workbench))
            .add_systems(
                Update,
                (
                    show_shelf,
                    work_shelf,
                    steer_hand,
                    move_ghost,
                    place_or_remove,
                    save_workbench,
                )
                    .chain(),
            );
    }
}

/// The full entry list: catalog parts, then widgets.
pub fn entry_kind(index: usize) -> Option<(PartKind, &'static str)> {
    if index < CATALOG.len() {
        let entry = &CATALOG[index];
        Some((entry.kind, entry.stage))
    } else {
        WIDGETS
            .get(index - CATALOG.len())
            .map(|(name, _, _)| (PartKind::Widget(name), "widget"))
    }
}

fn part_name(kind: &PartKind) -> String {
    match kind {
        PartKind::Wall(len) => format!("wall-{len}"),
        PartKind::Floor => "floor".to_string(),
        PartKind::Roof => "roof".to_string(),
        PartKind::Prop(name) => format!("prop:{name}"),
        PartKind::Widget(name) => format!("widget:{name}"),
    }
}

fn kind_from_name(name: &str) -> Option<PartKind> {
    if let Some(rest) = name.strip_prefix("wall-") {
        return rest.parse::<f32>().ok().map(PartKind::Wall);
    }
    if let Some(prop) = name.strip_prefix("prop:") {
        return CATALOG.iter().find_map(|e| match e.kind {
            PartKind::Prop(p) if p == prop => Some(e.kind),
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
        "floor" => Some(PartKind::Floor),
        "roof" => Some(PartKind::Roof),
        _ => None,
    }
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
    let translucent = ghostly || matches!(kind, PartKind::Widget(_));
    let root = commands
        .spawn((
            record.clone(),
            Transform::from_translation(Vec3::from(record.at)).with_rotation(
                Quat::from_rotation_y(record.yaw) * Quat::from_rotation_x(record.tilt),
            ),
            Visibility::default(),
            BuilderFurniture,
        ))
        .id();
    if ghostly {
        commands.entity(root).insert(Ghost);
    }
    let repaint = record.ramp.as_deref().map(|r| (r, record.shade));
    for Slab(at, size, ramp, shade) in body_of(kind, repaint) {
        let mut color = palette.shade(&ramp, shade);
        if translucent {
            color = color.with_alpha(if ghostly { 0.45 } else { 0.55 });
        }
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.95,
                reflectance: 0.03,
                alpha_mode: if translucent {
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
    root
}

// ---------------------------------------------------------------- the shelf

fn raise_shelf(mut commands: Commands, fonts: Res<Fonts>, palette: Res<Palette>) {
    let shelf = commands
        .spawn((
            Shelf,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(170.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::left(Val::Px(1.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
        ))
        .id();

    let header = |commands: &mut Commands, label: &str, top: f32| {
        commands.spawn((
            Text::new(label),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(theme::accent(&palette)),
            Node {
                margin: UiRect::top(Val::Px(top)),
                ..default()
            },
            ChildOf(shelf),
        ));
    };

    header(&mut commands, "THE SHELF", 0.0);
    for (index, entry) in CATALOG.iter().enumerate() {
        shelf_button(&mut commands, &fonts, &palette, shelf, index, entry.label);
    }
    header(&mut commands, "WIDGETS", 10.0);
    for (offset, (name, _, _)) in WIDGETS.iter().enumerate() {
        shelf_button(
            &mut commands,
            &fonts,
            &palette,
            shelf,
            CATALOG.len() + offset,
            Box::leak(name.to_uppercase().into_boxed_str()),
        );
    }

    // The save at the shelf's foot.
    let save = commands
        .spawn((
            SaveButton,
            Interaction::default(),
            Node {
                margin: UiRect::top(Val::Px(14.0)),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::accent(&palette).with_alpha(0.7)),
            ChildOf(shelf),
        ))
        .id();
    commands.spawn((
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
            "click places, X removes what the\ncursor touches. R turns, T tilts\n\
             a roof, Q/E lift and lower.\n[ and ] repaint, - and = shade.\n\
             esc empties the hand.",
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

fn shelf_button(
    commands: &mut Commands,
    fonts: &Fonts,
    palette: &Palette,
    shelf: Entity,
    index: usize,
    label: &'static str,
) {
    let button = commands
        .spawn((
            ShelfButton(index),
            Interaction::default(),
            Node {
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::panel_border(palette)),
            ChildOf(shelf),
        ))
        .id();
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

/// Shelf presses arm the hand; the armed entry wears the gold.
#[allow(clippy::too_many_arguments)]
fn work_shelf(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
    mut armed: ResMut<Armed>,
    mut hand: ResMut<Hand>,
    mut buttons: Query<(&Interaction, &ShelfButton, &mut BorderColor)>,
    ghosts: Query<Entity, With<Ghost>>,
) {
    let mut rearmed = false;
    for (interaction, button, _) in &buttons {
        if *interaction == Interaction::Pressed && armed.0 != Some(button.0) {
            armed.0 = Some(button.0);
            *hand = Hand::default();
            rearmed = true;
        }
    }
    if rearmed {
        for ghost in &ghosts {
            commands.entity(ghost).despawn();
        }
        if let Some(index) = armed.0
            && let Some((kind, stage)) = entry_kind(index)
        {
            let record = Placed {
                part: part_name(&kind),
                at: [0.0, 0.0, 0.0],
                yaw: 0.0,
                tilt: 0.0,
                ramp: None,
                shade: 0.7,
                stage: stage.to_string(),
            };
            spawn_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                &palette,
                &kind,
                &record,
                true,
            );
        }
    }
    for (_, button, mut border) in &mut buttons {
        let standing = armed.0 == Some(button.0);
        let dress = BorderColor::all(if standing {
            theme::accent(&palette)
        } else {
            theme::panel_border(&palette)
        });
        if *border != dress {
            *border = dress;
        }
    }
}

// ---------------------------------------------------------------- the hand

/// Keys that steer what the hand holds. Esc empties it.
#[allow(clippy::too_many_arguments)]
fn steer_hand(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    palette: Res<Palette>,
    mut armed: ResMut<Armed>,
    mut hand: ResMut<Hand>,
    ghosts: Query<Entity, With<Ghost>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if armed.0.is_none() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        armed.0 = None;
        for ghost in &ghosts {
            commands.entity(ghost).despawn();
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
    if keys.just_pressed(KeyCode::BracketRight) {
        let here = hand
            .ramp
            .as_deref()
            .and_then(|r| ramps.iter().position(|n| *n == r))
            .unwrap_or(0);
        hand.ramp = Some(ramps[(here + 1) % ramps.len()].to_string());
        redress = true;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
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
    // A repaint rebuilds the ghost in its new cloth.
    if redress
        && let Some(index) = armed.0
        && let Some((kind, stage)) = entry_kind(index)
    {
        for ghost in &ghosts {
            commands.entity(ghost).despawn();
        }
        let record = Placed {
            part: part_name(&kind),
            at: [0.0, 0.0, 0.0],
            yaw: hand.yaw,
            tilt: hand.tilt,
            ramp: hand.ramp.clone(),
            shade: hand.shade,
            stage: stage.to_string(),
        };
        spawn_part(
            &mut commands,
            &mut meshes,
            &mut materials,
            &palette,
            &kind,
            &record,
            true,
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

fn move_ghost(
    bench: Res<Bench>,
    hand: Res<Hand>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut ghosts: Query<&mut Transform, With<Ghost>>,
) {
    if *bench != Bench::Builder {
        return;
    }
    let Some(point) = cursor_point(&windows, &cameras, hand.lift) else {
        return;
    };
    // Quarter-metre snap: coarse enough to line walls up by eye, fine
    // enough to nudge a stool against a table.
    let snapped = Vec3::new(
        (point.x * 4.0).round() / 4.0,
        hand.lift,
        (point.z * 4.0).round() / 4.0,
    );
    for mut transform in &mut ghosts {
        transform.translation = snapped;
        transform.rotation = Quat::from_rotation_y(hand.yaw) * Quat::from_rotation_x(hand.tilt);
    }
}

/// Left click sets the part down; X removes the placed part nearest the
/// cursor's touch.
#[allow(clippy::too_many_arguments)]
fn place_or_remove(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    armed: Res<Armed>,
    hand: Res<Hand>,
    palette: Res<Palette>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    ghosts: Query<&Transform, With<Ghost>>,
    placed: Query<(Entity, &Transform), (With<Placed>, Without<Ghost>)>,
    hovers: Query<&Interaction>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if *bench != Bench::Builder {
        return;
    }
    // A click that lands on UI is the UI's business.
    let over_ui = hovers
        .iter()
        .any(|interaction| *interaction != Interaction::None);

    if buttons.just_pressed(MouseButton::Left)
        && !over_ui
        && let Some(index) = armed.0
        && let Some((kind, stage)) = entry_kind(index)
        && let Some(ghost_at) = ghosts.iter().next()
    {
        let record = Placed {
            part: part_name(&kind),
            at: ghost_at.translation.into(),
            yaw: hand.yaw,
            tilt: hand.tilt,
            ramp: hand.ramp.clone(),
            shade: hand.shade,
            stage: stage.to_string(),
        };
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

    if keys.just_pressed(KeyCode::KeyX)
        && let Some(point) = cursor_point(&windows, &cameras, 0.0)
    {
        // The nearest placed part within arm's reach of the touch.
        let mut nearest: Option<(Entity, f32)> = None;
        for (entity, transform) in &placed {
            let flat = Vec2::new(
                transform.translation.x - point.x,
                transform.translation.z - point.z,
            );
            let distance = flat.length();
            if distance < 1.4 && nearest.is_none_or(|(_, d)| distance < d) {
                nearest = Some((entity, distance));
            }
        }
        if let Some((entity, _)) = nearest {
            commands.entity(entity).despawn();
        }
    }
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

/// The save button writes the whole bench down; the work survives the
/// window closing, and the file is the thing the god carries into the game.
fn save_workbench(
    saves: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
    placed: Query<&Placed, Without<Ghost>>,
) {
    let pressed = saves
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed);
    if !pressed {
        return;
    }
    let bench = Workbench {
        format: 1,
        name: "workbench".to_string(),
        parts: placed.iter().cloned().collect(),
    };
    let path = bench_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(&bench) {
        Ok(json) => {
            let _ = std::fs::write(&path, json);
            info!("saved {} parts to {}", bench.parts.len(), path.display());
        }
        Err(e) => warn!("could not write the bench: {e}"),
    }
}

/// Whatever was on the bench when it was last saved comes back on launch.
fn load_workbench(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
) {
    let Ok(text) = std::fs::read_to_string(bench_path()) else {
        return;
    };
    let Ok(bench) = serde_json::from_str::<Workbench>(&text) else {
        warn!("the saved bench would not parse; starting clean");
        return;
    };
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
    info!("the bench remembers {count} parts");
}
