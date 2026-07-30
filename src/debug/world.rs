//! THE WORLD — the codex's last page: the lands your people walk, drawn as
//! a real map.
//!
//! The map is not an illustration: every pixel is sampled from the same
//! terrain function that builds the ground underfoot — heights, biomes,
//! rivers, sea — hillshaded, and dimmed beyond the ring of the known world.
//! It paints itself a few rows per frame when the page opens, like a
//! cartographer working, and redraws at a new radius when the zoom changes.

use crate::terrain::WATER_LEVEL;
use crate::ui;
use bevy::prelude::*;

use super::village::{glyph_canvas, house_glyph};

/// The toolbar button that turns the codex to this page.
#[derive(Component)]
pub(crate) struct WorldButton;

/// The world page node; every is-it-open gate reads its visibility.
#[derive(Component)]
pub(crate) struct WorldPanel;

/// The map image and the painter's progress.
#[derive(Resource)]
pub(crate) struct WorldMap {
    pub image: Handle<Image>,
    pub centre: Vec2,
    pub radius: f32,
    /// Rows painted so far; the painter rests at `size`.
    pub painted: u32,
    pub size: u32,
}

/// The zoom radii the +/- buttons walk.
const ZOOM_STEPS: [f32; 3] = [260.0, 440.0, 760.0];

/// A zoom button: +1 closes in, -1 pulls back.
#[derive(Component)]
pub(crate) struct MapZoomButton(i8);

/// The current zoom step.
#[derive(Resource)]
pub(crate) struct MapZoom(pub usize);

impl Default for MapZoom {
    fn default() -> Self {
        MapZoom(1)
    }
}

/// The overlay the markers live in, rebuilt as knowledge grows.
#[derive(Component)]
pub(crate) struct MapMarkers;

/// Rail texts: the big date, and the season progress pieces.
#[derive(Component)]
pub(crate) struct WorldDate;

#[derive(Component)]
pub(crate) struct SeasonFill;

#[derive(Component)]
pub(crate) struct SeasonDays;

/// A season stop on the time card's track: 0 spring .. 3 winter.
#[derive(Component)]
pub(crate) struct SeasonStop(u8);

/// A WORLD CONDITIONS value row: 0 sky, 1 warmth, 2 moisture, 3 winds,
/// 4 the front, 5 hazards.
#[derive(Component)]
pub(crate) struct ConditionValue(u8);

/// A WORLD SUMMARY value row: 0 region, 1 coastline, 2 known ground,
/// 3 explored, 4 known sites.
#[derive(Component)]
pub(crate) struct SummaryValue(u8);

/// SEASONS & YEAR texts: 0 current, 1 next, 2 year length, 3 current year.
#[derive(Component)]
pub(crate) struct SeasonFact(u8);

/// The season wheel's needle: rotated to the year's progress.
#[derive(Component)]
pub(crate) struct SeasonNeedle;

/// A WORLD TRENDS value: 0 temperature, 1 moisture, 2 vegetation,
/// 3 wildlife, 4 rivers.
#[derive(Component)]
pub(crate) struct TrendValue(u8);

/// The RECENT WORLD EVENTS well, rebuilt as the book grows.
#[derive(Component)]
pub(crate) struct WorldEvents;

// ---------------------------------------------------------------------------
// Season marks.
// ---------------------------------------------------------------------------

fn season_glyph(commands: &mut Commands, parent: Entity, season: u8, tint: Color) {
    let c = glyph_canvas(commands, parent, 16.0);
    match season {
        // Spring: a leaf.
        0 => {
            super::village::bar(commands, c, (4.0, 3.0, 8.0, 10.0), 0.0, tint, true);
            super::village::bar(
                commands,
                c,
                (7.5, 4.0, 1.0, 10.0),
                0.0,
                tint.with_alpha(0.5),
                false,
            );
        }
        // Summer: the sun.
        1 => {
            super::village::bar(commands, c, (5.0, 5.0, 6.0, 6.0), 0.0, tint, true);
            for (l, t, w, h) in [
                (7.25, 0.5, 1.5, 3.0),
                (7.25, 12.5, 1.5, 3.0),
                (0.5, 7.25, 3.0, 1.5),
                (12.5, 7.25, 3.0, 1.5),
            ] {
                super::village::bar(commands, c, (l, t, w, h), 0.0, tint.with_alpha(0.7), false);
            }
        }
        // Autumn: the leaf lets go.
        2 => {
            super::village::bar(commands, c, (5.0, 2.0, 7.0, 8.5), 40.0, tint, true);
            super::village::bar(
                commands,
                c,
                (3.5, 11.5, 3.0, 1.5),
                0.0,
                tint.with_alpha(0.5),
                false,
            );
        }
        // Winter: a crossed flake.
        _ => {
            super::village::bar(commands, c, (7.25, 1.5, 1.5, 13.0), 0.0, tint, false);
            super::village::bar(commands, c, (7.25, 1.5, 1.5, 13.0), 60.0, tint, false);
            super::village::bar(commands, c, (7.25, 1.5, 1.5, 13.0), -60.0, tint, false);
        }
    }
}

