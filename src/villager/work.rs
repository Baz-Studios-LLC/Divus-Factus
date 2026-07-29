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
    Activity, Chronicle, HUNGRY_THRESHOLD, Needs, Person, SettlementSite, SimRng, Villager,
};
use crate::creature::anim::CreatureMotion;
use crate::creature::genome::{Age, CreatureGenome, Species};
use crate::creature::{Airborne, Corpse, Creature, Held, MoveTarget, Vitality};
use crate::rng::Rng;
use crate::scatter::FoodSource;
use crate::terrain::{Biome, Terrain, WATER_LEVEL};

/// How long one unit of work takes, standing at the worksite.
const WORK_SECONDS: f32 = 6.0;

/// How close counts as being at the worksite.
const WORK_RANGE: f32 = 2.8;

/// How far afield anyone will go to work.
const WORK_REACH: f32 = 170.0;

/// How far the stone plinth rises above grade. Every building's timber sits
/// on this, and the plinth reaches well below grade — so nothing clips into
/// a slope.
const PLINTH_TOP: f32 = 0.35;

/// Hunger above which a worker downs tools and sees to themself.
const DOWN_TOOLS_HUNGER: f32 = HUNGRY_THRESHOLD + 0.1;

/// What one hunt's kill is worth in stored food.
const CARCASS_FOOD: f32 = 3.0;

/// A calling. Rolled once at adulthood and kept.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Vocation {
    Gatherer,
    Fisher,
    Hunter,
    Miner,
    Forester,
    Carpenter,
    /// Tills fields, tends crops, brings in harvests.
    Farmer,
    /// Lays the stone: foundations and, one day, walls.
    Mason,
    /// Feeds the tavern's kitchen; the village eats better for it.
    Cook,
    /// Tends the hurt back to their feet.
    Healer,
    /// Keeps the shrine, and retells what the god has done.
    Priest,
    /// Walks past the cairns and brings back the world.
    Explorer,
    /// Spear and post: walks the edge, walks the roads, meets the wolves.
    Guard,
}

impl Vocation {
    /// The vocation as the inspector names it.
    pub fn describe(self) -> &'static str {
        match self {
            Vocation::Gatherer => "gathers",
            Vocation::Fisher => "fishes",
            Vocation::Hunter => "hunts",
            Vocation::Miner => "mines",
            Vocation::Forester => "cuts wood",
            Vocation::Carpenter => "builds houses",
            Vocation::Farmer => "works the fields",
            Vocation::Mason => "lays stone",
            Vocation::Cook => "keeps the kitchen",
            Vocation::Healer => "tends the hurt",
            Vocation::Priest => "keeps the shrine",
            Vocation::Explorer => "walks past the cairns",
            Vocation::Guard => "stands guard",
        }
    }

    /// The line a life gains on taking this up.
    pub fn taking_up(self) -> &'static str {
        match self {
            Vocation::Gatherer => "took up the gathering basket",
            Vocation::Fisher => "took up the net",
            Vocation::Hunter => "took up the spear",
            Vocation::Miner => "took up the pick",
            Vocation::Forester => "took up the axe",
            Vocation::Carpenter => "took up the hammer",
            Vocation::Farmer => "took up the plough",
            Vocation::Mason => "took up the chisel",
            Vocation::Cook => "took up the ladle",
            Vocation::Healer => "took up the salves",
            Vocation::Priest => "took up the litany",
            Vocation::Explorer => "took up the wayfarer's staff",
            Vocation::Guard => "took up the guard's spear",
        }
    }
}

/// Rolls a calling from who this person is. Boldness pushes toward the hunt;
/// its absence toward the quiet work. Weighted, not deterministic — a timid
/// hunter exists, and is a story.
pub fn roll_vocation(boldness: f32, rng: &mut Rng) -> Vocation {
    // Cook, healer and priest are missing on purpose: those callings are not
    // rolled into, they are *called* into, when the village needs them.
    let weights = [
        (Vocation::Gatherer, 0.5 + (1.0 - boldness) * 0.8),
        (Vocation::Fisher, 0.9),
        (Vocation::Hunter, 0.25 + boldness * 1.6),
        (Vocation::Miner, 0.7),
        (Vocation::Forester, 0.9),
        (Vocation::Carpenter, 0.8),
        (Vocation::Farmer, 1.0),
        (Vocation::Mason, 0.7),
        (Vocation::Explorer, 0.1 + boldness * 0.4),
    ];
    let total: f32 = weights.iter().map(|(_, w)| w).sum();
    let mut roll = rng.f32() * total;
    for (vocation, weight) in weights {
        roll -= weight;
        if roll <= 0.0 {
            return vocation;
        }
    }
    Vocation::Gatherer
}

/// One kind of food in the larder. What the village eats is what its
/// trades bring home, and the mix is the village's story: a fishing
/// hamlet lives on fish, a farm town on bread, a bad year on berries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoodKind {
    Berries,
    Fish,
    Meat,
    Grain,
    Bread,
}

impl FoodKind {
    pub fn name(self) -> &'static str {
        match self {
            FoodKind::Berries => "berries",
            FoodKind::Fish => "fish",
            FoodKind::Meat => "meat",
            FoodKind::Grain => "grain",
            FoodKind::Bread => "bread",
        }
    }
}

/// The village's food, kept by kind.
#[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Larder {
    pub berries: f32,
    pub fish: f32,
    pub meat: f32,
    pub grain: f32,
    pub bread: f32,
}

impl Larder {
    pub fn total(&self) -> f32 {
        self.berries + self.fish + self.meat + self.grain + self.bread
    }

    pub fn add(&mut self, kind: FoodKind, amount: f32) {
        *self.of(kind) += amount;
    }

    pub fn stock(&self, kind: FoodKind) -> f32 {
        match kind {
            FoodKind::Berries => self.berries,
            FoodKind::Fish => self.fish,
            FoodKind::Meat => self.meat,
            FoodKind::Grain => self.grain,
            FoodKind::Bread => self.bread,
        }
    }

    fn of(&mut self, kind: FoodKind) -> &mut f32 {
        match kind {
            FoodKind::Berries => &mut self.berries,
            FoodKind::Fish => &mut self.fish,
            FoodKind::Meat => &mut self.meat,
            FoodKind::Grain => &mut self.grain,
            FoodKind::Bread => &mut self.bread,
        }
    }

    /// Draws one meal. Bread first — baked to stretch, a bread meal
    /// draws only three quarters as deep — then whatever the larder is
    /// deepest in, spilling into the next kind if the first runs short.
    /// Returns the kind that made most of the meal.
    pub fn draw(&mut self, meal: f32) -> Option<FoodKind> {
        if self.bread >= meal * 0.75 {
            self.bread -= meal * 0.75;
            return Some(FoodKind::Bread);
        }
        let mut order = [
            FoodKind::Berries,
            FoodKind::Fish,
            FoodKind::Meat,
            FoodKind::Grain,
            FoodKind::Bread,
        ];
        order.sort_by(|a, b| self.stock(*b).total_cmp(&self.stock(*a)));
        let mut owed = meal;
        let mut first: Option<FoodKind> = None;
        for kind in order {
            let stock = self.of(kind);
            if *stock <= 0.0 {
                continue;
            }
            let taken = stock.min(owed);
            *stock -= taken;
            owed -= taken;
            first.get_or_insert(kind);
            if owed <= 0.0 {
                break;
            }
        }
        first
    }
}

/// The last kind of food a villager ate: sameness dulls, variety cheers.
#[derive(Component)]
pub struct LastMeal(pub FoodKind);

/// What the settlement has put by.
#[derive(Component, Debug, Default)]
pub struct Stockpile {
    pub larder: Larder,
    pub timber: f32,
    pub stone: f32,
    /// Raw ore out of a vein, waiting on the blacksmith's fire.
    pub ore: f32,
    /// Smelted iron: while any is held, every trade's tools bite better.
    pub iron: f32,
    /// Dug clay: a brick where stone runs short.
    pub clay: f32,
    /// Herb for the shrine's coals: a censed sermon carries further.
    pub incense: f32,
    /// Dye for the weaver's vats: bright cloth, brighter spirits.
    pub dye: f32,
}

impl Stockpile {
    /// Food of every kind together — the number the old ledgers kept.
    pub fn food(&self) -> f32 {
        self.larder.total()
    }
}

/// Timber one ordinary house costs, delivered one unit per work cycle.
pub const HOUSE_TIMBER: f32 = 6.0;

/// What a building is for. Shape, cost and effect all follow from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BuildingKind {
    House,
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
}

impl BuildingKind {
    pub fn timber_cost(self) -> f32 {
        match self {
            BuildingKind::House => HOUSE_TIMBER,
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
        }
    }

