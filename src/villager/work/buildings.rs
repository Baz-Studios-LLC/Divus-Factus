//! Buildings: kinds, blueprints, the civic chooser, the visible stages
//! a construction rises through, and the planner that breaks ground.

use bevy::prelude::*;

use super::*;
use crate::rng::Rng;
use crate::terrain::{Terrain, WATER_LEVEL};
/// Timber one ordinary house costs, delivered one unit per work cycle.
pub const HOUSE_TIMBER: f32 = 6.0;

/// Timber a longhouse costs. Twice the beds, twice the timber — the price
/// per head is the same, but it is a single long commitment rather than
/// two small ones, which is why a village only starts one when it has hands
/// enough to finish it.
pub const LONGHOUSE_TIMBER: f32 = 12.0;

/// What a building is for. Shape, cost and effect all follow from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BuildingKind {
    House,
    /// One long roof over the unwed: where the grown children of full
    /// houses sleep until they marry out of it. A house is a family; a
    /// longhouse is everyone else.
    Longhouse,
    /// Mills felled timber: every tree yields more once it stands.
    Sawmill,
    /// Tools for everyone: work cycles run faster.
    Blacksmith,
    /// Evenings and rumours: spirits mend here, and stories change hands.
    Tavern,
    /// The village grown into a town: room for more people.
    TownHall,
    /// A place set apart for the god; where the priest keeps the litany.
    Shrine,
    /// Timber and stone under a roof: the piles brought indoors.
    Storehouse,
    /// The harvest raised on stilts, out of the damp and the rats.
    Granary,
    /// Sweet water and the midday gossip that gathers around it.
    Well,
    /// The catch cured over smoke: a fisher's day feeds twice as many.
    Smokehouse,
    /// Grain ground fine: every harvest goes further.
    Mill,
    /// Bread from the store's grain: rations stretch.
    Bakery,
    /// Warm cloth for everyone - hardest felt by those without walls.
    Weaver,
    /// Salves and simples: the healer's hands work faster.
    Herbalist,
    /// A high post and a horn: wolves learn to keep their distance.
    Watchtower,
    /// Planks run out over the water: fishers cast past the shallows,
    /// and the catch comes home heavier.
    Dock,
    /// A timbered adit driven into rising ground: miners bring out stone
    /// by the cartload instead of chipping boulders where they lie.
    Mine,
}

impl BuildingKind {
    /// How many the finished roof sleeps. Zero for the kinds nobody
    /// beds down in, which is most of them.
    ///
    /// A carried-in building sleeps the beds its maker DREW. The
    /// constants are the village's own hand, and stand only for a kind
    /// with no drawing behind it — the bench longhouse holds ten, and a
    /// village that went on believing eight would break ground for a
    /// second hall it did not need.
    pub fn sleeps(self) -> usize {
        use crate::villager::home::{HOUSE_CAPACITY, LONGHOUSE_CAPACITY};
        if let Some(drawn) = super::baked::beds(self) {
            return drawn;
        }
        match self {
            BuildingKind::House => HOUSE_CAPACITY,
            BuildingKind::Longhouse => LONGHOUSE_CAPACITY,
            _ => 0,
        }
    }

    pub fn timber_cost(self) -> f32 {
        match self {
            BuildingKind::House => HOUSE_TIMBER,
            BuildingKind::Longhouse => LONGHOUSE_TIMBER,
            BuildingKind::Sawmill => 8.0,
            BuildingKind::Blacksmith => 8.0,
            BuildingKind::Tavern => 9.0,
            BuildingKind::TownHall => 14.0,
            BuildingKind::Shrine => 7.0,
            BuildingKind::Storehouse => 6.0,
            BuildingKind::Granary => 7.0,
            BuildingKind::Well => 2.0,
            BuildingKind::Smokehouse => 6.0,
            BuildingKind::Mill => 9.0,
            BuildingKind::Bakery => 7.0,
            BuildingKind::Weaver => 6.0,
            BuildingKind::Herbalist => 6.0,
            BuildingKind::Watchtower => 10.0,
            BuildingKind::Dock => 5.0,
            BuildingKind::Mine => 6.0,
        }
    }

    /// Stone laid into the foundation when ground is broken.
    pub fn stone_cost(self) -> f32 {
        match self {
            BuildingKind::House => 2.0,
            BuildingKind::Longhouse => 4.0,
            BuildingKind::Sawmill => 2.0,
            BuildingKind::Blacksmith => 6.0,
            BuildingKind::Tavern => 2.0,
            BuildingKind::TownHall => 8.0,
            BuildingKind::Shrine => 4.0,
            BuildingKind::Storehouse => 2.0,
            BuildingKind::Granary => 3.0,
            BuildingKind::Well => 4.0,
            BuildingKind::Smokehouse => 2.0,
            BuildingKind::Mill => 4.0,
            BuildingKind::Bakery => 4.0,
            BuildingKind::Weaver => 2.0,
            BuildingKind::Herbalist => 2.0,
            BuildingKind::Watchtower => 6.0,
            // Pilings, not foundations: a dock is all carpentry.
            BuildingKind::Dock => 0.0,
            // The mountain provides its own stone; the timber shores it up.
            BuildingKind::Mine => 0.0,
        }
    }

    /// Every kind there is, so a rule about all of them can be written once.
    pub fn every() -> &'static [BuildingKind] {
        use BuildingKind::*;
        &[
            House, Longhouse, Sawmill, Blacksmith, Tavern, TownHall, Storehouse, Granary, Well,
            Smokehouse, Mill, Bakery, Weaver, Herbalist, Watchtower, Shrine, Dock, Mine,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            BuildingKind::House => "A house",
            BuildingKind::Longhouse => "The longhouse",
            BuildingKind::Sawmill => "The sawmill",
            BuildingKind::Blacksmith => "The blacksmith",
            BuildingKind::Tavern => "The tavern",
            BuildingKind::TownHall => "The town hall",
            BuildingKind::Storehouse => "The storehouse",
            BuildingKind::Granary => "The granary",
            BuildingKind::Well => "The well",
            BuildingKind::Smokehouse => "The smokehouse",
            BuildingKind::Mill => "The mill",
            BuildingKind::Bakery => "The bakery",
            BuildingKind::Weaver => "The weaver's cottage",
            BuildingKind::Herbalist => "The herbalist's hut",
            BuildingKind::Watchtower => "The watchtower",
            BuildingKind::Shrine => "The shrine",
            BuildingKind::Dock => "The dock",
            BuildingKind::Mine => "The mine",
        }
    }
}

/// Which civic building the settlement wants next, given what it has.
///
/// A pure priority ladder, so the village's growth arc is legible: industry,
/// then tools, then an evening hearth, then civic pride.
/// Everything the civic chooser weighs, gathered from the living village.
#[derive(Default)]
pub struct CivicNeeds {
    pub population: usize,
    pub stone: f32,
    pub timber_stored: f32,
    pub stone_stored: f32,
    pub food_stored: f32,
    pub avg_spirits: f32,
    pub homeless: usize,
    pub hurt: usize,
    pub believers: usize,
    pub fishers: usize,
    pub farmers: usize,
    pub foresters: usize,
    pub fields: usize,
    /// How badly the village fears the woods, summed over the people who
    /// carry a fresh memory of the teeth. NOT a count of live wolves: a
    /// tower is raised out of what the village has been through and told
    /// each other, not out of what a god's-eye census can see prowling.
    pub peril: f32,
    /// Couples who have courted long enough and have nowhere to be wed.
    /// Vows are made in the god's house, so a village with pairs waiting
    /// on one wants it built - which is the whole reason a shrine gets
    /// raised in a hamlet that has not yet grown to twelve souls.
    pub betrothed: usize,
    pub pending_builds: usize,
    /// Whether walkable shore lies within working reach — no water, no dock.
    pub shore_near: bool,
    /// Hands at the stone trade, arguing for a proper works.
    pub miners: usize,
    /// Whether rising rocky ground stands within working reach — no
    /// mountainside, no mine.
    pub rock_near: bool,
}

/// A trade and the works it will raise for ITSELF when the village has
/// none — without waiting on the civic ladder, which will not reach any
/// of these until everybody is housed and the town has grown to ten.
///
/// `at_once` says whether the works outrank the trade's ordinary work. A
/// fisher on a bare shore and a miner chipping loose boulders should both
/// down tools and build: the works are what makes their trade worth
/// having. A forester must NOT — the sawmill is built out of the timber
/// they have not cut yet — and a healer's patients come first. Those two
/// raise their works only when their own trade has nothing for them.
pub const OWN_WORKS: &[(Vocation, BuildingKind, bool)] = &[
    (Vocation::Fisher, BuildingKind::Dock, true),
    (Vocation::Priest, BuildingKind::Shrine, true),
    (Vocation::Miner, BuildingKind::Mine, true),
    (Vocation::Forester, BuildingKind::Sawmill, false),
    (Vocation::Healer, BuildingKind::Herbalist, false),
];

/// Chooses the next civic building by NEED, not by a fixed ladder: each
/// candidate scores against what the village actually lacks, and the
/// loudest need above a threshold gets ground broken. Soft population
/// minimums keep hamlets from dreaming of town halls.
pub fn next_civic(needs: &CivicNeeds, has: impl Fn(BuildingKind) -> bool) -> Option<BuildingKind> {
    use BuildingKind::*;
    let candidates = [
        Well, Dock, Mine, Storehouse, Sawmill, Blacksmith, Smokehouse, Granary, Tavern, Mill,
        Bakery, Weaver, Herbalist, Shrine, Watchtower, TownHall,
    ];
    let min_pop = |kind: BuildingKind| match kind {
        Well | Dock => 5,
        Mine => 7,
        Storehouse => 7,
        Sawmill => 8,
        Smokehouse => 9,
        Blacksmith | Granary | Tavern | Weaver | Herbalist | Watchtower => 10,
        Mill | Shrine => 12,
        Bakery => 14,
        TownHall => 18,
        // Shelter is not civic ambition: both roofs are planned by need in
        // `plan_houses`, never by this ladder.
        House | Longhouse => 0,
    };
    let mut best: Option<(f32, BuildingKind)> = None;
    for kind in candidates {
        if has(kind) || needs.population < min_pop(kind) || needs.stone < kind.stone_cost() {
            continue;
        }
        let score = match kind {
            // Water and the midday square: always wanted once there are
            // enough hands to dig it.
            Well => 0.6,
            // The water feeds without emptying the land: fishers argue for
            // planks, and a thin larder argues louder. No shore, no dock.
            Dock => {
                if needs.shore_near {
                    needs.fishers as f32 * 0.3 + (35.0 - needs.food_stored).max(0.0) / 45.0 + 0.2
                } else {
                    0.0
                }
            }
            // Foundations wait on stone more than anything else waits on
            // anything: half-raised walls argue for a real works in the
            // mountainside, and idle miners second them. No rock, no mine.
            Mine => {
                if needs.rock_near {
                    needs.miners as f32 * 0.3
                        + needs.pending_builds as f32 * 0.2
                        + (10.0 - needs.stone_stored).max(0.0) / 25.0
                } else {
                    0.0
                }
            }
            // Goods heaped in the open argue for a roof over them.
            Storehouse => (needs.timber_stored + needs.stone_stored) / 25.0,
            Granary => needs.food_stored / 70.0,
            // Working trades argue for the works that serve them.
            Sawmill => needs.foresters as f32 * 0.25 + needs.pending_builds as f32 * 0.15,
            Smokehouse => needs.fishers as f32 * 0.3,
            Mill => needs.fields as f32 * 0.22 + needs.farmers as f32 * 0.1,
            Bakery => needs.food_stored / 120.0 + if has(Granary) { 0.25 } else { 0.0 },
            Blacksmith => needs.population as f32 / 20.0,
            // Misery builds the tavern; cold builds the weaver.
            Tavern => (0.75 - needs.avg_spirits).max(0.0) * 2.2,
            Weaver => needs.homeless as f32 * 0.12,
            Herbalist => needs.hurt as f32 * 0.35,
            // Faith raises its own roof; fear raises a tower. The fear
            // is the village's own - three souls carrying a mauling
            // between them is what puts a watch on the treeline.
            Shrine => needs.believers as f32 * 0.12 + needs.betrothed as f32 * 0.5,
            Watchtower => needs.peril * 0.4,
            TownHall => (needs.population as f32 - 16.0) / 8.0,
            House | Longhouse => 0.0,
        };
        if score > best.map_or(0.0, |(b, _)| b) {
            best = Some((score, kind));
        }
    }
    best.filter(|(score, _)| *score >= 0.45).map(|(_, k)| k)
}

/// What the village's two roofs are carrying, and what is already rising.
#[derive(Default, Debug, Clone, Copy)]
pub struct RoofNeeds {
    /// The wed and the children: everyone with a claim on a family room.
    pub family_souls: usize,
    /// Everyone else — in practice the grown and unmarried.
    pub single_souls: usize,
    pub houses: usize,
    pub longhouses: usize,
    pub population: usize,
    /// How many family roofs are already going up. A count, not a flag:
    /// one-at-a-time was a queue, and a founding village spent days in it.
    pub houses_rising: usize,
    pub longhouse_rising: bool,
    /// Grown souls with no roof at all tonight.
    pub roofless: usize,
}

/// Which roof to break ground on next, if either.
///
/// The village builds AHEAD of need on both roofs at once: it wants a whole
/// spare house AND a whole spare longhouse standing empty at all times, so
/// growth never waits on a construction site. A town that builds exactly
/// what it needs is always one wedding behind — and a wedding needs a house
/// free the day it happens, not a fortnight later.
///
/// The fire circle is deliberately not counted as shelter here. It is the
/// shortfall made visible, not capacity to plan around.
pub fn next_roof(needs: &RoofNeeds) -> Option<BuildingKind> {
    use crate::villager::home::{HOUSE_CAPACITY, LONGHOUSE_CAPACITY};

    let family_slack = (needs.houses * HOUSE_CAPACITY) as i32 - needs.family_souls as i32;
    let single_slack = (needs.longhouses * LONGHOUSE_CAPACITY) as i32 - needs.single_souls as i32;

    // The hall goes up first, and ALONE. It sleeps ten of anybody, which
    // on the founding morning is the whole village, and it is the only
    // roof that will take a stranger - the shortest road there is from
    // people in the dirt to people under cover. Nothing else is planned
    // until it stands: a family house begun beside it only splits the
    // hands that could be closing it in, and both then take twice as
    // long. Houses are the reward that follows, not the way out of the
    // rain.
    if needs.longhouses == 0 && needs.roofless > 0 {
        return (!needs.longhouse_rising).then_some(BuildingKind::Longhouse);
    }

    // How many family roofs may rise at once. This was ONE, ever, and it
    // made a queue nothing could shorten: six couples wed on the founding
    // morning meant six houses in series, while the timber pile grew past
    // three hundred and carpenters wandered off for want of a site. The
    // cap is the shortfall itself now - a roof for every four kin who
    // want one - held to three, so a village never carries more open
    // shells than its hands can close in.
    let short = (needs.family_souls as i32 - (needs.houses * HOUSE_CAPACITY) as i32).max(0);
    let may_rise = (short as usize).div_ceil(HOUSE_CAPACITY).clamp(1, 3);
    let want_house = family_slack < HOUSE_CAPACITY as i32 && needs.houses_rising < may_rise;
    // A longhouse is a big single commitment: eight beds' worth of timber in
    // one build. A hamlet puts what it has into family roofs first and
    // sleeps its handful of unwed by the fire until there are enough of them
    // to be worth a long roof.
    // The population gate keeps a hamlet from sinking a hall's worth of
    // timber into beds for two - but it never applies while anyone is
    // actually roofless, which the clause above has already answered.
    let want_longhouse = single_slack < LONGHOUSE_CAPACITY as i32
        && needs.population >= 8
        && !needs.longhouse_rising;

    // Whichever roof is further behind goes first, so neither queue starves
    // the other in a village that needs both.
    match (want_house, want_longhouse) {
        (true, true) if family_slack <= single_slack => Some(BuildingKind::House),
        (true, true) => Some(BuildingKind::Longhouse),
        (true, false) => Some(BuildingKind::House),
        (false, true) => Some(BuildingKind::Longhouse),
        (false, false) => None,
    }
}

/// What a building's body is raised FROM. The land decides: timber where
/// woods stand near, mud brick where clay is the gift, bare masonry where
/// stone is all there is - a village never freezes for want of the one
/// material its ground never offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BuildStuff {
    #[default]
    Timber,
    Stone,
    MudBrick,
}

impl BuildStuff {
    pub fn word(self) -> &'static str {
        match self {
            BuildStuff::Timber => "timber",
            BuildStuff::Stone => "stone",
            BuildStuff::MudBrick => "mud brick",
        }
    }
}

