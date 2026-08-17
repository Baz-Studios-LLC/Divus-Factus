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

/// What a town can keep food in, before its roofs are counted: sacks
/// stacked in the open beside the banner, and no more than that.
const SACKS_IN_THE_OPEN: f32 = 90.0;

/// What each roof adds to it. A granary is the big one - it is what a
/// granary IS - and a smokehouse earns its place by making meat keep.
const STOREHOUSE_KEEPS: f32 = 120.0;
const GRANARY_KEEPS: f32 = 300.0;
const SMOKEHOUSE_KEEPS: f32 = 100.0;

/// Meals a head the sacks will always hold, however small the town's
/// roofs are. The ceiling must never be the thing that starves a
/// village - see the standing law that a village thrives by default -
/// so a growing town's floor rises with its mouths whether or not
/// anyone has got round to a granary.
const MEALS_A_HEAD: f32 = 8.0;

/// How much food this town can keep from spoiling.
///
/// There was no such number for a long time and the stockpile was a pair
/// of unbounded floats: a village of twenty-two sat on ten thousand food
/// and went on gathering, because nothing ever told a gatherer to stop.
/// Brett: "that food is ridiculous, lol maybe we should curb that too."
///
/// It also gives the storehouse and the granary an actual job. They were
/// civic ambition that held nothing; now a town that wants a deeper
/// larder has to build one.
pub fn larder_ceiling(mouths: usize, storehouse: bool, granary: bool, smokehouse: bool) -> f32 {
    let roofed = SACKS_IN_THE_OPEN
        + if storehouse { STOREHOUSE_KEEPS } else { 0.0 }
        + if granary { GRANARY_KEEPS } else { 0.0 }
        + if smokehouse { SMOKEHOUSE_KEEPS } else { 0.0 };
    roofed.max(mouths as f32 * MEALS_A_HEAD)
}

/// Food past what the town can keep goes bad, kind by kind.
///
/// Trimmed here rather than refused at every place food enters, because
/// food enters from a dozen places - a haul, a kill, a bake, a miracle -
/// and one honest ceiling beats a dozen doorkeepers who might disagree.
pub(crate) fn the_sacks_hold_what_they_hold(
    time: Res<Time>,
    mut since: Local<f32>,
    mut notices: MessageWriter<crate::ui::Notice>,
    buildings: Query<(&Building, &crate::villager::MemberOf)>,
    folk: Query<&crate::villager::MemberOf, (With<Villager>, Without<Corpse>)>,
    mut towns: Query<(Entity, &crate::villager::Settlement, &mut Stockpile)>,
) {
    *since += time.delta_secs();
    if *since < 10.0 {
        return;
    }
    *since = 0.0;
    for (town, settlement, mut store) in &mut towns {
        let has = |kind: BuildingKind| {
            buildings
                .iter()
                .any(|(b, member)| b.kind == kind && member.0 == town)
        };
        let mouths = folk.iter().filter(|member| member.0 == town).count();
        let ceiling = larder_ceiling(
            mouths,
            has(BuildingKind::Storehouse),
            has(BuildingKind::Granary),
            has(BuildingKind::Smokehouse),
        );
        let held = store.food();
        if held <= ceiling {
            continue;
        }
        // Everything spoils together, in proportion, so a larder does not
        // quietly turn into nothing but bread.
        let keep = ceiling / held;
        store.larder.berries *= keep;
        store.larder.fish *= keep;
        store.larder.meat *= keep;
        store.larder.grain *= keep;
        store.larder.bread *= keep;
        let lost = held - ceiling;
        // Only worth saying when it is worth hearing: a town living at
        // its ceiling would otherwise nag every ten seconds.
        if lost > ceiling * 0.08 {
            info!(
                "{} let {lost:.0} food spoil: the town can keep {ceiling:.0} and had {held:.0}",
                settlement.name
            );
            notices.write(crate::ui::Notice::new(format!(
                "Food spoiled in {} for want of somewhere to keep it",
                settlement.name
            )));
        }
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PileKind {
    Food,
    Timber,
    Stone,
    /// Dug clay, puddled and stacked in bricks. Brett: "why is there no clay
    /// pile at the village... piles for stone, food and wood, but not clay" -
    /// a store the god can now haul deserves a face in the square.
    Clay,
    /// Raw ore off the vein, heaped dark and rust-streaked.
    Ore,
}

impl PileKind {
    /// Every kind of goods a village stacks.
    ///
    /// ONE LIST, AND THE COMPILER GUARDS IT. Add a variant above and
    /// `held_of` stops compiling until it is told how much of the new thing
    /// the village has - and this list sits beside it so the two are updated
    /// in one sitting. Everything downstream then works untouched: the
    /// pallets are dealt among whichever kinds are actually held, so a new
    /// kind simply joins the deal the first day somebody brings one home.
    /// Brett: "Even if we add something new, the pallets should just work.
    /// Minus the graphic that shows what they have of course."
    pub const fn every() -> &'static [PileKind] {
        &[
            PileKind::Food,
            PileKind::Timber,
            PileKind::Stone,
            PileKind::Clay,
            PileKind::Ore,
        ]
    }
}

/// The pallets a housed pile was dealt, as offsets from the pile's own seat.
///
/// A pile stands on the FIRST pallet it was dealt and spills onto the rest as
/// it grows. Brett: "Its okay if it fills more than one wood pallet or more
/// than one stone pallet." The offsets are local to the pile, because the pile
/// itself is moved to the first seat and its pieces are its children.
#[derive(Component, Clone, Default)]
pub struct Stacked {
    /// Offset from the pile's seat, and the room that pallet holds.
    pub seats: Vec<(Vec3, Vec3)>,
}

/// How the pieces of one kind stack: the size of a piece, and how it is laid.
///
/// A log lies along the pallet and a block sits square on it, so the two fill
/// a box differently. Read off the meshes the piles are built from.
fn piece_of(kind: PileKind) -> Vec3 {
    match kind {
        PileKind::Timber => Vec3::new(1.3, 0.21, 0.26),
        PileKind::Stone => Vec3::new(0.46, 0.35, 0.46),
        PileKind::Clay => Vec3::new(0.46, 0.24, 0.32),
        PileKind::Ore => Vec3::new(0.42, 0.3, 0.42),
        PileKind::Food => Vec3::new(0.44, 0.36, 0.44),
    }
}

/// Where the nth piece of a kind stands, given the pallets it was dealt.
///
/// Fills one pallet before starting the next, in rows across and then layers
/// up, so a stack grows the way a person would build it: a floor's worth
/// first, then another on top, then the next pallet along.
pub fn piece_seat(kind: PileKind, index: u16, stacked: &Stacked) -> Option<(Vec3, bool)> {
    let piece = piece_of(kind);
    let mut left = index as u32;
    for (offset, room) in &stacked.seats {
        // How many fit this pallet, by its drawn box rather than by any
        // number written here: the maker sized the room, and the room says
        // how much stands in it.
        let across = (room.x / piece.x).floor().max(1.0) as u32;
        let deep = (room.z / piece.z).floor().max(1.0) as u32;
        let high = (room.y / piece.y).floor().max(1.0) as u32;
        let per_layer = across * deep;
        let holds = per_layer * high;
        if left >= holds {
            left -= holds;
            continue;
        }
        let layer = left / per_layer;
        let within = left % per_layer;
        let (col, row) = (within % across, within / across);
        // Alternate the lie of each layer, the way a woodpile is actually
        // built - it is what stops a tall stack reading as a solid brick.
        let turned = layer % 2 == 1;
        let (step_x, step_z) = if turned {
            (piece.z, piece.x)
        } else {
            (piece.x, piece.z)
        };
        let spread = Vec3::new(
            (col as f32 + 0.5) * step_x - room.x * 0.5,
            (layer as f32 + 0.5) * piece.y,
            (row as f32 + 0.5) * step_z - room.z * 0.5,
        );
        return Some((*offset + spread, turned));
    }
    None
}

/// Marks a pile in the square as an inspectable face of the stockpile.
#[derive(Component)]
pub struct StorePile(pub PileKind);

/// A parcel of the stores in the god's hand: two logs, a block, a basket.
///
/// The piles are not only where offerings LAND - the god can draw from them
/// too, and carry the goods where they are wanted: a stuck frame, a hungry
/// colony. "I should be able to pick up out of the stores too." Offered
/// anywhere the offerings land, it pays back exactly what was drawn.
#[derive(Component, Debug, Clone, Copy)]
pub struct Goods {
    pub kind: PileKind,
    pub amount: f32,
}

/// One brick of the clay pile; hidden until the store holds its number.
#[derive(Component)]
pub struct ClayPileBrick(pub u8);

/// One chunk of the ore heap; likewise.
#[derive(Component)]
pub struct OrePileChunk(pub u8);

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
            // The trends ledger predates these two; they read as steady
            // until it grows columns for them.
            PileKind::Clay | PileKind::Ore => 0.0,
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
    commands.entity(carrier).insert(crate::creature::Laden);
}

