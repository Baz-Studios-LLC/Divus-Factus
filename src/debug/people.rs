//! THE PEOPLE window: roster, paperdoll, and the dossier.

use super::*;
use crate::villager::{
    Activity, Chronicle, MemberOf, Morale, Needs, Parentage, Person, Settlement, Spouse, Villager,
};
use crate::witness::{Temperament, Witnessed};

/// The toolbar button that opens the people roster.
#[derive(Component)]
pub(crate) struct PeopleButton;

/// The roster panel: everyone alive, click to follow.
#[derive(Component)]
pub(crate) struct PeoplePanel;

/// The container the roster's rows are rebuilt into.
#[derive(Component)]
pub(crate) struct PeopleRows;

/// One roster row, pointing at its person.
#[derive(Component)]
pub(crate) struct PersonRow(Entity);

/// The text of a roster row, updated in place between rebuilds.
#[derive(Component)]
pub(crate) struct RowLabel(Entity);

/// Where the paperdoll stands: a stage far below the world, on its own
/// render layer, seen by no one but its own camera.
pub(crate) const DOLL_STAGE: Vec3 = Vec3::new(0.0, -600.0, 0.0);
pub(crate) const DOLL_LAYER: usize = 2;

/// The offscreen texture the paperdoll camera draws to.
#[derive(Resource)]
pub(crate) struct PaperdollTarget(#[allow(dead_code)] Handle<Image>);

/// Whose dossier the people window shows.
#[derive(Resource, Default)]
pub(crate) struct SelectedPerson(pub(crate) Option<Entity>);

/// The paperdoll body currently on stage.
#[derive(Component)]
pub(crate) struct DollBody;

/// The text block in the people window's detail pane.
#[derive(Component)]
pub(crate) struct PersonDetailText;

/// A follow button beside a roster name: click to fly to and follow them.
#[derive(Component)]
pub(crate) struct FollowButton(Entity);

/// A live stat row in the people window's detail pane, mirroring the
/// inspector's readouts.
#[derive(Component)]
pub(crate) struct DetailStat(InspectorValue);

/// The name line at the top of the detail pane.
#[derive(Component)]
pub(crate) struct DetailName;

/// The subtitle under the name.
#[derive(Component)]
pub(crate) struct DetailSubtitle;

/// The HAS SEEN body text in the detail pane.
#[derive(Component)]
pub(crate) struct DetailSeen;

/// The LIFE body text in the detail pane.
#[derive(Component)]
pub(crate) struct DetailLife;

/// The full life story on its own tab, every entry dated.
/// The dossier content, shown only while someone is selected.
#[derive(Component)]
pub(crate) struct DetailPage;

/// The empty state shown when no one is selected.
#[derive(Component)]
pub(crate) struct DetailEmpty;

/// The craft ledger's rebuilt-on-change container.
#[derive(Component)]
pub(crate) struct CraftWell;

/// The genealogy's rebuilt-on-change container.
#[derive(Component)]
pub(crate) struct KinWell;

/// The life story's rebuilt-on-change container.
#[derive(Component)]
pub(crate) struct LifeWell;

/// A roster row's face: who it belongs to and its resting shade, so the
/// selected row can glow and the rest can zebra.
#[derive(Component)]
pub(crate) struct RowFace {
    person: Entity,
    base: f32,
}

pub(crate) fn spawn_people_panel(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let split = ui::split_view(&mut commands, "THE PEOPLE", 300.0, 640.0);
    // Capture mode opens the window and picks somebody, so an unattended
    // screenshot can prove the pane works.
    let starts = if crate::capture_path().is_some() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    commands
        .entity(split.window.root)
        .insert((Name::new("People Panel"), PeoplePanel, starts));
    commands.entity(split.list).insert(PeopleRows);

    // The paperdoll: a private little stage far under the world, drawn by its
    // own camera to a texture the detail pane shows. The doll is the person's
    // real body, rebuilt, turning slowly.
    let target = images.add(bevy::image::Image::new_target_texture(
        440,
        520,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        Name::new("Paperdoll Camera"),
        Camera3d::default(),
        Camera {
            order: -20,
            clear_color: bevy::camera::ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        bevy::camera::RenderTarget::Image(target.clone().into()),
        Transform::from_translation(DOLL_STAGE + Vec3::new(0.0, 1.1, 3.1))
            .looking_at(DOLL_STAGE + Vec3::Y * 0.85, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER),
    ));
    // Portrait light: a warm key from the front quarter and a cool fill
    // from the far side, so the face has form instead of appliance-flat
    // panels.
    commands.spawn((
        Name::new("Paperdoll Key"),
        DirectionalLight {
            color: crate::palette::shade(&crate::palette::BONE, 1.0),
            illuminance: 13_000.0,
            ..default()
        },
        Transform::from_xyz(1.6, 2.6, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER),
    ));
    commands.spawn((
        Name::new("Paperdoll Fill"),
        DirectionalLight {
            color: crate::palette::shade(&crate::palette::SKY, 0.85),
            illuminance: 4_500.0,
            ..default()
        },
        Transform::from_xyz(-2.6, 1.4, -1.2).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER),
    ));

    // The page and its empty state: one shows, the other doesn't.
    let empty = commands
        .spawn((
            DetailEmpty,
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(4),
                ..default()
            },
            ChildOf(split.detail),
        ))
        .id();
    commands.spawn((ui::heading("NO ONE CHOSEN"), ChildOf(empty)));
    commands.spawn((ui::dim("click a name to meet them"), ChildOf(empty)));

    let page = commands
        .spawn((
            DetailPage,
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(ui::theme::GAP),
                ..default()
            },
            Visibility::Hidden,
            ChildOf(split.detail),
        ))
        .id();

    // Portrait beside name and standing: the page's masthead. The portrait
    // sits in a framed plaque, not floating on the panel.
    let masthead = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(page),
        ))
        .id();
    let plaque = commands
        .spawn((
            Node {
                padding: UiRect::all(px(3)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(ui::theme::title_bg()),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(masthead),
        ))
        .id();
    commands.spawn((
        bevy::ui::widget::ImageNode::new(target.clone()),
        Node {
            width: px(104),
            height: px(124),
            ..default()
        },
        ChildOf(plaque),
    ));
    let masthead_text = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            },
            ChildOf(masthead),
        ))
        .id();
    commands.spawn((DetailName, ui::heading(""), ChildOf(masthead_text)));
    commands.spawn((DetailSubtitle, ui::dim(""), ChildOf(masthead_text)));

    // Two faces of a person: the soul as it stands, and the life as it
    // was lived. The chronicle deserves its own page, not a footnote.
    let tabs = ui::tab_bar(
        &mut commands,
        page,
        &["THE SOUL", "KIN & CRAFT", "THE LIFE"],
    );
    let (soul, kin_tab, life_tab) = (tabs[0], tabs[1], tabs[2]);

    // Every readout the hover card has, permanent and orderly.
    for (value, label) in [
        (InspectorValue::State, "state"),
        (InspectorValue::Hunger, "hunger"),
        (InspectorValue::Rest, "rest"),
        (InspectorValue::Health, "health"),
        (InspectorValue::Spirits, "spirits"),
        (InspectorValue::Heart, "heart"),
        (InspectorValue::Manner, "manner"),
        (InspectorValue::FaithIn, "faith"),
        (InspectorValue::Work, "work"),
        (InspectorValue::Family, "family"),
        (InspectorValue::Seen, "seen you"),
    ] {
        let row = ui::stat_row(&mut commands, soul, label, None);
        commands.entity(row.value).insert(DetailStat(value));
    }
    ui::section_header(&mut commands, soul, "WANTS");
    commands.spawn((PersonDetailText, ui::body(""), ChildOf(soul)));
    ui::section_header(&mut commands, soul, "HAS SEEN");
    commands.spawn((DetailSeen, ui::body(""), ChildOf(soul)));
    ui::section_header(&mut commands, soul, "LATELY");
    commands.spawn((DetailLife, ui::dim(""), ChildOf(soul)));

    // KIN & CRAFT: the family tree and the working hands.
    ui::section_header(&mut commands, kin_tab, "THE CRAFT");
    commands.spawn((
        CraftWell,
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: UiRect::all(px(8)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.25)),
        ChildOf(kin_tab),
    ));
    ui::section_header(&mut commands, kin_tab, "THE KIN");
    commands.spawn((
        KinWell,
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(7),
            padding: UiRect::all(px(8)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.25)),
        ChildOf(kin_tab),
    ));

    // THE LIFE: the whole story, newest first, set in the chronicle's own
    // language - day bands, shelf glyphs, striped rows.
    commands.spawn((
        LifeWell,
        Node {
            width: percent(100),
            max_height: px(330),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::scroll_y(),
            padding: UiRect::all(px(8)),
            row_gap: px(2),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.35)),
        ui::Scrollable,
        ChildOf(life_tab),
    ));
    commands.insert_resource(PaperdollTarget(target));
}