/// One building's rolled shape and colours. No two houses need look alike:
/// footprint, height, roof style and paint all vary within the kind.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Blueprint {
    pub kind: BuildingKind,
    pub half_w: f32,
    pub half_d: f32,
    pub wall_h: f32,
    pub walls: Color,
    pub roof: Color,
    /// A single tilted slab instead of a gable — the working-shed look.
    pub shed_roof: bool,
    /// The material the walls rise from; the planner picks it from what
    /// the land around this settlement actually offers.
    #[serde(default)]
    pub stuff: BuildStuff,
    /// Which carried-in house this one follows, when there are several.
    /// Rolled once with the rest of the blueprint and kept, so the house
    /// that finishes is the house whose ground was broken.
    #[serde(default)]
    pub plan: usize,
    /// Whether this one stands as its drawing's own reflection. Rolled with the
    /// plan and kept for the same reason: the building that finishes has to be
    /// the building whose doorway the plot was cut for.
    #[serde(default)]
    pub mirrored: bool,
}

impl Blueprint {
    pub fn roll(kind: BuildingKind, rng: &mut Rng) -> Blueprint {
        let mut plan = Self::rolled(kind, rng);
        // A coin for which way round it stands, thrown for every kind so the
        // dice fall the same way whatever is being built - and thrown last, so
        // that adding it did not move any of the rolls that came before.
        plan.mirrored = rng.chance(0.5);
        plan
    }

    fn rolled(kind: BuildingKind, rng: &mut Rng) -> Blueprint {
        use crate::palette as pal;
        // Which of the maker's houses this one will be. Rolled for every
        // kind so the dice fall the same way whatever is being built.
        let plan = rng.range_i(0, super::baked::drawings(kind).len().max(1) as i32) as usize;
        let carried = super::baked::drawing_at(kind, plan);
        match kind {
            BuildingKind::House => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                // A carried-in house brings its own footprint, so the
                // plots are cut to fit it; otherwise the village's own
                // sizing stands - four beds, a floor to cross, a lane.
                half_w: carried
                    .map(|work| work.half_w)
                    .unwrap_or_else(|| rng.range(2.6, 3.2)),
                half_d: carried
                    .map(|work| work.half_d)
                    .unwrap_or_else(|| rng.range(2.7, 3.4)),
                wall_h: rng.range(2.2, 2.7),
                // Timber homes mostly; some whitewashed, a few painted in a
                // cloth colour — a street, not a barracks.
                walls: if rng.chance(0.55) {
                    pal::shade(&pal::WOOD, rng.range(0.55, 0.85))
                } else if rng.chance(0.6) {
                    pal::shade(&pal::BONE, rng.range(0.8, 0.95))
                } else {
                    pal::shade(&pal::ALL_RAMPS[*rng.pick(pal::CLOTH_RAMPS)], 0.75)
                },
                roof: if rng.chance(0.5) {
                    pal::shade(&pal::EARTH, rng.range(0.3, 0.5))
                } else {
                    pal::shade(&pal::SAND, rng.range(0.45, 0.6))
                },
                shed_roof: rng.chance(0.12),
                stuff: BuildStuff::Timber,
            },
            // Long and plain: one roof, one ridge, a row of doors. It is
            // built to house people who are not related to each other, and
            // it looks it — no paint, no pride, just length.
            BuildingKind::Longhouse => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                // As with a house: a carried-in hall brings its own
                // footprint, and the plot, the levelled pad and the
                // walls are all cut to it. Leaving the rolled numbers
                // here put a bench hall three times the size on a pad
                // measured for the village's own.
                half_w: carried
                    .map(|work| work.half_w)
                    .unwrap_or_else(|| rng.range(2.7, 3.1)),
                half_d: carried
                    .map(|work| work.half_d)
                    .unwrap_or_else(|| rng.range(6.0, 7.2)),
                wall_h: rng.range(2.0, 2.3),
                walls: if rng.chance(0.7) {
                    pal::shade(&pal::WOOD, rng.range(0.5, 0.7))
                } else {
                    pal::shade(&pal::BONE, rng.range(0.75, 0.9))
                },
                roof: pal::shade(&pal::EARTH, rng.range(0.28, 0.42)),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            BuildingKind::Sawmill => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 2.3,
                half_d: 1.7,
                wall_h: 0.9,
                walls: pal::shade(&pal::WOOD, 0.5),
                roof: pal::shade(&pal::WOOD, 0.35),
                shed_roof: true,
                stuff: BuildStuff::Timber,
            },
            BuildingKind::Blacksmith => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.7,
                half_d: 1.9,
                wall_h: 1.7,
                walls: pal::shade(&pal::STONE, 0.45),
                roof: pal::shade(&pal::EARTH, 0.25),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            BuildingKind::Tavern => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 2.2,
                half_d: 2.3,
                wall_h: 1.9,
                walls: pal::shade(&pal::WOOD, 0.7),
                roof: pal::shade(&pal::CLOTH_RED, 0.35),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            BuildingKind::TownHall => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 2.5,
                half_d: 2.9,
                wall_h: 2.6,
                walls: pal::shade(&pal::BONE, 0.88),
                roof: pal::shade(&pal::CLOTH_BLUE, 0.35),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            BuildingKind::Shrine => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.3,
                half_d: 1.3,
                wall_h: 1.1,
                walls: pal::shade(&pal::STONE, 0.55),
                roof: pal::shade(&pal::CLOTH_GOLD, 0.6),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            // Long, low and windowless: a roof over the piles.
            BuildingKind::Storehouse => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 2.8,
                half_d: 1.6,
                wall_h: 1.3,
                walls: pal::shade(&pal::WOOD, 0.5),
                roof: pal::shade(&pal::EARTH, 0.35),
                shed_roof: true,
                stuff: BuildStuff::Timber,
            },
            // Squat and tall-roofed; its stilts show in the frame stage.
            BuildingKind::Granary => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.5,
                half_d: 1.5,
                wall_h: 1.7,
                walls: pal::shade(&pal::BONE, 0.7),
                roof: pal::shade(&pal::GRASS, 0.3),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            // A stone ring with a little peaked cap.
            BuildingKind::Well => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 0.9,
                half_d: 0.9,
                wall_h: 0.7,
                walls: pal::shade(&pal::STONE, 0.5),
                roof: pal::shade(&pal::WOOD, 0.45),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            // Dark-walled and low, stained by its own trade.
            BuildingKind::Smokehouse => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.4,
                half_d: 1.4,
                wall_h: 1.5,
                walls: pal::shade(&pal::WOOD, 0.25),
                roof: pal::shade(&pal::STONE, 0.3),
                shed_roof: true,
                stuff: BuildStuff::Timber,
            },
            // Tall for its footprint; the sails are the tell.
            BuildingKind::Mill => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.7,
                half_d: 1.7,
                wall_h: 2.8,
                walls: pal::shade(&pal::BONE, 0.8),
                roof: pal::shade(&pal::CLOTH_RED, 0.3),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            // Warm-walled, wide-doored, always faintly floured.
            BuildingKind::Bakery => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.8,
                half_d: 1.6,
                wall_h: 1.6,
                walls: pal::shade(&pal::EARTH, 0.55),
                roof: pal::shade(&pal::CLOTH_GOLD, 0.35),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            // A cottage hung with dyed cloth.
            BuildingKind::Weaver => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.6,
                half_d: 1.5,
                wall_h: 1.6,
                walls: pal::shade(&pal::CLOTH_BLUE, 0.5),
                roof: pal::shade(&pal::BONE, 0.6),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            // Small, green-roofed, half garden already.
            BuildingKind::Herbalist => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.3,
                half_d: 1.4,
                wall_h: 1.4,
                walls: pal::shade(&pal::WOOD, 0.6),
                roof: pal::shade(&pal::GRASS, 0.5),
                shed_roof: true,
                stuff: BuildStuff::Timber,
            },
            // A narrow stone finger with a platform at the top. Tall
            // enough to see over the canopy, which is the entire point of
            // climbing one - at three and a half metres it was a shed
            // with ambitions.
            BuildingKind::Watchtower => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: 1.35,
                half_d: 1.35,
                wall_h: 7.5,
                walls: pal::shade(&pal::STONE, 0.4),
                roof: pal::shade(&pal::WOOD, 0.4),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            // A portal driven into the hillside: the frame is all that
            // shows, the works are in the dark. half_d points into the rise.
            BuildingKind::Mine => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: rng.range(1.4, 1.7),
                half_d: 1.6,
                wall_h: 1.4,
                walls: pal::shade(&pal::STONE, 0.35),
                roof: pal::shade(&pal::WOOD, 0.4),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
            // No walls at all: a narrow deck run out over the water on
            // pilings. half_d is the long axis, pointing seaward.
            BuildingKind::Dock => Blueprint {
                kind,
                plan,
                // Set below, once for every kind.
                mirrored: false,
                half_w: rng.range(1.0, 1.3),
                half_d: rng.range(2.8, 3.4),
                wall_h: 0.9,
                walls: pal::shade(&pal::WOOD, 0.5),
                roof: pal::shade(&pal::WOOD, 0.35),
                shed_roof: false,
                stuff: BuildStuff::Timber,
            },
        }
    }
}

/// A building going up: how much timber has been worked into it so far, and
/// which visual stage stands — the village is *seen* to rise.
#[derive(Component, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ConstructionSite {
    pub progress: f32,
    pub stage: u8,
    /// Foundation stone laid so far, by masons, one carried block at a time.
    pub stone_laid: f32,
    /// Post-framed straight into the earth: no stone footing wanted. The
    /// path to shelter for a land that is all woods — and one day, when
    /// fire comes to the world, the homes that will fear it most.
    #[serde(default)]
    pub timber_footing: bool,
}

impl ConstructionSite {
    /// Stone this build's foundation expects before walls may rise.
    pub fn footing_stone(&self, kind: BuildingKind) -> f32 {
        if self.timber_footing {
            0.0
        } else {
            kind.stone_cost()
        }
    }
}

/// Cuts the yard at a mine's mouth and banks the hill over its back.
///
/// Lifted out of the raising and made pure, because this is where the mine has
/// been going wrong since it was written. Brett: "the mine building was
/// supposed to attach to the side of a mountain, but it never worked right…
/// they used to somehow build the mines underground."
///
/// They did, and here is the arithmetic. The crown that banks the hill over
/// the portal's back was centred `half_d * 1.2` — one and nine tenths of a
/// unit — uphill of the mouth, with a FULL-HEIGHT radius of `half_w * 1.4`,
/// about two and a tenth. Its flat top therefore reached back to two tenths of
/// a unit BEHIND the mouth: it covered the doorway. It raised that ground to
/// `wall_h + 1.4` above the floor, and a mine's walls are one and four tenths
/// tall — so the building finished a clear unit and a half under the hill. And
/// because worked ground is applied in the order it is registered, the crown
/// went on after the yard and won every argument between them.
///
/// Three rules keep it above ground AND under a hill, and they are geometric
/// rather than tuned — which matters, because the first two alone quietly
/// traded one failure for the other:
///
/// 1. The crown's flat top starts BEHIND the back wall, so no part of the
///    raised disc can sit over the portal.
/// 2. The yard is registered LAST, so where the two overlap it is the yard —
///    the level ground the mine actually stands on — that has the final say.
/// 3. And therefore the yard must not REACH the crown's flat top at all,
///    radius and falloff together. With only the first two rules the mine
///    came out of the ground and the hill went with it: the yard's falloff
///    ran straight through the crown and pulled a bank of two and eight
///    tenths down to seven tenths, leaving a portal with open sky behind it,
///    which reads as a shed. The suite has both halves of that.
///
/// Returns where the hill stands at its full height, so nothing has to
/// reproduce this arithmetic to ask about it.
pub(crate) fn bank_the_mine(terrain: &Terrain, face: Vec3, uphill: Vec3, plan: &Blueprint) -> Vec3 {
    // The yard: the level ground the mine stands on, and an apron at its
    // mouth. Kept tight, because rule three prices every unit of it.
    let yard_radius = plan.half_w + 1.0;
    let yard_falloff = 1.6;

    // The crown, placed so its flat top begins past the yard's whole reach.
    let crown_radius = plan.half_w;
    let crown_offset = yard_radius + yard_falloff + crown_radius;
    let crown = face + uphill * crown_offset;
    terrain.flatten(
        crown.x,
        crown.z,
        crown_radius,
        3.2,
        face.y + plan.wall_h + 1.4,
    );
    terrain.flatten(face.x, face.z, yard_radius, yard_falloff, face.y);
    crown
}

/// A finished building of any kind.
#[derive(Component, Debug)]
pub struct Building {
    pub kind: BuildingKind,
}

/// A finished house: a roof for one family.
#[derive(Component)]
pub struct Hut;

/// A piece of a building's roof. Marked so the roofs can be lifted off the
/// world as one — the god's cutaway view into every interior.
#[derive(Component)]
pub struct RoofPart;

/// A piece of a building's walls - the panels, their leavings, and the
/// frames set in them. The cutaway takes these down after the roof, so
/// a house can be watched the way a dollhouse is.
#[derive(Component)]
pub struct WallPart;

/// The family table, in the hearth room: supper gathers around it.
#[derive(Component)]
pub struct Table;

/// One doorway: where it stands in the building's own space, and which
/// way leads out of it.
#[derive(Clone, Copy)]
pub struct Doorway {
    pub at: Vec2,
    pub out: Vec2,
}

impl Doorway {
    /// A door on the +X wall, the way the village's own buildings put
    /// them: at the wall's edge, opening outward.
    pub fn on_x_wall(half_w: f32, z: f32) -> Self {
        Doorway {
            at: Vec2::new(half_w, z),
            out: Vec2::X,
        }
    }
}

impl Shell {
    /// The two standing places either side of a doorway, in the
    /// building's own space: one indoors, one genuinely out.
    ///
    /// A fixed step outward is not enough, and the bench longhouse is
    /// why: its door sits at x=3.65 while the shell reaches 9.65, so
    /// "1.6 metres outside the door" was still six metres inside the
    /// building. Every route out of it therefore ran from one indoor
    /// point to another, nobody ever crossed the wall, and ten founders
    /// starved in their beds with food in the store.
    ///
    /// So the outer place is FOUND rather than assumed: step along the
    /// door's own facing until the shell is behind you, then one more
    /// stride for daylight.
    pub fn door_stand(&self, door: &Doorway) -> (Vec2, Vec2) {
        let out = door.out.normalize_or(Vec2::X);
        let inside = |p: Vec2| p.x.abs() < self.half_w + 0.15 && p.y.abs() < self.half_d + 0.15;
        let mut clear = 1.6;
        // Bounded: a doorway that never clears the shell is a doorway
        // into the middle of the building, and a long step out is a
        // better answer than an endless loop.
        while clear < 40.0 && inside(door.at + out * clear) {
            clear += 0.5;
        }
        (door.at - out * 1.6, door.at + out * (clear + 1.0))
    }
}

/// The walls of a walkable interior: footprint half-extents in the
/// building's own space, and every doorway through them. The router
/// steers any walk that crosses these walls through a door instead —
/// villagers use doors now that insides are real places.
#[derive(Component)]
pub struct Shell {
    pub half_w: f32,
    pub half_d: f32,
    /// Every doorway, in the building's own space: where it stands and
    /// which way leads out. A door is no longer assumed to sit on the
    /// +X wall at the shell's own edge - a carried-in house puts its
    /// door where the maker put it.
    pub doors: Vec<Doorway>,
}

/// One bed inside a home, numbered so each occupant owns theirs. The count
/// is the capacity constant made physical: a house sleeps HOUSE_CAPACITY,
/// a longhouse LONGHOUSE_CAPACITY, and the furniture cannot drift from the
/// promise.
#[derive(Component)]
pub struct Bed {
    pub slot: u8,
    /// The turn a sleeping body takes about the building's own Y before it
    /// is tipped onto its back — an ANGLE, not an axis and a sign.
    ///
    /// It was a pair of those once: which way the bed ran, and which end
    /// the pillow was at. Every maker of a bed then had to know the
    /// unobvious mapping from that pair to the turn, and the moment a
    /// second maker appeared - a carried-in house, whose marks carry a
    /// real direction - the two disagreed by a quarter and its sleepers
    /// lay across the mattress.
    pub lie: f32,
    /// A bed made for two: the wedded pair of the household claims it
    /// together, each to their own side, and children never do.
    pub double: bool,
}

/// The turn that lays a body down with its head pointing `head` - the one
/// piece of arithmetic behind every sleeper in the world, in a bed or on
/// the bare ground. A body tipped onto its back has its head along -Z, so
/// the turn is read straight off the direction wanted.
pub fn lie_toward(head: Vec3) -> f32 {
    (-head.x).atan2(-head.z)
}

