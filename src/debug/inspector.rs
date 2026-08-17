//! The hover inspector: whatever is beneath the divine hand.

use super::*;
use crate::creature::genome::Age;
use crate::hand::DivineHand;
use crate::witness::{Reaction, Temperament, Witnessed};
use bevy::ecs::system::SystemParam;

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
    /// The social weather: whom they hold dearest, whom they cannot stand.
    Feelings,
    Seen,
}

/// The social weather in one line: the strongest of each pole, by name —
/// "fond of Doenna - sour on Marruck". A heart with nothing notable says
/// so. Shared by the hover inspector and the People page dossier.
pub(crate) fn feelings_phrase(
    regard: Option<&crate::villager::regard::Regard>,
    names: &Query<&crate::villager::Person>,
) -> String {
    let name_of = |bond: &crate::villager::regard::Bond| {
        names
            .get(bond.toward)
            .map(|p| p.name.clone())
            .ok()
            .zip(crate::villager::regard::band(bond.warmth))
            .map(|(name, word)| format!("{word} {name}"))
    };
    let poles: Vec<String> = regard
        .into_iter()
        .flat_map(|r| [r.fondest(), r.sourest()])
        .flatten()
        .filter_map(name_of)
        .collect();
    if poles.is_empty() {
        "nothing notable".to_string()
    } else {
        poles.join(" - ")
    }
}

/// Everything shown only for a living person — the stat rows, the memory
/// section. One marker so the whole block can be shown and hidden together.
#[derive(Component)]
pub(crate) struct InspectorPersonBlock;

/// Everything that belongs only to a dwelling's quick household card. Keeping
/// it separate from the person dossier lets one inspector become a composed
/// information surface instead of a single block of equally loud prose.
#[derive(Component)]
pub(crate) struct InspectorHouseBlock;

/// The live values in a dwelling card.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectorHouseValue {
    Beds,
    Stores,
    Mood,
    Faith,
    Life,
    Concern,
}

/// The concern row disappears entirely when this home has nothing pressing.
#[derive(Component)]
pub(crate) struct InspectorHouseConcern;

/// The recent-memories text, in the villager's own phrasing.
#[derive(Component)]
pub(crate) struct InspectorMemories;

/// The life-so-far text: the tail of this person's chronicle.
#[derive(Component)]
pub(crate) struct InspectorLife;

