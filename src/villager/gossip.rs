//! The social fabric: courtship, conversation, and the gossip mill.
//!
//! What a villager carries secondhand is counted apart from what they saw,
//! because a faith built on rumour and a faith built on witness are
//! different faiths - and the road between them is two people stopping to
//! talk. This is the machine the shrine's sermons and the taverns' evenings
//! plug into.

use bevy::prelude::*;

use super::*;

/// Courtship: an unwed man and an unwed woman who find themselves near each
/// other may wed.
///
/// Proximity is the whole mechanism, and that is what makes it emergent: the
/// player shapes the village's families every time they carry someone across
/// the map, place the food that draws people together, or scatter a crowd.
pub(crate) fn form_bonds(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut since_last: Local<f32>,
    mut rng: Option<ResMut<SimRng>>,
    mut chronicles: Query<&mut Chronicle>,
    mut notices: MessageWriter<crate::ui::Notice>,
    singles: Query<
        (
            Entity,
            &Transform,
            &CreatureGenome,
            &Person,
            Option<&Courting>,
        ),
        (
            With<Villager>,
            Without<Spouse>,
            Without<crate::creature::Corpse>,
        ),
    >,
    shrines: Query<(&GlobalTransform, &crate::villager::work::Building)>,
) {
    *since_last += time.delta_secs();
    if *since_last < BOND_INTERVAL {
        return;
    }
    *since_last = 0.0;
    let Some(rng) = rng.as_mut() else {
        return;
    };

    let mut women = Vec::new();
    let mut men = Vec::new();
    for (entity, transform, genome, person, courting) in &singles {
        if genome.age != Age::Adult {
            continue;
        }
        match genome.sex {
            Sex::Female => women.push((entity, transform.translation, person, courting)),
            Sex::Male => men.push((entity, transform.translation, person)),
        }
    }

    let today = clock.day();
    for (woman, at, her, courting) in women {
        // Who she is walking out with, if he is still unwed and alive. A
        // courtship survives the working day: they are courting, not
        // conjoined, and requiring the two of them to be within a dozen
        // strides at every check meant every courtship in the village
        // was dropped the first morning both of them went to work.
        // Nearness is asked for again at the wedding itself, below.
        let walking_out = courting.and_then(|courting| {
            men.iter()
                .position(|(man, ..)| *man == courting.with)
                .map(|slot| (slot, courting.since))
        });
        if courting.is_some() && walking_out.is_none() {
            commands.entity(woman).remove::<Courting>();
        }

        let slot = match walking_out {
            // Courting, and not long enough yet. A wedding is a thing two
            // people arrive at, not a thing proximity does to them.
            Some((_, since)) if today.saturating_sub(since) < COURTSHIP_DAYS => continue,
            // Courted long enough - now they have to be standing
            // together for it, the way a wedding needs both of them,
            // and there has to be somewhere to do it.
            Some((slot, since)) if men[slot].1.distance(at) <= COURTSHIP_DISTANCE => {
                let god_house = shrines.iter().any(|(shrine_at, building)| {
                    building.kind == crate::villager::work::BuildingKind::Shrine
                        && shrine_at.translation().distance(at) < SHRINE_REACH
                });
                // Vows are made in the god's house. A village with none
                // has couples waiting on it, and that waiting is what
                // gets one built - see CivicNeeds::betrothed.
                //
                // But not forever. A hard dependency on a building runs
                // backwards into extinction: no shrine, no weddings, no
                // children, no village. A couple left a whole season
                // without one marry at the fire instead - grown people
                // live years, so a season is real patience rather than a
                // technicality, and the shrine stays what it should be:
                // the thing a village wants badly, not the thing it dies
                // without.
                let waited = today.saturating_sub(since);
                if !god_house && waited < crate::calendar::DAYS_PER_SEASON {
                    continue;
                }
                slot
            }
            Some(_) => continue,
            None => {
                // Not walking out with anyone: the nearest unwed man may
                // become the one she does.
                let Some((slot, _)) = men
                    .iter()
                    .enumerate()
                    .map(|(i, (_, position, _))| (i, position.distance(at)))
                    .filter(|(_, d)| *d <= COURTSHIP_DISTANCE)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                else {
                    continue;
                };
                // Nearness is necessary, not sufficient. Some pairs just
                // never happen.
                if !rng.0.chance(0.4) {
                    continue;
                }
                commands.entity(woman).insert(Courting {
                    with: men[slot].0,
                    since: today,
                });
                continue;
            }
        };

        let (man, _, him) = men.swap_remove(slot);
        commands.entity(woman).remove::<Courting>();
        info!("{} and {} were wed", her.full_name(), him.full_name());
        notices.write(crate::ui::Notice::new(format!(
            "{} and {} were wed",
            her.name, him.name
        )));
        commands.entity(woman).insert(Spouse(man));
        commands.entity(man).insert(Spouse(woman));

        // She takes his house. `born_surname` is deliberately left as it was:
        // it is the only thread left back to the parents who raised her, and
        // a family tree drawn without it loses every maternal line at the
        // first wedding.
        if !him.surname.is_empty() && him.surname != her.surname {
            commands.entity(woman).insert(Person {
                name: her.name.clone(),
                surname: him.surname.clone(),
                born_surname: her.born_surname.clone(),
            });
        }

        let day = clock.day();
        if let Ok(mut chronicle) = chronicles.get_mut(woman) {
            chronicle.record(day, format!("wed {} and took his name", him.full_name()));
        }
        if let Ok(mut chronicle) = chronicles.get_mut(man) {
            chronicle.record(day, format!("wed {}", her.full_name()));
        }
    }
}

