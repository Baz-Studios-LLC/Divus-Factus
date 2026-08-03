//! THE DEITY — the god's own page in the codex: the name the people gave,
//! the miracles earned and waiting, how the congregation feels, and the
//! whole biography of faith drawn as one line.
//!
//! The left column holds the god's room: real architecture on a private
//! stage, rendered by its own camera — standing empty, for now, on purpose.

use crate::ui;
use crate::villager::Villager;
use crate::witness::Witnessed;
use bevy::prelude::*;

use super::village::{glyph_canvas, hands_glyph, house_glyph, people_glyph};

/// The deity page node; every is-it-open gate reads its visibility.
#[derive(Component)]
pub(crate) struct GodPanel;

#[derive(Component)]
pub(crate) struct DeityName;

#[derive(Component)]
pub(crate) struct DeitySubline;

/// The epithet card's reading: locked counsel, or the earned name.
#[derive(Component)]
pub(crate) struct EpithetBody;

/// A stat cell's value: 0 followers, 1 souls, 2 village, 3 age.
#[derive(Component)]
pub(crate) struct StatValue(u8);

/// A miracle card's live text: cost line or state line.
#[derive(Component)]
pub(crate) struct MiracleText {
    miracle: crate::miracles::Miracle,
    /// 0 cost, 1 state, 2 reason.
    field: u8,
}

/// A miracle card's face, dressed by its lock state.
#[derive(Component)]
pub(crate) struct MiracleCard(crate::miracles::Miracle);

/// A feelings line: the row, and whether this is the right-hand why.
#[derive(Component)]
pub(crate) struct FeelText {
    row: u8,
    why: bool,
}

#[derive(Component)]
pub(crate) struct PresenceFill;

#[derive(Component)]
pub(crate) struct PresenceValue;

/// The alignment card's live words: value, and caption beneath.
#[derive(Component)]
pub(crate) struct AlignText {
    caption: bool,
}

/// The faith chart's bar well, rebuilt as the record grows.
#[derive(Component)]
pub(crate) struct FaithChart;

/// The chart's x-axis caption.
#[derive(Component)]
pub(crate) struct FaithChartSpan;

/// Where the god's room stands: a stage of its own, far from the world and
/// the paperdoll alike. The manifestation module builds the apparition on
/// the same stage.
pub(crate) const DEITY_STAGE: Vec3 = Vec3::new(0.0, -700.0, 0.0);
pub(crate) const DEITY_LAYER: usize = 3;

/// The room's own camera, so the manifestation can switch it off while
/// nobody is looking.
#[derive(Component)]
pub(crate) struct DeityCamera;

// ---------------------------------------------------------------------------
// Small marks this page alone needs.
// ---------------------------------------------------------------------------

fn deity_bar(
    commands: &mut Commands,
    canvas: Entity,
    (left, top, width, height): (f32, f32, f32, f32),
    turn: f32,
    tint: Color,
    round: bool,
) {
    super::village::bar(
        commands,
        canvas,
        (left, top, width, height),
        turn,
        tint,
        round,
    );
}

/// A lightning bolt: two leaning bars meeting at the strike.
fn bolt_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    deity_bar(commands, c, (7.5, 0.5, 4.0, 8.5), 18.0, tint, false);
    deity_bar(commands, c, (6.0, 8.5, 4.0, 8.5), 18.0, tint, false);
}

/// A flourishing tree, in gold.
fn flourish_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    deity_bar(commands, c, (7.75, 11.0, 2.5, 6.0), 0.0, tint, false);
    deity_bar(commands, c, (3.5, 6.5, 11.0, 5.0), 0.0, tint, true);
    deity_bar(commands, c, (5.5, 2.5, 7.0, 4.5), 0.0, tint, true);
}

/// A bounty: three berries.
fn berry_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    deity_bar(commands, c, (3.0, 8.5, 5.5, 5.5), 0.0, tint, true);
    deity_bar(
        commands,
        c,
        (9.5, 8.5, 5.5, 5.5),
        0.0,
        tint.with_alpha(0.8),
        true,
    );
    deity_bar(
        commands,
        c,
        (6.25, 3.0, 5.5, 5.5),
        0.0,
        tint.with_alpha(0.9),
        true,
    );
}