fn season_tint(season: u8) -> Color {
    match season {
        0 => crate::palette::shade(&crate::palette::GRASS, 0.75),
        1 => crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.9),
        2 => crate::palette::shade(&crate::palette::EARTH, 0.95),
        _ => crate::palette::shade(&crate::palette::SKY, 0.85),
    }
}

fn season_name(season: u8) -> &'static str {
    ["Spring", "Summer", "Autumn", "Winter"][season as usize % 4]
}

// ---------------------------------------------------------------------------
// The page.
// ---------------------------------------------------------------------------

/// The map texture's side, in pixels.
const MAP_SIZE: u32 = 288;
/// Rows the cartographer paints per frame.
const ROWS_PER_FRAME: u32 = 8;

pub(crate) fn spawn_world_panel(
    mut commands: Commands,
    codex: Res<super::village::Codex>,
    mut images: ResMut<Assets<Image>>,
) {
    let page = codex.world_page;
    commands
        .entity(page)
        .insert((Name::new("World Page"), WorldPanel));

    // The unpainted parchment: near-black until the cartographer works.
    let blank = vec![0u8; (MAP_SIZE * MAP_SIZE * 4) as usize];
    let image = images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: MAP_SIZE,
            height: MAP_SIZE,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        blank,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    ));
    commands.insert_resource(WorldMap {
        image: image.clone(),
        centre: Vec2::ZERO,
        radius: ZOOM_STEPS[1],
        painted: 0,
        size: MAP_SIZE,
    });
    commands.init_resource::<MapZoom>();

    // ---- The main row: the rail and the map. ------------------------------
    let main = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                align_items: AlignItems::Stretch,
                ..default()
            },
            ChildOf(page),
        ))
        .id();

    let rail = commands
        .spawn((
            Node {
                width: px(300),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            ChildOf(main),
        ))
        .id();

    // CURRENT TIME: the date writ large, the season track, the progress.
    let time_card = ui::card_well(&mut commands, rail, "CURRENT TIME");
    commands.entity(time_card).insert(Node {
        width: percent(100),
        flex_shrink: 0.0,
        flex_grow: 0.0,
        flex_basis: Val::Auto,
        flex_direction: FlexDirection::Column,
        row_gap: px(8),
        padding: UiRect::all(px(10)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(0)),
        overflow: Overflow::clip(),
        ..default()
    });
    commands.spawn((
        WorldDate,
        Text::new(""),
        ui::DisplayFace,
        TextFont {
            font_size: FontSize::Px(17.0),
            ..default()
        },
        TextColor(ui::theme::accent()),
        TextLayout {
            linebreak: LineBreak::NoWrap,
            ..default()
        },
        ChildOf(time_card),
    ));
    let track = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(8), px(2)),
                ..default()
            },
            ChildOf(time_card),
        ))
        .id();
    for season in 0u8..4 {
        let stop = commands
            .spawn((
                SeasonStop(season),
                Node {
                    padding: UiRect::all(px(5)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(20)),
                    ..default()
                },
                BorderColor::all(ui::theme::panel_border().with_alpha(0.3)),
                ChildOf(track),
            ))
            .id();
        season_glyph(&mut commands, stop, season, season_tint(season));
    }
    let progress_row = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            ChildOf(time_card),
        ))
        .id();
    let progress_track = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: px(9),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.45)),
            ChildOf(progress_row),
        ))
        .id();
    commands.spawn((
        SeasonFill,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            width: percent(0),
            ..default()
        },
        BackgroundColor(crate::palette::shade(&crate::palette::GRASS, 0.7)),
        ChildOf(progress_track),
    ));
    commands.spawn((SeasonDays, ui::dim(""), ChildOf(progress_row)));

    // WORLD CONDITIONS: the sky's whole report.
    let conditions = ui::card_well(&mut commands, rail, "WORLD CONDITIONS");
    commands.entity(conditions).insert(Node {
        width: percent(100),
        flex_shrink: 0.0,
        flex_grow: 0.0,
        flex_basis: Val::Auto,
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        padding: UiRect::all(px(10)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(0)),
        overflow: Overflow::clip(),
        ..default()
    });
    for (index, label) in [
        (0u8, "sky"),
        (1, "warmth"),
        (2, "moisture"),
        (3, "winds"),
        (4, "the front"),
    ] {
        let value = ui::ruled_row(&mut commands, conditions, label);
        commands.entity(value).insert(ConditionValue(index));
    }

    // WORLD SUMMARY: what kind of country this is.
    let summary = ui::card_well(&mut commands, rail, "WORLD SUMMARY");
    for (index, label) in [
        (0u8, "region"),
        (1, "coastline"),
        (2, "known ground"),
        (4u8, "known sites"),
    ] {
        let value = ui::ruled_row(&mut commands, summary, label);
        commands.entity(value).insert(SummaryValue(index));
    }

    // THE MAP: the land itself, with its legend and its zoom.
    let map_card = ui::card_well(&mut commands, main, "WORLD MAP");
    let map_row = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                align_items: AlignItems::Stretch,
                ..default()
            },
            ChildOf(map_card),
        ))
        .id();
    let map_frame = commands
        .spawn((
            Node {
                height: percent(100),
                aspect_ratio: Some(1.0),
                flex_shrink: 0.0,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.5)),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(map_row),
        ))
        .id();
    commands.spawn((
        bevy::ui::widget::ImageNode::new(image),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: percent(100),
            ..default()
        },
        ChildOf(map_frame),
    ));
    commands.spawn((
        MapMarkers,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: percent(100),
            ..default()
        },
        ChildOf(map_frame),
    ));
    // The zoom, riding the map's corner.
    let zooms = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(8),
                top: px(8),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
            ChildOf(map_frame),
        ))
        .id();
    for (delta, mark) in [(1i8, "+"), (-1, "-")] {
        let button = commands
            .spawn((
                MapZoomButton(delta),
                ui::UiButton,
                ui::KeepFace,
                Node {
                    width: px(26),
                    height: px(26),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    ..default()
                },
                BackgroundColor(ui::theme::title_bg().with_alpha(0.9)),
                BorderColor::all(ui::theme::panel_border()),
                Interaction::default(),
                ChildOf(zooms),
            ))
            .id();
        commands.spawn((
            Text::new(mark),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(ui::theme::accent()),
            ChildOf(button),
        ));
    }
    // The legend.
    let legend = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: UiRect::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.2)),
            ChildOf(map_row),
        ))
        .id();
    commands.spawn((
        Text::new("LEGEND"),
        ui::DisplayFace,
        TextFont {
            font_size: FontSize::Px(ui::theme::SMALL_SIZE),
            ..default()
        },
        TextColor(ui::theme::text_dim()),
        ChildOf(legend),
    ));
    let legend_row = |commands: &mut Commands, label: &str| -> Entity {
        let row = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    ..default()
                },
                ChildOf(legend),
            ))
            .id();
        let swatch = commands.spawn((Node::default(), ChildOf(row))).id();
        commands.spawn((ui::label(label), ChildOf(row)));
        swatch
    };
    let swatch_box = |commands: &mut Commands, parent: Entity, tint: Color| {
        let c = glyph_canvas(commands, parent, 16.0);
        super::village::bar(commands, c, (2.0, 2.0, 12.0, 12.0), 0.0, tint, false);
    };
    let s = legend_row(&mut commands, "the village");
    house_glyph(&mut commands, s, ui::theme::accent());
    let s = legend_row(&mut commands, "waystone cairn");
    {
        let c = glyph_canvas(&mut commands, s, 16.0);
        super::village::bar(
            &mut commands,
            c,
            (4.5, 4.5, 7.0, 7.0),
            45.0,
            ui::theme::accent().with_alpha(0.8),
            false,
        );
    }
    let s = legend_row(&mut commands, "a far place, known");
    {
        let c = glyph_canvas(&mut commands, s, 16.0);
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(3),
                top: px(3),
                width: px(10),
                height: px(10),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BorderColor::all(ui::theme::accent().with_alpha(0.55)),
            ChildOf(c),
        ));
    }
    let s = legend_row(&mut commands, "forest");
    swatch_box(&mut commands, s, Color::srgb(0.16, 0.30, 0.14));
    let s = legend_row(&mut commands, "the sea");
    swatch_box(&mut commands, s, Color::srgb(0.13, 0.29, 0.40));
    let s = legend_row(&mut commands, "unknown country");
    swatch_box(&mut commands, s, Color::srgb(0.07, 0.08, 0.10));

    // ---- The bottom row: seasons, trends, events. -------------------------
    let bottom = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(150),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                align_items: AlignItems::Stretch,
                ..default()
            },
            ChildOf(page),
        ))
        .id();

    let seasons = ui::card_well(&mut commands, bottom, "SEASONS & YEAR");
    let seasons_row = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Row,
                column_gap: px(14),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(seasons),
        ))
        .id();
    // The wheel: a ring, four season marks, and the year's needle.
    let wheel = commands
        .spawn((
            Node {
                width: px(86),
                height: px(86),
                flex_shrink: 0.0,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(86)),
                ..default()
            },
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(seasons_row),
        ))
        .id();
    for (season, (left, top)) in [
        (0u8, (33.0, 4.0)),
        (1, (62.0, 33.0)),
        (2, (33.0, 62.0)),
        (3, (4.0, 33.0)),
    ] {
        let seat = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(top),
                    ..default()
                },
                ChildOf(wheel),
            ))
            .id();
        season_glyph(&mut commands, seat, season, season_tint(season));
    }
    let needle_hub = commands
        .spawn((
            SeasonNeedle,
            Node {
                position_type: PositionType::Absolute,
                left: px(11),
                top: px(11),
                width: px(60),
                height: px(60),
                ..default()
            },
            ChildOf(wheel),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(29),
            top: px(6),
            width: px(2),
            height: px(24),
            ..default()
        },
        BackgroundColor(ui::theme::accent()),
        ChildOf(needle_hub),
    ));
    let season_facts = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                ..default()
            },
            ChildOf(seasons_row),
        ))
        .id();
    for (index, label) in [
        (0u8, "season"),
        (1, "next"),
        (2, "year length"),
        (3, "year"),
    ] {
        let value = ui::ruled_row(&mut commands, season_facts, label);
        commands.entity(value).insert(SeasonFact(index));
    }

    let trends = ui::card_well(&mut commands, bottom, "WORLD TRENDS");
    for (index, label) in [
        (0u8, "temperature"),
        (1, "moisture"),
        (2, "vegetation"),
        (3, "wildlife"),
    ] {
        let value = ui::ruled_row(&mut commands, trends, label);
        commands.entity(value).insert(TrendValue(index));
    }

    let events = ui::card_well(&mut commands, bottom, "RECENT WORLD EVENTS");
    commands.spawn((
        WorldEvents,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ui::Scrollable,
        ScrollPosition::DEFAULT,
        Interaction::default(),
        ChildOf(events),
    ));

    // The closing line.
    commands.spawn((
        ui::dim("\"The earth is vast and patient. Learn its ways, and it will sustain you.\""),
        Node {
            flex_shrink: 0.0,
            align_self: AlignSelf::Center,
            margin: UiRect::top(px(2)),
            ..default()
        },
        ChildOf(page),
    ));
}

