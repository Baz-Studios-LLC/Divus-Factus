//! Work: vocations, the settlement stockpile, and the working day.
//!
//! Every adult takes up a calling, weighted by temperament — the bold hunt,
//! the timid gather — and works it through the daylight hours. Work fills the
//! settlement's stockpile; the stockpile feeds anyone who cannot find a bush
//! when hunger comes. That one loop is the village's first economy, and the
//! first thing the player can *watch fail*: a store that empties is a famine
//! the god saw coming.
//!
//! Nobody is assigned anything. A vocation is rolled from who the person is,
//! and where they work emerges from where the fish, the game, the stone and
//! the woods actually are around this settlement on this seed.

use bevy::prelude::*;

use super::{
    Activity, Chronicle, HUNGRY_THRESHOLD, Needs, Person, SECONDS_TO_STARVE, SettlementSite,
    SimRng, Villager,
};
use crate::creature::anim::CreatureMotion;
use crate::creature::genome::{CreatureGenome, Species};
use crate::creature::{Airborne, Corpse, Creature, Held, MoveTarget, Vitality};
use crate::scatter::FoodSource;
use crate::terrain::Terrain;

pub(crate) mod baked;
pub(crate) mod buildings;
mod carry;
mod fields;
mod jobs;
mod stores;
mod vocation;

pub use buildings::*;
pub use carry::*;
pub use fields::*;
pub use jobs::*;
pub use stores::*;
pub use vocation::*;

/// How long one unit of work takes, standing at the worksite.
const WORK_SECONDS: f32 = 6.0;

/// How close counts as being at the worksite.
const WORK_RANGE: f32 = 2.8;

/// How many pieces a builder shoulders in one trip to the stores.
const A_LOAD: f32 = 4.0;

/// What one trip to the rock is worth, before craft. A single block was
/// the old figure, and the walk out to a boulder is long enough that a
/// four-stone footing cost a founding village most of a morning with
/// sixty timber already stacked beside the plot. A barrow, not a pocket.
const LOOSE_STONE: f32 = 2.5;

/// How far afield anyone will go to work.
pub(crate) const WORK_REACH: f32 = 170.0;

/// How far the stone plinth rises above grade. Every building's timber sits
/// on this, and the plinth reaches well below grade — so nothing clips into
/// a slope.
const PLINTH_TOP: f32 = 0.35;

/// Hunger above which a worker downs tools and sees to themself.
const DOWN_TOOLS_HUNGER: f32 = HUNGRY_THRESHOLD + 0.1;

/// What one hunt's kill is worth in stored food.
const CARCASS_FOOD: f32 = 3.0;

/// How close a hunter must be to strike. Longer than the bench-worker's
/// arm on purpose: prey drifts a few steps at a time while it grazes,
/// and a hunter held to the ordinary two-and-a-bit strides spent whole
/// mornings arriving at where a deer had just been - patience ran out,
/// the deer was shunned, and a village with prey nineteen strides from
/// the square brought home no meat at all.
const SPEAR_REACH: f32 = 4.2;