/// Mended: a break made whole - two halves and the seam.
fn mend_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    deity_bar(commands, c, (3.0, 4.0, 5.0, 10.0), 0.0, tint, false);
    deity_bar(commands, c, (10.0, 4.0, 5.0, 10.0), 0.0, tint, false);
    deity_bar(
        commands,
        c,
        (8.25, 2.0, 1.5, 14.0),
        0.0,
        tint.with_alpha(0.6),
        false,
    );
}

/// Quake: the ground thrown - a floor split at an angle.
fn quake_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    deity_bar(commands, c, (1.5, 10.0, 7.0, 3.5), -14.0, tint, false);
    deity_bar(commands, c, (9.5, 8.0, 7.0, 3.5), 14.0, tint, false);
    deity_bar(
        commands,
        c,
        (7.0, 3.0, 4.0, 4.0),
        45.0,
        tint.with_alpha(0.7),
        false,
    );
}

/// A leaf: your unseen nature.
fn leaf_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    deity_bar(commands, c, (3.5, 3.0, 10.0, 12.0), 0.0, tint, true);
    deity_bar(
        commands,
        c,
        (8.25, 4.0, 1.5, 12.0),
        0.0,
        tint.with_alpha(0.5),
        false,
    );
}

/// Scales: the beam, the post, two pans.
fn scales_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    deity_bar(commands, c, (2.0, 3.5, 14.0, 2.0), 0.0, tint, false);
    deity_bar(commands, c, (8.0, 3.5, 2.0, 11.0), 0.0, tint, false);
    deity_bar(commands, c, (5.0, 14.5, 8.0, 2.0), 0.0, tint, false);
    deity_bar(
        commands,
        c,
        (1.0, 7.0, 5.0, 2.5),
        0.0,
        tint.with_alpha(0.8),
        true,
    );
    deity_bar(
        commands,
        c,
        (12.0, 7.0, 5.0, 2.5),
        0.0,
        tint.with_alpha(0.8),
        true,
    );
}

/// The living's domain: a ring with a green heart.
fn domain_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(2),
            top: px(2),
            width: px(14),
            height: px(14),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(14)),
            ..default()
        },
        BorderColor::all(tint),
        ChildOf(c),
    ));
    deity_bar(
        commands,
        c,
        (6.5, 6.5, 5.0, 5.0),
        0.0,
        crate::palette::shade(&crate::palette::GRASS, 0.7),
        true,
    );
}

/// The age of the world: a turned square shedding four rays.
fn age_glyph(commands: &mut Commands, parent: Entity, tint: Color) {
    let c = glyph_canvas(commands, parent, 18.0);
    deity_bar(commands, c, (6.0, 6.0, 6.0, 6.0), 45.0, tint, false);
    for (left, top, width, height) in [
        (8.25, 0.5, 1.5, 3.5),
        (8.25, 14.0, 1.5, 3.5),
        (0.5, 8.25, 3.5, 1.5),
        (14.0, 8.25, 3.5, 1.5),
    ] {
        deity_bar(
            commands,
            c,
            (left, top, width, height),
            0.0,
            tint.with_alpha(0.7),
            false,
        );
    }
}

// ---------------------------------------------------------------------------
// The page.
// ---------------------------------------------------------------------------

/// The bottom band's height, and the left column matched to a quarter.
const CREED_BAND: f32 = 112.0;
const LEFT_COLUMN: f32 = 273.0;

