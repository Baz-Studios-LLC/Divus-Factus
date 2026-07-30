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
pub(crate) struct RowLabel(#[allow(dead_code)] Entity);

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

/// The LATELY card's dated rows, rebuilt with the dossier.
#[derive(Component)]
pub(crate) struct LatelyWell;

/// Which way the roster reads.
#[derive(Resource, Default)]
pub(crate) struct RosterSort(pub bool);

/// The little A-Z / Z-A toggle in the roster's header.
#[derive(Component)]
pub(crate) struct SortButton;

/// A roster row's face: who it belongs to and its resting shade, so the
/// selected row can glow and the rest can zebra.
#[derive(Component)]
pub(crate) struct RowFace {
    person: Entity,
    #[allow(dead_code)]
    base: f32,
}

pub(crate) fn spawn_people_panel(
    mut commands: Commands,
    codex: Res<super::village::Codex>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // The People page lives inside the codex now: its content builds into
    // the codex's people page, and the codex owns the window chrome, the
    // title and the page-turning. PeoplePanel rides the page node so every
    // is-the-panel-open gate keeps working unchanged.
    let page = codex.people_page;
    commands
        .entity(page)
        .insert((Name::new("People Page"), PeoplePanel));
    let (list, detail) = ui::split_row(&mut commands, page, 320.0);
    commands.entity(list).insert((
        PeopleRows,
        // The roster holds its ground: without flex_shrink 0 the stat
        // grid's minimum widths crushed the list to a ribbon.
        Node {
            width: px(320),
            flex_shrink: 0.0,
            max_height: px(640),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            overflow: Overflow::scroll_y(),
            padding: UiRect::all(px(6)),
            border_radius: BorderRadius::all(px(0)),
            ..default()
        },
    ));

    // The paperdoll: a private little stage far under the world, drawn by its
    // own camera to a texture the detail pane shows. The doll is the person's
    // real body, rebuilt, turning slowly.
    let target = images.add(bevy::image::Image::new_target_texture(
        440,
        672,
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
            // Near-neutral, and quiet: a blue fill painted the charcoal
            // alcove's edges sky-coloured.
            color: Color::srgb(0.75, 0.78, 0.85),
            illuminance: 2_800.0,
            ..default()
        },
        Transform::from_xyz(-2.6, 1.4, -1.2).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER),
    ));

    // The niche: real architecture behind the sitter - a stone alcove
    // with pillars, a shouldered arch, a recessed dark panel for the
    // vignette, and a two-tier plinth with a thread of gold. Actual
    // geometry under the portrait lights, not painted rectangles.
    let layer = bevy::camera::visibility::RenderLayers::layer(DOLL_LAYER);
    // Charcoal throughout, matched to the panel's own ground: the niche
    // is a stage, not a subject - nothing behind the sitter competes.
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
        base_color: Color::srgb(0.032, 0.035, 0.045),
        perceptual_roughness: 1.0,
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.13, 0.135, 0.155),
        perceptual_roughness: 0.8,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mut set_piece =
        |mesh: Handle<Mesh>, material: Handle<StandardMaterial>, offset: Vec3, scale: Vec3| {
            commands.spawn((
                Name::new("Niche"),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(DOLL_STAGE + offset).with_scale(scale),
                layer.clone(),
            ));
        };
    // The dark heart of the alcove, and the wall it is cut into.
    set_piece(
        cube.clone(),
        velvet.clone(),
        Vec3::new(0.0, 1.15, -0.95),
        Vec3::new(1.7, 2.5, 0.2),
    );
    set_piece(
        cube.clone(),
        stone.clone(),
        Vec3::new(0.0, 1.3, -1.15),
        Vec3::new(3.4, 3.4, 0.18),
    );
    // Pillars and the shouldered arch.
    for side in [-1.0f32, 1.0] {
        set_piece(
            cube.clone(),
            stone.clone(),
            Vec3::new(side * 1.06, 1.1, -0.85),
            Vec3::new(0.42, 2.5, 0.42),
        );
        set_piece(
            cube.clone(),
            stone_dark.clone(),
            Vec3::new(side * 0.72, 2.28, -0.85),
            Vec3::new(0.5, 0.34, 0.4),
        );
    }
    set_piece(
        cube.clone(),
        stone.clone(),
        Vec3::new(0.0, 2.52, -0.85),
        Vec3::new(2.6, 0.4, 0.46),
    );
    // The plinth: two tiers and the thread of gold.
    set_piece(
        meshes.add(Cylinder::new(1.05, 0.16)),
        stone.clone(),
        Vec3::new(0.0, -0.1, -0.15),
        Vec3::ONE,
    );
    set_piece(
        meshes.add(Cylinder::new(0.82, 0.18)),
        stone_dark.clone(),
        Vec3::new(0.0, 0.05, -0.15),
        Vec3::ONE,
    );
    set_piece(
        meshes.add(Cylinder::new(0.86, 0.03)),
        gold,
        Vec3::new(0.0, 0.13, -0.15),
        Vec3::ONE,
    );

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
            ChildOf(detail),
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
            ChildOf(detail),
        ))
        .id();

    // The mockup's spine: the portrait as a TALL plate down the left of
    // the dossier, with name, standing, tabs and the reading to its right.
    let dossier = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(400),
                flex_direction: FlexDirection::Row,
                column_gap: px(14),
                align_items: AlignItems::Stretch,
                ..default()
            },
            ChildOf(page),
        ))
        .id();
    let plaque = commands
        .spawn((
            Node {
                padding: UiRect::all(px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(ui::theme::title_bg()),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(dossier),
        ))
        .id();
    commands.spawn((
        bevy::ui::widget::ImageNode::new(target.clone()),
        Node {
            width: px(256),
            height: px(390),
            ..default()
        },
        ChildOf(plaque),
    ));
    let reading = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            ChildOf(dossier),
        ))
        .id();
    commands.spawn((DetailName, ui::title_sized("", 34.0), ChildOf(reading)));
    commands.spawn((
        DetailSubtitle,
        ui::dim(""),
        // Subtext, not a separate paragraph: Cinzel's line box carries
        // phantom depth below the capitals, so the standing is pulled up
        // to sit against the name it belongs to.
        Node {
            margin: UiRect::top(px(-14)),
            ..default()
        },
        ChildOf(reading),
    ));

    // Two faces of a person: the soul as it stands, and the life as it
    // was lived. The chronicle deserves its own page, not a footnote.
    let tabs = ui::tab_bar(
        &mut commands,
        reading,
        &["THE SOUL", "KIN & CRAFT", "THE LIFE"],
    );
    let (soul, kin_tab, life_tab) = (tabs[0], tabs[1], tabs[2]);
    // Pages stretch to the plate's foot, so the reading column and the
    // portrait close on one shared bottom edge - the puzzle contract.
    for (index, tab_page) in [soul, kin_tab, life_tab].into_iter().enumerate() {
        commands.entity(tab_page).insert((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(ui::theme::GAP),
                // A life can outgrow its band - Gabae of Tuhi had
                // twenty-three children spill over the footer - so every
                // page scrolls inside the puzzle contract's fixed row.
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ui::Scrollable,
            bevy::ui::ScrollPosition::default(),
        ));
        if index != 0 {
            commands
                .entity(tab_page)
                .entry::<Node>()
                .and_modify(|mut node| {
                    node.display = Display::None;
                });
        }
    }

    // Every readout the hover card has, as a two-column grid of chipped
    // rows - each stat wearing its little engraved glyph.
    let grid = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_content: AlignContent::SpaceEvenly,
                row_gap: px(3),
                padding: UiRect::axes(px(10), px(6)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.22)),
            ChildOf(soul),
        ))
        .id();
    for (value, label) in [
        (InspectorValue::State, "STATE"),
        (InspectorValue::Heart, "HEART"),
        (InspectorValue::Hunger, "HUNGER"),
        (InspectorValue::Manner, "MANNER"),
        (InspectorValue::Rest, "REST"),
        (InspectorValue::FaithIn, "FAITH"),
        (InspectorValue::Health, "HEALTH"),
        (InspectorValue::Work, "WORK"),
        (InspectorValue::Spirits, "SPIRITS"),
        (InspectorValue::Family, "FAMILY"),
        (InspectorValue::Seen, "SEEN YOU"),
    ] {
        let cell = commands
            .spawn((
                Node {
                    width: percent(50),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    padding: UiRect::axes(px(4), px(3)),
                    ..default()
                },
                ChildOf(grid),
            ))
            .id();
        stat_chip(&mut commands, cell, value);
        commands.spawn((
            ui::dim(label),
            Node {
                width: px(72),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(cell),
        ));
        commands.spawn((
            DetailStat(value),
            ui::body(""),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ChildOf(cell),
        ));
    }

    // WANTS and HAS SEEN, side by side; LATELY beneath with its dates in
    // the margin - the mockup's lower third.
    // The dossier's footer: WANTS, HAS SEEN and LATELY span the full
    // width beneath the portrait and the reading, whatever tab is open -
    // the mockup's composition, and the soul's want is never out of view.
    let cards = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(210),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                align_items: AlignItems::Stretch,
                margin: UiRect::top(px(10)),
                ..default()
            },
            ChildOf(page),
        ))
        .id();
    let card = |commands: &mut Commands, parent: Entity, title: &str| -> Entity {
        let card = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_height: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    padding: UiRect::all(px(10)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::BLACK.with_alpha(0.22)),
                BorderColor::all(ui::theme::text_dim().with_alpha(0.18)),
                ChildOf(parent),
            ))
            .id();
        commands.spawn((
            Text::new(title),
            ui::DisplayFace,
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(ui::theme::accent()),
            ChildOf(card),
        ));
        card
    };
    let left_col = commands
        .spawn((
            Node {
                flex_grow: 0.8,
                flex_basis: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                ..default()
            },
            ChildOf(cards),
        ))
        .id();
    let wants_card = card(&mut commands, left_col, "WANTS");
    commands.spawn((PersonDetailText, ui::body(""), ChildOf(wants_card)));
    let seen_card = card(&mut commands, left_col, "HAS SEEN");
    commands.spawn((DetailSeen, ui::body(""), ChildOf(seen_card)));
    let lately_card = card(&mut commands, cards, "LATELY");
    commands.spawn((
        LatelyWell,
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
            ..default()
        },
        ChildOf(lately_card),
    ));

    // KIN & CRAFT: the family tree and the working hands.
    ui::section_header(&mut commands, kin_tab, "THE CRAFT");
    commands.spawn((
        CraftWell,
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: UiRect::all(px(8)),
            border_radius: BorderRadius::all(px(0)),
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
            border_radius: BorderRadius::all(px(0)),
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
            border_radius: BorderRadius::all(px(0)),
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
    sort: Res<RosterSort>,
    mut last_sort: Local<bool>,
    settlements: Query<&Settlement>,
    people: Query<
        (
            Entity,
            &Person,
            &crate::creature::genome::CreatureGenome,
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

    // Rebuild only when the roll of the living - or the sort - changes.
    let _ = &mut labels;
    let mut current: Vec<Entity> = people.iter().map(|(e, ..)| e).collect();
    current.sort();
    if current == *roster && *last_sort == sort.0 && !just_opened {
        return;
    }
    *roster = current;
    *last_sort = sort.0;
    commands.entity(container).despawn_related::<Children>();

    // The roster's masthead: what this list is, which way it reads, and
    // how many souls it holds.
    let head = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8),
                padding: UiRect::new(px(13), px(10), px(7), px(7)),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(ui::theme::text_dim().with_alpha(0.25)),
            ChildOf(container),
        ))
        .id();
    commands.spawn((
        ui::dim("ALL PEOPLE"),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        ChildOf(head),
    ));
    let sort_button = commands
        .spawn((
            SortButton,
            ui::UiButton,
            Node {
                padding: UiRect::axes(px(7), px(1)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BorderColor::all(ui::theme::panel_border()),
            Interaction::default(),
            ChildOf(head),
        ))
        .id();
    commands.spawn((
        ui::dim(if sort.0 { "Z-A" } else { "A-Z" }),
        ChildOf(sort_button),
    ));
    commands.spawn((ui::dim(format!("{}", roster.len())), ChildOf(head)));

    let village = settlements.iter().next().map(|s| s.name.clone());
    let mut names: Vec<_> = people.iter().collect();
    names.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    if sort.0 {
        names.reverse();
    }
    for (entity, person, genome, _) in names {
        let base = 0.0;
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
                    column_gap: px(6),
                    padding: UiRect::new(px(12), px(10), px(6), px(6)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    ..default()
                },
                BackgroundColor(ui::theme::title_bg().with_alpha(0.55)),
                BorderColor::all(ui::theme::text_dim().with_alpha(0.14)),
                ChildOf(container),
            ))
            .id();
        let name_button = commands
            .spawn((
                PersonRow(entity),
                ui::UiButton,
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    border_radius: BorderRadius::all(px(0)),
                    ..default()
                },
                BackgroundColor(ui::theme::panel_bg().with_alpha(0.0)),
                Interaction::default(),
                ChildOf(row),
            ))
            .id();
        // The thumbnail: this person's own colours as a little bust -
        // hair over face over shoulders.
        let bust = commands
            .spawn((
                Node {
                    width: px(20),
                    height: px(28),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
                ChildOf(name_button),
            ))
            .id();
        let swatch = |commands: &mut Commands,
                      tone: crate::creature::genome::Tone,
                      w: f32,
                      h: f32,
                      round: BorderRadius| {
            commands
                .spawn((
                    Node {
                        width: px(w),
                        height: px(h),
                        border_radius: round,
                        ..default()
                    },
                    BackgroundColor(crate::palette::color_at(tone.palette_index())),
                ))
                .id()
        };
        let hair = swatch(
            &mut commands,
            genome.hair,
            14.0,
            6.0,
            BorderRadius::top(px(3)),
        );
        let face = swatch(
            &mut commands,
            genome.skin,
            12.0,
            9.0,
            BorderRadius::bottom(px(2)),
        );
        let torso = swatch(
            &mut commands,
            genome.cloth,
            18.0,
            11.0,
            BorderRadius::top(px(4)),
        );
        for part in [hair, face, torso] {
            commands.entity(part).insert(ChildOf(bust));
        }
        // Name over standing, the mockup's two lines.
        let words = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ChildOf(name_button),
            ))
            .id();
        commands.spawn((
            RowLabel(entity),
            ui::label(person.name.clone()),
            ChildOf(words),
        ));
        commands.spawn((
            ui::dim(format!(
                "{} of {}",
                super::person_phrase(genome.sex, genome.age),
                village.as_deref().unwrap_or("the wilds")
            )),
            ChildOf(words),
        ));
        // The eye flies the camera to them; the row opens their page.
        let follow_button = commands
            .spawn((
                FollowButton(entity),
                ui::UiButton,
                Node {
                    width: px(22),
                    height: px(22),
                    flex_shrink: 0.0,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(999)),
                    ..default()
                },
                BackgroundColor(ui::theme::title_bg()),
                BorderColor::all(ui::theme::accent().with_alpha(0.4)),
                Interaction::default(),
                ChildOf(row),
            ))
            .id();
        for (l, t, w, h, r, bright) in [
            (4.5, 7.5, 12.0, 6.0, 6.0, false),
            (8.5, 8.5, 4.0, 4.0, 4.0, true),
        ] {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(l),
                    top: px(t),
                    width: px(w),
                    height: px(h),
                    border_radius: BorderRadius::all(px(r)),
                    ..default()
                },
                BackgroundColor(if bright {
                    crate::palette::shade(&crate::palette::BONE, 0.95)
                } else {
                    ui::theme::accent().with_alpha(0.85)
                }),
                ChildOf(follow_button),
            ));
        }
    }
}