/// How many armloads of one kind stand in a cubic metre of authored room.
///
/// Not one number for all five: a cubic metre of firewood is more armloads
/// than a cubic metre of ore, and the caps the game has always used already
/// said so. These are exactly those caps read as densities - so a pallet drawn
/// a metre cubed holds precisely what the hardcoded corner held, and anything
/// larger holds proportionally more. The old numbers survive as the unit
/// rather than being replaced by a guess.
fn armloads_per_cubic_metre(kind: PileKind) -> f32 {
    match kind {
        PileKind::Timber => 24.0,
        PileKind::Stone => 12.0,
        PileKind::Clay => 8.0,
        PileKind::Ore => 8.0,
        // Food stacks two to an armload, hence half of the twenty-four.
        PileKind::Food => 12.0,
    }
}

/// Every pallet a drawing sets aside, in a settled order.
///
/// Sorted by where they stand rather than by the order they happen to sit in
/// the file, so which goods land on which pallet cannot quietly change when a
/// maker moves a mark up their list. The same drawing must furnish the same
/// storehouse twice.
fn pallets_of(drawn: Option<&super::baked::Baked>) -> Vec<(Vec3, f32)> {
    let mut room: Vec<(Vec3, f32)> = drawn
        .map(|work| {
            work.marks
                .iter()
                .filter(|m| m.mark == "pallet")
                .filter_map(|m| {
                    let [long, high, deep] = m.size?;
                    Some((Vec3::from(m.at), (long * high * deep).max(0.0)))
                })
                .collect()
        })
        .unwrap_or_default();
    room.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.z.total_cmp(&b.0.z)));
    room
}

fn held_of(kind: PileKind, store: &Stockpile) -> f32 {
    match kind {
        PileKind::Timber => store.timber,
        PileKind::Stone => store.stone,
        PileKind::Clay => store.clay,
        PileKind::Ore => store.ore,
        PileKind::Food => store.food(),
    }
}

