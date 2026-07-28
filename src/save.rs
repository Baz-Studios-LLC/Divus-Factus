//! Save slots: a living village written to disk and read back.
//!
//! The world is deterministic from its seed, so a save is the seed plus
//! everything that diverged: the clock, the worked ground, the known world,
//! the stores and beliefs, and every person with their body, their manner,
//! their family and their story. Loading clears the dynamic world, re-raises
//! the settlement fixtures through the same code that founded it, and
//! restores the living on top.
//!
//! Three slots, saved from the toolbar's disk button. Both passes run as
//! exclusive systems: serialising a village touches everything, and `&mut
//! World` is the honest signature for that.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::creature::body::CreatureAssets;
use crate::creature::genome::CreatureGenome;
use crate::creature::wildlife::Wild;
use crate::creature::{Childhood, Corpse, Vitality, spawn_creature};
use crate::terrain::{LoadedChunks, Terrain, TerrainChunk};
use crate::ui;
use crate::villager::belief::{Belief, Faith, Legend};
use crate::villager::explore::KnownWorld;
use crate::villager::home::Bonfire;
use crate::villager::traits::Traits;
use crate::villager::work::{
    Blueprint, Building, BuildingKind, ConstructionSite, Field, Hut, PileKind, Stockpile, StorePile,
};
use crate::villager::{
    Activity, Chronicle, MemberOf, Morale, Needs, Parentage, Person, Prime, RestoringSeed,
    Settlement, SettlementSite, Spouse, Villager, rites,
};
use crate::witness::{Temperament, Witnessed};

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_saves_window)
            .add_systems(
                Update,
                (
                    handle_slot_buttons,
                    relax_deletes,
                    refresh_slot_labels,
                    title_hides_save_buttons,
                    enter_on_title_load,
                ),
            )
            .add_systems(Update, patch_wild)
            .add_systems(Update, process_requests.run_if(request_pending));
        if std::env::var("EGREGORE_SAVE_TEST").is_ok() {
            app.add_systems(Update, save_test_harness);
        }
    }
}

// ---------------------------------------------------------------- the file

#[derive(Serialize, Deserialize)]
struct PersonSave {
    pos: Vec3,
    person: Person,
    genome: CreatureGenome,
    needs: Needs,
    morale: Morale,
    temperament_boldness: f32,
    witnessed: Witnessed,
    faith: Option<Faith>,
    traits: Option<Traits>,
    chronicle: Option<Chronicle>,
    vocation: Option<crate::villager::work::Vocation>,
    harm: f32,
    prime: Option<f32>,
    childhood: Option<f32>,
    spouse: Option<usize>,
    mother: Option<usize>,
    father: Option<usize>,
    home: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct BuildingSave {
    pos: Vec3,
    rot: Quat,
    blueprint: Blueprint,
    done: bool,
    progress: f32,
    stage: u8,
    stone_laid: f32,
}

#[derive(Serialize, Deserialize)]
struct SaveGame {
    version: u32,
    seed: u32,
    elapsed: f64,
    // The slot card's face.
    label_settlement: String,
    label_day: u32,
    label_souls: usize,
    god: String,
    founded: u32,
    centre: Vec3,
    woodpile: Vec3,
    worked: Vec<(f32, f32, f32, f32, f32)>,
    known: KnownWorld,
    belief: (f32, f32),
    legend_numbers: (f32, f32, f32, u8),
    legend_unlocked: Option<crate::miracles::Miracle>,
    legend_epithet: Option<String>,
    stores: (f32, f32, f32),
    fire_fuel: f32,
    piles: Vec<(u8, Vec3, Quat)>,
    people: Vec<PersonSave>,
    buildings: Vec<BuildingSave>,
    fields: Vec<(Vec3, Quat, f32, Option<usize>)>,
    graves: Vec<(Vec3, Quat, u32, Person, Chronicle)>,
    wildlife: Vec<(Vec3, CreatureGenome, f32, Vec3)>,
    // --- version 2: the rest of everything ---
    #[serde(default)]
    history: Vec<crate::villager::HistoryEvent>,
    #[serde(default)]
    kitchen_until: f64,
    #[serde(default)]
    corpses: Vec<(
        Vec3,
        Quat,
        Person,
        CreatureGenome,
        Option<Chronicle>,
        Option<f64>,
    )>,
    #[serde(default)]
    rehousing: Vec<(u8, Vec3, Quat, u8, u8)>,
    /// Chunk coords whose wild contents were fully captured at save time.
    #[serde(default)]
    patched_chunks: Vec<IVec2>,
    #[serde(default)]
    trees: Vec<(Vec3, f32)>,
    #[serde(default)]
    bushes: Vec<(Vec3, f32)>,
    #[serde(default)]
    boulders: Vec<(Vec3, Vec3)>,
    #[serde(default)]
    weather: (f32, f32, f64),
}

fn slots_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let base = if cfg!(target_os = "macos") {
        format!("{home}/Library/Application Support/Egregore/saves")
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(|a| format!("{a}/Egregore/saves"))
            .unwrap_or_else(|_| "saves".into())
    } else {
        format!("{home}/.local/share/egregore/saves")
    };
    std::path::PathBuf::from(base)
}