/// The sort toggle flips the roster's reading order.
pub(crate) fn handle_roster_sort(
    clicks: Query<&Interaction, (With<SortButton>, Changed<Interaction>)>,
    mut sort: ResMut<RosterSort>,
) {
    for interaction in &clicks {
        if *interaction == Interaction::Pressed {
            sort.0 = !sort.0;
        }
    }
}

/// The selected roster row glows; the rest keep their zebra shade.
pub(crate) fn style_roster_rows(
    selected: Res<SelectedPerson>,
    mut rows: Query<(&RowFace, &mut BackgroundColor, &mut BorderColor)>,
) {
    for (face, mut bg, mut border) in &mut rows {
        if selected.0 == Some(face.person) {
            bg.0 = ui::theme::accent().with_alpha(0.1);
            *border = BorderColor::all(ui::theme::accent().with_alpha(0.8));
        } else {
            bg.0 = ui::theme::title_bg().with_alpha(0.55);
            *border = BorderColor::all(ui::theme::text_dim().with_alpha(0.14));
        }
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
    // The camera stands on +Z and a body's face points -Z: without the
    // half-turn every portrait was a study of the sitter's back.
    let sway = std::f32::consts::PI + (time.elapsed_secs() * 0.45).sin() * 0.55 - 0.25;
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
    )>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }

    let Some((
        (person, genome, member_of),
        (needs, activity, vitality, morale),
        (temperament, witnessed, faith, _chronicle),
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
}

/// A circled glyph for one readout, drawn in the same hand-set node
/// vocabulary as the chronicle's shelf marks: gold line-work in a ring.
fn stat_chip(commands: &mut Commands, parent: Entity, which: InspectorValue) {
    let ink = ui::theme::accent().with_alpha(0.92);
    let chip = commands
        .spawn((
            Node {
                width: px(24),
                height: px(24),
                flex_shrink: 0.0,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(999)),
                ..default()
            },
            BackgroundColor(ui::theme::title_bg()),
            BorderColor::all(ink.with_alpha(0.45)),
            ChildOf(parent),
        ))
        .id();
    let mut mark = |node: Node, turned: bool, bright: bool| {
        let colour = if bright {
            crate::palette::shade(&crate::palette::BONE, 0.95)
        } else {
            ink
        };
        if turned {
            commands.spawn((
                node,
                UiTransform::from_rotation(Rot2::degrees(45.0)),
                BackgroundColor(colour),
                ChildOf(chip),
            ));
        } else {
            commands.spawn((node, BackgroundColor(colour), ChildOf(chip)));
        }
    };
    let at = |left: f32, top: f32, w: f32, h: f32, round: f32| Node {
        position_type: PositionType::Absolute,
        left: px(left),
        top: px(top),
        width: px(w),
        height: px(h),
        border_radius: BorderRadius::all(px(round)),
        ..default()
    };
    match which {
        // A compass rose: the turned square and its heart.
        InspectorValue::State => {
            mark(at(7.0, 7.0, 8.0, 8.0, 0.0), true, false);
            mark(at(9.5, 9.5, 3.0, 3.0, 0.0), true, true);
        }
        // A bowl.
        InspectorValue::Hunger => {
            mark(
                Node {
                    position_type: PositionType::Absolute,
                    left: px(6),
                    top: px(10),
                    width: px(10),
                    height: px(6),
                    border_radius: BorderRadius::bottom(px(6)),
                    ..default()
                },
                false,
                false,
            );
            mark(at(5.0, 9.0, 12.0, 2.0, 1.0), false, false);
        }
        // A crescent moon: a disc with a bite of chip-dark.
        InspectorValue::Rest => {
            mark(at(6.5, 6.5, 9.0, 9.0, 9.0), false, false);
            let bite = commands
                .spawn((
                    at(9.5, 5.5, 8.0, 8.0, 8.0),
                    BackgroundColor(ui::theme::title_bg()),
                    ChildOf(chip),
                ))
                .id();
            let _ = bite;
        }
        // The healer's cross.
        InspectorValue::Health => {
            mark(at(10.0, 6.0, 3.0, 11.0, 1.0), false, false);
            mark(at(6.0, 10.0, 11.0, 3.0, 1.0), false, false);
        }
        // A spark.
        InspectorValue::Spirits => {
            mark(at(8.0, 8.0, 7.0, 7.0, 0.0), true, false);
            mark(at(10.0, 10.0, 3.0, 3.0, 0.0), true, true);
        }
        // A heart: two lobes and the turned point.
        InspectorValue::Heart => {
            mark(at(6.5, 7.0, 5.5, 5.5, 5.5), false, false);
            mark(at(11.0, 7.0, 5.5, 5.5, 5.5), false, false);
            mark(at(8.5, 9.0, 6.0, 6.0, 0.0), true, false);
        }
        // A manner of speaking: three uneven strokes.
        InspectorValue::Manner => {
            mark(at(6.0, 8.0, 2.5, 8.0, 1.0), false, false);
            mark(at(10.0, 6.0, 2.5, 10.0, 1.0), false, false);
            mark(at(14.0, 9.0, 2.5, 7.0, 1.0), false, false);
        }
        // A shrine: lintel over two pillars.
        InspectorValue::FaithIn => {
            mark(at(6.0, 7.0, 11.0, 2.5, 1.0), false, false);
            mark(at(7.5, 10.0, 2.5, 7.0, 0.0), false, false);
            mark(at(13.0, 10.0, 2.5, 7.0, 0.0), false, false);
        }
        // The hammer.
        InspectorValue::Work => {
            mark(at(10.5, 8.0, 2.5, 10.0, 1.0), true, false);
            mark(at(8.0, 5.5, 8.0, 3.5, 1.0), false, false);
        }
        // Two heads together.
        InspectorValue::Family => {
            mark(at(6.0, 7.0, 5.0, 5.0, 5.0), false, false);
            mark(at(11.5, 7.0, 5.0, 5.0, 5.0), false, false);
            mark(
                Node {
                    position_type: PositionType::Absolute,
                    left: px(5),
                    top: px(13),
                    width: px(13),
                    height: px(5),
                    border_radius: BorderRadius::top(px(5)),
                    ..default()
                },
                false,
                false,
            );
        }
        // The god's eye.
        InspectorValue::Seen => {
            mark(at(5.5, 8.5, 12.0, 7.0, 7.0), false, false);
            mark(at(10.0, 10.0, 4.0, 4.0, 4.0), false, true);
        }
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
    lately_wells: Query<Entity, With<LatelyWell>>,
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
                        border_radius: BorderRadius::all(px(0)),
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
                    border_radius: BorderRadius::all(px(0)),
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
                            border_radius: BorderRadius::all(px(0)),
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
                        border_radius: BorderRadius::all(px(0)),
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
            let words = commands
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
                ui::body(event.text.clone()),
                Node {
                    width: percent(100),
                    ..default()
                },
                ChildOf(words),
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

    // LATELY: the last few turnings, dates in the margin.
    for well in &lately_wells {
        commands.entity(well).despawn_related::<Children>();
        let Some(chronicle) = chronicle else {
            commands.spawn((ui::dim("unwritten"), ChildOf(well)));
            continue;
        };
        let tail = chronicle.events.len().saturating_sub(4);
        for event in &chronicle.events[tail..] {
            let row = commands
                .spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(10),
                        ..default()
                    },
                    ChildOf(well),
                ))
                .id();
            commands.spawn((
                ui::dim(crate::calendar::date_of_day(event.day)),
                // Dates never wrap - and saying so kills the ghost height
                // their measurement otherwise reserves for the wrapping
                // they would never do.
                TextLayout {
                    linebreak: bevy::text::LineBreak::NoWrap,
                    ..default()
                },
                Node {
                    width: px(150),
                    flex_shrink: 0.0,
                    ..default()
                },
                ChildOf(row),
            ));
            let words = commands
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: px(0),
                        ..default()
                    },
                    ChildOf(row),
                ))
                .id();
            // The wrapper's width is definite once laid out, so the text
            // measures against it - a bare flex-grown Text measures its
            // height fully wrapped at minimum width and keeps that tall
            // ghost height, which spread these rows down the card.
            commands.spawn((
                ui::body(event.text.clone()),
                Node {
                    width: percent(100),
                    ..default()
                },
                ChildOf(words),
            ));
        }
        if tail == chronicle.events.len() {
            commands.spawn((ui::dim("a quiet life, so far"), ChildOf(well)));
        }
    }
}

/// A click on a roster row flies the camera to that person and follows them.
pub(crate) fn handle_people_rows(
    rows: Query<(&Interaction, &PersonRow), Changed<Interaction>>,
    followers: Query<(&Interaction, &FollowButton), Changed<Interaction>>,
    mut follow: ResMut<crate::camera::FollowTarget>,
    mut selected: ResMut<SelectedPerson>,
    mut panels: Query<&mut Visibility, With<super::village::VillagePanel>>,
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
            // Following is about WATCHING: the whole codex steps aside so
            // the eye lands on the person, not their paperwork.
            for mut visibility in &mut panels {
                *visibility = Visibility::Hidden;
            }
        }
    }
}
