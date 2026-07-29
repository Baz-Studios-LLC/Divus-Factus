//! Buildings: kinds, blueprints, the civic chooser, the visible stages
//! a construction rises through, and the planner that breaks ground.

use bevy::prelude::*;

use super::*;
use crate::creature::genome::{CreatureGenome, Species};
use crate::rng::Rng;
use crate::terrain::{Terrain, WATER_LEVEL};
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
    /// A timbered adit driven into rising ground: miners bring out stone
    /// by the cartload instead of chipping boulders where they lie.
    Mine,
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
            BuildingKind::Mine => 6.0,
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
            // The mountain provides its own stone; the timber shores it up.
            BuildingKind::Mine => 0.0,
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
    pub wolves_near: usize,
    pub pending_builds: usize,
    /// Whether walkable shore lies within working reach — no water, no dock.
    pub shore_near: bool,
    /// Hands at the stone trade, arguing for a proper works.
    pub miners: usize,
    /// Whether rising rocky ground stands within working reach — no
    /// mountainside, no mine.
    pub rock_near: bool,
}

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
                stuff: BuildStuff::Timber,
            },
            BuildingKind::Sawmill => Blueprint {
                kind,
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
                half_w: 1.3,
                half_d: 1.4,
                wall_h: 1.4,
                walls: pal::shade(&pal::WOOD, 0.6),
                roof: pal::shade(&pal::GRASS, 0.5),
                shed_roof: true,
                stuff: BuildStuff::Timber,
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
                stuff: BuildStuff::Timber,
            },
            // A portal driven into the hillside: the frame is all that
            // shows, the works are in the dark. half_d points into the rise.
            BuildingKind::Mine => Blueprint {
                kind,
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

/// A finished building of any kind.
#[derive(Component, Debug)]
pub struct Building {
    pub kind: BuildingKind,
}

/// A finished house.
#[derive(Component)]
pub struct Hut;

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

/// Which visual stage a build should show, at `progress` timber toward a
/// total `cost`: frame, walls, roof, at thirds of the way.
pub fn stage_for(progress: f32, cost: f32) -> u8 {
    ((progress / cost.max(1.0) * 3.0) as u8).min(2)
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
pub(crate) fn village_slots(centre: Vec3, rings: std::ops::Range<u32>) -> Vec<(f32, f32, f32)> {
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
pub(crate) fn plan_houses(
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
            ),
            (With<Villager>, Without<Corpse>),
        >,
        Query<&Field>,
        Query<(&Transform, &CreatureGenome), With<crate::creature::wildlife::Wild>>,
    ),
    civics: Query<&Building>,
    pending: Query<(&Blueprint, &ConstructionSite)>,
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
    let mut miners = 0usize;
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
            Some(Vocation::Miner) => miners += 1,
            _ => {}
        }
    }

    let has_kind = |kind: BuildingKind| {
        civics.iter().any(|b| b.kind == kind) || pending.iter().any(|(b, _)| b.kind == kind)
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
        let step = 3.5;
        let rise_x = t.height_at(x + step, z) - t.height_at(x - step, z);
        let rise_z = t.height_at(x, z + step) - t.height_at(x, z - step);
        let uphill = Vec3::new(rise_x, 0.0, rise_z).normalize_or_zero();
        if uphill == Vec3::ZERO {
            return false;
        }
        t.height_at(x + uphill.x * 7.0, z + uphill.z * 7.0) - t.height_at(x, z) > 3.0
    };
    let rock_near = find_ground(&terrain, site.centre, &mut rng.0, minable).is_some();

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
    } else if (roofless_adults > 0 || {
        // Build AHEAD of need: the village keeps at least one empty
        // house's worth of room, so growth never waits on a construction
        // site. A town that builds exactly what it needs is always one
        // wedding behind.
        let houses = civics
            .iter()
            .filter(|b| b.kind == BuildingKind::House)
            .count();
        let capacity = crate::villager::home::shelter_capacity(houses);
        (capacity as i32 - population as i32) < crate::villager::home::HOUSE_CAPACITY as i32
    }) && !pending.iter().any(|(b, _)| b.kind == BuildingKind::House)
    {
        BuildingKind::House
    } else if roofless_adults > 0 {
        // A house is already rising for them - say so, with arithmetic,
        // so a stalled build is a visible fact instead of a silent
        // population ceiling.
        if let Some((plan, cs)) = pending.iter().find(|(b, _)| b.kind == BuildingKind::House) {
            info!(
                "housing watch: {} roofless; a house stands at {:.0} of {:.0} timber, {:.0} of {:.0} footing stone",
                roofless_adults,
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
            wolves_near: wild
                .iter()
                .filter(|(at, genome)| {
                    genome.species == Species::Wolf && at.translation.distance(site.centre) < 130.0
                })
                .count(),
            pending_builds: pending.iter().count(),
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
    if kind == BuildingKind::Mine {
        let Some(face) = find_ground(&terrain, site.centre, &mut rng.0, minable) else {
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

        // A worked yard at the mouth; the hill keeps its shape past it.
        terrain.flatten(face.x, face.z, plan.half_w + 1.6, 2.4, face.y);
        // And the hill is banked OVER the portal's back: whatever the
        // face's true shape, the door reads as a door into the earth —
        // nobody walks behind the mine and finds the back of the set.
        let crown = face + uphill * (plan.half_d * 1.2);
        terrain.flatten(
            crown.x,
            crown.z,
            plan.half_w * 1.4,
            3.2,
            face.y + plan.wall_h + 1.4,
        );
        let (chunks, grass, chunk_assets, stripped, dirty_groves) = &mut ground;
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

        let building = commands
            .spawn((
                Name::new(format!("{}, rising", plan.kind.name())),
                ConstructionSite::default(),
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

    // Civic buildings claim the inner ring; houses begin one ring out and
    // the town SPRAWLS: the ring search widens with the population, so a
    // growing city pushes its streets outward instead of hitting a wall
    // of full plots.
    let ring_reach = 5 + (population / 6) as u32;
    let rings = if kind == BuildingKind::House {
        1..ring_reach
    } else {
        0..ring_reach.min(7)
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

        let mut plan = Blueprint::roll(kind, &mut rng.0);
        // A land of woods and nothing else still shelters its people: with
        // no stone in the pile and no clay to brick, a timber house is
        // post-framed straight into the earth and owes the masons nothing.
        // Another path to a roof — until fire comes to the world, when
        // these will be the homes that fear it.
        let timber_footing =
            kind == BuildingKind::House && store_now.stone < 1.0 && store_now.clay < 1.0;
        if kind == BuildingKind::House {
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
        let pad = plan.half_w.max(plan.half_d) + 1.6;
        terrain.flatten(at.x, at.z, pad, 2.4, at.y);
        let (chunks, grass, chunk_assets, stripped, dirty_groves) = &mut ground;
        crate::terrain::rebuild_chunks_near(
            &mut commands,
            &mut meshes,
            chunk_assets,
            &terrain,
            chunks,
            at.x,
            at.z,
            pad + 4.0,
        );
        grass.invalidate_near(&mut commands, at.x, at.z, pad + 4.0);

        // And clears it properly: every tree within canopy's reach of the
        // walls is felled into the pile. Nobody roofs over a living oak,
        // and nobody wants branches through the bedroom either.
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

        let building = commands
            .spawn((
                Name::new(format!("{}, rising", plan.kind.name())),
                ConstructionSite {
                    timber_footing,
                    ..default()
                },
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
pub(crate) fn sermons(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut say: MessageWriter<crate::ui::Say>,
    name: Option<Res<crate::villager::DivineName>>,
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
