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

/// A step of the build to set out, by its place in the list.
#[derive(Component, Clone, Copy)]
struct StageButton(usize);

/// The buttons that change how many steps there are.
#[derive(Component, Clone, Copy)]
struct StageDeedButton(crate::builder::StageDeed);

/// The row itself, rebuilt whenever the number of steps changes.
#[derive(Component)]
struct StageBar;

/// Where the file work lives on the rail: builder parents its save and
/// load drawers here instead of crowding the shelf.
#[derive(Resource)]
pub struct FileHome(pub Entity);

/// The button on the top bar that lifts the roof off.
/// The gear at the rail's foot, and the settings panel it opens.
#[derive(Component)]
struct SettingsButton;

#[derive(Component)]
struct SettingsPanel;

pub struct RailPlugin;

impl Plugin for RailPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, raise_rail).add_systems(
            Update,
            (work_buttons, work_mode_bar, work_settings, work_stage_bar, hang_the_stage_bar),
        );
    }
}

pub fn raise_rail(mut commands: Commands, fonts: Res<Fonts>, palette: Res<Palette>) {
    // The mode bar, top centre, the way the big programs wear it.
    //
    // Centred by a full-width row rather than by arithmetic. It used to be three
    // hundred and twenty pixels wide with a hundred and sixty of negative margin
    // pulling it back over the middle - true only while the buttons happened to
    // add up to that, and the fourth one spilled straight out of the panel it
    // was supposed to be inside. A hard-coded width is a measurement of the
    // contents kept somewhere the contents cannot reach.
    let centring = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();
    let bar = commands
        .spawn((
            Node {
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
            ChildOf(centring),
        ))
        .id();
    for (mode, label) in [
        (crate::gizmo::ToolMode::Normal, "NORMAL"),
        (crate::gizmo::ToolMode::Move, "MOVE"),
        (crate::gizmo::ToolMode::Resize, "RESIZE"),
        (crate::gizmo::ToolMode::Paint, "PAINT"),
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

    // The build steps, on their own row under the modes. Empty here and filled
    // by `hang_the_stage_bar`, because how many steps there are is a property of
    // the work on the bench rather than of the bench itself: a work opens with
    // as many as its file declares, and a maker adds and drops them while
    // working.
    commands.spawn((
        StageBar,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));

    let rail = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                // The shelf's own width. The two stand either side of the same
                // stage and a bench with mismatched margins reads as a bench
                // that was assembled rather than drawn.
                width: Val::Px(212.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
            crate::look::Scrollable,
            bevy::ui::ScrollPosition::default(),
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
    // The settings panel: hidden until the gear is pressed, carrying
    // the keybinds so the rail itself stays short.
    let panel = commands
        .spawn((
            SettingsPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(200.0),
                bottom: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::accent(&palette).with_alpha(0.5)),
            Visibility::Hidden,
            GlobalZIndex(40),
            crate::look::Scrollable,
            bevy::ui::ScrollPosition::default(),
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
        ChildOf(panel),
    ));
    for (cap, tale) in [
        ("click", "place, or pick back up"),
        ("X / del", "remove at the cursor"),
        ("cmd Z / Y", "undo and redo"),
        (
            "cmd C / V",
            "copy what you point at,
paste it into the hand",
        ),
        ("R", "turn a quarter"),
        ("SHIFT R", "turn the whole work"),
        ("T / SHIFT T", "lean it, and lean it back"),
        (
            "M",
            "mirror it - the far half
of a gable, the other hand
of an L",
        ),
        ("Q / E", "lift and lower"),
        ("[ ]", "repaint through the ramps"),
        ("- =", "darker and brighter"),
        (
            "PAINT",
            "colour what is already
standing: click a part,
shift-click the lot,
\\ empties the brush",
        ),
        ("shift-click", "choose several; right-
click to group them"),
        ("esc / del", "empty the hand"),
        (
            "del",
            "with a part chosen and
an empty hand: bury it",
        ),
        (
            "1 - 6",
            "front, right, back, left,\noverhead, and the perch",
        ),
        ("shift", "fine snap, 1 unit"),
        (
            "G",
            "cycle the grid interval:
1, 2, 4, 8, 16 units",
        ),
        ("F", "face snap on and off"),
        (
            "H",
            "the cutaway: whole, roof
off, walls down as well",
        ),
        ("tab", "normal, move, resize;\nclick selects, drag a handle"),
        ("D", "type exact dimensions\nof the resize selection"),
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
                ChildOf(panel),
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
    // The gear: a drawn sliders glyph, since the fonts keep no gear.
    let gear = commands
        .spawn((
            SettingsButton,
            Interaction::default(),
            Node {
                width: Val::Px(34.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.18)),
            BorderColor::all(theme::panel_border(&palette)),
            ChildOf(foot),
        ))
        .id();
    for offset in [-5.0f32, 3.0, -2.0] {
        let bar = commands
            .spawn((
                Node {
                    width: Val::Px(18.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(theme::text_dim(&palette).with_alpha(0.8)),
                ChildOf(gear),
            ))
            .id();
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(9.0 + offset),
                top: Val::Px(-1.5),
                width: Val::Px(5.0),
                height: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(theme::accent(&palette)),
            ChildOf(bar),
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

/// One button of the step row.
fn stage_face(
    commands: &mut Commands,
    fonts: &Fonts,
    palette: &Palette,
    parent: Entity,
    label: String,
    wide: f32,
) -> Entity {
    let button = commands
        .spawn((
            Interaction::default(),
            Node {
                padding: UiRect::axes(Val::Px(wide), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.30)),
            BorderColor::all(theme::panel_border(palette)),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(label),
        TextFont {
            font: fonts.display.clone().into(),
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(theme::accent(palette)),
        ChildOf(button),
    ));
    button
}

/// Rebuilds the row when the number of steps changes.
///
/// A work opens with as many steps as its file says, and a maker adds and drops
/// them while working, so the row cannot be built once at startup and left.
fn hang_the_stage_bar(
    mut commands: Commands,
    fonts: Res<Fonts>,
    palette: Res<Palette>,
    stages: Res<crate::builder::Stages>,
    bars: Query<Entity, With<StageBar>>,
    mut hung: Local<usize>,
) {
    let count = stages.count();
    if count == *hung && !stages.is_changed() {
        return;
    }
    if count == *hung {
        return;
    }
    *hung = count;
    let Some(row) = bars.iter().next() else {
        return;
    };
    commands.entity(row).despawn_related::<Children>();
    // The same panel the modes wear, on the floor rather than the ceiling: one
    // border along the edge it does not touch, and the buttons inside it. A row
    // of bare buttons on the background was the odd one out on this screen.
    let bar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect {
                    left: Val::Px(1.0),
                    right: Val::Px(1.0),
                    top: Val::Px(1.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border(&palette)),
            ChildOf(row),
        ))
        .id();

    for step in 0..count {
        let button = stage_face(
            &mut commands,
            &fonts,
            &palette,
            bar,
            format!("STAGE {}", step + 1),
            14.0,
        );
        commands.entity(button).insert(StageButton(step));
    }
    // Adding and dropping sit apart from the steps, so a miss lands on nothing
    // rather than on a step being deleted.
    for (deed, label) in [
        (crate::builder::StageDeed::AddCopying, "+ COPY"),
        (crate::builder::StageDeed::AddBare, "+ BARE"),
        (crate::builder::StageDeed::Drop, "-"),
    ] {
        let button = stage_face(
            &mut commands,
            &fonts,
            &palette,
            bar,
            label.to_string(),
            10.0,
        );
        commands.entity(button).insert((
            StageDeedButton(deed),
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                // A gap before the first of them, so a miss lands on nothing
                // rather than on a step being deleted.
                margin: UiRect::left(Val::Px(if label == "+ COPY" { 12.0 } else { 0.0 })),
                ..default()
            },
        ));
    }
}

/// Step presses set out that step; the standing step wears the gold.
fn work_stage_bar(
    palette: Res<Palette>,
    stages: Res<crate::builder::Stages>,
    mut wish: ResMut<crate::builder::StageWish>,
    deeds: Query<(&Interaction, &StageDeedButton)>,
    mut buttons: Query<(&Interaction, &StageButton, &mut BorderColor, &mut BackgroundColor)>,
) {
    for (interaction, button) in &deeds {
        if *interaction == Interaction::Pressed && wish.0.is_none() {
            wish.0 = Some(button.0);
        }
    }
    for (interaction, button, _, _) in &buttons {
        if *interaction == Interaction::Pressed
            && button.0 != stages.showing()
            && wish.0.is_none()
        {
            wish.0 = Some(crate::builder::StageDeed::Show(button.0));
        }
    }
    for (_, button, mut border, mut fill) in &mut buttons {
        let standing = button.0 == stages.showing();
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
        // Every mode button says the same thing the same way: the fill means
        // STANDING and nothing else. PAINT used to wear the brush's own colour,
        // which made it look chosen while another tool was in hand - Brett:
        // "Paint wasnt active, but the button isnt following the other buttons
        // conventions". The brush has a square of its own at the head of the
        // palette, which is a better place for it anyway: it is only wanted
        // while painting, and there it can be the size of a colour rather than
        // the size of a word.
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

/// The gear opens and closes the settings panel.
fn work_settings(
    gears: Query<&Interaction, (Changed<Interaction>, With<SettingsButton>)>,
    mut panels: Query<&mut Visibility, With<SettingsPanel>>,
) {
    let pressed = gears
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed);
    if !pressed {
        return;
    }
    for mut visibility in &mut panels {
        *visibility = if *visibility == Visibility::Hidden {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