    /// Stone laid into the foundation when ground is broken.
    pub fn stone_cost(self) -> f32 {
        match self {
            BuildingKind::House => 2.0,
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
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BuildingKind::House => "A house",
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
    pub wolves_near: usize,
    pub pending_builds: usize,
    /// Whether walkable shore lies within working reach — no water, no dock.
    pub shore_near: bool,
}

/// Chooses the next civic building by NEED, not by a fixed ladder: each
/// candidate scores against what the village actually lacks, and the
/// loudest need above a threshold gets ground broken. Soft population
/// minimums keep hamlets from dreaming of town halls.
pub fn next_civic(needs: &CivicNeeds, has: impl Fn(BuildingKind) -> bool) -> Option<BuildingKind> {
    use BuildingKind::*;
    let candidates = [
        Well, Dock, Storehouse, Sawmill, Blacksmith, Smokehouse, Granary, Tavern, Mill, Bakery,
        Weaver, Herbalist, Shrine, Watchtower, TownHall,
    ];
    let min_pop = |kind: BuildingKind| match kind {
        Well | Dock => 5,
        Storehouse => 7,
        Sawmill => 8,
        Smokehouse => 9,
        Blacksmith | Granary | Tavern | Weaver | Herbalist | Watchtower => 10,
        Mill | Shrine => 12,
        Bakery => 14,
        TownHall => 18,
        House => 0,
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
            // Faith raises its own roof; fear raises a tower.
            Shrine => needs.believers as f32 * 0.12,
            Watchtower => needs.wolves_near as f32 * 0.4,
            TownHall => (needs.population as f32 - 16.0) / 8.0,
            House => 0.0,
        };
        if score > best.map_or(0.0, |(b, _)| b) {
            best = Some((score, kind));
        }
    }
    best.filter(|(score, _)| *score >= 0.45).map(|(_, k)| k)
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
}

impl Blueprint {
    pub fn roll(kind: BuildingKind, rng: &mut Rng) -> Blueprint {
        use crate::palette as pal;
        match kind {
            BuildingKind::House => Blueprint {
                kind,
                half_w: rng.range(1.9, 2.5),
                half_d: rng.range(2.0, 2.7),
                wall_h: rng.range(2.1, 2.6),
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
            },
            BuildingKind::Sawmill => Blueprint {
                kind,
                half_w: 2.3,
                half_d: 1.7,
                wall_h: 0.9,
                walls: pal::shade(&pal::WOOD, 0.5),
                roof: pal::shade(&pal::WOOD, 0.35),
                shed_roof: true,
            },
            BuildingKind::Blacksmith => Blueprint {
                kind,
                half_w: 1.7,
                half_d: 1.9,
                wall_h: 1.7,
                walls: pal::shade(&pal::STONE, 0.45),
                roof: pal::shade(&pal::EARTH, 0.25),
                shed_roof: false,
            },
            BuildingKind::Tavern => Blueprint {
                kind,
                half_w: 2.2,
                half_d: 2.3,
                wall_h: 1.9,
                walls: pal::shade(&pal::WOOD, 0.7),
                roof: pal::shade(&pal::CLOTH_RED, 0.35),
                shed_roof: false,
            },
            BuildingKind::TownHall => Blueprint {
                kind,
                half_w: 2.5,
                half_d: 2.9,
                wall_h: 2.6,
                walls: pal::shade(&pal::BONE, 0.88),
                roof: pal::shade(&pal::CLOTH_BLUE, 0.35),
                shed_roof: false,
            },
            BuildingKind::Shrine => Blueprint {
                kind,
                half_w: 1.3,
                half_d: 1.3,
                wall_h: 1.1,
                walls: pal::shade(&pal::STONE, 0.55),
                roof: pal::shade(&pal::CLOTH_GOLD, 0.6),
                shed_roof: false,
            },
            // Long, low and windowless: a roof over the piles.
            BuildingKind::Storehouse => Blueprint {
                kind,
                half_w: 2.8,
                half_d: 1.6,
                wall_h: 1.3,
                walls: pal::shade(&pal::WOOD, 0.5),
                roof: pal::shade(&pal::EARTH, 0.35),
                shed_roof: true,
            },
            // Squat and tall-roofed; its stilts show in the frame stage.
            BuildingKind::Granary => Blueprint {
                kind,
                half_w: 1.5,
                half_d: 1.5,
                wall_h: 1.7,
                walls: pal::shade(&pal::BONE, 0.7),
                roof: pal::shade(&pal::GRASS, 0.3),
                shed_roof: false,
            },
            // A stone ring with a little peaked cap.
            BuildingKind::Well => Blueprint {
                kind,
                half_w: 0.9,
                half_d: 0.9,
                wall_h: 0.7,
                walls: pal::shade(&pal::STONE, 0.5),
                roof: pal::shade(&pal::WOOD, 0.45),
                shed_roof: false,
            },
            // Dark-walled and low, stained by its own trade.
            BuildingKind::Smokehouse => Blueprint {
                kind,
                half_w: 1.4,
                half_d: 1.4,
                wall_h: 1.5,
                walls: pal::shade(&pal::WOOD, 0.25),
                roof: pal::shade(&pal::STONE, 0.3),
                shed_roof: true,
            },
            // Tall for its footprint; the sails are the tell.
            BuildingKind::Mill => Blueprint {
                kind,
                half_w: 1.7,
                half_d: 1.7,
                wall_h: 2.8,
                walls: pal::shade(&pal::BONE, 0.8),
                roof: pal::shade(&pal::CLOTH_RED, 0.3),
                shed_roof: false,
            },
            // Warm-walled, wide-doored, always faintly floured.
            BuildingKind::Bakery => Blueprint {
                kind,
                half_w: 1.8,
                half_d: 1.6,
                wall_h: 1.6,
                walls: pal::shade(&pal::EARTH, 0.55),
                roof: pal::shade(&pal::CLOTH_GOLD, 0.35),
                shed_roof: false,
            },
            // A cottage hung with dyed cloth.
            BuildingKind::Weaver => Blueprint {
                kind,
                half_w: 1.6,
                half_d: 1.5,
                wall_h: 1.6,
                walls: pal::shade(&pal::CLOTH_BLUE, 0.5),
                roof: pal::shade(&pal::BONE, 0.6),
                shed_roof: false,
            },
            // Small, green-roofed, half garden already.
            BuildingKind::Herbalist => Blueprint {
                kind,
                half_w: 1.3,
                half_d: 1.4,
                wall_h: 1.4,
                walls: pal::shade(&pal::WOOD, 0.6),
                roof: pal::shade(&pal::GRASS, 0.5),
                shed_roof: true,
            },
            // A narrow stone finger with a platform at the top.
            BuildingKind::Watchtower => Blueprint {
                kind,
                half_w: 1.1,
                half_d: 1.1,
                wall_h: 3.6,
                walls: pal::shade(&pal::STONE, 0.4),
                roof: pal::shade(&pal::WOOD, 0.4),
                shed_roof: false,
            },
            // No walls at all: a narrow deck run out over the water on
            // pilings. half_d is the long axis, pointing seaward.
            BuildingKind::Dock => Blueprint {
                kind,
                half_w: rng.range(1.0, 1.3),
                half_d: rng.range(2.8, 3.4),
                wall_h: 0.9,
                walls: pal::shade(&pal::WOOD, 0.5),
                roof: pal::shade(&pal::WOOD, 0.35),
                shed_roof: false,
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
}

/// A finished building of any kind.
#[derive(Component, Debug)]
pub struct Building {
    pub kind: BuildingKind,
}

/// A finished house.
#[derive(Component)]
pub struct Hut;

/// Which visual stage a build should show, at `progress` timber toward a
/// total `cost`: frame, walls, roof, at thirds of the way.
pub fn stage_for(progress: f32, cost: f32) -> u8 {
    ((progress / cost.max(1.0) * 3.0) as u8).min(2)
}

/// A job in progress: where, how far along, and — for hunters — whom.
#[derive(Component, Debug)]
pub struct Job {
    pub site: Vec3,
    pub progress: f32,
    pub focus: Option<Entity>,
    /// Seconds left before an unreached worksite is given up on. Without this a
    /// site the pathfinder cannot actually deliver them to becomes a career of
    /// standing in a field.
    pub patience: f32,
}

impl Job {
    /// A job at `site` for a worker starting `distance` away. Patience scales
    /// with the commute — a shore forty seconds out must not be abandoned at
    /// thirty-five.
    fn at(site: Vec3, focus: Option<Entity>, distance: f32) -> Job {
        Job {
            site,
            progress: 0.0,
            focus,
            patience: 20.0 + distance * 0.8,
        }
    }
}

/// Whether this is an hour anyone works. Dawn to mid-afternoon; evenings and
/// nights belong to the village.
pub fn is_work_hour(time_of_day: f32) -> bool {
    (0.05..0.62).contains(&time_of_day)
}

/// Timber in someone's arms, on its way somewhere.
///
/// The whole economy is visible now: a log exists in the world from the moment
/// the tree falls to the moment it becomes wall — on a shoulder, on the pile,
/// or nailed into a frame. Nothing teleports.
#[derive(Component, Debug)]
pub struct CarryingWood {
    pub amount: f32,
}

/// The visible log in a carrier's arms.
#[derive(Component)]
pub struct WoodLoad;

/// Stone in someone's arms, on its way to a foundation.
#[derive(Component, Debug)]
pub struct CarryingStone {
    /// Clay brick rather than stone: the refund must go back to the
    /// right pile if the errand is abandoned.
    pub clay: bool,
}

/// Puts a stone block in someone's arms.
pub(super) fn shoulder_stone(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    carrier: Entity,
) {
    commands.spawn((
        WoodLoad,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::STONE, 0.5),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.05, -0.38).with_scale(Vec3::new(0.45, 0.32, 0.34)),
        ChildOf(carrier),
    ));
}

/// A tilled field: crops rise from bare soil to harvest.
#[derive(Component, Debug)]
pub struct Field {
    pub growth: f32,
    pub farmer: Entity,
}

/// One row of crops, scaled up as the field grows.
#[derive(Component)]
pub struct CropRow {
    /// This stalk's full height at harvest.
    pub height: f32,
}

/// Raises a field's visible body - soil bed, furrow ridges, and stalks -
/// and returns the field entity. Used by the plough and by the save loader.
#[allow(clippy::too_many_arguments)]
pub fn raise_field(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rng: &mut Rng,
    at: Vec3,
    rotation: Quat,
    growth: f32,
    farmer: Entity,
) -> Entity {
    let field = commands
        .spawn((
            Name::new("A field"),
            Field { growth, farmer },
            Transform::from_translation(at).with_rotation(rotation),
            Visibility::default(),
            crate::hand::PickRadius(2.2),
            crate::hand::Rooted,
        ))
        .id();
    let bed = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::EARTH, 0.2),
        perceptual_roughness: 1.0,
        ..default()
    });
    let ridge = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::EARTH, 0.32),
        perceptual_roughness: 1.0,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(bed),
        Transform::from_xyz(0.0, 0.03, 0.0).with_scale(Vec3::new(3.8, 0.1, 3.1)),
        ChildOf(field),
    ));
    for lane in 0..4 {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(ridge.clone()),
            Transform::from_xyz(0.0, 0.1, lane as f32 * 0.8 - 1.2)
                .with_scale(Vec3::new(3.5, 0.09, 0.34)),
            ChildOf(field),
        ));
    }
    for lane in 0..4 {
        for slot in 0..6 {
            let shade = 0.5 + rng.range(0.0, 0.3);
            let crop = materials.add(StandardMaterial {
                base_color: crate::palette::shade(&crate::palette::GRASS, shade),
                perceptual_roughness: 0.9,
                ..default()
            });
            commands.spawn((
                CropRow {
                    height: rng.range(0.35, 0.62),
                },
                Mesh3d(cube.clone()),
                MeshMaterial3d(crop),
                Transform::from_xyz(
                    slot as f32 * 0.56 - 1.4 + rng.range(-0.1, 0.1),
                    0.2,
                    lane as f32 * 0.8 - 1.2 + rng.range(-0.06, 0.06),
                )
                .with_rotation(Quat::from_rotation_z(rng.range(-0.09, 0.09)))
                .with_scale(Vec3::new(0.07, 0.05, 0.07)),
                ChildOf(field),
            ));
        }
    }
    field
}

/// Crops grow on their own; a farmer's tending hurries them greatly.
pub(super) fn grow_crops(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    mut fields: Query<(&mut Field, &Children)>,
    mut rows: Query<(&mut Transform, &CropRow)>,
) {
    let dt = time.delta_secs();
    // Rain is the farmer's other pair of hands - and the season is the
    // hand over both: winter fields sleep, and the village lives on what
    // the granary holds.
    let watered = weather.map_or(1.0, |w| 1.0 + w.intensity * 1.5);
    let seasonal = clock.season().growth();
    for (mut field, children) in &mut fields {
        field.growth = (field.growth + dt * watered * seasonal / 600.0).min(1.0);
        for &child in children {
            if let Ok((mut stalk, crop)) = rows.get_mut(child) {
                let height = crop.height * (0.1 + field.growth * 0.9);
                stalk.scale.y = height;
                stalk.translation.y = 0.14 + height * 0.5;
            }
        }
    }
}

/// The tavern kitchen's warmth, while a cook keeps it: meals instead of
/// scraps, for everyone who eats.
#[derive(Resource, Default)]
pub struct KitchenWarm {
    pub until: f64,
}

/// One log of the settlement's visible woodpile; shown while the stockpile
/// holds at least this many timber.
#[derive(Component)]
pub struct WoodpileLog(pub u8);

/// One block of the visible stone pile; shown while the stockpile holds at
/// least this much stone.
#[derive(Component)]
pub struct StonePileBlock(pub u8);

/// One sack of the visible food store; each stands for two food.
#[derive(Component)]
pub struct FoodSack(pub u8);

/// Which store a visible pile stands for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PileKind {
    Food,
    Timber,
    Stone,
}

/// Marks a pile in the square as an inspectable face of the stockpile.
#[derive(Component)]
pub struct StorePile(pub PileKind);

/// A rolling record of the stores, so a hovered pile can say not just how
/// much is there but which way it is going.
#[derive(Resource, Default)]
pub struct StoreTrends {
    samples: std::collections::VecDeque<(f64, f32, f32, f32)>,
}