/// Which goods this roof is holding for the village right now.
///
/// The kinds with something in the pile - a pallet dealt to a kind the village
/// has none of stands empty while the timber overflows beside it. Brett: "the
/// pallets need to be flexible enough to except whatever the villagers put in
/// them, like clay for example." So the answer is asked fresh every time the
/// stores are rehoused: the day the miners bring back the first clay, clay is
/// one of the kinds and the pallets are dealt again to include it.
///
/// Food only until a granary stands, and then never again.
fn kinds_under_this_roof(store: &Stockpile, granary_stands: bool) -> Vec<PileKind> {
    PileKind::every()
        .iter()
        .copied()
        .filter(|kind| !(*kind == PileKind::Food && granary_stands))
        .filter(|kind| held_of(*kind, store) > 0.0)
        .collect()
}

/// The pallets this kind has been dealt, out of all the drawing offers.
///
/// Dealt round the table rather than sliced into blocks, so that when the
/// number of kinds changes, every kind's share changes with it and no pallet
/// is left standing empty. Brett: "if I have 6 pallets in a sotre house, maybe
/// 2 food / 2 timber / 2 stone, but after the grainery is built it would be 3
/// timber and 3 stone." Six pallets and three kinds deal two each; the granary
/// takes the food away, and the same six deal three each to the two that
/// remain. Any number of pallets works, and any number of kinds.
fn dealt_to(kind: PileKind, kinds: &[PileKind], pallets: &[(Vec3, f32)]) -> Vec<(Vec3, f32)> {
    let Some(seat) = kinds.iter().position(|k| *k == kind) else {
        return Vec::new();
    };
    pallets
        .iter()
        .skip(seat)
        .step_by(kinds.len().max(1))
        .copied()
        .collect()
}

/// Where a pile stands under the storehouse roof, and how many armloads carry
/// it there. `None` for a kind this roof does not take.
///
/// THE MAKER'S ANSWER FIRST. Opificium can author `pallet` marks as VOLUMES -
/// boxes dragged out to the room the drawing sets aside - and where a drawing
/// offers any, both answers come from them: the seat from the pallet's foot,
/// and the cap from how much room this kind was dealt. A storehouse drawn with
/// more room holds more because it was drawn that way, not because a constant
/// somewhere was raised.
///
/// Failing that, the four corners the game has always used. A drawing need
/// place no pallets at all, and a village must not stop working because a
/// drawing was incomplete.
fn storehouse_seat(
    kind: PileKind,
    store: &Stockpile,
    granary_stands: bool,
    drawn: Option<&super::baked::Baked>,
    dealt: &mut Stacked,
) -> Option<(Vec3, u8)> {
    if kind == PileKind::Food && granary_stands {
        return None;
    }
    let held = held_of(kind, store);
    let pallets = pallets_of(drawn);
    if !pallets.is_empty() {
        let kinds = kinds_under_this_roof(store, granary_stands);
        let mine = dealt_to(kind, &kinds, &pallets);
        if let Some((seat, _)) = mine.first() {
            // Kept for the stacking: the pile stands on the first pallet and
            // spills onto the rest, so it has to know where the rest are.
            *dealt = Stacked {
                seats: mine
                    .iter()
                    .map(|(at, volume)| {
                        // A cube of that volume is the box the maker drew,
                        // near enough for stacking - the bake gives the room
                        // and the room is what fills.
                        let side = volume.max(0.001).cbrt();
                        (*at - *seat, Vec3::splat(side))
                    })
                    .collect(),
            };
            // Every pallet dealt to this kind counts toward what it holds,
            // though the stack stands on the first of them: the room is the
            // room, whether it is one big pallet or three small ones.
            let room: f32 = mine.iter().map(|(_, volume)| volume).sum();
            let cap = room * armloads_per_cubic_metre(kind);
            // A mark's `at` is the middle of its FOOT, which is where a stack
            // starts, so it is the seat exactly as authored.
            return Some((*seat, (held.min(cap).max(0.0) as u8).max(1)));
        }
    }

    Some(match kind {
        PileKind::Timber => (Vec3::new(-0.9, 0.0, 0.5), store.timber.min(24.0) as u8),
        PileKind::Stone => (Vec3::new(0.9, 0.0, -0.5), store.stone.min(12.0) as u8),
        PileKind::Clay => (Vec3::new(-0.9, 0.0, -0.5), store.clay.min(8.0) as u8),
        PileKind::Ore => (Vec3::new(0.9, 0.0, 0.5), store.ore.min(8.0) as u8),
        // Food shelters here too, until a granary stands.
        PileKind::Food => (
            Vec3::new(0.0, 0.0, 0.9),
            ((store.food().min(24.0) / 2.0).ceil() as u8).max(1),
        ),
    })
}

/// Where the food sits under the granary roof, and how many sacks carry it.
///
/// FOOD HAS TWO HOMES. It shelters in the storehouse until a granary stands
/// and moves the day one does - so both buildings may offer pallets, and both
/// are asked the same way. A granary holds one thing, so every pallet it
/// offers is the food's.
fn granary_seat(store: &Stockpile, drawn: Option<&super::baked::Baked>) -> (Vec3, u8) {
    let pallets = pallets_of(drawn);
    if let Some((seat, _)) = pallets.first() {
        let room: f32 = pallets.iter().map(|(_, volume)| volume).sum();
        let cap = room * armloads_per_cubic_metre(PileKind::Food);
        return (*seat, (store.food().min(cap).max(0.0) as u8).max(1));
    }
    (
        Vec3::new(0.0, 0.0, 0.4),
        ((store.food().min(24.0) / 2.0).ceil() as u8).max(1),
    )
}

/// The drawing a standing building follows, for reading its marks.
fn drawing_for(kind: BuildingKind, plan: Option<&super::buildings::Blueprint>) -> Option<&'static super::baked::Baked> {
    let plan = plan?;
    super::baked::drawing_of(kind, plan.plan, &plan.drawing)
}

