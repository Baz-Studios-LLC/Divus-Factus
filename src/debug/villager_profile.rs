//! Polished contextual villager profile and action hub.
//!
//! Opened by Shift + right-clicking on any live villager in the 3D diorama.
//! Displays 3D paperdoll, live vitals, social relationships, skills ledger,
//! personal chronicle, and actionable divine commands (Follow, Center, Retrain, Bless).

use super::{person_phrase, state_phrase};
use crate::camera::{CameraRig, FollowStyle, FollowTarget, GodCamera};
use crate::creature::Corpse;
use crate::hand::DivineHand;
use crate::ui;
use crate::ui::PointerContext;
use crate::villager::belief::Faith;
use crate::villager::home::Home;
use crate::villager::regard::Regard;
use crate::villager::speech::RecentlySaid;
use crate::villager::traits::Traits;
use crate::villager::work::{Jobless, Skills, Vocation};
use crate::villager::{
    Activity, Chronicle, Morale, Needs, Parentage, Person, Spouse, Stirrings, Villager,
};
use crate::witness::Reaction;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[inline]
fn px(v: f32) -> Val {
    Val::Px(v)
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum ProfileTab {
    #[default]
    Overview,
    Skills,
    Bonds,
    Inner,
    Chronicle,
}

#[derive(Resource, Default)]
pub(crate) struct VillagerProfile {
    pub(crate) open: bool,
    pub(crate) tab: ProfileTab,
}

#[derive(Component)]
pub(crate) struct VillagerProfileRoot;

#[derive(Component, Clone, Copy)]
pub(crate) enum ProfileText {
    Title,
    Subtitle,
    Rail,
}

#[derive(Component)]
pub(crate) struct ProfileTabButton(pub(crate) ProfileTab);

#[derive(Component)]
pub(crate) struct ProfileTabText(pub(crate) ProfileTab);

#[derive(Component)]
pub(crate) struct ProfilePageNode(pub(crate) ProfileTab);

#[derive(Component)]
pub(crate) struct CloseVillagerProfile;

#[derive(Component)]
pub(crate) struct FollowVillagerAction;

#[derive(Component)]
pub(crate) struct CenterVillagerAction;

#[derive(Component)]
pub(crate) struct BlessVillagerAction;

#[derive(Component)]
pub(crate) struct AssignVocationAction(pub(crate) Vocation);

#[derive(Component)]
pub(crate) struct ProfileContentWell(pub(crate) ProfileTab);

pub(crate) fn spawn_villager_profile(
    mut commands: Commands,
    portrait: Res<super::people::PaperdollTarget>,
) {
    let root = commands
        .spawn((
            VillagerProfileRoot,
            ui::Panel,
            Node {
                position_type: PositionType::Absolute,
                right: px(16.0),
                top: px(16.0),
                width: px(560.0),
                height: Val::Vh(82.0),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                padding: px(16.0).into(),
                row_gap: px(12.0),
                border: UiRect::all(px(1.5)),
                ..default()
            },
            BackgroundColor(ui::theme::panel_bg()),
            BorderColor::all(ui::theme::panel_border()),
            GlobalZIndex(40),
            Interaction::default(),
            Visibility::Hidden,
        ))
        .id();

    // --- Header ---
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: px(0.0),
                align_items: AlignItems::Stretch,
                column_gap: px(12.0),
                ..default()
            },
            ChildOf(root),
        ))
        .id();

    // 3D Portrait Card
    let portrait_card = commands
        .spawn((
            Node {
                width: px(116.0),
                height: px(146.0),
                flex_shrink: 0.0,
                padding: px(6.0).into(),
                border: UiRect::all(px(1.0)),
                ..default()
            },
            BackgroundColor(ui::theme::card_bg()),
            BorderColor::all(ui::theme::card_border()),
            Interaction::default(),
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        ImageNode::new(portrait.0.clone()),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ChildOf(portrait_card),
    ));

    // Identity + Actions
    let identity = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: px(0.0),
                ..default()
            },
            ChildOf(header),
        ))
        .id();

    commands.spawn((
        ProfileText::Title,
        ui::DisplayFace,
        Text::new("VILLAGER"),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(ui::theme::accent()),
        ChildOf(identity),
    ));

    commands.spawn((
        ProfileText::Subtitle,
        ui::SerifFace,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(ui::theme::text_dim()),
        Node {
            margin: UiRect::top(px(4.0)),
            ..default()
        },
        ChildOf(identity),
    ));

    commands.spawn((
        ProfileText::Rail,
        ui::SerifFace,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(ui::theme::text_dim().with_alpha(0.85)),
        Node {
            width: Val::Percent(100.0),
            margin: UiRect::top(px(6.0)),
            ..default()
        },
        ChildOf(identity),
    ));

    // Action buttons in header
    let actions_row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(6.0),
                margin: UiRect::top(px(10.0)),
                ..default()
            },
            ChildOf(identity),
        ))
        .id();

    for (action_entity, label) in [
        (commands.spawn((FollowVillagerAction,)).id(), "FOLLOW"),
        (commands.spawn((CenterVillagerAction,)).id(), "CENTER"),
        (commands.spawn((BlessVillagerAction,)).id(), "BLESS"),
    ] {
        commands.entity(action_entity).insert((
            Button,
            Interaction::default(),
            ui::UiButton,
            Node {
                padding: UiRect::axes(px(12.0), px(6.0)),
                border: UiRect::all(px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ui::theme::title_bg().with_alpha(0.6)),
            BorderColor::all(ui::theme::panel_border().with_alpha(0.5)),
            ChildOf(actions_row),
        ));
        commands.spawn((
            ui::DisplayFace,
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(ui::theme::accent()),
            ChildOf(action_entity),
        ));
    }

    // Close button
    let close_btn = commands
        .spawn((
            CloseVillagerProfile,
            Button,
            Interaction::default(),
            ui::UiButton,
            Node {
                width: px(30.0),
                height: px(30.0),
                border: UiRect::all(px(1.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ui::theme::title_bg().with_alpha(0.6)),
            BorderColor::all(ui::theme::panel_border().with_alpha(0.5)),
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        ui::DisplayFace,
        Text::new("X"),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(ui::theme::accent()),
        ChildOf(close_btn),
    ));

    // Divider
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: px(1.0),
            ..default()
        },
        BackgroundColor(ui::theme::panel_border().with_alpha(0.4)),
        ChildOf(root),
    ));

    // --- Tab Strip ---
    let tab_strip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: px(4.0),
                margin: UiRect::bottom(px(-1.0)),
                ..default()
            },
            ChildOf(root),
        ))
        .id();

    let tab_defs = [
        (ProfileTab::Overview, "OVERVIEW"),
        (ProfileTab::Skills, "SKILLS"),
        (ProfileTab::Bonds, "BONDS"),
        (ProfileTab::Inner, "INNER"),
        (ProfileTab::Chronicle, "CHRONICLE"),
    ];

    for (tab_variant, label) in &tab_defs {
        let is_active = *tab_variant == ProfileTab::Overview;
        let tab_btn = commands
            .spawn((
                ProfileTabButton(*tab_variant),
                Button,
                Interaction::default(),
                ui::UiButton,
                ui::KeepFace,
                Node {
                    flex_grow: 1.0,
                    padding: UiRect::axes(px(6.0), px(7.0)),
                    border: UiRect {
                        top: if is_active { px(3.0) } else { px(1.0) },
                        left: px(1.0),
                        right: px(1.0),
                        bottom: px(1.0),
                    },
                    border_radius: BorderRadius {
                        top_left: px(3.0),
                        top_right: px(3.0),
                        bottom_left: px(0.0),
                        bottom_right: px(0.0),
                    },
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if is_active {
                    ui::theme::card_bg()
                } else {
                    Color::BLACK.with_alpha(0.32)
                }),
                BorderColor {
                    top: if is_active {
                        ui::theme::accent()
                    } else {
                        Color::WHITE.with_alpha(0.06)
                    },
                    left: if is_active {
                        ui::theme::card_border()
                    } else {
                        ui::theme::panel_border().with_alpha(0.25)
                    },
                    right: if is_active {
                        ui::theme::card_border()
                    } else {
                        ui::theme::panel_border().with_alpha(0.25)
                    },
                    bottom: if is_active {
                        ui::theme::card_bg()
                    } else {
                        ui::theme::card_border()
                    },
                },
                ChildOf(tab_strip),
            ))
            .id();
        commands.spawn((
            ProfileTabText(*tab_variant),
            ui::DisplayFace,
            Text::new(*label),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(if is_active {
                ui::theme::accent()
            } else {
                ui::theme::text_dim().with_alpha(0.65)
            }),
            ChildOf(tab_btn),
        ));
    }

    // --- Page Container ---
    let page_container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(root),
        ))
        .id();

    for (tab_variant, _) in &tab_defs {
        let page_node = commands
            .spawn((
                ProfilePageNode(*tab_variant),
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: px(0.0),
                    flex_direction: FlexDirection::Column,
                    display: if *tab_variant == ProfileTab::Overview {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
                ChildOf(page_container),
            ))
            .id();

        let card = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: px(0.0),
                    padding: px(14.0).into(),
                    overflow: Overflow::scroll_y(),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10.0),
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius {
                        top_left: px(0.0),
                        top_right: px(3.0),
                        bottom_left: px(3.0),
                        bottom_right: px(3.0),
                    },
                    ..default()
                },
                BackgroundColor(ui::theme::card_bg()),
                BorderColor::all(ui::theme::card_border()),
                crate::ui::Scrollable,
                bevy::ui::ScrollPosition::default(),
                Interaction::default(),
                ChildOf(page_node),
            ))
            .id();

        // If SKILLS tab, insert the interactive vocation reassignment grid
        if *tab_variant == ProfileTab::Skills {
            commands.spawn((
                ui::DisplayFace,
                Text::new("ASSIGN CALLING"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(ui::theme::text_dim()),
                ChildOf(card),
            ));

            let chips_grid = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(6.0),
                        row_gap: px(6.0),
                        margin: UiRect::bottom(px(4.0)),
                        ..default()
                    },
                    ChildOf(card),
                ))
                .id();

            for vocation in [
                Vocation::Gatherer,
                Vocation::Fisher,
                Vocation::Hunter,
                Vocation::Farmer,
                Vocation::Forester,
                Vocation::Miner,
                Vocation::Builder,
                Vocation::Cook,
                Vocation::Healer,
                Vocation::Priest,
                Vocation::Explorer,
                Vocation::Guard,
            ] {
                let voc_btn = commands
                    .spawn((
                        AssignVocationAction(vocation),
                        Button,
                        Interaction::default(),
                        ui::UiButton,
                        Node {
                            padding: UiRect::axes(px(8.0), px(4.0)),
                            border: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.5)),
                        BorderColor::all(ui::theme::panel_border().with_alpha(0.4)),
                        ChildOf(chips_grid),
                    ))
                    .id();
                commands.spawn((
                    ui::DisplayFace,
                    Text::new(vocation.title()),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(ui::theme::accent()),
                    ChildOf(voc_btn),
                ));
            }

            commands.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: px(1.0),
                    margin: UiRect::bottom(px(4.0)),
                    ..default()
                },
                BackgroundColor(ui::theme::panel_border().with_alpha(0.3)),
                ChildOf(card),
            ));
        }

        commands.spawn((
            ProfileContentWell(*tab_variant),
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(10.0),
                ..default()
            },
            ChildOf(card),
        ));
    }
}