/// The turn a body takes to lie on a bed described the old way: which axis
/// the bed's length runs on, and which end its pillow is at. The quarter is
/// the other way from first instinct - a playtest photo of a sleeper lying
/// ACROSS the mattress is the authority - which is exactly why this is
/// written down once here instead of being rediscovered at every bedside.
pub fn lie_of(along_x: bool, head: f32) -> f32 {
    let mut lie = if along_x {
        0.0
    } else {
        std::f32::consts::FRAC_PI_2
    };
    if head > 0.0 {
        lie += std::f32::consts::PI;
    }
    lie
}

/// A finished longhouse: a roof for everyone with no family to sleep beside.
#[derive(Component)]
pub struct Longhouse;

/// A house that stands out past the rings, on its own ground, with its own
/// plot beside it.
///
/// Still the town's: its people belong to the settlement, eat from its stores,
/// carry timber to its woodpile and answer its famine watch. They simply do
/// not live in the square. Not every family wants a neighbour through the
/// wall, and a town where nobody chose otherwise reads as a single clump
/// rather than a place people settled.
#[derive(Component)]
pub struct Homestead;

/// How often a new family house is raised out on its own instead of in the
/// rings. Most families want the square; roughly one in four would rather
/// have the room.
const HOMESTEAD_CHANCE: f32 = 0.28;

/// A town needs a proper core before anyone chooses to live outside it.
const HOMESTEAD_MIN_POP: usize = 10;

/// How far past the outermost ring a homestead's ground begins.
const HOMESTEAD_STANDOFF: f32 = 26.0;

/// And how much further out it may go — kept inside a working walk, so the
/// square, the stores and the fire are all still a real part of their day.
const HOMESTEAD_SPREAD: f32 = 95.0;

/// How much clear ground a homestead keeps around itself. Far more than a
/// street house: the whole point is the room.
const HOMESTEAD_CLEARANCE: f32 = 38.0;

/// Candidate plots for a homestead: scattered in a band beyond the town's
/// rings, door turned back toward the square.
///
/// A band rather than the ring pattern, because the rings ARE the town — a
/// holding placed on one would just be another street house standing further
/// out.
pub(crate) fn homestead_slots(
    centre: Vec3,
    ring_reach: u32,
    rng: &mut Rng,
) -> Vec<(f32, f32, f32)> {
    let inner = 14.0 + ring_reach as f32 * 9.0 + HOMESTEAD_STANDOFF;
    // And never past a working walk: a holding whose people cannot reach the
    // square is not a holding, it is an abandonment. A town whose streets have
    // already grown out this far has no room left for new ones, and the caller
    // falls back to building in the rings.
    let outer = (inner + HOMESTEAD_SPREAD).min(crate::villager::work::WORK_REACH - 8.0);
    if inner >= outer {
        return Vec::new();
    }
    (0..90)
        .map(|_| {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let reach = rng.range(inner, outer);
            let (sin, cos) = angle.sin_cos();
            let (x, z) = (centre.x + cos * reach, centre.z + sin * reach);
            // The door still faces home.
            let toward = Vec3::new(centre.x - x, 0.0, centre.z - z).normalize_or_zero();
            (x, z, (-toward.z).atan2(toward.x))
        })
        .collect()
}

/// The moment a dock finishes — fresh-built or restored from a save — its
/// deck is opened to foot traffic: the terrain learns the planks as a
/// boardwalk, and from then on navigation, footing and the fisher's post
/// all treat the deck as ground.
pub(crate) fn open_boardwalks(
    terrain: Res<crate::terrain::Terrain>,
    docks: Query<(&Transform, &Building, Option<&Blueprint>), Added<Building>>,
) {
    for (transform, building, plan) in &docks {
        if building.kind != BuildingKind::Dock {
            continue;
        }
        let half_w = plan.map_or(1.15, |p| p.half_w);
        let half_d = plan.map_or(3.1, |p| p.half_d);
        let seaward = transform.rotation * Vec3::Z;
        terrain.register_boardwalk(
            transform.translation,
            Vec2::new(seaward.x, seaward.z),
            // The ramp starts a little shy of the origin; the deck ends at
            // twice the long half-axis out over the water.
            -half_d * 0.6,
            half_d * 2.0,
            // Wider than the planks look: the nav grid samples every 2.5
            // units, and a strip narrower than a cell slips between them.
            (half_w * 0.9).max(1.8),
            transform.translation.y + 0.68,
        );
    }
}

/// How many visible steps a building rises through. Three for the
/// village's own hand - footing, walls, roof - and four for a carried-in
/// house that was drawn with a frame, so its posts stand alone on the
/// footing for a while, the way a real house is raised.
pub fn steps_for(plan: &Blueprint) -> u8 {
    if let Some(work) = super::baked::drawing_at(plan.kind, plan.plan)
        && super::baked::has_frame(work)
    {
        4
    } else {
        3
    }
}

/// Which visual stage a build should show, at `progress` timber toward a
/// total `cost`, over `steps` steps: the stages land at even shares of
/// the whole, whatever the kind's cost and however many steps it has.
pub fn stage_for(progress: f32, cost: f32, steps: u8) -> u8 {
    ((progress / cost.max(1.0) * steps as f32) as u8).min(steps - 1)
}

/// The pitch every gabled roof in the world is cut to, in radians.
const GABLE_PITCH: f32 = 0.50;
/// How far a roof's underside clears the wall top at the outer edge of the
/// overhang. The reason walls do not poke through their own roofs.
const GABLE_SEAT: f32 = 0.12;
/// How far the eaves reach out past the gable wall.
const GABLE_OUT: f32 = 0.55;
/// How far the two slabs run past the ridge line, so they meet rather than
/// leaving a seam of daylight along the top.
const GABLE_LAP: f32 = 0.10;

/// How far a gabled roof's underside stands above the wall top, half a
/// building's width `w` from its middle. The single number the slabs, the
/// ridge beam and the gable end-caps are all cut from.
///
/// Everything is derived rather than tuned, because tuned numbers drift: the
/// first version of this drew the ridge beam a fifth of the building's width
/// ABOVE where the slabs actually met — a beam hanging in the air over every
/// gabled roof in the game — and made the slabs long enough that their outer
/// edges sank *below* the wall top, so the walls poked up through the roof
/// near the eaves. Both got worse the wider the building, because the eave
/// height was a constant while the slab length scaled with `w`.
pub fn gable_peak(w: f32) -> f32 {
    GABLE_SEAT + gable_span(w) * GABLE_PITCH.tan()
}

/// Half the width a gabled roof covers: the gable wall, plus the overhang.
fn gable_span(w: f32) -> f32 {
    // The gable end walls run a touch wider than the side walls, by their own
    // thickness; the roof has to clear the wider of the two.
    w * 1.05 + GABLE_OUT
}

/// The pitch a lean-to is cut to. Shallow on purpose: a working shed is a
/// shallow thing, and the sawmill should not look like a chapel.
const SHED_PITCH: f32 = 0.13;

/// How far a lean-to's underside stands above the wall top at its high side.
fn shed_head(w: f32) -> f32 {
    GABLE_SEAT + (w + gable_span(w)) * SHED_PITCH.tan()
}

/// Raises a lean-to: one tilted slab, the tall band that closes the high
/// side, and the stepped wedges that fill the sloping triangle at each end.
///
/// The wedges are the reason this is derived rather than tuned. They are
/// axis-aligned boxes sitting under a slope, and the roof above one is
/// LOWEST over its left-hand edge — so a wedge whose height came from its
/// step number instead of from the roof above it always broke through at that
/// corner. Every shed roof in the game was studded with little tabs.
fn raise_shed(
    part: &mut impl FnMut(Vec3, Vec3, f32, &Handle<StandardMaterial>),
    w: f32,
    d: f32,
    h: f32,
    roof: &Handle<StandardMaterial>,
    wall: &Handle<StandardMaterial>,
) {
    let tan = SHED_PITCH.tan();
    let cos = SHED_PITCH.cos();
    let span = gable_span(w);
    // The underside, measured up from the wall top, at any point across.
    let under = |x: f32| GABLE_SEAT + (x + span) * tan;

    // The slab: low edge out over -span, high edge out over +span.
    part(
        Vec3::new(0.0, h + under(0.0) + 0.06, 0.0),
        Vec3::new(2.0 * span / cos, 0.12, d * 2.4),
        SHED_PITCH,
        roof,
    );

    // The high wall, closing wall top to slab on the tall side. Measured at
    // the near face of the wall, where the roof is lowest across it.
    let head = under(w - 0.09);
    part(
        Vec3::new(w, h + head * 0.5, 0.0),
        Vec3::new(0.18, head, d * 2.1),
        0.0,
        wall,
    );

    // The sloping ends, as a staircase. Each tread rises only as far as the
    // roof allows over its low edge, so no corner can breach the slab.
    let limit = w * 1.05;
    let steps = 6;
    for k in 0..steps {
        let step = 2.0 * limit / steps as f32;
        let left = -limit + k as f32 * step;
        let tread = under(left) - 0.02;
        if tread <= 0.03 {
            continue;
        }
        for zed in [-d, d] {
            part(
                Vec3::new(left + step * 0.5, h + tread * 0.5, zed),
                Vec3::new(step, tread, 0.16),
                0.0,
                wall,
            );
        }
    }
}

/// Raises a gabled roof: two slabs, the ridge beam they meet under, and the
/// stepped end-caps that close the triangles at either end.
///
/// `w`, `d` and `h` are the building's half-width, half-depth and wall height;
/// the ridge runs along the local Z axis. Shared by every gabled building so
/// that a fix here cannot fix one roof and leave the rest wrong.
fn raise_gable(
    part: &mut impl FnMut(Vec3, Vec3, f32, &Handle<StandardMaterial>),
    w: f32,
    d: f32,
    h: f32,
    roof: &Handle<StandardMaterial>,
    wall: &Handle<StandardMaterial>,
    frame: &Handle<StandardMaterial>,
) {
    let tan = GABLE_PITCH.tan();
    let cos = GABLE_PITCH.cos();
    let span = gable_span(w);
    let peak = gable_peak(w);

    // Each slab covers from a little past the ridge out to the eave. Its
    // length is the hypotenuse of that run, so the covered span is exact
    // whatever the pitch.
    let reach = span + GABLE_LAP;
    let centre_x = (span - GABLE_LAP) * 0.5;
    let centre_y = GABLE_SEAT + (span - centre_x) * tan + 0.06;
    for side in [-1.0_f32, 1.0] {
        part(
            Vec3::new(side * centre_x, h + centre_y, 0.0),
            Vec3::new(reach / cos, 0.12, d * 2.35),
            -side * GABLE_PITCH,
            roof,
        );
    }

    // The ridge beam sits ON the join, straddling it.
    part(
        Vec3::new(0.0, h + peak, 0.0),
        Vec3::new(0.2, 0.2, d * 2.4),
        0.0,
        frame,
    );

    // The end-caps: bands stepped up under the slope, each only as wide as
    // the roof above it, and never wider than the wall below it.
    let limit = w * 1.05;
    for zed in [-d, d] {
        let mut y = 0.0_f32;
        while y < peak - 0.02 {
            let band = (peak - y).min(0.26);
            let top = y + band;
            // The roof's underside at this height, which is how wide the
            // masonry may be without breaking through it.
            let half = ((span - (top - GABLE_SEAT) / tan).min(limit) - 0.04).max(0.06);
            part(
                Vec3::new(0.0, h + y + band * 0.5, zed),
                Vec3::new(half * 2.0, band, 0.16),
                0.0,
                wall,
            );
            y = top;
        }
    }
}

/// Raises the visible stage of a building under construction, shaped and
/// coloured by its blueprint. Each stage spawns geometry as children of the
/// site, so the building accretes in place — and no two need look alike.
/// Something the god is pulling up out of the ground, and how far it has
/// come. Not saved: whatever was rising when the world was put down has
/// long since arrived.
#[derive(Component)]
pub struct OutOfTheEarth {
    /// The height it started buried at, and the one it is coming to.
    pub from: f32,
    pub to: f32,
    /// 0 still buried, 1 standing.
    pub risen: f32,
}

/// Seconds for the earth to give something up.
const RISES_OVER: f32 = 3.2;

/// The rising itself, eased so it slows as it arrives rather than
/// stopping dead.
pub(crate) fn rise_out_of_the_earth(
    mut commands: Commands,
    time: Res<Time>,
    mut rising: Query<(Entity, &mut Transform, &mut OutOfTheEarth)>,
) {
    for (entity, mut at, mut coming) in &mut rising {
        coming.risen = (coming.risen + time.delta_secs() / RISES_OVER).min(1.0);
        // Slow in, slow out: heavy at the start, settling at the end.
        // Interpolated from the REMEMBERED depth rather than from where
        // it happens to be this frame, or the easing reads its own
        // output and the whole rise drifts.
        let eased = coming.risen * coming.risen * (3.0 - 2.0 * coming.risen);
        at.translation.y = coming.from + (coming.to - coming.from) * eased;
        if coming.risen >= 1.0 {
            at.translation.y = coming.to;
            commands.entity(entity).remove::<OutOfTheEarth>();
        }
    }
}

/// Raises a building whole, in one breath, on ground nobody worked for
/// it — the god's own doing rather than a village's.
///
/// This is the founding hall, and it is the only building in the game
/// that is not built. Ten beds come up out of the earth under the light
/// and the founders walk out of them, which is both the opening of the
/// game and the reason `STARTING_POPULATION` is ten.
///
/// Deliberately does no terracing and rebuilds no chunks: the flag
/// already refused any ground that was not level enough for a village,
/// so there is nothing to flatten, and touching the terrain here would
/// drag half the world-building systems into the founding.
///
/// Returns where its door stands, so the people can come out of it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn raise_the_founding_hall(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    terrain: &mut Terrain,
    chunks: &mut crate::terrain::LoadedChunks,
    chunk_assets: &crate::terrain::TerrainAssets,
    stripped: &mut crate::scatter::StrippedGround,
    grass: &mut crate::grass::GrassChunks,
    standing: &[(Entity, Vec3)],
    settlement: Entity,
    centre: Vec3,
    rng: &mut Rng,
) -> Option<Vec3> {
    let plan = Blueprint::roll(BuildingKind::Longhouse, rng);
    let reach = plan.half_w.max(plan.half_d);

    // Well off the square: three of its own half-lengths out, so a hall
    // this size sits about fifty strides from the banner with its walls
    // a good thirty clear of it. Two half-lengths still read as crowding
    // the flag.
    let (at, angle) = (0..12)
        .map(|i| {
            let angle = i as f32 / 12.0 * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            let out = reach * 3.0 + 12.0;
            let (x, z) = (centre.x + cos * out, centre.z + sin * out);
            (Vec3::new(x, terrain.height_at(x, z), z), angle)
        })
        .filter(|(at, _)| terrain.is_walkable(at.x, at.z))
        .min_by(|a, b| {
            terrain
                .slope_at(a.0.x, a.0.z)
                .total_cmp(&terrain.slope_at(b.0.x, b.0.z))
        })?;

    // Facing the square. `Quat::from_rotation_y(yaw) * Vec3::X` is
    // `(cos yaw, 0, -sin yaw)`, and the way home is `-(cos a, 0, sin a)`
    // - so the turn that points a door at the banner is PI minus the
    // bearing, not the bearing plus PI. Adding PI flips the x and leaves
    // the z, which faced it a quarter of the way round the wrong way.
    let yaw = std::f32::consts::PI - angle;

    // And the ground is LEVELLED for it, the way it is for every other
    // building. Skipping this on the grounds that the flag had already
    // vetted the site was wrong: ground level enough for a village is
    // nowhere near level enough for a hall twenty-five metres long, and
    // the thing came up half buried.
    // The pad is the hall's own rectangle, turned the way the hall is, with a
    // pace of standing room around it. It used to be a circle wide enough to
    // hold the corners, which is nearly twice the ground and every bit of it
    // levelled - fine while nothing grew there, and a plateau once it did.
    let worked = terrain.terrace(at.x, at.z, plan.half_w, plan.half_d, yaw, 2.5, 2.4, at.y);

    // The clearing, BEFORE the chunks are swapped - a scattered tree is
    // a child of its chunk, and a chunk despawn takes its children with
    // it, which is the same ordering every other site-clearing obeys.
    // Nobody roofs over a living oak, and the god least of all.
    for (tree, tree_at) in standing {
        if tree_at.distance(at) < worked + 4.0 {
            stripped.strip(tree_at.x, tree_at.z);
            commands.entity(*tree).despawn();
        }
    }
    crate::terrain::rebuild_chunks_near(
        commands,
        meshes,
        chunk_assets,
        terrain,
        chunks,
        at.x,
        at.z,
        worked + 4.0,
    );
    // And the grass with it. Rebuilding the chunk leaves the grass where
    // it was until something else happens to invalidate it, so the blades
    // stood up through the floor and the porch for a good while after the
    // hall arrived. Every other site does these two together.
    grass.invalidate_near(commands, at.x, at.z, worked + 4.0);
    let at = Vec3::new(at.x, terrain.height_at(at.x, at.z), at.z);

    // How deep it starts. The whole building is set below the ground and
    // comes up out of it, so what the player sees is the earth giving up
    // a hall rather than one blinking into existence.
    let buried = plan.wall_h * 2.4 + 4.0;
    let hall = commands
        .spawn((
            Name::new(BuildingKind::Longhouse.name()),
            // One piece on any latitude - the founding hall was the building
            // Brett watched come out of the ground as cubism on the far side
            // of the world, because it rises through its own path and missed
            // the marking the ground-broken sites got.
            crate::globe::RigidlySeated,
            crate::villager::MemberOf(settlement),
            plan.clone(),
            Transform::from_translation(at - Vec3::Y * buried)
                .with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            crate::hand::PickRadius(reach + 0.9),
            crate::hand::Rooted,
            Building {
                kind: BuildingKind::Longhouse,
            },
            Longhouse,
            OutOfTheEarth {
                from: at.y - buried,
                to: at.y,
                risen: 0.0,
            },
        ))
        .id();
    // Every stage at once: footing, frame, walls, roof.
    for stage in 0..=steps_for(&plan) {
        raise_stage(commands, meshes, materials, hall, stage, &plan);
    }
    // And its beds, its table, its doors - out of the maker's own marks.
    match super::baked::drawing_at(BuildingKind::Longhouse, plan.plan) {
        Some(work) => super::baked::furnish_baked(commands, hall, work, plan.mirrored),
        None => {
            commands.entity(hall).insert(Shell {
                half_w: plan.half_w,
                half_d: plan.half_d,
                doors: vec![Doorway::on_x_wall(plan.half_w, 0.0)],
            });
        }
    }

    // The doorstep, in the world: where ten people are about to be
    // standing. Read off the drawing's own door when it has one.
    // Outside its door, found the same way every route out of it is
    // found - the drawing's shell reaches further than its doorway does,
    // and a fixed step put ten people indoors.
    let shell = Shell {
        half_w: plan.half_w,
        half_d: plan.half_d,
        doors: super::baked::drawing_at(BuildingKind::Longhouse, plan.plan)
            .map(|work| super::baked::doorways(work, plan.mirrored))
            .filter(|doors: &Vec<Doorway>| !doors.is_empty())
            .unwrap_or_else(|| vec![Doorway::on_x_wall(plan.half_w, 0.0)]),
    };
    let door = *shell.doors.first()?;
    let (_, outside) = shell.door_stand(&door);
    let step = at + Quat::from_rotation_y(yaw) * Vec3::new(outside.x, 0.0, outside.y);
    Some(Vec3::new(step.x, terrain.height_at(step.x, step.z), step.z))
}