/// The cartographer: paints a few rows of the map each frame while the page
/// is open, sampling the same terrain function the world is built from.
pub(crate) fn paint_world_map(
    codex: Res<super::village::Codex>,
    panels: Query<&Visibility, With<WorldPanel>>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    site: Option<Res<crate::villager::SettlementSite>>,
    known: Option<Res<crate::villager::explore::KnownWorld>>,
    mut map: ResMut<WorldMap>,
    mut images: ResMut<Assets<Image>>,
) {
    if codex.page != super::village::CodexPage::World
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        return;
    }
    let Some(terrain) = terrain else {
        return;
    };
    // The map centres on the banner the first time it can.
    if map.painted == 0
        && let Some(site) = site.as_ref()
    {
        map.centre = Vec2::new(site.centre.x, site.centre.z);
    }
    if map.painted >= map.size {
        return;
    }
    let Some(mut image) = images.get_mut(&map.image) else {
        return;
    };
    let Some(data) = image.data.as_mut() else {
        return;
    };

    let size = map.size;
    let radius = map.radius;
    let centre = map.centre;
    let known_centre = known.as_ref().map(|k| Vec2::new(k.centre.x, k.centre.z));
    let known_radius = known.as_ref().map_or(0.0, |k| k.radius);
    let pockets: Vec<(Vec2, f32)> = known.as_ref().map_or_else(Vec::new, |k| {
        k.pockets
            .iter()
            .map(|p| (Vec2::new(p.at.x, p.at.z), p.radius))
            .collect()
    });

    let rows_to_paint = ROWS_PER_FRAME.min(size - map.painted);
    for row in map.painted..map.painted + rows_to_paint {
        for col in 0..size {
            let u = col as f32 / (size - 1) as f32;
            let v = row as f32 / (size - 1) as f32;
            let x = centre.x + (u - 0.5) * 2.0 * radius;
            let z = centre.y + (v - 0.5) * 2.0 * radius;
            let height = terrain.height_at(x, z);

            let mut colour: [f32; 3];
            if height < WATER_LEVEL {
                // The sea, deeper is darker.
                let depth = ((WATER_LEVEL - height) / 14.0).clamp(0.0, 1.0);
                let shallow = [0.16, 0.34, 0.44];
                let deep = [0.05, 0.13, 0.20];
                colour = [
                    shallow[0] + (deep[0] - shallow[0]) * depth,
                    shallow[1] + (deep[1] - shallow[1]) * depth,
                    shallow[2] + (deep[2] - shallow[2]) * depth,
                ];
            } else if terrain.river_surface_at(x, z).is_some() {
                colour = [0.16, 0.36, 0.48];
            } else {
                let ground = crate::terrain::ground_color_at(&terrain, x, z);
                colour = [
                    ground[0].powf(1.0 / 2.2),
                    ground[1].powf(1.0 / 2.2),
                    ground[2].powf(1.0 / 2.2),
                ];
                // The woods darken their country.
                let forest = terrain.forest_at(x, z);
                if forest > 0.45 {
                    let shade = ((forest - 0.45) * 1.6).clamp(0.0, 0.55);
                    colour = [
                        colour[0] * (1.0 - shade) + 0.05 * shade,
                        colour[1] * (1.0 - shade) + 0.16 * shade,
                        colour[2] * (1.0 - shade) + 0.05 * shade,
                    ];
                }
                // Hillshade from the northwest, so relief reads as relief.
                let step = radius / size as f32 * 2.0;
                let lit = terrain.height_at(x - step, z - step);
                let slope = ((height - lit) / step).clamp(-1.2, 1.2);
                let shade = 1.0 + slope * 0.35;
                colour = [colour[0] * shade, colour[1] * shade, colour[2] * shade];
            }

            // Beyond the cairns' ring - and outside every far place the
            // explorers brought home - the land is rumour, drawn dark.
            let world = Vec2::new(x, z);
            let known_here = known_centre.is_some_and(|c| world.distance(c) <= known_radius)
                || pockets
                    .iter()
                    .any(|(at, reach)| world.distance(*at) <= *reach);
            if !known_here {
                colour = [
                    colour[0] * 0.30 + 0.015,
                    colour[1] * 0.30 + 0.018,
                    colour[2] * 0.30 + 0.025,
                ];
            }

            let offset = ((row * size + col) * 4) as usize;
            data[offset] = (colour[0].clamp(0.0, 1.0) * 255.0) as u8;
            data[offset + 1] = (colour[1].clamp(0.0, 1.0) * 255.0) as u8;
            data[offset + 2] = (colour[2].clamp(0.0, 1.0) * 255.0) as u8;
            data[offset + 3] = 255;
        }
    }
    map.painted += rows_to_paint;
}