pub(crate) fn open_villager_profile(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    pointer: Res<PointerContext>,
    hand: Res<DivineHand>,
    villagers: Query<(), (With<Villager>, Without<Corpse>)>,
    mut profile: ResMut<VillagerProfile>,
    mut selected: ResMut<super::people::SelectedPerson>,
    mut press_at: Local<Option<Vec2>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if !shift_held {
        press_at.take();
        return;
    }

    if buttons.just_pressed(MouseButton::Right) {
        *press_at = window.cursor_position();
    }
    if !buttons.just_released(MouseButton::Right) {
        return;
    }

    let (Some(pressed), Some(released)) = (press_at.take(), window.cursor_position()) else {
        return;
    };
    if pointer.over_ui || pressed.distance(released) > 6.0 {
        return;
    }
    let Some(villager) = hand.hovered.filter(|entity| villagers.get(*entity).is_ok()) else {
        return;
    };

    selected.0 = Some(villager);
    profile.open = true;
    profile.tab = ProfileTab::Overview;
}

pub(crate) fn handle_profile_actions(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut profile: ResMut<VillagerProfile>,
    selected: Res<super::people::SelectedPerson>,
    mut follow: ResMut<FollowTarget>,
    mut cameras: Query<&mut CameraRig, With<GodCamera>>,
    transforms: Query<&Transform, With<Villager>>,
    mut needs_query: Query<(&mut Needs, &mut Morale, Option<&mut Faith>), With<Villager>>,
    mut chronicle_query: Query<&mut Chronicle, With<Villager>>,
    clock: Option<Res<crate::calendar::WorldClock>>,
    tab_btns: Query<(&Interaction, &ProfileTabButton), Changed<Interaction>>,
    close_btns: Query<&Interaction, (With<CloseVillagerProfile>, Changed<Interaction>)>,
    follow_btns: Query<&Interaction, (With<FollowVillagerAction>, Changed<Interaction>)>,
    center_btns: Query<&Interaction, (With<CenterVillagerAction>, Changed<Interaction>)>,
    bless_btns: Query<&Interaction, (With<BlessVillagerAction>, Changed<Interaction>)>,
    vocation_btns: Query<(&Interaction, &AssignVocationAction), Changed<Interaction>>,
) {
    // Dismiss on Escape
    if keys.just_pressed(KeyCode::Escape) {
        profile.open = false;
        return;
    }

    // Tab Switch Action
    for (interaction, tab_btn) in &tab_btns {
        if *interaction == Interaction::Pressed {
            profile.tab = tab_btn.0;
        }
    }

    // Close Button Action
    for interaction in &close_btns {
        if *interaction == Interaction::Pressed {
            profile.open = false;
            return;
        }
    }

    let Some(entity) = selected.0 else {
        return;
    };

    // Follow Action
    for interaction in &follow_btns {
        if *interaction == Interaction::Pressed {
            if follow.entity == Some(entity) {
                // Cycle follow style or release
                match follow.style {
                    FollowStyle::Overhead => follow.style = FollowStyle::Eyes,
                    FollowStyle::Eyes => follow.entity = None,
                }
            } else {
                follow.entity = Some(entity);
                follow.style = FollowStyle::Overhead;
            }
        }
    }

    // Center Action
    for interaction in &center_btns {
        if *interaction == Interaction::Pressed {
            if let Ok(transform) = transforms.get(entity) {
                if let Ok(mut rig) = cameras.single_mut() {
                    rig.target_focus = transform.translation;
                }
            }
        }
    }

    // Bless Action (Divine Comfort)
    for interaction in &bless_btns {
        if *interaction == Interaction::Pressed {
            if let Ok((mut needs, mut morale, mut faith)) = needs_query.get_mut(entity) {
                needs.hunger = (needs.hunger - 0.5).max(0.0);
                needs.rest = (needs.rest - 0.5).max(0.0);
                morale.spirits = (morale.spirits + 0.4).min(1.0);
                if let Some(ref mut faith) = faith {
                    faith.trust = (faith.trust + 0.25).min(1.0);
                }
                let day = clock.as_ref().map_or(1, |c| c.day());
                if let Ok(mut chronicle) = chronicle_query.get_mut(entity) {
                    chronicle.record(day, "felt the comforting grace of the divine hand");
                }
            }
        }
    }

    // Reassign Vocation Action
    for (interaction, action) in &vocation_btns {
        if *interaction == Interaction::Pressed {
            commands.entity(entity).remove::<Jobless>();
            commands.entity(entity).insert(action.0);
            let day = clock.as_ref().map_or(1, |c| c.day());
            if let Ok(mut chronicle) = chronicle_query.get_mut(entity) {
                chronicle.record(
                    day,
                    format!("was called to the trade of the {}", action.0.trade()),
                );
            }
        }
    }
}