/// Talk: witnesses tell their neighbours what they saw, and the story spreads.
///
/// This is the game's central mechanism in embryo. Nobody needs to see a
/// miracle for it to change them — they need to know someone who did. What a
/// villager carries secondhand is counted apart from what they saw, because a
/// faith built on rumour and a faith built on witness are different faiths,
/// When this person last finished a conversation.
///
/// Talk has to be punctuation, not the day. Widening who may chat, and
/// then standing the whole founding village around one hall, had them
/// pairing off continuously: food and timber both flatlined inside a
/// minute of the world starting and every one of them starved beside a
/// full store, having spent the working day in conversation.
#[derive(Component)]
pub struct SpokeLately(pub f64);

/// Seconds before somebody is up for another conversation. About a
/// sixth of a working day, so a villager can have a few in a day and
/// still get something done in between.
const AGAIN_AFTER: f64 = 55.0;

/// What two people are talking ABOUT.
///
/// It used to be only ever a miracle: the whole conversation system hung
/// off somebody having a `Witnessed` memory to retell, so two foresters
/// felling the same tree had no way to talk about the tree. Most talk in
/// a village is not news - it is the work, the food, the roof and the
/// weather - and a village that only speaks when the god does something
/// is a very quiet village.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chat {
    /// Something one of them saw the god do.
    Memory(crate::witness::DivineEventKind),
    /// The trade they share, or the one the opener holds.
    Work(work::Vocation),
    /// The stores, and what there is to eat.
    Food,
    /// Roofs: who has one, who is still waiting.
    Roof,
    /// What the sky is doing to them.
    Weather,
}

impl Chat {
    /// The tag the corpus knows this subject by.
    pub fn tag(self) -> String {
        match self {
            Chat::Memory(kind) => format!("event:{kind:?}").to_lowercase(),
            Chat::Work(trade) => format!("topic:{}", trade.trade()),
            Chat::Food => "topic:food".to_string(),
            Chat::Roof => "topic:roof".to_string(),
            Chat::Weather => "topic:weather".to_string(),
        }
    }
}

/// A conversation in progress: who with, when it ends, and how far the
/// telling has got.
#[derive(Component)]
pub struct Conversing {
    pub partner: Entity,
    pub until: f64,
    pub spoke_at: Option<f64>,
    /// How many turns THIS speaker has taken. A conversation is four
    /// beats now - the telling, the answer, the teller's followup, the
    /// listener's close - and each side takes two of them.
    pub beat: u8,
    /// What the exchange is about, carried by BOTH speakers so the later
    /// beats stay on topic instead of being reinvented from whatever the
    /// speaker happens to remember.
    pub topic: Option<Chat>,
    /// Whether this one opened it. The two sides used to be told apart by
    /// which of them held the memory, which stops working the moment a
    /// conversation is about the weather.
    pub opener: bool,
    /// The whole memory, not just its kind: the telling needs who it
    /// happened to, in the teller's own terms.
    ///
    /// (A listener's fellow-witness standing is read at the moment their
    /// reply is composed, straight from their own memories — the old
    /// `hearing` echo retired with the stock replies it served.)
    pub kind: Option<crate::witness::Memory>,
}

/// Whose turn it is to speak, and in what role.
///
/// A conversation is four beats: the teller's opener, the listener's
/// answer, the teller's followup, the listener's close. The first two are
/// handled where the story changes hands; this schedules the two after
/// it, which carry no knowledge and only interpretation.
///
/// Spaced so it reads as an exchange rather than a volley - the answer
/// five seconds in, the followup at eight, the close at eleven, and three
/// seconds of standing together before they part.
fn beat_role(teller: bool, beat: u8, elapsed: f64, until: f64) -> Option<&'static str> {
    match (teller, beat) {
        // The teller pushes, softens, or explains themselves.
        (true, 1) if elapsed > until - 6.0 => Some("chat:followup"),
        // And the listener closes it.
        (false, 1) if elapsed > until - 3.0 => Some("chat:end"),
        _ => None,
    }
}

