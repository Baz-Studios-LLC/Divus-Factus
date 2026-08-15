use super::{people::PaperdollTarget, state_phrase};
use crate::creature::Corpse;
use crate::hand::DivineHand;
use crate::ui::PointerContext;
use crate::villager::belief::Faith;
use crate::villager::home::Home;
use crate::villager::regard::Regard;
use crate::villager::speech::RecentlySaid;
use crate::villager::traits::Traits;
use crate::villager::work::{Skills, Vocation};
use crate::villager::{
    Activity, Chronicle, Morale, Needs, Parentage, Person, Spouse, Stirrings, Villager,
};
use crate::witness::Reaction;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::PrimaryWindow;
use ordo::prelude::{inspector_panel, pane, tab, tab_strip};

#[derive(Resource, Default)]
pub(crate) struct VillagerProfile {
    open: bool,
}

#[derive(Component)]
pub(crate) struct VillagerProfileRoot;

#[derive(Component, Clone, Copy)]
pub(crate) enum ProfileText {
    Title,
    Subtitle,
    Rail,
    Overview,
    Work,
    Bonds,
    InnerLife,
    Chronicle,
}

#[derive(Component)]
pub(crate) struct CloseVillagerProfile;

pub(crate) fn spawn_villager_profile(mut commands: Commands, portrait: Res<PaperdollTarget>) {
    // This is an inspector, not a dossier: keep the village visible while its
    // details sit against the edge of the screen.
    let root = commands
        .spawn((
            inspector_panel(540.0),
            VillagerProfileRoot,
            Visibility::Hidden,
        ))
        .id();
    commands.entity(root).entry::<Node>().and_modify(|mut node| {
        node.width = Val::Px(560.0);
        node.height = Val::Vh(72.0);
        node.min_height = Val::Px(0.0);
    });

    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                align_items: AlignItems::Stretch,
                column_gap: Val::Px(12.0),
                ..default()
            },
            ChildOf(root),
        ))
        .id();

    let portrait_card = commands.spawn((ordo::card(), ChildOf(header))).id();
    commands.entity(portrait_card).insert(Node {
        width: Val::Px(116.0),
        height: Val::Px(146.0),
        flex_shrink: 0.0,
        padding: UiRect::all(Val::Px(8.0)),
        ..default()
    });
    commands.spawn((
        ImageNode::new(portrait.0.clone()),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ChildOf(portrait_card),
    ));

    let identity = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                ..default()
            },
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        ProfileText::Title,
        ordo::heading("VILLAGER"),
        ChildOf(identity),
    ));
    commands.spawn((
        ProfileText::Subtitle,
        ordo::dim(""),
        Node {
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
        ChildOf(identity),
    ));
    commands.spawn((
        ProfileText::Rail,
        ordo::dim(""),
        Node {
            width: Val::Percent(100.0),
            margin: UiRect::top(Val::Px(10.0)),
            ..default()
        },
        ChildOf(identity),
    ));
    commands.spawn((CloseVillagerProfile, ordo::button("CLOSE"), ChildOf(header)));

    commands.spawn((ordo::rule(), ChildOf(root)));

    let strip = commands.spawn((tab_strip(), ChildOf(root))).id();
    let pages = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    for (index, label, text) in [
        (0, "OVERVIEW", ProfileText::Overview),
        (1, "WORK", ProfileText::Work),
        (2, "BONDS", ProfileText::Bonds),
        (3, "INNER", ProfileText::InnerLife),
        (4, "HISTORY", ProfileText::Chronicle),
    ] {
        commands.spawn((tab(label, index), ChildOf(strip)));
        let pane = commands.spawn((pane(strip, index), ChildOf(pages))).id();
        commands.entity(pane).insert(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            padding: UiRect::top(Val::Px(14.0)),
            ..default()
        });
        let card = commands.spawn((ordo::card(), ChildOf(pane))).id();
        commands.entity(card).insert((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                padding: UiRect::all(Val::Px(18.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            crate::ui::Scrollable,
            bevy::ui::ScrollPosition::default(),
        ));
        commands.spawn((
            text,
            ordo::body(""),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            ChildOf(card),
        ));
    }
}

pub(crate) fn open_villager_profile(
    buttons: Res<ButtonInput<MouseButton>>,
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
}

pub(crate) fn close_villager_profile(
    activate: On<Activate>,
    closes: Query<(), With<CloseVillagerProfile>>,
    mut profile: ResMut<VillagerProfile>,
) {
    if closes.get(activate.entity).is_ok() {
        profile.open = false;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn update_villager_profile(
    profile: Res<VillagerProfile>,
    selected: Res<super::people::SelectedPerson>,
    mut roots: Query<&mut Visibility, With<VillagerProfileRoot>>,
    mut texts: Query<(&ProfileText, &mut Text)>,
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
) {
    let Ok(mut visibility) = roots.single_mut() else {
        return;
    };
    if !profile.open {
        *visibility = Visibility::Hidden;
        return;
    }

    let Some(entity) = selected.0 else {
        *visibility = Visibility::Hidden;
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
    )) = villagers.get(entity)
    else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Ok((stirrings, chronicle, vocation, skills, home)) = history.get(entity) else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Visible;

    let name = person.full_name();
    let vocation_title = vocation
        .map(|vocation| vocation.title())
        .unwrap_or("Villager");
    let faith_word = faith.map(Faith::describe).unwrap_or("uncertain");
    let hunger = needs
        .map(|needs| need_word(needs.hunger))
        .unwrap_or("unknown");
    let rest = needs
        .map(|needs| need_word(needs.rest))
        .unwrap_or("unknown");
    let spirits = morale
        .map(|morale| morale_word(morale.spirits))
        .unwrap_or("steady");
    let home_line = if home.is_some() {
        "They have a place in the village."
    } else {
        "They do not yet have a home."
    };
    let present = state_phrase(activity, reaction);

    let overview = format!(
        "THE MOMENT\n{name} is {present}.\n\nBODY\nHunger: {hunger}.\nRest: {rest}.\nSpirits: {spirits}.\n\nHOME\n{home_line}\n\nFAITH\n{name} is {faith_word}."
    );
    let work = format!(
        "CALLING\n{}\n\nCRAFT\n{}\n\nTODAY\n{} is {}.",
        vocation
            .map(|vocation| vocation.describe())
            .unwrap_or("No settled calling."),
        match (skills, vocation) {
            (Some(skills), Some(vocation)) => skills.describe(*vocation),
            _ => "Still learning the work of the village.".to_string(),
        },
        name,
        present,
    );
    let bonds = bonds_text(entity, spouse, parentage, regard, &names, &parents);
    let inner_life = inner_life_text(name.as_str(), traits, faith, recently_said, stirrings);
    let chronicle = chronicle_text(chronicle);

    for (kind, mut text) in &mut texts {
        let value = match kind {
            ProfileText::Title => name.clone(),
            ProfileText::Subtitle => vocation_title.to_string(),
            ProfileText::Rail => format!("{present} · spirits {spirits} · faith {faith_word}"),
            ProfileText::Overview => overview.clone(),
            ProfileText::Work => work.clone(),
            ProfileText::Bonds => bonds.clone(),
            ProfileText::InnerLife => inner_life.clone(),
            ProfileText::Chronicle => chronicle.clone(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

fn need_word(value: f32) -> &'static str {
    if value < 0.25 {
        "sated"
    } else if value < 0.5 {
        "a little worn"
    } else if value < 0.75 {
        "troubled"
    } else {
        "urgent"
    }
}

fn morale_word(value: f32) -> &'static str {
    if value > 0.65 {
        "high"
    } else if value > 0.35 {
        "steady"
    } else if value > 0.1 {
        "low"
    } else {
        "failing"
    }
}

fn villager_name(names: &Query<&Person, With<Villager>>, entity: Entity) -> String {
    names
        .get(entity)
        .map(Person::full_name)
        .unwrap_or_else(|_| "someone absent".to_string())
}

fn bonds_text(
    entity: Entity,
    spouse: Option<&Spouse>,
    parentage: Option<&Parentage>,
    regard: Option<&Regard>,
    names: &Query<&Person, With<Villager>>,
    parents: &Query<(Entity, &Parentage), With<Villager>>,
) -> String {
    let spouse_line = spouse
        .map(|spouse| format!("Married to {}.", villager_name(names, spouse.0)))
        .unwrap_or_else(|| "Not married.".to_string());
    let parent_line = parentage
        .map(|parents| {
            format!(
                "Child of {} and {}.",
                villager_name(names, parents.mother),
                villager_name(names, parents.father)
            )
        })
        .unwrap_or_else(|| "Their parents are not known here.".to_string());
    let children: Vec<String> = parents
        .iter()
        .filter(|(_, parents)| parents.mother == entity || parents.father == entity)
        .map(|(child, _)| villager_name(names, child))
        .collect();
    let children_line = match children.as_slice() {
        [] => "No children in the village.".to_string(),
        [child] => format!("Parent of {child}."),
        children => format!("Parent of {}.", children.join(", ")),
    };
    let feelings = regard
        .map(|regard| {
            let mut bonds = regard.bonds.iter().collect::<Vec<_>>();
            bonds.sort_by(|left, right| right.warmth.abs().total_cmp(&left.warmth.abs()));
            let lines = bonds
                .into_iter()
                .take(4)
                .map(|bond| {
                    let feeling = if bond.warmth > 0.4 {
                        "warmly"
                    } else if bond.warmth < -0.4 {
                        "with resentment"
                    } else {
                        "with mixed feelings"
                    };
                    let cause = bond
                        .over
                        .as_deref()
                        .map(|cause| format!(" — {cause}"))
                        .unwrap_or_default();
                    format!(
                        "They regard {} {}{}.",
                        villager_name(names, bond.toward),
                        feeling,
                        cause
                    )
                })
                .collect::<Vec<_>>();
            if lines.is_empty() {
                "No strong feelings have taken shape.".to_string()
            } else {
                lines.join("\n")
            }
        })
        .unwrap_or_else(|| "No strong feelings have taken shape.".to_string());

    format!("HOUSEHOLD\n{spouse_line}\n{parent_line}\n{children_line}\n\nFEELINGS\n{feelings}")
}

fn inner_life_text(
    name: &str,
    traits: Option<&Traits>,
    faith: Option<&Faith>,
    recently_said: Option<&RecentlySaid>,
    stirrings: Option<&Stirrings>,
) -> String {
    let character = traits
        .map(Traits::describe)
        .unwrap_or_else(|| "Their character is still taking shape.".to_string());
    let faith = faith.map(Faith::describe).unwrap_or("uncertain");
    let voice = recently_said
        .and_then(|said| said.0.last())
        .map(|said| format!("Day {} · {}", said.day, said.text))
        .unwrap_or_else(|| "Nothing recent has been recorded.".to_string());
    let stirrings = stirrings
        .and_then(|stirrings| stirrings.0.last())
        .map(|stirring| format!("Day {} · {}", stirring.day, stirring.text))
        .unwrap_or_else(|| "No recent turning point.".to_string());

    format!(
        "CHARACTER\n{character}\n\nFAITH\n{name} is {faith}.\n\nLATEST VOICE\n{voice}\n\nRECENT STIRRING\n{stirrings}"
    )
}

fn chronicle_text(chronicle: Option<&Chronicle>) -> String {
    let entries = chronicle
        .map(|chronicle| {
            chronicle
                .events
                .iter()
                .rev()
                .take(8)
                .map(|event| format!("Day {} · {}", event.day, event.text))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if entries.is_empty() {
        "Nothing has yet been entered into this life.".to_string()
    } else {
        format!("RECENT HISTORY\n{}", entries.join("\n\n"))
    }
}
