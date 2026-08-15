//! Civic life: the town elects a mayor, and the mayor leans the town.
//!
//! Brett: "The town hall should be the center of town, I would love for
//! people to elect a mayor and then that mayor organize the town
//! priorities." The regard graph runs the ballot — every adult votes for
//! the neighbour they hold WARMEST, which means campaigns are lived, not
//! played: the soul who answered your knock, wed your daughter and hauled
//! your timber has been running for office all along.
//!
//! The office is real work. The mayor reads the town — the larder, the
//! roofless, the stone pile, the faith — leans it toward the loudest want,
//! and the morning muster deals hands accordingly. Decrees arrive as
//! notices; the chain changes hands each spring, at a death, or when the
//! town sours on its holder badly enough to throw them out early.

use bevy::prelude::*;

use super::regard::Regard;
use super::work::{Building, BuildingKind, Stockpile, Vocation};
use super::{Activity, Chronicle, MemberOf, Person, Settlement, SettlementGround, Villager};
use crate::creature::genome::{Age, CreatureGenome};
use crate::creature::{Corpse, Held, MoveTarget};

/// The chain of office, worn by one soul per town.
#[derive(Component, Debug, Clone, Copy)]
pub struct Mayor(pub Entity);

/// A short public moment at the town hall. The ballot is still the regard
/// graph; this only gives the decision a place, witnesses, and a body in the
/// world so becoming a town does not happen in an invisible system tick.
#[derive(Component, Debug, Clone, Copy)]
pub struct CivicAssembly {
    pub mayor: Entity,
    pub hall: Vec3,
    pub until: f64,
    pub spoken: bool,
}

/// A townsperson temporarily called to a civic assembly. Keeping this marker
/// separate from `Activity` lets the gathering dissolve cleanly.
#[derive(Component, Debug, Clone, Copy)]
pub(super) struct CivicGuest;

const ASSEMBLY_DURATION: f64 = 32.0;

/// The fewest grown souls a town holds an election with. Below this a
/// hamlet needs every hand more than it needs a chairman.
const QUORUM: usize = 8;

/// A town this soured on its mayor throws them out before the year turns:
/// the mean warmth of everyone who holds an opinion at all.
const RECALL_AT: f32 = -0.15;

/// What the town is leaning on, by the mayor's judgement.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivicPriority {
    pub lean: Lean,
    /// The day the decree went out.
    pub set_on: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lean {
    /// Fill the sacks: the food trades get the hands.
    Larder,
    /// Wood before winter: the foresters get them.
    Timber,
    /// Stone for the footings: picks and hammers.
    Stone,
    /// Roofs for the roofless: frames first.
    Homes,
    /// The shrine and the litany: the god's own house.
    Faith,
}

impl Lean {
    pub fn decree(self) -> &'static str {
        match self {
            Lean::Larder => "calls for the larder filled",
            Lean::Timber => "calls for timber",
            Lean::Stone => "calls for stone",
            Lean::Homes => "calls for roofs raised",
            Lean::Faith => "calls the town to the shrine",
        }
    }

    fn address(self) -> &'static str {
        match self {
            Lean::Larder => "We will fill the larder first.",
            Lean::Timber => "We need timber. Put hands to the woods.",
            Lean::Stone => "We need stone for the work ahead.",
            Lean::Homes => "We will raise roofs for those without one.",
            Lean::Faith => "We will gather at the shrine.",
        }
    }
}

/// How hard a lean pulls one trade's want at the morning muster. Only ever
/// UP — a mayor concentrates hands, never starves the other work, because
/// villages must thrive and no decree is allowed to break that law.
pub fn lean_scale(lean: Lean, vocation: Vocation) -> f32 {
    use Vocation::*;
    match (lean, vocation) {
        (Lean::Larder, Gatherer | Fisher | Hunter | Farmer) => 1.5,
        (Lean::Timber, Forester) => 1.6,
        (Lean::Timber, Explorer) => 1.2,
        (Lean::Stone, Miner) => 1.6,
        (Lean::Stone, Builder) => 1.2,
        (Lean::Homes, Builder) => 1.6,
        (Lean::Homes, Forester | Miner) => 1.2,
        (Lean::Faith, Priest) => 2.0,
        _ => 1.0,
    }
}