/// Whoever has news finds an idle neighbour and goes TO them: both stop,
/// meet, and hold an actual conversation instead of talking over their
/// shoulders mid-stride.
#[allow(clippy::type_complexity)]
pub(crate) fn meet_to_talk(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut since_last: Local<f32>,
    mut rng: Option<ResMut<SimRng>>,
    talkers: Query<
        (
            Entity,
            &Transform,
            &Activity,
            &crate::witness::Witnessed,
            Option<&traits::Traits>,
        ),
        (
            With<Villager>,
            Without<crate::creature::Corpse>,
            Without<Conversing>,
        ),
    >,
    spoke: Query<&SpokeLately>,
    // What there is to talk about besides miracles: the work in their
    // hands, whether they have eaten, whether they have a bed.
    circumstances: Query<(
        Option<&work::Vocation>,
        Option<&super::Needs>,
        Has<super::home::Home>,
    )>,
    weather: Option<Res<crate::weather::Weather>>,
) {
    *since_last += time.delta_secs();
    if *since_last < GOSSIP_INTERVAL {
        return;
    }
    *since_last = 0.0;
    let Some(rng) = rng.as_mut() else {
        return;
    };

    let mut paired: Vec<Entity> = Vec::new();
    for (teller, at, activity, witnessed, manner) in talkers.iter() {
        if paired.contains(&teller) {
            continue;
        }
        let memory = witnessed.recent.first().cloned();
        // Who is free to fall into conversation. News interrupts an idle
        // moment; ordinary talk also happens over the work, at the fire,
        // and under the eaves waiting out rain - which is most of when
        // people actually talk to each other. Requiring both parties to
        // be idle meant the only pairs that ever qualified were the one
        // or two nobody had given a job to, and they were never standing
        // near each other.
        let free_to_talk = |activity: &Activity| {
            matches!(
                activity,
                Activity::Idle
                    | Activity::Wandering
                    | Activity::Working
                    | Activity::Sheltering
                    | Activity::TendingFire
            )
        };
        if !free_to_talk(activity) {
            continue;
        }
        let rested = |who: Entity| {
            spoke
                .get(who)
                .is_ok_and(|last| clock.elapsed - last.0 < AGAIN_AFTER)
        };
        if rested(teller) {
            continue;
        }
        // A story wears out in its own telling: each retelling cools the
        // urge until only the chattiest still bother — and a fresh sight
        // winds the whole square back up. Without this, one smite kept
        // every witness retelling it in a loop forever.
        let tongue = manner.map_or(1.0, |m| m.talkativeness());
        let fatigue = 1.0 / (1.0 + witnessed.told as f32 * 0.8);
        // With news, the urge is the story's. Without it, it is just the
        // ordinary wish to say something to somebody - rarer, because a
        // village where every passing pair strikes up a conversation is
        // as unreal as one where nobody ever does.
        let urge = if memory.is_some() {
            0.4 * tongue * fatigue
        } else {
            0.14 * tongue
        };
        if !rng.0.chance(urge.min(0.95)) {
            continue;
        }
        // The nearest idle neighbour becomes the audience.
        // News is worth crossing a yard for; a word about the weather is
        // said to whoever is already beside you.
        let reach = if memory.is_some() {
            EARSHOT * 2.0
        } else {
            EARSHOT * 1.4
        };
        let Some((listener, _)) = talkers
            .iter()
            .filter(|(other, _, other_activity, _, _)| {
                *other != teller
                    && !paired.contains(other)
                    && free_to_talk(other_activity)
                    && !rested(*other)
            })
            .map(|(other, other_at, ..)| (other, other_at.translation.distance(at.translation)))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            continue;
        };
        // What the two of them have to talk about, if it is not news.
        // Shared circumstances first - the same trade, the same empty
        // stomach, the same lack of a roof - because that is what people
        // actually talk about, and only then the sky, which is what you
        // fall back on with a stranger.
        let subject = match memory.as_ref() {
            Some(memory) => Chat::Memory(memory.kind),
            None => {
                let mine = circumstances.get(teller).ok();
                let theirs = circumstances.get(listener).ok();
                let (my_trade, my_needs, my_roof) = mine.unwrap_or((None, None, false));
                let (their_trade, their_needs, their_roof) = theirs.unwrap_or((None, None, false));
                let both_hungry = my_needs.is_some_and(|n| n.hunger > 0.4)
                    && their_needs.is_some_and(|n| n.hunger > 0.4);
                let both_roofless = !my_roof && !their_roof;
                let shared_trade = my_trade
                    .copied()
                    .filter(|mine| Some(mine) == their_trade.as_deref());
                let foul = weather
                    .as_ref()
                    .is_some_and(|weather| weather.intensity > 0.25 || weather.chill > 0.5);
                match (shared_trade, both_hungry, both_roofless, foul) {
                    (Some(trade), ..) => Chat::Work(trade),
                    (_, true, ..) => Chat::Food,
                    (_, _, true, _) => Chat::Roof,
                    (_, _, _, true) => Chat::Weather,
                    // Nothing pressing in common: their own work, or the
                    // weather if they have no trade between them.
                    _ => my_trade.copied().map_or(Chat::Weather, Chat::Work),
                }
            }
        };
        // Logged, because a chat about the work writes no chronicle and
        // shows no bubble unless the god happens to be watching - which
        // leaves a soak no way at all to tell talking from silence.
        if memory.is_none() {
            info!("two of them fell to talking about {}", subject.tag());
        }
        paired.push(teller);
        paired.push(listener);
        let until = clock.elapsed + 14.0;
        commands.entity(teller).insert((
            Conversing {
                partner: listener,
                until,
                spoke_at: None,
                beat: 0,
                topic: Some(subject),
                opener: true,
                kind: memory.clone(),
            },
            Activity::Chatting,
        ));
        commands.entity(listener).insert((
            Conversing {
                partner: teller,
                until,
                spoke_at: None,
                beat: 0,
                topic: Some(subject),
                opener: false,
                kind: None,
            },
            Activity::Chatting,
        ));
    }
}