fn spawn_section_header(commands: &mut Commands, parent: Entity, title: &str) {
    let header_node = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8.0),
                margin: UiRect::top(px(4.0)),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Node {
            width: px(10.0),
            height: px(1.5),
            ..default()
        },
        BackgroundColor(ui::theme::accent().with_alpha(0.7)),
        ChildOf(header_node),
    ));
    commands.spawn((
        ui::DisplayFace,
        Text::new(title),
        TextFont {
            font_size: FontSize::Px(11.5),
            ..default()
        },
        TextColor(ui::theme::accent()),
        ChildOf(header_node),
    ));
    commands.spawn((
        Node {
            flex_grow: 1.0,
            height: px(1.0),
            ..default()
        },
        BackgroundColor(ui::theme::panel_border().with_alpha(0.3)),
        ChildOf(header_node),
    ));
}

fn spawn_vital_card(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    value: &str,
    value_color: Color,
) {
    let card = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Percent(45.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(10.0), px(7.0)),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(3.0)),
                row_gap: px(2.0),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg().with_alpha(0.65)),
            BorderColor::all(ui::theme::panel_border().with_alpha(0.35)),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        ui::DisplayFace,
        Text::new(label),
        TextFont {
            font_size: FontSize::Px(9.5),
            ..default()
        },
        TextColor(ui::theme::text_dim().with_alpha(0.75)),
        ChildOf(card),
    ));
    commands.spawn((
        ui::SerifFace,
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(13.5),
            ..default()
        },
        TextColor(value_color),
        ChildOf(card),
    ));
}

