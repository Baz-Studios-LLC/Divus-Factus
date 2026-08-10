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
    Blueprint, Building, BuildingKind, ConstructionSite, Field, Homestead, Hut, Longhouse,
    PileKind, Stockpile, StorePile,
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
        if std::env::var("DIVUS_FACTUS_SAVE_TEST").is_ok() {
            app.add_systems(Update, save_test_harness);
        }
        if std::env::var("DIVUS_FACTUS_TITLE_LOAD_TEST").is_ok() {
            app.add_systems(Update, title_load_test_harness);
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
    #[serde(default)]
    skills: Option<crate::villager::work::Skills>,
    harm: f32,
    prime: Option<f32>,
    childhood: Option<f32>,
    spouse: Option<usize>,
    mother: Option<usize>,
    father: Option<usize>,
    home: Option<usize>,
    /// How many children she has borne. Absent in saves written before
    /// fertility began to fall with each birth; such mothers load as though
    /// their first child were still ahead of them.
    #[serde(default)]
    borne: u32,
    /// Whom this heart holds feelings about: (person index, warmth). Absent
    /// in saves written before regard; such souls load indifferent and the
    /// feeders warm them back up within a day.
    #[serde(default)]
    bonds: Vec<(usize, f32)>,
    /// Whether this soul wears the mayor's chain. Absent in saves from
    /// before the office existed; such towns elect within the day.
    #[serde(default)]
    mayor: bool,
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
    #[serde(default)]
    timber_footing: bool,
    /// Whether this house stands out past the rings on its own ground.
    #[serde(default)]
    homestead: bool,
}

#[derive(Serialize, Deserialize)]
struct SaveGame {
    version: u32,
    seed: u32,
    elapsed: f64,
    /// The god's faith curve, sampled daily. Old saves start theirs afresh.
    #[serde(default)]
    faith_history: crate::villager::belief::FaithHistory,
    // The slot card's face.
    label_settlement: String,
    label_day: u32,
    label_souls: usize,
    god: String,
    founded: u32,
    /// The banner as it flew: (cloth ramp, sigil index).
    #[serde(default)]
    banner: Option<(u32, u32)>,
    centre: Vec3,
    woodpile: Vec3,
    worked: Vec<crate::terrain::WorkedGround>,
    known: KnownWorld,
    belief: (f32, f32),
    legend_numbers: (f32, f32, f32, u8),
    legend_unlocked: Option<crate::miracles::Miracle>,
    legend_epithet: Option<String>,
    stores: (f32, f32, f32),
    /// The prayer board's receipts: answered, curdled, died waiting. The
    /// receipts are the board's whole point; a reload must not amnesty
    /// the god's record.
    #[serde(default)]
    prayer_receipts: Vec<crate::villager::belief::ClosedPrayer>,
    /// The hotbar's arrangement, the grimoire's unlocks and every
    /// miracle's rest — the whole cooldown economy, kept.
    #[serde(default)]
    hotbar: Option<crate::miracles::Hotbar>,
    #[serde(default)]
    grimoire: Option<crate::miracles::Grimoire>,
    #[serde(default)]
    cooldowns: Option<crate::miracles::Cooldowns>,
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
    /// Worn ground: (cell x, cell z, wear).
    #[serde(default)]
    trails: Vec<(i32, i32, f32)>,
    /// Ground stripped of its tree or boulder for good.
    #[serde(default)]
    stripped: Vec<(i32, i32)>,
    /// The larder by kind: berries, fish, meat, grain, bread. Older saves
    /// carried only the total (stores.0), which returns as berries.
    #[serde(default)]
    larder: Option<[f32; 5]>,
    /// Ore, iron, clay in the store.
    #[serde(default)]
    metals: (f32, f32, f32),
    /// Placed deposits still in the ground: (kind, position, amount).
    #[serde(default)]
    deposits: Vec<(u8, Vec3, f32)>,
    /// Incense and dye in the store.
    #[serde(default)]
    sacred: (f32, f32),
}