/// The meeting itself: close the distance, face each other, tell it, hear
/// the reply, and part. The knowledge changes hands at the meeting.
#[allow(clippy::type_complexity)]
pub(crate) fn hold_conversations(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    name: Option<Res<DivineName>>,
    mut say: MessageWriter<crate::ui::Say>,
    mut rng: ResMut<SimRng>,
    // Only the WORDS are aimed at the camera. Everything below that changes
    // hands — what the listener now carries secondhand, what their faith does
    // about it, what goes in their chronicle — happens whether or not anyone
    // is watching, because that is the simulation and not its presentation.
    attention: Option<Res<crate::attention::Attention>>,
    now: Option<Res<crate::now::WorldNow>>,
    members: Query<&MemberOf>,
    mut pairs: Query<
        (
            Entity,
            &Transform,
            &mut Conversing,
            &mut Activity,
            &mut MoveTarget,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    mut minds: Query<(
        &Person,
        &mut crate::witness::Witnessed,
        Option<&mut Chronicle>,
        Option<&mut belief::Faith>,
    )>,
    // Who each teller is, for putting the story in their own mouth. Absent
    // unless the teller is switched on, in which case none of this runs.
    voices: Query<(
        Option<&work::Vocation>,
        Option<&crate::witness::Temperament>,
        Option<&traits::Traits>,
    )>,
    tongue: Option<ResMut<crate::telling::Tongue>>,
) {
    let mut tongue = tongue;
    // Positions snapshot so both halves of a pair can steer at each other.
    let spots: Vec<(Entity, Vec3)> = pairs
        .iter()
        .map(|(entity, at, ..)| (entity, at.translation))
        .collect();
    let spot_of = |entity: Entity| spots.iter().find(|(e, _)| *e == entity).map(|(_, p)| *p);
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());

    for (entity, at, mut talk, mut activity, mut target) in &mut pairs {
        if *activity != Activity::Chatting || clock.elapsed > talk.until {
            commands
                .entity(entity)
                .remove::<Conversing>()
                .insert(SpokeLately(clock.elapsed));
            if *activity == Activity::Chatting {
                *activity = Activity::Idle;
                target.0 = None;
            }
            continue;
        }
        let Some(partner_at) = spot_of(talk.partner) else {
            commands.entity(entity).remove::<Conversing>();
            *activity = Activity::Idle;
            target.0 = None;
            continue;
        };
        let distance = at.translation.distance(partner_at);
        if distance > 1.9 {
            target.0 = Some(partner_at);
            continue;
        }
        target.0 = None;

        // Met: the teller speaks once; the listener takes it in and replies.
        if let Some(memory) = talk.kind.clone()
            && talk.spoke_at.is_none()
        {
            let kind = memory.kind;
            talk.spoke_at = Some(clock.elapsed);
            talk.beat = 1;
            let teller_name = minds
                .get(entity)
                .map(|(p, ..)| p.name.clone())
                .unwrap_or_default();
            // The same story never wears the same words twice in a row.
            //
            // If the teller is listening, the line comes from this villager's
            // own circumstances — what they saw with their own eyes against
            // what they were merely told, their trade, their belief. If it is
            // not, or has nothing ready, the written phrasings answer. There
            // is one fallback and it is always there.
            //
            // Asked for only when the god is close enough to read the answer.
            // A composed line is a second and a half of a model's time, and
            // spending it on a conversation happening off the frame buys
            // nothing — the written phrasing is instant and reads identically
            // to nobody.
            let regard = crate::attention::regard(attention.as_deref(), at.translation);
            let spoken = tongue
                .as_mut()
                .filter(|_| regard.worth_composing())
                .and_then(|tongue| {
                    let (voice, nature, bearing) = voices
                        .get(entity)
                        .map(|(v, t, manner)| {
                            (
                                v.copied(),
                                t,
                                manner.map(|m| m.bearing()).unwrap_or_default(),
                            )
                        })
                        .unwrap_or((None, None, traits::Bearing::default()));
                    let (hand, told_before) = minds
                        .get(entity)
                        .map(|(_, witnessed, ..)| {
                            (
                                crate::telling::Retelling::hand_of(witnessed),
                                witnessed.told,
                            )
                        })
                        .unwrap_or((crate::telling::Hand::Heard, 0));
                    let trust = minds
                        .get(entity)
                        .ok()
                        .and_then(|(_, _, _, faith)| faith.map(|f| f.trust))
                        .unwrap_or(0.3);
                    tongue.line(&crate::telling::Retelling::new(
                        kind,
                        hand,
                        voice,
                        trust,
                        bearing,
                        memory.whom.clone(),
                        nature.map_or(0.5, |t| t.boldness),
                        told_before,
                    ))
                });
            // Logged when it is the model's, so a run can be read back and
            // judged: everything else in the square is a written line.
            if let Some(line) = &spoken {
                info!("{teller_name} tells it in their own words: {line}");
            }
            // Drawn every telling whether or not it is the one used, so that
            // the simulation's draw from the shared stream does not depend on
            // whether a model happened to answer in time. The written
            // phrasing never reaches a bubble any more — it serves the
            // chronicle and the reply prompt, the world's record of what
            // passed between them.
            let written = (*rng.0.pick(kind.rumors())).to_string();
            let composed = spoken.is_some();
            // Kept in the world's own register — "the god", never the name —
            // because it goes back INTO a prompt as what the listener heard.
            let told_plain = spoken.clone().unwrap_or(written);
            let told = told_plain.replace("the god", god);

            // The listener's answer starts composing NOW, while the telling
            // hangs in the air: the reply beat lands several seconds from
            // here, which is more than a line takes, so the answer is
            // almost always waiting when their turn comes.
            if composed
                && let Some(tongue) = tongue.as_mut()
                && let Some(partner_at) = spot_of(talk.partner)
                && crate::attention::regard(attention.as_deref(), partner_at).worth_composing()
            {
                let (voice, _, bearing) = voices
                    .get(talk.partner)
                    .map(|(v, t, manner)| {
                        (
                            v.copied(),
                            t,
                            manner.map(|m| m.bearing()).unwrap_or_default(),
                        )
                    })
                    .unwrap_or((None, None, traits::Bearing::default()));
                let (saw_it_too, trust, listener_name) = minds
                    .get(talk.partner)
                    .map(|(person, witnessed, _, faith)| {
                        (
                            witnessed.remembers(kind),
                            faith.map_or(0.3, |f| f.trust),
                            person.name.clone(),
                        )
                    })
                    .unwrap_or((false, 0.3, String::new()));
                let place = members
                    .get(talk.partner)
                    .ok()
                    .and_then(|member| now.as_ref()?.places.get(&member.0).cloned());
                // Every name the answer may use: their own, the teller's,
                // their place, and whoever the story itself is about.
                let mut known: Vec<String> = vec![listener_name, teller_name.clone()];
                known.extend(place.as_ref().map(|p| p.name.clone()));
                known.extend(memory.whom.as_ref().map(|w| w.name.clone()));
                tongue.muse(crate::telling::Musing {
                    who: talk.partner,
                    voice,
                    bearing,
                    faith: crate::telling::FaithBand::of(trust),
                    body: Vec::new(),
                    place: place.map(|p| p.lines()).unwrap_or_default(),
                    mind: if saw_it_too {
                        "you stood there and saw it happen too".into()
                    } else {
                        "whether to believe a word of it".into()
                    },
                    heard: Some(told_plain.clone()),
                    aloud: true,
                    prayer: false,
                    known,
                });
            }
            // The bubble is for the player; the telling is for the village.
            // Only composed words are shown — a telling whose words never
            // came back happens quietly, and everything below still happens.
            if let Some(line) = &spoken
                && regard.worth_saying()
            {
                say.write(crate::ui::Say {
                    speaker: entity,
                    text: line.replace("the god", god),
                    thought: false,
                    prayer: false,
                    own_words: true,
                });
            }
            if let Ok((listener_person, mut witnessed, chronicle, faith)) =
                minds.get_mut(talk.partner)
            {
                if !witnessed.remembers(kind) {
                    // Whether the listener BUYS the god in it is their own
                    // grain's business: a skeptic pockets the story and none
                    // of the awe, the devout swallow it whole — and what
                    // they retell later carries their verdict, not the
                    // teller's.
                    let conviction = voices
                        .get(talk.partner)
                        .ok()
                        .and_then(|(_, _, manner)| manner.map(|m| m.conviction()))
                        .unwrap_or(1.0);
                    let believed = memory.divine
                        && rng.0.chance(
                            (0.35 * conviction * kind.unmistakably_divine().max(0.4)).min(0.9),
                        );
                    let mut heard = memory.clone();
                    heard.divine = believed;
                    // The story itself changes hands either way: the listener
                    // can now retell it — as something told to them, never
                    // seen, and in their own stance.
                    witnessed.hear(heard);
                    if let Some(mut chronicle) = chronicle {
                        chronicle.hear(clock.day(), &teller_name, &told);
                    }
                    if believed && let Some(mut faith) = faith {
                        faith.trust = (faith.trust + 0.02).min(0.8);
                    }
                }
                let _ = listener_person;
            }
            // The telling itself spends the teller's fire.
            if let Ok((_, mut own_witnessed, _, _)) = minds.get_mut(entity) {
                own_witnessed.told = own_witnessed.told.saturating_add(1);
            }
        }
        // The reply, a beat after the meeting settles, from the listener.
        if !talk.opener && talk.beat == 0 && clock.elapsed > talk.until - 9.0 {
            talk.beat = 1;
            let regard = crate::attention::regard(attention.as_deref(), at.translation);
            // Their own answer, if it came back in time. Composed against
            // the actual words they heard, so it reacts instead of picking
            // from a bowl of stock reactions.
            let composed = tongue.as_mut().and_then(|tongue| tongue.take_reply(entity));
            if let Some(line) = composed {
                if regard.worth_saying() {
                    info!("an answer in their own words: {line}");
                    say.write(crate::ui::Say {
                        speaker: entity,
                        text: line.replace("the god", god),
                        thought: false,
                        prayer: false,
                        own_words: true,
                    });
                }
                continue;
            }
            // No composed answer: the corpus answers instead, on topic.
            // Silence was honest when the only subject was a miracle and
            // the only source was a model; a chat about the day's work
            // has a pool of its own and no excuse to go quiet.
            if let Some(topic) = talk.topic {
                let trust = minds
                    .get(entity)
                    .ok()
                    .and_then(|(_, _, _, faith)| faith.map(|f| f.trust))
                    .unwrap_or(0.3);
                let voice = voices.get(entity).ok().and_then(|(v, ..)| v.copied());
                let answered = tongue.as_mut().and_then(|tongue| {
                    tongue.turn(
                        entity,
                        "chat:reply",
                        &topic.tag(),
                        crate::telling::FaithBand::of(trust),
                        voice,
                        None,
                    )
                });
                if let Some(line) = answered
                    && regard.worth_saying()
                {
                    say.write(crate::ui::Say {
                        speaker: entity,
                        text: line.replace("the god", god),
                        thought: false,
                        prayer: false,
                        own_words: true,
                    });
                }
            }
        }

        // The two later beats. Nothing changes hands here - the story
        // moved on the opener, and everything after it is the two of them
        // working out what to make of it. That split is the whole reason
        // a four-beat conversation does not spread a rumour four times.
        // A chat about the work or the weather has no story to hand over,
        // so it never enters the telling above - it opens here instead,
        // and runs the same four beats.
        let role = if talk.kind.is_none() && talk.opener {
            match talk.beat {
                0 => {
                    talk.spoke_at = Some(clock.elapsed);
                    Some("chat:open")
                }
                1 if clock.elapsed > talk.until - 6.0 => Some("chat:followup"),
                _ => None,
            }
        } else {
            beat_role(talk.opener, talk.beat, clock.elapsed, talk.until)
        };
        if let Some(role) = role
            && let Some(topic) = talk.topic
        {
            talk.beat = 2;
            let regard = crate::attention::regard(attention.as_deref(), at.translation);
            let said = tongue
                .as_mut()
                .filter(|_| regard.worth_saying())
                .and_then(|tongue| {
                    let (voice, _, _) = voices
                        .get(entity)
                        .map(|(v, t, manner)| (v.copied(), t, manner))
                        .unwrap_or((None, None, None));
                    let trust = minds
                        .get(entity)
                        .ok()
                        .and_then(|(_, _, _, faith)| faith.map(|f| f.trust))
                        .unwrap_or(0.3);
                    let whom = talk
                        .kind
                        .as_ref()
                        .and_then(|memory| memory.whom.as_ref())
                        .map(|whom| whom.name.clone());
                    // A retelling's later beats may reach for the
                    // witness voice - "I know what I saw" - which is
                    // nonsense in a conversation about a leaky roof.
                    let told = matches!(topic, Chat::Memory(_));
                    tongue.turn_about(
                        entity,
                        role,
                        &topic.tag(),
                        told,
                        crate::telling::FaithBand::of(trust),
                        voice,
                        whom.as_deref(),
                    )
                });
            if let Some(line) = said {
                say.write(crate::ui::Say {
                    speaker: entity,
                    text: line.replace("the god", god),
                    thought: false,
                    prayer: false,
                    own_words: true,
                });
            }
        }
    }
}