/// Shows the mason's slab and doorstep, the moment the last block is laid.
///
/// One author, called by the mason's final block and by the save restoring a
/// site already laid — the save used to carry its own copy of the slab, so
/// the two could disagree, and did.
///
/// FOR THE VILLAGE'S OWN HAND ONLY. A carried-in drawing builds nothing that
/// is not in its own stages — its footing stood up with ground-breaking, the
/// way the maker staged it. The generated slab put under a carried-in house
/// wrapped a second, fatter plinth around the maker's own, sized to the
/// drawing's whole bounding box, eaves and chimney included. Brett: "weird
/// foundations showing up that are bigger than the house... there should be
/// nothing built outside of the design from atelier."
pub(crate) fn reveal_foundation(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    site: Entity,
    plan: &Blueprint,
) {
    if super::baked::drawing_at(plan.kind, plan.plan).is_some() {
        return;
    }

    // The village's own hand - the kinds the bench has not replaced yet -
    // still gets the mason's slab.
    let (w, d) = (plan.half_w, plan.half_d);
    let slab = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let stone = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::STONE, 0.4),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(slab.clone()),
        MeshMaterial3d(stone.clone()),
        Transform::from_xyz(0.0, PLINTH_TOP - 0.6, 0.0).with_scale(Vec3::new(
            w * 2.0 + 0.3,
            1.2,
            d * 2.0 + 0.3,
        )),
        ChildOf(site),
    ));
    // Two stone steps down from the threshold, on the door side. (Not for
    // the well - nobody steps up into a well.)
    if plan.kind == BuildingKind::Well {
        return;
    }
    for (out, top, depth) in [(0.32_f32, 0.24_f32, 0.6_f32), (0.78, 0.1, 0.55)] {
        commands.spawn((
            Mesh3d(slab.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(w + out, top - 0.02, 0.0).with_scale(Vec3::new(
                depth,
                top * 2.0,
                1.2,
            )),
            ChildOf(site),
        ));
    }
}