fn slots_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let base = if cfg!(target_os = "macos") {
        format!("{home}/Library/Application Support/Divus Factus/saves")
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(|a| format!("{a}/Divus Factus/saves"))
            .unwrap_or_else(|_| "saves".into())
    } else {
        format!("{home}/.local/share/divus-factus/saves")
    };
    std::path::PathBuf::from(base)
}

fn slot_path(slot: u8) -> std::path::PathBuf {
    slots_dir().join(format!("slot{slot}.json"))
}

// ------------------------------------------------------------- the requests

#[derive(Resource)]
pub struct PendingSave(pub u8);

/// A request to abandon the current world and found a new one on a fresh
/// seed — the pause menu's Title door. Processed by `process_requests`.
#[derive(Resource)]
pub struct PendingNewWorld;

#[derive(Resource)]
pub struct PendingLoad(pub u8);

fn request_pending(
    save: Option<Res<PendingSave>>,
    load: Option<Res<PendingLoad>>,
    anew: Option<Res<PendingNewWorld>>,
) -> bool {
    save.is_some() || load.is_some() || anew.is_some()
}

fn process_requests(world: &mut World) {
    // The pause menu's Title door: abandon this world, found a fresh one for
    // the title to overlook. Handled before the on-title guard below — by the
    // time this runs, the state machine is already walking back to Title.
    if world.remove_resource::<PendingNewWorld>().is_some() {
        found_anew(world);
    }
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
    // A load requested on the title screen is NOT consumed here: it waits
    // for enter_on_title_load to walk the state machine through the
    // Loading door, and is honoured on the next frame, once the title has
    // been left. Consuming it while still on the title restored the world
    // invisibly behind the title screen and left the player stranded on
    // it — the LOAD button that "did nothing". (Which of the two systems
    // saw the request first was down to nondeterministic system order.)
    let on_title = world
        .get_resource::<State<crate::GameState>>()
        .is_some_and(|state| matches!(state.get(), crate::GameState::Title));
    if on_title {
        return;
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
    let faith_history = world
        .get_resource::<crate::villager::belief::FaithHistory>()
        .cloned()
        .unwrap_or_default();
    let site = world.get_resource::<SettlementSite>()?;
    let settlement_entity = site.settlement;
    let banner = world
        .get::<Settlement>(settlement_entity)
        .map(|s| (s.banner_ramp as u32, s.sigil as u32));
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
        .map(|s| (s.food(), s.timber, s.stone))
        .unwrap_or((0.0, 0.0, 0.0));
    let larder = world.get::<Stockpile>(settlement_entity).map(|s| {
        [
            s.larder.berries,
            s.larder.fish,
            s.larder.meat,
            s.larder.grain,
            s.larder.bread,
        ]
    });
    let metals = world
        .get::<Stockpile>(settlement_entity)
        .map_or((0.0, 0.0, 0.0), |s| (s.ore, s.iron, s.clay));
    let sacred = world
        .get::<Stockpile>(settlement_entity)
        .map_or((0.0, 0.0), |s| (s.incense, s.dye));
    let deposits: Vec<(u8, Vec3, f32)> = world
        .query::<(&GlobalTransform, &crate::matter::Deposit)>()
        .iter(world)
        .map(|(at, deposit)| {
            (
                match deposit.kind {
                    crate::matter::DepositKind::Iron => 0,
                    crate::matter::DepositKind::Clay => 1,
                    crate::matter::DepositKind::Stone => 2,
                },
                at.translation(),
                deposit.amount,
            )
        })
        .collect();

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
    let prayer_receipts = world
        .resource::<crate::villager::belief::PrayerLedger>()
        .closed
        .clone();
    let hotbar = Some(world.resource::<crate::miracles::Hotbar>().clone());
    let grimoire = Some(world.resource::<crate::miracles::Grimoire>().clone());
    let cooldowns = Some(world.resource::<crate::miracles::Cooldowns>().clone());
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
                PileKind::Clay => 3,
                PileKind::Ore => 4,
            };
            (kind, t.translation, t.rotation)
        })
        .collect();

    // Buildings first, so homes can point into the list.
    let mut building_ids: Vec<Entity> = Vec::new();
    let mut buildings: Vec<BuildingSave> = Vec::new();
    for (entity, transform, blueprint, building, site_state, homestead) in world
        .query::<(
            Entity,
            &Transform,
            &Blueprint,
            Option<&Building>,
            Option<&ConstructionSite>,
            Has<Homestead>,
        )>()
        .iter(world)
    {
        let (done, progress, stage, stone_laid, timber_footing) = match (building, site_state) {
            (Some(_), _) => (true, 0.0, 2, 0.0, false),
            (None, Some(cs)) => (
                false,
                cs.progress,
                cs.stage,
                cs.stone_laid,
                cs.timber_footing,
            ),
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
            timber_footing,
            homestead,
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
        Option<crate::villager::work::Skills>,
        f32,
        Option<f32>,
        Option<f32>,
        (
            Option<Entity>,
            Option<(Entity, Entity)>,
            Option<Entity>,
            u32,
            Vec<(Entity, f32)>,
            bool,
        ),
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
                    Option<&crate::villager::work::Skills>,
                    Option<&Vitality>,
                    Option<&Prime>,
                    Option<&Childhood>,
                ),
                (
                    Option<&Spouse>,
                    Option<&Parentage>,
                    Option<&crate::villager::home::Home>,
                    Option<&crate::villager::Motherhood>,
                    Option<&crate::villager::regard::Regard>,
                    Has<crate::villager::civic::Mayor>,
                ),
            ), (With<Villager>, Without<Corpse>)>()
            .iter(world)
    {
        let (faith, traits, chronicle, vocation, skills, vitality, prime, childhood) = extras;
        let (spouse, parentage, home, motherhood, regard, mayor) = ties;
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
            skills.cloned(),
            vitality.map_or(0.0, |v| v.harm),
            prime.map(|p| p.remaining),
            childhood.map(|c| c.remaining),
            (
                spouse.map(|s| s.0),
                parentage.map(|p| (p.mother, p.father)),
                home.map(|h| h.0),
                motherhood.map_or(0, |m| m.borne),
                regard.map_or_else(Vec::new, |r| {
                    r.bonds.iter().map(|b| (b.toward, b.warmth)).collect()
                }),
                mayor,
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
                skills,
                harm,
                prime,
                childhood,
                (spouse, parents, home, borne, bonds, mayor),
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
                    skills,
                    harm,
                    prime,
                    childhood,
                    spouse: spouse.and_then(|e| index_of(e, &people_ids)),
                    mother: parents.and_then(|(m, _)| index_of(m, &people_ids)),
                    father: parents.and_then(|(_, f)| index_of(f, &people_ids)),
                    home: home.and_then(|e| index_of(e, &building_ids)),
                    borne,
                    // Feelings toward the dead or the vanished are not
                    // carried across a save: only indexable souls keep
                    // their place in a heart.
                    bonds: bonds
                        .into_iter()
                        .filter_map(|(e, w)| index_of(e, &people_ids).map(|i| (i, w)))
                        .collect(),
                    mayor,
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
                PileKind::Clay => 3,
                PileKind::Ore => 4,
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
        faith_history,
        banner,
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
        prayer_receipts,
        hotbar,
        grimoire,
        cooldowns,
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
        trails: world
            .get_resource::<crate::trails::Trails>()
            .map_or_else(Vec::new, |t| t.export()),
        stripped: world
            .get_resource::<crate::scatter::StrippedGround>()
            .map_or_else(Vec::new, |s| s.0.iter().map(|c| (c.x, c.y)).collect()),
        larder,
        metals,
        deposits,
        sacred,
    })
}