pub(crate) fn spawn_god_panel(
    mut commands: Commands,
    codex: Res<super::village::Codex>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let page = codex.deity_page;
    commands
        .entity(page)
        .insert((Name::new("Deity Page"), GodPanel));

    // ---- The god's room: real architecture on a private stage, empty. -----
    let target = images.add(bevy::image::Image::new_target_texture(
        522,
        640,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        Name::new("Deity Camera"),
        DeityCamera,
        Camera3d::default(),
        Camera {
            order: -21,
            clear_color: bevy::camera::ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        bevy::camera::RenderTarget::Image(target.clone().into()),
        Transform::from_translation(DEITY_STAGE + Vec3::new(0.0, 2.0, 5.2))
            .looking_at(DEITY_STAGE + Vec3::Y * 1.55, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DEITY_LAYER),
    ));
    commands.spawn((
        Name::new("Deity Key"),
        DirectionalLight {
            color: crate::palette::shade(&crate::palette::BONE, 1.0),
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_xyz(1.4, 2.8, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DEITY_LAYER),
    ));
    commands.spawn((
        Name::new("Deity Fill"),
        DirectionalLight {
            color: Color::srgb(0.75, 0.78, 0.85),
            illuminance: 2_600.0,
            ..default()
        },
        Transform::from_xyz(-2.4, 1.6, -1.0).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DEITY_LAYER),
    ));
    let layer = bevy::camera::visibility::RenderLayers::layer(DEITY_LAYER);
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.098, 0.104, 0.122),
        perceptual_roughness: 1.0,
        ..default()
    });
    let stone_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.066, 0.07, 0.084),
        perceptual_roughness: 1.0,
        ..default()
    });
    let velvet = materials.add(StandardMaterial {
        base_color: Color::srgb(0.028, 0.031, 0.042),
        perceptual_roughness: 1.0,
        ..default()
    });
    let gilt = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.14, 0.09),
        perceptual_roughness: 0.7,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mut set_piece =
        |mesh: Handle<Mesh>, material: Handle<StandardMaterial>, offset: Vec3, scale: Vec3| {
            commands.spawn((
                Name::new("Deity Room"),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(DEITY_STAGE + offset).with_scale(scale),
                layer.clone(),
            ));
        };
    // The dark heart, the wall it is cut into, and the floor.
    set_piece(
        cube.clone(),
        velvet.clone(),
        Vec3::new(0.0, 1.5, -1.1),
        Vec3::new(2.1, 3.2, 0.2),
    );
    set_piece(
        cube.clone(),
        stone.clone(),
        Vec3::new(0.0, 1.7, -1.3),
        Vec3::new(4.4, 4.6, 0.18),
    );
    set_piece(
        cube.clone(),
        stone_dark.clone(),
        Vec3::new(0.0, -0.09, 0.2),
        Vec3::new(4.4, 0.18, 3.4),
    );
    // Pillars, their capitals, and the lintel they carry.
    for side in [-1.0f32, 1.0] {
        set_piece(
            cube.clone(),
            stone.clone(),
            Vec3::new(side * 1.34, 1.4, -0.9),
            Vec3::new(0.46, 3.2, 0.46),
        );
        set_piece(
            cube.clone(),
            stone_dark.clone(),
            Vec3::new(side * 1.34, 3.05, -0.9),
            Vec3::new(0.62, 0.22, 0.62),
        );
    }
    set_piece(
        cube.clone(),
        stone.clone(),
        Vec3::new(0.0, 3.35, -0.9),
        Vec3::new(3.5, 0.5, 0.5),
    );
    // The empty dais, waiting: two tiers and a thread of gold.
    set_piece(
        cube.clone(),
        stone.clone(),
        Vec3::new(0.0, 0.09, 0.15),
        Vec3::new(1.8, 0.18, 1.5),
    );
    set_piece(
        cube.clone(),
        stone_dark.clone(),
        Vec3::new(0.0, 0.26, 0.15),
        Vec3::new(1.4, 0.16, 1.15),
    );
    set_piece(
        cube.clone(),
        gilt.clone(),
        Vec3::new(0.0, 0.185, 0.15),
        Vec3::new(1.84, 0.02, 1.54),
    );

    // ---- The columns. -----------------------------------------------------
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

    // LEFT: the room, the wondering, the presence.
    let left = commands
        .spawn((
            Node {
                width: px(LEFT_COLUMN),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            ChildOf(main),
        ))
        .id();
    let plate = commands
        .spawn((
            Node {
                width: percent(100),
                padding: UiRect::all(px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(ui::theme::title_bg()),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(left),
        ))
        .id();
    commands.spawn((
        bevy::ui::widget::ImageNode::new(target.clone()),
        Node {
            width: px(261),
            height: px(320),
            ..default()
        },
        ChildOf(plate),
    ));
    commands.spawn((
        ui::dim(
            "\"You do not walk among them, yet you are in all things. \
             They look to the heavens, and weep, and wonder.\"",
        ),
        Node {
            width: percent(100),
            padding: UiRect::axes(px(6), px(2)),
            ..default()
        },
        ChildOf(left),
    ));
    let presence = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                padding: UiRect::axes(px(6), px(4)),
                margin: UiRect::top(Val::Auto),
                ..default()
            },
            ChildOf(left),
        ))
        .id();
    let presence_head = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            ChildOf(presence),
        ))
        .id();
    commands.spawn((
        Text::new("YOUR PRESENCE"),
        ui::DisplayFace,
        TextFont {
            font_size: FontSize::Px(ui::theme::SMALL_SIZE),
            ..default()
        },
        TextColor(ui::theme::accent().with_alpha(0.9)),
        ChildOf(presence_head),
    ));
    commands.spawn((PresenceValue, ui::dim(""), ChildOf(presence_head)));
    let track = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(10),
                border_radius: BorderRadius::all(px(0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.45)),
            ChildOf(presence),
        ))
        .id();
    commands.spawn((
        PresenceFill,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            width: percent(0),
            ..default()
        },
        BackgroundColor(ui::theme::accent().with_alpha(0.85)),
        ChildOf(track),
    ));
    commands.spawn((ui::dim("Your influence in the world."), ChildOf(presence)));

    // RIGHT: the name, the numbers, the powers, the feelings, the curve.
    let right = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                padding: px(ui::theme::PAD).into(),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(ui::theme::card_bg()),
            BorderColor::all(ui::theme::card_border()),
            Interaction::default(),
            ChildOf(main),
        ))
        .id();

    // The masthead: the given name, and the epithet's card.
    let masthead = commands
        .spawn((
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                column_gap: px(12),
                ..default()
            },
            ChildOf(right),
        ))
        .id();
    let name_words = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            },
            ChildOf(masthead),
        ))
        .id();
    commands.spawn((
        DeityName,
        Text::new("..."),
        ui::DisplayFace,
        TextFont {
            font_size: FontSize::Px(34.0),
            ..default()
        },
        TextColor(ui::theme::accent()),
        ChildOf(name_words),
    ));
    commands.spawn((DeitySubline, ui::dim(""), ChildOf(name_words)));
    let epithet_card = ui::card_well(&mut commands, masthead, "EPITHET");
    commands.entity(epithet_card).insert(Node {
        flex_grow: 0.0,
        flex_basis: Val::Auto,
        width: px(280),
        min_height: px(0),
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        padding: UiRect::all(px(10)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(0)),
        overflow: Overflow::clip(),
        ..default()
    });
    commands.spawn((EpithetBody, ui::dim(""), ChildOf(epithet_card)));

    // The numbers band: followers, souls, village, age.
    let band = commands
        .spawn((
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                padding: UiRect::axes(px(8), px(8)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                column_gap: px(8),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg().with_alpha(0.55)),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(right),
        ))
        .id();
    let ink = ui::theme::accent().with_alpha(0.75);
    for (index, label, caption) in [
        (0u8, "FOLLOWERS", "believers"),
        (1, "SOULS", "under your watch"),
        (2, "VILLAGE", "your first village"),
        (3, "AGE", "since the founding"),
    ] {
        let cell = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    ..default()
                },
                ChildOf(band),
            ))
            .id();
        let badge = commands.spawn((Node::default(), ChildOf(cell))).id();
        match index {
            0 => hands_glyph(&mut commands, badge, ink),
            1 => people_glyph(&mut commands, badge, ink),
            2 => house_glyph(&mut commands, badge, ink),
            _ => age_glyph(&mut commands, badge, ink),
        }
        let words = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(1),
                    min_width: px(0),
                    ..default()
                },
                ChildOf(cell),
            ))
            .id();
        commands.spawn((
            Text::new(label),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(ui::theme::text_dim()),
            ChildOf(words),
        ));
        commands.spawn((
            StatValue(index),
            Text::new(""),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(ui::theme::accent()),
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            ChildOf(words),
        ));
        commands.spawn((ui::dim(caption), ChildOf(words)));
    }

    // MIRACLES, ruled across, then the five cards spanning the width.
    let rule_band = commands
        .spawn((
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            ChildOf(right),
        ))
        .id();
    let rule = |commands: &mut Commands, parent| {
        commands.spawn((
            Node {
                flex_grow: 1.0,
                height: px(1),
                ..default()
            },
            BackgroundColor(ui::theme::accent().with_alpha(0.25)),
            ChildOf(parent),
        ));
    };
    rule(&mut commands, rule_band);
    commands.spawn((
        Text::new("MIRACLES"),
        ui::DisplayFace,
        TextFont {
            font_size: FontSize::Px(ui::theme::SMALL_SIZE + 1.0),
            ..default()
        },
        TextColor(ui::theme::accent().with_alpha(0.9)),
        ChildOf(rule_band),
    ));
    rule(&mut commands, rule_band);

    use crate::miracles::Miracle;
    let cards = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(148),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                overflow: Overflow::scroll_x(),
                ..default()
            },
            ui::Scrollable,
            ScrollPosition::DEFAULT,
            Interaction::default(),
            ChildOf(right),
        ))
        .id();
    for miracle in [
        Miracle::Flourish,
        Miracle::Smite,
        Miracle::Bounty,
        Miracle::Mend,
        Miracle::Quake,
    ] {
        let card = commands
            .spawn((
                MiracleCard(miracle),
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(140),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(6),
                    padding: UiRect::axes(px(8), px(10)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.3)),
                BorderColor::all(ui::theme::panel_border().with_alpha(0.2)),
                ChildOf(cards),
            ))
            .id();
        commands.spawn((
            Text::new(miracle.name().to_uppercase()),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(ui::theme::accent()),
            ChildOf(card),
        ));
        let badge = commands
            .spawn((
                MiracleText { miracle, field: 3 },
                Node {
                    margin: UiRect::axes(px(0), px(4)),
                    ..default()
                },
                ChildOf(card),
            ))
            .id();
        let gold = ui::theme::accent();
        match miracle {
            Miracle::Flourish => flourish_glyph(&mut commands, badge, gold),
            Miracle::Smite => bolt_glyph(&mut commands, badge, gold),
            Miracle::Bounty => berry_glyph(&mut commands, badge, gold),
            Miracle::Mend => mend_glyph(&mut commands, badge, gold),
            Miracle::Quake => quake_glyph(&mut commands, badge, gold),
            // No glyph of its own yet; the badge carries the name.
            Miracle::Avatar => {}
        }
        for field in [0u8, 1, 2] {
            commands.spawn((
                MiracleText { miracle, field },
                ui::dim(""),
                Node {
                    max_width: percent(100),
                    ..default()
                },
                ChildOf(card),
            ));
        }
    }

    // The congregation's heart, and the whole record of faith.
    let lower = commands
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
            ChildOf(right),
        ))
        .id();
    let feelings = ui::card_well(&mut commands, lower, "HOW THEY FEEL ABOUT YOU");
    for row in 0u8..4 {
        let line = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    padding: UiRect::bottom(px(5)),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(ui::theme::text_dim().with_alpha(0.12)),
                ChildOf(feelings),
            ))
            .id();
        commands.spawn((FeelText { row, why: false }, ui::body(""), ChildOf(line)));
        commands.spawn((
            FeelText { row, why: true },
            ui::dim(""),
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            ChildOf(line),
        ));
    }
    let chart_card = ui::card_well(&mut commands, lower, "FAITH OVER TIME");
    commands.entity(chart_card).insert(Node {
        flex_grow: 0.0,
        flex_basis: Val::Auto,
        width: px(320),
        min_height: px(0),
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        padding: UiRect::all(px(10)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(0)),
        overflow: Overflow::clip(),
        ..default()
    });
    commands.spawn((
        FaithChart,
        Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: px(1),
            padding: UiRect::axes(px(2), px(2)),
            ..default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.25)),
        ChildOf(chart_card),
    ));
    commands.spawn((FaithChartSpan, ui::dim(""), ChildOf(chart_card)));

    // ---- The creed band: nature, alignment, domain, prophets. -------------
    let creed = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(CREED_BAND),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                align_items: AlignItems::Stretch,
                ..default()
            },
            ChildOf(page),
        ))
        .id();
    fn creed_card(
        commands: &mut Commands,
        creed: Entity,
        title: &str,
        value: &str,
        caption: &str,
        live: bool,
    ) -> Entity {
        let card = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(12),
                    padding: UiRect::axes(px(14), px(10)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.22)),
                BorderColor::all(ui::theme::text_dim().with_alpha(0.18)),
                ChildOf(creed),
            ))
            .id();
        let badge = commands.spawn((Node::default(), ChildOf(card))).id();
        let words = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    min_width: px(0),
                    ..default()
                },
                ChildOf(card),
            ))
            .id();
        commands.spawn((
            Text::new(title),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(ui::theme::text_dim()),
            ChildOf(words),
        ));
        let mut value_text = commands.spawn((
            Text::new(value),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(ui::theme::accent()),
            ChildOf(words),
        ));
        if live {
            value_text.insert(AlignText { caption: false });
        }
        let mut caption_text = commands.spawn((ui::dim(caption), ChildOf(words)));
        if live {
            caption_text.insert(AlignText { caption: true });
        }
        badge
    }
    let ink = ui::theme::accent().with_alpha(0.75);
    let badge = creed_card(
        &mut commands,
        creed,
        "YOUR NATURE",
        "Unseen",
        "You have no form in the eyes of mortals.",
        false,
    );
    leaf_glyph(&mut commands, badge, ink);
    let badge = creed_card(&mut commands, creed, "ALIGNMENT", "", "", true);
    scales_glyph(&mut commands, badge, ink);
    let badge = creed_card(
        &mut commands,
        creed,
        "DOMAIN",
        "The Living",
        "Life, growth, and the cycles of the natural world.",
        false,
    );
    domain_glyph(&mut commands, badge, ink);
    let badge = creed_card(
        &mut commands,
        creed,
        "PROPHETS",
        "None",
        "No mortals have yet sworn themselves to you alone.",
        false,
    );
    people_glyph(&mut commands, badge, ink);
}

