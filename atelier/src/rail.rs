//! The bench rail: the strip of benches down the left edge, in the codex's
//! own dress. New tools become new benches here.

use bevy::prelude::*;
use bevy::text::FontSize;

use crate::Bench;
use crate::look::{Fonts, Palette, theme};

/// A button that walks the maker to a bench.
#[derive(Component)]
struct BenchButton(Bench);

pub struct RailPlugin;

impl Plugin for RailPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, raise_rail)
            .add_systems(Update, work_buttons);
    }
}

fn raise_rail(mut commands: Commands, fonts: Res<Fonts>, palette: Res<Palette>) {
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
        Text::new("the maker's own bench"),
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

    // The footer: how the bench speaks to the world.
    let foot = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                row_gap: Val::Px(4.0),
                ..default()
            },
            ChildOf(rail),
        ))
        .id();
    commands.spawn((
        Text::new(
            "right drag swings, middle drag pulls, \
             the wheel draws near.\n\nwhat you save here, the god \
             carries into the world by hand.",
        ),
        TextFont {
            font: fonts.text.clone().into(),
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(theme::text_dim(&palette).with_alpha(0.7)),
        ChildOf(foot),
    ));
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
