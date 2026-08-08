//! The nameplates: a card over every head, in three states on one key.
//!
//! Off; the name alone; or the whole soul — trade and doing, faith, body,
//! house. Dressed like the notice toasts and the codex rather than like a
//! debug readout: the dark panel, a border in the faith's own colour, the
//! name set in the game's display face. Brett: "something that looks like
//! the notifications in the notification tray, or like a WoW tooltip.
//! Colored border, black background, colored name at top."
//!
//! The colour IS the reading. A believer's name burns gold, a waverer's
//! sits in bone, a doubter's goes out to ash — the one question a god has
//! about a crowd, answered before a single word is read.

use bevy::prelude::*;
use bevy::text::FontSize;

use crate::creature::genome::{Age, CreatureGenome};
use crate::creature::{Childhood, Corpse};
use crate::villager::belief::Faith;
use crate::villager::gossip::Conversing;
use crate::villager::work::Vocation;
use crate::villager::{Activity, Needs, Person, Villager};

/// What the nameplates are saying, if anything.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum Labels {
    #[default]
    Quiet,
    /// The name alone: reading a crowd.
    Names,
    /// The whole soul: name, trade and doing, faith, body, house.
    Cards,
}

/// Nameplates further than this from the eye are put away. Past it the
/// cards shrink to confetti and stack twelve deep over a busy square —
/// a plate you cannot read is clutter, not information.
const PLATE_REACH: f32 = 170.0;

/// One person's nameplate. The parent node is a point of no size pinned
/// over the head; every written part is remembered here so the tending
/// pass can reach each one directly instead of walking children.
#[derive(Component)]
struct Doing {
    who: Entity,
    card: Entity,
    name: Entity,
    /// The rule and the four lines exist only in [`Labels::Cards`].
    lines: [Entity; 4],
}

pub struct DoingsPlugin;

impl Plugin for DoingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Labels>()
            .add_systems(Update, (toggle_labels, tend_labels, call_the_house).chain());
    }
}

fn toggle_labels(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    mut mode: ResMut<Labels>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    if !keymap.just_pressed(&keys, crate::keymap::Deed::Doings) {
        return;
    }
    // One key, three states, round and round: off, names, the whole soul.
    let (next, said) = match *mode {
        Labels::Quiet => (Labels::Names, Some("Every soul wears their name")),
        Labels::Names => (Labels::Cards, Some("Every soul tells their whole story")),
        Labels::Cards => (Labels::Quiet, None),
    };
    *mode = next;
    if let Some(said) = said {
        let cap = crate::keymap::key_name(keymap.key(crate::keymap::Deed::Doings))
            .unwrap_or("the key");
        notices.write(crate::ui::Notice::new(format!("{said} - {cap} cycles")));
    }
}

/// The name's ink, and the card's border: the faith answers at a glance.
fn faith_ink(faith: Option<&Faith>) -> Color {
    match faith {
        Some(faith) if faith.trust > 0.5 => {
            crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.95)
        }
        Some(faith) if faith.trust > 0.25 => crate::palette::shade(&crate::palette::BONE, 0.97),
        Some(_) => crate::palette::shade(&crate::palette::STONE, 0.72),
        // The unendowed - newborns in their first breath - read as bone.
        None => crate::palette::shade(&crate::palette::BONE, 0.97),
    }
}