impl StoreTrends {
    /// Net change per minute over the sampled window, by kind.
    pub fn rate_per_minute(&self, kind: PileKind) -> f32 {
        let (Some(oldest), Some(newest)) = (self.samples.front(), self.samples.back()) else {
            return 0.0;
        };
        let span = (newest.0 - oldest.0) as f32;
        if span < 10.0 {
            return 0.0;
        }
        let pick = |sample: &(f64, f32, f32, f32)| match kind {
            PileKind::Food => sample.1,
            PileKind::Timber => sample.2,
            PileKind::Stone => sample.3,
        };
        (pick(newest) - pick(oldest)) / span * 60.0
    }
}

/// Samples the stores every couple of seconds, keeping about ninety.
pub(super) fn track_store_trends(
    clock: Res<crate::calendar::WorldClock>,
    site: Option<Res<SettlementSite>>,
    stores: Query<&Stockpile>,
    mut trends: ResMut<StoreTrends>,
) {
    let Some(store) = site.and_then(|site| stores.get(site.settlement).ok()) else {
        return;
    };
    if trends
        .samples
        .back()
        .is_some_and(|(t, ..)| clock.elapsed - t < 2.0)
    {
        return;
    }
    let now = clock.elapsed;
    trends
        .samples
        .push_back((now, store.food(), store.timber, store.stone));
    while trends.samples.front().is_some_and(|(t, ..)| now - t > 90.0) {
        trends.samples.pop_front();
    }
}

/// Puts a log in someone's arms.
pub(super) fn shoulder_wood(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    carrier: Entity,
) {
    commands.spawn((
        WoodLoad,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::WOOD, 0.4),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.05, -0.38).with_scale(Vec3::new(0.95, 0.2, 0.2)),
        ChildOf(carrier),
    ));
}

/// Takes the log back out of their arms.
fn shed_wood(
    commands: &mut Commands,
    carrier: Entity,
    children: &Query<&Children>,
    loads: &Query<Entity, With<WoodLoad>>,
) {
    if let Ok(kids) = children.get(carrier) {
        for &child in kids {
            if loads.contains(child) {
                commands.entity(child).despawn();
            }
        }
    }
}

/// Carriers walk their wood to the pile and put it down.
pub(super) fn haul_wood(
    mut commands: Commands,
    site: Option<Res<SettlementSite>>,
    mut stores: Query<&mut Stockpile>,
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
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let Some(site) = site else {
        return;
    };
    let Ok(mut store) = stores.get_mut(site.settlement) else {
        return;
    };

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

        if transform.translation.distance(site.woodpile) > 2.6 {
            target.0 = Some(site.woodpile);
            continue;
        }
        store.timber += carrying.amount;
        commands.entity(entity).remove::<CarryingWood>();
        shed_wood(&mut commands, entity, &children, &loads);
        *activity = Activity::Idle;
        target.0 = None;
    }
}