fn spawn_kin_row(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    value: &str,
    val_col: Color,
) {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Baseline,
                column_gap: px(8.0),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        ui::DisplayFace,
        Text::new(label),
        TextFont {
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(ui::theme::text_dim().with_alpha(0.85)),
        Node {
            width: px(72.0),
            flex_shrink: 0.0,
            ..default()
        },
        ChildOf(row),
    ));
    commands.spawn((
        ui::SerifFace,
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(val_col),
        Node {
            flex_grow: 1.0,
            min_width: px(0.0),
            ..default()
        },
        ChildOf(row),
    ));
}

#[allow(clippy::type_complexity)]
pub(crate) fn update_villager_profile(
    mut commands: Commands,
    profile: Res<VillagerProfile>,
    selected: Res<super::people::SelectedPerson>,
    mut roots: Query<&mut Visibility, With<VillagerProfileRoot>>,
    mut page_nodes: Query<
        (&ProfilePageNode, &mut Node, &mut Visibility),
        (Without<VillagerProfileRoot>, Without<ProfileTabButton>),
    >,
    mut tab_buttons: Query<
        (
            &ProfileTabButton,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Node,
        ),
        (Without<VillagerProfileRoot>, Without<ProfilePageNode>),
    >,
    mut tab_texts: Query<(&ProfileTabText, &mut TextColor)>,
    mut texts: Query<(&ProfileText, &mut Text)>,
    wells: Query<(&ProfileContentWell, Entity)>,
    villagers: Query<
        (
            &Person,
            Option<&Activity>,
            Option<&Reaction>,
            Option<&Needs>,
            Option<&Morale>,
            Option<&Faith>,
            Option<&Traits>,
            Option<&RecentlySaid>,
            Option<&Spouse>,
            Option<&Parentage>,
            Option<&Regard>,
            Option<&crate::creature::genome::CreatureGenome>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
    history: Query<
        (
            Option<&Stirrings>,
            Option<&Chronicle>,
            Option<&Vocation>,
            Option<&Skills>,
            Option<&Home>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
    names: Query<&Person, With<Villager>>,
    parents: Query<(Entity, &Parentage), With<Villager>>,
    mut last_rendered: Local<Option<(Entity, ProfileTab, usize, usize, usize, usize, usize)>>,
) {
    let Ok(mut visibility) = roots.single_mut() else {
        return;
    };
    if !profile.open {
        *visibility = Visibility::Hidden;
        *last_rendered = None;
        return;
    }

    let Some(entity) = selected.0 else {
        *visibility = Visibility::Hidden;
        *last_rendered = None;
        return;
    };
    let Ok((
        person,
        activity,
        reaction,
        needs,
        morale,
        faith,
        traits,
        recently_said,
        spouse,
        parentage,
        regard,
        genome,
    )) = villagers.get(entity)
    else {
        *visibility = Visibility::Hidden;
        *last_rendered = None;
        return;
    };
    let Ok((stirrings, chronicle, vocation, skills, home)) = history.get(entity) else {
        *visibility = Visibility::Hidden;
        *last_rendered = None;
        return;
    };
    *visibility = Visibility::Visible;

    // Update active tab styles and page visibility
    for (page_tab, mut node, mut vis) in &mut page_nodes {
        let is_active = page_tab.0 == profile.tab;
        node.display = if is_active {
            Display::Flex
        } else {
            Display::None
        };
        *vis = if is_active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for (tab_btn, interaction, mut bg, mut border, mut node) in &mut tab_buttons {
        let is_active = tab_btn.0 == profile.tab;
        let is_hovered = *interaction == Interaction::Hovered;

        node.border = UiRect {
            top: if is_active { px(3.0) } else { px(1.0) },
            left: px(1.0),
            right: px(1.0),
            bottom: px(1.0),
        };

        *bg = if is_active {
            BackgroundColor(ui::theme::card_bg())
        } else if is_hovered {
            BackgroundColor(ui::theme::title_bg().with_alpha(0.45))
        } else {
            BackgroundColor(Color::BLACK.with_alpha(0.32))
        };

        *border = BorderColor {
            top: if is_active {
                ui::theme::accent()
            } else if is_hovered {
                ui::theme::accent().with_alpha(0.4)
            } else {
                Color::WHITE.with_alpha(0.06)
            },
            left: if is_active {
                ui::theme::card_border()
            } else {
                ui::theme::panel_border().with_alpha(0.25)
            },
            right: if is_active {
                ui::theme::card_border()
            } else {
                ui::theme::panel_border().with_alpha(0.25)
            },
            bottom: if is_active {
                ui::theme::card_bg()
            } else {
                ui::theme::card_border()
            },
        };
    }

    for (tab_text, mut text_color) in &mut tab_texts {
        let is_active = tab_text.0 == profile.tab;
        text_color.0 = if is_active {
            ui::theme::accent()
        } else {
            ui::theme::text_dim().with_alpha(0.65)
        };
    }

    let name = person.full_name();
    let vocation_title = vocation
        .map(|vocation| vocation.title())
        .unwrap_or("Villager");
    let faith_word = faith.map(Faith::describe).unwrap_or("uncertain");
    let (hunger_str, hunger_color) = needs
        .map(|n| need_status(n.hunger))
        .unwrap_or(("unknown", ui::theme::text_dim()));
    let (rest_str, rest_color) = needs
        .map(|n| need_status(n.rest))
        .unwrap_or(("unknown", ui::theme::text_dim()));
    let (spirits_str, spirits_color) = morale
        .map(|m| spirits_status(m.spirits))
        .unwrap_or(("steady", ui::theme::text()));
    let faith_color = if faith_word == "devout" || faith_word == "steadfast" {
        ui::theme::accent()
    } else if faith_word == "wavering" {
        ui::theme::text()
    } else {
        ui::theme::text_dim()
    };

    let home_line = if home.is_some() {
        "Housed in the village."
    } else {
        "Homeless - rests by the village fire."
    };
    let present = state_phrase(activity, reaction);

    let subtitle_desc = genome.map_or("Villager", |g| person_phrase(g.sex, g.age));
    for (kind, mut text) in &mut texts {
        let value = match kind {
            ProfileText::Title => name.clone(),
            ProfileText::Subtitle => format!("{} - {}", vocation_title, subtitle_desc),
            ProfileText::Rail => format!("{present} - spirits {spirits_str} - faith {faith_word}"),
        };
        if text.0 != value {
            text.0 = value;
        }
    }

    // Check if active tab well needs rebuilding
    let chronicle_count = chronicle.map_or(0, |c| c.events.len());
    let stirrings_count = stirrings.map_or(0, |s| s.0.len());
    let said_count = recently_said.map_or(0, |s| s.0.len());
    let regard_count = regard.map_or(0, |r| r.bonds.len());
    let skills_count = skills.map_or(0, |s| s.0.len());

    let state_signature = (
        entity,
        profile.tab,
        chronicle_count,
        stirrings_count,
        said_count,
        regard_count,
        skills_count,
    );

    if last_rendered.as_ref() == Some(&state_signature) {
        return;
    }
    *last_rendered = Some(state_signature);

    // Rebuild active tab well
    for (well_tab, well_entity) in &wells {
        if well_tab.0 != profile.tab {
            continue;
        }
        commands.entity(well_entity).despawn_related::<Children>();

        match profile.tab {
            ProfileTab::Overview => {
                // 1. CURRENT DOING
                spawn_section_header(&mut commands, well_entity, "CURRENT DOING");
                let doing_card = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(px(12.0), px(8.0)),
                            border: UiRect::left(px(3.0)),
                            border_radius: BorderRadius::all(px(2.0)),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.6)),
                        BorderColor::all(ui::theme::accent().with_alpha(0.7)),
                        ChildOf(well_entity),
                    ))
                    .id();
                commands.spawn((
                    ui::SerifFace,
                    Text::new(format!("{name} is {present}.")),
                    TextFont {
                        font_size: FontSize::Px(13.5),
                        ..default()
                    },
                    TextColor(ui::theme::text()),
                    ChildOf(doing_card),
                ));

                // 2. VITALS & NEEDS
                spawn_section_header(&mut commands, well_entity, "VITALS & NEEDS");
                let vitals_grid = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: px(8.0),
                            row_gap: px(8.0),
                            ..default()
                        },
                        ChildOf(well_entity),
                    ))
                    .id();

                spawn_vital_card(
                    &mut commands,
                    vitals_grid,
                    "HUNGER",
                    hunger_str,
                    hunger_color,
                );
                spawn_vital_card(&mut commands, vitals_grid, "REST", rest_str, rest_color);
                spawn_vital_card(
                    &mut commands,
                    vitals_grid,
                    "SPIRITS",
                    spirits_str,
                    spirits_color,
                );
                spawn_vital_card(&mut commands, vitals_grid, "FAITH", faith_word, faith_color);

                // 3. SHELTER
                spawn_section_header(&mut commands, well_entity, "SHELTER & HEARTH");
                let shelter_card = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(px(12.0), px(8.0)),
                            border_radius: BorderRadius::all(px(2.0)),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.5)),
                        ChildOf(well_entity),
                    ))
                    .id();
                commands.spawn((
                    ui::SerifFace,
                    Text::new(home_line),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(if home.is_some() {
                        ui::theme::text()
                    } else {
                        Color::srgb(0.92, 0.65, 0.45)
                    }),
                    ChildOf(shelter_card),
                ));
            }
            ProfileTab::Skills => {
                // 1. CURRENT CALLING
                spawn_section_header(&mut commands, well_entity, "CURRENT CALLING");
                let calling_box = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::axes(px(12.0), px(8.0)),
                            border_radius: BorderRadius::all(px(2.0)),
                            row_gap: px(4.0),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.6)),
                        ChildOf(well_entity),
                    ))
                    .id();
                commands.spawn((
                    ui::DisplayFace,
                    Text::new(vocation_title),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(ui::theme::accent()),
                    ChildOf(calling_box),
                ));
                if let Some(voc) = vocation {
                    commands.spawn((
                        ui::SerifFace,
                        Text::new(voc.describe()),
                        TextFont {
                            font_size: FontSize::Px(12.5),
                            ..default()
                        },
                        TextColor(ui::theme::text_dim()),
                        ChildOf(calling_box),
                    ));
                }

                // 2. SKILL PROGRESSION
                spawn_section_header(&mut commands, well_entity, "SKILL PROGRESSION");
                if let Some(skills) = skills {
                    let mut crafts: Vec<(Vocation, f32)> = skills.0.clone();
                    crafts.sort_by(|a, b| b.1.total_cmp(&a.1));
                    let practiced: Vec<_> =
                        crafts.into_iter().filter(|(_, s)| *s > 0.005).collect();
                    if practiced.is_empty() {
                        commands.spawn((
                            ui::dim("Still learning the first trades of the village."),
                            ChildOf(well_entity),
                        ));
                    } else {
                        for (voc, score) in practiced {
                            let row = commands
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: px(10.0),
                                        padding: UiRect::axes(px(8.0), px(4.0)),
                                        border_radius: BorderRadius::all(px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(ui::theme::title_bg().with_alpha(0.3)),
                                    ChildOf(well_entity),
                                ))
                                .id();
                            // Vocation title
                            commands.spawn((
                                ui::DisplayFace,
                                Text::new(voc.title()),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(ui::theme::text()),
                                Node {
                                    width: px(90.0),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                ChildOf(row),
                            ));
                            // Progress bar
                            let track = commands
                                .spawn((
                                    Node {
                                        flex_grow: 1.0,
                                        height: px(8.0),
                                        overflow: Overflow::clip(),
                                        ..default()
                                    },
                                    BackgroundColor(Color::WHITE.with_alpha(0.08)),
                                    ChildOf(row),
                                ))
                                .id();
                            commands.spawn((
                                Node {
                                    width: Val::Percent((score * 100.0).clamp(2.0, 100.0)),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(crate::palette::shade(
                                    &crate::palette::CLOTH_GOLD,
                                    0.55 + score * 0.35,
                                )),
                                ChildOf(track),
                            ));
                            // Pct
                            commands.spawn((
                                ui::SerifFace,
                                Text::new(format!("{}%", (score * 100.0) as u32)),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(ui::theme::text_dim()),
                                Node {
                                    width: px(36.0),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                ChildOf(row),
                            ));
                            // Tier Badge
                            let badge = commands
                                .spawn((
                                    Node {
                                        padding: UiRect::axes(px(6.0), px(2.0)),
                                        border_radius: BorderRadius::all(px(2.0)),
                                        border: UiRect::all(px(1.0)),
                                        ..default()
                                    },
                                    BackgroundColor(ui::theme::title_bg().with_alpha(0.8)),
                                    BorderColor::all(ui::theme::panel_border().with_alpha(0.4)),
                                    ChildOf(row),
                                ))
                                .id();
                            commands.spawn((
                                ui::DisplayFace,
                                Text::new(Skills::tier_word(score)),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(ui::theme::accent()),
                                ChildOf(badge),
                            ));
                        }
                    }
                } else {
                    commands.spawn((
                        ui::dim("Still learning the first trades of the village."),
                        ChildOf(well_entity),
                    ));
                }

                // 3. TODAY'S ACTIVITY
                spawn_section_header(&mut commands, well_entity, "TODAY'S ACTIVITY");
                let act_card = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(px(10.0), px(6.0)),
                            border_radius: BorderRadius::all(px(2.0)),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.4)),
                        ChildOf(well_entity),
                    ))
                    .id();
                commands.spawn((
                    ui::SerifFace,
                    Text::new(format!("{name} is {present}.")),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(ui::theme::text()),
                    ChildOf(act_card),
                ));
            }
            ProfileTab::Bonds => {
                // 1. HOUSEHOLD & KIN
                spawn_section_header(&mut commands, well_entity, "HOUSEHOLD & KIN");
                let kin_card = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::axes(px(12.0), px(8.0)),
                            row_gap: px(7.0),
                            border_radius: BorderRadius::all(px(2.0)),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.55)),
                        ChildOf(well_entity),
                    ))
                    .id();

                let spouse_str = spouse
                    .map(|sp| villager_name(&names, sp.0))
                    .unwrap_or_else(|| "Unwed".to_string());
                let spouse_col = if spouse.is_some() {
                    ui::theme::accent()
                } else {
                    ui::theme::text_dim()
                };
                spawn_kin_row(&mut commands, kin_card, "WED TO", &spouse_str, spouse_col);

                let parent_str = parentage
                    .map(|p| {
                        format!(
                            "{} & {}",
                            villager_name(&names, p.mother),
                            villager_name(&names, p.father)
                        )
                    })
                    .unwrap_or_else(|| "Unknown".to_string());
                spawn_kin_row(
                    &mut commands,
                    kin_card,
                    "BORN TO",
                    &parent_str,
                    ui::theme::text(),
                );

                let children: Vec<String> = parents
                    .iter()
                    .filter(|(_, p)| p.mother == entity || p.father == entity)
                    .map(|(c, _)| villager_name(&names, c))
                    .collect();
                let children_str = match children.as_slice() {
                    [] => "None in the village".to_string(),
                    list => list.join(", "),
                };
                spawn_kin_row(
                    &mut commands,
                    kin_card,
                    "CHILDREN",
                    &children_str,
                    ui::theme::text(),
                );

                // 2. SOCIAL REGARD
                spawn_section_header(&mut commands, well_entity, "SOCIAL REGARD & FEELINGS");
                if let Some(regard) = regard {
                    let mut bonds = regard.bonds.iter().collect::<Vec<_>>();
                    bonds.sort_by(|left, right| right.warmth.abs().total_cmp(&left.warmth.abs()));
                    let top_bonds: Vec<_> = bonds.into_iter().take(6).collect();
                    if top_bonds.is_empty() {
                        commands.spawn((
                            ui::dim("No strong personal feelings formed yet."),
                            ChildOf(well_entity),
                        ));
                    } else {
                        for bond in top_bonds {
                            let row = commands
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: px(8.0),
                                        padding: UiRect::axes(px(8.0), px(5.0)),
                                        border_radius: BorderRadius::all(px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(ui::theme::title_bg().with_alpha(0.35)),
                                    ChildOf(well_entity),
                                ))
                                .id();

                            let target_name = villager_name(&names, bond.toward);
                            commands.spawn((
                                ui::SerifFace,
                                Text::new(target_name),
                                TextFont {
                                    font_size: FontSize::Px(13.0),
                                    ..default()
                                },
                                TextColor(ui::theme::text()),
                                Node {
                                    width: px(130.0),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                ChildOf(row),
                            ));

                            let (feeling_label, bg_col, text_col) = if bond.warmth > 0.4 {
                                (
                                    "Warmly",
                                    crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.25)
                                        .with_alpha(0.6),
                                    ui::theme::accent(),
                                )
                            } else if bond.warmth < -0.4 {
                                (
                                    "Resentful",
                                    crate::palette::shade(&crate::palette::CLOTH_RUST, 0.25)
                                        .with_alpha(0.6),
                                    Color::srgb(0.92, 0.45, 0.4),
                                )
                            } else {
                                (
                                    "Mixed",
                                    Color::WHITE.with_alpha(0.06),
                                    ui::theme::text_dim(),
                                )
                            };
                            let badge = commands
                                .spawn((
                                    Node {
                                        padding: UiRect::axes(px(6.0), px(2.0)),
                                        border_radius: BorderRadius::all(px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(bg_col),
                                    ChildOf(row),
                                ))
                                .id();
                            commands.spawn((
                                ui::DisplayFace,
                                Text::new(feeling_label),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(text_col),
                                ChildOf(badge),
                            ));

                            if let Some(cause) = &bond.over {
                                commands.spawn((
                                    ui::SerifFace,
                                    Text::new(format!("- {cause}")),
                                    TextFont {
                                        font_size: FontSize::Px(12.0),
                                        ..default()
                                    },
                                    TextColor(ui::theme::text_dim()),
                                    Node {
                                        flex_grow: 1.0,
                                        min_width: px(0.0),
                                        ..default()
                                    },
                                    ChildOf(row),
                                ));
                            }
                        }
                    }
                } else {
                    commands.spawn((
                        ui::dim("No strong personal feelings formed yet."),
                        ChildOf(well_entity),
                    ));
                }
            }
            ProfileTab::Inner => {
                // 1. CHARACTER & TEMPERAMENT
                spawn_section_header(&mut commands, well_entity, "CHARACTER & TEMPERAMENT");
                let char_card = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(px(12.0), px(8.0)),
                            border: UiRect::left(px(3.0)),
                            border_radius: BorderRadius::all(px(2.0)),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.5)),
                        BorderColor::all(ui::theme::accent().with_alpha(0.6)),
                        ChildOf(well_entity),
                    ))
                    .id();
                commands.spawn((
                    ui::SerifFace,
                    Text::new(
                        traits
                            .map(Traits::describe)
                            .unwrap_or_else(|| "Character is still taking shape.".to_string()),
                    ),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(ui::theme::text()),
                    ChildOf(char_card),
                ));

                // 2. DIVINE BELIEF
                spawn_section_header(&mut commands, well_entity, "DIVINE BELIEF");
                let faith_card = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::axes(px(12.0), px(8.0)),
                            border_radius: BorderRadius::all(px(2.0)),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.5)),
                        ChildOf(well_entity),
                    ))
                    .id();
                commands.spawn((
                    ui::SerifFace,
                    Text::new(format!("{name} is {faith_word}.")),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(ui::theme::text()),
                    ChildOf(faith_card),
                ));
                let faith_badge = commands
                    .spawn((
                        Node {
                            padding: UiRect::axes(px(8.0), px(3.0)),
                            border_radius: BorderRadius::all(px(2.0)),
                            border: UiRect::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.8)),
                        BorderColor::all(ui::theme::accent().with_alpha(0.4)),
                        ChildOf(faith_card),
                    ))
                    .id();
                commands.spawn((
                    ui::DisplayFace,
                    Text::new(faith_word.to_uppercase()),
                    TextFont {
                        font_size: FontSize::Px(10.5),
                        ..default()
                    },
                    TextColor(ui::theme::accent()),
                    ChildOf(faith_badge),
                ));

                // 3. LAST WORDS SPOKEN
                spawn_section_header(&mut commands, well_entity, "LAST SPOKEN WORDS");
                if let Some(said) = recently_said.and_then(|s| s.0.last()) {
                    let quote_card = commands
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::axes(px(12.0), px(8.0)),
                                row_gap: px(4.0),
                                border_radius: BorderRadius::all(px(2.0)),
                                ..default()
                            },
                            BackgroundColor(ui::theme::title_bg().with_alpha(0.5)),
                            ChildOf(well_entity),
                        ))
                        .id();
                    let quote_hdr = commands
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: px(6.0),
                                ..default()
                            },
                            ChildOf(quote_card),
                        ))
                        .id();
                    commands.spawn((
                        ui::DisplayFace,
                        Text::new(format!("DAY {}", said.day)),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(ui::theme::accent()),
                        ChildOf(quote_hdr),
                    ));
                    commands.spawn((
                        ui::SerifFace,
                        Text::new(format!("\"{}\"", said.text)),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(ui::theme::text()),
                        ChildOf(quote_card),
                    ));
                } else {
                    commands.spawn((
                        ui::dim("No recent spoken words recorded."),
                        ChildOf(well_entity),
                    ));
                }

                // 4. RECENT INNER STIRRING
                spawn_section_header(&mut commands, well_entity, "RECENT INNER STIRRING");
                if let Some(stirring) = stirrings.and_then(|s| s.0.last()) {
                    let stir_card = commands
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::axes(px(12.0), px(8.0)),
                                row_gap: px(4.0),
                                border_radius: BorderRadius::all(px(2.0)),
                                ..default()
                            },
                            BackgroundColor(ui::theme::title_bg().with_alpha(0.5)),
                            ChildOf(well_entity),
                        ))
                        .id();
                    let stir_hdr = commands
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: px(6.0),
                                ..default()
                            },
                            ChildOf(stir_card),
                        ))
                        .id();
                    commands.spawn((
                        ui::DisplayFace,
                        Text::new(format!("DAY {}", stirring.day)),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(ui::theme::accent()),
                        ChildOf(stir_hdr),
                    ));
                    commands.spawn((
                        ui::SerifFace,
                        Text::new(&stirring.text),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(ui::theme::text()),
                        ChildOf(stir_card),
                    ));
                } else {
                    commands.spawn((
                        ui::dim("No recent turning point recorded."),
                        ChildOf(well_entity),
                    ));
                }
            }
            ProfileTab::Chronicle => {
                spawn_section_header(&mut commands, well_entity, "PERSONAL LIFE CHRONICLE");
                if let Some(chronicle) = chronicle {
                    if chronicle.events.is_empty() {
                        commands.spawn((
                            ui::dim("Nothing has yet been entered into this chronicle."),
                            ChildOf(well_entity),
                        ));
                    } else {
                        let mut stripe = false;
                        for event in chronicle.events.iter().rev().take(15) {
                            let row = commands
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: px(10.0),
                                        padding: UiRect::axes(px(8.0), px(5.0)),
                                        border_radius: BorderRadius::all(px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(if stripe {
                                        Color::WHITE.with_alpha(0.025)
                                    } else {
                                        ui::theme::title_bg().with_alpha(0.4)
                                    }),
                                    ChildOf(well_entity),
                                ))
                                .id();
                            stripe = !stripe;

                            // Day badge
                            let badge = commands
                                .spawn((
                                    Node {
                                        padding: UiRect::axes(px(6.0), px(2.0)),
                                        border: UiRect::all(px(1.0)),
                                        border_radius: BorderRadius::all(px(2.0)),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    BackgroundColor(ui::theme::title_bg().with_alpha(0.85)),
                                    BorderColor::all(ui::theme::panel_border().with_alpha(0.5)),
                                    ChildOf(row),
                                ))
                                .id();
                            commands.spawn((
                                ui::DisplayFace,
                                Text::new(format!("DAY {:>2}", event.day)),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(ui::theme::accent()),
                                ChildOf(badge),
                            ));

                            // Event text
                            commands.spawn((
                                ui::SerifFace,
                                Text::new(&event.text),
                                TextFont {
                                    font_size: FontSize::Px(13.0),
                                    ..default()
                                },
                                TextColor(ui::theme::text()),
                                Node {
                                    flex_grow: 1.0,
                                    min_width: px(0.0),
                                    ..default()
                                },
                                ChildOf(row),
                            ));
                        }
                    }
                } else {
                    commands.spawn((
                        ui::dim("Nothing has yet been entered into this chronicle."),
                        ChildOf(well_entity),
                    ));
                }
            }
        }
    }
}

fn need_status(value: f32) -> (&'static str, Color) {
    if value < 0.25 {
        ("sated", Color::srgb(0.75, 0.88, 0.55))
    } else if value < 0.5 {
        ("a little worn", Color::srgb(0.92, 0.85, 0.55))
    } else if value < 0.75 {
        ("troubled", Color::srgb(0.92, 0.65, 0.45))
    } else {
        ("urgent", Color::srgb(0.92, 0.45, 0.4))
    }
}

fn spirits_status(value: f32) -> (&'static str, Color) {
    if value > 0.65 {
        ("high", ui::theme::accent())
    } else if value > 0.35 {
        ("steady", ui::theme::text())
    } else if value > 0.1 {
        ("low", Color::srgb(0.92, 0.65, 0.45))
    } else {
        ("failing", Color::srgb(0.92, 0.45, 0.4))
    }
}

fn villager_name(names: &Query<&Person, With<Villager>>, entity: Entity) -> String {
    names
        .get(entity)
        .map(Person::full_name)
        .unwrap_or_else(|_| "someone absent".to_string())
}