pub(crate) fn raise_stage(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    site: Entity,
    stage: u8,
    plan: &Blueprint,
) {
    // A house carried in from the bench is raised from its own boxes;
    // the village's own hand still builds everything else. It says WHICH out
    // loud, once per building, because the difference is the whole point of the
    // bench and there was no way to tell from a log which one you were looking
    // at - Brett, watching his own longhouse not appear: "5 tests in a row all
    // procedural longhouses".
    if stage == 0 {
        match super::baked::drawing_at(plan.kind, plan.plan) {
            Some(work) => info!("{} rises from the drawing {}", plan.kind.name(), work.name),
            None => info!(
                "{} rises by the village's own hand - no drawing carried in",
                plan.kind.name()
            ),
        }
    }
    if let Some(work) = super::baked::drawing_at(plan.kind, plan.plan) {
        super::baked::raise_baked(
            commands,
            meshes,
            materials,
            site,
            stage,
            work,
            plan.mirrored,
        );
        return;
    }

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let frame = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::WOOD, 0.45),
        perceptual_roughness: 0.9,
        ..default()
    });
    let wall = materials.add(StandardMaterial {
        base_color: plan.walls,
        perceptual_roughness: 0.95,
        ..default()
    });
    let roof = materials.add(StandardMaterial {
        base_color: plan.roof,
        perceptual_roughness: 1.0,
        ..default()
    });
    let stonework = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::STONE, 0.4),
        perceptual_roughness: 1.0,
        ..default()
    });
    let shadowed_water = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::CLOTH_BLUE, 0.12),
        perceptual_roughness: 0.2,
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.85),
        perceptual_roughness: 0.6,
        ..default()
    });

    let (w, d, h) = (plan.half_w, plan.half_d, plan.wall_h);

    let lift = if stage == 0 { 0.0 } else { PLINTH_TOP };
    // Several furnishing closures below need the spawner; a RefCell lets
    // them share it, borrowing only for the instant of each spawn.
    let cmd = std::cell::RefCell::new(commands);
    // While set, every spawned part is a piece of ROOF — liftable by the
    // cutaway view. Set around the roof-raising calls for the buildings
    // whose interiors exist.
    let roofing = std::cell::Cell::new(false);
    // And the same for the walls, which come down after the roof does.
    let walling = std::cell::Cell::new(false);
    let mut part = |offset: Vec3, size: Vec3, rot_z: f32, material: &Handle<StandardMaterial>| {
        let mut cmd = cmd.borrow_mut();
        let mut spawned = cmd.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(offset + Vec3::Y * lift)
                .with_rotation(Quat::from_rotation_z(rot_z))
                .with_scale(size),
            ChildOf(site),
        ));
        if roofing.get() {
            spawned.insert(RoofPart);
        }
        if walling.get() {
            spawned.insert(WallPart);
        }
    };

    // Furnishes one bed. The MATTRESS carries the Bed marker — its transform
    // is where a sleeper lies. `along_x` lays the bed's length on local X
    // (longhouse bays), otherwise on Z (house walls).
    let bed = |slot: u8, at: Vec3, along_x: bool, head_sign: f32, length: f32| {
        let lie = lie_of(along_x, head_sign);
        let (lx, lz) = if along_x {
            (length, 0.62)
        } else {
            (0.62, length)
        };
        let mut cmd = cmd.borrow_mut();
        // Frame.
        cmd.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(frame.clone()),
            Transform::from_translation(at + Vec3::Y * (lift + 0.14)).with_scale(Vec3::new(
                lx + 0.14,
                0.24,
                lz + 0.14,
            )),
            ChildOf(site),
        ));
        // Mattress: the sleeper's mark.
        cmd.spawn((
            Bed {
                slot,
                lie,
                double: false,
            },
            Mesh3d(cube.clone()),
            MeshMaterial3d(roof.clone()),
            Transform::from_translation(at + Vec3::Y * (lift + 0.3))
                .with_scale(Vec3::new(lx, 0.14, lz)),
            ChildOf(site),
        ));
        // Pillow, at the wall end.
        let head = if along_x {
            Vec3::new(head_sign * lx * 0.38, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, head_sign * lz * 0.38)
        };
        cmd.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(stonework.clone()),
            Transform::from_translation(at + head + Vec3::Y * (lift + 0.4)).with_scale(Vec3::new(
                if along_x { 0.3 } else { 0.5 },
                0.1,
                if along_x { 0.5 } else { 0.3 },
            )),
            ChildOf(site),
        ));
    };
    // A plank floor: the inside made a place instead of bare ground.
    let floor = |half_w: f32, half_d: f32| {
        let mut cmd = cmd.borrow_mut();
        cmd.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(frame.clone()),
            Transform::from_translation(Vec3::Y * (lift + 0.03)).with_scale(Vec3::new(
                half_w * 2.0 - 0.25,
                0.07,
                half_d * 2.0 - 0.25,
            )),
            ChildOf(site),
        ));
    };

    // A mine is a doorway into the hill, not a house: a heavy timber
    // portal, a stone surround, and the dark past the lintel. The long
    // local +Z axis points into the rise, so the works read as driven
    // underground rather than perched on top of it.
    if plan.kind == BuildingKind::Mine {
        let dark = materials.add(StandardMaterial {
            base_color: Color::srgb(0.03, 0.03, 0.04),
            perceptual_roughness: 1.0,
            ..default()
        });
        walling.set(stage == 1);
        match stage {
            // The portal frame: two squared posts and a lintel proud of
            // them both, braced like the first metre of a drift.
            0 => {
                for x in [-w * 0.55, w * 0.55] {
                    part(
                        Vec3::new(x, h * 0.5, d * 0.3),
                        Vec3::new(0.3, h, 0.3),
                        0.0,
                        &frame,
                    );
                }
                part(
                    Vec3::new(0.0, h + 0.12, d * 0.3),
                    Vec3::new(w * 1.5, 0.28, 0.4),
                    0.0,
                    &frame,
                );
            }
            // The stone surround and the mouth itself: a dark plane set
            // behind the frame, flanked by dressed-stone cheeks banked
            // into the slope.
            1 => {
                part(
                    Vec3::new(0.0, h * 0.5, d * 0.5),
                    Vec3::new(w * 1.1, h, 0.2),
                    0.0,
                    &dark,
                );
                for side in [-1.0, 1.0] {
                    part(
                        Vec3::new(side * w * 0.95, h * 0.45, d * 0.35),
                        Vec3::new(w * 0.55, h * 0.9, 0.9),
                        0.0,
                        &stonework,
                    );
                }
                part(
                    Vec3::new(0.0, h + 0.45, d * 0.45),
                    Vec3::new(w * 1.9, 0.7, 1.1),
                    0.0,
                    &stonework,
                );

                // The spur the adit is driven into.
                //
                // A mine wants a hillside, and the ground where the stone is
                // does not always have one. Brett, looking at a portal
                // standing on flat grass with its back open to the sky: "this
                // is the back of a mine, can we make this taper into the
                // ground since it cant attach to the side of a cliff?"
                //
                // So the mine brings its own rock. A spur of bedrock stands
                // over the lintel and steps down and back until it meets the
                // earth, with the doorway cut in its face — which reads as a
                // mine on ANY ground and asks the terrain for nothing. The
                // hill banked behind it by `bank_the_mine` still helps where
                // there is a hill to bank; this is what happens where there
                // is not.
                //
                // Stepped rather than a true wedge, because every other piece
                // of stone in this world is boxes and a smooth ramp here
                // would be the one thing in the frame with a diagonal on it.
                let spur = h + 1.1;
                for (back, tall, wide) in [
                    (0.75, 1.00, 2.2),
                    (1.45, 0.76, 2.0),
                    (2.15, 0.52, 1.7),
                    (2.85, 0.28, 1.3),
                ] {
                    let height = spur * tall;
                    part(
                        Vec3::new(0.0, height * 0.5, d * back),
                        Vec3::new(w * wide, height, d * 0.75),
                        0.0,
                        &stonework,
                    );
                }
            }
            // The working yard: a spoil heap grown to one side, crates,
            // and a lantern post for the shift that comes out after dark.
            _ => {
                part(
                    Vec3::new(w * 1.5, 0.35, -d * 0.4),
                    Vec3::new(1.7, 0.7, 1.4),
                    0.12,
                    &stonework,
                );
                part(
                    Vec3::new(-w * 1.1, 0.25, -d * 0.6),
                    Vec3::new(0.5, 0.5, 0.5),
                    0.0,
                    &wall,
                );
                part(
                    Vec3::new(-w * 1.0, 0.2, -d * 0.1),
                    Vec3::new(0.4, 0.4, 0.4),
                    0.3,
                    &wall,
                );
                part(
                    Vec3::new(w * 0.9, 0.9, -d * 0.9),
                    Vec3::new(0.12, 1.8, 0.12),
                    0.0,
                    &frame,
                );
                part(
                    Vec3::new(w * 0.9, 1.85, -d * 0.9),
                    Vec3::new(0.3, 0.3, 0.3),
                    0.0,
                    &gold,
                );
            }
        }
        return;
    }

    // A dock is planks, not walls: pilings driven toward the water, a deck
    // run out over it, and the trimmings of a working waterfront. The long
    // local +Z axis points seaward; the site origin sits on the last dry
    // ground, so everything past it hangs over the shallows.
    if plan.kind == BuildingKind::Dock {
        let deck = materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::WOOD, 0.55),
            perceptual_roughness: 0.95,
            ..default()
        });
        match stage {
            // Pilings first, in pairs marching off the shore. Tall enough
            // that their heads stand proud of the deck to come — bollards,
            // not an accident.
            0 => {
                for i in 0..4 {
                    let z = d * (0.2 + i as f32 * 0.55);
                    for x in [-w * 0.8, w * 0.8] {
                        part(
                            Vec3::new(x, 0.1, z),
                            Vec3::new(0.16, 1.8, 0.16),
                            0.0,
                            &frame,
                        );
                    }
                }
            }
            // The deck itself, shore to open water, with a short ramp
            // where it leaves the grass.
            1 => {
                part(
                    Vec3::new(0.0, 0.62, d * 0.9),
                    Vec3::new(w * 1.8, 0.12, d * 2.2),
                    0.0,
                    &deck,
                );
                part(
                    Vec3::new(0.0, 0.3, -d * 0.3),
                    Vec3::new(w * 1.6, 0.1, d * 0.5),
                    0.0,
                    &deck,
                );
            }
            // A rail down one side, a mooring post at the deck's end, and
            // the crates a working morning leaves behind.
            _ => {
                part(
                    Vec3::new(w * 0.85, 1.15, d * 0.9),
                    Vec3::new(0.08, 0.08, d * 2.0),
                    0.0,
                    &frame,
                );
                for i in 0..3 {
                    part(
                        Vec3::new(w * 0.85, 0.9, d * (0.2 + i as f32 * 0.75)),
                        Vec3::new(0.08, 0.5, 0.08),
                        0.0,
                        &frame,
                    );
                }
                part(
                    Vec3::new(-w * 0.6, 0.95, d * 1.85),
                    Vec3::new(0.2, 0.8, 0.2),
                    0.0,
                    &frame,
                );
                part(
                    Vec3::new(-w * 0.45, 0.85, d * 0.4),
                    Vec3::new(0.44, 0.44, 0.44),
                    0.0,
                    &wall,
                );
                part(
                    Vec3::new(-w * 0.5, 0.72, d * 0.9),
                    Vec3::new(0.34, 0.3, 0.34),
                    0.2,
                    &wall,
                );
            }
        }
        return;
    }

    // A well is its own shape entirely: a stone curb, two posts, a
    // windlass beam, a little peaked cap, and the bucket on its rope.
    if plan.kind == BuildingKind::Well {
        match stage {
            0 => {
                for (x, z) in [(-w, -d), (w, -d), (-w, d), (w, d)] {
                    part(
                        Vec3::new(x, 0.3, z),
                        Vec3::new(0.14, 0.6, 0.14),
                        0.0,
                        &frame,
                    );
                }
            }
            1 => {
                // The curb: four low stone walls, no door, water dark within.
                for (offset, size) in [
                    (Vec3::new(0.0, 0.3, -d), Vec3::new(w * 2.2, 0.6, 0.24)),
                    (Vec3::new(0.0, 0.3, d), Vec3::new(w * 2.2, 0.6, 0.24)),
                    (Vec3::new(-w, 0.3, 0.0), Vec3::new(0.24, 0.6, d * 2.2)),
                    (Vec3::new(w, 0.3, 0.0), Vec3::new(0.24, 0.6, d * 2.2)),
                ] {
                    part(offset, size, 0.0, &stonework);
                }
                part(
                    Vec3::new(0.0, 0.5, 0.0),
                    Vec3::new(w * 1.6, 0.05, d * 1.6),
                    0.0,
                    &shadowed_water,
                );
            }
            _ => {
                // Posts on the cross-axis, so the beam spans the approach
                // and the cap's slopes shed away from whoever draws water.
                for z in [-d, d] {
                    part(
                        Vec3::new(0.0, 1.0, z),
                        Vec3::new(0.14, 1.4, 0.14),
                        0.0,
                        &frame,
                    );
                }
                // The windlass beam and its handle.
                part(
                    Vec3::new(0.0, 1.62, 0.0),
                    Vec3::new(0.14, 0.14, d * 2.3),
                    0.0,
                    &frame,
                );
                // The rope and the bucket, hanging over the dark.
                part(
                    Vec3::new(0.0, 1.25, 0.0),
                    Vec3::new(0.04, 0.7, 0.04),
                    0.0,
                    &frame,
                );
                part(
                    Vec3::new(0.0, 0.86, 0.0),
                    Vec3::new(0.3, 0.26, 0.3),
                    0.0,
                    &frame,
                );
                // A little peaked cap over the beam, pitched across it. The
                // part helper only tilts around Z, so these two spawn by
                // hand, after the closure's last use in this arm.
                let _ = &part;
                let mut cmd = cmd.borrow_mut();
                for (zed, tilt) in [(-d * 0.4, -0.5_f32), (d * 0.4, 0.5)] {
                    cmd.spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(roof.clone()),
                        Transform::from_translation(
                            Vec3::new(0.0, 1.95, zed) + Vec3::Y * PLINTH_TOP,
                        )
                        .with_rotation(Quat::from_rotation_x(tilt))
                        .with_scale(Vec3::new(w * 1.6, 0.08, d * 1.15)),
                        ChildOf(site),
                    ));
                }
            }
        }
        return;
    }

    // A longhouse is a house stretched until it stops being one: the same
    // carpentry, but bay after bay of it, and a door to each bay instead of
    // one door to a home. The long axis is local Z; the doors face +X with
    // the rest of the village's.
    if plan.kind == BuildingKind::Longhouse {
        // Bays, not rooms: each is one door's worth of frontage, and the
        // count follows the rolled length so a long roof never grows a
        // lonely single door.
        let bays = ((d * 2.0 / 3.4).round() as i32).clamp(3, 4);
        let bay_z = |i: i32| -d + (i as f32 + 0.5) * (d * 2.0 / bays as f32);
        match stage {
            // Ground broken: a post at every bay line down both long walls,
            // and sills running the full length.
            0 => {
                for i in 0..=bays {
                    let z = -d + i as f32 * (d * 2.0 / bays as f32);
                    for x in [-w, w] {
                        part(
                            Vec3::new(x, 0.55, z),
                            Vec3::new(0.16, 1.1, 0.16),
                            0.0,
                            &frame,
                        );
                    }
                }
                for x in [-w, w] {
                    part(
                        Vec3::new(x, 0.08, 0.0),
                        Vec3::new(0.14, 0.16, d * 2.1),
                        0.0,
                        &frame,
                    );
                }
                for z in [-d, d] {
                    part(
                        Vec3::new(0.0, 0.08, z),
                        Vec3::new(w * 2.1, 0.16, 0.14),
                        0.0,
                        &frame,
                    );
                }
            }
            // Walls: the back and both ends solid, the front broken by one
            // door per bay with a lintel over each.
            1 => {
                part(
                    Vec3::new(-w, h * 0.5, 0.0),
                    Vec3::new(0.18, h, d * 2.1),
                    0.0,
                    &wall,
                );
                for z in [-d, d] {
                    part(
                        Vec3::new(0.0, h * 0.5, z),
                        Vec3::new(w * 2.1, h, 0.18),
                        0.0,
                        &wall,
                    );
                }
                // The front wall, run as the segments BETWEEN the doors so
                // the gaps land exactly on the bay centres however the
                // length rolled.
                let gap = 1.0;
                let mut edge = -d;
                for i in 0..bays {
                    let door = bay_z(i);
                    let left = door - gap * 0.5;
                    if left > edge {
                        part(
                            Vec3::new(w, h * 0.5, (edge + left) * 0.5),
                            Vec3::new(0.18, h, left - edge),
                            0.0,
                            &wall,
                        );
                    }
                    part(
                        Vec3::new(w, h - 0.06, door),
                        Vec3::new(0.18, 0.24, gap + 0.4),
                        0.0,
                        &frame,
                    );
                    edge = door + gap * 0.5;
                }
                if edge < d {
                    part(
                        Vec3::new(w, h * 0.5, (edge + d) * 0.5),
                        Vec3::new(0.18, h, d - edge),
                        0.0,
                        &wall,
                    );
                }
            }
            // The roof: one long gable, and a smoke louver over every
            // other bay — the tell that more than one hearth burns under it.
            _ => {
                roofing.set(true);
                raise_gable(&mut part, w, d, h, &roof, &wall, &frame);
                // A smoke louver over every other bay — the tell that more
                // than one hearth burns under this roof. They straddle the
                // ridge beam, so they follow it wherever the pitch puts it.
                for i in (0..bays).step_by(2) {
                    part(
                        Vec3::new(0.0, h + gable_peak(w) + 0.20, bay_z(i)),
                        Vec3::new(0.5, 0.34, 0.7),
                        0.0,
                        &frame,
                    );
                }
                roofing.set(false);
                // The inside: a floor, and a bed to every berth — one per
                // occupant the capacity promises, heads to the back wall,
                // the door lane along the front left clear.
                floor(w, d);
                let berths = crate::villager::home::LONGHOUSE_CAPACITY as u8;
                let length = (w * 0.95).clamp(1.75, 2.1);
                for slot in 0..berths {
                    let z = -d + (slot as f32 + 0.5) * (d * 2.0 / berths as f32);
                    bed(
                        slot,
                        Vec3::new(-w + length * 0.5 + 0.25, 0.0, z),
                        true,
                        -1.0,
                        length,
                    );
                }
            }
        }
        return;
    }

    match stage {
        // Ground broken: corner posts and sill beams (plus laid stone for the
        // buildings whose foundations demand it).
        0 => {
            for (x, z) in [(-w, -d), (w, -d), (-w, d), (w, d)] {
                part(
                    Vec3::new(x, 0.55, z),
                    Vec3::new(0.16, 1.1, 0.16),
                    0.0,
                    &frame,
                );
            }
            part(
                Vec3::new(0.0, 0.08, -d),
                Vec3::new(w * 2.1, 0.16, 0.14),
                0.0,
                &frame,
            );
            part(
                Vec3::new(0.0, 0.08, d),
                Vec3::new(w * 2.1, 0.16, 0.14),
                0.0,
                &frame,
            );
            part(
                Vec3::new(-w, 0.08, 0.0),
                Vec3::new(0.14, 0.16, d * 2.1),
                0.0,
                &frame,
            );
        }
        // Walls, with a door gap on the front — and each kind's own tells.
        1 => {
            part(
                Vec3::new(-w, h * 0.5, 0.0),
                Vec3::new(0.18, h, d * 2.1),
                0.0,
                &wall,
            );
            part(
                Vec3::new(0.0, h * 0.5, -d),
                Vec3::new(w * 2.1, h, 0.18),
                0.0,
                &wall,
            );
            part(
                Vec3::new(0.0, h * 0.5, d),
                Vec3::new(w * 2.1, h, 0.18),
                0.0,
                &wall,
            );
            let gap = 1.0;
            let seg = (d * 2.0 - gap) * 0.5;
            part(
                Vec3::new(w, h * 0.5, -(gap * 0.5 + seg * 0.5)),
                Vec3::new(0.18, h, seg),
                0.0,
                &wall,
            );
            part(
                Vec3::new(w, h * 0.5, gap * 0.5 + seg * 0.5),
                Vec3::new(0.18, h, seg),
                0.0,
                &wall,
            );
            part(
                Vec3::new(w, h - 0.06, 0.0),
                Vec3::new(0.18, 0.24, gap + 0.4),
                0.0,
                &frame,
            );

            match plan.kind {
                // A chimney of stone, shouldering past the roofline.
                BuildingKind::Blacksmith => {
                    part(
                        Vec3::new(-w * 0.6, h + 0.9, -d * 0.6),
                        Vec3::new(0.5, h + 1.8, 0.5),
                        0.0,
                        &stonework,
                    );
                }
                // A hanging sign on a bracket by the door.
                BuildingKind::Tavern => {
                    part(
                        Vec3::new(w + 0.5, h + 0.1, -0.8),
                        Vec3::new(1.0, 0.1, 0.1),
                        0.0,
                        &frame,
                    );
                    part(
                        Vec3::new(w + 0.85, h - 0.35, -0.8),
                        Vec3::new(0.55, 0.5, 0.08),
                        0.0,
                        &gold,
                    );
                }
                _ => {}
            }
        }
        // The roof: gable, or a working shed's single tilted slab.
        _ => {
            // A house's roof lifts for the cutaway view; the working
            // buildings keep theirs until their interiors are built.
            roofing.set(plan.kind == BuildingKind::House);
            if plan.shed_roof {
                raise_shed(&mut part, w, d, h, &roof, &wall);
            } else {
                raise_gable(&mut part, w, d, h, &roof, &wall, &frame);
            }
            roofing.set(false);
            if plan.kind == BuildingKind::House {
                // A house has ROOMS now: the door opens into the hearth
                // room — table, stools, a stone hearth — and a partition
                // with a wide doorway leads back to the bedroom where the
                // four beds stand. One family, one supper, one back room.
                floor(w, d);
                // The partition: bedroom takes the back (local -Z) third-to-
                // half; a generous doorway so nobody clips a wall.
                let split = -d * 0.15;
                let opening = 1.7;
                let leaf = (w * 2.0 - opening) * 0.5;
                for side in [-1.0_f32, 1.0] {
                    part(
                        Vec3::new(side * (opening * 0.5 + leaf * 0.5), h * 0.4, split),
                        Vec3::new(leaf, h * 0.8, 0.14),
                        0.0,
                        &wall,
                    );
                }
                // Lintel over the room door.
                part(
                    Vec3::new(0.0, h * 0.8, split),
                    Vec3::new(opening + 0.3, 0.16, 0.14),
                    0.0,
                    &frame,
                );

                // The bedroom: two beds along each side wall, heads out.
                let length = ((d * 0.85 + split) * 0.8).clamp(1.75, 2.1);
                for slot in 0..crate::villager::home::HOUSE_CAPACITY as u8 {
                    let side = if slot % 2 == 0 { -1.0 } else { 1.0 };
                    let rank = slot < 2;
                    let z = if rank {
                        -d + length * 0.5 + 0.3
                    } else {
                        split - length * 0.5 - 0.35
                    };
                    bed(
                        slot,
                        Vec3::new(side * (w - 0.55), 0.0, z),
                        false,
                        if rank { -1.0 } else { 1.0 },
                        length,
                    );
                }

                // The hearth room: a table mid-room, stools at its sides,
                // and a stone hearth against the back-facing partition.
                let table_at = Vec3::new(-w * 0.15, 0.0, (split + d) * 0.55);
                {
                    let mut cmd = cmd.borrow_mut();
                    cmd.spawn((
                        Table,
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(frame.clone()),
                        Transform::from_translation(table_at + Vec3::Y * (lift + 0.52))
                            .with_scale(Vec3::new(1.5, 0.1, 0.9)),
                        ChildOf(site),
                    ));
                    // Legs.
                    for (lx, lz) in [(-0.6, -0.32), (0.6, -0.32), (-0.6, 0.32), (0.6, 0.32)] {
                        cmd.spawn((
                            Mesh3d(cube.clone()),
                            MeshMaterial3d(frame.clone()),
                            Transform::from_translation(table_at + Vec3::new(lx, lift + 0.24, lz))
                                .with_scale(Vec3::new(0.12, 0.48, 0.12)),
                            ChildOf(site),
                        ));
                    }
                    // Stools.
                    for (sx, sz) in [(-1.15, 0.0), (1.15, 0.0), (0.0, -0.95), (0.0, 0.95)] {
                        cmd.spawn((
                            Mesh3d(cube.clone()),
                            MeshMaterial3d(frame.clone()),
                            Transform::from_translation(table_at + Vec3::new(sx, lift + 0.18, sz))
                                .with_scale(Vec3::new(0.42, 0.36, 0.42)),
                            ChildOf(site),
                        ));
                    }
                    // The hearth: dressed stone against the partition wall.
                    cmd.spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(stonework.clone()),
                        Transform::from_translation(Vec3::new(w * 0.62, lift + 0.42, split + 0.45))
                            .with_scale(Vec3::new(0.9, 0.84, 0.6)),
                        ChildOf(site),
                    ));
                }
            }
            // The gilded finial: on the ridge for a gable, on the high wall
            // for a shed. Either way it follows the roof rather than a
            // number copied out of it.
            if matches!(plan.kind, BuildingKind::TownHall | BuildingKind::Shrine) {
                let top = if plan.shed_roof {
                    shed_head(w) + 0.2
                } else {
                    gable_peak(w) + 0.3
                };
                part(
                    Vec3::new(0.0, h + top, 0.0),
                    Vec3::new(0.35, 0.35, 0.35),
                    0.0,
                    &gold,
                );
            }
        }
    }
}

/// One petition pinned up in the world, by seat number.
#[derive(Component)]
pub struct PetitionNote(pub usize);

/// The prayer board, nailed up in the world: a parchment with a pink pin
/// for every open prayer — the same channel the codex page reads, made a
/// place the god can fly to.
///
/// Prayers are addressed to the GOD, so they hang at the god's house: the
/// shrine's front wall. Before a shrine stands, the founding banner holds
/// them — the flag is the level-zero everything, and the first desperate
/// prayers tacked to the founding pole is the truth of a young village.
/// The town hall's door is deliberately left bare: that is the MAYOR'S
/// mail, when the town grows one, and the ratio of shrine-mail to
/// hall-mail will be the measure of what kind of town this is. Six at
/// most; past that the wall is simply covered, which says what it needs
/// to.
pub(crate) fn pin_petitions(
    mut commands: Commands,
    time: Res<Time>,
    mut since: Local<f32>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    prayers: Query<&crate::villager::MemberOf, With<crate::villager::belief::Prayer>>,
    shrines: Query<(Entity, &Building, &Blueprint, &crate::villager::MemberOf)>,
    settlements: Query<Entity, With<crate::villager::Settlement>>,
    notes: Query<(Entity, &PetitionNote, &ChildOf)>,
) {
    *since += time.delta_secs();
    if *since < 2.0 {
        return;
    }
    *since = 0.0;

    // The asks, counted per town.
    let mut asks: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for member in &prayers {
        *asks.entry(member.0).or_default() += 1;
    }

    // Where each town's mail hangs: its shrine, or its banner until one
    // stands. `(post, half_w)` — a shrine hangs notes on its door wall, a
    // banner (half_w zero) clusters them around the pole.
    let mut posts: std::collections::HashMap<Entity, (Entity, f32)> =
        std::collections::HashMap::new();
    for town in &settlements {
        posts.insert(town, (town, 0.0));
    }
    for (shrine, building, plan, owner) in &shrines {
        if building.kind == BuildingKind::Shrine {
            posts.insert(owner.0, (shrine, plan.half_w));
        }
    }

    // Whose wall wants how many notes.
    let wanted = |post: Entity| -> usize {
        posts
            .iter()
            .find(|(_, (p, _))| *p == post)
            .map_or(0, |(town, _)| asks.get(town).copied().unwrap_or(0).min(6))
    };

    // Take down the stale — answered prayers, and every note still on the
    // banner the day the shrine rises.
    let mut worn: std::collections::HashMap<(Entity, usize), bool> =
        std::collections::HashMap::new();
    for (note, seat, parent) in &notes {
        let post = parent.parent();
        if seat.0 < wanted(post) {
            worn.insert((post, seat.0), true);
        } else {
            commands.entity(note).despawn();
        }
    }

    // On the shrine, petitions flank the door; on the banner they cluster
    // round the pole. Each a little crooked, the way real notices weather.
    const WALL_SEATS: [(f32, f32, f32); 6] = [
        (-1.05, 1.75, 0.09),
        (1.05, 1.70, -0.07),
        (-1.45, 1.42, -0.11),
        (1.42, 1.38, 0.08),
        (-0.92, 1.26, 0.13),
        (0.90, 1.22, -0.10),
    ];

    for (town, (post, half_w)) in &posts {
        let want = asks.get(town).copied().unwrap_or(0).min(6);
        for index in 0..want {
            if worn.contains_key(&(*post, index)) {
                continue;
            }
            let parchment = materials.add(StandardMaterial {
                base_color: crate::palette::shade(&crate::palette::BONE, 0.97),
                perceptual_roughness: 1.0,
                ..default()
            });
            let pin = materials.add(StandardMaterial {
                base_color: crate::palette::shade(&crate::palette::CLOTH_PINK, 1.0),
                emissive: LinearRgba::from(crate::palette::shade(&crate::palette::CLOTH_PINK, 1.0))
                    * 0.8,
                perceptual_roughness: 0.8,
                ..default()
            });
            let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
            let seat = if *half_w > 0.0 {
                // The shrine wall, beside the door.
                let (z, y, tilt) = WALL_SEATS[index];
                Transform::from_xyz(half_w + 0.12, y, z)
                    .with_rotation(Quat::from_rotation_x(tilt))
                    .with_scale(Vec3::new(0.05, 0.42, 0.32))
            } else {
                // The banner pole, notes tacked around the shaft.
                let angle = index as f32 * std::f32::consts::TAU / 6.0 + 0.4;
                let (sin, cos) = angle.sin_cos();
                Transform::from_xyz(cos * 0.24, 1.15 + (index % 3) as f32 * 0.42, sin * 0.24)
                    .with_rotation(Quat::from_rotation_y(-angle))
                    .with_scale(Vec3::new(0.04, 0.34, 0.26))
            };
            let note = commands
                .spawn((
                    PetitionNote(index),
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(parchment),
                    seat,
                    ChildOf(*post),
                ))
                .id();
            commands.spawn((
                Mesh3d(cube),
                MeshMaterial3d(pin),
                Transform::from_xyz(0.9, 0.42, 0.0).with_scale(Vec3::new(1.4, 0.16, 0.2)),
                ChildOf(note),
            ));
        }
    }
}