fn slot_path(slot: u8) -> std::path::PathBuf {
    slots_dir().join(format!("slot{slot}.json"))
}

// ------------------------------------------------------------- the requests

#[derive(Resource)]
pub struct PendingSave(pub u8);

#[derive(Resource)]
pub struct PendingLoad(pub u8);

fn request_pending(save: Option<Res<PendingSave>>, load: Option<Res<PendingLoad>>) -> bool {
    save.is_some() || load.is_some()
}

fn process_requests(world: &mut World) {
    if let Some(PendingSave(slot)) = world.remove_resource::<PendingSave>() {
        let outcome = match gather(world) {
            Some(save) => write_slot(slot, &save),
            None => Err("nothing to save yet".to_string()),
        };
        let text = match outcome {
            Ok(()) => format!("Saved to slot {slot}"),
            Err(why) => format!("Could not save: {why}"),
        };
        world.write_message(ui::Notice::new(text));
    }
    if let Some(PendingLoad(slot)) = world.remove_resource::<PendingLoad>() {
        match read_slot(slot) {
            Ok(save) => {
                apply(world, save);
                world.resource_mut::<Time<Virtual>>().unpause();
                let panels: Vec<Entity> = world
                    .query_filtered::<Entity, With<SavesPanel>>()
                    .iter(world)
                    .collect();
                for panel in panels {
                    world.entity_mut(panel).insert(Visibility::Hidden);
                }
                let menus: Vec<Entity> = world
                    .query_filtered::<Entity, With<crate::title::PauseMenu>>()
                    .iter(world)
                    .collect();
                for menu in menus {
                    world.entity_mut(menu).insert(Visibility::Hidden);
                }
                world.write_message(ui::Notice::fanfare(format!(
                    "The world returns as it was - slot {slot}"
                )));
            }
            Err(why) => {
                world.write_message(ui::Notice::new(format!("Could not load: {why}")));
            }
        }
    }
}

fn write_slot(slot: u8, save: &SaveGame) -> Result<(), String> {
    let dir = slots_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string(save).map_err(|e| e.to_string())?;
    std::fs::write(slot_path(slot), json).map_err(|e| e.to_string())
}

