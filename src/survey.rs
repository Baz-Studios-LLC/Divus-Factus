//! The god's survey: a translucent heat-wash over the ground showing where
//! the land's wealth lies, toggled with R. Woods glow green, quarry-rock
//! slate, clay banks ochre, iron rust, and wild food rose — the same fields
//! the scatterer and the founding survey read, so what the overlay promises
//! is what the villagers will actually find.
//!
//! The wash is one mesh of colored quads floated just above the ground,
//! rebuilt around the camera as it roams. No terrain is touched: toggling
//! it off simply removes the sheet.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::camera::CameraRig;
use crate::terrain::{Biome, Terrain, WATER_LEVEL};

pub struct SurveyPlugin;

impl Plugin for SurveyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Survey>()
            .add_systems(Startup, spawn_legend)
            .add_systems(
                Update,
                (
                    // Playing only: the title's veil is translucent now, and a
                    // stray R typed over the menu must not open the sight on
                    // the world drifting behind it.
                    toggle_survey.run_if(in_state(crate::GameState::Playing)),
                    refresh_survey,
                    show_legend,
                )
                    .chain(),
            );
    }
}

/// The legend panel that names the wash's colors while the sight is open.
#[derive(Component)]
struct SurveyLegend;

/// What each color of the wash means, in reading order.
const LEGEND: [([f32; 3], &str); 5] = [
    ([0.08, 0.68, 0.12], "woods - deeper is thicker"),
    ([0.50, 0.56, 0.70], "quarry rock"),
    ([0.82, 0.45, 0.12], "clay"),
    ([0.70, 0.15, 0.08], "iron"),
    ([0.88, 0.20, 0.45], "wild food"),
];

fn spawn_legend(mut commands: Commands) {
    let panel = commands
        .spawn((
            Name::new("Survey Legend"),
            SurveyLegend,
            Node {
                position_type: PositionType::Absolute,
                left: px(12),
                bottom: px(52),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                padding: UiRect::all(px(10)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(crate::ui::theme::panel_bg()),
            Visibility::Hidden,
            // Above the windows: while the sight is open, its legend reads
            // over whatever else is on screen, the way a mode should.
            GlobalZIndex(250),
        ))
        .id();
    commands.spawn((crate::ui::dim("THE GOD'S SIGHT"), ChildOf(panel)));
    for (color, name) in LEGEND {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(7),
                    ..default()
                },
                ChildOf(panel),
            ))
            .id();
        commands.spawn((
            Node {
                width: px(12),
                height: px(12),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BackgroundColor(Color::srgb(color[0], color[1], color[2])),
            ChildOf(row),
        ));
        commands.spawn((crate::ui::label(name), ChildOf(row)));
    }
}