/// The village has a shape: concentric rings of building plots around the
/// banner, each ring staggered against the last. Civic buildings take the
/// inner ring and face the plaza; houses fill outward, doors toward the
/// centre. Terrain still vetoes any plot, so the rings bend around rivers
/// and hills — planned, but not gridded.
///
/// Returns (x, z, yaw) per plot, innermost first. Yaw turns the door
/// (local +X) toward the centre.
pub(crate) fn village_slots(
    centre: Vec3,
    rings: std::ops::Range<u32>,
    span: f32,
) -> Vec<(f32, f32, f32)> {
    // The lanes are cut to the buildings that will stand in them: rings
    // a building's width apart, and slots along each ring the same. The
    // old fixed nine and twelve were measured for houses half this size,
    // and packed the new ones wall into wall.
    // A neighbourhood, not a barracks. The world is enormous and the
    // village is the only thing in it: gardens between the houses, a
    // lane wide enough to walk two abreast, and room for a roof to
    // overhang without touching the neighbour's.
    let apart = (span + 12.0).max(20.0);
    let mut slots = Vec::new();
    for ring in rings {
        let radius = 14.0 + ring as f32 * apart;
        let count = ((std::f32::consts::TAU * radius) / apart).floor().max(4.0) as u32;
        // The golden angle staggers each ring so lanes never align.
        let offset = ring as f32 * 2.399_963;
        for i in 0..count {
            let angle = offset + i as f32 / count as f32 * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            let x = centre.x + cos * radius;
            let z = centre.z + sin * radius;
            let toward = Vec3::new(centre.x - x, 0.0, centre.z - z).normalize_or_zero();
            let yaw = (-toward.z).atan2(toward.x);
            slots.push((x, z, yaw));
        }
    }
    slots
}