fn read_slot(slot: u8) -> Result<SaveGame, String> {
    let text = std::fs::read_to_string(slot_path(slot)).map_err(|_| "empty slot".to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

// --------------------------------------------------------------- gathering

fn gather(world: &mut World) -> Option<SaveGame> {
    let seed = world.resource::<crate::WorldSeed>().0;
    let elapsed = world.resource::<crate::calendar::WorldClock>().elapsed;
    let site = world.get_resource::<SettlementSite>()?;
    let settlement_entity = site.settlement;
    let centre = site.centre;
    let woodpile = site.woodpile;

    let (name, founded) = {
        let settlement = world.get::<Settlement>(settlement_entity)?;
        (settlement.name.clone(), settlement.founded)
    };
    let god = world
        .get_resource::<crate::villager::DivineName>()
        .map(|n| n.0.clone())
        .unwrap_or_else(|| "the god".to_string());
    let stores = world
        .get::<Stockpile>(settlement_entity)
        .map(|s| (s.food, s.timber, s.stone))
        .unwrap_or((0.0, 0.0, 0.0));

    let worked = world.resource::<Terrain>().export_worked();
    let known = {
        let k = world.resource::<KnownWorld>();
        KnownWorld {
            centre: k.centre,
            radius: k.radius,
            pockets: k
                .pockets
                .iter()
                .map(|p| crate::villager::explore::Pocket {
                    at: p.at,
                    radius: p.radius,
                })
                .collect(),
        }
    };
    let belief = {
        let b = world.resource::<Belief>();
        (b.total, b.spent)
    };
    let legend = world.resource::<Legend>();
    let legend_numbers = (
        legend.providence,
        legend.dread,
        legend.sustained,
        legend.tier,
    );
    let legend_unlocked = legend.unlocked;
    let legend_epithet = legend.epithet.map(|e| e.to_string());

    let fire_fuel = world
        .query::<&Bonfire>()
        .iter(world)
        .next()
        .map_or(0.0, |f| f.fuel);
    let piles: Vec<(u8, Vec3, Quat)> = world
        .query::<(&StorePile, &Transform)>()
        .iter(world)
        .map(|(pile, t)| {
            let kind = match pile.0 {
                PileKind::Food => 0,
                PileKind::Timber => 1,
                PileKind::Stone => 2,
            };
            (kind, t.translation, t.rotation)
        })
        .collect();

    // Buildings first, so homes can point into the list.
    let mut building_ids: Vec<Entity> = Vec::new();
    let mut buildings: Vec<BuildingSave> = Vec::new();
    for (entity, transform, blueprint, building, site_state) in world
        .query::<(
            Entity,
            &Transform,
            &Blueprint,
            Option<&Building>,
            Option<&ConstructionSite>,
        )>()
        .iter(world)
    {
        let (done, progress, stage, stone_laid) = match (building, site_state) {
            (Some(_), _) => (true, 0.0, 2, 0.0),
            (None, Some(cs)) => (false, cs.progress, cs.stage, cs.stone_laid),
            _ => continue,
        };
        building_ids.push(entity);
        buildings.push(BuildingSave {
            pos: transform.translation,
            rot: transform.rotation,
            blueprint: blueprint.clone(),
            done,
            progress,
            stage,
            stone_laid,
        });
    }

    // People, with an index map so ties survive the trip.
    let mut people_ids: Vec<Entity> = Vec::new();
    let mut raw: Vec<(
        Entity,
        Vec3,
        Person,
        CreatureGenome,
        Needs,
        Morale,
        f32,
        Witnessed,
        Option<Faith>,
        Option<Traits>,
        Option<Chronicle>,
        Option<crate::villager::work::Vocation>,
        f32,
        Option<f32>,
        Option<f32>,
        (Option<Entity>, Option<(Entity, Entity)>, Option<Entity>),
    )> = Vec::new();
    for (entity, transform, person, genome, needs, morale, temperament, witnessed, extras, ties) in
        world
            .query_filtered::<(
                Entity,
                &Transform,
                &Person,
                &CreatureGenome,
                &Needs,
                &Morale,
                &Temperament,
                &Witnessed,
                (
                    Option<&Faith>,
                    Option<&Traits>,
                    Option<&Chronicle>,
                    Option<&crate::villager::work::Vocation>,
                    Option<&Vitality>,
                    Option<&Prime>,
                    Option<&Childhood>,
                ),
                (
                    Option<&Spouse>,
                    Option<&Parentage>,
                    Option<&crate::villager::home::Home>,
                ),
            ), (With<Villager>, Without<Corpse>)>()
            .iter(world)
    {
        let (faith, traits, chronicle, vocation, vitality, prime, childhood) = extras;
        let (spouse, parentage, home) = ties;
        people_ids.push(entity);
        raw.push((
            entity,
            transform.translation,
            person.clone(),
            genome.clone(),
            Needs {
                hunger: needs.hunger,
                rest: needs.rest,
            },
            Morale {
                spirits: morale.spirits,
            },
            temperament.boldness,
            witnessed.clone(),
            faith.cloned(),
            traits.map(|t| Traits(t.0.clone())),
            chronicle.cloned(),
            vocation.copied(),
            vitality.map_or(0.0, |v| v.harm),
            prime.map(|p| p.remaining),
            childhood.map(|c| c.remaining),
            (
                spouse.map(|s| s.0),
                parentage.map(|p| (p.mother, p.father)),
                home.map(|h| h.0),
            ),
        ));
    }
    let index_of = |entity: Entity, ids: &[Entity]| ids.iter().position(|e| *e == entity);
    let people: Vec<PersonSave> = raw
        .into_iter()
        .map(
            |(
                _,
                pos,
                person,
                genome,
                needs,
                morale,
                temperament_boldness,
                witnessed,
                faith,
                traits,
                chronicle,
                vocation,
                harm,
                prime,
                childhood,
                (spouse, parents, home),
            )| {
                PersonSave {
                    pos,
                    person,
                    genome,
                    needs,
                    morale,
                    temperament_boldness,
                    witnessed,
                    faith,
                    traits,
                    chronicle,
                    vocation,
                    harm,
                    prime,
                    childhood,
                    spouse: spouse.and_then(|e| index_of(e, &people_ids)),
                    mother: parents.and_then(|(m, _)| index_of(m, &people_ids)),
                    father: parents.and_then(|(_, f)| index_of(f, &people_ids)),
                    home: home.and_then(|e| index_of(e, &building_ids)),
                }
            },
        )
        .collect();

    let fields: Vec<(Vec3, Quat, f32, Option<usize>)> = world
        .query::<(&Transform, &Field)>()
        .iter(world)
        .map(|(t, field)| {
            (
                t.translation,
                t.rotation,
                field.growth,
                index_of(field.farmer, &people_ids),
            )
        })
        .collect();

    let graves: Vec<(Vec3, Quat, u32, Person, Chronicle)> = world
        .query::<(&Transform, &rites::Grave, &Person, &Chronicle)>()
        .iter(world)
        .map(|(t, grave, person, story)| {
            (
                t.translation,
                t.rotation,
                grave.day,
                person.clone(),
                story.clone(),
            )
        })
        .collect();

    let wildlife: Vec<(Vec3, CreatureGenome, f32, Vec3)> = world
        .query_filtered::<(&Transform, &CreatureGenome, &Wild), Without<Corpse>>()
        .iter(world)
        .map(|(t, genome, wild)| (t.translation, genome.clone(), wild.hunger, wild.home))
        .collect();

    let history = world
        .get_resource::<crate::villager::WorldChronicle>()
        .map(|h| h.events.clone())
        .unwrap_or_default();
    let kitchen_until = world
        .get_resource::<crate::villager::work::KitchenWarm>()
        .map_or(0.0, |k| k.until);
    let corpses: Vec<(
        Vec3,
        Quat,
        Person,
        CreatureGenome,
        Option<Chronicle>,
        Option<f64>,
    )> = world
        .query_filtered::<(
            &Transform,
            &Person,
            &CreatureGenome,
            Option<&Chronicle>,
            Option<&rites::Passing>,
        ), (With<Villager>, With<Corpse>)>()
        .iter(world)
        .map(|(t, person, genome, chronicle, passing)| {
            (
                t.translation,
                t.rotation,
                person.clone(),
                genome.clone(),
                chronicle.cloned(),
                passing.map(|p| p.since),
            )
        })
        .collect();
    let rehousing: Vec<(u8, Vec3, Quat, u8, u8)> = world
        .query::<(&StorePile, &crate::villager::work::Rehouse)>()
        .iter(world)
        .map(|(pile, r)| {
            let kind = match pile.0 {
                PileKind::Food => 0,
                PileKind::Timber => 1,
                PileKind::Stone => 2,
            };
            (kind, r.to, r.to_rot, r.hauled, r.goal)
        })
        .collect();
    // The wild things whose chunks are loaded right now are fully known;
    // their chunks go in the ledger so the loader only patches ground it
    // truly knew.
    let patched_chunks: Vec<IVec2> = world
        .query::<(&TerrainChunk, &Transform)>()
        .iter(world)
        .map(|(chunk, _)| chunk.coord)
        .collect();
    let trees: Vec<(Vec3, f32)> = world
        .query::<(&GlobalTransform, &crate::scatter::FellableTree)>()
        .iter(world)
        .map(|(t, tree)| (t.translation(), tree.maturity))
        .collect();
    let bushes: Vec<(Vec3, f32)> = world
        .query::<(&GlobalTransform, &crate::scatter::FoodSource)>()
        .iter(world)
        .map(|(t, bush)| (t.translation(), bush.amount))
        .collect();
    let boulders: Vec<(Vec3, Vec3)> = world
        .query_filtered::<&Transform, With<crate::matter::Boulder>>()
        .iter(world)
        .map(|t| (t.translation, t.scale))
        .collect();

    let label_day = world.resource::<crate::calendar::WorldClock>().day();
    Some(SaveGame {
        version: 1,
        seed,
        elapsed,
        label_settlement: name.clone(),
        label_day,
        label_souls: people.len(),
        god,
        founded,
        centre,
        woodpile,
        worked,
        known,
        belief,
        legend_numbers,
        legend_unlocked,
        legend_epithet,
        stores,
        fire_fuel,
        piles,
        people,
        buildings,
        fields,
        graves,
        wildlife,
        history,
        kitchen_until,
        corpses,
        rehousing,
        patched_chunks,
        trees,
        bushes,
        boulders,
        weather: world
            .get_resource::<crate::weather::Weather>()
            .map_or((0.15, 0.15, 0.0), |w| (w.intensity, w.target, w.next_front)),
    })
}

// --------------------------------------------------------------- restoring

fn apply(world: &mut World, save: SaveGame) {
    // 1. Clear the dynamic world. Recursive despawns take children with them.
    let mut doomed: Vec<Entity> = Vec::new();
    macro_rules! sweep {
        ($t:ty) => {
            doomed.extend(world.query_filtered::<Entity, With<$t>>().iter(world));
        };
    }
    sweep!(crate::creature::Creature);
    sweep!(Building);
    sweep!(ConstructionSite);
    sweep!(Field);
    sweep!(rites::Grave);
    sweep!(crate::villager::explore::Cairn);
    sweep!(StorePile);
    sweep!(Bonfire);
    sweep!(Settlement);
    sweep!(TerrainChunk);
    sweep!(crate::matter::Boulder);
    sweep!(crate::matter::Matter);
    doomed.sort();
    doomed.dedup();
    for entity in doomed {
        if let Ok(e) = world.get_entity_mut(entity) {
            e.despawn();
        }
    }
    world.resource_mut::<LoadedChunks>().take_all();
    world.resource_scope(|world, mut grass: Mut<crate::grass::GrassChunks>| {
        let mut commands = world.commands();
        grass.invalidate_all(&mut commands);
    });
    world.flush();
    {
        let mut hand = world.resource_mut::<crate::hand::DivineHand>();
        hand.held = None;
        hand.hovered = None;
    }
    if let Some(mut follow) = world.get_resource_mut::<crate::camera::FollowTarget>() {
        follow.entity = None;
    }

    // 2. The ground remembers.
    let terrain = Terrain::new(save.seed);
    terrain.import_worked(&save.worked);
    world.insert_resource(terrain);
    world.insert_resource(crate::WorldSeed(save.seed));
    world.insert_resource(crate::calendar::WorldClock {
        elapsed: save.elapsed,
    });

    // 3. Re-raise the fixtures through the founding code, under the old names.
    world.insert_resource(RestoringSeed {
        centre: save.centre,
        name: save.label_settlement.clone(),
        god: save.god.clone(),
        founded: save.founded,
    });
    let _ = world.run_system_once(crate::villager::spawn_settlement);
    world.remove_resource::<RestoringSeed>();

    let Some(site) = world.get_resource::<SettlementSite>() else {
        return;
    };
    let settlement_entity = site.settlement;

    // 4. Everything the fixtures own, put back the way it was.
    world.resource_mut::<SettlementSite>().woodpile = save.woodpile;
    if let Some(mut store) = world.get_mut::<Stockpile>(settlement_entity) {
        store.food = save.stores.0;
        store.timber = save.stores.1;
        store.stone = save.stores.2;
    }
    for (pile, mut transform) in world
        .query::<(&StorePile, &mut Transform)>()
        .iter_mut(world)
    {
        let kind = match pile.0 {
            PileKind::Food => 0,
            PileKind::Timber => 1,
            PileKind::Stone => 2,
        };
        if let Some((_, pos, rot)) = save.piles.iter().find(|(k, _, _)| *k == kind) {
            transform.translation = *pos;
            transform.rotation = *rot;
        }
    }
    for mut fire in world.query::<&mut Bonfire>().iter_mut(world) {
        fire.fuel = save.fire_fuel;
        fire.tender = None;
    }
    world.insert_resource(save.known);
    world.insert_resource(crate::villager::WorldChronicle {
        events: save.history.clone(),
    });
    world.insert_resource(crate::villager::work::KitchenWarm {
        until: save.kitchen_until,
    });
    world.insert_resource(crate::weather::Weather {
        intensity: save.weather.0,
        target: save.weather.1,
        next_front: save.weather.2,
        wind: 0.15 + save.weather.0 * 0.85,
    });
    world.insert_resource(Belief {
        total: save.belief.0,
        spent: save.belief.1,
    });
    let epithet = match save.legend_epithet.as_deref() {
        Some("the Provider") => Some("the Provider"),
        Some("the Stormhand") => Some("the Stormhand"),
        _ => None,
    };
    world.insert_resource(Legend {
        providence: save.legend_numbers.0,
        dread: save.legend_numbers.1,
        sustained: save.legend_numbers.2,
        tier: save.legend_numbers.3,
        unlocked: save.legend_unlocked,
        epithet,
    });

    // 5. Buildings, replayed stage by stage.
    let mut building_ids: Vec<Entity> = Vec::new();
    for b in &save.buildings {
        let entity = {
            let mut commands = world.commands();
            let entity = commands
                .spawn((
                    b.blueprint.clone(),
                    Transform::from_translation(b.pos).with_rotation(b.rot),
                    Visibility::default(),
                    crate::hand::PickRadius(b.blueprint.half_w.max(b.blueprint.half_d) + 0.9),
                    crate::hand::Rooted,
                ))
                .id();
            entity
        };
        world.flush();
        let kind = b.blueprint.kind;
        {
            let mut commands = world.commands();
            if b.done {
                commands
                    .entity(entity)
                    .insert((Building { kind }, Name::new(kind.name())));
                if kind == BuildingKind::House {
                    commands.entity(entity).insert(Hut);
                }
            } else {
                commands.entity(entity).insert((
                    Name::new(format!("{}, rising", kind.name())),
                    ConstructionSite {
                        progress: b.progress,
                        stage: b.stage,
                        stone_laid: b.stone_laid,
                    },
                ));
            }
        }
        world.flush();
        world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
            world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
                let mut commands = world.commands();
                let top_stage = if b.done { 2 } else { b.stage };
                for stage in 0..=top_stage {
                    crate::villager::work::raise_stage(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        entity,
                        stage,
                        &b.blueprint,
                    );
                }
                // The mason's plinth and steps, if the foundation was laid.
                let cost = b.blueprint.kind.stone_cost();
                if cost > 0.0 && (b.done || b.stone_laid >= cost) {
                    let slab = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
                    let stone = materials.add(StandardMaterial {
                        base_color: crate::palette::shade(&crate::palette::STONE, 0.4),
                        perceptual_roughness: 1.0,
                        ..default()
                    });
                    commands.spawn((
                        Mesh3d(slab.clone()),
                        MeshMaterial3d(stone.clone()),
                        Transform::from_xyz(0.0, -0.25, 0.0).with_scale(Vec3::new(
                            b.blueprint.half_w * 2.0 + 0.3,
                            1.2,
                            b.blueprint.half_d * 2.0 + 0.3,
                        )),
                        ChildOf(entity),
                    ));
                    for (out, top, depth) in [(0.32_f32, 0.24_f32, 0.6_f32), (0.78, 0.1, 0.55)] {
                        commands.spawn((
                            Mesh3d(slab.clone()),
                            MeshMaterial3d(stone.clone()),
                            Transform::from_xyz(b.blueprint.half_w + out, top - 0.02, 0.0)
                                .with_scale(Vec3::new(depth, top * 2.0, 1.2)),
                            ChildOf(entity),
                        ));
                    }
                }
            });
        });
        world.flush();
        building_ids.push(entity);
    }

    // 6. The people themselves.
    let mut people_ids: Vec<Entity> = Vec::new();
    for p in &save.people {
        let entity = world.resource_scope(|world, assets: Mut<CreatureAssets>| {
            let mut commands = world.commands();
            spawn_creature(
                &mut commands,
                &assets,
                p.genome.clone(),
                p.pos,
                0.0,
                people_ids.len() as f32 * 0.618,
            )
        });
        world.flush();
        {
            let mut commands = world.commands();
            let mut e = commands.entity(entity);
            e.insert((
                Villager,
                p.person.clone(),
                Needs {
                    hunger: p.needs.hunger,
                    rest: p.needs.rest,
                },
                Morale {
                    spirits: p.morale.spirits,
                },
                Temperament {
                    boldness: p.temperament_boldness,
                },
                p.witnessed.clone(),
                Activity::Idle,
                MemberOf(settlement_entity),
            ));
            if let Some(faith) = &p.faith {
                e.insert(faith.clone());
            }
            if let Some(traits) = &p.traits {
                e.insert(Traits(traits.0.clone()));
            }
            if let Some(chronicle) = &p.chronicle {
                e.insert(chronicle.clone());
            }
            if let Some(vocation) = p.vocation {
                e.insert(vocation);
            }
            if let Some(remaining) = p.prime {
                e.insert(Prime { remaining });
            }
            if let Some(remaining) = p.childhood {
                e.insert(Childhood { remaining });
            }
        }
        world.flush();
        if let Some(mut vitality) = world.get_mut::<Vitality>(entity) {
            vitality.harm = p.harm;
        }
        people_ids.push(entity);
    }
    // Ties, now that every index has an entity.
    for (i, p) in save.people.iter().enumerate() {
        let entity = people_ids[i];
        let mut commands = world.commands();
        let mut e = commands.entity(entity);
        if let Some(spouse) = p.spouse.and_then(|i| people_ids.get(i).copied()) {
            e.insert(Spouse(spouse));
        }
        if let (Some(mother), Some(father)) = (
            p.mother.and_then(|i| people_ids.get(i).copied()),
            p.father.and_then(|i| people_ids.get(i).copied()),
        ) {
            e.insert(Parentage { mother, father });
        }
        if let Some(home) = p.home.and_then(|i| building_ids.get(i).copied()) {
            e.insert(crate::villager::home::Home(home));
        }
    }
    world.flush();

    // 7. Fields, graves, and the wild.
    for (pos, rot, growth, farmer) in &save.fields {
        let farmer = farmer
            .and_then(|i| people_ids.get(i).copied())
            .unwrap_or(Entity::PLACEHOLDER);
        world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
            world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
                world.resource_scope(|world, mut rng: Mut<crate::villager::SimRng>| {
                    let mut commands = world.commands();
                    crate::villager::work::raise_field(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &mut rng.0,
                        *pos,
                        *rot,
                        *growth,
                        farmer,
                    );
                });
            });
        });
        world.flush();
    }
    for (pos, rot, day, person, story) in &save.graves {
        world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
            world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
                let mut commands = world.commands();
                let grave = commands
                    .spawn((
                        rites::Grave { day: *day },
                        person.clone(),
                        story.clone(),
                        Name::new(format!("The grave of {}", person.name)),
                        Transform::from_translation(*pos).with_rotation(*rot),
                        Visibility::default(),
                        crate::hand::PickRadius(1.6),
                        crate::hand::Rooted,
                    ))
                    .id();
                let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
                let earth = materials.add(StandardMaterial {
                    base_color: crate::palette::shade(&crate::palette::EARTH, 0.35),
                    perceptual_roughness: 1.0,
                    ..default()
                });
                let stone = materials.add(StandardMaterial {
                    base_color: crate::palette::shade(&crate::palette::STONE, 0.55),
                    perceptual_roughness: 0.95,
                    ..default()
                });
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(earth),
                    Transform::from_xyz(0.0, 0.16, 0.0).with_scale(Vec3::new(1.6, 0.32, 0.9)),
                    ChildOf(grave),
                ));
                commands.spawn((
                    Mesh3d(cube),
                    MeshMaterial3d(stone),
                    Transform::from_xyz(0.75, 0.42, 0.0).with_scale(Vec3::new(0.14, 0.84, 0.5)),
                    ChildOf(grave),
                ));
            });
        });
        world.flush();
    }
    for (pos, rot, person, genome, chronicle, passing) in &save.corpses {
        let entity = world.resource_scope(|world, assets: Mut<CreatureAssets>| {
            let mut commands = world.commands();
            spawn_creature(&mut commands, &assets, genome.clone(), *pos, 0.0, 0.0)
        });
        world.flush();
        {
            let mut commands = world.commands();
            let mut e = commands.entity(entity);
            e.insert((Villager, person.clone(), Corpse));
            if let Some(chronicle) = chronicle {
                e.insert(chronicle.clone());
            }
            if let Some(since) = passing {
                e.insert(rites::Passing { since: *since });
            }
        }
        world.flush();
        if let Some(mut transform) = world.get_mut::<Transform>(entity) {
            transform.rotation = *rot;
        }
    }
    world.insert_resource(WorldPatch {
        chunks: save.patched_chunks.iter().copied().collect(),
        trees: save.trees.clone(),
        bushes: save.bushes.clone(),
    });
    world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
        world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
            world.resource_scope(|world, mut rng: Mut<crate::villager::SimRng>| {
                let material = materials.add(StandardMaterial {
                    base_color: crate::palette::shade(&crate::palette::STONE, 0.45),
                    perceptual_roughness: 1.0,
                    ..default()
                });
                let mut commands = world.commands();
                for (pos, scale) in &save.boulders {
                    let boulder = crate::scatter::spawn_boulder(
                        &mut commands,
                        &mut meshes,
                        material.clone(),
                        *pos,
                        &mut rng.0,
                    );
                    commands.entity(boulder).insert(SavedBoulder);
                    commands.entity(boulder).entry::<Transform>().and_modify({
                        let scale = *scale;
                        move |mut t| t.scale = scale
                    });
                }
            });
        });
    });
    world.flush();

    for (pos, genome, hunger, home) in &save.wildlife {
        let entity = world.resource_scope(|world, assets: Mut<CreatureAssets>| {
            let mut commands = world.commands();
            spawn_creature(&mut commands, &assets, genome.clone(), *pos, 0.0, 0.0)
        });
        world.flush();
        let mut commands = world.commands();
        commands.entity(entity).insert((
            Activity::Idle,
            Wild {
                hunger: *hunger,
                busy: 0.0,
                home: *home,
            },
        ));
    }
    world.flush();
}