/// The mayor's judgement: which want is loudest, measured in the same
/// coin so the comparison is honest. Boldness breaks ties toward works
/// (stone and homes) over comforts — a bold chain builds.
pub fn choose_lean(
    hungry: f32,
    roofless: usize,
    stone_short: f32,
    timber: f32,
    faithless: f32,
    boldness: f32,
) -> Lean {
    let mut wants = [
        (Lean::Larder, hungry * 1.2),
        (Lean::Homes, (roofless as f32) * 0.3),
        (
            Lean::Stone,
            (stone_short * 0.15).min(1.0) * (0.6 + boldness * 0.5),
        ),
        (
            Lean::Timber,
            ((8.0 - timber) / 8.0).clamp(0.0, 1.0) * (0.5 + boldness * 0.3),
        ),
        (Lean::Faith, faithless * 0.5),
    ];
    wants.sort_by(|a, b| b.1.total_cmp(&a.1));
    wants[0].0
}

/// Holds the vote wherever a hall stands and the chain is owed: no sitting
/// mayor, a new spring, or a town soured past bearing on its holder.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn hold_elections(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut last_held: Local<std::collections::HashMap<Entity, u32>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut proclaim: ResMut<ordo::Proclamations>,
    mut sounds: MessageWriter<crate::sfx::PlaySfx>,
    towns: Query<(Entity, &Settlement), With<SettlementGround>>,
    halls: Query<(&Building, &MemberOf, &Transform)>,
    mayors: Query<(Entity, &Mayor)>,
    mut chronicles: Query<&mut Chronicle>,
    folk: Query<
        (
            Entity,
            &MemberOf,
            &Person,
            &CreatureGenome,
            Option<&Regard>,
            Option<&crate::witness::Temperament>,
        ),
        (With<Villager>, Without<Corpse>),
    >,
) {
    // A year runs four seasons; the chain changes hands each spring.
    let year = crate::calendar::DAYS_PER_SEASON * crate::calendar::SEASONS_PER_YEAR;
    let day = clock.day();

    for (town, settlement) in &towns {
        let Some(hall) = halls
            .iter()
            .find(|(b, m, _)| b.kind == BuildingKind::TownHall && m.0 == town)
            .map(|(_, _, at)| at.translation)
        else {
            continue;
        };
        let adults: Vec<_> = folk
            .iter()
            .filter(|(_, m, _, genome, ..)| m.0 == town && genome.age == Age::Adult)
            .collect();
        if adults.len() < QUORUM {
            continue;
        }

        let sitting = mayors
            .iter()
            .find(|(who, mayor)| mayor.0 == town && folk.get(*who).is_ok());
        // A chain on a corpse is history, not office: taken off so the
        // dead never out-vote or out-decree the living.
        for (who, mayor) in &mayors {
            if mayor.0 == town && folk.get(who).is_err() {
                commands.entity(who).remove::<Mayor>();
            }
        }

        // Why the town votes today, if it does.
        let vacancy = sitting.is_none();
        let spring = day > 0 && day.is_multiple_of(year) && last_held.get(&town) != Some(&day);
        let soured = sitting.is_some_and(|(who, _)| {
            let (count, warmth) = adults
                .iter()
                .filter_map(|(_, _, _, _, regard, _)| regard.as_ref().map(|r| r.toward(who)))
                .filter(|w| w.abs() > 0.01)
                .fold((0usize, 0.0f32), |(n, sum), w| (n + 1, sum + w));
            count >= 6 && warmth / count as f32 <= RECALL_AT
        });
        if !vacancy && !spring && !soured {
            continue;
        }
        last_held.insert(town, day);

        // The ballot: every adult names the neighbour they hold warmest.
        // No hustings, no promises — the vote is the regard graph reading
        // itself out loud.
        let mut votes: std::collections::HashMap<Entity, (u32, f32)> =
            std::collections::HashMap::new();
        for (voter, _, _, _, regard, _) in &adults {
            let Some(regard) = regard else { continue };
            let pick = regard
                .bonds
                .iter()
                .filter(|bond| bond.warmth > 0.05)
                .filter(|bond| {
                    *voter != bond.toward && adults.iter().any(|(other, ..)| *other == bond.toward)
                })
                .max_by(|a, b| a.warmth.total_cmp(&b.warmth));
            if let Some(bond) = pick {
                let tally = votes.entry(bond.toward).or_insert((0, 0.0));
                tally.0 += 1;
                tally.1 += bond.warmth;
            }
        }
        // Most named first; total warmth breaks ties; nerve breaks those;
        // entity bits keep the last word deterministic.
        let chosen = adults
            .iter()
            .map(|(who, _, _, _, _, manner)| {
                let (named, warmth) = votes.get(who).copied().unwrap_or((0, 0.0));
                let nerve = manner.map_or(0.5, |t| t.boldness);
                (*who, named, warmth, nerve)
            })
            .max_by(|a, b| {
                a.1.cmp(&b.1)
                    .then(a.2.total_cmp(&b.2))
                    .then(a.3.total_cmp(&b.3))
                    .then(a.0.to_bits().cmp(&b.0.to_bits()))
            })
            .map(|(who, ..)| who);
        let Some(chosen) = chosen else { continue };
        let name = adults
            .iter()
            .find(|(who, ..)| *who == chosen)
            .map(|(_, _, person, ..)| person.name.clone())
            .unwrap_or_default();

        // The chain changes hands.
        if let Some((old, _)) = sitting {
            if old == chosen && !soured {
                // Re-elected in spring: the town simply keeps its mayor.
                continue;
            }
            commands.entity(old).remove::<Mayor>();
            if let Ok(mut story) = chronicles.get_mut(old) {
                story.record(
                    day,
                    if soured {
                        "was thrown out of the mayor's chain".to_string()
                    } else {
                        format!("handed the chain to {name}")
                    },
                );
            }
        }
        commands.entity(chosen).insert(Mayor(town));
        commands.entity(town).insert(CivicAssembly {
            mayor: chosen,
            hall,
            until: clock.elapsed + ASSEMBLY_DURATION,
            spoken: false,
        });
        if let Ok(mut story) = chronicles.get_mut(chosen) {
            story.record(day, format!("was chosen mayor of {}", settlement.name));
        }

        info!("{name} was chosen mayor of {}", settlement.name);
        notices.write(crate::ui::Notice::fanfare(format!(
            "{name} is chosen mayor of {}",
            settlement.name
        )));
        proclaim.push(ordo::Proclamation {
            title: "A MAYOR IS CHOSEN".into(),
            line: format!("{name} speaks for {}", settlement.name),
            color: crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.9),
            token: Some(chosen.to_bits()),
        });
        sounds.write(crate::sfx::PlaySfx {
            kind: crate::sfx::SfxKind::ProclaimGold,
            at: None,
        });
    }
}