/// Wants drive feet: the unwed go looking for company instead of waiting
/// for it to wander past. Bonds still form by the old rule - nearness and
/// time - but now the lonely close the distance themselves.
#[allow(clippy::type_complexity)]
pub(crate) fn seek_company(
    time: Res<Time>,
    mut since_last: Local<f32>,
    clock: Res<crate::calendar::WorldClock>,
    mut rng: ResMut<SimRng>,
    mut singles: Query<
        (
            Entity,
            &Transform,
            &CreatureGenome,
            &Activity,
            &mut MoveTarget,
        ),
        (
            With<Villager>,
            Without<Spouse>,
            Without<Childhood>,
            Without<crate::creature::Corpse>,
            Without<Held>,
            Without<Airborne>,
        ),
    >,
) {
    *since_last += time.delta_secs();
    if *since_last < 2.0 {
        return;
    }
    *since_last = 0.0;

    let hour = clock.time_of_day();
    let ardour = if (0.58..0.78).contains(&hour) {
        0.3
    } else {
        0.04
    };

    let others: Vec<(Entity, Vec3, crate::creature::genome::Sex)> = singles
        .iter()
        .map(|(entity, at, genome, _, _)| (entity, at.translation, genome.sex))
        .collect();

    for (entity, at, genome, activity, mut target) in &mut singles {
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        if !rng.0.chance(ardour) {
            continue;
        }
        let Some((_, near, _)) = others
            .iter()
            .filter(|(other, other_at, sex)| {
                *other != entity
                    && *sex != genome.sex
                    && other_at.distance(at.translation) > 5.0
                    && other_at.distance(at.translation) < 70.0
            })
            .min_by(|a, b| {
                a.1.distance(at.translation)
                    .total_cmp(&b.1.distance(at.translation))
            })
        else {
            continue;
        };
        target.0 = Some(*near + Vec3::new(rng.0.range(-2.0, 2.0), 0.0, rng.0.range(-2.0, 2.0)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creature::genome::CreatureGenome;
    use crate::rng::Rng;

    /// One woman and one man, grown, standing close enough to court.
    fn courting_pair(app: &mut App) -> (Entity, Entity) {
        let mut adult = |sex: Sex, name: &str, house: &str, at: f32| {
            let mut genome =
                CreatureGenome::random(crate::creature::genome::Species::Human, &mut Rng::new(1));
            genome.sex = sex;
            genome.age = Age::Adult;
            app.world_mut()
                .spawn((
                    Villager,
                    Transform::from_xyz(at, 0.0, 0.0),
                    genome,
                    Person::born(name.into(), house.into()),
                    Chronicle::default(),
                ))
                .id()
        };
        let bride = adult(Sex::Female, "Temewa", "Kirap", 0.0);
        let groom = adult(Sex::Male, "Shezirav", "Rohap", 1.0);
        (bride, groom)
    }

    #[test]
    fn a_bride_takes_her_husbands_house_and_keeps_her_own_on_record() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(BOND_INTERVAL + 1.0));
        app.insert_resource(time);
        app.init_resource::<crate::calendar::WorldClock>();
        // A seed that clears the 0.4 "some pairs never happen" roll.
        app.insert_resource(SimRng(Rng::new(3)));
        app.add_message::<crate::ui::Notice>();
        app.add_systems(Update, form_bonds);

        let (bride, groom) = courting_pair(&mut app);
        for _ in 0..12 {
            app.update();
        }

        // Standing next to someone is not a wedding. They are walking out
        // together, and that is all, until the courtship has run.
        assert!(
            app.world().entity(bride).get::<Spouse>().is_none(),
            "they wed on the day they met",
        );
        assert!(
            app.world().entity(bride).get::<Courting>().is_some(),
            "nobody started walking out at all",
        );

        // Courted long enough now - and still not wed, because vows are
        // made in the god's house and this village has not built one.
        app.world_mut()
            .resource_mut::<crate::calendar::WorldClock>()
            .elapsed += (COURTSHIP_DAYS as f32 * crate::calendar::DAY_SECONDS) as f64;
        for _ in 0..12 {
            app.update();
        }
        assert!(
            app.world().entity(bride).get::<Spouse>().is_none(),
            "they wed with no shrine to be wed in",
        );

        // Raise one, and they marry.
        app.world_mut().spawn((
            GlobalTransform::from(Transform::from_xyz(4.0, 0.0, 0.0)),
            crate::villager::work::Building {
                kind: crate::villager::work::BuildingKind::Shrine,
            },
        ));
        for _ in 0..12 {
            app.update();
        }

        let world = app.world();
        assert!(
            world.entity(bride).get::<Spouse>().is_some(),
            "the pair never wed, so there is nothing to test",
        );
        assert!(
            world.entity(bride).get::<Courting>().is_none(),
            "the courtship should end at the wedding",
        );

        let wife = world.entity(bride).get::<Person>().unwrap();
        let husband = world.entity(groom).get::<Person>().unwrap();
        assert_eq!(wife.surname, husband.surname, "she takes his house");
        assert_eq!(
            wife.born_surname, "Kirap",
            "her birth house is the family tree's only thread back to her parents",
        );
        assert_eq!(wife.maiden_house(), Some("Kirap"));
        assert_eq!(wife.full_name(), "Temewa Rohap");
        assert_eq!(wife.name_with_house(), "Temewa Rohap (of Kirap)");

        // His own name is untouched, and carries no maiden note.
        assert_eq!(husband.full_name(), "Shezirav Rohap");
        assert_eq!(husband.maiden_house(), None);
    }

    #[test]
    fn a_conversation_runs_four_beats_in_order() {
        // The whole exchange, on the clock it actually uses: a meeting
        // that ends at 14 seconds. Nobody speaks out of turn, nobody
        // speaks twice, and the two later beats do not fire until the
        // ones before them have.
        let until = 14.0;
        let at = |t: f64| until - 14.0 + t;

        // The teller has opened (beat 1) and the listener has answered
        // (beat 1). Neither later beat is due yet.
        assert_eq!(beat_role(true, 1, at(6.0), until), None);
        assert_eq!(beat_role(false, 1, at(6.0), until), None);

        // Eight seconds in, the teller follows up - and only the teller.
        assert_eq!(beat_role(true, 1, at(8.5), until), Some("chat:followup"));
        assert_eq!(beat_role(false, 1, at(8.5), until), None);

        // Eleven, and the listener closes it.
        assert_eq!(beat_role(false, 1, at(11.5), until), Some("chat:end"));

        // Nobody who has not taken their first beat gets a later one, and
        // nobody speaks a third time.
        for who in [true, false] {
            assert_eq!(
                beat_role(who, 0, at(13.0), until),
                None,
                "spoke out of turn"
            );
            assert_eq!(beat_role(who, 2, at(13.0), until), None, "spoke twice");
        }
    }

    #[test]
    fn every_conversation_beat_has_something_to_say() {
        // A role with no line for some event is a beat of silence in the
        // middle of a conversation, which reads worse than no
        // conversation at all. Both roles need an unconditional line -
        // one that needs nothing of the speaker but the topic.
        let corpus: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../assets/voice/chat.json")).expect("chat.json");
        for role in ["chat:followup", "chat:end"] {
            let plain = corpus
                .iter()
                .filter(|line| {
                    let tags: Vec<&str> = line["tags"]
                        .as_array()
                        .expect("tags")
                        .iter()
                        .map(|t| t.as_str().expect("tag"))
                        .collect();
                    tags == [role]
                })
                .count();
            assert!(plain >= 4, "{role} has only {plain} lines that always fit");
        }
    }

    #[test]
    fn a_village_with_no_shrine_still_marries_eventually() {
        // The safety valve. A hard dependency on a building runs
        // backwards into extinction - no shrine, no weddings, no
        // children - so a couple left a whole season without one marry
        // at the fire. Grown people live years; a season is patience,
        // not a technicality.
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(BOND_INTERVAL + 1.0));
        app.insert_resource(time);
        app.init_resource::<crate::calendar::WorldClock>();
        app.insert_resource(SimRng(Rng::new(3)));
        app.add_message::<crate::ui::Notice>();
        app.add_systems(Update, form_bonds);

        let (bride, _) = courting_pair(&mut app);
        for _ in 0..12 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<crate::calendar::WorldClock>()
            .elapsed +=
            (crate::calendar::DAYS_PER_SEASON as f32 * crate::calendar::DAY_SECONDS) as f64;
        for _ in 0..12 {
            app.update();
        }
        assert!(
            app.world().entity(bride).get::<Spouse>().is_some(),
            "a season of waiting and still no wedding: the village cannot renew itself",
        );
    }
}