/// A saved boulder, exempt from the wild patch's dedupe sweep.
#[derive(Component)]
pub struct SavedBoulder;

/// What the save knew about the wild ground, applied as chunks stream back
/// in: known trees get their maturity, unknown trees in known chunks were
/// felled before the save and despawn, bushes get their fruit back, and
/// freshly scattered boulders in known chunks yield to the saved ones.
#[derive(Resource)]
pub struct WorldPatch {
    chunks: std::collections::HashSet<IVec2>,
    trees: Vec<(Vec3, f32)>,
    bushes: Vec<(Vec3, f32)>,
}

#[allow(clippy::type_complexity)]
fn patch_wild(
    mut commands: Commands,
    patch: Option<ResMut<WorldPatch>>,
    terrain: Option<Res<Terrain>>,
    chunk_transforms: Query<&Transform, With<TerrainChunk>>,
    mut new_trees: Query<
        (
            Entity,
            &ChildOf,
            &Transform,
            &mut crate::scatter::FellableTree,
        ),
        (Added<crate::scatter::FellableTree>, Without<TerrainChunk>),
    >,
    mut new_bushes: Query<
        (
            Entity,
            &ChildOf,
            &Transform,
            &mut crate::scatter::FoodSource,
        ),
        (Added<crate::scatter::FoodSource>, Without<TerrainChunk>),
    >,
    new_boulders: Query<
        (Entity, &Transform),
        (
            Added<crate::matter::Boulder>,
            Without<SavedBoulder>,
            Without<TerrainChunk>,
        ),
    >,
) {
    let (Some(mut patch), Some(terrain)) = (patch, terrain) else {
        return;
    };
    let world_of = |childof: &ChildOf, local: &Transform| -> Option<Vec3> {
        chunk_transforms
            .get(childof.parent())
            .ok()
            .map(|chunk| chunk.translation + local.translation)
    };
    for (entity, childof, local, mut tree) in &mut new_trees {
        let Some(at) = world_of(childof, local) else {
            continue;
        };
        if !patch.chunks.contains(&terrain.chunk_of(at.x, at.z)) {
            continue;
        }
        let found = patch
            .trees
            .iter()
            .position(|(saved, _)| saved.distance(at) < 1.2);
        match found {
            Some(i) => {
                tree.maturity = patch.trees.swap_remove(i).1;
            }
            None => {
                // This tree was felled before the save; it stays felled.
                commands.entity(entity).despawn();
            }
        }
    }
    for (entity, childof, local, mut bush) in &mut new_bushes {
        let Some(at) = world_of(childof, local) else {
            continue;
        };
        if !patch.chunks.contains(&terrain.chunk_of(at.x, at.z)) {
            continue;
        }
        let found = patch
            .bushes
            .iter()
            .position(|(saved, _)| saved.distance(at) < 1.2);
        match found {
            Some(i) => {
                bush.amount = patch.bushes.swap_remove(i).1;
            }
            None => {
                commands.entity(entity).despawn();
            }
        }
    }
    for (entity, transform) in &new_boulders {
        let at = transform.translation;
        if patch.chunks.contains(&terrain.chunk_of(at.x, at.z)) {
            // The save's own boulders stand in for these.
            commands.entity(entity).despawn();
        }
    }
}

