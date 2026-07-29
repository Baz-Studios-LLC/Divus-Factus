//! The hover inspector: whatever is beneath the divine hand.

use super::*;
use crate::creature::genome::Age;
use crate::hand::DivineHand;
use crate::witness::{Reaction, Temperament, Witnessed};

/// The inspector, bottom left: whoever the hand is over, read out as a person.
#[derive(Component)]
pub(crate) struct InspectorPanel;

#[derive(Component)]
pub(crate) struct InspectorName;

/// The line under the name: who they are in a phrase.
#[derive(Component)]
pub(crate) struct InspectorSubtitle;

/// One line of prose for subjects that are not living people — corpses, animals,
/// bushes. Hidden while a living person's rows are showing.
#[derive(Component)]
pub(crate) struct InspectorDetail;

/// Which live readout an inspector row shows.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectorValue {
    State,
    Manner,
    Hunger,
    Rest,
    Health,
    Spirits,
    Heart,
    FaithIn,
    Work,
    Family,
    Seen,
}

/// Everything shown only for a living person — the stat rows, the memory
/// section. One marker so the whole block can be shown and hidden together.
#[derive(Component)]
pub(crate) struct InspectorPersonBlock;

/// The recent-memories text, in the villager's own phrasing.
#[derive(Component)]
pub(crate) struct InspectorMemories;

/// The life-so-far text: the tail of this person's chronicle.
#[derive(Component)]
pub(crate) struct InspectorLife;