/// What a person is at, in the fewest words that still tell the truth.
fn doing_word(activity: &Activity, talk: Option<&Conversing>) -> &'static str {
    match activity {
        Activity::Idle => "idle",
        Activity::Wandering => "wandering",
        Activity::SeekingFood(_) => "foraging, hungry",
        Activity::Eating(_) => "eating off a bush",
        Activity::VisitingStore => "eating at the stores",
        Activity::Sleeping => "asleep",
        Activity::Working => "working",
        Activity::Praying => "praying",
        Activity::Sheltering => "sheltering",
        Activity::TendingFire => "tending the fire",
        // Most of a conversation is the walk to it: two people who have
        // agreed to speak can be a dozen strides apart and still be
        // Chatting. Saying "talking" over both of them is a lie the label
        // was telling all by itself.
        Activity::Chatting => match talk {
            Some(talk) if talk.spoke_at.is_some() => "talking",
            _ => "off to talk",
        },
        Activity::Mourning => "mourning",
        Activity::Hauling => "hauling",
        Activity::Bearing => "bearing the dead",
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

/// The body's state, said plainly. "Hale" earns its place: a card that
/// only spoke up in trouble read as a blank line the rest of the time.
fn body_words(needs: Option<&Needs>) -> (&'static str, bool) {
    let Some(needs) = needs else {
        return ("hale", false);
    };
    match (needs.hunger > 0.55, needs.rest > 0.65) {
        (true, true) => ("hungry and weary", true),
        (true, false) => ("hungry", true),
        (false, true) => ("weary", true),
        (false, false) => ("hale", false),
    }
}

/// Who they are to the village: their season of life and their house.
fn house_words(genome: Option<&CreatureGenome>, person: &Person) -> String {
    let season = match genome.map(|g| g.age) {
        Some(Age::Child) => "child",
        Some(Age::Elder) => "elder",
        _ => "adult",
    };
    if person.surname.is_empty() {
        format!("{season} of the village")
    } else {
        format!("{season} of house {}", person.surname)
    }
}

/// Keeps a nameplate over every living head while the plates are up, and
/// sweeps them all away when they go quiet. A mode change also sweeps:
/// the two dressed states build different cards, and rebuilding a handful
/// of nodes once per keypress is cheaper than carrying both layouts.
///
/// The pinning - projection, culling, reading range - is Ordo's
/// [`ordo::Placard`]; this system only keeps the WORDS true. The kit owns
/// the widget, the game owns the content.
#[allow(clippy::type_complexity)]
fn tend_labels(
    mut commands: Commands,
    mode: Res<Labels>,
    fonts: Option<Res<crate::ui::Fonts>>,
    folk: Query<
        (
            Entity,
            &Person,
            &Activity,
            Option<&Vocation>,
            Has<Childhood>,
            Option<&Conversing>,
            Option<&Faith>,
            Option<&Needs>,
            Option<&CreatureGenome>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
    labels: Query<(Entity, &Doing)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    mut borders: Query<&mut BorderColor>,
) {
    if *mode == Labels::Quiet || mode.is_changed() {
        for (label, ..) in &labels {
            commands.entity(label).despawn();
        }
        if *mode == Labels::Quiet {
            return;
        }
    }
    let Some(fonts) = fonts else {
        return;
    };
    let whole = *mode == Labels::Cards;

    // Everyone who already wears a plate: keep its words true.
    let mut worn: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for (label, plate) in &labels {
        let Ok((_, person, activity, vocation, child, talk, faith, needs, genome)) =
            folk.get(plate.who)
        else {
            commands.entity(label).despawn();
            continue;
        };
        worn.insert(plate.who);

        // The faith may have moved since the card was dressed.
        let ink = faith_ink(faith);
        if let Ok(mut border) = borders.get_mut(plate.card) {
            *border = BorderColor::all(ink.with_alpha(0.85));
        }
        if let Ok((_, mut colour)) = texts.get_mut(plate.name) {
            if colour.0 != ink {
                *colour = TextColor(ink);
            }
        }
        if !whole {
            continue;
        }
        let said = [
            format!("{} - {}", trade_of(vocation, child), doing_word(activity, talk)),
            faith.map_or("unproven", |f| f.describe()).to_string(),
            body_words(needs).0.to_string(),
            house_words(genome, person),
        ];
        for (index, (line, fresh)) in plate.lines.iter().zip(said).enumerate() {
            if let Ok((mut text, mut colour)) = texts.get_mut(*line) {
                if text.0 != fresh {
                    *text = Text::new(fresh);
                }
                // Faith and trouble carry their own colours; the rest is quiet.
                let wanted = match index {
                    1 => ink.with_alpha(0.88),
                    2 if body_words(needs).1 => {
                        crate::palette::shade(&crate::palette::BONE, 0.97)
                    }
                    _ => crate::ui::theme::text_dim(),
                };
                if colour.0 != wanted {
                    *colour = TextColor(wanted);
                }
            }
        }
    }

    // Everyone who does not: dress them a plate — Ordo's placard, with the
    // game's own words and faith-dyed border inside it.
    for (who, person, activity, vocation, child, talk, faith, needs, genome) in &folk {
        if worn.contains(&who) {
            continue;
        }
        let ink = faith_ink(faith);
        let parts = ordo::placard(&mut commands, who, 2.15, Some(PLATE_REACH), 55.0);
        // The border is the faith's to dye, live, not the theme's: the tag
        // comes off so the repaint pass never argues with it.
        commands
            .entity(parts.card)
            .remove::<ordo::Edge>()
            .insert(BorderColor::all(ink.with_alpha(0.85)));
        let card = parts.card;
        let name = commands
            .spawn((
                Text::new(person.full_name()),
                TextFont {
                    font: fonts.display_bold.clone().into(),
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(ink),
                TextLayout {
                    justify: Justify::Center,
                    ..default()
                },
                ChildOf(card),
            ))
            .id();

        let mut lines = [Entity::PLACEHOLDER; 4];
        if whole {
            // The hairline under the name, in the same ink: the card reads
            // as a title and its matter, not five equal strangers.
            commands.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(2.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(ink.with_alpha(0.35)),
                ChildOf(card),
            ));
            let said = [
                format!("{} - {}", trade_of(vocation, child), doing_word(activity, talk)),
                faith.map_or("unproven", |f| f.describe()).to_string(),
                body_words(needs).0.to_string(),
                house_words(genome, person),
            ];
            for (slot, words) in lines.iter_mut().zip(said) {
                *slot = commands
                    .spawn((
                        Text::new(words),
                        TextFont {
                            font: fonts.text.clone().into(),
                            font_size: FontSize::Px(12.5),
                            ..default()
                        },
                        TextColor(crate::ui::theme::text_dim()),
                        ChildOf(card),
                    ))
                    .id();
            }
        }
        commands.entity(parts.root).insert(Doing {
            who,
            card,
            name,
            lines,
        });
    }
}

/// The knock's answer, written over the door: how many of the household
/// were home. The same widget as the nameplates — Brett: "it could use the
/// same widget as the player cards do when you press L" — and the same
/// dress: the display face and a bright border, gold because a knock is
/// the god's own doing. It lives just long enough to be read, then goes
/// by Ordo's own ageing.
fn call_the_house(
    mut commands: Commands,
    fonts: Option<Res<crate::ui::Fonts>>,
    mut reports: MessageReader<crate::villager::home::KnockReport>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for report in reports.read() {
        let gold = crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.95);
        let parts = ordo::placard(&mut commands, report.building, 5.2, None, 70.0);
        commands
            .entity(parts.root)
            .insert(ordo::Lifetime::new(3.0, 0.5));
        commands
            .entity(parts.card)
            .remove::<ordo::Edge>()
            .insert(BorderColor::all(gold.with_alpha(0.85)));
        commands.spawn((
            Text::new(format!("{} of {} home", report.home, report.household)),
            TextFont {
                font: fonts.display_bold.clone().into(),
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(gold),
            TextLayout {
                justify: Justify::Center,
                ..default()
            },
            ChildOf(parts.card),
        ));
    }
}