/// Rebuilds the roster while it is open: one clickable row per living person.
pub(crate) fn update_people_panel(
    mut commands: Commands,
    time: Res<Time>,
    mut last_rebuild: Local<f32>,
    mut was_open: Local<bool>,
    mut roster: Local<Vec<Entity>>,
    panels: Query<&Visibility, With<PeoplePanel>>,
    containers: Query<Entity, With<PeopleRows>>,
    mut labels: Query<(&RowLabel, &mut Text)>,
    people: Query<
        (
            Entity,
            &Person,
            Option<&crate::villager::work::Vocation>,
            &Activity,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
) {
    let open = panels.iter().any(|v| *v != Visibility::Hidden);
    // A window that just opened fills instantly; only the refresh is paced.
    let just_opened = open && !*was_open;
    *was_open = open;
    if !open {
        return;
    }
    *last_rebuild += time.delta_secs();
    if *last_rebuild < 2.0 && !just_opened {
        return;
    }
    *last_rebuild = 0.0;

    let Ok(container) = containers.single() else {
        return;
    };

    // The same people as last pass: refresh each row's words IN PLACE.
    // Tearing the rows down and rebuilding them was a once-a-heartbeat
    // flicker across the whole roster.
    let mut current: Vec<Entity> = people.iter().map(|(e, ..)| e).collect();
    current.sort();
    if current == *roster && !just_opened {
        for (label, mut text) in &mut labels {
            let Ok((_, person, vocation, activity)) = people.get(label.0) else {
                continue;
            };
            let doing = match activity {
                Activity::Working => vocation.map_or("at work", |v| v.describe()),
                other => state_phrase(Some(other), None),
            };
            let fresh = format!("{} - {}", person.name, doing);
            if text.0 != fresh {
                *text = Text::new(fresh);
            }
        }
        return;
    }
    *roster = current;
    commands.entity(container).despawn_related::<Children>();

    let mut names: Vec<_> = people.iter().collect();
    names.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    for (index, (entity, person, vocation, activity)) in names.into_iter().enumerate() {
        let doing = match activity {
            Activity::Working => vocation.map_or("at work", |v| v.describe()),
            other => state_phrase(Some(other), None),
        };
        let base = if index % 2 == 1 { 0.045 } else { 0.0 };
        let row = commands
            .spawn((
                RowFace {
                    person: entity,
                    base,
                },
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(4),
                    padding: UiRect::right(px(4)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(Color::WHITE.with_alpha(base)),
                ChildOf(container),
            ))
            .id();
        let name_button = commands
            .spawn((
                PersonRow(entity),
                ui::UiButton,
                Node {
                    flex_grow: 1.0,
                    padding: UiRect::axes(px(6), px(2)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(ui::theme::panel_bg().with_alpha(0.0)),
                Interaction::default(),
                ChildOf(row),
            ))
            .id();
        commands.spawn((
            RowLabel(entity),
            ui::body(format!("{} - {}", person.name, doing)),
            ChildOf(name_button),
        ));
        // The little chevron flies the camera to them; the name just opens
        // their page.
        let follow_button = commands
            .spawn((
                FollowButton(entity),
                ui::UiButton,
                Node {
                    width: px(18),
                    height: px(18),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(ui::theme::panel_border()),
                Interaction::default(),
                ChildOf(row),
            ))
            .id();
        commands.spawn((ui::dim(">"), ChildOf(follow_button)));
    }
}

/// The selected roster row glows; the rest keep their zebra shade.
pub(crate) fn style_roster_rows(
    selected: Res<SelectedPerson>,
    mut rows: Query<(&RowFace, &mut BackgroundColor)>,
) {
    for (face, mut bg) in &mut rows {
        bg.0 = if selected.0 == Some(face.person) {
            ui::theme::accent().with_alpha(0.16)
        } else {
            Color::WHITE.with_alpha(face.base)
        };
    }
}

pub(crate) fn update_paperdoll(
    mut commands: Commands,
    selected: Res<SelectedPerson>,
    assets: Option<Res<crate::creature::body::CreatureAssets>>,
    genomes: Query<&crate::creature::genome::CreatureGenome>,
    dolls: Query<Entity, With<DollBody>>,
) {
    if !selected.is_changed() {
        return;
    }
    let Some(assets) = assets else {
        return;
    };
    for doll in &dolls {
        commands.entity(doll).despawn();
    }
    let Some(genome) = selected.0.and_then(|person| genomes.get(person).ok()) else {
        return;
    };
    let root = commands
        .spawn((
            DollBody,
            Transform::from_translation(DOLL_STAGE),
            Visibility::default(),
            bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER),
        ))
        .id();
    let rig = crate::creature::body::build_body(&mut commands, &assets, root, genome);
    commands.entity(root).insert(rig);
}

/// `RenderLayers` does not inherit: every part the body builder spawned has
/// to be stamped onto the doll's private layer, or it renders in the world.
pub(crate) fn stamp_doll_layers(
    mut commands: Commands,
    dolls: Query<Entity, With<DollBody>>,
    children: Query<&Children>,
    unstamped: Query<(), Without<bevy::camera::visibility::RenderLayers>>,
) {
    for doll in &dolls {
        for part in children.iter_descendants(doll) {
            if unstamped.get(part).is_ok() {
                commands
                    .entity(part)
                    .insert(bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER));
            }
        }
    }
}

/// The doll turns slowly, so the whole person can be seen.
pub(crate) fn spin_doll(time: Res<Time>, mut dolls: Query<&mut Transform, With<DollBody>>) {
    // A portrait, not a rotisserie: the doll sways through a gentle
    // three-quarter arc, face toward the reader - nobody's head should
    // spend half its life being the back of a microwave.
    let sway = (time.elapsed_secs() * 0.45).sin() * 0.55 - 0.25;
    for mut doll in &mut dolls {
        doll.rotation = Quat::from_rotation_y(sway);
    }
}

/// Fills the detail pane: the whole dossier the hover card shows, plus what
/// they want — the people window is the long look, the hover card the glance.
#[allow(clippy::type_complexity)]
pub(crate) fn update_person_detail(
    selected: Res<SelectedPerson>,
    panels: Query<&Visibility, With<PeoplePanel>>,
    people: Query<
        (
            (
                &Person,
                Option<&crate::creature::genome::CreatureGenome>,
                Option<&MemberOf>,
            ),
            (
                Option<&Needs>,
                Option<&Activity>,
                Option<&crate::creature::Vitality>,
                Option<&Morale>,
            ),
            (
                Option<&Temperament>,
                Option<&Witnessed>,
                Option<&crate::villager::belief::Faith>,
                Option<&Chronicle>,
            ),
            (
                Option<&Spouse>,
                Option<&Parentage>,
                Option<&crate::villager::home::Home>,
                Option<&crate::villager::work::Vocation>,
                Option<&crate::villager::work::Skills>,
                Option<&crate::villager::traits::Traits>,
                Has<crate::creature::Childhood>,
            ),
        ),
        Without<crate::creature::Corpse>,
    >,
    kin_names: Query<&Person>,
    corpse_check: Query<Option<&crate::creature::Vitality>, With<crate::creature::Corpse>>,
    settlements: Query<&Settlement>,
    mut page: Query<
        (&mut Visibility, &mut Node),
        (With<DetailPage>, Without<DetailEmpty>, Without<PeoplePanel>),
    >,
    mut empty: Query<
        (&mut Visibility, &mut Node),
        (With<DetailEmpty>, Without<DetailPage>, Without<PeoplePanel>),
    >,
    mut texts: ParamSet<(
        Query<&mut Text, With<DetailName>>,
        Query<&mut Text, With<DetailSubtitle>>,
        Query<(&DetailStat, &mut Text)>,
        Query<&mut Text, With<PersonDetailText>>,
        Query<&mut Text, With<DetailSeen>>,
        Query<&mut Text, With<DetailLife>>,
    )>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }

    let Some((
        (person, genome, member_of),
        (needs, activity, vitality, morale),
        (temperament, witnessed, faith, chronicle),
        (spouse, parentage, home, vocation, skills, manner, child),
    )) = selected.0.and_then(|entity| people.get(entity).ok())
    else {
        // Hidden means GONE: Visibility alone leaves the node's empty
        // height in the layout - the mystery gap above the dossier.
        for (mut visibility, mut node) in &mut page {
            *visibility = Visibility::Hidden;
            node.display = Display::None;
        }
        for (mut visibility, mut node) in &mut empty {
            *visibility = Visibility::Inherited;
            node.display = Display::Flex;
        }
        return;
    };
    for (mut visibility, mut node) in &mut page {
        *visibility = Visibility::Inherited;
        node.display = Display::Flex;
    }
    for (mut visibility, mut node) in &mut empty {
        *visibility = Visibility::Hidden;
        node.display = Display::None;
    }

    if let Ok(mut name) = texts.p0().single_mut()
        && name.0 != person.name
    {
        *name = Text::new(person.name.clone());
    }
    let who = genome.map_or("a soul", |g| person_phrase(g.sex, g.age));
    let of = member_of
        .and_then(|m| settlements.get(m.0).ok())
        .map_or_else(|| "the wilds".to_string(), |s| s.name.clone());
    if let Ok(mut subtitle) = texts.p1().single_mut() {
        let fresh = format!("{who} of {of}");
        if subtitle.0 != fresh {
            *subtitle = Text::new(fresh);
        }
    }

    let hunger = needs.map_or(0.0, |n| n.hunger);
    let harm = vitality.map_or(0.0, |v| v.harm);
    for (stat, mut text) in &mut texts.p2() {
        let fresh = match stat.0 {
            InspectorValue::State => state_phrase(activity, None).to_string(),
            InspectorValue::Hunger => hunger_word(hunger).to_string(),
            InspectorValue::Rest => needs.map_or("wakeful", |n| rest_word(n.rest)).to_string(),
            InspectorValue::Health => health_word(harm).to_string(),
            InspectorValue::Spirits => morale
                .map_or("steady", |m| spirits_word(m.spirits))
                .to_string(),
            InspectorValue::Heart => temperament.map_or("unread", |t| t.describe()).to_string(),
            InspectorValue::Manner => manner.map_or("unremarkable".to_string(), |m| m.describe()),
            InspectorValue::FaithIn => faith
                .map_or("has never wondered", |f| f.describe())
                .to_string(),
            InspectorValue::Work => vocation.map_or("none yet".to_string(), |v| {
                skills.map_or_else(|| v.describe().to_string(), |s| s.describe(*v))
            }),
            InspectorValue::Family => family_phrase(spouse, parentage, &kin_names, &corpse_check),
            InspectorValue::Seen => match witnessed {
                Some(w) if w.is_innocent() && w.secondhand > 0 => "only in stories".to_string(),
                Some(w) if w.is_innocent() => "never".to_string(),
                Some(w) => format!("{} times", w.total),
                None => "never".to_string(),
            },
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }

    // Wants, memories, and the life so far — the parts of a person a table
    // cannot hold, each under its own ruled header.
    let mut wants: Vec<&str> = Vec::new();
    if needs.is_some_and(|n| n.hunger > 0.55) {
        wants.push("a full belly");
    }
    if needs.is_some_and(|n| n.rest > 0.7) {
        wants.push("a night's sleep");
    }
    if home.is_none() {
        wants.push("a roof of their own");
    }
    if spouse.is_none() && !child {
        wants.push("someone to come home to");
    }
    if morale.is_some_and(|m| m.spirits < 0.35) {
        wants.push("better days");
    }
    if vocation.is_none() && !child {
        wants.push("a calling");
    }
    if wants.is_empty() {
        wants.push("nothing - life, for now, is enough");
    }
    let fresh = wants
        .iter()
        .map(|want| format!("- {want}"))
        .collect::<Vec<_>>()
        .join("\n");
    if let Ok(mut text) = texts.p3().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }

    let fresh = match witnessed {
        Some(w) if !w.recent.is_empty() => w
            .recent
            .iter()
            .take(3)
            .map(|kind| format!("- {}", kind.describe()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => "nothing they could not explain".to_string(),
    };
    if let Ok(mut text) = texts.p4().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }

    let fresh = chronicle.as_ref().map_or_else(
        || "unwritten".to_string(),
        |chronicle| {
            let tail = chronicle.events.len().saturating_sub(4);
            chronicle.events[tail..]
                .iter()
                .map(|event| {
                    format!(
                        "{}  {}",
                        crate::calendar::date_of_day(event.day),
                        event.text
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        },
    );
    if let Ok(mut text) = texts.p5().single_mut()
        && text.0 != fresh
    {
        *text = Text::new(fresh);
    }
}

/// A clickable soul: a small plaque that selects its person when pressed.
/// The dead lie quiet - named, dimmed, unclickable.
fn kin_card(commands: &mut Commands, parent: Entity, who: Entity, name: &str, gone: bool) {
    let card = commands
        .spawn((
            Node {
                padding: UiRect::axes(px(10), px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(999)),
                ..default()
            },
            BackgroundColor(if gone {
                Color::BLACK.with_alpha(0.2)
            } else {
                ui::theme::title_bg()
            }),
            BorderColor::all(ui::theme::card_border()),
            ChildOf(parent),
        ))
        .id();
    if !gone {
        commands
            .entity(card)
            .insert((PersonRow(who), ui::UiButton, Interaction::default()));
    }
    let text = if gone {
        format!("{name} - at rest")
    } else {
        name.to_string()
    };
    if gone {
        commands.spawn((ui::dim(text), ChildOf(card)));
    } else {
        commands.spawn((ui::label(text), ChildOf(card)));
    }
}

/// Rebuilds the craft ledger, the family tree and the life story when the
/// chosen person - or their story - changes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_dossier(
    mut commands: Commands,
    selected: Res<SelectedPerson>,
    panels: Query<&Visibility, With<PeoplePanel>>,
    craft_wells: Query<Entity, With<CraftWell>>,
    kin_wells: Query<Entity, With<KinWell>>,
    life_wells: Query<Entity, With<LifeWell>>,
    souls: Query<(
        Option<&crate::villager::work::Skills>,
        Option<&Chronicle>,
        Option<&Spouse>,
        Option<&Parentage>,
    )>,
    parentages: Query<(Entity, &Parentage)>,
    names: Query<&Person>,
    corpses: Query<(), With<crate::creature::Corpse>>,
    mut last: Local<(Option<Entity>, usize)>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    let Some(person) = selected.0 else {
        return;
    };
    let Ok((skills, chronicle, spouse, parentage)) = souls.get(person) else {
        return;
    };
    let story_len = chronicle.map_or(0, |c| c.events.len());
    if last.0 == Some(person) && last.1 == story_len {
        return;
    }
    *last = (Some(person), story_len);

    // THE CRAFT: one bar per calling ever practised, deepest first.
    for well in &craft_wells {
        commands.entity(well).despawn_related::<Children>();
        let mut crafts: Vec<(crate::villager::work::Vocation, f32)> =
            skills.map_or_else(Vec::new, |s| s.0.clone());
        crafts.sort_by(|a, b| b.1.total_cmp(&a.1));
        if crafts.is_empty() {
            commands.spawn((ui::dim("no craft yet - young hands"), ChildOf(well)));
        }
        for (vocation, skill) in crafts {
            let row = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        ..default()
                    },
                    ChildOf(well),
                ))
                .id();
            commands.spawn((
                ui::label(vocation.describe().to_string()),
                Node {
                    width: px(150),
                    flex_shrink: 0.0,
                    ..default()
                },
                ChildOf(row),
            ));
            let track = commands
                .spawn((
                    Node {
                        width: px(170),
                        height: px(9),
                        border_radius: BorderRadius::all(px(5)),
                        flex_shrink: 0.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE.with_alpha(0.07)),
                    ChildOf(row),
                ))
                .id();
            commands.spawn((
                Node {
                    width: percent((skill * 100.0).max(2.0)),
                    height: percent(100),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BackgroundColor(crate::palette::shade(
                    &crate::palette::CLOTH_GOLD,
                    0.55 + skill * 0.35,
                )),
                ChildOf(track),
            ));
            commands.spawn((
                ui::dim(crate::villager::work::Skills::tier_word(skill)),
                ChildOf(row),
            ));
        }
    }

    // THE KIN: born to, wed to, and the children - each living soul a
    // plaque that opens their own page.
    for well in &kin_wells {
        commands.entity(well).despawn_related::<Children>();
        let relation = |commands: &mut Commands, label: &str, kin: Vec<Entity>| {
            let row = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(8),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: px(5),
                        ..default()
                    },
                    ChildOf(well),
                ))
                .id();
            commands.spawn((
                ui::dim(label.to_string()),
                Node {
                    width: px(84),
                    flex_shrink: 0.0,
                    ..default()
                },
                ChildOf(row),
            ));
            let mut any = false;
            for kin_entity in kin {
                if let Ok(kin_person) = names.get(kin_entity) {
                    any = true;
                    kin_card(
                        commands,
                        row,
                        kin_entity,
                        &kin_person.name,
                        corpses.get(kin_entity).is_ok(),
                    );
                }
            }
            if !any {
                commands.spawn((ui::dim("-"), ChildOf(row)));
            }
        };
        relation(
            &mut commands,
            "BORN TO",
            parentage.map_or_else(Vec::new, |p| vec![p.mother, p.father]),
        );
        relation(
            &mut commands,
            "WED TO",
            spouse.map_or_else(Vec::new, |s| vec![s.0]),
        );
        let children: Vec<Entity> = parentages
            .iter()
            .filter(|(_, p)| p.mother == person || p.father == person)
            .map(|(child, _)| child)
            .collect();
        relation(&mut commands, "CHILDREN", children);
    }

    // THE LIFE: the story in day bands, the chronicle's own language.
    for well in &life_wells {
        commands.entity(well).despawn_related::<Children>();
        let Some(chronicle) = chronicle else {
            commands.spawn((ui::dim("no story yet"), ChildOf(well)));
            continue;
        };
        let mut current_day = u32::MAX;
        let mut stripe = false;
        for event in chronicle.events.iter().rev().take(120) {
            if event.day != current_day {
                current_day = event.day;
                stripe = false;
                let band = commands
                    .spawn((
                        Node {
                            width: percent(100),
                            padding: UiRect::axes(px(10), px(4)),
                            margin: UiRect::top(px(6)),
                            border: UiRect::left(px(3)),
                            border_radius: BorderRadius::all(px(5)),
                            ..default()
                        },
                        BorderColor::all(ui::theme::accent().with_alpha(0.8)),
                        BackgroundColor(ui::theme::title_bg().with_alpha(0.85)),
                        ChildOf(well),
                    ))
                    .id();
                commands.spawn((
                    Text::new(crate::calendar::date_of_day(event.day).to_uppercase()),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(ui::theme::accent()),
                    ChildOf(band),
                ));
            }
            let ledger = super::history::Ledger::of(&event.text);
            let row = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(8),
                        padding: UiRect::axes(px(6), px(4)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(if stripe {
                        Color::WHITE.with_alpha(0.028)
                    } else {
                        Color::NONE
                    }),
                    ChildOf(well),
                ))
                .id();
            stripe = !stripe;
            super::history::spawn_glyph(&mut commands, row, ledger);
            commands.spawn((
                ui::body(event.text.clone()),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                ChildOf(row),
            ));
        }
        if chronicle.events.len() > 120 {
            commands.spawn((
                ui::dim(format!(
                    "... and {} earlier days of this life",
                    chronicle.events.len() - 120
                )),
                Node {
                    padding: UiRect::axes(px(10), px(8)),
                    ..default()
                },
                ChildOf(well),
            ));
        }
    }
}

/// A click on a roster row flies the camera to that person and follows them.
pub(crate) fn handle_people_rows(
    rows: Query<(&Interaction, &PersonRow), Changed<Interaction>>,
    followers: Query<(&Interaction, &FollowButton), Changed<Interaction>>,
    mut follow: ResMut<crate::camera::FollowTarget>,
    mut selected: ResMut<SelectedPerson>,
) {
    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed {
            selected.0 = Some(row.0);
        }
    }
    for (interaction, button) in &followers {
        if *interaction == Interaction::Pressed {
            follow.entity = Some(button.0);
            follow.style = crate::camera::FollowStyle::Overhead;
            selected.0 = Some(button.0);
        }
    }
}