/// Unattended round-trip check: save to slot 3 at 25 s, load it at 32 s.
/// Only registered under EGREGORE_SAVE_TEST; greppable in the log.
fn save_test_harness(mut commands: Commands, time: Res<Time>, mut fired: Local<u8>) {
    if *fired == 0 && time.elapsed_secs() > 25.0 {
        *fired = 1;
        info!("SAVE_TEST: writing slot 3");
        commands.insert_resource(PendingSave(3));
    }
    if *fired == 1 && time.elapsed_secs() > 32.0 {
        *fired = 2;
        info!("SAVE_TEST: loading slot 3");
        commands.insert_resource(PendingLoad(3));
    }
}

// -------------------------------------------------------------------- the UI

/// The saves window: three slots, each with its card and its two buttons.
#[derive(Component)]
pub struct SavesPanel;

#[derive(Component)]
struct SlotLabel(u8);

#[derive(Component)]
struct SaveButton(u8);

#[derive(Component)]
struct LoadButton(u8);

/// A slot's delete button; pressing arms it, pressing again deletes.
#[derive(Component)]
struct DeleteButton(u8);

/// A delete button waiting for its confirming second press.
#[derive(Component)]
struct DeleteArmed {
    until: f32,
}

fn spawn_saves_window(mut commands: Commands) {
    let window = ui::window(&mut commands, "SAVES", 340.0);
    commands
        .entity(window.root)
        .insert((Name::new("Saves Panel"), SavesPanel, Visibility::Hidden));
    for slot in 1..=3u8 {
        let row = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    ..default()
                },
                ChildOf(window.body),
            ))
            .id();
        commands.spawn((
            SlotLabel(slot),
            ui::body(format!("Slot {slot} - empty")),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ChildOf(row),
        ));
        for (label, role) in [("SAVE", 0u8), ("LOAD", 1), ("DEL", 2)] {
            let button = commands
                .spawn((
                    ui::UiButton,
                    Node {
                        padding: UiRect::axes(px(8), px(3)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BorderColor::all(ui::theme::panel_border()),
                    Interaction::default(),
                    ChildOf(row),
                ))
                .id();
            match role {
                0 => {
                    commands.entity(button).insert(SaveButton(slot));
                }
                1 => {
                    commands.entity(button).insert(LoadButton(slot));
                }
                _ => {
                    commands.entity(button).insert(DeleteButton(slot));
                }
            }
            commands.spawn((ui::dim(label), ChildOf(button)));
        }
    }
}