/// The settlement plans what to raise next: civic buildings by the growth
/// ladder, houses whenever people outnumber roofs.
pub(crate) fn plan_houses(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    terrain: Res<Terrain>,
    towns: Query<(Entity, &crate::villager::SettlementGround)>,
    mut turn: Local<usize>,
    mut rng: ResMut<SimRng>,
    mut stores: Query<&mut Stockpile>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    standing: Query<
        (Entity, &GlobalTransform, &crate::scatter::InGrove),
        With<crate::scatter::FellableTree>,
    >,
    mut ground: (
        ResMut<crate::terrain::LoadedChunks>,
        ResMut<crate::grass::GrassChunks>,
        Res<crate::terrain::TerrainAssets>,
        ResMut<crate::scatter::StrippedGround>,
        ResMut<crate::scatter::DirtyGroves>,
    ),
    census: (
        Query<
            (
                Option<&Vocation>,
                &crate::villager::Morale,
                Option<&crate::villager::home::Home>,
                Option<&crate::villager::belief::Faith>,
                Option<&Vitality>,
                Has<crate::creature::Childhood>,
                Option<&crate::villager::Spouse>,
                Option<&crate::villager::MemberOf>,
                Option<&crate::witness::Witnessed>,
                Option<&crate::villager::Courting>,
            ),
            (With<Villager>, Without<Corpse>),
        >,
        Query<&Field>,
        // The clock rides with the census because this system sits at
        // Bevy's parameter ceiling, and what the day is for is reading
        // the age of a memory in the census below.
        Res<crate::calendar::WorldClock>,
    ),
    // Bundled: this system sits at Bevy's parameter ceiling, and the three
    // plot queries belong together anyway — what stands, what is rising, and
    // what ground is already taken.
    plots: (
        Query<(&Building, &crate::villager::MemberOf)>,
        Query<(&Blueprint, &ConstructionSite, &crate::villager::MemberOf)>,
        Query<&Transform, Or<(With<ConstructionSite>, With<Hut>, With<Building>)>>,
    ),
) {
    *since_last += time.delta_secs();
    if *since_last < 12.0 {
        return;
    }
    *since_last = 0.0;
    let (civics, pending, roofs) = plots;

    // One town plans per tick, taken strictly in turn. The census below is
    // not cheap, and running it once per town per tick would scale badly; the
    // cost is that a settlement breaks ground a little less often the more
    // towns there are, which is a fair trade.
    let mut roster: Vec<(Entity, crate::villager::SettlementGround)> =
        towns.iter().map(|(town, ground)| (town, *ground)).collect();
    if roster.is_empty() {
        return;
    }
    roster.sort_unstable_by_key(|(town, _)| town.to_bits());
    *turn = (*turn).wrapping_add(1) % roster.len();
    let (settlement, home_ground) = roster[*turn];
    let site = crate::villager::SettlementSite {
        centre: home_ground.centre,
        radius: home_ground.radius,
        woodpile: home_ground.woodpile,
        settlement,
    };
    let (souls, fields, clock) = census;
    // What the village carries of the teeth, and how fresh. Gathered from
    // its OWN people: a settlement is not frightened by another's wolves.
    let mut remembered: Vec<&crate::witness::Witnessed> = Vec::new();

    // The census: what the village actually is, right now.
    let mut population = 0usize;
    let mut roofless_adults = 0usize;
    let mut spirits_sum = 0.0f32;
    let mut hurt = 0usize;
    let mut believers = 0usize;
    let mut fishers = 0usize;
    let mut farmers = 0usize;
    let mut foresters = 0usize;
    let mut miners = 0usize;
    let mut guards = 0usize;
    let mut healers = 0usize;
    let mut priests = 0usize;
    let mut betrothed = 0usize;
    // Shelter demand splits in two, because the roofs do: the wed and their
    // children want family houses, everyone else wants longhouse beds. A
    // village with four spare house rooms and no longhouse bed is not
    // housed — it is two buildings away from housed.
    let mut family_souls = 0usize;
    let mut single_souls = 0usize;
    for (vocation, morale, home, faith, vitality, child, spouse, member, held, courting) in &souls {
        if member.map(|m| m.0) != Some(settlement) {
            continue;
        }
        population += 1;
        if let Some(held) = held {
            remembered.push(held);
        }
        if courting.is_some_and(|courting| courting.ripe(clock.day())) {
            betrothed += 1;
        }
        spirits_sum += morale.spirits;
        if crate::villager::home::wants_family_roof(spouse, child) {
            family_souls += 1;
        } else {
            single_souls += 1;
        }
        if home.is_none() && !child {
            roofless_adults += 1;
        }
        if vitality.is_some_and(|v| v.harm > 0.15) {
            hurt += 1;
        }
        if faith.is_some_and(|f| f.is_believer()) {
            believers += 1;
        }
        match vocation {
            Some(Vocation::Fisher) => fishers += 1,
            Some(Vocation::Farmer) => farmers += 1,
            Some(Vocation::Forester) => foresters += 1,
            Some(Vocation::Miner) => miners += 1,
            Some(Vocation::Guard) => guards += 1,
            Some(Vocation::Healer) => healers += 1,
            Some(Vocation::Priest) => priests += 1,
            _ => {}
        }
    }
    // Hands at each trade that raises its own works.
    let hands_at = |trade: Vocation| match trade {
        Vocation::Fisher => fishers,
        Vocation::Miner => miners,
        Vocation::Forester => foresters,
        Vocation::Healer => healers,
        Vocation::Priest => priests,
        _ => 0,
    };
    // What the village is carrying of the teeth, and who is standing
    // watch because of it.
    let peril = crate::witness::peril_of(remembered.iter().copied(), clock.day());
    let standing_longhouses = civics
        .iter()
        .filter(|(b, member)| b.kind == BuildingKind::Longhouse && member.0 == settlement)
        .count();

    let has_kind = |kind: BuildingKind| {
        civics
            .iter()
            .any(|(b, member)| b.kind == kind && member.0 == settlement)
            || pending
                .iter()
                .any(|(b, _, member)| b.kind == kind && member.0 == settlement)
    };
    let Ok(store_now) = stores.get(site.settlement) else {
        return;
    };
    // Build from ANY material on hand - a treeless coast raises stone,
    // a clay bank raises mud brick. Only a village with nothing at all
    // waits.
    if store_now.timber < 2.0 && store_now.stone < 2.0 && store_now.clay < 2.0 {
        if roofless_adults > 0 {
            info!(
                "housing watch: {} roofless and the piles hold nothing to build from",
                roofless_adults
            );
        }
        return;
    }

    let shore_near = find_shore(&terrain, site.centre, &mut rng.0).is_some();

    // Ground fit for a mine mouth: walkable footing against a genuine
    // face — the ground a short walk uphill must stand well above the
    // mouth, so the drift is driven INTO something rather than standing
    // exposed on a rise. High flat country does not qualify, however
    // stony: a portal needs a wall of rock behind it.
    let minable = |t: &Terrain, x: f32, z: f32| {
        // Well above the tide, always: a steep coastal bank passes the
        // rise test on pure geometry, and a village once drove its mine
        // into the beach bluff beside the dock. Mines belong in rising
        // country, not the shore.
        if t.height_at(x, z) < WATER_LEVEL + 10.0 {
            return false;
        }
        let step = 3.5;
        let rise_x = t.height_at(x + step, z) - t.height_at(x - step, z);
        let rise_z = t.height_at(x, z + step) - t.height_at(x, z - step);
        let uphill = Vec3::new(rise_x, 0.0, rise_z).normalize_or_zero();
        if uphill == Vec3::ZERO {
            return false;
        }
        // A drift wants a bank to cut into, not a cliff. Three metres of
        // rise over seven is a twenty-three degree slope, and a village on
        // merely rolling country never found any - so it never wanted a
        // mine, and its only stone was the loose boulders, which run out.
        t.height_at(x + uphill.x * 7.0, z + uphill.z * 7.0) - t.height_at(x, z) > 1.8
    };
    // Thrown wide, because this is the question the whole stone economy
    // turns on and a false negative here costs the village every building
    // that wants masonry.
    let rock_near = find_ground_in(&terrain, site.centre, &mut rng.0, 200, minable).is_some();

    // A person needs a house, so a house gets built: ground breaks because
    // roofless people exist, not because a formula says the town is due.
    // Only when everyone sleeps under a roof does the village have the
    // spare hands for what it merely wants. One need outranks even the
    // roof: an empty larder beside open water breaks ground on the dock
    // first, because hunger kills faster than rain.
    let kind = if population >= 5
        && store_now.food() < (population as f32 * 0.8).max(8.0)
        && shore_near
        && !has_kind(BuildingKind::Dock)
    {
        BuildingKind::Dock
    // A trade with no works to work at raises its own — see OWN_WORKS.
    // The person who needs the building is the person who breaks ground
    // for it. Never before the first hall, though: ten beds under one
    // roof outranks any works, and a village that starts a sawmill on
    // its founding morning is a village milling planks over people
    // asleep in the open.
    } else if let Some(works) = (standing_longhouses > 0 || roofless_adults == 0)
        .then(|| {
            OWN_WORKS
                .iter()
                .find(|(trade, works, _)| {
                    hands_at(*trade) > 0
                        && !has_kind(*works)
                        && store_now.timber >= 2.0
                        && match works {
                            BuildingKind::Dock => shore_near,
                            BuildingKind::Mine => rock_near,
                            _ => true,
                        }
                })
                .map(|(_, works, _)| *works)
        })
        .flatten()
    {
        works
    // And one need outranks even that: a village that has been bitten
    // puts up a watch. This jumps the whole queue on purpose. The civic
    // ladder wants everyone housed first, ten souls in the town and six
    // stone in the pile, and by the time all three are true the fear has
    // faded and the tower is never built - while the tower is the only
    // thing in the world that actually stops a mauling, because wolves
    // will not hunt in its shadow. A guard is not civic ambition. It is
    // somebody frightened doing something about it.
    } else if guards > 0
        && peril >= 1.5
        && !has_kind(BuildingKind::Watchtower)
        && store_now.timber >= 2.0
        // But never before the first hall. Ten beds under one roof is
        // still the shortest road out of the dirt, and a village that
        // starts a tower on its founding morning is a village raising a
        // lookout over people asleep in the open.
        && (standing_longhouses > 0 || roofless_adults == 0)
    {
        BuildingKind::Watchtower
    } else if let Some(roof) = {
        let rising = |kind: BuildingKind| {
            pending
                .iter()
                .any(|(b, _, member)| b.kind == kind && member.0 == settlement)
        };
        let standing = |kind: BuildingKind| {
            civics
                .iter()
                .filter(|(b, member)| b.kind == kind && member.0 == settlement)
                .count()
        };
        next_roof(&RoofNeeds {
            family_souls,
            single_souls,
            roofless: roofless_adults,
            houses: standing(BuildingKind::House),
            longhouses: standing(BuildingKind::Longhouse),
            population,
            houses_rising: pending
                .iter()
                .filter(|(b, _, member)| b.kind == BuildingKind::House && member.0 == settlement)
                .count(),
            longhouse_rising: rising(BuildingKind::Longhouse),
        })
    } {
        roof
    } else if roofless_adults > 0 {
        // A roof is already rising for them - say so, with arithmetic,
        // so a stalled build is a visible fact instead of a silent
        // population ceiling.
        if let Some((plan, cs, _)) = pending.iter().find(|(b, _, member)| {
            member.0 == settlement
                && matches!(b.kind, BuildingKind::House | BuildingKind::Longhouse)
        }) {
            info!(
                "housing watch: {} roofless ({} want family rooms, {} want longhouse beds); {} stands at {:.0} of {:.0} timber, {:.0} of {:.0} footing stone",
                roofless_adults,
                family_souls,
                single_souls,
                plan.kind.name().to_lowercase(),
                cs.progress,
                plan.kind.timber_cost(),
                cs.stone_laid,
                cs.footing_stone(plan.kind),
            );
        }
        return;
    } else {
        let needs = CivicNeeds {
            population,
            stone: store_now.stone,
            timber_stored: store_now.timber,
            stone_stored: store_now.stone,
            food_stored: store_now.food(),
            avg_spirits: spirits_sum / population.max(1) as f32,
            homeless: roofless_adults,
            hurt,
            believers,
            fishers,
            farmers,
            foresters,
            fields: fields.iter().count(),
            peril,
            betrothed,
            pending_builds: pending
                .iter()
                .filter(|(_, _, member)| member.0 == settlement)
                .count(),
            shore_near,
            miners,
            rock_near,
        };
        match next_civic(&needs, has_kind) {
            Some(kind) => kind,
            None => return,
        }
    };
    let _ = roofs;

    // A mine is sited by the rock, not by the rings: the nearest walkable
    // ground beside rising stone takes the portal, driven uphill so the
    // works read as underground.
    //
    // See [`bank_the_mine`] for the ground work, and for the bug that made
    // this whole building a disappointment for as long as it has existed.
    if kind == BuildingKind::Mine {
        // Thrown as wide as the question that decided to want one. Asking
        // with forty darts what was answered with two hundred is how a
        // village comes to want a mine it can never find a face for.
        let Some(face) = find_ground_in(&terrain, site.centre, &mut rng.0, 200, minable) else {
            return;
        };
        // The portal faces uphill: sample the gradient and point local +Z
        // at the climb.
        let step = 4.0;
        let rise_x =
            terrain.height_at(face.x + step, face.z) - terrain.height_at(face.x - step, face.z);
        let rise_z =
            terrain.height_at(face.x, face.z + step) - terrain.height_at(face.x, face.z - step);
        let uphill = Vec3::new(rise_x, 0.0, rise_z).normalize_or(Vec3::Z);
        let yaw = uphill.x.atan2(uphill.z);
        let plan = Blueprint::roll(kind, &mut rng.0);
        // Set the portal INTO the cut: the mouth steps a stride up the
        // slope, and the flattened yard below carves a notch out of the
        // hillside for it to stand against.
        let face = face + uphill * 1.4;

        bank_the_mine(&terrain, face, uphill, &plan);
        let (chunks, grass, chunk_assets, stripped, dirty_groves) = &mut ground;
        // The felling comes FIRST, before the chunks are swapped. A
        // scattered tree is a child of its chunk, and a chunk despawn
        // takes its children with it - so clearing after the rebuild was
        // despawning entities that were already dead, dozens of ECS
        // warnings per ground-breaking.
        let mut cleared = 0.0;
        for (tree, tree_at, home) in &standing {
            if tree_at.translation().distance(face) < plan.half_w + 4.0 {
                stripped.strip(tree_at.translation().x, tree_at.translation().z);
                dirty_groves.0.push(home.0);
                commands.entity(tree).despawn();
                cleared += 1.0;
            }
        }
        if cleared > 0.0
            && let Ok(mut store) = stores.get_mut(site.settlement)
        {
            store.timber += cleared;
        }
        crate::terrain::rebuild_chunks_near(
            &mut commands,
            &mut meshes,
            chunk_assets,
            &terrain,
            chunks,
            face.x,
            face.z,
            plan.half_w + 9.0,
        );
        grass.invalidate_near(&mut commands, face.x, face.z, plan.half_w + 9.0);

        let building = commands
            .spawn((
                Name::new(format!("{}, rising", plan.kind.name())),
                ConstructionSite::default(),
                crate::villager::MemberOf(site.settlement),
                plan.clone(),
                Transform::from_translation(face).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
                crate::hand::PickRadius(plan.half_d + 0.9),
                crate::hand::Rooted,
            ))
            .id();
        raise_stage(
            &mut commands,
            &mut meshes,
            &mut materials,
            building,
            0,
            &plan,
        );
        info!("ground was broken: {}", plan.kind.name());
        notices.write(crate::ui::Notice::new(
            "Ground was broken for the mine".to_string(),
        ));
        return;
    }

    // A dock is sited by the water, not by the rings: the nearest walkable
    // shore takes the pilings, and the deck points out over the water.
    if kind == BuildingKind::Dock {
        let Some(shore) = find_shore(&terrain, site.centre, &mut rng.0) else {
            return;
        };
        let toward = shore - site.centre;
        let yaw = toward.x.atan2(toward.z);
        let plan = Blueprint::roll(kind, &mut rng.0);

        // A small worked pad on the dry end; the rest stands on pilings.
        terrain.flatten(shore.x, shore.z, plan.half_w + 1.2, 2.0, shore.y);
        let (chunks, grass, chunk_assets, stripped, dirty_groves) = &mut ground;
        // Felling before the chunk swap: see the mine above.
        let mut cleared = 0.0;
        for (tree, tree_at, home) in &standing {
            if tree_at.translation().distance(shore) < plan.half_w + 4.0 {
                stripped.strip(tree_at.translation().x, tree_at.translation().z);
                dirty_groves.0.push(home.0);
                commands.entity(tree).despawn();
                cleared += 1.0;
            }
        }
        if cleared > 0.0
            && let Ok(mut store) = stores.get_mut(site.settlement)
        {
            store.timber += cleared;
        }
        crate::terrain::rebuild_chunks_near(
            &mut commands,
            &mut meshes,
            chunk_assets,
            &terrain,
            chunks,
            shore.x,
            shore.z,
            plan.half_w + 5.0,
        );
        grass.invalidate_near(&mut commands, shore.x, shore.z, plan.half_w + 5.0);

        let building = commands
            .spawn((
                Name::new(format!("{}, rising", plan.kind.name())),
                ConstructionSite::default(),
                crate::villager::MemberOf(site.settlement),
                plan.clone(),
                Transform::from_translation(shore).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
                crate::hand::PickRadius(plan.half_d + 0.9),
                crate::hand::Rooted,
            ))
            .id();
        raise_stage(
            &mut commands,
            &mut meshes,
            &mut materials,
            building,
            0,
            &plan,
        );
        info!("ground was broken: {}", plan.kind.name());
        notices.write(crate::ui::Notice::new(
            "Ground was broken for the dock".to_string(),
        ));
        return;
    }

    // Civic buildings claim the inner ring; houses begin one ring out and
    // the town SPRAWLS: the ring search widens with the population, so a
    // growing city pushes its streets outward instead of hitting a wall
    // of full plots.
    let ring_reach = 5 + (population / 6) as u32;
    // Both roofs are homes, and homes leave the plaza to the civic works.
    let dwelling = matches!(kind, BuildingKind::House | BuildingKind::Longhouse);
    let rings = if kind == BuildingKind::Watchtower {
        // A watch post belongs on the edge of the village's reach, not
        // in the middle of its square. Civic works take the inner ring
        // because that is where people go to use them; nobody visits a
        // tower. Out here its shadow - the fifty-five metres wolves will
        // not hunt inside of - falls across the ground where people are
        // actually taken, which in a soak was sixty to a hundred and
        // forty strides out. In the plaza it covered nothing but the
        // plaza.
        2..(ring_reach.min(5) + 1)
    } else if dwelling {
        1..ring_reach
    } else {
        0..ring_reach.min(7)
    };
    // A longhouse is two and a half houses long, and the plot test runs on
    // world axes while the building is turned to face the centre — so it is
    // probed square, at its LONGEST extent, whichever way it ends up lying.
    // Conservative by construction: it turns down plots it could have used
    // rather than dropping one gable end into a hillside.
    // Some families would rather have the room than the neighbours: now and
    // then a family house is raised out past the rings on its own ground,
    // with a plot beside it. Still the town's people — they walk in to the
    // square, the stores and the fire — they simply do not live in it.
    let mut homestead = kind == BuildingKind::House
        && population >= HOMESTEAD_MIN_POP
        && rng.0.chance(HOMESTEAD_CHANCE);
    let mut plots = if homestead {
        homestead_slots(site.centre, ring_reach, &mut rng.0)
    } else {
        Vec::new()
    };
    if homestead && plots.is_empty() {
        // The streets have grown out as far as a holding could stand; this
        // family takes a plot in the rings after all.
        homestead = false;
    }
    // The plot is measured against the building that will stand in it,
    // not against a remembered size: a carried-in house is half again as
    // long as the village's own were, and the old numbers packed them
    // wall into wall. The probe reaches the building's LONGEST extent,
    // since the plot test runs on world axes while the house is turned
    // to face the centre; the clearance is two of those and a lane.
    // The size this KIND will be, known before a plan is rolled: a
    // carried-in house brings its own footprint, and the rolled ones
    // stay within their own range.
    let reach = match kind {
        BuildingKind::House => super::baked::widest(BuildingKind::House).unwrap_or(3.2),
        BuildingKind::Longhouse => super::baked::widest(BuildingKind::Longhouse).unwrap_or(7.2),
        BuildingKind::TownHall => super::baked::widest(BuildingKind::TownHall).unwrap_or(3.2),
        _ => 3.0,
    };
    let (probe, clearance) = if kind == BuildingKind::Longhouse {
        (reach + 0.9, reach * 2.0 + 12.0)
    } else if kind == BuildingKind::TownHall {
        // The hall squeezes into the heart of a village that already
        // stands: a tight clearance, or the founding longhouse beside the
        // square vetoes every seat and the town never gets its hall.
        (reach * 0.6, reach * 2.0 + 4.0)
    } else if homestead {
        // A wider probe, because the plot needs flat ground beside the house.
        (reach + 1.4, HOMESTEAD_CLEARANCE.max(reach * 2.0 + 20.0))
    } else {
        (reach * 0.6, reach * 2.0 + 12.0)
    };
    if !homestead {
        plots = village_slots(site.centre, rings, reach * 2.0);
    }
    // THE TOWN HALL TAKES THE SQUARE, not a ring plot. It is the flag's
    // own upgrade path: the banner was the civic seat from the founding
    // morning, and the hall is that seat grown a roof - raised just off
    // the plaza with its door facing the banner, so the flag ends up
    // standing before the hall it always promised. Candidates ring the
    // centre starting opposite the woodpile; the ordinary vetting below
    // still gets its veto on each.
    if kind == BuildingKind::TownHall {
        let breadth = reach + 5.5;
        let away = (site.centre - site.woodpile).with_y(0.0);
        let start = away.z.atan2(away.x);
        plots = (0..8)
            .map(|step| {
                let a = start + step as f32 * std::f32::consts::TAU / 8.0;
                let (sin, cos) = a.sin_cos();
                (
                    site.centre.x + cos * breadth,
                    site.centre.z + sin * breadth,
                    std::f32::consts::PI - a,
                )
            })
            .collect();
    }
    'darts: for (x, z, yaw) in plots {
        if !terrain.is_walkable(x, z) {
            continue;
        }
        let centre_height = terrain.height_at(x, z);
        // High and dry: nobody builds a home on the beach, however flat it is.
        if centre_height < WATER_LEVEL + 2.5 {
            continue;
        }
        for (dx, dz) in [
            (-probe, -probe),
            (probe, -probe),
            (-probe, probe),
            (probe, probe),
        ] {
            let corner = terrain.height_at(x + dx, z + dz);
            if !terrain.is_walkable(x + dx, z + dz) || (corner - centre_height).abs() > 0.9 {
                continue 'darts;
            }
        }
        // And clear of water on every side — the sea and rivers both.
        for step in 0..8 {
            let ring = step as f32 / 8.0 * std::f32::consts::TAU;
            let (rs, rc) = ring.sin_cos();
            let (wx, wz) = (x + rc * 7.0, z + rs * 7.0);
            if terrain.height_at(wx, wz) < WATER_LEVEL + 1.2
                || terrain.river_surface_at(wx, wz).is_some()
            {
                continue 'darts;
            }
        }
        let at = Vec3::new(x, centre_height, z);
        for other in &roofs {
            if other.translation.distance(at) < clearance {
                continue 'darts;
            }
        }

        let mut plan = Blueprint::roll(kind, &mut rng.0);
        if homestead {
            // A farmhouse is broader than a street house: it has work to
            // shelter as well as people, and no neighbour to crowd.
            plan.half_w *= 1.18;
            plan.half_d *= 1.12;
        }
        // A land of woods and nothing else still shelters its people: with
        // nothing to found on, a timber home is post-framed straight into
        // the earth and owes the masons nothing. Another path to a roof —
        // until fire comes to the world, when these will be the homes
        // that fear it.
        //
        // And a roofless village always takes that path. The test used to
        // be an EMPTY pile, so a founding hall with one chip of stone in
        // the store waited on the other three - and on the founding
        // morning that is a hundred seconds of everybody sleeping in the
        // dirt beside sixty timber, while one mason walks to a boulder
        // and back four times. Stone footings are for the village that
        // already has roofs; people in the open get posts in the ground
        // tonight.
        let nothing_to_found_on = store_now.stone < 1.0 && store_now.clay < 1.0;
        let timber_footing = dwelling && (nothing_to_found_on || roofless_adults > 0);
        if dwelling {
            // The land dictates the walls: too few standing trees within a
            // working walk makes timber homes a fantasy - build from what
            // the ground actually gives.
            let trees_near = standing
                .iter()
                .filter(|(_, t, _)| t.translation().distance(site.centre) < 140.0)
                .count();
            if trees_near < 6 && store_now.timber < 8.0 {
                use crate::palette as pal;
                if store_now.clay >= 4.0 {
                    plan.stuff = BuildStuff::MudBrick;
                    plan.walls = pal::shade(&pal::EARTH, rng.0.range(0.5, 0.65));
                    plan.roof = pal::shade(&pal::SAND, rng.0.range(0.4, 0.55));
                } else {
                    plan.stuff = BuildStuff::Stone;
                    plan.walls = pal::shade(&pal::STONE, rng.0.range(0.45, 0.6));
                    plan.roof = pal::shade(&pal::EARTH, rng.0.range(0.3, 0.45));
                }
            }
        }

        // Breaking ground levels the plot: the pad is worked flat and rolls
        // back into the hillside, so no floor, sill, or stake ever clips
        // into a slope. This is what a foundation is *for*.
        let worked = terrain.terrace(at.x, at.z, plan.half_w, plan.half_d, yaw, 1.6, 2.4, at.y);
        let (chunks, grass, chunk_assets, stripped, dirty_groves) = &mut ground;

        // And clears it properly: every tree within canopy's reach of the
        // walls is felled into the pile. Nobody roofs over a living oak,
        // and nobody wants branches through the bedroom either.
        //
        // The felling comes BEFORE the chunk swap. A scattered tree is a
        // child of its chunk, and a chunk despawn takes its children with
        // it, so clearing afterwards was despawning entities that were
        // already dead - dozens of ECS warnings per ground-breaking.
        let clearing = plan.half_w.max(plan.half_d) + 4.5;
        let mut cleared = 0.0;
        for (tree, tree_at, home) in &standing {
            if tree_at.translation().distance(at) < clearing {
                stripped.strip(tree_at.translation().x, tree_at.translation().z);
                dirty_groves.0.push(home.0);
                commands.entity(tree).despawn();
                cleared += 1.0;
            }
        }
        if cleared > 0.0
            && let Ok(mut store) = stores.get_mut(site.settlement)
        {
            store.timber += cleared;
        }
        crate::terrain::rebuild_chunks_near(
            &mut commands,
            &mut meshes,
            chunk_assets,
            &terrain,
            chunks,
            at.x,
            at.z,
            worked + 4.0,
        );
        grass.invalidate_near(&mut commands, at.x, at.z, worked + 4.0);

        let building = commands
            .spawn((
                Name::new(format!("{}, rising", plan.kind.name())),
                // Seated as one piece, or a far-latitude village rises as
                // cubism. See `globe::RigidlySeated`.
                crate::globe::RigidlySeated,
                ConstructionSite {
                    timber_footing,
                    ..default()
                },
                crate::villager::MemberOf(site.settlement),
                plan.clone(),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
                crate::hand::PickRadius(plan.half_w.max(plan.half_d) + 0.9),
                crate::hand::Rooted,
            ))
            .id();
        if homestead {
            commands.entity(building).insert(Homestead);
            info!(
                "ground was broken for a holding {:.0} strides out of town",
                site.centre.distance(at)
            );
        }
        raise_stage(
            &mut commands,
            &mut meshes,
            &mut materials,
            building,
            0,
            &plan,
        );
        info!("ground was broken: {}", plan.kind.name());
        // Houses go up all the time and are not news; everything else is,
        // the longhouse most of all — it is the village admitting it has
        // more grown children than rooms to put them in.
        if plan.kind != BuildingKind::House {
            notices.write(crate::ui::Notice::new(format!(
                "Ground was broken for {}",
                plan.kind.name().to_lowercase()
            )));
        }
        return;
    }
}