// --------------------------------------------------------------- restoring

/// Sweeps the living world away: every creature, building, chunk and loose
/// stone, plus whatever the hand or camera was holding onto. Step one of any
/// world replacement — loading a save, or founding a new world from the
/// pause menu's Title door.
fn raze(world: &mut World) {
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
    sweep!(crate::matter::Deposit);
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
    // The RESOURCES that point into the world, not just the entities in it.
    //
    // Razing the settlement's entities while leaving `SettlementSite` behind
    // left a resource holding a despawned entity, and the title screen reads
    // exactly that resource to decide whether Begin resumes a game or starts
    // one. So an abandoned world's site made the next Begin skip the founding
    // altogether. Whoever razes the world owns clearing these too.
    world.remove_resource::<SettlementSite>();
    world.remove_resource::<crate::villager::DivineName>();
    // The surveyed vantage belongs to the terrain it was surveyed ON. It is
    // computed once, guarded by its own absence, so leaving it behind meant the
    // new world never got surveyed at all and the opening dive came down on the
    // OLD world's likeliest ground - which in a fresh world is wherever that
    // happens to be. Brett arrived in the middle of an ocean.
    world.remove_resource::<crate::founding::OpeningVantage>();
    if let Some(mut chosen) = world.get_resource_mut::<crate::villager::ChosenGround>() {
        chosen.0 = None;
    }
}