/// When the storehouse rises, the village carries its piles in under the
/// eaves - and the granary takes the food sacks. Nothing teleports: each
/// armload is walked across the square.
///
/// Two triggers, one rule — nothing lives outside a standing roof. A new
/// roof sweeps the piles that already exist, and a NEW PILE born into a
/// town whose roof already stands heads indoors the moment it appears.
/// The second trigger is the one that was missing: a town that mined its
/// first ore after the storehouse went up kept that pile at the square
/// forever. Brett: "they shouldn't live outside."
#[allow(clippy::type_complexity)]
pub(crate) fn stores_move_indoors(
    mut commands: Commands,
    stores: Query<&Stockpile>,
    mut notices: MessageWriter<crate::ui::Notice>,
    new_buildings: Query<
        (
            &Transform,
            &Building,
            &crate::villager::MemberOf,
            Option<&super::buildings::Blueprint>,
        ),
        Added<Building>,
    >,
    standing: Query<(
        &Transform,
        &Building,
        &crate::villager::MemberOf,
        Option<&super::buildings::Blueprint>,
    )>,
    piles: Query<(Entity, &StorePile, &crate::villager::MemberOf), Without<Rehouse>>,
    late_piles: Query<
        (Entity, &StorePile, &crate::villager::MemberOf),
        (Added<StorePile>, Without<Rehouse>),
    >,
) {
    // A pile born under an already-standing roof goes straight in.
    for (pile, kind, pile_owner) in &late_piles {
        let town = pile_owner.0;
        let Ok(store) = stores.get(town) else {
            continue;
        };
        let granary_stands = standing
            .iter()
            .any(|(_, b, m, _)| b.kind == BuildingKind::Granary && m.0 == town);
        let mut dealt = Stacked::default();
        let shelter = if kind.0 == PileKind::Food && granary_stands {
            standing
                .iter()
                .find(|(_, b, m, _)| b.kind == BuildingKind::Granary && m.0 == town)
                .map(|(at, _, _, plan)| {
                    let (local, goal) =
                        granary_seat(store, drawing_for(BuildingKind::Granary, plan));
                    (*at, local, goal)
                })
        } else {
            standing
                .iter()
                .find(|(_, b, m, _)| b.kind == BuildingKind::Storehouse && m.0 == town)
                .and_then(|(at, _, _, plan)| {
                    let drawn = drawing_for(BuildingKind::Storehouse, plan);
                    storehouse_seat(kind.0, store, granary_stands, drawn, &mut dealt)
                        .map(|(local, goal)| (*at, local, goal))
                })
        };
        if let Some((at, local, goal)) = shelter {
            commands.entity(pile).insert(dealt.clone());
            commands.entity(pile).insert(Rehouse {
                to: at.translation + at.rotation * local,
                to_rot: at.rotation,
                hauled: 0,
                goal: goal.max(1),
            });
        }
    }

    for (at, building, owner, plan) in &new_buildings {
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
                    .any(|(_, b, m, _)| b.kind == BuildingKind::Granary && m.0 == town);
                let drawn = drawing_for(BuildingKind::Storehouse, plan);
                for (pile, kind, pile_owner) in &piles {
                    if pile_owner.0 != town {
                        continue;
                    }
                    let mut dealt = Stacked::default();
                    let Some((local, goal)) =
                        storehouse_seat(kind.0, store, granary_stands, drawn, &mut dealt)
                    else {
                        continue;
                    };
                    commands.entity(pile).insert(dealt);
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
                let (local, goal) = granary_seat(store, drawing_for(BuildingKind::Granary, plan));
                for (pile, kind, pile_owner) in &piles {
                    if pile_owner.0 != town || kind.0 != PileKind::Food {
                        continue;
                    }
                    // The food leaves whatever storehouse pallet it was on and
                    // walks to the granary's - the same haul, to a new seat.
                    commands.entity(pile).insert(Rehouse {
                        to: at.translation + at.rotation * local,
                        to_rot: at.rotation,
                        hauled: 0,
                        goal: goal.max(1),
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
        // Finished: the pile stands at its new spot. The delivery points on
        // the town's ground follow on their own — see `pile_points_follow`.
        if rehouse.hauled >= rehouse.goal {
            pile_at.translation = rehouse.to;
            pile_at.rotation = rehouse.to_rot;
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
                        PileKind::Stone | PileKind::Clay | PileKind::Ore => {
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

/// Lays a housed pile's pieces out across the pallets it was dealt.
///
/// In the square a pile is a heap the village built itself, with its own
/// hand-set arrangement. Under a roof the maker has drawn where the goods go
/// and how much room they have, so the pieces stand where the drawing says -
/// filling one pallet before starting the next, in rows and then layers.
/// Brett: "Its okay if it fills more than one wood pallet or more than one
/// stone pallet."
///
/// Runs only when the pallets change, which is when a pile is first housed
/// and again whenever the deal changes under it - the day a granary takes the
/// food away and its pallets are shared out afresh.
#[allow(clippy::type_complexity)]
pub(crate) fn stack_on_the_pallets(
    piles: Query<(Entity, &StorePile, &Stacked), Changed<Stacked>>,
    children: Query<&Children>,
    mut pieces: Query<&mut Transform>,
    logs: Query<&WoodpileLog>,
    blocks: Query<&StonePileBlock>,
    sacks: Query<&FoodSack>,
    bricks: Query<&ClayPileBrick>,
    chunks: Query<&OrePileChunk>,
) {
    for (pile, kind, stacked) in &piles {
        if stacked.seats.is_empty() {
            continue;
        }
        let Ok(kids) = children.get(pile) else {
            continue;
        };
        for kid in kids.iter() {
            // Whichever kind of piece this pile is made of, its index is what
            // decides where it stands.
            let index = logs
                .get(kid)
                .map(|p| p.0 as u16)
                .or_else(|_| blocks.get(kid).map(|p| p.0 as u16))
                .or_else(|_| sacks.get(kid).map(|p| p.0 as u16))
                .or_else(|_| bricks.get(kid).map(|p| p.0 as u16))
                .or_else(|_| chunks.get(kid).map(|p| p.0 as u16));
            let Ok(index) = index else {
                continue;
            };
            let Some((at, turned)) = piece_seat(kind.0, index, stacked) else {
                // More pieces than the drawing has room for. They stay where
                // they were and their own system keeps them hidden, because
                // the goods that fit are the goods that show.
                continue;
            };
            if let Ok(mut spot) = pieces.get_mut(kid) {
                spot.translation = at;
                spot.rotation = if turned {
                    Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                } else {
                    Quat::IDENTITY
                };
            }
        }
    }
}

/// The stone and food stores, countable at a glance like the woodpile.
#[allow(clippy::type_complexity)]
pub(crate) fn update_store_piles(
    stores: Query<&Stockpile>,
    owners: Query<&crate::villager::MemberOf>,
    mut blocks: Query<
        (&StonePileBlock, &ChildOf, &mut Visibility),
        (
            Without<FoodSack>,
            Without<ClayPileBrick>,
            Without<OrePileChunk>,
        ),
    >,
    mut sacks: Query<
        (&FoodSack, &ChildOf, &mut Visibility),
        (
            Without<StonePileBlock>,
            Without<ClayPileBrick>,
            Without<OrePileChunk>,
        ),
    >,
    mut bricks: Query<
        (&ClayPileBrick, &ChildOf, &mut Visibility),
        (
            Without<StonePileBlock>,
            Without<FoodSack>,
            Without<OrePileChunk>,
        ),
    >,
    mut chunks: Query<
        (&OrePileChunk, &ChildOf, &mut Visibility),
        (
            Without<StonePileBlock>,
            Without<FoodSack>,
            Without<ClayPileBrick>,
        ),
    >,
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
    for (brick, parent, mut visibility) in &mut bricks {
        let Some(store) = store_of(parent.parent()) else {
            continue;
        };
        let wanted = if (brick.0 as f32) < store.clay.min(8.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
    for (chunk, parent, mut visibility) in &mut chunks {
        let Some(store) = store_of(parent.parent()) else {
            continue;
        };
        let wanted = if (chunk.0 as f32) < store.ore.min(8.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// A personal place at the table. Every diner used to aim at the sack
/// point itself, so a mealtime crowd stood inside one another, and the
/// separation push fought the walk in a visible shudder - two villagers
/// vibrating in each other's chests at the storehouse door. Same soul,
/// same seat, every meal; the ring stays inside the four-stride reach of
/// the sacks, so a seat still counts as being at the table.
fn table_seat(who: Entity, table: Vec3) -> Vec3 {
    let bits = who.to_bits();
    let angle = (bits % 12) as f32 / 12.0 * std::f32::consts::TAU;
    let ring = 1.3 + ((bits >> 4) % 3) as f32 * 0.5;
    table + Vec3::new(angle.cos() * ring, 0.0, angle.sin() * ring)
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
    bushes: Query<(&GlobalTransform, &FoodSource)>,
    folk: Query<
        (Entity, &Transform, Option<&Vocation>),
        (
            With<Villager>,
            Without<Corpse>,
            Without<crate::creature::Held>,
        ),
    >,
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
            Option<&Vocation>,
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

    for (who, transform, mut needs, mut morale, mut activity, mut target, manner, last, vocation) in
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
        // The table is wherever the food sacks stand — the square first,
        // the storehouse when one rises, the granary after that. Meals used
        // to be eaten at the square's exact centre point, and the day that
        // point went unwalkable under the hall's terraces, every route to a
        // meal was refused and six villagers starved beside a full larder.
        let table = ground.foodpile;
        match *activity {
            Activity::VisitingStore => {
                if store.food() < 1.0 {
                    // An empty larder turns away the peckish, not the
                    // desperate: whoever is starving keeps coming to the
                    // sacks anyway, because the square is where the hungry
                    // kneel and where the god looks first.
                    //
                    // EXCEPT the food trades. A hunter standing vigil at
                    // the empty sacks is how a town starves with prey
                    // twenty strides off: the moment the larder hit zero,
                    // every soul past desperate froze at this point -
                    // hunters, gatherers and fishers among them - and the
                    // village's food income went to exactly nothing.
                    // Brett, watching the pile-up: "they are all just
                    // standing by the door." Whoever can DO something
                    // about the hunger goes and does it.
                    let provider = matches!(
                        vocation,
                        Some(
                            Vocation::Fisher
                                | Vocation::Gatherer
                                | Vocation::Hunter
                                | Vocation::Farmer
                        )
                    );
                    if provider || needs.hunger < crate::villager::belief::DESPERATE_HUNGER {
                        *activity = Activity::Idle;
                        target.0 = None;
                    } else if transform.translation.distance(table) > 4.0 {
                        target.0 = Some(table_seat(who, table));
                    }
                    continue;
                }
                if transform.translation.distance(table) > 4.0 {
                    target.0 = Some(table_seat(who, table));
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
            // ANY other errand, once hunger stops being a background
            // discomfort and starts being the thing that kills them.
            //
            // This arm was `_ => {}`, and that is how people died on the
            // road. The decision to go and eat was only ever taken by
            // somebody already idle or wandering - so a forester walking
            // out to the treeline, an explorer three hundred strides into
            // the country, anyone at all with an errand in hand, simply
            // never took it. They walked on with their work while the
            // hunger ran to the top, and died with a full larder behind
            // them. Brett, twice now: "People on the road are still
            // starving."
            //
            // Deliberately later than the down-tools line, so an errand is
            // not abandoned over ordinary appetite: this is the point where
            // finishing the job stops being worth it.
            _ if needs.hunger > crate::villager::belief::DESPERATE_HUNGER
                && store.food() >= 1.0 =>
            {
                *activity = Activity::VisitingStore;
                target.0 = Some(table_seat(who, table));
            }
            Activity::Idle | Activity::Wandering => {
                // The store is the table: anyone hungry enough to put
                // their tools down comes to the sacks, bushes or no
                // bushes. Picking from the heath is the gatherer's work,
                // not everybody's dinner.
                if needs.hunger > DOWN_TOOLS_HUNGER && store.food() >= 1.0 {
                    *activity = Activity::VisitingStore;
                    target.0 = Some(table);
                }
            }
            _ => {}
        }
    }
}

/// Every town's delivery points follow its piles. The pile entities are the
/// truth — rehousing walks them under the storehouse roof, the granary takes
/// the sacks — and the points on the ground record where they stand NOW.
///
/// Before this, only the founding town's `SettlementSite` resource ever
/// followed the timber, and nothing at all followed the food: every trade
/// kept delivering armloads, gathering essence and eating meals at the bare
/// patch of square the piles had left. Brett, watching the granary rise and
/// the harvest keep landing outside it: "the storage building is for food
/// too and I think that is broken. Once built the particles should also go
/// there as well."
pub(crate) fn pile_points_follow(
    mut site: Option<ResMut<SettlementSite>>,
    piles: Query<(&StorePile, &Transform, &crate::villager::MemberOf)>,
    mut grounds: Query<&mut crate::villager::SettlementGround>,
    shells: Query<(&Transform, &super::buildings::Shell)>,
) {
    for (kind, at, owner) in &piles {
        let Ok(mut ground) = grounds.get_mut(owner.0) else {
            continue;
        };
        // The DELIVERY point, not the pile's own spot. When the sacks
        // move indoors the pile stands behind walls, and a walker sent to
        // the sacks themselves paths to a point no route can reach - the
        // route is denied, the walk is abandoned, and the walker stands
        // at the wall until the starvation watch names them. Brett, live:
        // "People still starve in sight of a stocked larder." Third time
        // this class of bug has drawn blood (the square's centre under
        // the hall's terraces; the longhouse door that opened indoors),
        // so the rule is now absolute: everyone is sent to the DOORSTEP,
        // found through the shell the same way every route out is found.
        let table = doorstep_of(at.translation, &shells);
        match kind.0 {
            PileKind::Timber => ground.woodpile = table,
            PileKind::Food => ground.foodpile = table,
            _ => continue,
        }
        if kind.0 == PileKind::Timber
            && let Some(site) = site.as_mut()
            && site.settlement == owner.0
        {
            site.woodpile = table;
        }
    }
}

/// Where deliveries and meals actually happen for a pile standing at
/// `at`: the pile's own spot when it stands under the sky, and the
/// sheltering building's outdoor door-stand when it stands behind walls.
fn doorstep_of(at: Vec3, shells: &Query<(&Transform, &super::buildings::Shell)>) -> Vec3 {
    for (housing, shell) in shells {
        // Into the building's own space; a margin so a pile against the
        // inner wall still counts as indoors.
        let local = housing.rotation.inverse() * (at - housing.translation);
        if local.x.abs() > shell.half_w + 0.5 || local.z.abs() > shell.half_d + 0.5 {
            continue;
        }
        let Some(door) = shell.doors.first() else {
            continue;
        };
        let (_, outside) = shell.door_stand(door);
        return housing.translation + housing.rotation * Vec3::new(outside.x, 0.0, outside.y);
    }
    at
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

#[cfg(test)]
mod tests {

    /// Goods fill one pallet before they start the next.
    ///
    /// Brett: "Its okay if it fills more than one wood pallet or more than one
    /// stone pallet." A pallet holds what its DRAWN BOX holds - so how many
    /// pieces spill onto the second pallet is a fact about the drawing, not a
    /// number written here.
    #[test]
    fn a_pile_fills_one_pallet_before_it_starts_the_next() {
        let stacked = Stacked {
            seats: vec![
                (Vec3::ZERO, Vec3::splat(1.0)),
                (Vec3::new(3.0, 0.0, 0.0), Vec3::splat(1.0)),
            ],
        };
        let (first, _) = piece_seat(PileKind::Stone, 0, &stacked).expect("a first block");
        assert!(
            first.x.abs() < 3.0,
            "the first block stands on the first pallet",
        );

        // Walk up the indices until one lands on the second pallet, and check
        // nothing skipped ahead to it before the first was full.
        let mut moved_at = None;
        for index in 0..200u16 {
            let Some((at, _)) = piece_seat(PileKind::Stone, index, &stacked) else {
                break;
            };
            if at.x > 1.5 && moved_at.is_none() {
                moved_at = Some(index);
            }
            if let Some(when) = moved_at {
                assert!(
                    index >= when,
                    "pieces must not go back to the first pallet once the second is started",
                );
            }
        }
        assert!(
            moved_at.is_some(),
            "enough blocks must eventually spill onto the second pallet",
        );
    }

    /// A pallet holds what fits in it, and a bigger pallet holds more.
    #[test]
    fn a_bigger_pallet_holds_more() {
        let small = Stacked {
            seats: vec![(Vec3::ZERO, Vec3::splat(1.0))],
        };
        let large = Stacked {
            seats: vec![(Vec3::ZERO, Vec3::splat(2.0))],
        };
        let holds = |stacked: &Stacked| {
            (0..500u16)
                .take_while(|i| piece_seat(PileKind::Stone, *i, stacked).is_some())
                .count()
        };
        assert!(
            holds(&large) > holds(&small),
            "two metres cubed must hold more than one",
        );
    }

    /// Six pallets and three kinds deal two each; take the food away and the
    /// same six deal three each to the two that remain.
    ///
    /// Brett's own scenario, and the reason a pallet carries no kind in its
    /// name: "if I have 6 pallets in a sotre house, maybe 2 food / 2 timber /
    /// 2 stone, but after the grainery is built it would be 3 timber and 3
    /// stone." A pallet nailed to a word by its name could not do this.
    #[test]
    fn pallets_are_dealt_again_when_the_granary_takes_the_food() {
        let pallets: Vec<(Vec3, f32)> = (0..6)
            .map(|i| (Vec3::new(i as f32, 0.0, 0.0), 1.0))
            .collect();

        let with_food = vec![PileKind::Timber, PileKind::Stone, PileKind::Food];
        for kind in &with_food {
            assert_eq!(
                dealt_to(*kind, &with_food, &pallets).len(),
                2,
                "three kinds share six pallets two apiece",
            );
        }

        let after = vec![PileKind::Timber, PileKind::Stone];
        for kind in &after {
            assert_eq!(
                dealt_to(*kind, &after, &pallets).len(),
                3,
                "the granary takes the food, and the same six pallets deal three each",
            );
        }
    }

    /// A kind nobody has any of is dealt nothing, so its pallets go to the
    /// goods that exist. The day the first clay comes home it joins the deal.
    #[test]
    fn a_kind_joins_the_deal_the_day_the_village_has_any() {
        let mut store = Stockpile::default();
        store.timber = 10.0;
        assert_eq!(
            kinds_under_this_roof(&store, false),
            vec![PileKind::Timber],
            "only what the village actually holds gets room",
        );
        store.clay = 3.0;
        let kinds = kinds_under_this_roof(&store, false);
        assert!(
            kinds.contains(&PileKind::Clay),
            "the first clay home earns the clay a pallet, with nothing changed to allow it",
        );
    }

    /// A drawing with no pallets keeps the corners the game has always used.
    /// A half-furnished storehouse must still store things.
    #[test]
    fn a_storehouse_with_no_pallets_drawn_still_stores() {
        let mut store = Stockpile::default();
        store.timber = 10.0;
        let (seat, goal) =
            storehouse_seat(PileKind::Timber, &store, false, None, &mut Stacked::default())
                .expect("timber has a seat");
        assert_eq!(seat, Vec3::new(-0.9, 0.0, 0.5));
        assert_eq!(goal, 10, "ten timber, ten armloads");
    }

    /// Every kind of goods is in `PileKind::every()`. The list is what the
    /// pallets are dealt among, so a kind missing from it would be a kind that
    /// silently never gets room - and `held_of`'s match is what makes the
    /// compiler insist the two are updated together.
    #[test]
    fn every_kind_of_goods_can_be_held() {
        let store = Stockpile::default();
        for kind in PileKind::every() {
            let _ = held_of(*kind, &store);
        }
        assert_eq!(PileKind::every().len(), 5);
    }

    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn town_with(foodpile: Vec3) -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<crate::calendar::WorldClock>();
        app.init_resource::<KitchenWarm>();
        let town = app
            .world_mut()
            .spawn((
                crate::villager::SettlementGround {
                    centre: Vec3::ZERO,
                    radius: 36.0,
                    woodpile: Vec3::ZERO,
                    foodpile,
                },
                Stockpile::default(),
            ))
            .id();
        (app, town)
    }

    /// The ceiling curbs plenty and can never cause a famine.
    ///
    /// Brett, looking at ten thousand food for twenty-two people: "that
    /// food is ridiculous, lol maybe we should curb that too." It is a
    /// curb on hoarding, not a ration: whatever the town has built, the
    /// sacks always hold several days' meals a head, because a village
    /// starving on account of its own storage rules would be the exact
    /// bug this project keeps promising never to write again.
    #[test]
    fn the_larder_ceiling_curbs_plenty_without_causing_famine() {
        let bare = larder_ceiling(10, false, false, false);
        assert!(bare >= 10.0 * 4.0, "a hamlet keeps days of meals: {bare}");
        // Building for it earns a deeper larder.
        let stored = larder_ceiling(10, true, false, false);
        let full = larder_ceiling(10, true, true, true);
        assert!(stored > bare, "a storehouse keeps more than open sacks");
        assert!(full > stored, "a granary and smokehouse keep more again");
        // And a big town is never rationed by the ceiling: mouths raise
        // the floor whether or not anyone has built anything.
        let crowd = larder_ceiling(90, false, false, false);
        assert!(
            crowd >= 90.0 * 4.0,
            "ninety souls must be able to keep days of food: {crowd}",
        );
        // But plenty is still finite - the number Brett was looking at is
        // out of reach for any town of that size.
        assert!(full < 1000.0, "hoarding stays curbed: {full}");
    }

    /// An empty larder turns away the peckish, never the starving.
    ///
    /// The starving keep coming to the sacks — the square is where the
    /// hungry kneel and where the god looks first. Sending them back to
    /// Idle bounced them between "nothing to eat here" and "go to the
    /// store" while they starved out of sight of the banner.
    #[test]
    fn the_starving_hold_vigil_at_an_empty_larder() {
        let sacks = Vec3::new(3.0, 0.0, 0.0);
        let (mut app, town) = town_with(sacks);
        let soul = app
            .world_mut()
            .spawn((
                crate::villager::Villager,
                Transform::from_xyz(63.0, 0.0, 0.0),
                Needs {
                    hunger: 0.95,
                    ..default()
                },
                crate::villager::Morale::default(),
                Activity::VisitingStore,
                crate::creature::MoveTarget::default(),
                crate::villager::MemberOf(town),
            ))
            .id();

        app.world_mut().run_system_once(eat_from_store).unwrap();
        assert_eq!(
            *app.world().get::<Activity>(soul).unwrap(),
            Activity::VisitingStore,
            "an empty larder must not turn away the starving",
        );
        // At the sacks means a personal seat WITHIN the table's reach,
        // not the sack point itself: aiming every diner at one exact
        // spot stood the whole mealtime crowd inside one another.
        let stood = app
            .world()
            .get::<crate::creature::MoveTarget>(soul)
            .unwrap()
            .0
            .expect("the vigil walks to the sacks");
        assert!(
            stood.distance(sacks) < 4.0,
            "the vigil is kept within the table's reach, not wherever hunger struck",
        );

        // The merely peckish go back to their day and wait for the trades.
        app.world_mut().get_mut::<Needs>(soul).unwrap().hunger = 0.5;
        app.world_mut().run_system_once(eat_from_store).unwrap();
        assert_eq!(
            *app.world().get::<Activity>(soul).unwrap(),
            Activity::Idle,
            "hunger the gatherers will fix should not besiege an empty larder",
        );
    }

    /// The vigil never captures a food trade. The day the larder hit
    /// zero, every soul past desperate froze at the sacks - the hunters
    /// among them - and the village's food income went to exactly
    /// nothing: prey nineteen strides out, ten founders down to six.
    /// Whoever can DO something about the hunger is sent to do it.
    #[test]
    fn the_hungry_hunter_hunts_instead_of_holding_vigil() {
        let sacks = Vec3::new(3.0, 0.0, 0.0);
        let (mut app, town) = town_with(sacks);
        let hunter = app
            .world_mut()
            .spawn((
                crate::villager::Villager,
                Transform::from_xyz(63.0, 0.0, 0.0),
                Needs {
                    hunger: 0.95,
                    ..default()
                },
                crate::villager::Morale::default(),
                Activity::VisitingStore,
                crate::creature::MoveTarget::default(),
                crate::villager::MemberOf(town),
                Vocation::Hunter,
            ))
            .id();

        app.world_mut().run_system_once(eat_from_store).unwrap();
        assert_eq!(
            *app.world().get::<Activity>(hunter).unwrap(),
            Activity::Idle,
            "a starving hunter's answer to the empty sacks is a deer, not a vigil",
        );
    }

    /// Delivery points follow the piles — into the storehouse, into the
    /// granary, wherever the rehousing walks them.
    #[test]
    fn delivery_points_follow_the_piles() {
        let (mut app, town) = town_with(Vec3::ZERO);
        app.world_mut().spawn((
            StorePile(PileKind::Food),
            Transform::from_xyz(8.0, 0.0, -3.0),
            crate::villager::MemberOf(town),
        ));
        app.world_mut().spawn((
            StorePile(PileKind::Timber),
            Transform::from_xyz(-6.0, 0.0, 2.0),
            crate::villager::MemberOf(town),
        ));

        app.world_mut().run_system_once(pile_points_follow).unwrap();

        let ground = app
            .world()
            .get::<crate::villager::SettlementGround>(town)
            .unwrap();
        assert_eq!(
            ground.foodpile,
            Vec3::new(8.0, 0.0, -3.0),
            "meals and harvests move with the sacks",
        );
        assert_eq!(
            ground.woodpile,
            Vec3::new(-6.0, 0.0, 2.0),
            "timber deliveries move with the woodpile",
        );
    }

    /// Hunger interrupts an errand, wherever the errand has taken them.
    ///
    /// The road deaths. Deciding to go and eat was only ever done by
    /// somebody already idle or wandering, so anyone with work in hand -
    /// a forester walking out to the treeline, an explorer three hundred
    /// strides into the country - walked on with it until they died, with
    /// a full larder behind them. Brett, twice: "People on the road are
    /// still starving."
    ///
    /// The threshold is what the test pins, because a rule that fires too
    /// early empties the fields over ordinary appetite and one that fires
    /// too late is what we already had.
    #[test]
    fn a_working_villager_stops_to_eat_before_starving() {
        use crate::villager::belief::DESPERATE_HUNGER;

        // Desperate is well short of dying, so there is a walk's worth of
        // life left when the errand is dropped: from here to the hunger
        // that starts killing takes minutes, and the longest road home a
        // village has takes about two.
        let seconds_of_life_left = (0.99 - DESPERATE_HUNGER) * crate::villager::SECONDS_TO_STARVE;
        let strides_home = 259.0;
        let walking = 2.4;
        assert!(
            seconds_of_life_left > strides_home / walking,
            "a villager who turns for home at {DESPERATE_HUNGER} hunger has \
             {seconds_of_life_left:.0}s of life and needs {:.0}s to walk it",
            strides_home / walking,
        );

        // And it fires later than the ordinary down-tools line, so a full
        // day's work is not abandoned over an appetite.
        assert!(
            DESPERATE_HUNGER > super::super::DOWN_TOOLS_HUNGER,
            "hunger would pull people off their work before they were hungry",
        );
    }
}
