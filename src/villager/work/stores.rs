//! The stockpile and everything that moves through it: the larder by
//! kind, the piles, the smelter, the bakery, the weaver's vats, and the
//! meals drawn at the banner.

use bevy::prelude::*;

use super::*;
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

/// Whether a given town has finished works of a kind.
///
/// The question every per-town industry asks, and the reason buildings carry
/// a [`crate::villager::MemberOf`]: asked globally, one town's blacksmith
/// sharpened every town's tools and one town's bakery fed the whole map.
fn has_works(
    buildings: &Query<(&Building, &crate::villager::MemberOf)>,
    town: Entity,
    kind: BuildingKind,
) -> bool {
    buildings
        .iter()
        .any(|(building, member)| building.kind == kind && member.0 == town)
}

/// Samples the stores every couple of seconds, keeping about ninety.
pub(crate) fn track_store_trends(
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
pub(crate) fn shoulder_sack(
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
pub(crate) fn stores_move_indoors(
    mut commands: Commands,
    stores: Query<&Stockpile>,
    mut notices: MessageWriter<crate::ui::Notice>,
    new_buildings: Query<(&Transform, &Building, &crate::villager::MemberOf), Added<Building>>,
    standing: Query<(&Building, &crate::villager::MemberOf)>,
    piles: Query<(Entity, &StorePile, &crate::villager::MemberOf), Without<Rehouse>>,
) {
    for (at, building, owner) in &new_buildings {
        // A storehouse shelters its OWN town's piles. Without this, the first
        // colony to raise one would have reorganised the mother town's square
        // from across the map.
        let town = owner.0;
        let Ok(store) = stores.get(town) else {
            continue;
        };
        match building.kind {
            BuildingKind::Storehouse => {
                let granary_stands = standing
                    .iter()
                    .any(|(b, m)| b.kind == BuildingKind::Granary && m.0 == town);
                for (pile, kind, pile_owner) in &piles {
                    if pile_owner.0 != town {
                        continue;
                    }
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
                for (pile, kind, pile_owner) in &piles {
                    if pile_owner.0 != town || kind.0 != PileKind::Food {
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
pub(crate) fn rehouse_stores(
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
pub(crate) fn update_store_piles(
    stores: Query<&Stockpile>,
    owners: Query<&crate::villager::MemberOf>,
    mut blocks: Query<(&StonePileBlock, &ChildOf, &mut Visibility), Without<FoodSack>>,
    mut sacks: Query<(&FoodSack, &ChildOf, &mut Visibility), Without<StonePileBlock>>,
) {
    // A block belongs to a pile and a pile to a town: every settlement's
    // square shows its own stores.
    let store_of = |parent: Entity| {
        owners
            .get(parent)
            .ok()
            .and_then(|member| stores.get(member.0).ok())
    };
    for (block, parent, mut visibility) in &mut blocks {
        let Some(store) = store_of(parent.parent()) else {
            continue;
        };
        let wanted = if (block.0 as f32) < store.stone.min(12.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
    for (sack, parent, mut visibility) in &mut sacks {
        let Some(store) = store_of(parent.parent()) else {
            continue;
        };
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

/// The hungry eat from the store when the bushes cannot feed them.
///
/// This is what the stockpile is *for*: the difference between a bad berry
/// season and a funeral.
/// The famine watch: when the larder runs thin, the village says WHY out
/// loud - too few food hands, land picked bare, the store draining
/// faster than it fills - so a starvation is never a mystery, only a
/// story the god read too late.
#[allow(clippy::type_complexity)]
pub(crate) fn famine_watch(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut last_said: Local<std::collections::HashMap<Entity, String>>,
    towns: Query<(Entity, &crate::villager::SettlementGround, &Stockpile)>,
    members: Query<&crate::villager::MemberOf>,
    settlements: Query<&crate::villager::Settlement>,
    trends: Res<StoreTrends>,
    folk: Query<
        (Entity, &Transform, Option<&Vocation>),
        (
            With<Villager>,
            Without<Corpse>,
            Without<crate::creature::Held>,
        ),
    >,
    bushes: Query<(&GlobalTransform, &FoodSource)>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    *since_last += time.delta_secs();
    if *since_last < 40.0 {
        return;
    }
    *since_last = 0.0;
    // Every town keeps its own watch. A colony can starve while its mother
    // town is fat, and the ledger has to say which one is in trouble.
    for (town, ground, store) in &towns {
        let of_town = |who: Entity| members.get(who).is_ok_and(|m| m.0 == town);
        let mouths = folk.iter().filter(|(who, ..)| of_town(*who)).count();
        if mouths == 0 {
            continue;
        }
        if store.food() >= mouths as f32 * 1.5 {
            last_said.remove(&town);
            continue;
        }
        let village = settlements
            .get(town)
            .map_or("the village", |s| s.name.as_str());
        let hands = folk
            .iter()
            .filter(|(who, ..)| of_town(*who))
            .filter(|(_, _, vocation)| {
                matches!(
                    vocation,
                    Some(Vocation::Fisher)
                        | Some(Vocation::Gatherer)
                        | Some(Vocation::Hunter)
                        | Some(Vocation::Farmer)
                )
            })
            .count();
        let fruiting_near = bushes
            .iter()
            .filter(|(at, bush)| {
                bush.amount > 0.3 && at.translation().distance(ground.centre) < 170.0
            })
            .count();
        let nearest_berries = bushes
            .iter()
            .filter(|(_, bush)| bush.amount > 0.3)
            .map(|(at, _)| at.translation().distance(ground.centre) as u32)
            .min();
        let rate = trends.rate_per_minute(PileKind::Food);

        let line = if hands == 0 {
            format!("famine watch: nobody in {village} works a food trade")
        } else if fruiting_near == 0 {
            match nearest_berries {
                Some(d) => format!(
                    "famine watch: the near land is picked bare - the closest berries stand {d} strides from {village}"
                ),
                None => {
                    format!("famine watch: not a fruiting bush remains anywhere near {village}")
                }
            }
        } else if rate < -0.5 {
            format!(
                "famine watch: {village} eats {:.0} more than it gathers each minute, {hands} hands feeding {mouths} mouths",
                -rate
            )
        } else {
            // Rounded to fives so the ledger records the squeeze, not every
            // twitch of the count.
            format!(
                "famine watch: {village} holds about {:.0} food for {mouths} mouths, {hands} hands at the food trades",
                (store.food() / 5.0).round() * 5.0
            )
        };
        if last_said.get(&town) != Some(&line) {
            last_said.insert(town, line.clone());
            info!("{line}");
            notices.write(crate::ui::Notice::new(line));
        }
    }
}

/// The blacksmith at work: ore out of the far hills becomes iron, and
/// iron in the store means every trade's tools bite better - until the
/// edges dull. Mine, smelt, wear out, mine again: the first strategic
/// resource loop.
pub(crate) fn smelt(
    time: Res<Time>,
    mut since_last: Local<f32>,
    buildings: Query<(&Building, &crate::villager::MemberOf)>,
    mut towns: Query<(Entity, &mut Stockpile)>,
) {
    *since_last += time.delta_secs();
    if *since_last < 22.0 {
        return;
    }
    let interval = *since_last;
    *since_last = 0.0;
    // Every town smelts its own ore at its own forge. A blacksmith is a
    // building in ONE settlement, not a fact about the world.
    for (town, mut store) in &mut towns {
        if !has_works(&buildings, town, BuildingKind::Blacksmith) {
            continue;
        }
        if store.ore >= 1.0 {
            store.ore -= 1.0;
            store.iron += 1.0;
        }
        // Tools wear: the edge is spent slowly whenever iron is in use.
        if store.iron > 0.0 {
            store.iron = (store.iron - 0.004 * interval).max(0.0);
        }
    }
}

/// The weaver at work: dye out of the flowers becomes bright cloth on
/// the village's backs, and bright cloth is a quiet lift to every day
/// it is worn. Vanity, but vanity that keeps spirits above the line.
pub(crate) fn dye_cloth(
    time: Res<Time>,
    mut since_last: Local<f32>,
    buildings: Query<(&Building, &crate::villager::MemberOf)>,
    mut towns: Query<(Entity, &mut Stockpile)>,
    mut wearers: Query<(&mut crate::villager::Morale, &crate::villager::MemberOf), With<Villager>>,
) {
    *since_last += time.delta_secs();
    if *since_last < 45.0 {
        return;
    }
    *since_last = 0.0;
    for (town, mut store) in &mut towns {
        if !has_works(&buildings, town, BuildingKind::Weaver) {
            continue;
        }
        if store.dye < 0.3 {
            continue;
        }
        store.dye -= 0.3;
        // Bright cloth on the backs of the people who wove it, and nobody
        // else's: a neighbouring town's weaver does not dress this one.
        for (mut morale, member) in &mut wearers {
            if member.0 == town {
                morale.spirits = (morale.spirits + 0.03).min(1.0);
            }
        }
    }
}

/// The bakery at work: the store's grain becomes bread, loaf by loaf.
/// Bread stretches — a baked meal draws only three quarters as deep — so
/// a working bakery quietly makes every harvest feed more mouths.
pub(crate) fn bake(
    time: Res<Time>,
    mut since_last: Local<f32>,
    buildings: Query<(&Building, &crate::villager::MemberOf)>,
    mut towns: Query<(Entity, &mut Stockpile)>,
) {
    *since_last += time.delta_secs();
    if *since_last < 25.0 {
        return;
    }
    *since_last = 0.0;
    for (town, mut store) in &mut towns {
        if !has_works(&buildings, town, BuildingKind::Bakery) {
            continue;
        }
        if store.larder.grain >= 1.0 {
            store.larder.grain -= 1.0;
            store.larder.bread += 1.3;
        }
    }
}

pub(crate) fn eat_from_store(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    kitchen: Res<KitchenWarm>,
    members: Query<&crate::villager::MemberOf>,
    mut towns: Query<(&crate::villager::SettlementGround, &mut Stockpile)>,
    bushes: Query<(&GlobalTransform, &FoodSource)>,
    mut hungry: Query<
        (
            Entity,
            &Transform,
            &mut Needs,
            &mut crate::villager::Morale,
            &mut Activity,
            &mut MoveTarget,
            Option<&crate::villager::traits::Traits>,
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

    for (who, transform, mut needs, mut morale, mut activity, mut target, manner, last) in
        &mut hungry
    {
        // Everyone eats from their OWN town's larder and walks to their own
        // square to do it. Reaching for one global store was the thing that
        // made a second settlement impossible: two towns would have shared
        // one pile of food across the whole map.
        let Ok(&crate::villager::MemberOf(home)) = members.get(who) else {
            continue;
        };
        let Ok((ground, mut store)) = towns.get_mut(home) else {
            continue;
        };
        let centre = ground.centre;
        match *activity {
            Activity::VisitingStore => {
                if store.food() < 1.0 {
                    *activity = Activity::Idle;
                    target.0 = None;
                    continue;
                }
                if transform.translation.distance(centre) > 4.0 {
                    target.0 = Some(centre);
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
                // A second helping at the same table: one visit feeds to
                // satisfaction, instead of half the village trotting back
                // to the banner twice for every mealtime.
                if needs.hunger > 0.3 && store.food() >= 1.0 {
                    if let Some(second) = store.larder.draw(meal) {
                        needs.hunger = (needs.hunger - ration).max(0.0);
                        let _ = second;
                    }
                }
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
                    target.0 = Some(centre);
                }
            }
            _ => {}
        }
    }
}

/// A line in the log once a minute, so an unattended run leaves an account of
/// whether the village is feeding itself.
pub(crate) fn log_stores(
    time: Res<Time>,
    mut since_last: Local<f32>,
    stores: Query<(&crate::villager::Settlement, &Stockpile)>,
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