/// Carriers walk their wood to the pile and put it down.
pub(super) fn haul_wood(
    mut commands: Commands,
    members: Query<&crate::villager::MemberOf>,
    mut towns: Query<(&crate::villager::SettlementGround, &mut Stockpile)>,
    children: Query<&Children>,
    loads: Query<Entity, With<WoodLoad>>,
    mut haulers: Query<
        (
            Entity,
            &Transform,
            &CarryingWood,
            &mut Activity,
            &mut MoveTarget,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    for (entity, transform, carrying, mut activity, mut target) in &mut haulers {
        match *activity {
            Activity::Hauling => {}
            // A carry interrupted — thrown by the god, woken from sleep —
            // resumes as soon as their feet are their own again.
            Activity::Idle | Activity::Wandering => {
                *activity = Activity::Hauling;
            }
            _ => continue,
        }

        // Timber goes to the hauler's OWN woodpile. Reaching for the focused
        // town's pile would have every carpenter in the world walking to one
        // square with their armful.
        let Ok(&crate::villager::MemberOf(home)) = members.get(entity) else {
            continue;
        };
        let Ok((ground, mut store)) = towns.get_mut(home) else {
            continue;
        };

        if transform.translation.distance(ground.woodpile) > 2.6 {
            target.0 = Some(ground.woodpile);
            continue;
        }
        // The far end of the only resource in this village that has to be
        // CARRIED. Stone and food are credited where they are won; timber is
        // won in the woods and only becomes the village's here. So when the
        // stores read plenty of food, rising stone and a flat nought of
        // timber - which is exactly what Brett was looking at - the question
        // is which half of that walk failed, and until these two lines existed
        // there was no way to tell from outside.
        info!("{:.0} timber reached the pile", carrying.amount);
        store.timber += carrying.amount;
        commands.entity(entity).remove::<CarryingWood>();
        shed_wood(&mut commands, entity, &children, &loads);
        *activity = Activity::Idle;
        target.0 = None;
    }
}

/// How near an offering must land to count as given, to a build site or to
/// the village stores.
const OFFERING_REACH: f32 = 9.0;

/// What a god-given thing is worth, in the store's own units.
///
/// A tree pays what felling it would have paid - the god saves the walk and
/// the axe-work, not the arithmetic - and stone pays by the mass of the rock,
/// so an outcrop-sized boulder is worth a working day and a loaf-sized stone
/// is worth a stone.
pub(crate) fn offering_worth(matter: &crate::matter::Matter) -> (f32, f32) {
    match matter.substance {
        crate::matter::Substance::Wood => {
            // `Matter::felled_tree` writes mass as 30 + maturity * 50, so this
            // reads maturity back out and pays the feller's own yield.
            let maturity = ((matter.mass - 30.0) / 50.0).clamp(0.0, 1.0);
            (2.0 + maturity, 0.0)
        }
        crate::matter::Substance::Stone => (0.0, (matter.mass / 60.0).clamp(1.5, 8.0)),
        crate::matter::Substance::Plant => (0.0, 0.0),
    }
}

/// The god's own hand provisions a build.
///
/// A tree or boulder carried by the hand and set down at a construction site -
/// or on the village stores - becomes the timber or stone it is made of.
/// Brett: "I want the ability to pick up trees and rocks and help them build
/// with them similar to B&W", and this is that loop: uproot a tree, drop it on
/// the scaffold, and the village builds with what the god gave.
///
/// Only the DIVINELY PLACED convert - the mark the hand leaves on whatever it
/// sets down, already load-bearing for answered prayers. Without that gate
/// every boulder lying naturally beside a build site would quietly become
/// masonry, and the scenery would feed the economy on its own.
pub(super) fn receive_offerings(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    offered: Query<
        (
            Entity,
            &Transform,
            Option<&crate::matter::Matter>,
            Option<&crate::matter::Lump>,
            Option<&Goods>,
            Option<&crate::scatter::FoodSource>,
            Option<&crate::scatter::SacredFlora>,
            Option<&crate::creature::genome::CreatureGenome>,
            Has<crate::creature::wildlife::Wild>,
        ),
        (
            With<crate::hand::DivinelyPlaced>,
            Without<crate::creature::Held>,
            Without<Airborne>,
            Without<crate::villager::Person>,
        ),
    >,
    mut sites: Query<(
        &Transform,
        &crate::villager::MemberOf,
        &mut ConstructionSite,
        &Blueprint,
    )>,
    mut towns: Query<(Entity, &crate::villager::SettlementGround, &mut Stockpile)>,
    mut fires: Query<(&Transform, &mut crate::villager::home::Bonfire)>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut witnessed: MessageWriter<crate::witness::DivineEvent>,
) {
    use crate::palette;
    for (offering, at, matter, lump, goods, food, sacred, genome, wild) in &offered {
        // What this thing is worth, and what colour its essence bursts in.
        // Wood pays the feller's rate, stone the rock's mass, a berry bush
        // the meals still on it, and a wild animal the hunter's own carcass
        // rate - Brett: "same with rocks or food (I should be able to grab
        // bushes or animals for the food)". A PERSON is never an offering,
        // which the query above enforces before any arithmetic can.
        // (timber, stone, clay, ore, berries, meat, incense, dye) - the whole
        // pantry - plus the colours its essence bursts in.
        let mut pays = [0.0f32; 8];
        // A withdrawn parcel FIRST, before the generic matter arm can claim
        // it: parcels wear `Matter` for physics, and the matter arm priced
        // every one of them as the stone its boulder-body pretended to be -
        // clay came back as masonry and burst in grey. Brett: "the particles
        // are stone colored not clay colored... same for food and wood." A
        // parcel pays back exactly the kind and amount that was drawn.
        let colors: Vec<Color> = if let Some(parcel) = goods {
            match parcel.kind {
                PileKind::Timber => {
                    pays[0] = parcel.amount;
                    vec![
                        palette::shade(&palette::WOOD, 0.55),
                        palette::shade(&palette::WOOD, 0.4),
                    ]
                }
                PileKind::Stone => {
                    pays[1] = parcel.amount;
                    vec![
                        palette::shade(&palette::STONE, 0.6),
                        palette::shade(&palette::STONE, 0.85),
                    ]
                }
                PileKind::Clay => {
                    pays[2] = parcel.amount;
                    vec![Color::srgb(0.62, 0.36, 0.24), Color::srgb(0.5, 0.28, 0.2)]
                }
                PileKind::Ore => {
                    pays[3] = parcel.amount;
                    vec![Color::srgb(0.48, 0.26, 0.14), Color::srgb(0.3, 0.3, 0.34)]
                }
                PileKind::Food => {
                    pays[4] = parcel.amount;
                    vec![
                        palette::shade(&palette::CLOTH_RED, 0.85),
                        palette::shade(&palette::BONE, 0.75),
                    ]
                }
            }
        } else if let Some(lump) = lump {
            match lump.kind {
                crate::matter::DepositKind::Clay => {
                    pays[2] = lump.amount;
                    vec![Color::srgb(0.62, 0.36, 0.24), Color::srgb(0.5, 0.28, 0.2)]
                }
                crate::matter::DepositKind::Iron => {
                    pays[3] = lump.amount;
                    vec![Color::srgb(0.48, 0.26, 0.14), Color::srgb(0.3, 0.3, 0.34)]
                }
                crate::matter::DepositKind::Stone => {
                    pays[1] = lump.amount;
                    vec![
                        palette::shade(&palette::STONE, 0.6),
                        palette::shade(&palette::STONE, 0.85),
                    ]
                }
            }
        } else if let Some(flora) = sacred {
            match flora.kind {
                crate::scatter::SacredKind::Incense => {
                    pays[6] = flora.amount;
                    vec![
                        palette::shade(&palette::BONE, 0.85),
                        palette::shade(&palette::STONE, 0.9),
                    ]
                }
                crate::scatter::SacredKind::Dye => {
                    pays[7] = flora.amount;
                    vec![
                        palette::shade(&palette::CLOTH_RED, 0.9),
                        palette::shade(&palette::GRASS, 0.65),
                    ]
                }
            }
        } else if let Some(source) = food {
            pays[4] = source.amount;
            vec![
                palette::shade(&palette::CLOTH_RED, 0.85),
                palette::shade(&palette::GRASS, 0.6),
            ]
        } else if wild && genome.is_some() {
            pays[5] = CARCASS_FOOD;
            vec![
                palette::shade(&palette::CLOTH_RED, 0.55),
                palette::shade(&palette::BONE, 0.9),
            ]
        } else if let Some(matter) = matter {
            let (t, s) = offering_worth(matter);
            pays[0] = t;
            pays[1] = s;
            match matter.substance {
                crate::matter::Substance::Wood => vec![
                    palette::shade(&palette::WOOD, 0.55),
                    palette::shade(&palette::GRASS, 0.55),
                ],
                crate::matter::Substance::Stone => vec![
                    palette::shade(&palette::STONE, 0.6),
                    palette::shade(&palette::STONE, 0.85),
                ],
                crate::matter::Substance::Plant => vec![],
            }
        } else {
            continue;
        };
        let [
            mut timber,
            mut stone,
            clay,
            ore,
            berries,
            meat,
            incense,
            dye,
        ] = pays;
        if pays.iter().all(|worth| *worth <= 0.0) {
            continue;
        }
        let here = at.translation;

        // Wood dropped ON the fire burns instead of banking. Brett: "the
        // god should be able to add fuel to the fire the same way they add
        // trees to the stockpile." A tight reach on purpose - the bonfire
        // stands in the square within a few strides of the woodpile, and
        // anything generous here would swallow every gift meant for the
        // stores. You have to drop it on the flames.
        const ONTO_THE_FLAMES: f32 = 3.0;
        if timber > 0.0
            && let Some((fire_at, mut fire)) = fires
                .iter_mut()
                .find(|(at, _)| at.translation.distance(here) < ONTO_THE_FLAMES)
        {
            let was_out = fire.fuel <= 0.0;
            // Paid at the villagers' own rate, so a god-given tree buys
            // exactly what the same tree buys carried in by hand.
            fire.fuel += timber * crate::villager::home::SECONDS_PER_LOG;
            crate::matter::burst_of(
                &mut commands,
                &mut meshes,
                &mut materials,
                here,
                fire_at.translation,
                &colors,
                18,
            );
            commands.entity(offering).despawn();
            notices.write(crate::ui::Notice::new(if was_out {
                "The god rekindles the village fire"
            } else {
                "The god feeds the village fire"
            }));
            witnessed.write(crate::witness::DivineEvent {
                kind: crate::witness::DivineEventKind::Provided,
                position: here,
                subject: None,
                intensity: 0.6,
            });
            continue;
        }

        // A build site takes the offering for its own town; failing that, the
        // village stores take it at the woodpile. Whichever takes it is also
        // where the burst GATHERS - the essence visibly joins what took it.
        //
        // Dropped ON a rising building, the materials go into THAT BUILDING
        // first - "dropping resources on a partially built building should
        // add to the building's missing materials" - stone into the footing
        // it still wants, timber into the frame's progress. The last half
        // log is left for a carpenter's hands, the same line helpers hold:
        // many hands and even the god's speed a build, but a person finishes
        // it. Whatever the site cannot take spills over into the stores.
        let mut town_and_home = None;
        for (site_at, member, mut construction, plan) in &mut sites {
            if site_at.translation.distance(here) >= OFFERING_REACH {
                continue;
            }
            let footing_want =
                (construction.footing_stone(plan.kind) - construction.stone_laid).max(0.0);
            let laid = stone.min(footing_want);
            construction.stone_laid += laid;
            stone -= laid;

            let frame_want = (plan.kind.timber_cost() - 0.5 - construction.progress).max(0.0);
            let framed = timber.min(frame_want);
            construction.progress += framed;
            timber -= framed;

            // The burst tells the truth about where the gift went: into the
            // frame while the frame wants it, and to the STORES once the
            // site is full - a drop on a finished frame gathered into the
            // scaffold anyway, and read as swallowed. Brett: "adding more
            // wood doesn't bring the number up any... but the wood still
            // goes into it?" It goes to the pile, and now it visibly does.
            town_and_home = Some((member.0, site_at.translation, laid + framed > 0.0));
            break;
        }
        let taker = town_and_home
            .map(|(town, site, took)| (town, (took).then_some(site)))
            .or_else(|| {
                towns
                    .iter()
                    .filter(|(_, ground, _)| {
                        ground.woodpile.distance(here) < OFFERING_REACH
                            || ground.foodpile.distance(here) < OFFERING_REACH
                    })
                    .map(|(town, _, _)| (town, None))
                    .next()
            });
        let Some((town, site_home)) = taker else {
            continue;
        };
        let Ok((_, ground, mut store)) = towns.get_mut(town) else {
            continue;
        };
        // Food gathers to the food sacks, everything else to the timber
        // side — and both points follow their piles indoors, so a gift
        // dropped on a town with a granary visibly pours into the granary.
        let home = site_home.unwrap_or(if berries + meat > 0.0 {
            ground.foodpile
        } else {
            ground.woodpile
        });

        store.timber += timber;
        store.stone += stone;
        store.clay += clay;
        store.ore += ore;
        store.incense += incense;
        store.dye += dye;
        if berries > 0.0 {
            store.larder.add(FoodKind::Berries, berries);
        }
        if meat > 0.0 {
            store.larder.add(FoodKind::Meat, meat);
        }
        // The pop: the thing becomes its essence where everyone can see it,
        // bursting in its own colours and gathering into what took it.
        crate::matter::burst_of(
            &mut commands,
            &mut meshes,
            &mut materials,
            here,
            home,
            &colors,
            18,
        );
        commands.entity(offering).despawn();
        // Named by what it WAS, not what spilled over: pays[] still holds the
        // whole gift even where the site swallowed a share of it.
        let [t, s, c, o, b, m, i, d] = pays;
        let named = [
            ("timber", t),
            ("stone", s),
            ("clay", c),
            ("ore", o),
            ("food", b + m),
            ("incense", i),
            ("dye", d),
        ];
        let (name, worth) = named
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .copied()
            .unwrap_or(("providence", 0.0));
        notices.write(crate::ui::Notice::new(format!(
            "The god provides: {worth:.0} {name}"
        )));
        // Seen and believed: providence with witnesses is the whole game.
        witnessed.write(crate::witness::DivineEvent {
            kind: crate::witness::DivineEventKind::Provided,
            position: here,
            subject: None,
            intensity: 0.6,
        });
    }
}

/// The pile shows what the store holds, log by log.
pub(super) fn update_woodpile(
    stores: Query<&Stockpile>,
    piles: Query<&crate::villager::MemberOf>,
    moving: Query<&Rehouse>,
    mut logs: Query<(&WoodpileLog, &ChildOf, &mut Visibility)>,
) {
    for (log, parent, mut visibility) in &mut logs {
        // A log belongs to a pile, and a pile to a town: each settlement's
        // woodpile shows its own timber, not the focused town's.
        let Some(store) = piles
            .get(parent.parent())
            .ok()
            .and_then(|member| stores.get(member.0).ok())
        else {
            continue;
        };
        let away = moving.get(parent.parent()).map_or(0.0, |r| r.hauled as f32);
        let shown = (log.0 as f32) < store.timber.min(24.0) - away;
        let wanted = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// A neighbour pitching in at someone else's build site.
#[derive(Component)]
pub struct Helper(pub Entity);

/// The bored and good-hearted drift over to help a build in progress:
/// steadying the frame adds real progress, at half a worker's pace. The
/// slothful, famously, do not.
#[allow(clippy::type_complexity)]
pub(super) fn lend_a_hand(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut rng: ResMut<SimRng>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sites: Query<(Entity, &Transform, &mut ConstructionSite, &Blueprint)>,
    helpers_now: Query<&Helper>,
    mut folk: Query<
        (
            Entity,
            &Transform,
            &mut Activity,
            &mut MoveTarget,
            &mut CreatureMotion,
            Option<&Helper>,
            &Needs,
            Option<&super::traits::Traits>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
            Without<crate::creature::Childhood>,
        ),
    >,
) {
    let dt = time.delta_secs();
    if !is_work_hour(clock.time_of_day()) {
        return;
    }

    // Helpers at work: walk in, steady the frame. Hunger releases them
    // like any worker - a helper carries no Job, so the ordinary
    // down-tools check never sees them, and before this check the
    // good-hearted starved at stalled sites they could not finish.
    for (villager, at, mut activity, mut target, mut motion, helper, needs, _) in &mut folk {
        let Some(helper) = helper else {
            continue;
        };
        let hungry = needs.hunger > DOWN_TOOLS_HUNGER;
        let done = sites.get_mut(helper.0).is_err();
        if done || hungry || *activity != Activity::Working {
            commands.entity(villager).remove::<Helper>();
            if *activity == Activity::Working {
                *activity = Activity::Idle;
                target.0 = None;
            }
            continue;
        }
        let Ok((_, site_at, mut construction, plan)) = sites.get_mut(helper.0) else {
            continue;
        };
        if at.translation.distance(site_at.translation) > 3.0 {
            target.0 = Some(site_at.translation);
        } else {
            target.0 = None;
            motion.flail = motion.flail.max(0.25);
            // A neighbour's labour speeds the work but cannot finish it:
            // the last of a building takes a carpenter's hand, so helped
            // progress stops just short and never runs past the cost —
            // "22 of 6 timber" is a lie no site should tell.
            let cost = plan.kind.timber_cost();
            construction.progress = (construction.progress + dt / WORK_SECONDS * 0.5)
                .min(cost - 0.5)
                .max(construction.progress);
            // Helped work shows: the frame rises under many hands too.
            let target_stage = stage_for(construction.progress, cost, steps_for(plan));
            while construction.stage < target_stage {
                construction.stage += 1;
                raise_stage(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    helper.0,
                    construction.stage,
                    plan,
                );
            }
        }
    }

    // Recruit: an idle neighbour near an active site, two helpers at most.
    // Only where the work can actually advance — a site still waiting on
    // its foundation stone needs a mason, not a crowd.
    for (site, site_at, construction, plan) in &sites {
        if construction.stone_laid < construction.footing_stone(plan.kind) {
            continue;
        }
        let already = helpers_now.iter().filter(|h| h.0 == site).count();
        if already >= 2 {
            continue;
        }
        for (villager, at, mut activity, _, _, helper, _, manner) in &mut folk {
            if helper.is_some()
                || !matches!(
                    *activity,
                    Activity::Idle | Activity::Wandering | Activity::Sheltering
                )
                || manner.is_some_and(|m| m.has(super::traits::Trait::Slothful))
                || at.translation.distance(site_at.translation) > 45.0
                || !rng.0.chance(0.01)
            {
                continue;
            }
            *activity = Activity::Working;
            commands.entity(villager).insert(Helper(site));
            break;
        }
    }
}

/// This villager is fetching a loose log home.
#[derive(Component)]
pub struct SalvageHauler(pub Entity);

/// Loose felled timber near the village gets carried to the pile. A log
/// still carrying the divine mark counts as providence when it lands.
#[allow(clippy::type_complexity)]
pub(super) fn salvage_timber(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    members: Query<&crate::villager::MemberOf>,
    mut towns: Query<(Entity, &crate::villager::SettlementGround, &mut Stockpile)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut witnessed: MessageWriter<crate::witness::DivineEvent>,
    children: Query<&Children>,
    loads: Query<Entity, With<WoodLoad>>,
    logs: Query<
        (
            Entity,
            &Transform,
            &crate::matter::Matter,
            Has<crate::hand::DivinelyPlaced>,
        ),
        (
            Without<crate::scatter::FellableTree>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
            Without<Villager>,
        ),
    >,
    mut villagers: Query<
        (
            Entity,
            &Transform,
            &mut Activity,
            &mut MoveTarget,
            Option<&mut SalvageHauler>,
            Option<&mut Chronicle>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
        ),
    >,
) {
    // Every town's square, gathered before the mutable loops below: a loose
    // log is worth salvaging if it lies near ANY settlement, not just the one
    // the player is watching.
    let centres: Vec<Vec3> = towns.iter().map(|(_, ground, _)| ground.centre).collect();

    // Carriers in progress first.
    let mut carrying: Vec<Entity> = Vec::new();
    for (villager, at, mut activity, mut target, hauler, chronicle) in &mut villagers {
        let Some(hauler) = hauler else {
            continue;
        };
        if *activity != Activity::Hauling {
            commands.entity(villager).remove::<SalvageHauler>();
            shed_wood(&mut commands, villager, &children, &loads);
            continue;
        }
        match logs.get(hauler.0) {
            Ok((log, log_at, _, marked)) => {
                carrying.push(log);
                if at.translation.distance(log_at.translation) > 2.0 {
                    target.0 = Some(log_at.translation);
                } else {
                    // Shoulder it: the log entity vanishes into the load.
                    info!("a loose log was salvaged by hand (divinely marked: {marked})");
                    commands.entity(log).despawn();
                    shoulder_wood(&mut commands, &mut meshes, &mut materials, villager);
                    if marked {
                        // Collected straight from the god's hand.
                        witnessed.write(crate::witness::DivineEvent {
                            kind: crate::witness::DivineEventKind::Provided,
                            position: at.translation,
                            subject: Some(villager),
                            intensity: 0.6,
                        });
                        if let Some(mut chronicle) = chronicle {
                            chronicle
                                .record(clock.day(), "gathered what the god set down".to_string());
                        }
                    }
                    // Remember the errand is now homeward: retarget the pile.
                    commands
                        .entity(villager)
                        .insert(SalvageHauler(Entity::PLACEHOLDER));
                }
            }
            Err(_) => {
                // Log gone (or already shouldered): walk it home — to the
                // carrier's own woodpile.
                let Some((pile, town)) = members
                    .get(villager)
                    .ok()
                    .and_then(|m| towns.get(m.0).ok().map(|(t, g, _)| (g.woodpile, t)))
                else {
                    continue;
                };
                if at.translation.distance(pile) > 2.2 {
                    target.0 = Some(pile);
                } else {
                    if let Ok((_, _, mut store)) = towns.get_mut(town) {
                        store.timber += 2.0;
                    }
                    shed_wood(&mut commands, villager, &children, &loads);
                    commands.entity(villager).remove::<SalvageHauler>();
                    *activity = Activity::Idle;
                    target.0 = None;
                }
            }
        }
    }

    // Recruit one idle villager per unclaimed log near the village.
    for (log, log_at, matter, _) in &logs {
        if matter.substance != crate::matter::Substance::Wood {
            continue;
        }
        if carrying.contains(&log) {
            continue;
        }
        if !centres
            .iter()
            .any(|centre| log_at.translation.distance(*centre) <= 70.0)
        {
            continue;
        }
        let volunteer = villagers
            .iter_mut()
            .filter(|(_, _, activity, _, hauler, _)| {
                hauler.is_none()
                    && matches!(
                        **activity,
                        Activity::Idle | Activity::Wandering | Activity::Sheltering
                    )
            })
            .min_by(|a, b| {
                a.1.translation
                    .distance(log_at.translation)
                    .total_cmp(&b.1.translation.distance(log_at.translation))
            });
        let Some((villager, _, mut activity, _, _, _)) = volunteer else {
            break;
        };
        *activity = Activity::Hauling;
        commands.entity(villager).insert(SalvageHauler(log));
    }
}

/// Whether a build site is somewhere people will sleep, for the
/// personal-stake speedup. Either roof counts: a carpenter with no bed of
/// their own is as invested in the longhouse going up as in a house.
fn plan_kind_is_home(
    build_sites: &Query<(&mut ConstructionSite, &mut Blueprint)>,
    site: Entity,
) -> bool {
    build_sites
        .get(site)
        .is_ok_and(|(_, plan)| matches!(plan.kind, BuildingKind::House | BuildingKind::Longhouse))
}

/// Work gets done: walk there, do the thing, and the stockpile grows.
pub(super) fn do_work(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut rng: ResMut<SimRng>,
    mut towns: Query<(&crate::villager::SettlementGround, &mut Stockpile)>,
    members: Query<&crate::villager::MemberOf>,
    mut workers: Query<
        (
            Entity,
            &Transform,
            &mut Needs,
            &Vocation,
            &mut Activity,
            &mut Job,
            &mut MoveTarget,
            &mut CreatureMotion,
            &Person,
            (
                Option<&mut Chronicle>,
                Option<&super::traits::Traits>,
                Option<&super::home::Home>,
                Option<&mut Rations>,
                Option<&mut Skills>,
            ),
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
    mut bushes: Query<&mut FoodSource, Without<Villager>>,
    mut trees: Query<
        (
            &mut crate::scatter::FellableTree,
            &Transform,
            &crate::scatter::TreeBody,
            &crate::scatter::InGrove,
        ),
        (Without<Villager>, Without<crate::matter::Boulder>),
    >,
    mut boulders_mut: Query<
        (&mut Transform, &crate::matter::Boulder),
        (Without<Villager>, Without<Creature>),
    >,
    context: (
        Query<&CarryingWood>,
        Query<&Children>,
        Query<Entity, With<WoodLoad>>,
        Query<&Building>,
    ),
    trades: (
        Query<&CarryingStone>,
        Query<(&Transform, &mut Vitality), (With<Villager>, Without<Corpse>)>,
        Query<(&mut Field, &Transform), Without<crate::matter::Boulder>>,
        ResMut<KitchenWarm>,
        Query<&mut crate::matter::Deposit>,
        Query<&mut crate::scatter::SacredFlora>,
    ),
    civic: (
        Query<(&mut ConstructionSite, &mut Blueprint)>,
        Query<&super::Settlement>,
        MessageWriter<crate::ui::Notice>,
        MessageWriter<crate::witness::DivineEvent>,
        // Which sites are holdings, and where each site stands — a homestead
        // breaks its own ground the moment its roof is on.
        Query<(), With<Homestead>>,
        // Disjoint from the boulder query above, which takes Transform
        // mutably. Bevy reasons on component sets, not on what can actually
        // co-occur, so a build site and a boulder must be told apart
        // explicitly even though no entity is ever both.
        Query<&Transform, (With<Blueprint>, Without<crate::matter::Boulder>)>,
    ),
    ground: (
        Res<Terrain>,
        ResMut<crate::terrain::LoadedChunks>,
        ResMut<crate::grass::GrassChunks>,
        Option<Res<crate::weather::Weather>>,
        ResMut<crate::scatter::StrippedGround>,
        Res<crate::terrain::TerrainAssets>,
        ResMut<crate::scatter::DirtyGroves>,
    ),
    assets: (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    // CreatureMotion is OPTIONAL, and the whole hunt hangs on it: death
    // strips a body of its motion, so a required `&mut CreatureMotion`
    // quietly dropped every carcass out of this query the instant the
    // kill landed. The hunter then "lost the quarry", went idle, was
    // dealt the same carcass again, lost it again - three and a half
    // thousand times in four minutes - and the harvest arm below, which
    // exists precisely to turn a corpse into meat, could never run. A
    // village in the deep woods with prey twenty strides from the square
    // brought home nothing and starved to six.
    mut prey_query: Query<
        (
            &Transform,
            &mut Vitality,
            Option<&mut CreatureMotion>,
            Has<Corpse>,
            &CreatureGenome,
        ),
        (With<Creature>, Without<Villager>),
    >,
) {
    let dt = time.delta_secs();
    // DIVUS_FACTUS_HUNT_PROBE=1 narrates every hunt: the deal, the chase,
    // the shun, the strike. The woods famine was a village that starved
    // with prey twenty strides out, and the census alone could not say
    // which link of the hunt was broken.
    let hunt_probe = std::env::var("DIVUS_FACTUS_HUNT_PROBE").is_ok();
    // Mouths per town, so one settlement's thin larder does not push
    // another settlement's trades into working after dark.
    let mut alive_by_town: std::collections::HashMap<Entity, usize> =
        std::collections::HashMap::new();
    for (worker, ..) in workers.iter() {
        if let Ok(member) = members.get(worker) {
            *alive_by_town.entry(member.0).or_default() += 1;
        }
    }
    let (ref carrying, ref children, ref loads, ref _buildings) = context;
    let (carrying, children, loads) = (carrying, children, loads);
    let (
        carrying_stone,
        mut patients,
        mut fields_mut,
        mut kitchen,
        mut deposits_mut,
        mut sacred_mut,
    ) = trades;
    let (mut build_sites, settlements, mut notices, mut witnessed, steadings, build_at) = civic;
    let (terrain, mut chunks, mut grass, weather, mut stripped, terrain_assets, mut dirty_groves) =
        ground;
    let (mut meshes, mut materials) = assets;

    for (
        entity,
        transform,
        mut needs,
        vocation,
        mut activity,
        mut job,
        mut target,
        mut motion,
        person,
        (mut chronicle, manner, home, rations, mut skills),
    ) in &mut workers
    {
        if *activity != Activity::Working {
            commands.entity(entity).remove::<Job>();
            continue;
        }

        // Every trade fills its OWN town's store and walks back to its own
        // square. `home` in this scope is the villager's house; `town` is
        // the settlement they belong to.
        let Ok(&crate::villager::MemberOf(town)) = members.get(entity) else {
            continue;
        };
        let Ok((ground, mut store)) = towns.get_mut(town) else {
            continue;
        };
        let centre = ground.centre;
        let woodpile = ground.woodpile;
        let workers_alive = alive_by_town.get(&town).copied().unwrap_or(0);

        // Shifts end: at day's end, or when hunger calls. The food trades
        // do not down tools for hunger — their meal is at the worksite —
        // and while the larder is thin they keep working after dark,
        // because the village eats all night whether anyone produces or
        // not.
        let feeds_the_village = matches!(
            vocation,
            Vocation::Fisher | Vocation::Gatherer | Vocation::Hunter | Vocation::Farmer
        );
        // Rations on the road: the provisioned eat from the satchel the
        // moment hunger bites, wherever they happen to be standing.
        let mut rations_left = 0.0;
        if let Some(mut rations) = rations {
            if needs.hunger > 0.5 && rations.0 > 0.0 {
                rations.0 -= 1.0;
                needs.hunger = (needs.hunger - 0.6).max(0.0);
                if rations.0 <= 0.0 {
                    commands.entity(entity).remove::<Rations>();
                }
            }
            rations_left = rations.0.max(0.0);
        }
        let larder_thin = store.food() < workers_alive as f32;
        let on_shift = is_work_hour(clock.time_of_day()) || (feeds_the_village && larder_thin);
        // The wise walk home before hunger walks them: what decides the
        // shift's end is hunger ON ARRIVAL, not hunger here - a worker
        // two hundred strides out must leave two hundred strides early.
        // Meals still in the satchel buy the road back.
        let walk_home = transform.translation.distance(centre) / 2.4;
        let projected = needs.hunger + walk_home / SECONDS_TO_STARVE - rations_left * 0.6;
        // The food trades work through hunger only while the larder is
        // THIN. That exemption exists so a village does not starve beside
        // an empty store with its fishers too hungry to fish; left on
        // while the store was full, it meant a gatherer a hundred and
        // twenty strides out never turned for home at all.
        let works_through_hunger = feeds_the_village && larder_thin;
        if !on_shift || (projected > DOWN_TOOLS_HUNGER && !works_through_hunger) {
            *activity = Activity::Idle;
            target.0 = None;
            commands.entity(entity).remove::<Job>();
            continue;
        }

        // A guard whose job is a broken plot is raising a watch post, and
        // builds it exactly as a carpenter would. Wolves still come first
        // - the hammer goes down the moment one shows.
        // A tradesman whose job is a broken plot is raising their own
        // works - the guard his watch post, the fisher her dock - and
        // builds it exactly as a carpenter would. The trade still comes
        // first where it has a claim: the guard's hammer goes down the
        // moment a wolf shows.
        let own_works =
            OWN_WORKS.iter().any(|(trade, ..)| trade == vocation) || *vocation == Vocation::Guard;
        let raising_a_post =
            own_works && job.focus.is_some_and(|site| build_sites.get(site).is_ok());
        // Which half of the build this plot is waiting on. One trade does
        // both now, so which loop a builder runs is a property of the
        // SITE and not of the person: stone until the footing is down,
        // timber after it.
        let footing_first = job
            .focus
            .and_then(|site| build_sites.get(site).ok())
            .is_some_and(|(cs, plan)| cs.stone_laid < cs.footing_stone(plan.kind));

        // Guards are their own trade: no pile, no yield — the work is the
        // walking, and the wolves are the deadline.
        if *vocation == Vocation::Guard {
            let at = transform.translation;
            let nearest_wolf = prey_query
                .iter()
                .filter(|(_, _, _, is_corpse, genome)| {
                    !is_corpse && genome.species == Species::Wolf
                })
                .map(|(t, ..)| t.translation)
                .filter(|w| w.distance(at) < 26.0)
                .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)));
            if let Some(wolf_at) = nearest_wolf {
                if wolf_at.distance(at) > 1.8 {
                    target.0 = Some(wolf_at);
                } else {
                    // Close enough to strike: a blow a beat, until the
                    // beast dies or breaks off.
                    target.0 = None;
                    motion.flail = 1.0;
                    job.progress += dt;
                    if job.progress >= 1.1 {
                        job.progress = 0.0;
                        for (wolf_t, mut vitality, wolf_motion, is_corpse, genome) in
                            prey_query.iter_mut()
                        {
                            if is_corpse
                                || genome.species != Species::Wolf
                                || wolf_t.translation.distance(at) > 2.2
                            {
                                continue;
                            }
                            vitality.harm += 0.7;
                            vitality.violent = true;
                            vitality.undoing = crate::creature::Undoing::Blow;
                            if let Some(mut wolf_motion) = wolf_motion {
                                wolf_motion.flail = 1.0;
                            }
                            if vitality.harm >= 1.0 {
                                info!("{} slew a wolf", person.name);
                                if let Some(chronicle) = chronicle.as_mut() {
                                    chronicle.record(
                                        clock.day(),
                                        "stood between the village and a wolf, and won".to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
                continue;
            }
            // No wolves in sight. If ground is broken for a watch post,
            // THAT is the work - a guard raises their own tower, and
            // falls through to the builder's loop below to do it. Only a
            // guard with nothing rising walks the round.
            if !raising_a_post {
                // A new leg of the patrol whenever the last one is done.
                if at.distance(job.site) > 2.0 {
                    target.0 = Some(job.site);
                } else {
                    target.0 = None;
                    job.progress += dt;
                    if job.progress >= 6.0 {
                        job.progress = 0.0;
                        let angle = rng.0.range(0.0, std::f32::consts::TAU);
                        let (sin, cos) = angle.sin_cos();
                        let reach = rng.0.range(14.0, 30.0);
                        let (x, z) = (centre.x + cos * reach, centre.z + sin * reach);
                        job.site = Vec3::new(x, 0.0, z);
                    }
                }
                continue;
            }
        }

        // Carpenters run a fetch-and-carry loop: to the pile for a log, log to
        // the site, hammer it in, and back for the next — every step walked.
        // A guard raising their own watch post runs the same loop.
        if (*vocation == Vocation::Builder || raising_a_post) && !footing_first {
            let Some(house) = job.focus else {
                *activity = Activity::Idle;
                commands.entity(entity).remove::<Job>();
                continue;
            };
            // Read into plain values and let the borrow go: the material
            // switch below needs the plan back mutably.
            let Some((stuff, site_kind, site_progress)) = build_sites
                .get(house)
                .ok()
                .map(|(site_now, site_plan)| (site_plan.stuff, site_plan.kind, site_now.progress))
            else {
                // Finished under someone else's hammer.
                *activity = Activity::Idle;
                commands.entity(entity).remove::<Job>();
                continue;
            };

            if carrying.get(entity).is_err() {
                // Empty-handed: fetch whatever this house is BUILT from -
                // or whatever the town HAS, if that has run out.
                //
                // What a plot is made of is chosen the morning its ground
                // is broken, out of what stood in the store and what grew
                // nearby. Held to for ever, that choice is a trap: a
                // longhouse begun in mud brick during a clay week stands
                // at nought of twelve for ever once the clay is gone,
                // however deep the timber pile gets. Brett's town had
                // fourteen hundred timber and a frame that had never
                // received one stick of it, and one soul slept rough
                // behind it for days.
                //
                // A builder out of brick finishes the house in wood, the
                // way a builder would.
                let held = |stuff: BuildStuff| match stuff {
                    BuildStuff::Timber => store.timber,
                    BuildStuff::Stone => store.stone,
                    BuildStuff::MudBrick => store.clay,
                };
                let stuff = if held(stuff) >= 1.0 {
                    stuff
                } else if let Some(instead) =
                    [BuildStuff::Timber, BuildStuff::Stone, BuildStuff::MudBrick]
                        .into_iter()
                        .filter(|other| held(*other) >= 4.0)
                        .max_by(|a, b| held(*a).total_cmp(&held(*b)))
                {
                    if let Ok((_, mut site_plan)) = build_sites.get_mut(house) {
                        site_plan.stuff = instead;
                    }
                    info!(
                        "{} ran out of {} for the {} and went on in {}",
                        person.name,
                        stuff.word(),
                        site_kind.name().to_lowercase(),
                        instead.word(),
                    );
                    instead
                } else {
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                    continue;
                };
                if transform.translation.distance(woodpile) > 2.6 {
                    target.0 = Some(woodpile);
                    job.patience -= dt;
                    if job.patience <= 0.0 {
                        *activity = Activity::Idle;
                        target.0 = None;
                        commands.entity(entity).remove::<Job>().insert(Shunned {
                            site: woodpile,
                            remaining: 90.0,
                        });
                    }
                    continue;
                }
                // An ARMFUL, not a stick. One piece a trip meant twelve
                // round trips to the pile for a longhouse, and the whole
                // build read as a man pacing between the stores and a
                // half-raised frame for no visible reason. He takes what
                // the site still wants, up to a load.
                let wanting = (site_kind.timber_cost() - site_progress).max(0.0);
                let load = match stuff {
                    BuildStuff::Timber => store.timber,
                    BuildStuff::Stone => store.stone,
                    BuildStuff::MudBrick => store.clay,
                }
                .min(wanting)
                .min(A_LOAD)
                .floor()
                .max(1.0);
                match stuff {
                    BuildStuff::Timber => store.timber -= load,
                    BuildStuff::Stone => store.stone -= load,
                    BuildStuff::MudBrick => store.clay -= load,
                }
                commands
                    .entity(entity)
                    .insert(CarryingWood { amount: load });
                shoulder_wood(&mut commands, &mut meshes, &mut materials, entity);
                job.patience = 20.0 + job.site.distance(transform.translation) * 0.8;
                continue;
            }

            // Loaded: to the site.
            if transform.translation.distance(job.site) > WORK_RANGE {
                target.0 = Some(job.site);
                job.patience -= dt;
                if job.patience <= 0.0 {
                    // Give the load back rather than stranding it in their arms.
                    store.timber += carrying.get(entity).map_or(1.0, |load| load.amount);
                    commands.entity(entity).remove::<CarryingWood>();
                    shed_wood(&mut commands, entity, &children, &loads);
                    *activity = Activity::Idle;
                    target.0 = None;
                    commands.entity(entity).remove::<Job>().insert(Shunned {
                        site: job.site,
                        remaining: 90.0,
                    });
                }
                continue;
            }

            // At the frame, hammering - and a carpenter with no roof of
            // their own drives nails like it is personal, because it is.
            target.0 = None;
            motion.flail = motion.flail.max(0.3);
            let stake = if plan_kind_is_home(&build_sites, house) && home.is_none() {
                1.4
            } else {
                1.0
            };
            job.progress += dt * stake;
            if job.progress < 3.5 {
                continue;
            }
            job.progress = 0.0;

            // One piece worked in per beat; the rest of the armful stays
            // on the shoulder, so a load is several beats of hammering
            // rather than several walks to the stores.
            let left = carrying.get(entity).map_or(0.0, |load| load.amount) - 1.0;
            if left > 0.0 {
                commands
                    .entity(entity)
                    .insert(CarryingWood { amount: left });
            } else {
                commands.entity(entity).remove::<CarryingWood>();
                shed_wood(&mut commands, entity, &children, &loads);
            }
            let Ok((mut construction, plan)) = build_sites.get_mut(house) else {
                continue;
            };
            construction.progress += 1.0;
            construction.last_hand = clock.elapsed;
            // Stages land at thirds of the build, whatever the kind's cost.
            let cost = plan.kind.timber_cost();
            let target_stage = stage_for(construction.progress, cost, steps_for(&plan));
            while construction.stage < target_stage {
                construction.stage += 1;
                raise_stage(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    house,
                    construction.stage,
                    &plan,
                );
            }
            if construction.progress >= cost {
                let kind = plan.kind;
                let steading = steadings.get(house).is_ok();
                let house_at = build_at.get(house).map(|t| *t).ok();
                let plan_size = plan.half_w.max(plan.half_d);
                let mut done = commands.entity(house);
                done.remove::<ConstructionSite>()
                    .insert((Building { kind }, Name::new(kind.name())));
                // A carried-in building brings its own shell, doors and
                // furnishings out of its own marks, below. Only a kind
                // the bench has not drawn gets the village's own walls.
                let drawn = baked::drawing_at(kind, plan.plan);
                match kind {
                    BuildingKind::House => {
                        done.insert(Hut);
                        if drawn.is_none() {
                            done.insert(Shell {
                                half_w: plan.half_w,
                                half_d: plan.half_d,
                                doors: vec![Doorway::on_x_wall(plan.half_w, 0.0)],
                            });
                        }
                    }
                    BuildingKind::Longhouse => {
                        done.insert(Longhouse);
                        if drawn.is_none() {
                            // One door per bay, mirroring the walls' own gaps.
                            let d = plan.half_d;
                            let bays = ((d * 2.0 / 3.4).round() as i32).clamp(3, 4);
                            let doors = (0..bays)
                                .map(|i| {
                                    Doorway::on_x_wall(
                                        plan.half_w,
                                        -d + (i as f32 + 0.5) * (d * 2.0 / bays as f32),
                                    )
                                })
                                .collect();
                            done.insert(Shell {
                                half_w: plan.half_w,
                                half_d: plan.half_d,
                                doors,
                            });
                        }
                    }
                    _ => {}
                }
                // The carried-in building furnishes itself: the beds it
                // holds, the doors it opens, the table its people gather
                // at, all read from the marks the bench wrote.
                if let Some(work) = drawn {
                    baked::furnish_baked(&mut commands, house, work, plan.mirrored);
                }
                // A holding comes with its ground broken: a plot turned beside
                // the house, waiting for whoever lives there to work it. Left
                // unclaimed on purpose — the farmer who moves in takes it, and
                // that is how the field ends up belonging to the family rather
                // than to the town's grid.
                if steading && let Some(house_at) = house_at {
                    let beside = house_at.translation
                        + house_at.rotation * Vec3::new(0.0, 0.0, plan_size + 4.2);
                    let beside =
                        Vec3::new(beside.x, terrain.height_at(beside.x, beside.z), beside.z);
                    if terrain.is_walkable(beside.x, beside.z) {
                        raise_field(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            &mut rng.0,
                            beside,
                            house_at.rotation,
                            0.05,
                            Entity::PLACEHOLDER,
                        );
                    }
                }
                let home = settlements
                    .get(town)
                    .map(|s| s.name.as_str())
                    .unwrap_or("the village");
                info!("{} raised {} in {}", person.name, kind.name(), home);
                if kind == BuildingKind::House {
                    notices.write(crate::ui::Notice::new(format!(
                        "{} raised a house in {}",
                        person.name, home
                    )));
                } else {
                    notices.write(crate::ui::Notice::fanfare(format!(
                        "{home} has raised {}",
                        kind.name().to_lowercase()
                    )));
                }
                if let Some(mut chronicle) = chronicle {
                    chronicle.record(
                        clock.day(),
                        format!("raised {} in {home}", kind.name().to_lowercase()),
                    );
                }
                *activity = Activity::Idle;
                commands.entity(entity).remove::<Job>();
            }
            continue;
        }

        // The same fetch-and-carry loop, in stone, for a footing that has
        // not been laid yet.
        if *vocation == Vocation::Builder
            && footing_first
            && let Some(site_entity) = job.focus
            && build_sites.get(site_entity).is_ok()
        {
            let Ok((mut construction, plan)) = build_sites.get_mut(site_entity) else {
                continue;
            };
            if construction.stone_laid >= construction.footing_stone(plan.kind) {
                *activity = Activity::Idle;
                commands.entity(entity).remove::<Job>();
                continue;
            }
            if carrying_stone.get(entity).is_err() {
                if store.stone < 1.0 && store.clay < 1.0 {
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                    continue;
                }
                if transform.translation.distance(woodpile) > 2.6 {
                    target.0 = Some(woodpile);
                    job.patience -= dt;
                    if job.patience <= 0.0 {
                        *activity = Activity::Idle;
                        target.0 = None;
                        commands.entity(entity).remove::<Job>();
                    }
                    continue;
                }
                // Stone first; clay brick where stone runs short, so a
                // flat-land village is not walled out of foundations.
                let clay = store.stone < 1.0;
                if clay {
                    store.clay -= 1.0;
                } else {
                    store.stone -= 1.0;
                }
                commands.entity(entity).insert(CarryingStone { clay });
                shoulder_stone(&mut commands, &mut meshes, &mut materials, entity);
                job.patience = 20.0 + job.site.distance(transform.translation) * 0.8;
                continue;
            }
            if transform.translation.distance(job.site) > WORK_RANGE {
                target.0 = Some(job.site);
                job.patience -= dt;
                if job.patience <= 0.0 {
                    match carrying_stone.get(entity) {
                        Ok(c) if c.clay => store.clay += 1.0,
                        _ => store.stone += 1.0,
                    }
                    commands.entity(entity).remove::<CarryingStone>();
                    shed_wood(&mut commands, entity, &children, &loads);
                    *activity = Activity::Idle;
                    target.0 = None;
                    commands.entity(entity).remove::<Job>();
                }
                continue;
            }
            // Lay the block.
            target.0 = None;
            motion.flail = motion.flail.max(0.3);
            job.progress += dt;
            if job.progress < 3.0 {
                continue;
            }
            job.progress = 0.0;
            commands.entity(entity).remove::<CarryingStone>();
            shed_wood(&mut commands, entity, &children, &loads);
            construction.stone_laid += 1.0;
            construction.last_hand = clock.elapsed;
            // The block lands where it was laid: courses appear around the
            // perimeter, corners first - but only for buildings raised by
            // the village's own hand. A carried-in drawing builds NOTHING
            // that is not in its own stages; its half-extents span the eaves
            // and the chimney, so blocks placed by them ringed the house a
            // stride out from its walls. Brett: "there should be nothing
            // built outside of the design from atelier."
            if baked::drawing_at(plan.kind, plan.plan).is_none() {
                let (w, d) = (plan.half_w, plan.half_d);
                let slots = [
                    (-w, -d),
                    (w, d),
                    (w, -d),
                    (-w, d),
                    (0.0, -d),
                    (0.0, d),
                    (-w, 0.0),
                    (w, 0.0),
                ];
                let (x, z) = slots[(construction.stone_laid as usize - 1) % slots.len()];
                let block = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
                let block_material = materials.add(StandardMaterial {
                    base_color: crate::palette::shade(&crate::palette::STONE, 0.45),
                    perceptual_roughness: 1.0,
                    ..default()
                });
                commands.spawn((
                    Mesh3d(block),
                    MeshMaterial3d(block_material),
                    Transform::from_xyz(x, 0.15, z).with_scale(Vec3::new(0.5, 0.3, 0.5)),
                    ChildOf(site_entity),
                ));
            }
            if construction.stone_laid >= construction.footing_stone(plan.kind) {
                // The foundation shows itself the moment the last block
                // lands - the mason's slab for the village's own kinds,
                // and NOTHING for a carried-in drawing, whose own footing
                // has stood since ground-breaking.
                reveal_foundation(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    site_entity,
                    &plan,
                );
                info!(
                    "the foundation of {} is laid",
                    plan.kind.name().to_lowercase()
                );
                notices.write(crate::ui::Notice::new(format!(
                    "The foundation of {} is laid",
                    plan.kind.name().to_lowercase()
                )));
                *activity = Activity::Idle;
                commands.entity(entity).remove::<Job>();
            }
            continue;
        }

        // Hunters follow the prey; everyone else's worksite stands still.
        if *vocation == Vocation::Hunter {
            match job.focus.and_then(|prey| prey_query.get(prey).ok()) {
                Some((prey_transform, _, _, _, _)) => {
                    job.site = prey_transform.translation;
                }
                None => {
                    if hunt_probe {
                        // The full anatomy of the miss, deferred so it costs
                        // no query parameters: which part of the prey query
                        // the quarry stopped satisfying, or whether it still
                        // exists at all.
                        let name = person.name.clone();
                        match job.focus {
                            Some(prey) => commands.queue(move |world: &mut World| {
                                match world.get_entity(prey) {
                                    Err(_) => {
                                        info!("hunt probe: {name}'s quarry no longer exists")
                                    }
                                    Ok(quarry) => info!(
                                        "hunt probe: {name}'s quarry stands but mismatches: creature={} villager={} vitality={} motion={} genome={} corpse={}",
                                        quarry.contains::<Creature>(),
                                        quarry.contains::<crate::villager::Villager>(),
                                        quarry.contains::<Vitality>(),
                                        quarry.contains::<CreatureMotion>(),
                                        quarry.contains::<CreatureGenome>(),
                                        quarry.contains::<Corpse>(),
                                    ),
                                }
                            }),
                            None => info!("hunt probe: {name} carries a hunt with no quarry"),
                        }
                    }
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                    continue;
                }
            }
        }
        // The healer follows their patient the same way.
        if *vocation == Vocation::Healer {
            match job.focus.and_then(|p| patients.get(p).ok()) {
                Some((patient_transform, vitality)) if vitality.harm > 0.02 => {
                    job.site = patient_transform.translation;
                }
                _ => {
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                    continue;
                }
            }
        }

        let distance = transform.translation.distance(job.site);
        let reach = if *vocation == Vocation::Hunter {
            SPEAR_REACH
        } else {
            WORK_RANGE
        };
        if distance > reach {
            target.0 = Some(job.site);
            job.patience -= dt;
            if job.patience <= 0.0 {
                if hunt_probe && *vocation == Vocation::Hunter {
                    info!(
                        "hunt probe: {} shunned the chase {:.1} strides short",
                        person.name, distance
                    );
                }
                *activity = Activity::Idle;
                target.0 = None;
                commands.entity(entity).remove::<Job>().insert(Shunned {
                    site: job.site,
                    remaining: 90.0,
                });
            }
            continue;
        }
        target.0 = None;

        // At the worksite. The arms move: work is something you can *see*.
        motion.flail = motion.flail.max(0.3);

        // Blacksmith's tools quicken every trade's hands - and the diligent
        // need less quickening than the slothful. Foul weather slows all of
        // them alike: nobody hammers well in a downpour.
        // Iron tools bite better - but only while there IS iron: the
        // blacksmith's speed is now something the village mines, smelts
        // and wears out, not a property of the building's silhouette.
        // A practiced hand is a quicker one - mastery shaves a fifth off
        // the cycle - and a better-rewarded one below: most trades yield
        // more per cycle as the craft grows.
        let skill = skills.as_ref().map_or(0.0, |s| s.of(*vocation));
        let cycle = if context.3.iter().any(|b| b.kind == BuildingKind::Blacksmith) && store.iron > 0.0 {
                WORK_SECONDS * 0.75
            } else {
                WORK_SECONDS
            } * manner.map_or(1.0, |m| m.work_pace())
                * weather.as_ref().map_or(1.0, |w| w.toil())
                * (1.0 - skill * 0.2)
                // A hunt is jabs, not a shift at a bench. Six seconds of
                // standing beside a grazing deer never comes - it drifts,
                // the clock resets, and the kill is forever three strides
                // away. A spear thrust is quick or it is nothing.
                * if *vocation == Vocation::Hunter { 0.35 } else { 1.0 };
        job.progress += dt;
        if job.progress < cycle {
            continue;
        }
        job.progress = 0.0;

        // The cycle itself is the teacher. A real day at the post is
        // twenty-odd cycles once walking and meals take their share, so
        // this rate makes 'getting the knack' a day and a half's work
        // and mastery the better part of a season. The chronicle marks
        // each threshold crossed.
        if let Some(skills) = skills.as_mut()
            && let Some(tier) = skills.practice(*vocation, 0.005)
        {
            info!("{} is now {} at their craft", person.name, tier);
            if let Some(chronicle) = chronicle.as_mut() {
                chronicle.record(
                    clock.day(),
                    format!("became {tier} as one who {}", vocation.describe()),
                );
            }
        }

        // The food trades eat where they work — the fisher at the water's
        // edge, the gatherer over the basket. Without this, the hunger
        // that no longer sends them home would starve them at their post.
        let ate_at_work = feeds_the_village && needs.hunger > 0.35;
        if ate_at_work {
            needs.hunger = (needs.hunger - 0.5).max(0.0);
        }

        match vocation {
            // Guards never reach this match: their patrol-and-fight block
            // runs earlier and always continues.
            Vocation::Guard => {}
            Vocation::Gatherer => {
                // A sacred stand yields its kind and is spent.
                if let Some(mut flora) = job.focus.and_then(|f| sacred_mut.get_mut(f).ok()) {
                    let taken = flora.amount.min(1.0);
                    flora.amount -= taken;
                    match flora.kind {
                        crate::scatter::SacredKind::Incense => store.incense += taken,
                        crate::scatter::SacredKind::Dye => store.dye += taken,
                    }
                    if flora.amount <= 0.1 {
                        if let Some(stand) = job.focus {
                            commands.entity(stand).despawn();
                        }
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                    }
                    continue;
                }
                let Some(mut source) = job.focus.and_then(|b| bushes.get_mut(b).ok()) else {
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                    continue;
                };
                let mut picked = source.amount.min(1.0 + skill * 0.5);
                source.amount -= picked;
                // What went into the gatherer does not also reach the sacks.
                if ate_at_work {
                    picked = (picked - 0.4).max(0.0);
                }
                store.larder.add(FoodKind::Berries, picked);
                if source.amount <= 0.1 {
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                }
            }

            // A dock casts past the shallows, and a smokehouse cures the
            // catch: one day's fishing can feed three days of village.
            Vocation::Fisher => {
                let mut catch = 1.0_f32 + skill * 0.8;
                if context.3.iter().any(|b| b.kind == BuildingKind::Dock) {
                    // Past the shallows the fish run twice as thick: a dock
                    // doubles the line, and is meant to carry a village.
                    catch += 1.0;
                }
                if context.3.iter().any(|b| b.kind == BuildingKind::Smokehouse) {
                    catch *= 2.0;
                }
                if ate_at_work {
                    catch = (catch - 0.4).max(0.2);
                }
                store.larder.add(FoodKind::Fish, catch);
            }

            Vocation::Miner | Vocation::Builder => match job.focus {
                // Bare high ground still yields loose stone.
                None => store.stone += LOOSE_STONE + skill * 1.5,
                // The mine: a drift into standing rock gives up stone by
                // the cartload, and never runs out the way a boulder does.
                Some(works)
                    if context
                        .3
                        .get(works)
                        .is_ok_and(|b| b.kind == BuildingKind::Mine) =>
                {
                    store.stone += 3.0 + skill * 1.5;
                }
                // A deposit gives up its kind, load by load, until the
                // ground is empty and the diggings are abandoned.
                Some(worked) if deposits_mut.get(worked).is_ok() => {
                    let Ok(mut deposit) = deposits_mut.get_mut(worked) else {
                        continue;
                    };
                    deposit.amount -= 1.0;
                    match deposit.kind {
                        crate::matter::DepositKind::Iron => store.ore += 1.0,
                        crate::matter::DepositKind::Clay => store.clay += 1.0,
                        // A quarry pays like a boulder does, because the work
                        // is the same work - a pick, a barrow, and a walk
                        // home. What it does not do is run out after four
                        // blows, which is the whole reason it exists.
                        crate::matter::DepositKind::Stone => {
                            store.stone += LOOSE_STONE + skill * 1.5
                        }
                    }
                    if deposit.amount <= 0.5 {
                        // A worked-out vein settles back into the earth.
                        commands
                            .entity(worked)
                            .remove::<crate::hand::PickRadius>()
                            .insert(crate::scatter::Sinking::default());
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                    }
                }
                // A boulder is chipped down blow by blow until it is gone.
                Some(rock) => {
                    let Ok((mut rock_transform, _)) = boulders_mut.get_mut(rock) else {
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                        continue;
                    };
                    store.stone += LOOSE_STONE + skill * 1.5;
                    // An outcrop gives up its stone slowly - the pick takes
                    // the same bite from a much bigger body - then chips
                    // away like any boulder once it is down to one.
                    let wear = if rock_transform.scale.x > 1.6 {
                        0.93
                    } else {
                        0.72
                    };
                    rock_transform.scale *= wear;
                    if rock_transform.scale.x < 0.4 {
                        // Chipped to nothing: the remnant sinks away, and
                        // the ground remembers, so no chunk rebuild quietly
                        // restocks the quarry.
                        commands
                            .entity(rock)
                            .remove::<crate::matter::Boulder>()
                            .remove::<crate::hand::PickRadius>()
                            .insert(crate::scatter::Sinking::default());
                        stripped.strip(job.site.x, job.site.z);
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                    }
                }
            },

            Vocation::Forester => match job.focus {
                // No tree under the axe: the woods here are spent, and no
                // free timber pretends otherwise. The want stands until
                // someone walks far enough to answer it.
                None => {
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                }
                // A standing tree comes down and a sapling starts over.
                Some(tree) => {
                    let Ok((felled, _, body, home)) = trees.get_mut(tree) else {
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                        continue;
                    };
                    if !felled.harvestable() {
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                        continue;
                    }
                    // The tree comes down and STAYS down: it topples away
                    // from the axe, and the ground remembers the felling, so
                    // a worked woods thins for good and clear-cut country
                    // reads from the air. Scarcity is the door to elsewhere.
                    let away = (job.site - transform.translation).with_y(0.0);
                    let away = away.normalize_or(Vec3::X);
                    // The standing tree is meshless bookkeeping inside its
                    // grove; the FALL is played by a one-off actor baked
                    // from the same body, while the grove rebakes without
                    // the felled tree the same frame.
                    let actor_mesh = body.bake(&mut meshes);
                    dirty_groves.0.push(home.0);
                    drop(felled);
                    commands.spawn((
                        Name::new("A felled tree"),
                        Mesh3d(actor_mesh),
                        MeshMaterial3d(terrain_assets.ground_material.clone()),
                        Transform::from_translation(job.site),
                        crate::scatter::Toppling {
                            axis: Vec3::Y.cross(away).normalize_or(Vec3::Z),
                            base_rot: Quat::IDENTITY,
                            base_y: job.site.y,
                            elapsed: 0.0,
                        },
                    ));
                    commands.entity(tree).despawn();
                    // Cut, not cleared: the wood comes back over four
                    // seasons unless this ground is inside the walls, which
                    // `the_walls_keep_their_ground` settles daily.
                    stripped.fell(job.site.x, job.site.z, clock.day());
                    // Shoulder the logs and turn for home. The timber only
                    // becomes the village's when it reaches the pile — and a
                    // sawmill wrings a third log from every tree.
                    let yield_ = if context.3.iter().any(|b| b.kind == BuildingKind::Sawmill) {
                        3.0
                    } else {
                        2.0
                    } + skill;
                    // And the near end of it. See `haul_wood`.
                    info!("a tree came down: {yield_:.0} timber shouldered");
                    commands
                        .entity(entity)
                        .insert(CarryingWood { amount: yield_ });
                    shoulder_wood(&mut commands, &mut meshes, &mut materials, entity);
                    *activity = Activity::Hauling;
                    target.0 = None;
                    commands.entity(entity).remove::<Job>();
                }
            },

            // Handled by the fetch-and-carry loop above.

            // A worked field surges; a ripe one is brought in.
            Vocation::Farmer => match job.focus {
                None => {
                    // Till a new plot where they stand — and level it first.
                    // The pad is real terrain: the ground itself is worked
                    // flat and rolls back into the hillside around it.
                    let level = terrain.height_at(job.site.x, job.site.z);
                    terrain.flatten(job.site.x, job.site.z, 3.4, 2.6, level);
                    // Rebuilt in place, not despawned for the streamer to
                    // refill: the streamer takes several budgeted frames to
                    // notice, and every tilled field opened a chunk-sized
                    // hole in the world while it did.
                    crate::terrain::rebuild_chunks_near(
                        &mut commands,
                        &mut meshes,
                        &terrain_assets,
                        &terrain,
                        &mut chunks,
                        job.site.x,
                        job.site.z,
                        7.0,
                    );
                    grass.invalidate_near(&mut commands, job.site.x, job.site.z, 7.0);
                    let at = Vec3::new(job.site.x, level, job.site.z);
                    // A field beside other fields shares their rows — the
                    // grid the siting chose only reads as one farm if the
                    // furrows agree. The first field of a farm faces home.
                    let rotation = fields_mut
                        .iter()
                        .map(|(_, t)| t)
                        .filter(|t| t.translation.distance(at) < 16.0)
                        .min_by(|a, b| {
                            a.translation
                                .distance(at)
                                .total_cmp(&b.translation.distance(at))
                        })
                        .map(|t| t.rotation)
                        .unwrap_or_else(|| {
                            Quat::from_rotation_y({
                                let toward = (centre - at).with_y(0.0);
                                let toward = toward.normalize_or_zero();
                                (-toward.z).atan2(toward.x)
                            })
                        });
                    raise_field(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &mut rng.0,
                        at,
                        rotation,
                        0.05,
                        entity,
                    );
                    info!("{} tilled a new field", person.name);
                    notices.write(crate::ui::Notice::new(format!(
                        "{} broke ground on a new field",
                        person.name
                    )));
                    if let Some(mut chronicle) = chronicle {
                        chronicle.record(clock.day(), "broke ground on a field");
                    }
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                }
                Some(field_entity) => {
                    let Ok((mut field, _)) = fields_mut.get_mut(field_entity) else {
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                        continue;
                    };
                    if field.growth >= 1.0 {
                        // The mill grinds the harvest into half again as much.
                        // A field is the steadiest table the village has:
                        // one brought-in harvest feeds a family for days.
                        store.larder.add(
                            FoodKind::Grain,
                            if context.3.iter().any(|b| b.kind == BuildingKind::Mill) {
                                12.0
                            } else {
                                8.0
                            },
                        );
                        field.growth = 0.08;
                        // The yield is an act of the world too: those who
                        // read favor into a heavy harvest thank the god.
                        witnessed.write(crate::witness::DivineEvent {
                            kind: crate::witness::DivineEventKind::Flourished,
                            position: job.site,
                            subject: None,
                            intensity: 0.5,
                        });
                        info!("{} brought in a harvest", person.name);
                        notices.write(crate::ui::Notice::new(format!(
                            "{} brought in a harvest",
                            person.name
                        )));
                        if let Some(mut chronicle) = chronicle {
                            chronicle.record(clock.day(), "brought in a harvest");
                        }
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                    } else {
                        field.growth = (field.growth + 0.10 * (1.0 + skill * 0.5)).min(1.0);
                    }
                }
            },

            // The kitchen warms while the cook stands at it.
            Vocation::Cook => {
                kitchen.until = clock.elapsed + 120.0;
            }

            // Hands on the hurt: harm ebbs, and both lives record it.
            Vocation::Healer => {
                let Some(patient) = job.focus else {
                    continue;
                };
                let Ok((_, mut vitality)) = patients.get_mut(patient) else {
                    continue;
                };
                // Herbs and salves: a stocked herbalist quickens the mending.
                let mending = if context.3.iter().any(|b| b.kind == BuildingKind::Herbalist) {
                    0.5
                } else {
                    0.3
                } * (1.0 + skill * 0.8);
                vitality.harm = (vitality.harm - mending).max(0.0);
                if vitality.harm <= 0.0 {
                    info!("{} nursed someone back to health", person.name);
                    if let Some(mut chronicle) = chronicle {
                        chronicle.record(clock.day(), "nursed a neighbour back to health");
                    }
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                }
            }

            // The priest stands the watch; the sermons system does the telling.
            Vocation::Priest => {}
            // Explorers never take ordinary jobs.
            Vocation::Explorer => {}

            Vocation::Hunter => {
                let Some(prey) = job.focus else {
                    continue;
                };
                let Ok((_, mut vitality, prey_motion, is_corpse, genome)) =
                    prey_query.get_mut(prey)
                else {
                    continue;
                };

                if is_corpse {
                    // The kill is made; bring it home as food.
                    store.larder.add(FoodKind::Meat, CARCASS_FOOD + skill * 1.5);
                    commands.entity(prey).despawn();
                    let quarry = match genome.species {
                        Species::Deer => "brought down a deer",
                        Species::Boar => "brought down a boar",
                        Species::Wolf => "slew a wolf",
                        Species::Human => "brought down quarry",
                    };
                    info!("{} {}", person.name, quarry);
                    if let Some(mut chronicle) = chronicle {
                        chronicle.record(clock.day(), quarry);
                    }
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                    continue;
                }

                // A strike. Two or three land a kill; the succumb system does
                // the dying, the same as for every other creature.
                vitality.harm += 0.55;
                vitality.violent = true;
                vitality.undoing = crate::creature::Undoing::Blow;
                if let Some(mut prey_motion) = prey_motion {
                    prey_motion.flail = 1.0;
                }
                motion.flail = motion.flail.max(0.4);
                if hunt_probe {
                    info!(
                        "hunt probe: {} struck, the quarry's harm stands at {:.2}",
                        person.name, vitality.harm
                    );
                }
            }
        }

        // Some shifts simply end after a haul, so workers drift home rather
        // than strip-mining one spot forever.
        if *activity == Activity::Working && rng.0.chance(0.25) {
            *activity = Activity::Idle;
            commands.entity(entity).remove::<Job>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The offering loop, end to end, in a bare world: a divinely-set-down
    /// tree beside the woodpile must become timber AND burst into sparks.
    ///
    /// Written because the live game did neither and two rounds of tracer
    /// logging could not say why: the conversion, the notice and the burst
    /// all hang off one query, so if that query is wrong the whole feature
    /// silently is. This bench asks the exact question the game asks.
    #[test]
    fn an_offering_at_the_pile_becomes_stores_and_sparks() {
        let mut app = bevy::app::App::new();
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::default())
            .add_message::<crate::ui::Notice>()
            .add_message::<crate::witness::DivineEvent>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Update, receive_offerings);

        let pile = Vec3::new(10.0, 40.0, -6.0);
        app.world_mut().spawn((
            crate::villager::SettlementGround {
                centre: pile,
                radius: 40.0,
                woodpile: pile,
                foodpile: pile,
            },
            Stockpile::default(),
        ));
        app.world_mut().spawn((
            Transform::from_translation(pile + Vec3::X * 2.0),
            crate::matter::Matter::felled_tree(1.0),
            crate::hand::DivinelyPlaced { remaining: 20.0 },
        ));

        app.update();

        let world = app.world_mut();
        let store = world
            .query::<&Stockpile>()
            .single(world)
            .expect("the town still stands");
        assert!(
            (store.timber - 3.0).abs() < 0.01,
            "a mature offered tree should pay 3 timber, the store holds {}",
            store.timber
        );
        let sparks = world.query::<&crate::matter::Spark>().iter(world).count();
        assert!(
            sparks >= 12,
            "the burst should fill the air with flecks; {sparks} spawned"
        );
    }

    #[test]
    fn a_tree_dropped_on_the_fire_burns_instead_of_banking() {
        // Brett: "the god should be able to add fuel to the fire the same
        // way they add trees to the stockpile." Dropped ON the flames it
        // burns; dropped beside them it still goes to the stores, because
        // the fire stands a few strides from the woodpile and must not
        // swallow every gift meant for the village.
        let build = || {
            let mut app = bevy::app::App::new();
            app.add_plugins(bevy::app::ScheduleRunnerPlugin::default())
                .add_message::<crate::ui::Notice>()
                .add_message::<crate::witness::DivineEvent>()
                .init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<StandardMaterial>>()
                .add_systems(Update, receive_offerings);
            let pile = Vec3::new(10.0, 40.0, -6.0);
            app.world_mut().spawn((
                crate::villager::SettlementGround {
                    centre: pile,
                    radius: 40.0,
                    woodpile: pile,
                    foodpile: pile,
                },
                Stockpile::default(),
            ));
            app.world_mut().spawn((
                Transform::from_translation(pile + Vec3::X * 6.0),
                crate::villager::home::Bonfire {
                    fuel: 0.0,
                    tender: None,
                },
            ));
            (app, pile)
        };

        // Onto the flames.
        let (mut app, pile) = build();
        app.world_mut().spawn((
            Transform::from_translation(pile + Vec3::X * 6.5),
            crate::matter::Matter::felled_tree(1.0),
            crate::hand::DivinelyPlaced { remaining: 20.0 },
        ));
        app.update();
        let world = app.world_mut();
        let fire = world
            .query::<&crate::villager::home::Bonfire>()
            .single(world)
            .expect("the fire still stands");
        assert!(
            fire.fuel > 0.0,
            "wood dropped on the fire should burn; the fire holds {}",
            fire.fuel,
        );
        let store = world.query::<&Stockpile>().single(world).unwrap();
        assert_eq!(store.timber, 0.0, "it burned, so the stores gain nothing");

        // Beside them: the stores take it, as they always did.
        let (mut app, pile) = build();
        app.world_mut().spawn((
            Transform::from_translation(pile + Vec3::X * 2.0),
            crate::matter::Matter::felled_tree(1.0),
            crate::hand::DivinelyPlaced { remaining: 20.0 },
        ));
        app.update();
        let world = app.world_mut();
        let store = world.query::<&Stockpile>().single(world).unwrap();
        assert!(store.timber > 0.0, "a gift at the pile still banks");
        let fire = world
            .query::<&crate::villager::home::Bonfire>()
            .single(world)
            .unwrap();
        assert_eq!(fire.fuel, 0.0, "the fire only takes what lands on it");
    }

    /// What the god's own deliveries pay, held against what labour pays.
    ///
    /// A dropped tree must be worth what felling it is worth - the god saves
    /// the walk, not the arithmetic - or provisioning becomes either pointless
    /// or the only sane way to play. Stone scales with the rock: a loaf-sized
    /// stone is a stone, an outcrop is a working day.
    #[test]
    fn an_offering_pays_what_the_labour_would_have() {
        // A mature tree: the feller's own yield without a sawmill is 2.0
        // plus skill; the offering pays the skilless rate.
        let tree = crate::matter::Matter::felled_tree(1.0);
        let (timber, stone) = offering_worth(&tree);
        assert!((timber - 3.0).abs() < 0.01, "a mature tree pays {timber}");
        assert_eq!(stone, 0.0);

        // A sapling pays less than a grown tree.
        let sapling = crate::matter::Matter::felled_tree(0.1);
        assert!(offering_worth(&sapling).0 < timber);

        // A common boulder is a couple of stone; a great outcrop boulder is
        // capped so one throw cannot fill the yard.
        let stone_small = crate::matter::Matter::boulder(120.0, 0.9);
        let (t, s) = offering_worth(&stone_small);
        assert_eq!(t, 0.0);
        assert!((1.5..=3.0).contains(&s), "a common boulder pays {s}");
        let outcrop = crate::matter::Matter::boulder(500.0, 3.0);
        assert_eq!(offering_worth(&outcrop).1, 8.0, "the cap holds");

        // A bush is a bush.
        assert_eq!(offering_worth(&crate::matter::Matter::bush()), (0.0, 0.0));
    }
    use crate::rng::Rng;

    #[test]
    fn every_vocation_gets_taken_up() {
        let mut rng = Rng::new(11);
        let mut seen = std::collections::HashSet::new();
        for i in 0..300 {
            let boldness = (i as f32 / 300.0).clamp(0.05, 0.95);
            seen.insert(format!("{:?}", roll_vocation(boldness, &mut rng)));
        }
        // Eight, since mason and carpenter became one builder. Cook,
        // healer and priest are absent on purpose: those are not rolled
        // into, they are called into.
        assert_eq!(seen.len(), 8, "some vocation never occurs: {seen:?}");
    }

    #[test]
    fn houses_scale_with_population_and_stages_with_timber() {
        // Ground broken, walls at a third, roof from two-thirds on — for any
        // building, whatever its cost.
        assert_eq!(stage_for(0.0, HOUSE_TIMBER, 3), 0);
        assert_eq!(stage_for(2.0, HOUSE_TIMBER, 3), 1);
        assert_eq!(stage_for(4.0, HOUSE_TIMBER, 3), 2);
        assert_eq!(stage_for(5.0, 14.0, 3), 1);
        // A framed house takes four: footing, posts, walls, roof, at
        // quarters of the way — and never runs past its last step.
        assert_eq!(stage_for(0.0, 8.0, 4), 0);
        assert_eq!(stage_for(2.0, 8.0, 4), 1);
        assert_eq!(stage_for(4.0, 8.0, 4), 2);
        assert_eq!(stage_for(6.0, 8.0, 4), 3);
        assert_eq!(stage_for(99.0, 8.0, 4), 3);
    }

    #[test]
    fn civic_works_answer_needs_not_a_ladder() {
        let none = |_: BuildingKind| false;

        // A tiny hamlet builds nothing civic.
        let hamlet = CivicNeeds {
            population: 4,
            stone: 99.0,
            ..Default::default()
        };
        assert_eq!(next_civic(&hamlet, none), None);

        // Goods heaped outdoors call for the storehouse over everything.
        let heaped = CivicNeeds {
            population: 12,
            stone: 99.0,
            timber_stored: 20.0,
            stone_stored: 15.0,
            avg_spirits: 0.7,
            ..Default::default()
        };
        assert_eq!(next_civic(&heaped, none), Some(BuildingKind::Storehouse));

        // A miserable village builds itself the tavern.
        let glum = CivicNeeds {
            population: 12,
            stone: 99.0,
            avg_spirits: 0.3,
            ..Default::default()
        };
        assert_eq!(
            next_civic(&glum, |k| k == BuildingKind::Well),
            Some(BuildingKind::Tavern)
        );

        // A village that has been bitten raises a tower. Note what this
        // is NOT: a count of wolves. Three souls carrying the memory of
        // the teeth is what puts a watch on the treeline.
        let hunted = CivicNeeds {
            population: 12,
            stone: 99.0,
            avg_spirits: 0.7,
            peril: 3.0,
            ..Default::default()
        };
        assert_eq!(
            next_civic(&hunted, |k| k == BuildingKind::Well),
            Some(BuildingKind::Watchtower)
        );

        // No stone, no stone buildings.
        let broke = CivicNeeds {
            population: 12,
            stone: 0.0,
            avg_spirits: 0.2,
            ..Default::default()
        };
        assert_eq!(next_civic(&broke, none), None, "no stone, no works");
    }

    #[test]
    fn a_thin_larder_by_the_sea_builds_the_dock() {
        let none = |_: BuildingKind| false;
        // Hungry, on the water, fishers already at work: planks go up.
        // (Docks cost no stone, so even a stone-poor village can build one.)
        let coastal = CivicNeeds {
            population: 6,
            stone: 0.0,
            food_stored: 5.0,
            fishers: 1,
            avg_spirits: 0.7,
            shore_near: true,
            ..Default::default()
        };
        assert_eq!(next_civic(&coastal, none), Some(BuildingKind::Dock));

        // The same hunger inland stays hungry: no shore, no dock.
        let inland = CivicNeeds {
            shore_near: false,
            ..coastal
        };
        assert_ne!(next_civic(&inland, none), Some(BuildingKind::Dock));
    }

    /// A town of twenty-two with its works already up builds the hall.
    ///
    /// Brett's own run, to the number: "my current run is at 22 people and
    /// no town hall has been built." Ten civic buildings standing, fourteen
    /// stone in the pile, and the ladder had never once been asked - one
    /// soul was sleeping rough behind a longhouse that had not been touched
    /// in days, and the planner returned at that branch every time it ran.
    /// The ladder itself was always willing, which is what this pins.
    #[test]
    fn a_town_of_twenty_two_wants_its_hall() {
        let built = [
            BuildingKind::Well,
            BuildingKind::Dock,
            BuildingKind::Storehouse,
            BuildingKind::Sawmill,
            BuildingKind::Granary,
            BuildingKind::Blacksmith,
            BuildingKind::Mill,
            BuildingKind::Bakery,
            BuildingKind::Herbalist,
            BuildingKind::Shrine,
        ];
        let grown = CivicNeeds {
            population: 22,
            stone: 14.0,
            stone_stored: 14.0,
            timber_stored: 1469.0,
            food_stored: 300.0,
            avg_spirits: 0.8,
            ..Default::default()
        };
        assert_eq!(
            next_civic(&grown, |k| built.contains(&k)),
            Some(BuildingKind::TownHall),
        );

        // Eighteen is the stated price of a hall, so eighteen buys one.
        let just_enough = CivicNeeds {
            population: 18,
            ..grown
        };
        assert_eq!(
            next_civic(&just_enough, |k| built.contains(&k)),
            Some(BuildingKind::TownHall),
        );

        // Seventeen does not.
        let hamlet = CivicNeeds {
            population: 17,
            ..grown
        };
        assert_eq!(next_civic(&hamlet, |k| built.contains(&k)), None);
    }

    #[test]
    fn the_bold_hunt_more_than_the_timid() {
        let mut rng = Rng::new(21);
        let hunters_among = |boldness: f32, rng: &mut Rng| {
            (0..400)
                .filter(|_| roll_vocation(boldness, rng) == Vocation::Hunter)
                .count()
        };
        let timid = hunters_among(0.1, &mut rng);
        let bold = hunters_among(0.9, &mut rng);
        assert!(bold > timid * 2, "bold {bold} vs timid {timid}");
    }

    #[test]
    fn the_working_day_matches_the_sun() {
        assert!(is_work_hour(0.1), "morning is for working");
        assert!(is_work_hour(0.5), "afternoon is for working");
        assert!(!is_work_hour(0.7), "evening is not");
        assert!(!is_work_hour(0.9), "night is not");
    }

    #[test]
    fn every_vocation_has_words() {
        for vocation in [
            Vocation::Gatherer,
            Vocation::Fisher,
            Vocation::Hunter,
            Vocation::Miner,
            Vocation::Forester,
        ] {
            assert!(!vocation.describe().is_empty());
            assert!(vocation.taking_up().starts_with("took up"));
        }
    }

    /// A kill must still be visible to the hunter who made it.
    ///
    /// Death strips a body of its `CreatureMotion`, so `do_work`'s prey
    /// query may never REQUIRE that component: it did, every carcass
    /// fell out of the query the instant it died, the harvest arm could
    /// not run, and a village in the deep woods starved from ten souls
    /// to six with venison lying in the grass. The query shape is the
    /// contract; this test guards it without booting the whole game.
    #[test]
    fn a_carcass_still_answers_the_hunters_query() {
        let mut world = World::new();
        let quarry = world
            .spawn((
                Creature,
                Transform::default(),
                Vitality::default(),
                CreatureGenome::random(Species::Deer, &mut Rng::new(4)),
                Corpse,
            ))
            .id();
        let mut query = world.query_filtered::<(
            &Transform,
            &mut Vitality,
            Option<&mut CreatureMotion>,
            Has<Corpse>,
            &CreatureGenome,
        ), (With<Creature>, Without<Villager>)>();
        let (_, _, motion, is_corpse, genome) = query
            .get_mut(&mut world, quarry)
            .expect("a carcass with no motion must still answer the hunter's query");
        assert!(is_corpse, "the harvest arm keys off this flag");
        assert!(motion.is_none(), "death took the motion, as it should");
        assert_eq!(genome.species, Species::Deer);
    }
}