/// The zoom buttons redraw the map at the next radius in or out.
pub(crate) fn handle_map_zoom(
    buttons: Query<(&Interaction, &MapZoomButton), Changed<Interaction>>,
    mut zoom: ResMut<MapZoom>,
    mut map: ResMut<WorldMap>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let fresh = if button.0 > 0 {
            zoom.0.saturating_sub(1)
        } else {
            (zoom.0 + 1).min(ZOOM_STEPS.len() - 1)
        };
        if fresh != zoom.0 {
            zoom.0 = fresh;
            map.radius = ZOOM_STEPS[fresh];
            map.painted = 0;
        }
    }
}

/// Lays the markers over the map: the village, every cairn, every known
/// pocket. Rebuilt on a slow clock while the page is open.
#[allow(clippy::type_complexity)]
pub(crate) fn update_world_markers(
    mut commands: Commands,
    codex: Res<super::village::Codex>,
    panels: Query<&Visibility, With<WorldPanel>>,
    time: Res<Time>,
    mut last_rebuild: Local<f32>,
    map: Res<WorldMap>,
    site: Option<Res<crate::villager::SettlementSite>>,
    settlements: Query<&crate::villager::Settlement>,
    villagers: Query<
        (),
        (
            With<crate::villager::Villager>,
            Without<crate::creature::Corpse>,
        ),
    >,
    cairns: Query<&GlobalTransform, With<crate::villager::explore::Cairn>>,
    known: Option<Res<crate::villager::explore::KnownWorld>>,
    wells: Query<Entity, With<MapMarkers>>,
) {
    if codex.page != super::village::CodexPage::World
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        *last_rebuild = 10.0;
        return;
    }
    *last_rebuild += time.delta_secs();
    if *last_rebuild < 2.0 {
        return;
    }
    *last_rebuild = 0.0;
    let Ok(well) = wells.single() else {
        return;
    };
    commands.entity(well).despawn_related::<Children>();

    let place = |world: Vec2| -> Option<(f32, f32)> {
        let u = (world.x - map.centre.x) / (2.0 * map.radius) + 0.5;
        let v = (world.y - map.centre.y) / (2.0 * map.radius) + 0.5;
        ((0.02..0.98).contains(&u) && (0.02..0.98).contains(&v)).then_some((u * 100.0, v * 100.0))
    };

    // The village, named, with its souls.
    if let Some(site) = site.as_ref()
        && let Some((u, v)) = place(Vec2::new(site.centre.x, site.centre.z))
    {
        let tag = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(u),
                    top: percent(v),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(5),
                    padding: UiRect::axes(px(6), px(3)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.55)),
                BorderColor::all(ui::theme::accent().with_alpha(0.5)),
                ChildOf(well),
            ))
            .id();
        house_glyph(&mut commands, tag, ui::theme::accent());
        let name = settlements
            .get(site.settlement)
            .map_or("the village".to_string(), |s| s.name.clone());
        commands.spawn((
            ui::label(format!("{name}  -  {} souls", villagers.iter().count())),
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            ChildOf(tag),
        ));
    }

    // The cairns: the boundary of knowledge, as turned squares.
    for cairn in &cairns {
        let at = cairn.translation();
        if let Some((u, v)) = place(Vec2::new(at.x, at.z)) {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(u),
                    top: percent(v),
                    width: px(6),
                    height: px(6),
                    ..default()
                },
                UiTransform::from_rotation(Rot2::degrees(45.0)),
                BackgroundColor(ui::theme::accent().with_alpha(0.75)),
                ChildOf(well),
            ));
        }
    }

    // The known pockets: rings of rumour made ground.
    if let Some(known) = known.as_ref() {
        for pocket in &known.pockets {
            if let Some((u, v)) = place(Vec2::new(pocket.at.x, pocket.at.z)) {
                commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(u),
                        top: percent(v),
                        width: px(12),
                        height: px(12),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(12)),
                        ..default()
                    },
                    BorderColor::all(ui::theme::accent().with_alpha(0.55)),
                    ChildOf(well),
                ));
            }
        }
    }
}