fn handle_slot_buttons(
    mut commands: Commands,
    time: Res<Time>,
    saves: Query<(&Interaction, &SaveButton), Changed<Interaction>>,
    loads: Query<(&Interaction, &LoadButton), Changed<Interaction>>,
    mut deletes: Query<
        (
            Entity,
            &Interaction,
            &DeleteButton,
            Option<&DeleteArmed>,
            &Children,
        ),
        Changed<Interaction>,
    >,
    mut texts: Query<&mut Text>,
    mut notices: MessageWriter<ui::Notice>,
) {
    for (interaction, button) in &saves {
        if *interaction == Interaction::Pressed {
            commands.insert_resource(PendingSave(button.0));
        }
    }
    for (interaction, button) in &loads {
        if *interaction == Interaction::Pressed {
            commands.insert_resource(PendingLoad(button.0));
        }
    }
    // Deleting takes two presses: the first arms, the second, soon after,
    // actually burns the world.
    for (entity, interaction, button, armed, children) in &mut deletes {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let now = time.elapsed_secs();
        match armed {
            Some(armed) if now < armed.until => {
                let _ = std::fs::remove_file(slot_path(button.0));
                commands.entity(entity).remove::<DeleteArmed>();
                for &child in children {
                    if let Ok(mut text) = texts.get_mut(child) {
                        *text = Text::new("DEL");
                    }
                }
                notices.write(ui::Notice::new(format!("Slot {} deleted", button.0)));
            }
            _ => {
                commands
                    .entity(entity)
                    .insert(DeleteArmed { until: now + 3.0 });
                for &child in children {
                    if let Ok(mut text) = texts.get_mut(child) {
                        *text = Text::new("SURE?");
                    }
                }
            }
        }
    }
}

