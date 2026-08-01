//! The doings: a plain label over every head saying what that person is
//! at, right now. The god's own diagnostic - when a village builds two
//! longhouses in twelve days, this is how you find out what the other
//! nine were doing instead.

use bevy::prelude::*;
use bevy::text::FontSize;

use crate::creature::Corpse;
use crate::villager::work::Vocation;
use crate::villager::{Activity, Person, Villager};

/// Whether the labels are showing.
#[derive(Resource, Default)]
pub struct DoingsMode(pub bool);

/// One person's label, following their head.
#[derive(Component)]
struct Doing(Entity);

pub struct DoingsPlugin;

impl Plugin for DoingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DoingsMode>()
            .add_systems(Update, (toggle_doings, tend_doings).chain());
    }
}

fn toggle_doings(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    mut mode: ResMut<DoingsMode>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    if !keymap.just_pressed(&keys, crate::keymap::Deed::Doings) {
        return;
    }
    mode.0 = !mode.0;
    if mode.0 {
        let cap =
            crate::keymap::key_name(keymap.key(crate::keymap::Deed::Doings)).unwrap_or("the key");
        notices.write(crate::ui::Notice::new(format!(
            "Every soul says what they are at - press {cap} again for quiet"
        )));
    }
}

/// What a person is at, in the fewest words that still tell the truth.
fn doing_of(activity: &Activity, vocation: Option<&Vocation>) -> String {
    let work = vocation.map(|v| v.describe()).unwrap_or("no trade");
    match activity {
        Activity::Idle => "idle".to_string(),
        Activity::Wandering => "wandering".to_string(),
        Activity::SeekingFood(_) => "foraging, hungry".to_string(),
        Activity::Eating(_) => "eating off a bush".to_string(),
        Activity::VisitingStore => "eating at the stores".to_string(),
        Activity::Sleeping => "asleep".to_string(),
        Activity::Working => format!("working - {work}"),
        Activity::Praying => "praying".to_string(),
        Activity::Sheltering => "sheltering".to_string(),
        Activity::TendingFire => "tending the fire".to_string(),
        Activity::Chatting => "talking".to_string(),
        Activity::Mourning => "mourning".to_string(),
        Activity::Hauling => format!("hauling - {work}"),
        Activity::Bearing => "bearing the dead".to_string(),
    }
}

/// Keeps a label over every living head while the mode is on, and
/// sweeps them all away when it goes off.
#[allow(clippy::too_many_arguments)]
fn tend_doings(
    mut commands: Commands,
    mode: Res<DoingsMode>,
    fonts: Option<Res<crate::ui::Fonts>>,
    cameras: Query<(&bevy::camera::Camera, &GlobalTransform)>,
    folk: Query<
        (Entity, &GlobalTransform, &Activity, Option<&Vocation>),
        (With<Villager>, With<Person>, Without<Corpse>),
    >,
    mut labels: Query<(Entity, &Doing, &mut Node, &mut Text, &mut Visibility)>,
) {
    if !mode.0 {
        for (label, ..) in &labels {
            commands.entity(label).despawn();
        }
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    let Some((camera, camera_at)) = cameras
        .iter()
        .find(|(camera, _)| camera.order == 0 && camera.is_active)
    else {
        return;
    };

    // Everyone who already wears a label.
    let mut worn: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for (label, doing, mut node, mut text, mut visibility) in &mut labels {
        let Ok((_, at, activity, vocation)) = folk.get(doing.0) else {
            commands.entity(label).despawn();
            continue;
        };
        worn.insert(doing.0);
        let word = doing_of(activity, vocation);
        if text.0 != word {
            *text = Text::new(word);
        }
        // Over the head, and only while the camera can see them.
        match camera.world_to_viewport(camera_at, at.translation() + Vec3::Y * 2.05) {
            Ok(spot) => {
                node.left = Val::Px(spot.x - 44.0);
                node.top = Val::Px(spot.y);
                *visibility = Visibility::Inherited;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }

    for (who, _, activity, vocation) in &folk {
        if worn.contains(&who) {
            continue;
        }
        commands.spawn((
            Doing(who),
            Text::new(doing_of(activity, vocation)),
            TextFont {
                font: fonts.text.clone().into(),
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(crate::palette::shade(&crate::palette::BONE, 0.97)),
            BackgroundColor(Color::BLACK.with_alpha(0.55)),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(120.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(6),
        ));
    }
}