/// The legend stands only while the sight is open.
fn show_legend(survey: Res<Survey>, mut legends: Query<&mut Visibility, With<SurveyLegend>>) {
    if !survey.is_changed() {
        return;
    }
    for mut visibility in &mut legends {
        *visibility = if survey.on {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Whether the god's sight is open, and where the sheet was last woven.
#[derive(Resource, Default)]
pub struct Survey {
    pub on: bool,
    built_at: Option<Vec2>,
    since: f32,
}

/// The overlay mesh entity.
#[derive(Component)]
struct SurveySheet;

/// World units per survey cell, and cells per side of the woven sheet.
const CELL: f32 = 5.0;
const SIDE: i32 = 80;

fn toggle_survey(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    mut survey: ResMut<Survey>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut booted: Local<bool>,
) {
    if !*booted {
        *booted = true;
        if std::env::var("DIVUS_FACTUS_SURVEY").is_ok() {
            survey.on = true;
        }
    }
    if !keymap.just_pressed(&keys, crate::keymap::Deed::Survey) {
        return;
    }
    survey.on = !survey.on;
    if survey.on {
        notices.write(crate::ui::Notice::new(
            "The god's sight opens: woods green, stone slate, clay ochre, iron rust, wild food rose"
                .to_string(),
        ));
    }
    // Force a reweave on the next refresh either way.
    survey.built_at = None;
}

/// Weaves, moves and unweaves the sheet as the sight and the camera demand.
#[allow(clippy::too_many_arguments)]
fn refresh_survey(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut survey: ResMut<Survey>,
    rigs: Query<&CameraRig>,
    deposits: Query<(&GlobalTransform, &crate::matter::Deposit)>,
    rocks: Query<&GlobalTransform, With<crate::matter::Boulder>>,
    bushes: Query<(&GlobalTransform, &crate::scatter::FoodSource)>,
    sheets: Query<Entity, With<SurveySheet>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !survey.on {
        if !sheets.is_empty() {
            for sheet in &sheets {
                commands.entity(sheet).despawn();
            }
            survey.built_at = None;
        }
        return;
    }
    let (Some(terrain), Ok(rig)) = (terrain, rigs.single()) else {
        return;
    };

    // Reweave when the eye has wandered off the woven ground, or on a slow
    // clock so spent deposits and stripped heaths fade from the sight.
    survey.since += time.delta_secs();
    let focus = Vec2::new(rig.focus.x, rig.focus.z);
    let stale = survey
        .built_at
        .is_none_or(|at| at.distance(focus) > CELL * 14.0)
        || survey.since > 5.0;
    if !stale {
        return;
    }
    survey.since = 0.0;
    survey.built_at = Some(focus);

    for sheet in &sheets {
        commands.entity(sheet).despawn();
    }

    // Point wealth first: what the ground holds in one place beats what it
    // grows everywhere.
    let veins: Vec<(Vec2, crate::matter::DepositKind)> = deposits
        .iter()
        .filter(|(_, deposit)| deposit.amount > 0.5)
        .map(|(at, deposit)| {
            (
                Vec2::new(at.translation().x, at.translation().z),
                deposit.kind,
            )
        })
        .collect();
    // Loose stone the picks can actually reach: boulders and outcrops
    // sit on flat ground too, and the sight must show them there or the
    // map claims the only stone in the world is mountainside.
    let stones: Vec<Vec2> = rocks
        .iter()
        .map(|at| Vec2::new(at.translation().x, at.translation().z))
        .collect();
    let heaths: Vec<Vec2> = bushes
        .iter()
        .filter(|(_, food)| food.amount > 0.3)
        .map(|(at, _)| Vec2::new(at.translation().x, at.translation().z))
        .collect();

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let origin_x = (focus.x / CELL).round() * CELL - SIDE as f32 * 0.5 * CELL;
    let origin_z = (focus.y / CELL).round() * CELL - SIDE as f32 * 0.5 * CELL;
    for i in 0..SIDE {
        for j in 0..SIDE {
            let x = origin_x + i as f32 * CELL + CELL * 0.5;
            let z = origin_z + j as f32 * CELL + CELL * 0.5;
            if terrain.is_submerged(x, z) {
                continue;
            }

            let color = if veins.iter().any(|(v, k)| {
                *k == crate::matter::DepositKind::Iron && v.distance(Vec2::new(x, z)) < 9.0
            }) {
                Some([0.70, 0.15, 0.08, 0.75])
            } else if veins.iter().any(|(v, k)| {
                *k == crate::matter::DepositKind::Clay && v.distance(Vec2::new(x, z)) < 9.0
            }) {
                Some([0.82, 0.45, 0.12, 0.70])
            } else if veins.iter().any(|(v, k)| {
                *k == crate::matter::DepositKind::Stone && v.distance(Vec2::new(x, z)) < 11.0
            }) {
                // A worked quarry reads STRONGER than the broken ground below
                // it. Once the loose stone came off the world this is the one
                // answer to "where is our masonry", and a wash that says
                // "somewhere on that slope" is not an answer.
                Some([0.50, 0.56, 0.70, 0.85])
            } else if terrain.slope_at(x, z) > 0.42
                || stones.iter().any(|s| s.distance(Vec2::new(x, z)) < 5.0)
            {
                Some([0.50, 0.56, 0.70, 0.45])
            } else if terrain.forest_at(x, z) > 0.50 && terrain.moisture_at(x, z) > 0.38 {
                // Promise what the biome will deliver, as the founding
                // survey does — thin arid scrub reads faint.
                let density = ((terrain.forest_at(x, z) - 0.50) / 0.30).clamp(0.05, 1.0);
                let thick = match terrain.biome_at(x, z) {
                    Biome::Arid => 0.25,
                    Biome::Alpine => 0.4,
                    _ => 1.0,
                };
                Some([0.08, 0.68, 0.12, 0.15 + 0.55 * density * thick])
            } else if heaths.iter().any(|h| h.distance(Vec2::new(x, z)) < 4.0) {
                // Open heath with food on the bush — below the woods in
                // priority, or every berried forest would read rose.
                Some([0.88, 0.20, 0.45, 0.55])
            } else {
                None
            };
            let Some(color) = color else {
                continue;
            };
            // The translucency is baked, not blended: each cell's wash is
            // mixed with the true ground color beneath it and rendered
            // opaque, because the transparent pass proved unwilling to
            // draw a ground-hugging sheet this large at all.
            let ground = crate::terrain::ground_color_at(&terrain, x, z);
            let a = color[3];
            let color = [
                ground[0] * (1.0 - a) + color[0] * a,
                ground[1] * (1.0 - a) + color[1] * a,
                ground[2] * (1.0 - a) + color[2] * a,
                1.0,
            ];

            let base = positions.len() as u32;
            // Inset each tile so a seam of true ground shows between
            // cells: a wash with grout lines reads as the god's chart,
            // where an unbroken blanket just looks like repainted grass.
            for (cx, cz) in [
                (0.35, 0.35),
                (CELL - 0.35, 0.35),
                (CELL - 0.35, CELL - 0.35),
                (0.35, CELL - 0.35),
            ] {
                let wx = origin_x + i as f32 * CELL + cx;
                let wz = origin_z + j as f32 * CELL + cz;
                let y = terrain.height_at(wx, wz).max(WATER_LEVEL) + 0.45;
                // BENT ONTO THE PLANET, like the ground it lies on.
                //
                // These went in as flat sim coordinates with a flat up-normal,
                // which was right when the world was flat and has been wrong
                // since the day it went round: the sheet stayed a plane in sim
                // space while the ground curved away under it, so the chart hung
                // in the air at the edges and sank through the hill in the
                // middle. The rivers hit this and the boulders hit this; the
                // survey was simply never converted. Brett: "the resource
                // heatmap from pressing R never was fixed after the world went
                // from flat to round."
                //
                // Bent per VERTEX rather than seating the whole sheet, because
                // it is four hundred meters across - far too wide for one
                // seat's flat approximation to hold at its corners.
                let (seat, turn) = crate::globe::bend_frame(Vec3::new(wx, y, wz));
                positions.push(seat.to_array());
                normals.push((turn * Vec3::Y).to_array());
                colors.push(color);
            }
            indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    if positions.is_empty() {
        return;
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    // Each corner's own up, off the sphere. A single flat normal lit the whole
    // chart as though it faced straight up wherever it was standing.
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));

    commands.spawn((
        Name::new("The god's survey"),
        SurveySheet,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        })),
        Transform::default(),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}
