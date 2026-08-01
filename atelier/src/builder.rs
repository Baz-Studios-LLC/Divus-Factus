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
    structure("WALL, 1M", PartKind::Wall(1.0), "walls"),
    structure("WALL, 2M", PartKind::Wall(2.0), "walls"),
    structure("WALL, 4M", PartKind::Wall(4.0), "walls"),
    structure("FLOOR, 2M", PartKind::Floor, "footing"),
    structure("ROOF PANEL", PartKind::Roof, "roof"),
];

pub const FURNITURE: &[CatalogEntry] = &[
    prop("BED", "bed"),
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
        )
    };
    let mut slabs = match kind {
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
        PartKind::Floor => vec![slab(0.0, 0.06, 0.0, 2.0, 0.12, 2.0, "wood", 0.5)],
        PartKind::Roof => vec![slab(0.0, 0.07, 0.0, 2.2, 0.14, 2.2, "earth", 0.4)],
        PartKind::Prop("bed") => vec![
            // The game's own bed: frame, mattress, pillow at +Z (the head).
            slab(0.0, 0.26, 0.0, 0.76, 0.24, 1.64, "wood", 0.55),
            slab(0.0, 0.44, 0.0, 0.62, 0.18, 1.5, "bone", 0.8),
            slab(0.0, 0.56, 0.55, 0.46, 0.1, 0.32, "bone", 0.95),
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
}

/// The ghost that follows the cursor while the hand is full.
#[derive(Component)]
pub struct Ghost;

/// The maker's hand: what it holds and how it holds it. Filled from the
/// shelf, or by picking a placed part back up with an empty hand.
#[derive(Resource, Default)]
pub struct Hand {
    pub kind: Option<PartKind>,
    pub stage: String,
    pub yaw: f32,
    pub tilt: f32,
    pub lift: f32,
    pub ramp: Option<String>,
    pub shade: f32,
}

impl Hand {
    fn filled(kind: PartKind, stage: String) -> Self {
        Hand {
            kind: Some(kind),
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

/// The button that writes a numbered copy that nothing ever overwrites.
#[derive(Component)]
struct ExportButton;

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

pub struct BuilderPlugin;

impl Plugin for BuilderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hand>()
            .add_systems(Startup, (raise_shelf, load_workbench))
            .add_systems(
                Update,
                (
                    show_shelf,
                    work_drawers,
                    work_shelf,
                    work_templates,
                    steer_hand,
                    move_ghost,
                    place_grab_remove,
                    save_workbench,
                )
                    .chain(),
            );
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
    if let Some(wanted) = name.strip_prefix("prop:") {
        return FURNITURE.iter().chain(DECOR).find_map(|e| match e.kind {
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

fn raise_shelf(mut commands: Commands, fonts: Res<Fonts>, palette: Res<Palette>) {
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

    // READY-MADE: templates and the broom.
    let ready = drawer(&mut commands, &fonts, &palette, shelf, "READY-MADE", true);
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
    let saved = drawer(&mut commands, &fonts, &palette, shelf, "SAVED WORK", false);
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
            let button = plain_button(&mut commands, &palette, saved);
            commands.entity(button).insert(LoadFileButton(path));
            button_label(
                &mut commands,
                &fonts,
                &palette,
                button,
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
            ChildOf(shelf),
        ))
        .id();
    let export = commands
        .spawn((
            ExportButton,
            Interaction::default(),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::panel_border(&palette)),
            ChildOf(shelf),
        ))
        .id();
    commands.spawn((
        Text::new("EXPORT A COPY"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(theme::text_dim(&palette)),
        ChildOf(export),
    ));
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
            "an empty hand picks a placed\npart back up. click places,\n\
             X removes. R turns, T tilts,\nQ/E lift. [ ] repaint, - = shade.\n\
             esc empties the hand. build\ntoward the gold: that is the\ndoor side.",
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
) {
    let base = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
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
                .map(|(_, file)| file.0.clone())
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
        return;
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
    mut hand: ResMut<Hand>,
    ghosts: Query<Entity, With<Ghost>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if hand.kind.is_none() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        *hand = Hand::default();
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

/// The nearest placed part within arm's reach of a ground point.
fn nearest_part(
    placed: &Query<(Entity, &Transform, &Placed), Without<Ghost>>,
    point: Vec3,
) -> Option<Entity> {
    let mut nearest: Option<(Entity, f32)> = None;
    for (entity, transform, _) in placed {
        let flat = Vec2::new(
            transform.translation.x - point.x,
            transform.translation.z - point.z,
        );
        let distance = flat.length();
        if distance < 1.2 && nearest.is_none_or(|(_, d)| distance < d) {
            nearest = Some((entity, distance));
        }
    }
    nearest.map(|(entity, _)| entity)
}

/// A full hand places on click. An empty hand picks a placed part back up.
/// X removes what the cursor touches either way.
#[allow(clippy::too_many_arguments)]
fn place_grab_remove(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    bench: Res<Bench>,
    mut hand: ResMut<Hand>,
    palette: Res<Palette>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    ghosts: Query<Entity, With<Ghost>>,
    ghost_spots: Query<&Transform, With<Ghost>>,
    placed: Query<(Entity, &Transform, &Placed), Without<Ghost>>,
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

    if buttons.just_pressed(MouseButton::Left) && !over_ui {
        if let Some(kind) = hand.kind {
            // Setting down.
            if let Some(ghost_at) = ghost_spots.iter().next()
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
        } else if let Some(point) = cursor_point(&windows, &cameras, 0.0)
            && let Some(grabbed) = nearest_part(&placed, point)
            && let Ok((_, transform, record)) = placed.get(grabbed)
            && let Some(kind) = kind_from_name(&record.part)
        {
            // Picking back up: the part leaves the floor and rides the
            // cursor again with its paint, turn and height intact.
            *hand = Hand {
                kind: Some(kind),
                stage: record.stage.clone(),
                yaw: record.yaw,
                tilt: record.tilt,
                lift: transform.translation.y,
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
        && let Some(point) = cursor_point(&windows, &cameras, 0.0)
        && let Some(doomed) = nearest_part(&placed, point)
    {
        commands.entity(doomed).despawn();
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
/// The export button writes a numbered copy nothing ever overwrites.
fn save_workbench(
    saves: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
    exports: Query<&Interaction, (Changed<Interaction>, With<ExportButton>)>,
    placed: Query<&Placed, Without<Ghost>>,
) {
    let exporting = exports
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed);
    if exporting {
        let dir = bench_path().parent().map(|d| d.to_path_buf());
        if let Some(dir) = dir {
            let _ = std::fs::create_dir_all(&dir);
            let mut n = 1;
            let mut path = dir.join(format!("build-{n}.json"));
            while path.exists() {
                n += 1;
                path = dir.join(format!("build-{n}.json"));
            }
            let bench = Workbench {
                format: 1,
                name: format!("build-{n}"),
                parts: placed.iter().cloned().collect(),
            };
            if let Ok(json) = serde_json::to_string_pretty(&bench) {
                let _ = std::fs::write(&path, json);
                info!("exported {} parts to {}", bench.parts.len(), path.display());
            }
        }
    }
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