/// Sermons: the priest at the shrine retells what they have seen, and the
/// telling carries. Belief maintenance and rumour amplification in one — a
/// pulpit is a gossip engine with authority.
pub(crate) fn sermons(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut say: MessageWriter<crate::ui::Say>,
    name: Option<Res<crate::villager::DivineName>>,
    clock: Res<crate::calendar::WorldClock>,
    site: Option<Res<SettlementSite>>,
    mut stores: Query<&mut Stockpile>,
    shrines: Query<(&GlobalTransform, &Building)>,
    attention: Option<Res<crate::attention::Attention>>,
    mut tongue: Option<ResMut<crate::telling::Tongue>>,
    mut congregation: Query<
        (
            Entity,
            &Transform,
            &mut crate::witness::Witnessed,
            &mut crate::villager::belief::Faith,
            Option<&mut Chronicle>,
            &Vocation,
            &Activity,
        ),
        (With<Villager>, Without<Corpse>),
    >,
) {
    *since_last += time.delta_secs();
    if *since_last < 50.0 {
        return;
    }
    *since_last = 0.0;

    let Some(shrine) = shrines
        .iter()
        .find(|(_, b)| b.kind == BuildingKind::Shrine)
        .map(|(t, _)| t.translation())
    else {
        return;
    };

    // The priest must be at their post, with something to tell.
    let Some((preacher, at, memory, hand, trust, told)) = congregation
        .iter()
        .find(|(_, at, witnessed, _, _, vocation, activity)| {
            **vocation == Vocation::Priest
                && **activity == Activity::Working
                && at.translation.distance(shrine) < 6.0
                && !witnessed.recent.is_empty()
        })
        .map(|(preacher, at, witnessed, faith, _, _, _)| {
            (
                preacher,
                at.translation,
                witnessed.recent[0].clone(),
                crate::telling::Retelling::hand_of(witnessed),
                faith.trust,
                witnessed.told,
            )
        })
    else {
        return;
    };
    let sermon = memory.kind;
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());

    // The pulpit preaches in the priest's own words — the same corpus, the
    // same shape as any retelling, picked whether or not anyone is looking:
    // an unwatched pulpit still sways its congregation below, and only the
    // bubble waits on the god's regard.
    let regard = crate::attention::regard(attention.as_deref(), at);
    let composed = tongue.as_mut().and_then(|tongue| {
        tongue.line(&crate::telling::Retelling::new(
            sermon,
            hand,
            Some(Vocation::Priest),
            trust,
            memory.whom.clone(),
            told,
            memory.of,
        ))
    });
    if let Some(line) = composed
        && regard.worth_saying()
    {
        say.write(crate::ui::Say {
            speaker: preacher,
            text: format!("hear how {}", line.replace("the god", god)),
            thought: false,
            prayer: false,
        });
    }

    // Incense on the coals: the telling carries further and sinks
    // deeper. Sacred goods spend themselves feeding belief - that is
    // what makes them sacred.
    let censed = site
        .as_ref()
        .and_then(|s| stores.get_mut(s.settlement).ok())
        .is_some_and(|mut store| {
            if store.incense >= 0.5 {
                store.incense -= 0.5;
                true
            } else {
                false
            }
        });
    let (reach, sway) = if censed { (34.0, 0.05) } else { (22.0, 0.03) };

    let day = clock.day();
    for (_, at, mut witnessed, mut faith, chronicle, vocation, _) in &mut congregation {
        if *vocation == Vocation::Priest {
            continue;
        }
        if at.translation.distance(shrine) > reach {
            continue;
        }
        // The first hearing plants the story itself; every hearing after
        // deepens only the count and the faith.
        if witnessed.remembers(sermon) {
            witnessed.secondhand = witnessed.secondhand.saturating_add(1);
        } else {
            witnessed.hear(memory.clone());
        }
        faith.trust = (faith.trust + sway).min(0.8);
        if let Some(mut chronicle) = chronicle {
            chronicle.record(
                day,
                format!(
                    "heard the priest tell how {}",
                    sermon.rumor().replace("the god", god)
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mine stands ABOVE the ground it is cut into.
    ///
    /// Brett, on a building that has never once worked: "they used to somehow
    /// build the mines underground." They did — see [`bank_the_mine`] for the
    /// arithmetic of how. This is that failure written down, so it cannot come
    /// back the next time somebody tunes the hill behind the door.
    ///
    /// Measured over the building's whole footprint, on a slope steep enough
    /// that the mine's own siting test would accept it, and from several
    /// bearings — a bug that only shows when the hill climbs north-east is
    /// still a bug.
    #[test]
    fn a_mine_is_never_buried_by_the_hill_banked_behind_it() {
        for bearing in 0..8 {
            let terrain = crate::terrain::Terrain::new(4242);
            let turn = std::f32::consts::TAU * bearing as f32 / 8.0;
            let (sin, cos) = turn.sin_cos();
            let uphill = Vec3::new(cos, 0.0, sin);

            let face = Vec3::new(0.0, 40.0, 0.0);
            let plan = Blueprint {
                kind: BuildingKind::Mine,
                half_w: 1.55,
                half_d: 1.6,
                wall_h: 1.4,
                ..Blueprint::roll(BuildingKind::Mine, &mut crate::rng::Rng::new(1))
            };
            let _ = bank_the_mine(&terrain, face, uphill, &plan);

            // Every corner and edge of the floor the mine stands on.
            for dw in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
                for dd in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
                    // Local +Z is uphill; across it is the width.
                    let across = Vec3::new(-uphill.z, 0.0, uphill.x);
                    let at = face + across * (dw * plan.half_w) + uphill * (dd * plan.half_d);
                    let ground = terrain.height_at(at.x, at.z);
                    assert!(
                        ground <= face.y + 0.35,
                        "on bearing {bearing} the ground under the mine stands at \
                         {ground}, {:.2} above a floor at {} - the doorway is \
                         {:.2} under the hill and the walls are only {} tall",
                        ground - face.y,
                        face.y,
                        ground - face.y - plan.wall_h,
                        plan.wall_h,
                    );
                }
            }
        }
    }

    /// And the hill still rises behind it, which is the whole point of the
    /// crown: a portal with flat ground behind it reads as a shed.
    #[test]
    fn the_hill_still_stands_over_the_mines_back() {
        let terrain = crate::terrain::Terrain::new(4242);
        let uphill = Vec3::Z;
        let face = Vec3::new(0.0, 40.0, 0.0);
        let plan = Blueprint {
            kind: BuildingKind::Mine,
            half_w: 1.55,
            half_d: 1.6,
            wall_h: 1.4,
            ..Blueprint::roll(BuildingKind::Mine, &mut crate::rng::Rng::new(1))
        };
        // Asked exactly where the code says the hill stands, rather than at a
        // distance guessed here - two copies of that arithmetic is how the
        // burial got in.
        let crown = bank_the_mine(&terrain, face, uphill, &plan);
        let ground = terrain.height_at(crown.x, crown.z);
        assert!(
            ground > face.y + plan.wall_h,
            "the ground behind the portal stands at {ground} against a floor of \
             {} - the mine has open sky behind it and reads as a shed",
            face.y,
        );
    }
    use crate::villager::home::{HOUSE_CAPACITY, LONGHOUSE_CAPACITY};

    /// A village whose roofs exactly fit its people, with nothing rising.
    fn snug(family: usize, single: usize) -> RoofNeeds {
        RoofNeeds {
            family_souls: family,
            single_souls: single,
            houses: family.div_ceil(HOUSE_CAPACITY),
            longhouses: single.div_ceil(LONGHOUSE_CAPACITY),
            population: family + single,
            houses_rising: 0,
            longhouse_rising: false,
            // Snug means everyone is under a roof already, which is what
            // lets these cases test the slack arithmetic rather than the
            // shelter-first rule that outranks it.
            roofless: 0,
        }
    }

    #[test]
    fn the_hall_goes_up_before_any_house_while_people_sleep_outside() {
        // The founding: twelve in the open, three couples wed on the first
        // morning. The slack alone would have started a house - it did,
        // and the six wed could not take a longhouse bed anyway, so half
        // the village lay in the dirt waiting on family rooms.
        let founding = RoofNeeds {
            family_souls: 6,
            single_souls: 6,
            houses: 0,
            longhouses: 0,
            population: 12,
            houses_rising: 0,
            longhouse_rising: false,
            roofless: 12,
        };
        assert_eq!(next_roof(&founding), Some(BuildingKind::Longhouse));

        // With the hall up, houses resume for the families that want them.
        let sheltered = RoofNeeds {
            longhouses: 1,
            roofless: 0,
            ..founding
        };
        assert_eq!(next_roof(&sheltered), Some(BuildingKind::House));

        // And a hall already rising is not asked for twice.
        let waiting = RoofNeeds {
            longhouse_rising: true,
            ..founding
        };
        assert_ne!(next_roof(&waiting), Some(BuildingKind::Longhouse));
    }

    #[test]
    fn a_gabled_roof_clears_its_own_walls_at_every_rolled_size() {
        // The bug this pins: the eave height was a constant while the slab
        // length scaled with the building's width, so the wider the building
        // the further its roof edge sank BELOW the wall top — and the walls
        // came up through the roof near the eaves. Every gabled kind, at
        // every size the blueprints roll.
        let tan = GABLE_PITCH.tan();
        for w in [0.8, 1.3, 1.7, 2.1, 2.5, 2.9, 3.4] {
            let span = gable_span(w);
            // The roof must cover the wall it sits on, overhang included.
            assert!(
                span > w * 1.05,
                "at w={w} the roof does not reach the gable wall",
            );
            // Underside at the outermost point of the roof, relative to the
            // wall top. Positive means the wall stays under cover.
            let at_eave = GABLE_SEAT;
            assert!(
                at_eave > 0.0,
                "at w={w} the eave sits on or below the wall top",
            );
            // And at the wall line itself it is higher still.
            let at_wall = GABLE_SEAT + (span - w * 1.05) * tan;
            assert!(at_wall > at_eave, "at w={w} the roof slopes the wrong way",);
        }
    }

    #[test]
    fn a_shed_roofs_stepped_ends_stay_under_the_slab() {
        // The sawmill bug: each stepped wedge is an axis-aligned box under a
        // slope, and the roof above it is LOWEST over its left-hand edge. A
        // wedge sized from its step number instead of from the roof above
        // breaks through at that corner — eight little tabs on every shed
        // roof in the game. Every tread must clear.
        let tan = SHED_PITCH.tan();
        for w in [1.4, 1.7, 2.3, 2.8] {
            let span = gable_span(w);
            let under = |x: f32| GABLE_SEAT + (x + span) * tan;
            let limit = w * 1.05;
            let steps = 6;
            for k in 0..steps {
                let step = 2.0 * limit / steps as f32;
                let left = -limit + k as f32 * step;
                let tread = under(left) - 0.02;
                if tread <= 0.03 {
                    continue;
                }
                // The roof is lowest over `left`; the tread must sit below it.
                assert!(
                    tread < under(left),
                    "at w={w} tread {k} tops {tread} against a roof at {}",
                    under(left),
                );
            }
            // The slab must clear the wall top along its whole width, and the
            // high side must genuinely stand taller than the low side.
            assert!(under(-limit) > 0.0, "at w={w} the low eave cuts the wall");
            assert!(shed_head(w) > under(-limit), "at w={w} the shed is flat");
        }
    }

    #[test]
    fn the_ridge_beam_sits_where_the_slabs_actually_meet() {
        // The other half of the bug: the ridge was drawn a fifth of the
        // building's width above the join, so a beam hung in mid-air over
        // every gabled roof in the game. `gable_peak` is now the one number
        // the slabs, the beam and the end-caps are all cut from, so it must
        // equal the height the slope reaches at the middle of the building.
        let tan = GABLE_PITCH.tan();
        for w in [0.9, 1.6, 2.2, 3.0] {
            let reached = GABLE_SEAT + gable_span(w) * tan;
            assert!(
                (gable_peak(w) - reached).abs() < 1e-5,
                "at w={w} the ridge is at {} but the slopes reach {reached}",
                gable_peak(w),
            );
            // And a peak has to be above the wall it caps, or there is no roof.
            assert!(gable_peak(w) > GABLE_SEAT, "at w={w} the roof is flat");
        }
    }

    #[test]
    fn a_holding_stands_clear_of_the_town_but_within_a_walk() {
        // The whole point: outside the rings, so it reads as its own place —
        // and inside a working walk, so its people are still the town's.
        let mut rng = crate::rng::Rng::new(4);
        let mut any = false;
        for population in [10usize, 24, 60, 200] {
            let ring_reach = 5 + (population / 6) as u32;
            let outermost = 14.0 + ring_reach as f32 * 9.0;
            let plots = homestead_slots(Vec3::ZERO, ring_reach, &mut rng);
            // A town whose streets already reach a working walk has no room
            // left for new holdings, and says so by offering no plots.
            if plots.is_empty() {
                continue;
            }
            any = true;
            for (x, z, _) in plots {
                let out = Vec3::new(x, 0.0, z).length();
                assert!(
                    out > outermost,
                    "at population {population} a holding at {out:.0} sits inside the rings ({outermost:.0})",
                );
                assert!(
                    out < crate::villager::work::WORK_REACH,
                    "a holding at {out:.0} is past a working walk",
                );
            }
        }
        assert!(any, "no town size could place a holding at all");
    }

    #[test]
    fn a_holding_faces_the_town_it_belongs_to() {
        // The door looks homeward: these are the town's people living out, not
        // a separate settlement turning its back.
        let mut rng = crate::rng::Rng::new(9);
        for (x, z, yaw) in homestead_slots(Vec3::ZERO, 6, &mut rng) {
            let facing = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
            let homeward = Vec3::new(-x, 0.0, -z).normalize_or_zero();
            assert!(
                facing.dot(homeward) > 0.9,
                "a holding at ({x:.0}, {z:.0}) faces away from the square",
            );
        }
    }

    #[test]
    fn most_families_still_want_the_square() {
        // Some, not all: a town where everyone moved out is not a town.
        assert!(HOMESTEAD_CHANCE > 0.0 && HOMESTEAD_CHANCE < 0.5);
        // And a holding keeps far more room around it than a street house.
        assert!(HOMESTEAD_CLEARANCE > 30.0);
    }

    #[test]
    fn a_hamlet_roofs_its_families_before_it_dreams_of_a_longhouse() {
        // Five people cannot spare twelve timber for eight beds they will
        // not fill. Family roofs first; the unwed take the firelight.
        let needs = RoofNeeds {
            family_souls: 4,
            single_souls: 1,
            houses: 0,
            longhouses: 0,
            population: 5,
            ..default()
        };
        assert_eq!(next_roof(&needs), Some(BuildingKind::House));
    }

    #[test]
    fn grown_children_with_nowhere_to_go_raise_a_longhouse() {
        // Houses enough and to spare, but six unwed adults and no long
        // roof: the next build is theirs.
        let needs = RoofNeeds {
            family_souls: 4,
            single_souls: 6,
            houses: 3,
            longhouses: 0,
            population: 10,
            ..default()
        };
        assert_eq!(next_roof(&needs), Some(BuildingKind::Longhouse));
    }

    #[test]
    fn the_village_always_wants_one_spare_of_each() {
        // The whole rule in one assertion: roofs that fit the population
        // exactly are not enough — a village housed to the last bed is
        // still one wedding and one birthday from the rain.
        for (family, single) in [(8, 16), (4, 8), (12, 24)] {
            let needs = snug(family, single);
            assert!(
                next_roof(&needs).is_some(),
                "a village of {family} kin and {single} unwed, housed exactly, should still be building"
            );
        }
    }

    #[test]
    fn a_whole_spare_roof_of_each_kind_contents_it() {
        // And the appetite stops there: one empty house and one empty
        // longhouse is slack, not a shortage.
        let needs = RoofNeeds {
            family_souls: 8,
            single_souls: 8,
            houses: 3,
            longhouses: 2,
            population: 16,
            ..default()
        };
        assert_eq!(next_roof(&needs), None);
    }

    #[test]
    fn nothing_doubles_up_on_a_roof_already_rising() {
        let wants_house = RoofNeeds {
            family_souls: 8,
            single_souls: 0,
            houses: 2,
            longhouses: 1,
            population: 8,
            ..default()
        };
        assert_eq!(next_roof(&wants_house), Some(BuildingKind::House));
        assert_eq!(
            next_roof(&RoofNeeds {
                houses_rising: 1,
                ..wants_house
            }),
            None
        );
    }

    #[test]
    fn the_roof_further_behind_goes_first() {
        // Both wanted, but fourteen unwed against one longhouse is the
        // louder need than six kin against one house.
        let crowded_longhouse = RoofNeeds {
            family_souls: 6,
            single_souls: 14,
            houses: 1,
            longhouses: 1,
            population: 20,
            ..default()
        };
        assert_eq!(next_roof(&crowded_longhouse), Some(BuildingKind::Longhouse));
        // Turn the shortage around and the answer turns with it.
        let crowded_houses = RoofNeeds {
            family_souls: 14,
            single_souls: 6,
            ..crowded_longhouse
        };
        assert_eq!(next_roof(&crowded_houses), Some(BuildingKind::House));
    }

    #[test]
    fn a_longhouse_costs_what_the_beds_it_adds_are_worth() {
        // Per bed, the two roofs price the same: the longhouse is a bigger
        // commitment, not a better deal, so choosing it is about who needs
        // housing rather than about timber efficiency.
        let per_house = BuildingKind::House.timber_cost() / HOUSE_CAPACITY as f32;
        let per_long = BuildingKind::Longhouse.timber_cost() / LONGHOUSE_CAPACITY as f32;
        assert!((per_house - per_long).abs() < 0.01);
    }
}