/// Makes an election visible: the mayor comes to the hall and nearby idle
/// adults form a small, loose crowd. Work continues for everyone else.
#[allow(clippy::type_complexity)]
pub(super) fn stage_civic_assemblies(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut say: MessageWriter<crate::ui::Say>,
    mut assemblies: Query<(Entity, &mut CivicAssembly, Option<&CivicPriority>)>,
    grounds: Query<&SettlementGround>,
    ramparts: Query<&super::rampart::Rampart>,
    mut folk: Query<
        (
            Entity,
            &MemberOf,
            &Transform,
            &mut Activity,
            &mut MoveTarget,
            Option<&CivicGuest>,
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
) {
    for (town, mut assembly, priority) in &mut assemblies {
        if clock.elapsed >= assembly.until {
            commands.entity(town).remove::<CivicAssembly>();
            for (who, member, _, mut activity, mut target, guest) in &mut folk {
                if member.0 != town || guest.is_none() {
                    continue;
                }
                commands.entity(who).remove::<CivicGuest>();
                if *activity == Activity::Chatting {
                    *activity = Activity::Idle;
                    target.0 = None;
                }
            }
            continue;
        }

        let mut mayor_at = None;
        let inside_radius = ramparts.get(town).map_or_else(
            |_| grounds.get(town).map_or(36.0, |ground| ground.radius),
            |wall| wall.radius,
        );
        let centre = grounds
            .get(town)
            .map_or(assembly.hall, |ground| ground.centre);
        for (who, member, at, mut activity, mut target, guest) in &mut folk {
            if member.0 != town {
                continue;
            }
            if who == assembly.mayor {
                *activity = Activity::Chatting;
                target.0 = Some(assembly.hall);
                mayor_at = Some(at.translation);
                continue;
            }
            // The whole town hears the call, but a person already beyond the
            // wall stays on the road. An election does not pull a hunter or
            // explorer back across the countryside just to make a crowd.
            if guest.is_some() || at.translation.distance(centre) > inside_radius {
                continue;
            }
            let turn = who.index().index() as f32 * 2.399_963;
            let radius = 4.0 + (who.index().index() % 3) as f32 * 1.3;
            target.0 =
                Some(assembly.hall + Vec3::new(turn.cos() * radius, 0.0, turn.sin() * radius));
            *activity = Activity::Chatting;
            commands
                .entity(who)
                .remove::<super::home::Abed>()
                .insert(CivicGuest);
        }
        if !assembly.spoken
            && mayor_at.is_some_and(|at| at.distance(assembly.hall) < 4.0)
            && let Some(priority) = priority
        {
            say.write(crate::ui::Say {
                speaker: assembly.mayor,
                text: priority.lean.address().to_string(),
                thought: false,
                prayer: false,
            });
            assembly.spoken = true;
        }
    }
}

/// The mayor reads the town and leans it: the loudest want becomes the
/// standing decree, and the morning muster deals hands by it.
#[allow(clippy::type_complexity)]
pub(super) fn set_the_agenda(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mayors: Query<
        (
            Entity,
            &Mayor,
            &Person,
            Option<&crate::witness::Temperament>,
        ),
        Without<Corpse>,
    >,
    mut towns: Query<(Entity, &Stockpile, Option<&CivicPriority>)>,
    mut chronicles: Query<&mut Chronicle>,
    homeless: Query<
        &MemberOf,
        (
            With<Villager>,
            Without<super::home::Home>,
            Without<crate::creature::Childhood>,
            Without<Corpse>,
        ),
    >,
    faiths: Query<(&MemberOf, &super::belief::Faith), (With<Villager>, Without<Corpse>)>,
    mouths: Query<&MemberOf, (With<Villager>, Without<Corpse>)>,
) {
    let day = clock.day();
    for (mayor_entity, mayor, person, manner) in &mayors {
        let Ok((town, store, standing)) = towns.get_mut(mayor.0) else {
            continue;
        };
        // A fresh decree weekly, or the moment the office is taken up.
        if standing.is_some_and(|held| day.saturating_sub(held.set_on) < 7) {
            continue;
        }

        let heads = mouths.iter().filter(|m| m.0 == town).count().max(1) as f32;
        let floor = (heads * 1.2).max(12.0);
        let hungry = ((floor - store.food()) / floor).clamp(0.0, 1.0);
        let roofless = homeless.iter().filter(|m| m.0 == town).count();
        let stone_short = (8.0 - (store.stone + store.clay)).max(0.0);
        let believers = faiths
            .iter()
            .filter(|(m, faith)| m.0 == town && faith.is_believer())
            .count() as f32;
        let faithless = (1.0 - believers / heads).clamp(0.0, 1.0);
        let boldness = manner.map_or(0.5, |t| t.boldness);

        let lean = choose_lean(
            hungry,
            roofless,
            stone_short,
            store.timber,
            faithless,
            boldness,
        );
        if standing.is_some_and(|held| held.lean == lean) {
            // Same judgement as last week: the decree stands unspoken.
            commands
                .entity(town)
                .insert(CivicPriority { lean, set_on: day });
            continue;
        }
        commands
            .entity(town)
            .insert(CivicPriority { lean, set_on: day });
        notices.write(crate::ui::Notice::new(format!(
            "Mayor {} {}",
            person.name,
            lean.decree()
        )));
        if let Ok(mut story) = chronicles.get_mut(mayor_entity) {
            story.record(day, format!("decreed: the town {}", lean.decree()));
        }
        info!("mayor {} {}", person.name, lean.decree());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mayor_never_starves_the_other_work() {
        // A lean only ever raises wants. Scaling one down is how a decree
        // would fight the villages-must-thrive law.
        for lean in [
            Lean::Larder,
            Lean::Timber,
            Lean::Stone,
            Lean::Homes,
            Lean::Faith,
        ] {
            for vocation in [
                Vocation::Gatherer,
                Vocation::Fisher,
                Vocation::Hunter,
                Vocation::Miner,
                Vocation::Forester,
                Vocation::Builder,
                Vocation::Farmer,
                Vocation::Cook,
                Vocation::Healer,
                Vocation::Priest,
                Vocation::Explorer,
                Vocation::Guard,
            ] {
                assert!(
                    lean_scale(lean, vocation) >= 1.0,
                    "{lean:?} would strip hands from {vocation:?}"
                );
            }
        }
    }

    #[test]
    fn the_judgement_answers_the_loudest_want() {
        // Starving town: the larder, whoever holds the chain.
        assert_eq!(choose_lean(1.0, 1, 2.0, 20.0, 0.2, 0.5), Lean::Larder);
        // Fed and housed but faithless: the shrine.
        assert_eq!(choose_lean(0.0, 0, 0.0, 20.0, 1.0, 0.3), Lean::Faith);
        // Roofless folk outweigh a thin woodpile.
        assert_eq!(choose_lean(0.0, 5, 0.0, 20.0, 0.1, 0.5), Lean::Homes);
        // A bold mayor with an empty pile builds.
        assert_eq!(choose_lean(0.0, 0, 8.0, 20.0, 0.1, 0.9), Lean::Stone);
    }

    #[test]
    fn civic_systems_initialise_without_query_conflicts() {
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.init_resource::<crate::calendar::WorldClock>();
        app.init_resource::<ordo::Proclamations>();
        app.add_message::<crate::ui::Notice>();
        app.add_message::<crate::ui::Say>();
        app.add_message::<crate::sfx::PlaySfx>();

        app.world_mut().run_system_once(hold_elections).unwrap();
        app.world_mut().run_system_once(set_the_agenda).unwrap();
        app.world_mut()
            .run_system_once(stage_civic_assemblies)
            .unwrap();
    }

    /// The ballot crowns whoever the town holds warmest — the regard
    /// graph reading itself out loud.
    #[test]
    fn the_vote_crowns_the_warmest() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<crate::calendar::WorldClock>();
        app.init_resource::<ordo::Proclamations>();
        app.add_message::<crate::ui::Notice>();
        app.add_message::<crate::sfx::PlaySfx>();

        let town = app
            .world_mut()
            .spawn((
                Settlement {
                    name: "Tutu".into(),
                    founded: 0,
                    banner_ramp: 0,
                    sigil: 0,
                },
                SettlementGround {
                    centre: Vec3::ZERO,
                    radius: 36.0,
                    woodpile: Vec3::ZERO,
                    foodpile: Vec3::ZERO,
                },
            ))
            .id();
        app.world_mut().spawn((
            Building {
                kind: BuildingKind::TownHall,
            },
            MemberOf(town),
            Transform::default(),
        ));

        let mut rng = crate::rng::Rng::new(11);
        let souls: Vec<Entity> = (0..8)
            .map(|n| {
                app.world_mut()
                    .spawn((
                        Villager,
                        MemberOf(town),
                        Person::born(format!("Soul{n}"), "Test".into()),
                        CreatureGenome::adult(
                            crate::creature::genome::Species::Human,
                            crate::creature::genome::Sex::Female,
                            &mut rng,
                        ),
                    ))
                    .id()
            })
            .collect();
        // Everyone warms to Soul3; Soul3 warms to Soul0.
        for (n, soul) in souls.iter().enumerate() {
            let mut regard = Regard::default();
            if n == 3 {
                regard.warm(souls[0], 0.4);
            } else {
                regard.warm(souls[3], 0.6);
            }
            app.world_mut().entity_mut(*soul).insert(regard);
        }

        app.world_mut().run_system_once(hold_elections).unwrap();
        assert!(
            app.world().get::<Mayor>(souls[3]).is_some(),
            "the warmest-held soul takes the chain",
        );
    }
}
