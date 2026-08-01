//! The labels: a word over every head. Two of them, on two keys.
//!
//! One says what a soul is AT, this minute - the god's own diagnostic,
//! for when a village builds two longhouses in twelve days and you want
//! to know what the other nine were doing instead. The other says what
//! they ARE: the trade they were set to, whatever they happen to be
//! doing with it.

use bevy::prelude::*;
use bevy::text::FontSize;

use crate::creature::{Childhood, Corpse};
use crate::villager::work::Vocation;
use crate::villager::{Activity, Person, Villager};

/// What the labels are saying, if anything.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum Labels {
    #[default]
    Quiet,
    /// What each soul is at, this minute.
    Doings,
    /// The trade each soul was set to.
    Trades,
}

/// One person's label, following their head. The word itself hangs in a
/// child, so the pill can shrink to fit it while the parent stays a
/// point pinned over the head.
#[derive(Component)]
struct Doing(Entity);

pub struct DoingsPlugin;

impl Plugin for DoingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Labels>()
            .add_systems(Update, (toggle_labels, tend_labels).chain());
    }
}

fn toggle_labels(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    mut mode: ResMut<Labels>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    // Either key turns its own labels on, turns them off again, or takes
    // the words over from the other.
    let asked = [
        (crate::keymap::Deed::Doings, Labels::Doings),
        (crate::keymap::Deed::Trades, Labels::Trades),
    ]
    .into_iter()
    .find(|(deed, _)| keymap.just_pressed(&keys, *deed));
    let Some((deed, wanted)) = asked else {
        return;
    };
    *mode = if *mode == wanted {
        Labels::Quiet
    } else {
        wanted
    };
    if *mode != Labels::Quiet {
        let cap = crate::keymap::key_name(keymap.key(deed)).unwrap_or("the key");
        let said = match wanted {
            Labels::Trades => "Every soul says their trade",
            _ => "Every soul says what they are at",
        };
        notices.write(crate::ui::Notice::new(format!(
            "{said} - press {cap} again for quiet"
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

/// The trade they were set to - the one word an overseer wants.
fn trade_of(vocation: Option<&Vocation>, child: bool) -> String {
    match (vocation, child) {
        (Some(trade), _) => trade.trade().to_string(),
        (None, true) => "child".to_string(),
        (None, false) => "no trade".to_string(),
    }
}

/// Keeps a label over every living head while the labels are up, and
/// sweeps them all away when they go quiet.
fn tend_labels(
    mut commands: Commands,
    mode: Res<Labels>,
    fonts: Option<Res<crate::ui::Fonts>>,
    cameras: Query<(&bevy::camera::Camera, &GlobalTransform)>,
    folk: Query<
        (
            Entity,
            &GlobalTransform,
            &Activity,
            Option<&Vocation>,
            Has<Childhood>,
        ),
        (With<Villager>, With<Person>, Without<Corpse>),
    >,
    mut labels: Query<(Entity, &Doing, &mut Node, &mut Visibility, &Children)>,
    mut words: Query<(&mut Text, &mut TextColor)>,
) {
    if *mode == Labels::Quiet {
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
    // Trades in gold, doings in bone: which question is being answered
    // should be plain without reading a word of it.
    let ink = match *mode {
        Labels::Trades => crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.92),
        _ => crate::palette::shade(&crate::palette::BONE, 0.97),
    };
    let say = |activity: &Activity, vocation: Option<&Vocation>, child: bool| match *mode {
        Labels::Trades => trade_of(vocation, child),
        _ => doing_of(activity, vocation),
    };

    // Everyone who already wears a label.
    let mut worn: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for (label, doing, mut node, mut visibility, children) in &mut labels {
        let Ok((_, at, activity, vocation, child)) = folk.get(doing.0) else {
            commands.entity(label).despawn();
            continue;
        };
        worn.insert(doing.0);
        if let Some(&pill) = children.first()
            && let Ok((mut text, mut colour)) = words.get_mut(pill)
        {
            let word = say(activity, vocation, child);
            if text.0 != word {
                *text = Text::new(word);
            }
            if colour.0 != ink {
                *colour = TextColor(ink);
            }
        }
        // Over the head, and only while the camera can see them. The
        // node itself is a point of no size: the pill inside it is
        // centred on that point and overflows to whatever width the
        // word needs, so nothing is padded out into a black bar.
        match camera.world_to_viewport(camera_at, at.translation() + Vec3::Y * 2.05) {
            Ok(spot) => {
                node.left = Val::Px(spot.x);
                node.top = Val::Px(spot.y);
                *visibility = Visibility::Inherited;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }

    for (who, _, activity, vocation, child) in &folk {
        if worn.contains(&who) {
            continue;
        }
        let label = commands
            .spawn((
                Doing(who),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Visibility::Hidden,
                GlobalZIndex(6),
            ))
            .id();
        commands.spawn((
            (
                Text::new(say(activity, vocation, child)),
                TextFont {
                    font: fonts.text.clone().into(),
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(ink),
                TextLayout {
                    justify: Justify::Center,
                    ..default()
                },
            ),
            BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 0.68)),
            BorderColor::all(Color::BLACK.with_alpha(0.45)),
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                // The parent is a point with no width of its own; without
                // this the pill would be squeezed to nothing.
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(label),
        ));
    }
}