/// Tears the current world down and founds a brand-new one on a fresh seed.
///
/// This is what makes the pause menu's Title button honest: the vantage the
/// title then overlooks is a NEW world, so Begin is a true new game rather
/// than a descent back into the one just abandoned.
fn found_anew(world: &mut World) {
    raze(world);

    // A fresh seed from the clock — the same door a fresh launch walks
    // through, so DIVUS_FACTUS_SEED still pins reproducible worlds.
    let seed = crate::WorldSeed::default();
    world.insert_resource(Terrain::new(seed.0));
    world.insert_resource(seed);
    // The clock's own default, not zero: games open mid-morning, and raw
    // zero is the small hours - returning to the title showed a new world
    // sunk in a night nobody asked for.
    world.insert_resource(crate::calendar::WorldClock::default());

    // Everything the old world wrote into the annals, blank again.
    world.insert_resource(crate::villager::explore::KnownWorld::default());
    world.insert_resource(crate::villager::WorldChronicle::default());
    world.insert_resource(crate::villager::work::KitchenWarm::default());
    if let Some(mut trails) = world.get_resource_mut::<crate::trails::Trails>() {
        trails.restore(std::iter::empty());
    }
    if let Some(mut stripped) = world.get_resource_mut::<crate::scatter::StrippedGround>() {
        stripped.0.clear();
    }
    if let Some(mut groves) = world.get_resource_mut::<crate::scatter::DirtyGroves>() {
        groves.0.clear();
    }
    world.insert_resource(crate::weather::Weather::default());
    world.insert_resource(Belief::default());
    world.insert_resource(Legend::default());
    world.insert_resource(crate::villager::belief::FaithHistory::default());
    // The god's portrait snaps to the new world's blank slate rather than
    // easing out of the abandoned world's temperament.
    if let Some(mut manifest) = world.get_resource_mut::<crate::debug::Manifestation>() {
        manifest.arrive();
    }

    // And NOTHING is founded. This is where the bug was: it used to found a
    // settlement here, "the same way a fresh launch does", which was true when
    // a fresh launch founded one at startup. It has not been true since the god
    // started planting the first village themselves — a launch now opens on an
    // empty world with a flag in hand.
    //
    // So going to the title and pressing Begin walked into a world that had
    // already been founded, at the ABANDONED world's chosen ground (that
    // resource outlived the raze too), and `Begin` seeing a settlement sent the
    // player to `Playing` rather than `Choosing`. Brett: it should reset "as if
    // its a new game."
    //
    // A new game is an empty world. The founding belongs to the flag, and the
    // flag belongs to the player.
}