/// Fills THE DEITY while its page is open.
#[allow(clippy::type_complexity)]
pub(crate) fn update_god_panel(
    codex: Res<super::village::Codex>,
    panels: Query<&Visibility, With<GodPanel>>,
    name: Option<Res<crate::villager::DivineName>>,
    legend: Option<Res<crate::villager::belief::Legend>>,
    belief: Option<Res<crate::villager::belief::Belief>>,
    clock: Res<crate::calendar::WorldClock>,
    site: Option<Res<crate::villager::SettlementSite>>,
    settlements: Query<&crate::villager::Settlement>,
    flock: Query<
        (Option<&crate::villager::belief::Faith>, Option<&Witnessed>),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    mut cards: Query<(&MiracleCard, &mut BackgroundColor, &mut BorderColor)>,
    mut presence_fills: Query<&mut Node, With<PresenceFill>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<DeityName>>,
        Query<&mut Text, With<DeitySubline>>,
        Query<&mut Text, With<EpithetBody>>,
        Query<(&StatValue, &mut Text)>,
        Query<(&MiracleText, &mut Text)>,
        Query<(&FeelText, &mut Text)>,
        Query<&mut Text, With<PresenceValue>>,
        Query<(&AlignText, &mut Text)>,
    )>,
) {
    if codex.page != super::village::CodexPage::Deity
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        return;
    }
    use crate::miracles::Miracle;

    let set = |text: &mut Text, fresh: String| {
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    };

    if let Ok(mut text) = texts.p0().single_mut() {
        let fresh = name.as_ref().map_or("...".to_string(), |n| n.0.clone());
        set(&mut text, fresh);
    }
    let epithet = legend.as_ref().and_then(|l| l.epithet);
    if let Ok(mut text) = texts.p1().single_mut() {
        let fresh = match epithet {
            Some(epithet) => format!("named by the people; they call you {epithet}"),
            None => "named by the people; no epithet yet earned".to_string(),
        };
        set(&mut text, fresh);
    }
    if let Ok(mut text) = texts.p2().single_mut() {
        let fresh = match epithet {
            Some(epithet) => format!("{epithet} - earned in the stories they tell."),
            None => "Earn the love and awe of your people to receive an epithet.".to_string(),
        };
        set(&mut text, fresh);
    }

    // The congregation, counted.
    let mut total = 0usize;
    let mut believers = 0usize;
    let mut trust_sum = 0.0f32;
    let mut eyewitnesses = 0usize;
    let mut heard = 0usize;
    for (faith, witnessed) in &flock {
        total += 1;
        if let Some(faith) = faith {
            trust_sum += faith.trust;
            if faith.is_believer() {
                believers += 1;
            }
        }
        if let Some(w) = witnessed {
            if w.total > 0 {
                eyewitnesses += 1;
            } else if w.secondhand > 0 {
                heard += 1;
            }
        }
    }
    let village = site
        .as_ref()
        .and_then(|s| settlements.get(s.settlement).ok())
        .map_or("...".to_string(), |s| s.name.clone());
    let season = {
        let names = ["Spring", "Summer", "Autumn", "Winter"];
        names[((clock.day() / 28) % 4) as usize]
    };
    for (stat, mut text) in &mut texts.p3() {
        let fresh = match stat.0 {
            0 => format!("{believers}"),
            1 => format!("{total}"),
            2 => village.clone(),
            _ => format!("Year {}, {}", clock.day() / 112 + 1, season),
        };
        set(&mut text, fresh);
    }

    // The powers: three born active, two crystallised by legend.
    let unlocked = |miracle: Miracle| match miracle {
        Miracle::Flourish | Miracle::Smite | Miracle::Bounty => true,
        m => legend.as_ref().is_some_and(|l| l.unlocked == Some(m)),
    };
    for (card, mut fill, mut border) in &mut cards {
        let lit = unlocked(card.0);
        fill.0 = if lit {
            ui::theme::title_bg()
        } else {
            Color::BLACK.with_alpha(0.3)
        };
        *border = BorderColor::all(if lit {
            ui::theme::accent().with_alpha(0.55)
        } else {
            ui::theme::panel_border().with_alpha(0.2)
        });
    }
    for (label, mut text) in &mut texts.p4() {
        if label.field == 3 {
            continue;
        }
        let lit = unlocked(label.miracle);
        let fresh = match label.field {
            0 => {
                if lit {
                    format!("{:.0} belief", label.miracle.cost())
                } else {
                    String::new()
                }
            }
            1 => if lit { "Active" } else { "Locked" }.to_string(),
            _ => {
                if lit {
                    String::new()
                } else if label.miracle == Miracle::Mend {
                    "a legend of providence unlocks it".to_string()
                } else {
                    "a legend of dread unlocks it".to_string()
                }
            }
        };
        set(&mut text, fresh);
    }

    // How they feel, in four lines.
    let avg = trust_sum / total.max(1) as f32;
    let mood = match avg {
        a if a > 0.6 => "they are sure of you",
        a if a > 0.45 => "they believe, on the whole",
        a if a > 0.25 => "they waver",
        _ => "they doubt you",
    };
    let lean = legend.as_ref().map_or("", |l| {
        if l.providence > l.dread * 1.4 {
            "the stories they tell are of gifts"
        } else if l.dread > l.providence * 1.4 {
            "the stories they tell are of terror"
        } else {
            "their stories cannot decide what you are"
        }
    });
    let power = belief.as_ref().map_or(0.0, |b| b.available());
    for (feel, mut text) in &mut texts.p5() {
        let fresh = match (feel.row, feel.why) {
            (0, false) => format!("{believers} of {total} believe"),
            (0, true) => mood.to_string(),
            (1, false) => format!("{eyewitnesses} have seen you"),
            (1, true) => "with their own eyes".to_string(),
            (2, false) => format!("{heard} know you only from stories"),
            (2, true) => lean.to_string(),
            (3, false) => format!("{power:.0} belief stands ready"),
            _ => "to spend".to_string(),
        };
        set(&mut text, fresh);
    }

    // Presence: the average trust of every living soul.
    for mut node in &mut presence_fills {
        node.width = percent((avg * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut text) = texts.p6().single_mut() {
        set(&mut text, format!("{:.0}%", avg * 100.0));
    }

    // Alignment, read from the legend's lean.
    let (align_value, align_caption) = legend.as_ref().map_or(
        (
            "Neutral",
            "You are not bound by good or evil, only purpose.",
        ),
        |l| {
            if l.providence > l.dread * 1.4 {
                ("Benevolent", "You are known by your gifts.")
            } else if l.dread > l.providence * 1.4 {
                ("Terrible", "You are known by your storms.")
            } else {
                (
                    "Neutral",
                    "You are not bound by good or evil, only purpose.",
                )
            }
        },
    );
    for (align, mut text) in &mut texts.p7() {
        let fresh = if align.caption {
            align_caption.to_string()
        } else {
            align_value.to_string()
        };
        set(&mut text, fresh);
    }
}

/// Redraws the faith curve when the record grows (or the page opens, or
/// the well is first laid out): the whole biography bucketed and drawn as
/// one gold line, oldest left, a bright point marking today.
pub(crate) fn update_faith_chart(
    mut commands: Commands,
    codex: Res<super::village::Codex>,
    panels: Query<&Visibility, With<GodPanel>>,
    history: Res<crate::villager::belief::FaithHistory>,
    charts: Query<(Entity, &ComputedNode), With<FaithChart>>,
    mut spans: Query<&mut Text, With<FaithChartSpan>>,
    mut drawn: Local<Option<(usize, Vec2)>>,
) {
    if codex.page != super::village::CodexPage::Deity
        || !panels.iter().any(|v| *v != Visibility::Hidden)
    {
        *drawn = None;
        return;
    }
    let Ok((chart, computed)) = charts.single() else {
        return;
    };
    // The line is plotted in the well's own pixels, so the well must have
    // been laid out; a zero-sized first frame just waits its turn.
    let size = computed.size() * computed.inverse_scale_factor();
    if size.x < 12.0 || size.y < 12.0 {
        return;
    }
    if *drawn == Some((history.samples.len(), size)) {
        return;
    }
    *drawn = Some((history.samples.len(), size));
    commands.entity(chart).despawn_related::<Children>();

    const BUCKETS: usize = 48;
    let samples = &history.samples;
    if samples.len() < 2 {
        commands.spawn((
            ui::dim("the record begins"),
            Node {
                margin: UiRect::all(px(8)),
                ..default()
            },
            ChildOf(chart),
        ));
    } else {
        // Bucket-average the whole record into the chart's width, then
        // join the points with thin rotated strokes - the same trick the
        // sigils use for their turned bars.
        let buckets = BUCKETS.min(samples.len());
        let mut heights = Vec::with_capacity(buckets);
        for bucket in 0..buckets {
            let from = bucket * samples.len() / buckets;
            let to = ((bucket + 1) * samples.len() / buckets).max(from + 1);
            let slice = &samples[from..to];
            heights.push(slice.iter().sum::<f32>() / slice.len() as f32);
        }
        let top = heights.iter().cloned().fold(1.0f32, f32::max);
        let inner = Vec2::new(size.x - 12.0, size.y - 14.0);
        let point = |i: usize, value: f32| {
            Vec2::new(
                6.0 + i as f32 / (buckets - 1) as f32 * inner.x,
                7.0 + (1.0 - value / top) * inner.y,
            )
        };
        for (i, pair) in heights.windows(2).enumerate() {
            let a = point(i, pair[0]);
            let b = point(i + 1, pair[1]);
            let mid = (a + b) * 0.5;
            let length = a.distance(b).max(1.0);
            let angle = (b.y - a.y).atan2(b.x - a.x);
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(mid.x - length * 0.5),
                    top: px(mid.y - 1.0),
                    width: px(length),
                    height: px(2),
                    border_radius: BorderRadius::all(px(2)),
                    ..default()
                },
                UiTransform::from_rotation(Rot2::radians(angle)),
                BackgroundColor(ui::theme::accent().with_alpha(0.9)),
                ChildOf(chart),
            ));
        }
        // Today, marked: the line's living end.
        let now = point(buckets - 1, *heights.last().unwrap());
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(now.x - 2.5),
                top: px(now.y - 2.5),
                width: px(5),
                height: px(5),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BackgroundColor(ui::theme::accent()),
            ChildOf(chart),
        ));
    }
    if let Ok(mut text) = spans.single_mut() {
        let days = history.samples.len().max(1) as u32;
        let fresh = if days <= 112 {
            format!(
                "the first year, day by day - faith stands at {:.0}",
                history.samples.last().copied().unwrap_or(0.0)
            )
        } else {
            format!(
                "year 1 through year {}, day by day - faith stands at {:.0}",
                days / 112 + 1,
                history.samples.last().copied().unwrap_or(0.0)
            )
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}