/// One window on every soul the corner card can speak of: the dossier
/// tuple, the corpse check, and the names that kin and houses go by.
#[derive(SystemParam)]
pub(crate) struct SoulSight<'w, 's> {
    people: Query<
        'w,
        's,
        (
            &'static Person,
            &'static Temperament,
            &'static Witnessed,
            Option<&'static Reaction>,
            Option<&'static Needs>,
            Option<&'static Activity>,
            Option<&'static crate::creature::Vitality>,
            Option<&'static crate::creature::genome::CreatureGenome>,
            Option<&'static crate::villager::Spouse>,
            Option<&'static crate::villager::Parentage>,
            Option<&'static MemberOf>,
            Option<&'static Chronicle>,
            Option<&'static crate::villager::work::Vocation>,
            (
                Option<&'static Morale>,
                Option<&'static crate::villager::belief::Faith>,
                Option<&'static crate::villager::traits::Traits>,
                Option<&'static crate::villager::speech::RecentlySaid>,
                Option<&'static crate::villager::regard::Regard>,
            ),
        ),
    >,
    corpse_check:
        Query<'w, 's, Option<&'static crate::creature::Vitality>, With<crate::creature::Corpse>>,
    kin_names: Query<'w, 's, &'static Person>,
    households: Query<
        'w,
        's,
        (
            &'static Person,
            &'static crate::villager::home::Home,
            &'static Activity,
            &'static MemberOf,
            Option<&'static Needs>,
            Option<&'static Morale>,
            Option<&'static crate::villager::belief::Faith>,
            Option<&'static crate::creature::genome::CreatureGenome>,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    residents: Query<
        'w,
        's,
        (
            &'static MemberOf,
            &'static crate::creature::genome::CreatureGenome,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    names: Query<'w, 's, &'static Name>,
}

/// The town around whatever the hand is over: its name, its stores, and
/// which roofs are whose.
#[derive(SystemParam)]
pub(crate) struct TownSight<'w, 's> {
    settlements: Query<'w, 's, &'static Settlement>,
    settlement_info: Query<
        'w,
        's,
        (
            &'static Settlement,
            &'static crate::villager::work::Stockpile,
        ),
    >,
    huts: Query<
        'w,
        's,
        Has<crate::villager::work::Longhouse>,
        Or<(
            With<crate::villager::work::Hut>,
            With<crate::villager::work::Longhouse>,
        )>,
    >,
}

/// The still-life cards: graves, piles, buildings, and the stores they
/// front. What the tuple positions used to hide, named.
#[derive(SystemParam)]
pub(crate) struct StillCards<'w, 's> {
    graves: Query<
        'w,
        's,
        (
            &'static crate::villager::rites::Grave,
            &'static Person,
            &'static Chronicle,
        ),
        Without<Temperament>,
    >,
    piles: Query<'w, 's, &'static crate::villager::work::StorePile>,
    trends: Option<Res<'w, crate::villager::work::StoreTrends>>,
    site: Option<Res<'w, crate::villager::SettlementSite>>,
    buildings: Query<'w, 's, &'static crate::villager::work::Building>,
}

/// What is not yet or no longer a person under the hand: works rising,
/// the ground's gifts, hinted buttons - and the book whose opening
/// smothers the card.
#[derive(SystemParam)]
pub(crate) struct RisingSight<'w, 's> {
    builds: Query<
        'w,
        's,
        (
            &'static crate::villager::work::ConstructionSite,
            &'static crate::villager::work::Blueprint,
        ),
    >,
    deposits: Query<'w, 's, &'static crate::matter::Deposit>,
    trees: Query<'w, 's, &'static crate::scatter::FellableTree>,
    boulders: Query<'w, 's, &'static crate::matter::Matter, With<crate::matter::Boulder>>,
    food_sources: Query<'w, 's, &'static crate::scatter::FoodSource>,
    fires: Query<'w, 's, &'static crate::villager::home::Bonfire>,
    hints: Query<'w, 's, (&'static Interaction, &'static ui::HoverHint)>,
    /// What a held thing is WORTH, for the card.
    worth: Query<
        'w,
        's,
        (
            Option<&'static crate::matter::Matter>,
            Option<&'static crate::matter::Lump>,
            Option<&'static crate::scatter::FoodSource>,
            Option<&'static crate::scatter::SacredFlora>,
        ),
    >,
    /// The book, whose opening smothers the card. Disjoint from EVERY
    /// Visibility this system writes - panel, person block AND the
    /// detail line - or B0001 panics at boot.
    book: Query<
        'w,
        's,
        &'static Visibility,
        (
            With<crate::debug::village::VillagePanel>,
            Without<InspectorPanel>,
            Without<InspectorPersonBlock>,
            Without<InspectorDetail>,
            Without<InspectorHouseBlock>,
        ),
    >,
}

/// The card itself - every surface the inspector writes.
#[derive(SystemParam)]
pub(crate) struct InspectorInk<'w, 's> {
    panels: Query<
        'w,
        's,
        &'static mut Visibility,
        (With<InspectorPanel>, Without<InspectorPersonBlock>),
    >,
    person_block: Query<
        'w,
        's,
        (&'static mut Visibility, &'static mut Node),
        (
            With<InspectorPersonBlock>,
            Without<InspectorPanel>,
            Without<InspectorHouseBlock>,
        ),
    >,
    house_block: Query<
        'w,
        's,
        (
            &'static mut Visibility,
            &'static mut Node,
            Option<&'static InspectorHouseConcern>,
        ),
        (
            With<InspectorHouseBlock>,
            Without<InspectorPanel>,
            Without<InspectorPersonBlock>,
        ),
    >,
    texts: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, &'static mut Text, With<InspectorName>>,
            Query<'w, 's, &'static mut Text, With<InspectorSubtitle>>,
            Query<
                'w,
                's,
                (&'static mut Text, &'static mut Visibility),
                (
                    With<InspectorDetail>,
                    Without<InspectorPanel>,
                    Without<InspectorPersonBlock>,
                    Without<InspectorHouseBlock>,
                ),
            >,
            Query<'w, 's, (&'static InspectorValue, &'static mut Text)>,
            Query<'w, 's, &'static mut Text, With<InspectorMemories>>,
            Query<'w, 's, &'static mut Text, With<InspectorLife>>,
            Query<
                'w,
                's,
                (&'static InspectorHouseValue, &'static mut Text),
                (
                    With<InspectorHouseBlock>,
                    Without<InspectorName>,
                    Without<InspectorSubtitle>,
                    Without<InspectorDetail>,
                    Without<InspectorMemories>,
                    Without<InspectorLife>,
                    Without<InspectorValue>,
                ),
            >,
        ),
    >,
}

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
    souls: SoulSight,
    towns: TownSight,
    cards: StillCards,
    rising: RisingSight,
    ink: InspectorInk,
) {
    let SoulSight {
        people,
        corpse_check,
        kin_names,
        households,
        residents,
        names,
    } = souls;
    let TownSight {
        settlements,
        settlement_info,
        huts,
    } = towns;
    let InspectorInk {
        mut panels,
        mut person_block,
        mut house_block,
        mut texts,
    } = ink;
    let Ok(mut visibility) = panels.single_mut() else {
        return;
    };

    // House-only furniture stays out of every other card. `Display::None`
    // matters as much as hidden visibility here: an empty section still has
    // height, which is how a card develops mysterious blank gutters.
    for (mut block, mut node, _) in &mut house_block {
        *block = Visibility::Hidden;
        node.display = Display::None;
    }

    // The open book smothers every tooltip: the codex covers the world,
    // and its own controls explain themselves. Brett: "nothing in the
    // codex needs a tooltip since it covers them anyway."
    if rising.book.iter().any(|v| *v != Visibility::Hidden) {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    // ONE tooltip system: a hinted button under the cursor speaks through
    // the same top-corner card the world does. Interface wins over world
    // - if you are over a button, the button is what you are asking about.
    if let Some(hint) = rising
        .hints
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
    //
    // Unless the god is INSIDE them, in which case there is no watching going
    // on and nothing to put on a card. A dossier of your own hunger and
    // spirits, pinned open in the corner while you walk about in the body it
    // describes, gives the whole conceit away — and it cannot be dismissed,
    // because the follow that raises it is the possession itself.
    let worn = follow.style == crate::camera::FollowStyle::Eyes;
    let subject = hand
        .held
        .as_ref()
        .map(|h| h.entity)
        .or(hand.hovered)
        .or_else(|| follow.entity.filter(|_| !worn));
    let Some(entity) = subject else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Visible;

    let held = hand.held.is_some();
    let corpse = corpse_check.get(entity);

    // A pile in the square: the store it fronts, and which way it is going.
    if let Ok(pile) = cards.piles.get(entity) {
        use crate::villager::work::PileKind;
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Hidden;
            node.display = Display::None;
        }
        let store = cards
            .site
            .as_ref()
            .and_then(|site| settlement_info.get(site.settlement).ok())
            .map(|(_, store)| store);
        let (title, amount) = match (pile.0, store) {
            (PileKind::Food, Some(s)) => ("The food store", s.larder.total()),
            (PileKind::Timber, Some(s)) => ("The woodpile", s.timber),
            (PileKind::Stone, Some(s)) => ("The stone pile", s.stone),
            (PileKind::Clay, Some(s)) => ("The clay pile", s.clay),
            (PileKind::Ore, Some(s)) => ("The ore heap", s.ore),
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
                PileKind::Clay => format!("{amount:.0} loads of clay, puddled and stacked"),
                PileKind::Ore => format!("{amount:.0} loads of ore for the fire"),
            };
            if subtitle.0 != fresh {
                *subtitle = Text::new(fresh);
            }
        }
        if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
            let rate = cards
                .trends
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
    if let Ok(building) = cards.buildings.get(entity) {
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
                .site
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
                            PileKind::Clay => s.clay,
                            PileKind::Ore => s.ore,
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
                            .trends
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
    if let Ok((grave, person, story)) = cards.graves.get(entity) {
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Hidden;
            node.display = Display::None;
        }
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(format!("The grave of {}", person.full_name()));
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
        (morale, faith, manner, said, regard),
    )) = people.get(entity)
        && corpse.is_err()
    {
        for (mut block, mut node) in &mut person_block {
            *block = Visibility::Inherited;
            node.display = Display::Flex;
        }
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(person.full_name());
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
                InspectorValue::Feelings => feelings_phrase(regard, &kin_names),
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
                    .map(|memory| match &memory.whom {
                        // The memory knows who it befell: "saw someone hurled
                        // across the ground — Feitreh, your neighbor".
                        Some(whom) => {
                            format!("- {} — {}", memory.kind.describe(), whom.phrase())
                        }
                        None => format!("- {}", memory.kind.describe()),
                    })
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
            // And under the life, the last few things out of their mouth.
            // The chronicle keeps what they DID and what they were told;
            // this is what they have been saying, which is the question a
            // person's page is really being asked.
            let fresh = match said.filter(|said| !said.0.is_empty()) {
                Some(said) => {
                    let tail = said.0.len().saturating_sub(3);
                    let lately = said.0[tail..]
                        .iter()
                        .map(|u| {
                            if u.thought {
                                format!("d{}  ({})", u.day, u.text)
                            } else {
                                format!("d{}  \"{}\"", u.day, u.text)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{fresh}\n\nlately\n{lately}")
                }
                None => fresh,
            };
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

    if let Ok(longhouse) = huts.get(entity) {
        // A finished roof is a household, not merely a list of sleepers. The
        // card gives the god one quick read of who belongs here, whether there
        // is room, how their shared town is provisioned, and what is pressing
        // on them right now.
        let village = settlements
            .iter()
            .next()
            .map_or_else(|| "the village".to_string(), |s| s.name.clone());
        let household: Vec<_> = households
            .iter()
            .filter(|(_, home, ..)| home.0 == entity)
            .collect();
        let resident_count = household.len();
        let children = household
            .iter()
            .filter(|(_, _, _, _, _, _, _, genome)| {
                genome.is_some_and(|genome| genome.age == Age::Child)
            })
            .count();
        let sleeping = household
            .iter()
            .filter(|(_, _, activity, ..)| matches!(activity, Activity::Sleeping))
            .count();
        let capacity = if longhouse {
            crate::villager::home::LONGHOUSE_CAPACITY
        } else {
            crate::villager::home::HOUSE_CAPACITY
        };
        let household_town = household.first().map(|(_, _, _, member, ..)| member.0);
        let mouths = household_town.map_or(0, |town| {
            residents
                .iter()
                .filter(|(member, _)| member.0 == town)
                .count()
        });
        let store = household_town.and_then(|town| settlement_info.get(town).ok().map(|(_, s)| s));
        let average_spirits = household
            .iter()
            .filter_map(|(_, _, _, _, _, morale, ..)| morale.map(|morale| morale.spirits))
            .sum::<f32>();
        let spirits_count = household
            .iter()
            .filter(|(_, _, _, _, _, morale, ..)| morale.is_some())
            .count();
        let average_faith = household
            .iter()
            .filter_map(|(_, _, _, _, _, _, faith, _)| faith.map(|faith| faith.trust))
            .sum::<f32>();
        let faith_count = household
            .iter()
            .filter(|(_, _, _, _, _, _, faith, _)| faith.is_some())
            .count();
        // A house is a FAMILY'S: it bears their name, not the town's. The
        // longhouse, which belongs to no one family, keeps the village's.
        let family = household
            .iter()
            .map(|(person, ..)| person.surname.clone())
            .find(|surname| !surname.is_empty());
        let title = if longhouse {
            format!("The longhouse of {village}")
        } else {
            match family {
                Some(name) => format!("The house of {name}"),
                None => format!("An empty house of {village}"),
            }
        };
        if let Ok(mut name) = texts.p0().single_mut() {
            *name = Text::new(title);
        }
        if household.is_empty() {
            let empty = if longhouse {
                "No one has taken a bed here yet."
            } else {
                "No one yet calls it home."
            };
            if let Ok(mut subtitle) = texts.p1().single_mut() {
                *subtitle = Text::new("UNCLAIMED");
            }
            if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
                *detail = Text::new(empty);
                *detail_visibility = Visibility::Inherited;
            }
            return;
        } else {
            for (mut block, mut node, concern) in &mut house_block {
                if concern.is_none() {
                    *block = Visibility::Inherited;
                    node.display = Display::Flex;
                }
            }
            if let Ok(mut subtitle) = texts.p1().single_mut() {
                *subtitle = Text::new(format!(
                    "{}, {}, {}",
                    counted(resident_count, "resident", "residents"),
                    counted(children, "child", "children"),
                    sleep_phrase(sleeping),
                ));
            }
            if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
                *detail = Text::new("");
                *detail_visibility = Visibility::Hidden;
            }
            let beds = if resident_count > capacity {
                format!("Crowded: {resident_count} people for {capacity} beds")
            } else {
                let free = capacity - resident_count;
                format!(
                    "{resident_count} of {capacity} filled - {}",
                    counted(free, "bed free", "beds free")
                )
            };
            let stores = store.map(|store| {
                format!(
                    "food {} - timber {}",
                    food_horizon(store.food(), mouths),
                    timber_horizon(store.timber, mouths),
                )
            });
            let mood =
                (spirits_count > 0).then(|| household_mood(average_spirits / spirits_count as f32));
            let faith =
                (faith_count > 0).then(|| household_faith(average_faith / faith_count as f32));
            let life = household
                .iter()
                .take(2)
                .map(|(person, _, activity, ..)| {
                    format!("{} is {}.", person.name, state_phrase(Some(activity), None))
                })
                .collect::<Vec<_>>()
                .join("\n");
            let concern = household
                .iter()
                .filter_map(|(person, _, _, _, needs, ..)| {
                    needs
                        .filter(|needs| needs.hunger >= 0.6)
                        .map(|needs| (person.name.as_str(), needs.hunger))
                })
                .max_by(|(_, left), (_, right)| left.total_cmp(right))
                .map(|(name, hunger)| {
                    if hunger >= 0.85 {
                        format!("{name} is starving")
                    } else {
                        format!("{name} is hungry")
                    }
                })
                .or_else(|| {
                    (resident_count > capacity).then(|| "There are not enough beds".to_string())
                })
                .or_else(|| {
                    store
                        .filter(|store| mouths > 0 && store.food() / (mouths as f32) < 1.0)
                        .map(|_| "The village food store is running low".to_string())
                })
                .or_else(|| {
                    (spirits_count > 0 && average_spirits / (spirits_count as f32) < 0.35)
                        .then(|| "Their spirits are low".to_string())
                });
            for (value, mut text) in &mut texts.p6() {
                let fresh = match value {
                    InspectorHouseValue::Beds => beds.clone(),
                    InspectorHouseValue::Stores => {
                        stores.clone().unwrap_or_else(|| "unread".to_string())
                    }
                    InspectorHouseValue::Mood => mood.unwrap_or("unread").to_string(),
                    InspectorHouseValue::Faith => faith.unwrap_or("unread").to_string(),
                    InspectorHouseValue::Life => life.clone(),
                    InspectorHouseValue::Concern => concern.clone().unwrap_or_default(),
                };
                if text.0 != fresh {
                    *text = Text::new(fresh);
                }
            }
            if concern.is_some() {
                for (mut block, mut node, concern) in &mut house_block {
                    if concern.is_some() {
                        *block = Visibility::Inherited;
                        node.display = Display::Flex;
                    }
                }
            }
            return;
        }
    }

    let (title, description) = if let Ok((construction, plan)) = rising.builds.get(entity) {
        // This is a needs list, not a progress report. Completed materials
        // disappear so the card answers the useful question: what still has
        // to arrive before this building can be finished?
        let footing = construction.footing_stone(plan.kind);
        let frame = plan.kind.timber_cost();
        let mut missing: Vec<(String, f32, f32)> = Vec::new();
        let mut want = |name: &str, have: f32, needs: f32| {
            if needs > 0.0 && have < needs {
                missing.push((name.to_string(), have, needs));
            }
        };
        // The walls take their material's name; the footing is always stone
        // and is called what it is, which keeps the two apart on a stone
        // building without either line having to explain itself.
        let mut stuff = plan.stuff.word().to_string();
        stuff[..1].make_ascii_uppercase();
        want(&stuff, construction.progress.min(frame), frame);
        want("Footing", construction.stone_laid.min(footing), footing);
        // Numbers and nothing else. Brett: "The building a building tooltip
        // doesn't need to say anything except what it is missing lol. If the
        // timber is 7/7 for example that is all they need to put."
        let lines: Vec<String> = missing
            .iter()
            .map(|(name, have, needs)| format!("{name} {have:.0}/{needs:.0}"))
            .collect();
        (format!("{}, rising", plan.kind.name()), lines.join("\n"))
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
        let population = match children {
            0 => format!("{}, no children", counted(grown, "adult", "adults")),
            _ => format!(
                "{}, {}",
                counted(grown, "adult", "adults"),
                counted(children, "child", "children")
            ),
        };
        let mouths = grown + children;
        let (houses, longhouses) = huts.iter().fold((0usize, 0usize), |(houses, halls), long| {
            if long {
                (houses, halls + 1)
            } else {
                (houses + 1, halls)
            }
        });
        let beds = houses * crate::villager::home::HOUSE_CAPACITY
            + longhouses * crate::villager::home::LONGHOUSE_CAPACITY;
        let materials = [
            (store.timber, "timber"),
            (store.stone, "stone"),
            (store.clay, "clay"),
            (store.ore, "ore"),
        ]
        .into_iter()
        .filter(|(amount, _)| *amount >= 0.5)
        .map(|(amount, kind)| format!("{amount:.0} {kind}"))
        .collect::<Vec<_>>();
        let stores = if materials.is_empty() {
            "No building materials laid by".to_string()
        } else {
            format!("Materials: {}", materials.join(", "))
        };
        let shelter = match beds.saturating_sub(mouths) {
            0 => format!("Shelter: {beds} beds, all occupied"),
            free => format!(
                "Shelter: {beds} beds - {}",
                counted(free, "bed free", "beds free")
            ),
        };
        (
            settlement.name.clone(),
            format!(
                "Founded on day {}\n{population}\n{shelter}\nFood: {:.0} ({})\n{stores}",
                settlement.founded,
                store.food(),
                food_horizon(store.food(), mouths),
            ),
        )
    } else if let Ok(vitality) = corpse {
        let name = people
            .get(entity)
            .map(|(person, ..)| format!("the body of {}", person.full_name()))
            .unwrap_or_else(|_| "a body".to_string());
        let cause = match vitality {
            Some(v) if v.violent => v.undoing.how(),
            Some(_) => "wasted away by hunger",
            None => "still",
        };
        (name, cause.to_string())
    } else if let Ok(deposit) = rising.deposits.get(entity) {
        // The god reads the ground itself: what it is, and how much the
        // village could still carry out of it.
        (
            deposit.kind.title().to_string(),
            format!("{:.0} loads still worth working", deposit.amount),
        )
    } else if let Ok(fire) = rising.fires.get(entity) {
        let tending = fire
            .tender
            .and_then(|tender| kin_names.get(tender).ok())
            .map(|person| format!("{} is tending it.", person.name));
        let state = match fire.fuel {
            fuel if fuel <= 0.0 => "Cold. It needs a log before nightfall.".to_string(),
            fuel if fuel < crate::villager::home::SECONDS_PER_LOG * 0.35 => {
                "Burning low. It will need wood soon.".to_string()
            }
            fuel => format!(
                "Burning steadily for about {} more minutes.",
                (fuel / 60.0).ceil()
            ),
        };
        (
            "The village fire".to_string(),
            tending.map_or(state.clone(), |tending| format!("{state}\n{tending}")),
        )
    } else if let Ok(tree) = rising.trees.get(entity) {
        let description = match tree.maturity {
            maturity if maturity >= 0.95 => "Mature timber. A forester can fell it.",
            maturity if maturity >= 0.55 => "Young growth. It will make better timber with time.",
            _ => "A sapling taking root.",
        };
        ("A tree".to_string(), description.to_string())
    } else if let Ok(source) = rising.food_sources.get(entity) {
        let description = match source.amount {
            amount if amount < 0.5 => "Picked bare. Its berries will grow back.".to_string(),
            amount => format!(
                "A gatherer can collect about {:.0} meals of berries.",
                amount.floor()
            ),
        };
        ("A berry bush".to_string(), description)
    } else if let Ok(boulder) = rising.boulders.get(entity) {
        let (_, stone) = crate::villager::work::offering_worth(boulder);
        let description = if boulder.radius >= 1.6 {
            format!("A rich source of stone. About {stone:.0} loads can be worked from it.")
        } else {
            format!("Loose building stone. About {stone:.0} loads can be carried away.")
        };
        (
            if boulder.radius >= 1.6 {
                "An outcrop"
            } else {
                "A boulder"
            }
            .to_string(),
            description,
        )
    } else {
        let what = names
            .get(entity)
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|_| "something".into());
        // A held resource says what it is worth, so the god knows what the
        // fist is carrying before it opens - "the tooltip should say how
        // much of what resource you have in your hand".
        let worth = held
            .then(|| rising.worth.get(entity).ok())
            .flatten()
            .and_then(|(matter, lump, food, sacred)| {
                if let Some(lump) = lump {
                    let kind = match lump.kind {
                        crate::matter::DepositKind::Clay => "clay",
                        crate::matter::DepositKind::Iron => "ore",
                        crate::matter::DepositKind::Stone => "stone",
                    };
                    Some(format!("{:.0} {kind}", lump.amount))
                } else if let Some(flora) = sacred {
                    let kind = match flora.kind {
                        crate::scatter::SacredKind::Incense => "incense",
                        crate::scatter::SacredKind::Dye => "dye",
                    };
                    Some(format!("{:.0} {kind}", flora.amount))
                } else if let Some(source) = food {
                    Some(format!("{:.0} food", source.amount))
                } else if let Some(matter) = matter {
                    let (timber, stone) = crate::villager::work::offering_worth(matter);
                    if timber > 0.0 {
                        Some(format!("{timber:.0} timber"))
                    } else if stone > 0.0 {
                        Some(format!("{stone:.0} stone"))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
        (
            what,
            match worth {
                Some(worth) => format!("Held: {worth}"),
                None if held => "Held in your grasp".to_string(),
                None => "No interaction available.".to_string(),
            },
        )
    };

    if let Ok(mut name) = texts.p0().single_mut() {
        *name = Text::new(title.clone());
    }
    if let Ok(mut subtitle) = texts.p1().single_mut() {
        *subtitle = Text::new("");
    }
    if let Ok((mut detail, mut detail_visibility)) = texts.p2().single_mut() {
        *detail = Text::new(description);
        *detail_visibility = if detail.0.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

fn food_horizon(food: f32, mouths: usize) -> String {
    if mouths == 0 || food <= 0.0 {
        return "empty".to_string();
    }
    let days = food / mouths as f32 / 2.0;
    match days {
        d if d < 0.5 => "less than a day".to_string(),
        d if d < 1.5 => "about 1 day".to_string(),
        d => format!("about {:.0} days", d.floor()),
    }
}

fn counted(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

fn sleep_phrase(sleeping: usize) -> String {
    match sleeping {
        0 => "all awake".to_string(),
        1 => "1 asleep".to_string(),
        count => format!("{count} asleep"),
    }
}

fn timber_horizon(timber: f32, mouths: usize) -> &'static str {
    match timber / mouths.max(1) as f32 {
        t if t < 0.5 => "barely any",
        t if t < 2.0 => "low",
        t if t < 6.0 => "modest",
        _ => "plentiful",
    }
}

fn household_mood(spirits: f32) -> &'static str {
    match spirits {
        s if s > 0.75 => "content",
        s if s > 0.5 => "steady",
        s if s > 0.3 => "uneasy",
        _ => "troubled",
    }
}

fn household_faith(trust: f32) -> &'static str {
    match trust {
        t if t > 0.75 => "devoted",
        t if t > 0.5 => "believing",
        t if t > 0.25 => "doubtful",
        _ => "resentful",
    }
}
