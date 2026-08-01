//! The bench rail: the strip of benches down the left edge, in the codex's
//! own dress. New tools become new benches here.

use bevy::prelude::*;
use bevy::text::FontSize;

use crate::Bench;
use crate::look::{Fonts, Palette, theme};

/// A button that walks the maker to a bench.
#[derive(Component)]
struct BenchButton(Bench);

/// A button on the top bar that sets the tool mode.
#[derive(Component)]
struct ModeButton(crate::gizmo::ToolMode);

/// Where the file work lives on the rail: builder parents its save and
/// load drawers here instead of crowding the shelf.
#[derive(Resource)]
pub struct FileHome(pub Entity);

pub struct RailPlugin;

impl Plugin for RailPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, raise_rail)
            .add_systems(Update, (work_buttons, work_mode_bar));
    }
}

pub fn raise_rail(mut commands: Commands, fonts: Res<Fonts>, palette: Res<Palette>) {
    // The mode bar, top centre, the way the big programs wear it.
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Px(0.0),
                margin: UiRect::left(Val::Px(-160.0)),
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect {
                    left: Val::Px(1.0),
                    right: Val::Px(1.0),
                    bottom: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
        ))
        .id();
    for (mode, label) in [
        (crate::gizmo::ToolMode::Normal, "NORMAL"),
        (crate::gizmo::ToolMode::Move, "MOVE"),
        (crate::gizmo::ToolMode::Resize, "RESIZE"),
    ] {
        let button = commands
            .spawn((
                ModeButton(mode),
                Interaction::default(),
                Node {
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(5.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.18)),
                BorderColor::all(theme::panel_border(&palette)),
                ChildOf(bar),
            ))
            .id();
        commands.spawn((
            Text::new(label),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(theme::accent(&palette)),
            ChildOf(button),
        ));
    }

    let rail = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(190.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
        ))
        .id();

    commands.spawn((
        Text::new("THE ATELIER"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(theme::accent(&palette)),
        ChildOf(rail),
    ));
    commands.spawn((
        Text::new(concat!(
            "the maker's own bench - v",
            env!("CARGO_PKG_VERSION")
        )),
        TextFont {
            font: fonts.text.clone().into(),
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(theme::text_dim(&palette)),
        Node {
            margin: UiRect::bottom(Val::Px(14.0)),
            ..default()
        },
        ChildOf(rail),
    ));

    for (bench, label, tale) in [
        (Bench::Builder, "THE BUILDER", "boxes, ramps and widgets"),
        (Bench::Rig, "THE RIG", "the body and its clips"),
    ] {
        let button = commands
            .spawn((
                BenchButton(bench),
                Interaction::default(),
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.18)),
                BorderColor::all(theme::panel_border(&palette)),
                ChildOf(rail),
            ))
            .id();
        commands.spawn((
            Text::new(label),
            TextFont {
                font: fonts.display.clone().into(),
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(theme::accent(&palette)),
            ChildOf(button),
        ));
        commands.spawn((
            Text::new(tale),
            TextFont {
                font: fonts.text.clone().into(),
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(theme::text_dim(&palette)),
            ChildOf(button),
        ));
    }

    // The file work's home, between the benches and the keybinds.
    let file_home = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            },
            ChildOf(rail),
        ))
        .id();
    commands.insert_resource(FileHome(file_home));

    // The footer: how the bench speaks to the world.
    let foot = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                row_gap: Val::Px(3.0),
                ..default()
            },
            ChildOf(rail),
        ))
        .id();
    commands.spawn((
        Text::new("KEYBINDS"),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(theme::accent(&palette)),
        Node {
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        },
        ChildOf(foot),
    ));
    for (cap, tale) in [
        ("click", "place, or pick back up"),
        ("X", "remove at the cursor"),
        ("R", "turn a quarter"),
        ("T", "tilt a roof panel"),
        ("Q / E", "lift and lower"),
        ("[ ]", "repaint through the ramps"),
        ("- =", "darker and brighter"),
        ("esc", "empty the hand"),
        (
            "1 - 6",
            "front, right, back, left,\noverhead, and the perch",
        ),
        ("shift", "fine snap, 1/16m"),
        ("F", "face snap on and off"),
        ("tab", "normal, move, resize;\nclick selects, drag a handle"),
        ("RMB drag", "swing the camera"),
        ("MMB drag", "pull the bench along"),
        ("wheel", "draw near, pull away"),
        ("WASD", "glide over the bench"),
    ] {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                },
                ChildOf(foot),
            ))
            .id();
        let chip = commands
            .spawn((
                Node {
                    min_width: Val::Px(52.0),
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.35)),
                BorderColor::all(theme::panel_border(&palette)),
                ChildOf(row),
            ))
            .id();
        // The machine's own face on the caps, the way manuals do it.
        commands.spawn((
            Text::new(cap),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(theme::accent(&palette).with_alpha(0.95)),
            ChildOf(chip),
        ));
        commands.spawn((
            Text::new(tale),
            TextFont {
                font: fonts.text.clone().into(),
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(theme::text_dim(&palette).with_alpha(0.85)),
            ChildOf(row),
        ));
    }
    commands.spawn((
        Text::new("what you save here, the god carries into the world by hand."),
        TextFont {
            font: fonts.text.clone().into(),
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(theme::text_dim(&palette).with_alpha(0.6)),
        Node {
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        },
        ChildOf(foot),
    ));
}

/// Mode presses set the tool; the standing mode wears the gold.
fn work_mode_bar(
    palette: Res<Palette>,
    mut mode: ResMut<crate::gizmo::ToolMode>,
    mut buttons: Query<(
        &Interaction,
        &ModeButton,
        &mut BorderColor,
        &mut BackgroundColor,
    )>,
) {
    for (interaction, button, _, _) in &buttons {
        if *interaction == Interaction::Pressed && *mode != button.0 {
            *mode = button.0;
        }
    }
    for (_, button, mut border, mut fill) in &mut buttons {
        let standing = *mode == button.0;
        let dress = BorderColor::all(if standing {
            theme::accent(&palette)
        } else {
            theme::panel_border(&palette)
        });
        if *border != dress {
            *border = dress;
        }
        let wanted = BackgroundColor(if standing {
            Color::srgb(0.075, 0.082, 0.102)
        } else {
            Color::BLACK.with_alpha(0.18)
        });
        if fill.0 != wanted.0 {
            *fill = wanted;
        }
    }
}

/// Bench presses walk the maker over; the standing bench wears the gold.
fn work_buttons(
    palette: Res<Palette>,
    mut bench: ResMut<Bench>,
    mut buttons: Query<(
        &Interaction,
        &BenchButton,
        &mut BorderColor,
        &mut BackgroundColor,
    )>,
) {
    for (interaction, button, _, _) in &buttons {
        if *interaction == Interaction::Pressed && *bench != button.0 {
            *bench = button.0;
        }
    }
    for (_, button, mut border, mut fill) in &mut buttons {
        let standing = *bench == button.0;
        *border = BorderColor::all(if standing {
            theme::accent(&palette)
        } else {
            theme::panel_border(&palette)
        });
        *fill = BackgroundColor(if standing {
            Color::srgb(0.075, 0.082, 0.102)
        } else {
            Color::BLACK.with_alpha(0.18)
        });
    }
}
