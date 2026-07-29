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
        (Entity, &Transform, &CreatureGenome, &Person),
        (
            With<Villager>,
            Without<Spouse>,
            Without<crate::creature::Corpse>,
        ),
    >,
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
    for (entity, transform, genome, person) in &singles {
        if genome.age != Age::Adult {
            continue;
        }
        match genome.sex {
            Sex::Female => women.push((entity, transform.translation, person)),
            Sex::Male => men.push((entity, transform.translation, person)),
        }
    }

    for (woman, at, her) in women {
        let Some((slot, _)) = men
            .iter()
            .enumerate()
            .map(|(i, (_, position, _))| (i, position.distance(at)))
            .filter(|(_, d)| *d <= COURTSHIP_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            continue;
        };

        // Nearness is necessary, not sufficient. Some pairs just never happen.
        if !rng.0.chance(0.4) {
            continue;
        }

        let (man, _, him) = men.swap_remove(slot);
        info!("{} and {} were wed", her.name, him.name);
        notices.write(crate::ui::Notice::new(format!(
            "{} and {} were wed",
            her.name, him.name
        )));
        commands.entity(woman).insert(Spouse(man));
        commands.entity(man).insert(Spouse(woman));

        let day = clock.day();
        if let Ok(mut chronicle) = chronicles.get_mut(woman) {
            chronicle.record(day, format!("wed {}", him.name));
        }
        if let Ok(mut chronicle) = chronicles.get_mut(man) {
            chronicle.record(day, format!("wed {}", her.name));
        }
    }
}

/// Talk: witnesses tell their neighbours what they saw, and the story spreads.
///
/// This is the game's central mechanism in embryo. Nobody needs to see a
/// miracle for it to change them — they need to know someone who did. What a
/// villager carries secondhand is counted apart from what they saw, because a
/// faith built on rumour and a faith built on witness are different faiths,
/// A conversation in progress: who with, when it ends, and how far the
/// telling has got.
#[derive(Component)]
pub struct Conversing {
    pub partner: Entity,
    pub until: f64,
    pub spoke_at: Option<f64>,
    pub replied: bool,
    pub kind: Option<crate::witness::DivineEventKind>,
    /// What the other party is telling — so a listener who saw the same
    /// thing can answer as a fellow witness, not a doubter.
    pub hearing: Option<crate::witness::DivineEventKind>,
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
        let Some(&kind) = witnessed.recent.first() else {
            continue;
        };
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        // A story wears out in its own telling: each retelling cools the
        // urge until only the chattiest still bother — and a fresh sight
        // winds the whole square back up. Without this, one smite kept
        // every witness retelling it in a loop forever.
        let tongue = manner.map_or(1.0, |m| m.talkativeness());
        let fatigue = 1.0 / (1.0 + witnessed.told as f32 * 0.8);
        if !rng.0.chance((0.4 * tongue * fatigue).min(0.95)) {
            continue;
        }
        // The nearest idle neighbour becomes the audience.
        let Some((listener, _)) = talkers
            .iter()
            .filter(|(other, _, other_activity, _, _)| {
                *other != teller
                    && !paired.contains(other)
                    && matches!(**other_activity, Activity::Idle | Activity::Wandering)
            })
            .map(|(other, other_at, ..)| (other, other_at.translation.distance(at.translation)))
            .filter(|(_, d)| *d <= EARSHOT * 2.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            continue;
        };
        paired.push(teller);
        paired.push(listener);
        let until = clock.elapsed + 14.0;
        commands.entity(teller).insert((
            Conversing {
                partner: listener,
                until,
                spoke_at: None,
                replied: false,
                kind: Some(kind),
                hearing: None,
            },
            Activity::Chatting,
        ));
        commands.entity(listener).insert((
            Conversing {
                partner: teller,
                until,
                spoke_at: None,
                replied: false,
                kind: None,
                hearing: Some(kind),
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
) {
    // Positions snapshot so both halves of a pair can steer at each other.
    let spots: Vec<(Entity, Vec3)> = pairs
        .iter()
        .map(|(entity, at, ..)| (entity, at.translation))
        .collect();
    let spot_of = |entity: Entity| spots.iter().find(|(e, _)| *e == entity).map(|(_, p)| *p);
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());

    for (entity, at, mut talk, mut activity, mut target) in &mut pairs {
        if *activity != Activity::Chatting || clock.elapsed > talk.until {
            commands.entity(entity).remove::<Conversing>();
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
        if let Some(kind) = talk.kind
            && talk.spoke_at.is_none()
        {
            talk.spoke_at = Some(clock.elapsed);
            let teller_name = minds
                .get(entity)
                .map(|(p, ..)| p.name.clone())
                .unwrap_or_default();
            // The same story never wears the same words twice in a row.
            let told = (*rng.0.pick(kind.rumors())).replace("the god", god);
            say.write(crate::ui::Say {
                speaker: entity,
                text: told.clone(),
                thought: false,
            });
            if let Ok((listener_person, mut witnessed, chronicle, faith)) =
                minds.get_mut(talk.partner)
            {
                if !witnessed.recent.contains(&kind) {
                    witnessed.secondhand = witnessed.secondhand.saturating_add(1);
                    if let Some(mut chronicle) = chronicle {
                        chronicle.hear(clock.day(), &teller_name, &told);
                    }
                    if let Some(mut faith) = faith {
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
        if talk.kind.is_none() && !talk.replied && clock.elapsed > talk.until - 9.0 {
            talk.replied = true;
            // A listener who stood there too answers as a fellow witness,
            // not a doubter — shared awe, not scepticism.
            let saw_it_too = talk.hearing.is_some_and(|kind| {
                minds
                    .get(entity)
                    .is_ok_and(|(_, witnessed, _, _)| witnessed.recent.contains(&kind))
            });
            let reply = if saw_it_too {
                *rng.0.pick(&[
                    "I saw it too - I can still hardly breathe",
                    "you too? I thought my eyes had broken",
                    "I was THERE - every word of it is true",
                    "no story. I stood right beside you",
                    "I have thought of nothing else since",
                    "we saw it together - who will believe us",
                ])
            } else {
                *rng.0.pick(&[
                    "truly?",
                    "I half believe it",
                    "the god again...",
                    "so the stories are true",
                    "keep your voice down",
                    "who else knows of this?",
                    "you swear it?",
                    "stranger things have happened here",
                    "tell it again, slower",
                    "do not spread that too far",
                    "I will believe it when I see it",
                ])
            };
            say.write(crate::ui::Say {
                speaker: entity,
                text: reply.replace("the god", god),
                thought: false,
            });
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