/// Fills the rail, the wheel, the trends and the events while the page is
/// open.
#[allow(clippy::type_complexity)]
pub(crate) fn update_world_panel(
    mut commands: Commands,
    codex: Res<super::village::Codex>,
    // Grouped: sixteen params is the compiler's ceiling, not this page's.
    (clock, sky, weather, terrain, site, known, map, history): (
        Res<crate::calendar::WorldClock>,
        Option<Res<crate::calendar::Sky>>,
        Option<Res<crate::weather::Weather>>,
        Option<Res<crate::terrain::Terrain>>,
        Option<Res<crate::villager::SettlementSite>>,
        Option<Res<crate::villager::explore::KnownWorld>>,
        Option<Res<WorldMap>>,
        Res<crate::villager::WorldChronicle>,
    ),
    (panels, cairns, wildlife, events_wells): (
        Query<&Visibility, With<WorldPanel>>,
        Query<(), With<crate::villager::explore::Cairn>>,
        Query<
            (),
            (
                With<crate::creature::wildlife::Wild>,
                Without<crate::creature::Corpse>,
            ),
        >,
        Query<Entity, With<WorldEvents>>,
    ),
    (mut needles, mut stops, mut fills): (
        Query<&mut UiTransform, With<SeasonNeedle>>,
        Query<(&SeasonStop, &mut BorderColor)>,
        Query<&mut Node, With<SeasonFill>>,
    ),
    mut events_seen: Local<(usize, bool)>,
    mut texts: ParamSet<(
        Query<&mut Text, With<WorldDate>>,
        Query<&mut Text, With<SeasonDays>>,
        Query<(&ConditionValue, &mut Text)>,
        Query<(&SummaryValue, &mut Text)>,
        Query<(&SeasonFact, &mut Text)>,
        Query<(&TrendValue, &mut Text)>,
    )>,
) {
    if codex.page != super::village::CodexPage::World
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        events_seen.1 = false;
        return;
    }
    let set = |text: &mut Text, fresh: String| {
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    };

    let day = clock.day();
    let season = ((day / 28) % 4) as u8;
    let day_of_season = day % 28 + 1;
    let year = day / 112 + 1;

    if let Ok(mut text) = texts.p0().single_mut() {
        set(&mut text, clock.date_phrase());
    }
    if let Ok(mut text) = texts.p1().single_mut() {
        set(&mut text, format!("day {day_of_season} / 28"));
    }
    for (stop, mut border) in &mut stops {
        *border = BorderColor::all(if stop.0 == season {
            ui::theme::accent().with_alpha(0.9)
        } else {
            ui::theme::panel_border().with_alpha(0.3)
        });
    }
    for mut node in &mut fills {
        node.width = percent(day_of_season as f32 / 28.0 * 100.0);
    }
    for mut needle in &mut needles {
        let year_fraction = (day % 112) as f32 / 112.0;
        *needle = UiTransform::from_rotation(Rot2::degrees(year_fraction * 360.0));
    }

    // Conditions, in the sky's own words.
    let intensity = weather.as_ref().map_or(0.0, |w| w.intensity);
    let wind = weather.as_ref().map_or(0.0, |w| w.wind);
    for (row, mut text) in &mut texts.p2() {
        let fresh = match row.0 {
            0 => weather
                .as_ref()
                .map_or("-".to_string(), |w| w.kind().describe().to_string()),
            1 => match (&weather, &sky) {
                (Some(weather), Some(sky)) => weather.temperature_word(sky.daylight).to_string(),
                _ => "-".to_string(),
            },
            2 => match intensity {
                i if i > 0.65 => "sodden".to_string(),
                i if i > 0.35 => "humid".to_string(),
                i if i > 0.15 => "fresh".to_string(),
                _ => "dry".to_string(),
            },
            3 => match wind {
                w if w > 0.7 => "a hard gale".to_string(),
                w if w > 0.45 => "a strong wind".to_string(),
                w if w > 0.22 => "a light breeze".to_string(),
                _ => "still air".to_string(),
            },
            _ => {
                if intensity > 0.75 {
                    "storm overhead".to_string()
                } else {
                    weather.as_ref().map_or("-".to_string(), |w| {
                        if w.target > w.intensity + 0.12 {
                            "weather gathering".to_string()
                        } else if w.target < w.intensity - 0.12 {
                            "skies clearing".to_string()
                        } else {
                            "holding".to_string()
                        }
                    })
                }
            }
        };
        set(&mut text, fresh);
    }

    // The summary: what kind of country, and how much of it is yours.
    let known_radius = known.as_ref().map_or(0.0, |k| k.radius);
    let map_radius = map.as_ref().map_or(440.0, |m| m.radius);
    for (row, mut text) in &mut texts.p3() {
        let fresh = match row.0 {
            0 => match (&terrain, &site) {
                (Some(terrain), Some(site)) => {
                    match terrain.biome_at(site.centre.x, site.centre.z) {
                        crate::terrain::Biome::Temperate => "temperate country".to_string(),
                        crate::terrain::Biome::Boreal => "cold forest country".to_string(),
                        crate::terrain::Biome::Arid => "dry country".to_string(),
                        crate::terrain::Biome::Wetland => "wet country".to_string(),
                        crate::terrain::Biome::Alpine => "high country".to_string(),
                    }
                }
                _ => "-".to_string(),
            },
            1 => match (&terrain, &site) {
                (Some(terrain), Some(site)) => {
                    // Eight bearings, a long look each way: where the sea lies.
                    let names = [
                        "north",
                        "northeast",
                        "east",
                        "southeast",
                        "south",
                        "southwest",
                        "west",
                        "northwest",
                    ];
                    let mut sea: Option<&str> = None;
                    for (index, name) in names.iter().enumerate() {
                        let angle = index as f32 * std::f32::consts::TAU / 8.0;
                        let x = site.centre.x + angle.sin() * 500.0;
                        let z = site.centre.z - angle.cos() * 500.0;
                        if terrain.height_at(x, z) < WATER_LEVEL {
                            sea = Some(name);
                            break;
                        }
                    }
                    sea.map_or("no sea within reach".to_string(), |name| {
                        format!("the sea lies {name}")
                    })
                }
                _ => "-".to_string(),
            },
            2 => format!("{known_radius:.0} paces around the banner"),
            3 => {
                let known_area = known_radius * known_radius;
                let map_area = map_radius * map_radius;
                format!(
                    "{:.0}% of the map",
                    (known_area / map_area * 100.0).min(100.0)
                )
            }
            _ => format!(
                "{} cairns, {} far places",
                cairns.iter().count(),
                known.as_ref().map_or(0, |k| k.pockets.len()),
            ),
        };
        set(&mut text, fresh);
    }

    // Seasons and the year.
    for (fact, mut text) in &mut texts.p4() {
        let fresh = match fact.0 {
            0 => season_name(season).to_string(),
            1 => season_name((season + 1) % 4).to_string(),
            2 => "112 days".to_string(),
            _ => format!("Year {year}"),
        };
        set(&mut text, fresh);
    }

    // Trends: read from the season and the sky, never invented.
    let wild = wildlife.iter().count();
    for (trend, mut text) in &mut texts.p5() {
        let fresh = match trend.0 {
            0 => match season {
                0 => "warming".to_string(),
                1 => "high summer".to_string(),
                2 => "cooling".to_string(),
                _ => "deep cold".to_string(),
            },
            1 => weather.as_ref().map_or("-".to_string(), |w| {
                if w.target > w.intensity + 0.12 {
                    "gathering".to_string()
                } else if w.target < w.intensity - 0.12 {
                    "drying".to_string()
                } else {
                    "steady".to_string()
                }
            }),
            2 => match season {
                0 | 1 => "thriving".to_string(),
                2 => "fading".to_string(),
                _ => "dormant".to_string(),
            },
            _ => match wild {
                w if w >= 30 => format!("abundant  ({w})"),
                w if w >= 10 => format!("present  ({w})"),
                w => format!("scarce  ({w})"),
            },
        };
        set(&mut text, fresh);
    }

    // Recent world events, from the world's own shelf of the chronicle.
    let fresh_events = history.events.len() != events_seen.0 || !events_seen.1;
    if fresh_events && let Ok(well) = events_wells.single() {
        *events_seen = (history.events.len(), true);
        commands.entity(well).despawn_related::<Children>();
        let mut shown = 0;
        for event in history.events.iter().rev() {
            if super::history::Ledger::of(&event.text) != super::history::Ledger::World {
                continue;
            }
            let row = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        padding: UiRect::bottom(px(3)),
                        border: UiRect::bottom(px(1)),
                        ..default()
                    },
                    BorderColor::all(ui::theme::text_dim().with_alpha(0.1)),
                    ChildOf(well),
                ))
                .id();
            let text_cell = commands
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: px(0),
                        ..default()
                    },
                    ChildOf(row),
                ))
                .id();
            commands.spawn((
                ui::label(event.text.clone()),
                Node {
                    width: percent(100),
                    ..default()
                },
                ChildOf(text_cell),
            ));
            let ago = day.saturating_sub(event.day);
            let when = match ago {
                0 => "today".to_string(),
                1 => "yesterday".to_string(),
                n => format!("{n} days ago"),
            };
            commands.spawn((
                ui::dim(when),
                TextLayout {
                    linebreak: LineBreak::NoWrap,
                    ..default()
                },
                ChildOf(row),
            ));
            shown += 1;
            if shown >= 6 {
                break;
            }
        }
        if shown == 0 {
            commands.spawn((
                ui::dim("the world keeps its own counsel, for now"),
                ChildOf(well),
            ));
        }
    }
}