fn apply(world: &mut World, save: SaveGame) {
    // 1. Clear the dynamic world.
    raze(world);

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
        banner: save
            .banner
            .map(|(ramp, sigil)| (ramp as usize, sigil as usize)),
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
        // Old saves kept one food number; it comes back as berries.
        store.larder = match save.larder {
            Some([berries, fish, meat, grain, bread]) => crate::villager::work::Larder {
                berries,
                fish,
                meat,
                grain,
                bread,
            },
            None => crate::villager::work::Larder {
                berries: save.stores.0,
                ..Default::default()
            },
        };
        store.timber = save.stores.1;
        store.stone = save.stores.2;
        store.ore = save.metals.0;
        store.iron = save.metals.1;
        store.clay = save.metals.2;
        store.incense = save.sacred.0;
        store.dye = save.sacred.1;
    }
    for (pile, mut transform) in world
        .query::<(&StorePile, &mut Transform)>()
        .iter_mut(world)
    {
        let kind = match pile.0 {
            PileKind::Food => 0,
            PileKind::Timber => 1,
            PileKind::Stone => 2,
            PileKind::Clay => 3,
            PileKind::Ore => 4,
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
    if let Some(mut trails) = world.get_resource_mut::<crate::trails::Trails>() {
        trails.restore(save.trails.iter().copied());
    }
    if let Some(mut stripped) = world.get_resource_mut::<crate::scatter::StrippedGround>() {
        stripped.0 = save
            .stripped
            .iter()
            .map(|&(x, z)| IVec2::new(x, z))
            .collect();
    }
    world.insert_resource(save.faith_history.clone());
    world.insert_resource(crate::weather::Weather {
        intensity: save.weather.0,
        target: save.weather.1,
        next_front: save.weather.2,
        wind: 0.15 + save.weather.0 * 0.85,
        // Re-derived from the calendar on the next frame.
        chill: 0.0,
    });
    // The bar economy: older saves carry none and get the founding kit.
    world.insert_resource(save.hotbar.clone().unwrap_or_default());
    world.insert_resource(save.grimoire.clone().unwrap_or_default());
    world.insert_resource(save.cooldowns.clone().unwrap_or_default());
    world.insert_resource(Belief {
        total: save.belief.0,
        spent: save.belief.1,
    });
    world.insert_resource(crate::villager::belief::PrayerLedger {
        closed: save.prayer_receipts.clone(),
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
    // The god's portrait snaps to the restored legend rather than easing
    // out of whatever the previous world had made of it.
    if let Some(mut manifest) = world.get_resource_mut::<crate::debug::Manifestation>() {
        manifest.arrive();
    }

    // 5. Buildings, replayed stage by stage.
    let mut building_ids: Vec<Entity> = Vec::new();
    for b in &save.buildings {
        let entity = {
            let mut commands = world.commands();
            let entity = commands
                .spawn((
                    b.blueprint.clone(),
                    // Which town raised it. Restored the same way it is
                    // stamped when ground is broken, so a loaded world's
                    // trades serve their own settlement and not every one.
                    MemberOf(settlement_entity),
                    Transform::from_translation(b.pos).with_rotation(b.rot),
                    Visibility::default(),
                    crate::hand::PickRadius(b.blueprint.half_w.max(b.blueprint.half_d) + 0.9),
                    crate::hand::Rooted,
                    // One piece, wherever on the sphere the save was made.
                    crate::globe::RigidlySeated,
                ))
                .id();
            entity
        };
        world.flush();
        let kind = b.blueprint.kind;
        // Read before the commands borrow the world: a loaded plot gets a
        // fresh grace, since the clock it was last touched by belonged to
        // another session and a saved town should not open with its
        // builds already written off as abandoned.
        let loaded_at = world
            .get_resource::<crate::calendar::WorldClock>()
            .map_or(0.0, |clock| clock.elapsed);
        {
            let mut commands = world.commands();
            if b.done {
                commands
                    .entity(entity)
                    .insert((Building { kind }, Name::new(kind.name())));
                match kind {
                    BuildingKind::House => {
                        commands.entity(entity).insert(Hut);
                    }
                    BuildingKind::Longhouse => {
                        commands.entity(entity).insert(Longhouse);
                    }
                    _ => {}
                }
                // A holding out past the rings stays a holding on reload.
                if b.homestead {
                    commands.entity(entity).insert(Homestead);
                }
            } else {
                commands.entity(entity).insert((
                    Name::new(format!("{}, rising", kind.name())),
                    crate::globe::RigidlySeated,
                    ConstructionSite {
                        progress: b.progress,
                        stage: b.stage,
                        stone_laid: b.stone_laid,
                        timber_footing: b.timber_footing,
                        last_hand: loaded_at,
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
                // The mason's plinth and steps, if the foundation was laid -
                // through the mason's own reveal, so a reload cannot dress a
                // building differently than the build did. (It also knows a
                // carried-in drawing gets nothing: the drawing's stages are
                // the whole of the building. The copy of the slab that used
                // to live here predated that rule and re-wrapped every baked
                // house on every load.)
                let cost = b.blueprint.kind.stone_cost();
                if cost > 0.0 && (b.done || b.stone_laid >= cost) {
                    crate::villager::work::reveal_foundation(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        entity,
                        &b.blueprint,
                    );
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
            if p.mayor {
                e.insert(crate::villager::civic::Mayor(settlement_entity));
            }
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
            if let Some(skills) = &p.skills {
                e.insert(skills.clone());
            }
            if let Some(remaining) = p.prime {
                e.insert(Prime { remaining });
            }
            if let Some(remaining) = p.childhood {
                e.insert(Childhood { remaining });
            }
            // A mother's history of births, so her fertility resumes where it
            // left off rather than starting over on load.
            if p.borne > 0 {
                e.insert(crate::villager::Motherhood { borne: p.borne });
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
        if !p.bonds.is_empty() {
            e.insert(crate::villager::regard::Regard {
                bonds: p
                    .bonds
                    .iter()
                    .filter_map(|(i, warmth)| {
                        people_ids.get(*i).copied().map(|toward| {
                            crate::villager::regard::Bond {
                                toward,
                                warmth: *warmth,
                                // A grudge keeps its heat across a save and
                                // loses its citation - how old wounds work.
                                over: None,
                            }
                        })
                    })
                    .collect(),
            });
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
                    let roll = crate::scatter::roll_rock(&mut rng.0);
                    let boulder = crate::scatter::spawn_boulder(
                        &mut commands,
                        &mut meshes,
                        material.clone(),
                        *pos,
                        &mut rng.0,
                        roll,
                        None,
                    );
                    commands.entity(boulder).insert(SavedBoulder);
                    commands.entity(boulder).entry::<Transform>().and_modify({
                        let scale = *scale;
                        move |mut t| t.scale = scale
                    });
                }
                // The deposits come back with exactly what was left in
                // the ground; a worked-out vein stays worked out.
                for (kind, pos, amount) in &save.deposits {
                    crate::matter::spawn_deposit(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        *pos,
                        match kind {
                            0 => crate::matter::DepositKind::Iron,
                            2 => crate::matter::DepositKind::Stone,
                            // Clay is the fallback rather than a case, so a
                            // save written by a later build with a kind this
                            // one has never heard of comes back as SOMETHING
                            // in the ground instead of refusing to load.
                            _ => crate::matter::DepositKind::Clay,
                        },
                        *amount,
                    );
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
/// Only registered under DIVUS_FACTUS_SAVE_TEST; greppable in the log.
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

/// Presses the title screen's LOAD button from the environment: requests
/// slot 3 while still on the title, then reports each state the flow
/// passes through. Only registered under DIVUS_FACTUS_TITLE_LOAD_TEST.
fn title_load_test_harness(
    mut commands: Commands,
    time: Res<Time<Real>>,
    state: Res<State<crate::GameState>>,
    mut fired: Local<bool>,
    mut last: Local<Option<crate::GameState>>,
) {
    if *last != Some(*state.get()) {
        *last = Some(*state.get());
        info!("TITLE_LOAD_TEST: state is now {:?}", state.get());
    }
    if !*fired && time.elapsed_secs() > 3.0 {
        *fired = true;
        info!("TITLE_LOAD_TEST: requesting load of slot 3 from the title");
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
    mut commands: Commands,
    state: Res<State<crate::GameState>>,
    pending: Option<Res<PendingLoad>>,
    mut next: ResMut<NextState<crate::GameState>>,
    mut panels: Query<&mut Visibility, With<SavesPanel>>,
    mut notices: MessageWriter<ui::Notice>,
) {
    let Some(pending) = pending else {
        return;
    };
    if !matches!(state.get(), crate::GameState::Title) {
        return;
    }
    // Prove the save reads before leaving the title: the Loading door
    // begins a fresh world when nothing loads, and a failed load must
    // never quietly become a new game.
    if let Err(why) = read_slot(pending.0) {
        commands.remove_resource::<PendingLoad>();
        notices.write(ui::Notice::new(format!("Could not load: {why}")));
        return;
    }
    next.set(crate::GameState::Loading);
    for mut visibility in &mut panels {
        *visibility = Visibility::Hidden;
    }
}

/// A slot's card line, read tolerantly. A save written by an older
/// version of the game may no longer parse as a whole SaveGame, but its
/// label fields are still sitting right there in the file — and a slot
/// with a world in it must never claim to be empty.
fn label_line(slot: u8, raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return format!("Slot {slot} - an unreadable save");
    };
    let day = value.get("label_day").and_then(|v| v.as_u64());
    let name = value.get("label_settlement").and_then(|v| v.as_str());
    let souls = value.get("label_souls").and_then(|v| v.as_u64());
    match (day, name, souls) {
        (Some(day), Some(name), Some(souls)) => {
            format!("Slot {slot} - day {day}, {name} ({souls} souls)")
        }
        _ => format!("Slot {slot} - a saved world"),
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
        // Held at the threshold while hidden, so the first open frame
        // refreshes at once instead of flashing "empty" for a second.
        *since_last = 1.0;
        return;
    }
    *since_last += time.delta_secs();
    if *since_last < 1.0 {
        return;
    }
    *since_last = 0.0;
    for (slot, mut text) in &mut labels {
        // Only a missing file is "empty"; anything else in the slot gets
        // the most honest label its contents allow.
        let fresh = match std::fs::read_to_string(slot_path(slot.0)) {
            Err(_) => format!("Slot {} - empty", slot.0),
            Ok(raw) => label_line(slot.0, &raw),
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world with a village in it, near enough for the teardown to work on.
    ///
    /// Only what `raze` and `found_anew` actually reach for — the point is the
    /// RESOURCES that outlive a razing, which is where the bug lived.
    fn a_world_mid_game() -> World {
        let mut world = World::new();
        world.insert_resource(crate::terrain::LoadedChunks::default());
        world.insert_resource(crate::grass::GrassChunks::default());
        world.insert_resource(crate::hand::DivineHand::default());
        world.insert_resource(crate::villager::ChosenGround(Some(Vec3::new(
            70.0, 0.0, 9.0,
        ))));
        world.insert_resource(crate::villager::DivineName("Hesh".to_string()));
        let settlement = world.spawn_empty().id();
        world.insert_resource(SettlementSite {
            centre: Vec3::new(70.0, 0.0, 9.0),
            radius: 170.0,
            woodpile: Vec3::new(74.0, 0.0, 11.0),
            settlement,
        });
        world
    }

    #[test]
    fn going_to_the_title_leaves_nothing_of_the_old_world_behind() {
        let mut world = a_world_mid_game();
        raze(&mut world);

        // The bug, in one assertion. `SettlementSite` survived the razing, so it
        // held a despawned entity - and the title reads exactly this to decide
        // whether Begin resumes a game or starts one, which is why Begin used to
        // skip the founding entirely.
        assert!(
            world.get_resource::<SettlementSite>().is_none(),
            "the settlement's site outlived the world it was in, so the next \
             Begin will resume the abandoned game instead of starting a new one"
        );
        assert_eq!(
            world.resource::<crate::villager::ChosenGround>().0,
            None,
            "the ground the old god chose is still chosen, so a new world would \
             be founded at the abandoned world's spot"
        );
        assert!(
            world
                .get_resource::<crate::villager::DivineName>()
                .is_none(),
            "the new world's god inherited the old one's name"
        );
    }

    #[test]
    fn a_slot_with_a_world_in_it_never_reads_as_empty() {
        // A healthy save labels in full.
        let healthy = r#"{"label_day": 12, "label_settlement": "Deire", "label_souls": 14}"#;
        assert_eq!(label_line(1, healthy), "Slot 1 - day 12, Deire (14 souls)");

        // A save from an older age of the game — label fields renamed or
        // missing — still admits a world is in there.
        let elder = r#"{"seed": 7, "elapsed": 900.0}"#;
        assert_eq!(label_line(2, elder), "Slot 2 - a saved world");

        // Even a half-written file owns up instead of claiming vacancy.
        let torn = r#"{"label_day": 12, "label_set"#;
        assert_eq!(label_line(3, torn), "Slot 3 - an unreadable save");
    }
}