/// Armed deletes relax if the second press never comes.
fn relax_deletes(
    mut commands: Commands,
    time: Res<Time>,
    armed: Query<(Entity, &DeleteArmed, &Children)>,
    mut texts: Query<&mut Text>,
) {
    for (entity, arm, children) in &armed {
        if time.elapsed_secs() >= arm.until {
            commands.entity(entity).remove::<DeleteArmed>();
            for &child in children {
                if let Ok(mut text) = texts.get_mut(child) {
                    *text = Text::new("DEL");
                }
            }
        }
    }
}

/// On the title screen the window is a load menu: saving a world you have
/// not entered means nothing, so those buttons step out.
fn title_hides_save_buttons(
    state: Res<State<crate::GameState>>,
    mut buttons: Query<&mut Node, With<SaveButton>>,
) {
    let on_title = matches!(state.get(), crate::GameState::Title);
    for mut node in &mut buttons {
        let wanted = if on_title {
            Display::None
        } else {
            Display::Flex
        };
        if node.display != wanted {
            node.display = wanted;
        }
    }
}

/// Loading from the title enters the world through the same door Begin uses.
fn enter_on_title_load(
    state: Res<State<crate::GameState>>,
    pending: Option<Res<PendingLoad>>,
    mut next: ResMut<NextState<crate::GameState>>,
    mut panels: Query<&mut Visibility, With<SavesPanel>>,
) {
    if pending.is_some() && matches!(state.get(), crate::GameState::Title) {
        next.set(crate::GameState::Loading);
        for mut visibility in &mut panels {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Slot cards read their files while the window is open.
fn refresh_slot_labels(
    time: Res<Time>,
    mut since_last: Local<f32>,
    panels: Query<&Visibility, With<SavesPanel>>,
    mut labels: Query<(&SlotLabel, &mut Text)>,
) {
    if !panels.iter().any(|v| *v != Visibility::Hidden) {
        return;
    }
    *since_last += time.delta_secs();
    if *since_last < 1.0 {
        return;
    }
    *since_last = 0.0;
    for (slot, mut text) in &mut labels {
        let fresh = match read_slot(slot.0) {
            Ok(save) => format!(
                "Slot {} - day {}, {} ({} souls)",
                slot.0, save.label_day, save.label_settlement, save.label_souls
            ),
            Err(_) => format!("Slot {} - empty", slot.0),
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}