/// The pile shows what the store holds, log by log.
pub(super) fn update_woodpile(
    site: Option<Res<SettlementSite>>,
    stores: Query<&Stockpile>,
    moving: Query<&Rehouse>,
    mut logs: Query<(&WoodpileLog, &ChildOf, &mut Visibility)>,
) {
    let Some(site) = site else {
        return;
    };
    let Ok(store) = stores.get(site.settlement) else {
        return;
    };
    for (log, parent, mut visibility) in &mut logs {
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

/// A pile being carried to its new home, one armload at a time.
#[derive(Component)]
pub struct Rehouse {
    pub to: Vec3,
    pub to_rot: Quat,
    pub hauled: u8,
    pub goal: u8,
}

/// The villager doing the carrying, and which pile they serve.
#[derive(Component)]
pub struct RehouseHauler(pub Entity);

/// A load in a rehousing hauler's arms (visual already on their shoulder).
#[derive(Component)]
pub struct RehouseLoad;

/// A sack on the shoulder, for carrying the food store home.
pub(super) fn shoulder_sack(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    carrier: Entity,
) {
    commands.spawn((
        WoodLoad,
        Mesh3d(meshes.add(Cuboid::new(0.42, 0.36, 0.42))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::BONE, 0.6),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.5, 0.1),
        ChildOf(carrier),
    ));
}

/// When the storehouse rises, the village carries its piles in under the
/// eaves - and the granary takes the food sacks. Nothing teleports: each
/// armload is walked across the square.
#[allow(clippy::type_complexity)]
pub(super) fn stores_move_indoors(
    mut commands: Commands,
    site: Option<Res<SettlementSite>>,
    stores: Query<&Stockpile>,
    mut notices: MessageWriter<crate::ui::Notice>,
    new_buildings: Query<(&Transform, &Building), Added<Building>>,
    standing: Query<&Building>,
    piles: Query<(Entity, &StorePile), Without<Rehouse>>,
) {
    let Some(store) = site
        .as_ref()
        .and_then(|site| stores.get(site.settlement).ok())
    else {
        return;
    };
    for (at, building) in &new_buildings {
        match building.kind {
            BuildingKind::Storehouse => {
                let granary_stands = standing.iter().any(|b| b.kind == BuildingKind::Granary);
                for (pile, kind) in &piles {
                    let (local, goal) = match kind.0 {
                        PileKind::Timber => {
                            (Vec3::new(-0.9, 0.0, 0.5), store.timber.min(24.0) as u8)
                        }
                        PileKind::Stone => (Vec3::new(0.9, 0.0, -0.5), store.stone.min(12.0) as u8),
                        // Food shelters here too, until a granary stands.
                        PileKind::Food if !granary_stands => (
                            Vec3::new(0.0, 0.0, 0.9),
                            ((store.food().min(24.0) / 2.0).ceil() as u8).max(1),
                        ),
                        PileKind::Food => continue,
                    };
                    commands.entity(pile).insert(Rehouse {
                        to: at.translation + at.rotation * local,
                        to_rot: at.rotation,
                        hauled: 0,
                        goal: goal.max(1),
                    });
                }
                notices.write(crate::ui::Notice::new(
                    "The village begins carrying its stores in under the storehouse roof",
                ));
            }
            BuildingKind::Granary => {
                for (pile, kind) in &piles {
                    if kind.0 != PileKind::Food {
                        continue;
                    }
                    commands.entity(pile).insert(Rehouse {
                        to: at.translation + at.rotation * Vec3::new(0.0, 0.0, 0.4),
                        to_rot: at.rotation,
                        hauled: 0,
                        goal: ((store.food().min(24.0) / 2.0).ceil() as u8).max(1),
                    });
                }
                notices.write(crate::ui::Notice::new(
                    "The harvest is being carried into the granary",
                ));
            }
            _ => {}
        }
    }
}

/// Walks each armload across: recruit an idle carrier, load at the old
/// pile, set down at the new spot, repeat until the pile itself follows.
#[allow(clippy::type_complexity)]
pub(super) fn rehouse_stores(
    mut commands: Commands,
    mut site: Option<ResMut<SettlementSite>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    children: Query<&Children>,
    loads: Query<Entity, With<WoodLoad>>,
    mut piles: Query<(Entity, &StorePile, &mut Transform, &mut Rehouse)>,
    mut carriers: Query<
        (
            Entity,
            &Transform,
            &mut Activity,
            &mut MoveTarget,
            Option<&RehouseHauler>,
            Has<RehouseLoad>,
        ),
        (
            With<Villager>,
            Without<crate::creature::Corpse>,
            Without<Held>,
            Without<Airborne>,
            Without<Rehouse>,
        ),
    >,
) {
    for (pile, kind, mut pile_at, mut rehouse) in &mut piles {
        // Finished: the pile stands at its new spot; the fetch point follows.
        if rehouse.hauled >= rehouse.goal {
            pile_at.translation = rehouse.to;
            pile_at.rotation = rehouse.to_rot;
            if kind.0 == PileKind::Timber
                && let Some(site) = site.as_mut()
            {
                site.woodpile = rehouse.to;
            }
            commands.entity(pile).remove::<Rehouse>();
            for (carrier, _, mut activity, mut target, hauler, _) in &mut carriers {
                if hauler.is_some_and(|h| h.0 == pile) {
                    commands
                        .entity(carrier)
                        .remove::<(RehouseHauler, RehouseLoad)>();
                    shed_wood(&mut commands, carrier, &children, &loads);
                    *activity = Activity::Idle;
                    target.0 = None;
                }
            }
            continue;
        }

        // A carrier on the job walks the loop; if none, recruit one.
        let mut have_carrier = false;
        for (carrier, at, activity, mut target, hauler, loaded) in &mut carriers {
            if !hauler.is_some_and(|h| h.0 == pile) {
                continue;
            }
            have_carrier = true;
            if *activity != Activity::Hauling {
                // Pulled away by hunger or night; release the post.
                commands
                    .entity(carrier)
                    .remove::<(RehouseHauler, RehouseLoad)>();
                shed_wood(&mut commands, carrier, &children, &loads);
                continue;
            }
            if !loaded {
                if at.translation.distance(pile_at.translation) > 2.2 {
                    target.0 = Some(pile_at.translation);
                } else {
                    match kind.0 {
                        PileKind::Timber => {
                            shoulder_wood(&mut commands, &mut meshes, &mut materials, carrier)
                        }
                        PileKind::Stone => {
                            shoulder_stone(&mut commands, &mut meshes, &mut materials, carrier)
                        }
                        PileKind::Food => {
                            shoulder_sack(&mut commands, &mut meshes, &mut materials, carrier)
                        }
                    }
                    commands.entity(carrier).insert(RehouseLoad);
                }
            } else if at.translation.distance(rehouse.to) > 2.2 {
                target.0 = Some(rehouse.to);
            } else {
                rehouse.hauled += 1;
                shed_wood(&mut commands, carrier, &children, &loads);
                commands.entity(carrier).remove::<RehouseLoad>();
            }
        }
        if !have_carrier {
            for (carrier, _, mut activity, _, hauler, _) in &mut carriers {
                if hauler.is_some() {
                    continue;
                }
                if matches!(*activity, Activity::Idle | Activity::Wandering) {
                    *activity = Activity::Hauling;
                    commands.entity(carrier).insert(RehouseHauler(pile));
                    break;
                }
            }
        }
    }
}

/// The stone and food stores, countable at a glance like the woodpile.
pub(super) fn update_store_piles(
    site: Option<Res<SettlementSite>>,
    stores: Query<&Stockpile>,
    mut blocks: Query<(&StonePileBlock, &mut Visibility), Without<FoodSack>>,
    mut sacks: Query<(&FoodSack, &mut Visibility), Without<StonePileBlock>>,
) {
    let Some(site) = site else {
        return;
    };
    let Ok(store) = stores.get(site.settlement) else {
        return;
    };
    for (block, mut visibility) in &mut blocks {
        let wanted = if (block.0 as f32) < store.stone.min(12.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
    for (sack, mut visibility) in &mut sacks {
        // Two food to the sack, or the pile would dwarf the village.
        let wanted = if (sack.0 as f32) * 2.0 < store.food().min(24.0) {
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
            Option<&super::traits::Traits>,
        ),
        (
            With<Villager>,
            Without<Corpse>,
            Without<Held>,
            Without<Airborne>,
            Without<crate::creature::Childhood>,
        ),
    >,
) {
    let dt = time.delta_secs();
    if !is_work_hour(clock.time_of_day()) {
        return;
    }

    // Helpers at work: walk in, steady the frame.
    for (villager, at, mut activity, mut target, mut motion, helper, _) in &mut folk {
        let Some(helper) = helper else {
            continue;
        };
        let done = sites.get_mut(helper.0).is_err();
        if done || *activity != Activity::Working {
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
            let target_stage = stage_for(construction.progress, cost);
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
        if construction.stone_laid < plan.kind.stone_cost() {
            continue;
        }
        let already = helpers_now.iter().filter(|h| h.0 == site).count();
        if already >= 2 {
            continue;
        }
        for (villager, at, mut activity, _, _, helper, manner) in &mut folk {
            if helper.is_some()
                || !matches!(*activity, Activity::Idle | Activity::Wandering)
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
    site: Option<Res<SettlementSite>>,
    mut stores: Query<&mut Stockpile>,
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
            Without<Airborne>,
        ),
    >,
) {
    let Some(site) = site else {
        return;
    };
    let Ok(mut store) = stores.get_mut(site.settlement) else {
        return;
    };

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
                // Log gone (or already shouldered): walk it home.
                if at.translation.distance(site.woodpile) > 2.2 {
                    target.0 = Some(site.woodpile);
                } else {
                    store.timber += 2.0;
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
        if log_at.translation.distance(site.centre) > 70.0 {
            continue;
        }
        let volunteer = villagers
            .iter_mut()
            .filter(|(_, _, activity, _, hauler, _)| {
                hauler.is_none() && matches!(**activity, Activity::Idle | Activity::Wandering)
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

/// Whether a build site is a house, for the personal-stake speedup.
fn plan_kind_is_house(
    build_sites: &Query<(&mut ConstructionSite, &Blueprint)>,
    site: Entity,
) -> bool {
    build_sites
        .get(site)
        .is_ok_and(|(_, plan)| plan.kind == BuildingKind::House)
}

/// A worksite this worker gave up on reaching. Without this, the nearest tree
/// across a river is chosen, abandoned, and chosen again, forever — the whole
/// profession stuck on one impossible errand.
#[derive(Component, Debug)]
pub struct Shunned {
    pub site: Vec3,
    pub remaining: f32,
}

/// How close to a shunned site a new job offer has to be to be refused too.
const SHUN_RADIUS: f32 = 5.0;

/// Grudges against unreachable worksites fade.
pub(super) fn forget_shunned(
    mut commands: Commands,
    time: Res<Time>,
    mut shunned: Query<(Entity, &mut Shunned)>,
) {
    for (entity, mut shun) in &mut shunned {
        shun.remaining -= time.delta_secs();
        if shun.remaining <= 0.0 {
            commands.entity(entity).remove::<Shunned>();
        }
    }
}

/// Adults take up a calling.
pub(super) fn assign_vocations(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut rng: Option<ResMut<SimRng>>,
    mut grown: Query<
        (
            Entity,
            &CreatureGenome,
            &Person,
            &crate::witness::Temperament,
            Option<&mut Chronicle>,
        ),
        (With<Villager>, Without<Vocation>, Without<Corpse>),
    >,
) {
    let Some(rng) = rng.as_mut() else {
        return;
    };

    for (entity, genome, person, temperament, chronicle) in &mut grown {
        if genome.age != Age::Adult {
            continue;
        }
        let vocation = roll_vocation(temperament.boldness, &mut rng.0);
        commands.entity(entity).insert(vocation);
        info!("{} {}", person.name, vocation.taking_up());
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), vocation.taking_up());
        }
    }
}

/// The village calls people into the trades it lacks.
///
/// A tavern with no cook, a shrine with no keeper, a bruised village with no
/// healer: when demand appears, someone from the crowded trades sets their
/// tools down and takes up new ones — and their chronicle records the turn.
pub(super) fn retrain(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    clock: Res<crate::calendar::WorldClock>,
    buildings: Query<&Building>,
    sites: Query<(&ConstructionSite, &Blueprint)>,
    stores: Query<&Stockpile>,
    homeless: Query<
        (),
        (
            With<Villager>,
            Without<crate::villager::home::Home>,
            Without<crate::creature::Childhood>,
            Without<Corpse>,
        ),
    >,
    mut notices: MessageWriter<crate::ui::Notice>,
    hurt: Query<&Vitality, (With<Villager>, Without<Corpse>)>,
    known: Option<Res<super::explore::KnownWorld>>,
    trees: Query<(&GlobalTransform, &crate::scatter::FellableTree)>,
    terrain: Option<Res<Terrain>>,
    site: Option<Res<SettlementSite>>,
    wild: Query<
        (&Transform, &crate::creature::genome::CreatureGenome),
        (With<crate::creature::wildlife::Wild>, Without<Corpse>),
    >,
    mut workers: Query<
        (Entity, &Vocation, &Person, Option<&mut Chronicle>),
        (With<Villager>, Without<Corpse>),
    >,
) {
    // A standing audit, not a frame-by-frame twitch: the village looks at
    // itself every little while and reallocates one pair of hands at most.
    *since_last += time.delta_secs();
    if *since_last < 20.0 {
        return;
    }
    *since_last = 0.0;

    let has_building = |kind: BuildingKind| buildings.iter().any(|b| b.kind == kind);
    let count_of = |v: Vocation| workers.iter().filter(|(_, w, _, _)| **w == v).count();
    let has_vocation = |v: Vocation| count_of(v) > 0;
    let timber_low = stores.iter().next().is_none_or(|s| s.timber < 6.0);
    // Whether any fellable tree stands on ground the village knows. When
    // none does, more foresters are useless — someone has to go find woods.
    let wood_known = known.as_ref().is_none_or(|k| {
        trees
            .iter()
            .any(|(at, tree)| tree.harvestable() && k.knows(at.translation()))
    });

    // The village's needs, in the order they kill. Every want here is a
    // deadlock somewhere else: no woodcutter starves the fire and every
    // build; no carpenter leaves the homeless in the rain; and so on.
    // Hunger kills first, so food hands top the ladder: a thin larder
    // with too few of the trades that fill it retrains someone toward
    // the water or the bushes before anything else gets a say.
    let food_low = stores.iter().next().is_none_or(|s| s.food() < 12.0);
    let food_hands: usize = [
        Vocation::Fisher,
        Vocation::Gatherer,
        Vocation::Hunter,
        Vocation::Farmer,
    ]
    .into_iter()
    .map(|v| count_of(v))
    .sum();
    let mouths = workers.iter().count();
    // Whether water lies within a working walk of the square — twelve
    // spokes, no dart-throwing, so no rng is needed here.
    let shore_near = terrain.as_ref().zip(site.as_ref()).is_some_and(|(t, s)| {
        (0..12).any(|i| {
            let angle = i as f32 / 12.0 * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            (1..=14).any(|step| {
                let d = step as f32 * 4.0;
                t.height_at(s.centre.x + cos * d, s.centre.z + sin * d) <= WATER_LEVEL
            })
        })
    });
    // Wolves pressing the village, or a manned post waiting for its
    // man: danger calls a guard the way hunger calls a fisher.
    let wolves_near = site.as_ref().map_or(0, |s| {
        wild.iter()
            .filter(|(at, genome)| {
                genome.species == crate::creature::genome::Species::Wolf
                    && at.translation.distance(s.centre) < 130.0
            })
            .count()
    });
    let mut wanted: Option<Vocation> = None;
    if food_low && food_hands * 5 < mouths.max(4) {
        wanted = Some(if shore_near {
            Vocation::Fisher
        } else {
            Vocation::Gatherer
        });
    } else if !has_vocation(Vocation::Guard)
        && (wolves_near >= 2 || has_building(BuildingKind::Watchtower))
    {
        wanted = Some(Vocation::Guard);
    } else if timber_low && !wood_known && !has_vocation(Vocation::Explorer) {
        wanted = Some(Vocation::Explorer);
    } else if !has_vocation(Vocation::Forester) && timber_low && wood_known {
        wanted = Some(Vocation::Forester);
    } else if has_building(BuildingKind::Dock) && !has_vocation(Vocation::Fisher) {
        wanted = Some(Vocation::Fisher);
    } else if !has_vocation(Vocation::Carpenter)
        && (!sites.is_empty() || homeless.iter().count() > 2)
    {
        wanted = Some(Vocation::Carpenter);
    } else if !has_vocation(Vocation::Mason)
        && sites
            .iter()
            .any(|(cs, plan)| cs.stone_laid < plan.kind.stone_cost())
    {
        wanted = Some(Vocation::Mason);
    } else if has_building(BuildingKind::Tavern) && !has_vocation(Vocation::Cook) {
        wanted = Some(Vocation::Cook);
    } else if has_building(BuildingKind::Shrine) && !has_vocation(Vocation::Priest) {
        wanted = Some(Vocation::Priest);
    } else if !has_vocation(Vocation::Healer) && hurt.iter().filter(|v| v.harm > 0.15).count() >= 2
    {
        wanted = Some(Vocation::Healer);
    }
    let Some(wanted) = wanted else {
        return;
    };

    // The most crowded trade gives someone up — and no trade gives up its
    // last pair of hands. The village rebalances itself by need.
    let mut counts: Vec<(Vocation, usize)> = [
        Vocation::Gatherer,
        Vocation::Fisher,
        Vocation::Hunter,
        Vocation::Miner,
        Vocation::Forester,
        Vocation::Carpenter,
        Vocation::Farmer,
        Vocation::Mason,
    ]
    .into_iter()
    .map(|v| (v, count_of(v)))
    .collect();
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let Some((give, _)) = counts.into_iter().find(|(v, n)| *v != wanted && *n >= 2) else {
        return;
    };
    let donor = workers.iter_mut().find(|(_, v, _, _)| **v == give);
    let Some((entity, old, person, chronicle)) = donor else {
        return;
    };
    let old = *old;

    commands.entity(entity).insert(wanted);
    info!(
        "{} set down their tools and {}",
        person.name,
        wanted.taking_up()
    );
    notices.write(crate::ui::Notice::new(format!(
        "{} {} - the village needed it",
        person.name,
        wanted.taking_up()
    )));
    if let Some(mut chronicle) = chronicle {
        chronicle.record(
            clock.day(),
            format!(
                "set down the tools of one who {} and {}",
                old.describe(),
                wanted.taking_up()
            ),
        );
    }
}

/// Finds the nearest shoreline point: walk outward from the settlement until
/// the ground dips under water, and stand on the last dry step.
fn find_shore(terrain: &Terrain, centre: Vec3, rng: &mut Rng) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for _ in 0..24 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let (sin, cos) = angle.sin_cos();
        let mut last_dry: Option<Vec3> = None;
        let mut step = 4.0;
        while step < WORK_REACH {
            let x = centre.x + cos * step;
            let z = centre.z + sin * step;
            let height = terrain.height_at(x, z);
            if height <= WATER_LEVEL || terrain.river_surface_at(x, z).is_some() {
                if let Some(dry) = last_dry
                    && best.is_none_or(|(d, _)| step < d)
                {
                    best = Some((step, dry));
                }
                break;
            }
            if terrain.is_walkable(x, z) {
                last_dry = Some(Vec3::new(x, height, z));
            }
            step += 4.0;
        }
    }
    best.map(|(_, at)| at)
}

/// Finds the nearest ground matching a predicate, by throwing darts around the
/// settlement and keeping the closest hit. Commutes are dead time; the fun is
/// at the worksite, not on the road to it.
fn find_ground(
    terrain: &Terrain,
    centre: Vec3,
    rng: &mut Rng,
    wanted: impl Fn(&Terrain, f32, f32) -> bool,
) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for _ in 0..40 {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let distance = rng.range(10.0, WORK_REACH);
        let (sin, cos) = angle.sin_cos();
        let x = centre.x + cos * distance;
        let z = centre.z + sin * distance;
        if terrain.is_walkable(x, z)
            && wanted(terrain, x, z)
            && best.is_none_or(|(d, _)| distance < d)
        {
            best = Some((distance, Vec3::new(x, terrain.height_at(x, z), z)));
        }
    }
    best.map(|(_, at)| at)
}

/// The idle and able take up the day's work.
pub(super) fn take_up_work(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    terrain: Res<Terrain>,
    known: Option<Res<super::explore::KnownWorld>>,
    site: Option<Res<SettlementSite>>,
    mut rng: ResMut<SimRng>,
    mut workers: Query<
        (
            Entity,
            &Transform,
            &Needs,
            &Vocation,
            &mut Activity,
            Option<&Shunned>,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
    bushes: Query<(Entity, &GlobalTransform, &FoodSource), Without<Villager>>,
    build_sites: Query<(Entity, &Transform, &ConstructionSite, &Blueprint)>,
    trees: Query<(Entity, &GlobalTransform, &crate::scatter::FellableTree)>,
    boulders: Query<(Entity, &GlobalTransform), With<crate::matter::Boulder>>,
    town: (
        Query<(Entity, &GlobalTransform, &Building)>,
        Query<(Entity, &Transform, &Field)>,
        Query<(Entity, &Transform, &Vitality), (With<Villager>, Without<Corpse>)>,
        Query<(Entity, &GlobalTransform, &crate::matter::Deposit)>,
        Query<(Entity, &GlobalTransform, &crate::scatter::SacredFlora)>,
    ),
    stores: Query<&Stockpile>,
    game: Query<
        (Entity, &Transform, &CreatureGenome),
        (
            With<Creature>,
            Without<Villager>,
            Without<Corpse>,
            Without<Held>,
        ),
    >,
    carcasses: Query<
        (Entity, &Transform, &CreatureGenome),
        (
            With<Creature>,
            With<Corpse>,
            Without<Villager>,
            Without<Held>,
        ),
    >,
) {
    let Some(site) = site else {
        return;
    };
    if !is_work_hour(clock.time_of_day()) {
        return;
    }
    let (buildings, fields, patients, deposits, sacred) = town;

    for (entity, transform, needs, vocation, mut activity, shunned) in &mut workers {
        if !matches!(*activity, Activity::Idle | Activity::Wandering) {
            continue;
        }
        // Hunger sends most trades off to find a meal — but the trades
        // that MAKE food work through it, or the village deadlocks: a
        // fisher too hungry to fish is how everyone starves beside an
        // empty larder. The food trades eat from their own yield instead.
        let feeds_the_village = matches!(
            vocation,
            Vocation::Fisher | Vocation::Gatherer | Vocation::Hunter | Vocation::Farmer
        );
        if needs.hunger > HUNGRY_THRESHOLD && !feeds_the_village {
            continue;
        }
        // The exhausted do not show up. Sleep is the cure, and the fire or a
        // roof is where sleep lives.
        if needs.rest > 0.9 {
            continue;
        }

        // A recently unreachable worksite is off the table; anything near it too.
        let permitted = |at: Vec3| shunned.is_none_or(|shun| shun.site.distance(at) > SHUN_RADIUS);
        // Ground an explorer has brought home counts as workable even far
        // out: the pockets are why expeditions matter to the trades.
        let known_far = |at: Vec3, d: f32| d < 700.0 && known.as_ref().is_some_and(|k| k.knows(at));

        let job = match vocation {
            // Gatherers fill the larder first; with food put by, they go
            // after the rarer gifts — incense herb and dyeflowers.
            Vocation::Gatherer => {
                let food_job = bushes
                    .iter()
                    .filter(|(_, _, source)| source.amount > 0.5)
                    .map(|(bush, bush_transform, _)| {
                        (
                            bush,
                            bush_transform.translation(),
                            bush_transform.translation().distance(transform.translation),
                        )
                    })
                    .filter(|(_, at, d)| (*d < WORK_REACH || known_far(*at, *d)) && permitted(*at))
                    .min_by(|a, b| a.2.total_cmp(&b.2))
                    .map(|(bush, at, d)| Job::at(at, Some(bush), d));
                let sacred_job = || {
                    sacred
                        .iter()
                        .filter(|(_, _, flora)| flora.amount > 0.5)
                        .map(|(stand, stand_transform, _)| {
                            (
                                stand,
                                stand_transform.translation(),
                                stand_transform
                                    .translation()
                                    .distance(transform.translation),
                            )
                        })
                        .filter(|(_, at, d)| {
                            (*d < WORK_REACH || known_far(*at, *d)) && permitted(*at)
                        })
                        .min_by(|a, b| a.2.total_cmp(&b.2))
                        .map(|(stand, at, d)| Job::at(at, Some(stand), d))
                };
                if stores.get(site.settlement).is_ok_and(|s| s.food() >= 25.0) {
                    sacred_job().or(food_job)
                } else {
                    food_job.or_else(sacred_job)
                }
            }

            // A carcass already down is free meat: harvest before hunting,
            // and the village stops drowning in carrion.
            Vocation::Hunter => carcasses
                .iter()
                .filter(|(_, _, genome)| genome.species != Species::Human)
                .map(|(kill, kill_transform, _)| {
                    (
                        kill,
                        kill_transform.translation,
                        kill_transform.translation.distance(transform.translation),
                    )
                })
                .filter(|(_, at, d)| {
                    (*d < WORK_REACH * 1.6 || known_far(*at, *d)) && permitted(*at)
                })
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(kill, at, d)| Job::at(at, Some(kill), d))
                .or_else(|| {
                    game.iter()
                        .filter(|(_, _, genome)| genome.species != Species::Human)
                        .map(|(prey, prey_transform, _)| {
                            (
                                prey,
                                prey_transform.translation,
                                prey_transform.translation.distance(transform.translation),
                            )
                        })
                        .filter(|(_, at, d)| {
                            (*d < WORK_REACH * 1.6 || known_far(*at, *d)) && permitted(*at)
                        })
                        .min_by(|a, b| a.2.total_cmp(&b.2))
                        .map(|(prey, at, d)| Job::at(at, Some(prey), d))
                }),

            // The dock is the fisher's post when one stands; the bare
            // shore otherwise.
            Vocation::Fisher => buildings
                .iter()
                .find(|(_, _, b)| b.kind == BuildingKind::Dock)
                .map(|(dock, dock_at, _)| {
                    let at = dock_at.translation();
                    Job::at(at, Some(dock), at.distance(transform.translation))
                })
                .or_else(|| {
                    find_shore(&terrain, site.centre, &mut rng.0)
                        .filter(|at| permitted(*at))
                        .map(|at| Job::at(at, None, at.distance(transform.translation)))
                }),

            // Miners feed two hungers: the stone every foundation wants,
            // and the ore the blacksmith's fire waits on. Stone while the
            // pile runs thin; the far vein once the village can spare the
            // walk.
            Vocation::Miner => {
                let stone_job = boulders
                    .iter()
                    .map(|(rock, rock_transform)| {
                        (
                            rock,
                            rock_transform.translation(),
                            rock_transform.translation().distance(transform.translation),
                        )
                    })
                    .filter(|(_, at, d)| (*d < WORK_REACH || known_far(*at, *d)) && permitted(*at))
                    .min_by(|a, b| a.2.total_cmp(&b.2))
                    .map(|(rock, at, d)| Job::at(at, Some(rock), d))
                    .or_else(|| {
                        find_ground(&terrain, site.centre, &mut rng.0, |t, x, z| {
                            matches!(t.biome_at(x, z), Biome::Alpine)
                                || t.height_at(x, z) > WATER_LEVEL + 40.0
                        })
                        .filter(|at| permitted(*at))
                        .map(|at| Job::at(at, None, at.distance(transform.translation)))
                    });
                let ore_job = deposits
                    .iter()
                    .filter(|(_, _, deposit)| {
                        deposit.kind == crate::matter::DepositKind::Iron && deposit.amount > 0.5
                    })
                    .map(|(vein, vein_transform, _)| {
                        (
                            vein,
                            vein_transform.translation(),
                            vein_transform.translation().distance(transform.translation),
                        )
                    })
                    .filter(|(_, at, d)| (*d < WORK_REACH || known_far(*at, *d)) && permitted(*at))
                    .min_by(|a, b| a.2.total_cmp(&b.2))
                    .map(|(vein, at, d)| Job::at(at, Some(vein), d));
                if stores.get(site.settlement).is_ok_and(|s| s.stone >= 12.0) {
                    ore_job.or(stone_job)
                } else {
                    stone_job.or(ore_job)
                }
            }

            // Foresters fell real trees, and only real trees — where none
            // stand on known ground, the want goes unmet until an explorer
            // brings home a wood. Timber that appears from nowhere would
            // let the village stay home forever, and staying home is death.
            Vocation::Forester => trees
                .iter()
                .filter(|(_, _, tree)| tree.harvestable())
                .map(|(tree, tree_transform, _)| {
                    (
                        tree,
                        tree_transform.translation(),
                        tree_transform.translation().distance(transform.translation),
                    )
                })
                .filter(|(_, at, d)| (*d < WORK_REACH || known_far(*at, *d)) && permitted(*at))
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(tree, at, d)| Job::at(at, Some(tree), d)),

            // Carpenters go where ground is broken — if there is timber to work.
            Vocation::Carpenter => {
                if stores.get(site.settlement).is_ok_and(|s| s.timber >= 1.0) {
                    build_sites
                        .iter()
                        .filter(|(_, _, cs, plan)| cs.stone_laid >= plan.kind.stone_cost())
                        .map(|(house, house_transform, ..)| {
                            (
                                house,
                                house_transform.translation,
                                house_transform.translation.distance(transform.translation),
                            )
                        })
                        .min_by(|a, b| a.2.total_cmp(&b.2))
                        .map(|(house, at, d)| Job::at(at, Some(house), d))
                } else {
                    None
                }
            }

            // Masons serve any site whose foundation still wants stone.
            Vocation::Mason => {
                let laying = if stores
                    .get(site.settlement)
                    .is_ok_and(|s| s.stone >= 1.0 || s.clay >= 1.0)
                {
                    build_sites
                        .iter()
                        .filter(|(_, _, cs, plan)| cs.stone_laid < plan.kind.stone_cost())
                        .map(|(b, t, ..)| {
                            (
                                b,
                                t.translation,
                                t.translation.distance(transform.translation),
                            )
                        })
                        .min_by(|a, b| a.2.total_cmp(&b.2))
                        .map(|(b, at, d)| Job::at(at, Some(b), d))
                } else {
                    None
                };
                // No foundations to lay: cut stone like a miner — or, when
                // the clay store is thin, dig the red bank instead.
                laying
                    .or_else(|| {
                        boulders
                            .iter()
                            .map(|(rock, rock_transform)| {
                                (
                                    rock,
                                    rock_transform.translation(),
                                    rock_transform.translation().distance(transform.translation),
                                )
                            })
                            .filter(|(_, at, d)| {
                                (*d < WORK_REACH || known_far(*at, *d)) && permitted(*at)
                            })
                            .min_by(|a, b| a.2.total_cmp(&b.2))
                            .map(|(rock, at, d)| Job::at(at, Some(rock), d))
                    })
                    .or_else(|| {
                        if stores.get(site.settlement).is_ok_and(|s| s.clay >= 14.0) {
                            return None;
                        }
                        deposits
                            .iter()
                            .filter(|(_, _, deposit)| {
                                deposit.kind == crate::matter::DepositKind::Clay
                                    && deposit.amount > 0.5
                            })
                            .map(|(bank, bank_transform, _)| {
                                (
                                    bank,
                                    bank_transform.translation(),
                                    bank_transform.translation().distance(transform.translation),
                                )
                            })
                            .filter(|(_, at, d)| {
                                (*d < WORK_REACH || known_far(*at, *d)) && permitted(*at)
                            })
                            .min_by(|a, b| a.2.total_cmp(&b.2))
                            .map(|(bank, at, d)| Job::at(at, Some(bank), d))
                    })
            }

            // Farmers work their own field, tilling a new one if they lack it.
            Vocation::Farmer => {
                let mine = fields
                    .iter()
                    .find(|(_, _, f)| f.farmer == entity)
                    .map(|(f, t, _)| (f, t.translation));
                match mine {
                    Some((field, at)) => {
                        Some(Job::at(at, Some(field), at.distance(transform.translation)))
                    }
                    // A new plot: flat, dry, out of everyone's way. The Job
                    // carries no focus; arriving farmers till on the spot.
                    None => village_slots(site.centre, 5..7)
                        .into_iter()
                        .map(|(x, z, _)| Vec3::new(x, terrain.height_at(x, z), z))
                        .find(|at| {
                            terrain.is_walkable(at.x, at.z)
                                && at.y > WATER_LEVEL + 2.0
                                && !matches!(
                                    terrain.biome_at(at.x, at.z),
                                    Biome::Alpine | Biome::Arid
                                )
                                && fields
                                    .iter()
                                    .all(|(_, t, _)| t.translation.distance(*at) > 7.0)
                                && trees
                                    .iter()
                                    .all(|(_, t, _)| t.translation().distance(*at) > 3.5)
                                && permitted(*at)
                        })
                        .map(|at| Job::at(at, None, at.distance(transform.translation))),
                }
            }

            // The cook's post is the tavern kitchen.
            Vocation::Cook => buildings
                .iter()
                .find(|(_, _, b)| b.kind == BuildingKind::Tavern)
                .map(|(tavern, at, _)| {
                    let at = at.translation();
                    Job::at(at, Some(tavern), at.distance(transform.translation))
                }),

            // The healer goes to whoever is worst hurt.
            Vocation::Healer => patients
                .iter()
                .filter(|(patient, _, vitality)| *patient != entity && vitality.harm > 0.15)
                .max_by(|a, b| a.2.harm.total_cmp(&b.2.harm))
                .map(|(patient, at, _)| {
                    Job::at(
                        at.translation,
                        Some(patient),
                        at.translation.distance(transform.translation),
                    )
                }),

            // Explorers muster on their own; the expedition system owns them.
            Vocation::Explorer => None,

            // The priest's post is the shrine.
            Vocation::Priest => buildings
                .iter()
                .find(|(_, _, b)| b.kind == BuildingKind::Shrine)
                .map(|(shrine, at, _)| {
                    let at = at.translation();
                    Job::at(at, Some(shrine), at.distance(transform.translation))
                }),

            // A guard's post is the tower if one stands, the village edge
            // otherwise; the walking of it is the work.
            Vocation::Guard => {
                let post = buildings
                    .iter()
                    .find(|(_, _, b)| b.kind == BuildingKind::Watchtower)
                    .map(|(_, at, _)| at.translation())
                    .unwrap_or_else(|| {
                        let angle = rng.0.range(0.0, std::f32::consts::TAU);
                        let (sin, cos) = angle.sin_cos();
                        let (x, z) = (site.centre.x + cos * 22.0, site.centre.z + sin * 22.0);
                        Vec3::new(x, terrain.height_at(x, z), z)
                    });
                Some(Job::at(post, None, post.distance(transform.translation)))
            }
        };

        if let Some(job) = job {
            *activity = Activity::Working;
            commands.entity(entity).insert(job);
        }
    }
}

/// Work gets done: walk there, do the thing, and the stockpile grows.
pub(super) fn do_work(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut rng: ResMut<SimRng>,
    mut stores: Query<&mut Stockpile>,
    site: Option<Res<SettlementSite>>,
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
            ),
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
    mut bushes: Query<&mut FoodSource, Without<Villager>>,
    mut trees: Query<&mut crate::scatter::FellableTree>,
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
        Query<&mut Field>,
        ResMut<KitchenWarm>,
        Query<&mut crate::matter::Deposit>,
        Query<&mut crate::scatter::SacredFlora>,
    ),
    civic: (
        Query<(&mut ConstructionSite, &Blueprint)>,
        Query<&super::Settlement>,
        MessageWriter<crate::ui::Notice>,
    ),
    ground: (
        Res<Terrain>,
        ResMut<crate::terrain::LoadedChunks>,
        ResMut<crate::grass::GrassChunks>,
        Option<Res<crate::weather::Weather>>,
    ),
    assets: (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut prey_query: Query<
        (
            &Transform,
            &mut Vitality,
            &mut CreatureMotion,
            Has<Corpse>,
            &CreatureGenome,
        ),
        (With<Creature>, Without<Villager>),
    >,
) {
    let dt = time.delta_secs();
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
    let (mut build_sites, settlements, mut notices) = civic;
    let (terrain, mut chunks, mut grass, weather) = ground;
    let (mut meshes, mut materials) = assets;
    let Some(site) = site else {
        return;
    };
    let Ok(mut store) = stores.get_mut(site.settlement) else {
        return;
    };

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
        (mut chronicle, manner, home),
    ) in &mut workers
    {
        if *activity != Activity::Working {
            commands.entity(entity).remove::<Job>();
            continue;
        }

        // Shifts end: at day's end, or when hunger calls. The food trades
        // do not down tools for hunger — their meal is at the worksite.
        let feeds_the_village = matches!(
            vocation,
            Vocation::Fisher | Vocation::Gatherer | Vocation::Hunter | Vocation::Farmer
        );
        if !is_work_hour(clock.time_of_day())
            || (needs.hunger > DOWN_TOOLS_HUNGER && !feeds_the_village)
        {
            *activity = Activity::Idle;
            target.0 = None;
            commands.entity(entity).remove::<Job>();
            continue;
        }

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
                        for (wolf_t, mut vitality, mut wolf_motion, is_corpse, genome) in
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
                            wolf_motion.flail = 1.0;
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
            // No wolves in sight: walk the round. A new leg of the patrol
            // whenever the last one is done.
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
                    let (x, z) = (site.centre.x + cos * reach, site.centre.z + sin * reach);
                    job.site = Vec3::new(x, 0.0, z);
                }
            }
            continue;
        }

        // Carpenters run a fetch-and-carry loop: to the pile for a log, log to
        // the site, hammer it in, and back for the next — every step walked.
        if *vocation == Vocation::Carpenter {
            let Some(house) = job.focus else {
                *activity = Activity::Idle;
                commands.entity(entity).remove::<Job>();
                continue;
            };
            if build_sites.get(house).is_err() {
                // Finished under someone else's hammer.
                *activity = Activity::Idle;
                commands.entity(entity).remove::<Job>();
                continue;
            }

            if carrying.get(entity).is_err() {
                // Empty-handed: fetch from the pile.
                if store.timber < 1.0 {
                    *activity = Activity::Idle;
                    commands.entity(entity).remove::<Job>();
                    continue;
                }
                if transform.translation.distance(site.woodpile) > 2.6 {
                    target.0 = Some(site.woodpile);
                    job.patience -= dt;
                    if job.patience <= 0.0 {
                        *activity = Activity::Idle;
                        target.0 = None;
                        commands.entity(entity).remove::<Job>().insert(Shunned {
                            site: site.woodpile,
                            remaining: 90.0,
                        });
                    }
                    continue;
                }
                store.timber -= 1.0;
                commands.entity(entity).insert(CarryingWood { amount: 1.0 });
                shoulder_wood(&mut commands, &mut meshes, &mut materials, entity);
                job.patience = 20.0 + job.site.distance(transform.translation) * 0.8;
                continue;
            }

            // Loaded: to the site.
            if transform.translation.distance(job.site) > WORK_RANGE {
                target.0 = Some(job.site);
                job.patience -= dt;
                if job.patience <= 0.0 {
                    // Give the log back rather than stranding it in their arms.
                    store.timber += 1.0;
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
            let stake = if plan_kind_is_house(&build_sites, house) && home.is_none() {
                1.4
            } else {
                1.0
            };
            job.progress += dt * stake;
            if job.progress < 3.5 {
                continue;
            }
            job.progress = 0.0;

            commands.entity(entity).remove::<CarryingWood>();
            shed_wood(&mut commands, entity, &children, &loads);
            let Ok((mut construction, plan)) = build_sites.get_mut(house) else {
                continue;
            };
            construction.progress += 1.0;
            // Stages land at thirds of the build, whatever the kind's cost.
            let cost = plan.kind.timber_cost();
            let target_stage = stage_for(construction.progress, cost);
            while construction.stage < target_stage {
                construction.stage += 1;
                raise_stage(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    house,
                    construction.stage,
                    plan,
                );
            }
            if construction.progress >= cost {
                let kind = plan.kind;
                let mut done = commands.entity(house);
                done.remove::<ConstructionSite>()
                    .insert((Building { kind }, Name::new(kind.name())));
                if kind == BuildingKind::House {
                    done.insert(Hut);
                }
                let home = settlements
                    .get(site.settlement)
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

        // Masons run the same fetch-and-carry loop as carpenters, in stone.
        if *vocation == Vocation::Mason
            && let Some(site_entity) = job.focus
            && build_sites.get(site_entity).is_ok()
        {
            let Ok((mut construction, plan)) = build_sites.get_mut(site_entity) else {
                continue;
            };
            if construction.stone_laid >= plan.kind.stone_cost() {
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
                if transform.translation.distance(site.woodpile) > 2.6 {
                    target.0 = Some(site.woodpile);
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
            // The block lands where it was laid: courses appear around the
            // perimeter, corners first.
            {
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
            if construction.stone_laid >= plan.kind.stone_cost() {
                // The foundation shows itself the moment the last block lands.
                let slab = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
                let stone_material = materials.add(StandardMaterial {
                    base_color: crate::palette::shade(&crate::palette::STONE, 0.4),
                    perceptual_roughness: 1.0,
                    ..default()
                });
                commands.spawn((
                    Mesh3d(slab),
                    MeshMaterial3d(stone_material.clone()),
                    Transform::from_xyz(0.0, PLINTH_TOP - 0.6, 0.0).with_scale(Vec3::new(
                        plan.half_w * 2.0 + 0.3,
                        1.2,
                        plan.half_d * 2.0 + 0.3,
                    )),
                    ChildOf(site_entity),
                ));
                // Two stone steps down from the threshold, on the door side.
                // (Not for the well - nobody steps up into a well.)
                let step_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
                let steps: &[(f32, f32, f32)] = if plan.kind == BuildingKind::Well {
                    &[]
                } else {
                    &[(0.32, 0.24, 0.6), (0.78, 0.1, 0.55)]
                };
                for &(out, top, depth) in steps {
                    commands.spawn((
                        Mesh3d(step_mesh.clone()),
                        MeshMaterial3d(stone_material.clone()),
                        Transform::from_xyz(plan.half_w + out, top - 0.02, 0.0)
                            .with_scale(Vec3::new(depth, top * 2.0, 1.2)),
                        ChildOf(site_entity),
                    ));
                }
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
        if distance > WORK_RANGE {
            target.0 = Some(job.site);
            job.patience -= dt;
            if job.patience <= 0.0 {
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
        let cycle =
            if context.3.iter().any(|b| b.kind == BuildingKind::Blacksmith) && store.iron > 0.0 {
                WORK_SECONDS * 0.75
            } else {
                WORK_SECONDS
            } * manner.map_or(1.0, |m| m.work_pace())
                * weather.as_ref().map_or(1.0, |w| w.toil());
        job.progress += dt;
        if job.progress < cycle {
            continue;
        }
        job.progress = 0.0;

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
                let mut picked = source.amount.min(1.0);
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
                let mut catch = 1.0_f32;
                if context.3.iter().any(|b| b.kind == BuildingKind::Dock) {
                    catch += 0.5;
                }
                if context.3.iter().any(|b| b.kind == BuildingKind::Smokehouse) {
                    catch *= 2.0;
                }
                if ate_at_work {
                    catch = (catch - 0.4).max(0.2);
                }
                store.larder.add(FoodKind::Fish, catch);
            }

            Vocation::Miner | Vocation::Mason => match job.focus {
                // Bare high ground still yields loose stone.
                None => store.stone += 1.0,
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
                    }
                    if deposit.amount <= 0.5 {
                        commands.entity(worked).despawn();
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
                    store.stone += 1.0;
                    rock_transform.scale *= 0.72;
                    if rock_transform.scale.x < 0.4 {
                        commands.entity(rock).despawn();
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
                    let Ok(mut felled) = trees.get_mut(tree) else {
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                        continue;
                    };
                    if !felled.harvestable() {
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                        continue;
                    }
                    felled.maturity = 0.05;
                    // Shoulder the logs and turn for home. The timber only
                    // becomes the village's when it reaches the pile — and a
                    // sawmill wrings a third log from every tree.
                    let yield_ = if context.3.iter().any(|b| b.kind == BuildingKind::Sawmill) {
                        3.0
                    } else {
                        2.0
                    };
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
            Vocation::Carpenter => {}

            // A worked field surges; a ripe one is brought in.
            Vocation::Farmer => match job.focus {
                None => {
                    // Till a new plot where they stand — and level it first.
                    // The pad is real terrain: the ground itself is worked
                    // flat and rolls back into the hillside around it.
                    let level = terrain.height_at(job.site.x, job.site.z);
                    terrain.flatten(job.site.x, job.site.z, 3.4, 2.6, level);
                    for chunk in chunks.take_near(job.site.x, job.site.z, 7.0) {
                        commands.entity(chunk).despawn();
                    }
                    grass.invalidate_near(&mut commands, job.site.x, job.site.z, 7.0);
                    let at = Vec3::new(job.site.x, level, job.site.z);
                    let rotation = Quat::from_rotation_y({
                        let toward = (site.centre - at).with_y(0.0);
                        let toward = toward.normalize_or_zero();
                        (-toward.z).atan2(toward.x)
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
                    let Ok(mut field) = fields_mut.get_mut(field_entity) else {
                        *activity = Activity::Idle;
                        commands.entity(entity).remove::<Job>();
                        continue;
                    };
                    if field.growth >= 1.0 {
                        // The mill grinds the harvest into half again as much.
                        store.larder.add(
                            FoodKind::Grain,
                            if context.3.iter().any(|b| b.kind == BuildingKind::Mill) {
                                9.0
                            } else {
                                6.0
                            },
                        );
                        field.growth = 0.08;
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
                        field.growth = (field.growth + 0.10).min(1.0);
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
                };
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
                let Ok((_, mut vitality, mut prey_motion, is_corpse, genome)) =
                    prey_query.get_mut(prey)
                else {
                    continue;
                };

                if is_corpse {
                    // The kill is made; bring it home as food.
                    store.larder.add(FoodKind::Meat, CARCASS_FOOD);
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
                prey_motion.flail = 1.0;
                motion.flail = motion.flail.max(0.4);
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

/// The hungry eat from the store when the bushes cannot feed them.
///
/// This is what the stockpile is *for*: the difference between a bad berry
/// season and a funeral.
/// The blacksmith at work: ore out of the far hills becomes iron, and
/// iron in the store means every trade's tools bite better - until the
/// edges dull. Mine, smelt, wear out, mine again: the first strategic
/// resource loop.
pub(super) fn smelt(
    time: Res<Time>,
    mut since_last: Local<f32>,
    site: Option<Res<SettlementSite>>,
    buildings: Query<&Building>,
    mut stores: Query<&mut Stockpile>,
) {
    *since_last += time.delta_secs();
    if *since_last < 22.0 {
        return;
    }
    let interval = *since_last;
    *since_last = 0.0;
    let Some(site) = site else {
        return;
    };
    if !buildings.iter().any(|b| b.kind == BuildingKind::Blacksmith) {
        return;
    }
    let Ok(mut store) = stores.get_mut(site.settlement) else {
        return;
    };
    if store.ore >= 1.0 {
        store.ore -= 1.0;
        store.iron += 1.0;
    }
    // Tools wear: the edge is spent slowly whenever iron is in use.
    if store.iron > 0.0 {
        store.iron = (store.iron - 0.004 * interval).max(0.0);
    }
}

/// The weaver at work: dye out of the flowers becomes bright cloth on
/// the village's backs, and bright cloth is a quiet lift to every day
/// it is worn. Vanity, but vanity that keeps spirits above the line.
pub(super) fn dye_cloth(
    time: Res<Time>,
    mut since_last: Local<f32>,
    site: Option<Res<SettlementSite>>,
    buildings: Query<&Building>,
    mut stores: Query<&mut Stockpile>,
    mut wearers: Query<&mut super::Morale, With<Villager>>,
) {
    *since_last += time.delta_secs();
    if *since_last < 45.0 {
        return;
    }
    *since_last = 0.0;
    let Some(site) = site else {
        return;
    };
    if !buildings.iter().any(|b| b.kind == BuildingKind::Weaver) {
        return;
    }
    let Ok(mut store) = stores.get_mut(site.settlement) else {
        return;
    };
    if store.dye < 0.3 {
        return;
    }
    store.dye -= 0.3;
    for mut morale in &mut wearers {
        morale.spirits = (morale.spirits + 0.03).min(1.0);
    }
}

/// The bakery at work: the store's grain becomes bread, loaf by loaf.
/// Bread stretches — a baked meal draws only three quarters as deep — so
/// a working bakery quietly makes every harvest feed more mouths.
pub(super) fn bake(
    time: Res<Time>,
    mut since_last: Local<f32>,
    site: Option<Res<SettlementSite>>,
    buildings: Query<&Building>,
    mut stores: Query<&mut Stockpile>,
) {
    *since_last += time.delta_secs();
    if *since_last < 25.0 {
        return;
    }
    *since_last = 0.0;
    let Some(site) = site else {
        return;
    };
    if !buildings.iter().any(|b| b.kind == BuildingKind::Bakery) {
        return;
    }
    let Ok(mut store) = stores.get_mut(site.settlement) else {
        return;
    };
    if store.larder.grain >= 1.0 {
        store.larder.grain -= 1.0;
        store.larder.bread += 1.3;
    }
}

pub(super) fn eat_from_store(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    kitchen: Res<KitchenWarm>,
    site: Option<Res<SettlementSite>>,
    mut stores: Query<&mut Stockpile>,
    bushes: Query<(&GlobalTransform, &FoodSource)>,
    mut hungry: Query<
        (
            Entity,
            &Transform,
            &mut Needs,
            &mut super::Morale,
            &mut Activity,
            &mut MoveTarget,
            Option<&super::traits::Traits>,
            Option<&LastMeal>,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let cooked = clock.elapsed < kitchen.until;
    let Some(site) = site else {
        return;
    };
    let Ok(mut store) = stores.get_mut(site.settlement) else {
        return;
    };

    for (who, transform, mut needs, mut morale, mut activity, mut target, manner, last) in
        &mut hungry
    {
        match *activity {
            Activity::VisitingStore => {
                if store.food() < 1.0 {
                    *activity = Activity::Idle;
                    target.0 = None;
                    continue;
                }
                if transform.translation.distance(site.centre) > 4.0 {
                    target.0 = Some(site.centre);
                    continue;
                }
                // The meal comes out of the larder by kind — bread first
                // and cheapest, then whatever the village is deepest in.
                let meal = manner.map_or(1.0, |m| m.appetite());
                let Some(kind) = store.larder.draw(meal) else {
                    *activity = Activity::Idle;
                    target.0 = None;
                    continue;
                };
                let ration = if cooked { 0.85 } else { 0.55 };
                needs.hunger = (needs.hunger - ration).max(0.0);
                if cooked {
                    morale.spirits = (morale.spirits + 0.08).min(1.0);
                }
                // The tongue keeps score: the same meal again dulls the
                // day a little, a change of kind brightens it.
                match last {
                    Some(last) if last.0 == kind => {
                        morale.spirits = (morale.spirits - 0.02).max(0.0);
                    }
                    Some(_) => {
                        morale.spirits = (morale.spirits + 0.05).min(1.0);
                    }
                    None => {}
                }
                commands.entity(who).insert(LastMeal(kind));
                target.0 = None;
                if needs.hunger < 0.1 {
                    *activity = Activity::Idle;
                }
            }
            Activity::Idle | Activity::Wandering => {
                // The store opens for anyone hungry with no fruiting bush
                // in reasonable reach - a berry heath three ridges away is
                // no reason to starve beside a full larder.
                let bush_near = bushes.iter().any(|(at, bush)| {
                    bush.amount > 0.2 && at.translation().distance(transform.translation) < 30.0
                });
                if !bush_near && needs.hunger > DOWN_TOOLS_HUNGER && store.food() >= 1.0 {
                    *activity = Activity::VisitingStore;
                    target.0 = Some(site.centre);
                }
            }
            _ => {}
        }
    }
}

/// Raises the visible stage of a building under construction, shaped and
/// coloured by its blueprint. Each stage spawns geometry as children of the
/// site, so the building accretes in place — and no two need look alike.
pub(crate) fn raise_stage(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    site: Entity,
    stage: u8,
    plan: &Blueprint,
) {
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
    let mut part = |offset: Vec3, size: Vec3, rot_z: f32, material: &Handle<StandardMaterial>| {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(offset + Vec3::Y * lift)
                .with_rotation(Quat::from_rotation_z(rot_z))
                .with_scale(size),
            ChildOf(site),
        ));
    };

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
                drop(part);
                for (zed, tilt) in [(-d * 0.4, -0.5_f32), (d * 0.4, 0.5)] {
                    commands.spawn((
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
            if plan.shed_roof {
                // A real lean-to: the slab's low edge seats on the wall top,
                // the high side is walled to meet it, and the sloping sides
                // are closed with stepped wedges.
                let slope = 0.13_f32;
                let rise = (2.0 * w) * slope.tan();
                part(
                    Vec3::new(0.0, h + rise * 0.5 + 0.05, 0.0),
                    Vec3::new(w * 2.35 / slope.cos(), 0.12, d * 2.4),
                    slope,
                    &roof,
                );
                // The high wall band, over the door side.
                part(
                    Vec3::new(w, h + rise * 0.5, 0.0),
                    Vec3::new(0.18, rise + 0.08, d * 2.1),
                    0.0,
                    &wall,
                );
                // Stepped wedges up the sloping sides.
                let steps = 4;
                for k in 0..steps {
                    let band_h = rise / steps as f32;
                    let left = -w + (k as f32) * (2.0 * w / steps as f32);
                    let width = w - left;
                    for zed in [-d, d] {
                        part(
                            Vec3::new(left + width * 0.5, h + (k as f32 + 0.5) * band_h, zed),
                            Vec3::new(width, band_h + 0.03, 0.16),
                            0.0,
                            &wall,
                        );
                    }
                }
            } else {
                let slab = w * 1.42;
                part(
                    Vec3::new(-w * 0.55, h + 0.45, 0.0),
                    Vec3::new(slab, 0.12, d * 2.35),
                    0.55,
                    &roof,
                );
                part(
                    Vec3::new(w * 0.55, h + 0.45, 0.0),
                    Vec3::new(slab, 0.12, d * 2.35),
                    -0.55,
                    &roof,
                );
                part(
                    Vec3::new(0.0, h + 0.45 + w * 0.58, 0.0),
                    Vec3::new(0.2, 0.2, d * 2.4),
                    0.0,
                    &frame,
                );
                // Gable end-caps: each band's width comes from the roof
                // slabs' actual underside line - the slabs pass through
                // (0.55w, eaves) at slope 0.613 - so nothing ever pokes
                // through the roof, on any rolled size.
                let pitch = 0.613; // tan of the slab tilt
                let eave = 0.45;
                let peak = eave + w * 0.55 * pitch;
                for zed in [-d, d] {
                    let mut y = 0.02_f32;
                    loop {
                        let band_top = y + 0.26;
                        let half = (w * 0.55 - (band_top - eave) / pitch).min(w * 0.92) - 0.06;
                        if half < 0.12 {
                            break;
                        }
                        part(
                            Vec3::new(0.0, h + y + 0.13, zed),
                            Vec3::new(half * 2.0, 0.26, 0.16),
                            0.0,
                            &wall,
                        );
                        y += 0.26;
                    }
                    // The last sliver under the ridge, plugged with one
                    // narrow block sized to what actually remains.
                    let remaining = (peak - 0.05) - y;
                    if remaining > 0.05 {
                        let half =
                            (w * 0.55 - (y + remaining * 0.7 - eave) / pitch).max(0.1) - 0.02;
                        part(
                            Vec3::new(0.0, h + y + remaining * 0.5, zed),
                            Vec3::new((half * 2.0).max(0.14), remaining, 0.16),
                            0.0,
                            &wall,
                        );
                    }
                }
            }
            if matches!(plan.kind, BuildingKind::TownHall | BuildingKind::Shrine) {
                part(
                    Vec3::new(0.0, h + 0.45 + w * 0.58 + 0.35, 0.0),
                    Vec3::new(0.35, 0.35, 0.35),
                    0.0,
                    &gold,
                );
            }
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
pub(super) fn village_slots(centre: Vec3, rings: std::ops::Range<u32>) -> Vec<(f32, f32, f32)> {
    let mut slots = Vec::new();
    for ring in rings {
        let radius = 14.0 + ring as f32 * 9.0;
        let count = ((std::f32::consts::TAU * radius) / 12.0).floor().max(4.0) as u32;
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
pub(super) fn plan_houses(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    terrain: Res<Terrain>,
    site: Option<Res<SettlementSite>>,
    mut rng: ResMut<SimRng>,
    mut stores: Query<&mut Stockpile>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    standing: Query<(Entity, &GlobalTransform), With<crate::scatter::FellableTree>>,
    mut ground: (
        ResMut<crate::terrain::LoadedChunks>,
        ResMut<crate::grass::GrassChunks>,
    ),
    census: (
        Query<
            (
                Option<&Vocation>,
                &super::Morale,
                Option<&super::home::Home>,
                Option<&super::belief::Faith>,
                Option<&Vitality>,
                Has<crate::creature::Childhood>,
            ),
            (With<Villager>, Without<Corpse>),
        >,
        Query<&Field>,
        Query<(&Transform, &CreatureGenome), With<crate::creature::wildlife::Wild>>,
    ),
    civics: Query<&Building>,
    pending: Query<&Blueprint, With<ConstructionSite>>,
    roofs: Query<&Transform, Or<(With<ConstructionSite>, With<Hut>, With<Building>)>>,
) {
    *since_last += time.delta_secs();
    if *since_last < 12.0 {
        return;
    }
    *since_last = 0.0;

    let Some(site) = site else {
        return;
    };
    let (souls, fields, wild) = census;

    // The census: what the village actually is, right now.
    let mut population = 0usize;
    let mut roofless_adults = 0usize;
    let mut spirits_sum = 0.0f32;
    let mut hurt = 0usize;
    let mut believers = 0usize;
    let mut fishers = 0usize;
    let mut farmers = 0usize;
    let mut foresters = 0usize;
    for (vocation, morale, home, faith, vitality, child) in &souls {
        population += 1;
        spirits_sum += morale.spirits;
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
            _ => {}
        }
    }

    let has_kind = |kind: BuildingKind| {
        civics.iter().any(|b| b.kind == kind) || pending.iter().any(|b| b.kind == kind)
    };
    let Ok(store_now) = stores.get(site.settlement) else {
        return;
    };
    if store_now.timber < 2.0 {
        return;
    }

    let shore_near = find_shore(&terrain, site.centre, &mut rng.0).is_some();

    // A person needs a house, so a house gets built: ground breaks because
    // roofless people exist, not because a formula says the town is due.
    // Only when everyone sleeps under a roof does the village have the
    // spare hands for what it merely wants. One need outranks even the
    // roof: an empty larder beside open water breaks ground on the dock
    // first, because hunger kills faster than rain.
    let kind =
        if population >= 5 && store_now.food() < 8.0 && shore_near && !has_kind(BuildingKind::Dock)
        {
            BuildingKind::Dock
        } else if roofless_adults > 0 && !pending.iter().any(|b| b.kind == BuildingKind::House) {
            BuildingKind::House
        } else if roofless_adults > 0 {
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
                wolves_near: wild
                    .iter()
                    .filter(|(at, genome)| {
                        genome.species == Species::Wolf
                            && at.translation.distance(site.centre) < 130.0
                    })
                    .count(),
                pending_builds: pending.iter().count(),
                shore_near,
            };
            match next_civic(&needs, has_kind) {
                Some(kind) => kind,
                None => return,
            }
        };
    let _ = roofs;

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
        let (chunks, grass) = &mut ground;
        for chunk in chunks.take_near(shore.x, shore.z, plan.half_w + 5.0) {
            commands.entity(chunk).despawn();
        }
        grass.invalidate_near(&mut commands, shore.x, shore.z, plan.half_w + 5.0);
        let mut cleared = 0.0;
        for (tree, tree_at) in &standing {
            if tree_at.translation().distance(shore) < plan.half_w + 4.0 {
                commands.entity(tree).despawn();
                cleared += 1.0;
            }
        }
        if cleared > 0.0
            && let Ok(mut store) = stores.get_mut(site.settlement)
        {
            store.timber += cleared;
        }

        let building = commands
            .spawn((
                Name::new(format!("{}, rising", plan.kind.name())),
                ConstructionSite::default(),
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

    // Civic buildings claim the inner ring; houses begin one ring out. The
    // first plot the terrain permits wins, so the village fills inside-out.
    let rings = if kind == BuildingKind::House {
        1..5
    } else {
        0..5
    };
    'darts: for (x, z, yaw) in village_slots(site.centre, rings) {
        if !terrain.is_walkable(x, z) {
            continue;
        }
        let centre_height = terrain.height_at(x, z);
        // High and dry: nobody builds a home on the beach, however flat it is.
        if centre_height < WATER_LEVEL + 2.5 {
            continue;
        }
        for (dx, dz) in [(-1.7, -1.9), (1.7, -1.9), (-1.7, 1.9), (1.7, 1.9)] {
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
            if other.translation.distance(at) < 10.0 {
                continue 'darts;
            }
        }

        let plan = Blueprint::roll(kind, &mut rng.0);

        // Breaking ground levels the plot: the pad is worked flat and rolls
        // back into the hillside, so no floor, sill, or stake ever clips
        // into a slope. This is what a foundation is *for*.
        let pad = plan.half_w.max(plan.half_d) + 1.6;
        terrain.flatten(at.x, at.z, pad, 2.4, at.y);
        let (chunks, grass) = &mut ground;
        for chunk in chunks.take_near(at.x, at.z, pad + 4.0) {
            commands.entity(chunk).despawn();
        }
        grass.invalidate_near(&mut commands, at.x, at.z, pad + 4.0);

        // And clears it properly: every tree within canopy's reach of the
        // walls is felled into the pile. Nobody roofs over a living oak,
        // and nobody wants branches through the bedroom either.
        let clearing = plan.half_w.max(plan.half_d) + 4.5;
        let mut cleared = 0.0;
        for (tree, tree_at) in &standing {
            if tree_at.translation().distance(at) < clearing {
                commands.entity(tree).despawn();
                cleared += 1.0;
            }
        }
        if cleared > 0.0
            && let Ok(mut store) = stores.get_mut(site.settlement)
        {
            store.timber += cleared;
        }

        let building = commands
            .spawn((
                Name::new(format!("{}, rising", plan.kind.name())),
                ConstructionSite::default(),
                plan.clone(),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
                crate::hand::PickRadius(plan.half_w.max(plan.half_d) + 0.9),
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
pub(super) fn sermons(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut say: MessageWriter<crate::ui::Say>,
    name: Option<Res<super::DivineName>>,
    clock: Res<crate::calendar::WorldClock>,
    site: Option<Res<SettlementSite>>,
    mut stores: Query<&mut Stockpile>,
    shrines: Query<(&GlobalTransform, &Building)>,
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
    let Some((preacher, sermon)) = congregation
        .iter()
        .find(|(_, at, witnessed, _, _, vocation, activity)| {
            **vocation == Vocation::Priest
                && **activity == Activity::Working
                && at.translation.distance(shrine) < 6.0
                && !witnessed.recent.is_empty()
        })
        .map(|(preacher, _, witnessed, _, _, _, _)| (preacher, witnessed.recent[0]))
    else {
        return;
    };
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());
    say.write(crate::ui::Say {
        speaker: preacher,
        text: format!("hear how {}", sermon.rumor().replace("the god", god)),
        thought: false,
    });

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
        witnessed.secondhand = witnessed.secondhand.saturating_add(1);
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

/// A line in the log once a minute, so an unattended run leaves an account of
/// whether the village is feeding itself.
pub(super) fn log_stores(
    time: Res<Time>,
    mut since_last: Local<f32>,
    stores: Query<(&super::Settlement, &Stockpile)>,
    site: Option<Res<SettlementSite>>,
    trees: Query<&crate::scatter::FellableTree>,
    wildlife: Query<(&Transform, &crate::creature::wildlife::Wild), Without<Corpse>>,
    working: Query<(&Vocation, &Activity), With<Villager>>,
) {
    *since_last += time.delta_secs();
    if *since_last < 60.0 {
        return;
    }
    *since_last = 0.0;

    let standing = trees.iter().filter(|t| t.harvestable()).count();
    let animals = wildlife.iter().count();
    let nearest = site
        .as_ref()
        .and_then(|site| {
            wildlife
                .iter()
                .map(|(t, _)| t.translation.distance(site.centre))
                .min_by(f32::total_cmp)
        })
        .unwrap_or(0.0);
    let at_work = working
        .iter()
        .filter(|(_, a)| **a == Activity::Working)
        .count();
    for (settlement, store) in &stores {
        info!(
            "the stores of {} hold {:.0} food, {:.0} timber, {:.0} stone \
             ({at_work} at work, {standing} trees standing, {animals} wild things, \
             nearest {nearest:.0} away)",
            settlement.name,
            store.food(),
            store.timber,
            store.stone,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vocation_gets_taken_up() {
        let mut rng = Rng::new(11);
        let mut seen = std::collections::HashSet::new();
        for i in 0..300 {
            let boldness = (i as f32 / 300.0).clamp(0.05, 0.95);
            seen.insert(format!("{:?}", roll_vocation(boldness, &mut rng)));
        }
        assert_eq!(seen.len(), 9, "some vocation never occurs: {seen:?}");
    }

    #[test]
    fn houses_scale_with_population_and_stages_with_timber() {
        // Ground broken, walls at a third, roof from two-thirds on — for any
        // building, whatever its cost.
        assert_eq!(stage_for(0.0, HOUSE_TIMBER), 0);
        assert_eq!(stage_for(2.0, HOUSE_TIMBER), 1);
        assert_eq!(stage_for(4.0, HOUSE_TIMBER), 2);
        assert_eq!(stage_for(5.0, 14.0), 1);
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

        // Wolves at the door raise a tower.
        let hunted = CivicNeeds {
            population: 12,
            stone: 99.0,
            avg_spirits: 0.7,
            wolves_near: 3,
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
}