/// Fills the inspector with whoever the hand is over or carrying.
///
/// A living person gets the full dossier: state, hunger, health, heart, what
/// they have seen you do and how they would put it. This is the seed of the
/// doctrine panel — the point of names, temperaments and memories is that you
/// can look at one villager and find a person rather than a unit, and this is
/// where that person will eventually speak.
pub(crate) fn update_inspector(
    hand: Res<DivineHand>,
    follow: Res<crate::camera::FollowTarget>,
    people: Query<(
        &Person,
        &Temperament,
        &Witnessed,
        Option<&Reaction>,
        Option<&Needs>,
        Option<&Activity>,
        Option<&crate::creature::Vitality>,
        Option<&crate::creature::genome::CreatureGenome>,
        Option<&crate::villager::Spouse>,
        Option<&crate::villager::Parentage>,
        Option<&MemberOf>,
        Option<&Chronicle>,
        Option<&crate::villager::work::Vocation>,
        (
            Option<&Morale>,
            Option<&crate::villager::belief::Faith>,
            Option<&crate::villager::traits::Traits>,
        ),
    )>,
    corpse_check: Query<Option<&crate::creature::Vitality>, With<crate::creature::Corpse>>,
    cards: (
        Query<(&crate::villager::rites::Grave, &Person, &Chronicle), Without<Temperament>>,
        Query<&crate::villager::work::StorePile>,
        Option<Res<crate::villager::work::StoreTrends>>,
        Option<Res<crate::villager::SettlementSite>>,
        Query<&crate::villager::work::Building>,
    ),
    kin_names: Query<&Person>,
    settlements: Query<&Settlement>,
    huts: Query<(), With<crate::villager::work::Hut>>,
    rising: (
        Query<(
            &crate::villager::work::ConstructionSite,
            &crate::villager::work::Blueprint,
        )>,
        Query<&crate::matter::Deposit>,
        Query<(&Interaction, &ui::HoverHint)>,
    ),
    households: Query<
        (&Person, &crate::villager::home::Home, &Activity),
        Without<crate::creature::Corpse>,
    >,
    settlement_info: Query<(&Settlement, &crate::villager::work::Stockpile)>,
    residents: Query<
        (&MemberOf, &crate::creature::genome::CreatureGenome),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    names: Query<&Name>,
    mut panels: Query<&mut Visibility, (With<InspectorPanel>, Without<InspectorPersonBlock>)>,
    mut person_block: Query<
        (&mut Visibility, &mut Node),
        (With<InspectorPersonBlock>, Without<InspectorPanel>),
    >,
    mut texts: ParamSet<(
        Query<&mut Text, With<InspectorName>>,
        Query<&mut Text, With<InspectorSubtitle>>,
        Query<
            (&mut Text, &mut Visibility),
            (
                With<InspectorDetail>,
                Without<InspectorPanel>,
                Without<InspectorPersonBlock>,
            ),
        >,
        Query<(&InspectorValue, &mut Text)>,
        Query<&mut Text, With<InspectorMemories>>,
        Query<&mut Text, With<InspectorLife>>,
    )>,
) {
    let Ok(mut visibility) = panels.single_mut() else {
        return;
    };

    // ONE tooltip system: a hinted button under the cursor speaks through
    // the same top-corner card the world does. Interface wins over world
    // - if you are over a button, the button is what you are asking about.
    if let Some(hint) = rising
        .2
        .iter()
        .find(|(interaction, _)| !matches!(interaction, Interaction::None))
        .map(|(_, hint)| hint)
    {
        *visibility = Visibility::Visible;
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Hidden;
            node.display = Display::None;
        }
        if let Ok(mut name) = texts.p0().single_mut()
            && name.0 != hint.title
        {
            *name = Text::new(hint.title.clone());
        }
        if let Ok(mut subtitle) = texts.p1().single_mut()
            && subtitle.0 != hint.line
        {
            *subtitle = Text::new(hint.line.clone());
        }
        if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
            if !detail.0.is_empty() {
                *detail = Text::new("");
            }
            *detail_visibility = Visibility::Hidden;
        }
        return;
    }

    // Whoever the hand holds or hovers — and failing that, whoever the camera
    // is following. The card of a follow stays up for the whole ride.
    let subject = hand
        .held
        .as_ref()
        .map(|h| h.entity)
        .or(hand.hovered)
        .or(follow.entity);
    let Some(entity) = subject else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Visible;

    let held = hand.held.is_some();
    let corpse = corpse_check.get(entity);

    // A pile in the square: the store it fronts, and which way it is going.
    if let Ok(pile) = cards.1.get(entity) {
        use crate::villager::work::PileKind;
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Hidden;
            node.display = Display::None;
        }
        let store = cards
            .3
            .as_ref()
            .and_then(|site| settlement_info.get(site.settlement).ok())
            .map(|(_, store)| store);
        let (title, amount) = match (pile.0, store) {
            (PileKind::Food, Some(s)) => ("The food store", s.larder.total()),
            (PileKind::Timber, Some(s)) => ("The woodpile", s.timber),
            (PileKind::Stone, Some(s)) => ("The stone pile", s.stone),
            (_, None) => ("The stores", 0.0),
        };
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(title);
        }
        if let Ok(mut subtitle) = texts.p1().single_mut() {
            let fresh = match pile.0 {
                PileKind::Food => format!("{amount:.0} food laid by"),
                PileKind::Timber => format!("{amount:.0} logs on the pile"),
                PileKind::Stone => format!("{amount:.0} blocks cut and stacked"),
            };
            if subtitle.0 != fresh {
                *subtitle = Text::new(fresh);
            }
        }
        if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
            let rate = cards
                .2
                .as_ref()
                .map_or(0.0, |trends| trends.rate_per_minute(pile.0));
            let fresh = if rate > 0.5 {
                format!("growing - about {rate:.0} more each minute")
            } else if rate < -0.5 {
                format!("being drawn down - about {:.0} a minute", -rate)
            } else {
                "holding steady".to_string()
            };
            if detail.0 != fresh {
                *detail = Text::new(fresh);
            }
            *detail_visibility = Visibility::Inherited;
        }
        return;
    }

    // A storehouse or granary: what shelters under its roof, and the drift.
    if let Ok(building) = cards.4.get(entity) {
        use crate::villager::work::{BuildingKind, PileKind};
        let holds: &[(PileKind, &str)] = match building.kind {
            BuildingKind::Storehouse => &[(PileKind::Timber, "logs"), (PileKind::Stone, "stone")],
            BuildingKind::Granary => &[(PileKind::Food, "food")],
            _ => &[],
        };
        if !holds.is_empty() {
            for (mut block, mut node) in &mut person_block {
                *block = Visibility::Hidden;
                node.display = Display::None;
            }
            let store = cards
                .3
                .as_ref()
                .and_then(|site| settlement_info.get(site.settlement).ok())
                .map(|(_, store)| store);
            if let Ok(mut name) = texts.p0().single_mut() {
                *name = Text::new(building.kind.name());
            }
            if let Ok(mut subtitle) = texts.p1().single_mut() {
                let fresh = holds
                    .iter()
                    .map(|(kind, word)| {
                        let amount = store.map_or(0.0, |s| match kind {
                            PileKind::Food => s.food(),
                            PileKind::Timber => s.timber,
                            PileKind::Stone => s.stone,
                        });
                        // Food opens its sacks: the kinds inside, named.
                        if *kind == PileKind::Food
                            && let Some(s) = store
                        {
                            let kinds = [
                                crate::villager::work::FoodKind::Berries,
                                crate::villager::work::FoodKind::Fish,
                                crate::villager::work::FoodKind::Meat,
                                crate::villager::work::FoodKind::Grain,
                                crate::villager::work::FoodKind::Bread,
                            ]
                            .into_iter()
                            .filter(|k| s.larder.stock(*k) >= 0.5)
                            .map(|k| format!("{:.0} {}", s.larder.stock(k), k.name()))
                            .collect::<Vec<_>>()
                            .join(", ");
                            if kinds.is_empty() {
                                format!("{amount:.0} {word}")
                            } else {
                                format!("{amount:.0} {word} ({kinds})")
                            }
                        } else {
                            format!("{amount:.0} {word}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let fresh = format!("{fresh} under its roof");
                if subtitle.0 != fresh {
                    *subtitle = Text::new(fresh);
                }
            }
            if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
                let fresh = holds
                    .iter()
                    .map(|(kind, word)| {
                        let rate = cards
                            .2
                            .as_ref()
                            .map_or(0.0, |trends| trends.rate_per_minute(*kind));
                        if rate > 0.5 {
                            format!("{word}: growing, about {rate:.0} a minute")
                        } else if rate < -0.5 {
                            format!("{word}: being drawn down, {:.0} a minute", -rate)
                        } else {
                            format!("{word}: holding steady")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if detail.0 != fresh {
                    *detail = Text::new(fresh);
                }
                *detail_visibility = Visibility::Inherited;
            }
            return;
        }
    }

    // A grave: the life that ended under it, read back from the stone.
    if let Ok((grave, person, story)) = cards.0.get(entity) {
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Hidden;
            node.display = Display::None;
        }
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(format!("The grave of {}", person.name));
        }
        if let Ok(mut subtitle) = texts.p1().single_mut() {
            *subtitle = Text::new(format!("laid to rest on day {}", grave.day));
        }
        if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
            let tail = story.events.len().saturating_sub(6);
            let life = story.events[tail..]
                .iter()
                .map(|event| format!("d{}  {}", event.day, event.text))
                .collect::<Vec<_>>()
                .join("\n");
            *detail = Text::new(life);
            *detail_visibility = Visibility::Inherited;
        }
        return;
    }

    // A living person: the full dossier.
    if let Ok((
        person,
        temperament,
        witnessed,
        reaction,
        needs,
        activity,
        vitality,
        genome,
        spouse,
        parentage,
        member_of,
        chronicle,
        vocation,
        (morale, faith, manner),
    )) = people.get(entity)
        && corpse.is_err()
    {
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Inherited;
            node.display = Display::Flex;
        }
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(person.name.clone());
        }

        let who = genome.map_or("a soul", |g| person_phrase(g.sex, g.age));
        let home = member_of
            .and_then(|m| settlements.get(m.0).ok())
            .map_or_else(|| "the wilds".to_string(), |s| s.name.clone());
        if let Ok(mut subtitle) = texts.p1().single_mut() {
            *subtitle = Text::new(format!("{who} of {home}"));
        }
        if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
            *detail = Text::new("");
            *detail_visibility = Visibility::Hidden;
        }

        let hunger = needs.map_or(0.0, |n| n.hunger);
        let harm = vitality.map_or(0.0, |v| v.harm);
        for (value, mut text) in &mut texts.p3() {
            let fresh = match value {
                InspectorValue::State => if held {
                    "in your grasp"
                } else {
                    state_phrase(activity, reaction)
                }
                .to_string(),
                InspectorValue::Hunger => hunger_word(hunger).to_string(),
                InspectorValue::Rest => needs.map_or("wakeful", |n| rest_word(n.rest)).to_string(),
                InspectorValue::Health => health_word(harm).to_string(),
                InspectorValue::Spirits => morale
                    .map_or("steady", |m| spirits_word(m.spirits))
                    .to_string(),
                InspectorValue::Heart => temperament.describe().to_string(),
                InspectorValue::Manner => {
                    manner.map_or("unremarkable".to_string(), |m| m.describe())
                }
                InspectorValue::FaithIn => faith
                    .map_or("has never wondered", |f| f.describe())
                    .to_string(),
                InspectorValue::Work => vocation.map_or("none yet", |v| v.describe()).to_string(),
                InspectorValue::Family => {
                    family_phrase(spouse, parentage, &kin_names, &corpse_check)
                }
                InspectorValue::Seen => {
                    if witnessed.is_innocent() && witnessed.secondhand > 0 {
                        "only in stories".to_string()
                    } else if witnessed.is_innocent() {
                        "never".to_string()
                    } else {
                        format!("{} times", witnessed.total)
                    }
                }
            };
            if text.0 != fresh {
                *text = Text::new(fresh);
            }
        }

        if let Ok(mut memories) = texts.p4().single_mut() {
            let fresh = if witnessed.recent.is_empty() {
                "nothing they could not explain".to_string()
            } else {
                witnessed
                    .recent
                    .iter()
                    .take(4)
                    .map(|kind| format!("- {}", kind.describe()))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if memories.0 != fresh {
                *memories = Text::new(fresh);
            }
        }

        if let Ok(mut life) = texts.p5().single_mut() {
            let fresh = chronicle.map_or_else(
                || "unwritten".to_string(),
                |chronicle| {
                    let events = &chronicle.events;
                    let tail = events.len().saturating_sub(4);
                    events[tail..]
                        .iter()
                        .map(|event| format!("d{}  {}", event.day, event.text))
                        .collect::<Vec<_>>()
                        .join("\n")
                },
            );
            if life.0 != fresh {
                *life = Text::new(fresh);
            }
        }
        return;
    }

    // Anything else — a corpse, an animal, a bush — gets a name and one line.
    for (mut block, mut node) in &mut person_block {
        *block = Visibility::Hidden;
        node.display = Display::None;
    }

    let (title, description) = if huts.get(entity).is_ok() {
        // A finished house: the household, and what each of them is up to.
        let village = settlements
            .iter()
            .next()
            .map_or_else(|| "the village".to_string(), |s| s.name.clone());
        let residents: Vec<String> = households
            .iter()
            .filter(|(_, home, _)| home.0 == entity)
            .map(|(person, _, activity)| {
                format!("{} - {}", person.name, state_phrase(Some(activity), None))
            })
            .collect();
        (
            format!("A house of {village}"),
            if residents.is_empty() {
                "no one yet calls it home".to_string()
            } else {
                residents.join("\n")
            },
        )
    } else if let Ok((construction, plan)) = rising.0.get(entity) {
        // Say what the site is actually waiting on: a foundation short of
        // stone blocks the carpenters, and that must be legible, or an
        // honest wait reads as a broken village.
        let stone_cost = plan.kind.stone_cost();
        let line = if construction.stone_laid < stone_cost {
            format!(
                "waiting on stone - {:.0} of {:.0} laid in the foundation",
                construction.stone_laid, stone_cost,
            )
        } else {
            format!(
                "{:.0} of {:.0} {} worked into it",
                construction.progress.min(plan.kind.timber_cost()),
                plan.kind.timber_cost(),
                plan.stuff.word(),
            )
        };
        (format!("{}, rising", plan.kind.name()), line)
    } else if let Ok((settlement, store)) = settlement_info.get(entity) {
        // The banner: the settlement's own dossier.
        let mut grown = 0;
        let mut children = 0;
        for (member, genome) in &residents {
            if member.0 == entity {
                match genome.age {
                    Age::Child => children += 1,
                    _ => grown += 1,
                }
            }
        }
        (
            settlement.name.clone(),
            format!(
                "a village, founded on day {}\n\
                 {grown} grown, {children} children\n\
                 stores  {:.0} food, {:.0} timber, {:.0} stone",
                settlement.founded,
                store.food(),
                store.timber,
                store.stone,
            ),
        )
    } else if let Ok(vitality) = corpse {
        let name = people
            .get(entity)
            .map(|(person, ..)| format!("the body of {}", person.name))
            .unwrap_or_else(|_| "a body".to_string());
        let cause = match vitality {
            Some(v) if v.violent => "broken against the earth",
            Some(_) => "wasted away by hunger",
            None => "still",
        };
        (name, cause.to_string())
    } else if let Ok(deposit) = rising.1.get(entity) {
        // The god reads the ground itself: what it is, and how much the
        // village could still carry out of it.
        (
            deposit.kind.title().to_string(),
            format!("{:.0} loads left in the ground", deposit.amount),
        )
    } else {
        let what = names
            .get(entity)
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|_| "something".into());
        (
            what,
            if held {
                "in your grasp"
            } else {
                "beneath your hand"
            }
            .to_string(),
        )
    };

    if let Ok(mut name) = texts.p0().single_mut() {
        *name = Text::new(title);
    }
    if let Ok(mut subtitle) = texts.p1().single_mut() {
        *subtitle = Text::new("");
    }
    if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
        *detail = Text::new(description);
        *detail_visibility = Visibility::Inherited;
    }
}
