//! The library, the scholars in it, and the tree of everything a town can work
//! out for itself.
//!
//! Brett: "maybe a library building with scholars that research to unlock new
//! things to build and upgrades and what materials things need"; then "I would
//! like a large branching tree for the research table. Towns can pick their own
//! paths through the tree, but they always must be researching something. If
//! that something requires them to go find something, like an ore or an herb for
//! example, that triggers expeditions to go get it. Maybe the scholars can even
//! point them in the right direction of the closest place to get it?"; and then
//! "this system drives a lot of the game, so it should be very deep and
//! branching. Similar to a path of exile tree maybe."
//!
//! # Why the tree is DATA and not code
//!
//! A tree of that size cannot be a `match` arm per node with a bespoke effect
//! behind each one — that is a few dozen nodes before it becomes unauthorable,
//! and the whole point of a deep tree is that it keeps growing. So:
//!
//! - **Nodes are rows** in [`THE_TREE`]: a key, a name, what it needs first,
//!   what it costs in time, what it costs in stuff, and what it gives.
//! - **Gifts are a small closed vocabulary** ([`Gift`]), and each variant is
//!   read in exactly ONE place in the game. That is the whole trick: the number
//!   of node ROWS can run to hundreds while the number of things the rest of the
//!   game has to know about stays at eight.
//! - **Keys are strings**, not enum variants, so the tree can move out to an
//!   authored asset without touching the save format or any reader. A typo in a
//!   prerequisite is caught by [`tests::the_tree_is_whole`] rather than by the
//!   compiler, which is the trade an authorable tree makes.
//! - **`Gift::Insight` nodes** carry no effect at all. Real trees need cheap
//!   connective tissue, and it is what lets a branch be long without every step
//!   of it having to matter.
//!
//! # Two phases, and why nobody ever idles
//!
//! Brett settled the shape: "Maybe a research topic 'Iron weapons' will have two
//! phases; a time gate for phase 1 and then they need to bring back iron that
//! unlocks the time gate for phase 2. Once phase two is complete the research is
//! complete." And separately, that a town "always must be researching
//! something".
//!
//! So a node with a sample runs:
//!
//! 1. **THEORY** — scholar-seconds. What can be worked out from first
//!    principles, before anybody has seen the thing.
//! 2. **THE SAMPLE** — theory runs out and the question stops. The town names
//!    what it wants, how much, and where the nearest one it knows of is. This is
//!    the want, and the want is what the library is FOR.
//! 3. **PRACTICE** — the sample arrives and is spent, and there is still work to
//!    do with it in hand.
//!
//! The fetch is the MIDDLE of a node, not the end of one, which is the whole
//! difference between an errand and a discovery — and it means a delivery is
//! followed by visible progress rather than by an instant unlock.
//!
//! A node with no sample is all theory and never blocks, which is what keeps a
//! town that can reach nothing from ever running out of things to read.
//!
//! # How a town picks its path
//!
//! Out of its own ground, mostly. A node whose sample is already on the yard
//! outranks one that needs a journey, so a town on clay banks walks down the
//! brick branch and a town under iron hills goes to the forge — without
//! anything anywhere naming a "coastal town" or a "hill town". The tie is broken
//! by a number seeded from the settlement itself, so two towns on identical
//! ground still diverge.

use bevy::prelude::*;

use super::work::{Building, BuildingKind, Stockpile, Vocation};
use super::{MemberOf, Settlement, Villager};
use crate::creature::Corpse;

/// What a node wants fetched before it can be finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Sample {
    Clay,
    Stone,
    Ore,
    /// Incense herb off a sacred stand — the first sample that is not dug.
    Herb,
    /// Dyeflowers, likewise.
    Dye,
}

impl Sample {
    pub fn word(self) -> &'static str {
        match self {
            Sample::Clay => "clay",
            Sample::Stone => "stone",
            Sample::Ore => "ore",
            Sample::Herb => "incense herb",
            Sample::Dye => "dyeflowers",
        }
    }

    pub fn held(self, store: &Stockpile) -> f32 {
        match self {
            Sample::Clay => store.clay,
            Sample::Stone => store.stone,
            Sample::Ore => store.ore,
            Sample::Herb => store.incense,
            Sample::Dye => store.dye,
        }
    }

    fn take(self, store: &mut Stockpile, amount: f32) {
        let held = match self {
            Sample::Clay => &mut store.clay,
            Sample::Stone => &mut store.stone,
            Sample::Ore => &mut store.ore,
            Sample::Herb => &mut store.incense,
            Sample::Dye => &mut store.dye,
        };
        *held = (*held - amount).max(0.0);
    }

    /// The kind of place in the world this comes out of, for the pointing.
    ///
    /// `None` is a sample the scholars cannot give directions to yet, and the
    /// want still stands — it is just unhelpful about where.
    pub fn dug_from(self) -> Option<crate::matter::DepositKind> {
        match self {
            Sample::Clay => Some(crate::matter::DepositKind::Clay),
            Sample::Stone => Some(crate::matter::DepositKind::Stone),
            Sample::Ore => Some(crate::matter::DepositKind::Iron),
            Sample::Herb | Sample::Dye => None,
        }
    }

    /// Or the kind of stand it is picked off.
    pub fn picked_from(self) -> Option<crate::scatter::SacredKind> {
        match self {
            Sample::Herb => Some(crate::scatter::SacredKind::Incense),
            Sample::Dye => Some(crate::scatter::SacredKind::Dye),
            _ => None,
        }
    }
}

/// What a node gives the town that finishes it.
///
/// A CLOSED VOCABULARY, and deliberately small. Every variant here is read in
/// exactly one place in the game, which is what keeps the cost of a hundred more
/// nodes at zero: a new row in [`THE_TREE`] is a new row, not a new hook.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gift {
    /// A building the civic planner may now want. Only ever a LATE ambition —
    /// gating a survival rung behind a book is how a village dies holding a
    /// question.
    Unlocks(BuildingKind),
    /// Logs off a felled tree.
    TimberPerTree(f32),
    /// Grain out of a brought-in harvest.
    HarvestPerField(f32),
    /// How much food the town can keep before it spoils.
    LarderKeeps(f32),
    /// Stone shouldered per trip to the face.
    StonePerTrip(f32),
    /// What a gatherer's basket holds.
    BasketSize(f32),
    /// How far afield anybody will go to work — the road itself, and the
    /// reason the far country is worth anything.
    Reach(f32),
    /// Nothing but the way onward. The connective tissue a deep tree is mostly
    /// made of.
    Insight,
}

/// How long the second half of a fetched node takes, by default.
///
/// ONE NUMBER for every node that wants a sample, because the shape being tuned
/// is the same in all of them: long enough that a delivery is followed by
/// visible work rather than an instant unlock, short enough that the delivery
/// still feels like the moment it turned. A row may write its own figure instead
/// where a node deserves one.
pub const PRACTICE: f32 = 200.0;

/// One node of the tree.
pub struct Node {
    /// Stable name, used in saves and in `requires`. Never change one.
    pub key: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    /// Keys that must be known first. Empty is a root.
    pub requires: &'static [&'static str],
    /// Scholar-seconds of reading before the sample means anything. Time makes
    /// a node SLOW; another scholar is the answer.
    pub theory: f32,
    /// What has to be brought back before the second half can start. A sample
    /// makes a node a JOURNEY, and no number of scholars substitutes for it.
    pub sample: Option<(Sample, f32)>,
    /// And the work of actually doing it, with the thing in hand. Zero is
    /// legitimate for a node that is finished the moment it is understood.
    pub practice: f32,
    pub gives: Gift,
}

/// THE TREE.
///
/// Authored as rows so it can grow without the code growing. Branches are laid
/// out in blocks below, and they cross-link on purpose — a tree whose branches
/// never meet is a set of lists.
///
/// Adding a node: add a row. Nothing else in the game changes. Adding a new KIND
/// of effect is the expensive move, and there is a checklist for it on [`Gift`].
pub const THE_TREE: &[Node] = &[
    // ── The root. Everything begins with somebody writing things down.
    Node {
        key: "letters",
        name: "Letters",
        blurb: "Marks that keep. What one scholar learns, the next one starts from.",
        requires: &[],
        theory: 180.0,
        sample: None,
        practice: 0.0,
        gives: Gift::Insight,
    },
    // ── WOOD AND CRAFT. The branch a forest town falls into.
    Node {
        key: "joinery",
        name: "Joinery",
        blurb: "A truer joint. Every tree gives up one more log.",
        requires: &["letters"],
        theory: 240.0,
        sample: None,
        practice: 0.0,
        gives: Gift::TimberPerTree(1.0),
    },
    Node {
        key: "seasoning",
        name: "Seasoning",
        blurb: "Timber stacked and left. Slower to cut, longer to stand.",
        requires: &["joinery"],
        theory: 260.0,
        sample: None,
        practice: 0.0,
        gives: Gift::Insight,
    },
    Node {
        key: "framing",
        name: "Framing",
        blurb: "The frame understood as a whole. Another log out of every trunk.",
        requires: &["seasoning"],
        theory: 340.0,
        sample: None,
        practice: 0.0,
        gives: Gift::TimberPerTree(1.0),
    },
    Node {
        key: "coopering",
        name: "Coopering",
        blurb: "Barrels that hold. The larder keeps a good deal more.",
        requires: &["seasoning"],
        theory: 300.0,
        sample: None,
        practice: 0.0,
        gives: Gift::LarderKeeps(10.0),
    },
    // ── FIELD AND LARDER. The branch a hungry town falls into.
    Node {
        key: "husbandry",
        name: "Husbandry",
        blurb: "The ground read season by season. Every harvest comes in heavier.",
        requires: &["letters"],
        theory: 260.0,
        sample: None,
        practice: 0.0,
        gives: Gift::HarvestPerField(2.0),
    },
    Node {
        key: "preserving",
        name: "Preserving",
        blurb: "Salt and smoke. The sacks keep, so the larder holds more.",
        requires: &["letters"],
        theory: 300.0,
        sample: None,
        practice: 0.0,
        gives: Gift::LarderKeeps(12.0),
    },
    Node {
        key: "rotation",
        name: "Rotation",
        blurb: "The field rested and turned. Heavier again, and the soil lasts.",
        requires: &["husbandry"],
        theory: 380.0,
        sample: None,
        practice: 0.0,
        gives: Gift::HarvestPerField(2.0),
    },
    Node {
        key: "herbcraft",
        name: "Herbcraft",
        blurb: "Which stand is worth the walk, and what to do with it after.",
        requires: &["preserving"],
        theory: 320.0,
        sample: Some((Sample::Herb, 4.0)),
        practice: PRACTICE,
        gives: Gift::Insight,
    },
    Node {
        key: "physick",
        name: "Physick",
        blurb: "Salves that work. The hurt are back on their feet sooner.",
        requires: &["herbcraft"],
        theory: 420.0,
        sample: Some((Sample::Herb, 6.0)),
        practice: PRACTICE,
        gives: Gift::Unlocks(BuildingKind::Herbalist),
    },
    // ── FORAGE AND ROAD. Where a town learns that elsewhere exists.
    Node {
        key: "foraging",
        name: "Foraging",
        blurb: "What the heath gives, and when. A fuller basket every trip.",
        requires: &["letters"],
        theory: 220.0,
        sample: None,
        practice: 0.0,
        gives: Gift::BasketSize(1.0),
    },
    Node {
        key: "pathfinding",
        name: "Pathfinding",
        blurb: "The lie of the land held in the head. People will work further out.",
        requires: &["foraging"],
        theory: 300.0,
        sample: None,
        practice: 0.0,
        gives: Gift::Reach(30.0),
    },
    Node {
        key: "wayfaring",
        name: "Wayfaring",
        blurb: "A night away is nothing to fear. The far country opens.",
        requires: &["pathfinding"],
        theory: 420.0,
        sample: None,
        practice: 0.0,
        gives: Gift::Reach(45.0),
    },
    Node {
        key: "cartage",
        name: "Cartage",
        blurb: "A barrow that rolls true. Stone comes off the face by the load.",
        requires: &["pathfinding", "joinery"],
        theory: 380.0,
        sample: None,
        practice: 0.0,
        gives: Gift::StonePerTrip(1.0),
    },
    Node {
        key: "dyeing",
        name: "Dyeing",
        blurb: "Color that holds in cloth. Warm backs and better spirits.",
        requires: &["foraging"],
        theory: 300.0,
        sample: Some((Sample::Dye, 4.0)),
        practice: PRACTICE,
        gives: Gift::Unlocks(BuildingKind::Weaver),
    },
    // ── STONE AND BRICK. The branch the ground decides.
    Node {
        key: "quarrying",
        name: "Quarrying",
        blurb: "Where the rock wants to split. More stone off the same face.",
        requires: &["letters"],
        theory: 280.0,
        sample: Some((Sample::Stone, 8.0)),
        practice: PRACTICE,
        gives: Gift::StonePerTrip(1.0),
    },
    Node {
        key: "brickwork",
        name: "Brickwork",
        blurb: "Clay fired hard. The oven becomes a thing that can be built.",
        requires: &["quarrying"],
        theory: 320.0,
        sample: Some((Sample::Clay, 6.0)),
        practice: PRACTICE,
        gives: Gift::Unlocks(BuildingKind::Bakery),
    },
    Node {
        key: "masonry",
        name: "Masonry",
        blurb: "Cut stone laid true. Ground can be walled and consecrated.",
        requires: &["quarrying"],
        theory: 400.0,
        sample: Some((Sample::Stone, 14.0)),
        practice: PRACTICE,
        gives: Gift::Unlocks(BuildingKind::Cemetery),
    },
    Node {
        key: "milling",
        name: "Milling",
        blurb: "Stone dressed to grind. Grain becomes flour becomes bread.",
        requires: &["brickwork", "husbandry"],
        theory: 420.0,
        sample: None,
        practice: 0.0,
        gives: Gift::Unlocks(BuildingKind::Mill),
    },
    Node {
        key: "vaulting",
        name: "Vaulting",
        blurb: "A roof that carries itself. Dry stores, and a great deal more of them.",
        requires: &["masonry"],
        theory: 520.0,
        sample: None,
        practice: 0.0,
        gives: Gift::LarderKeeps(18.0),
    },
    // ── FIRE AND METAL. The branch that always wants a journey.
    Node {
        key: "charcoal",
        name: "Charcoal",
        blurb: "Wood burned slow and shut in. A fire hot enough to change things.",
        requires: &["letters"],
        theory: 300.0,
        sample: None,
        practice: 0.0,
        gives: Gift::Insight,
    },
    Node {
        key: "smelting",
        name: "Smelting",
        blurb: "Rock made to give up its metal. Nobody works this out from a book.",
        requires: &["charcoal"],
        theory: 420.0,
        sample: Some((Sample::Ore, 8.0)),
        practice: PRACTICE,
        gives: Gift::Insight,
    },
    Node {
        key: "ironwork",
        name: "Iron Weapons",
        blurb: "Spearheads that hold an edge. The armory becomes worth racking.",
        requires: &["smelting"],
        theory: 520.0,
        sample: Some((Sample::Ore, 10.0)),
        practice: PRACTICE,
        gives: Gift::Unlocks(BuildingKind::Armory),
    },
    Node {
        key: "toolmaking",
        name: "Toolmaking",
        blurb: "An edge that keeps its edge. Every axe bites deeper.",
        requires: &["ironwork"],
        theory: 560.0,
        sample: Some((Sample::Ore, 8.0)),
        practice: PRACTICE,
        gives: Gift::TimberPerTree(1.0),
    },
    Node {
        key: "quarrytools",
        name: "Hardened Picks",
        blurb: "Iron at the rock face. A load and a half where a load came before.",
        requires: &["ironwork", "quarrying"],
        theory: 480.0,
        sample: None,
        practice: 0.0,
        gives: Gift::StonePerTrip(1.5),
    },
    Node {
        key: "plow",
        name: "The Plow",
        blurb: "An iron share through heavy soil. The fields answer properly.",
        requires: &["ironwork", "rotation"],
        theory: 520.0,
        sample: None,
        practice: 0.0,
        gives: Gift::HarvestPerField(3.0),
    },
];

/// A node by key.
pub fn node(key: &str) -> Option<&'static Node> {
    THE_TREE.iter().find(|node| node.key == key)
}

/// What a town's finished nodes add up to.
///
/// Resolved from `known` on demand rather than kept alongside it, so there is no
/// second copy of the truth to fall out of step. Every field here is added at
/// exactly one place in the game.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Boons {
    pub timber_per_tree: f32,
    pub harvest_per_field: f32,
    pub larder_keeps: f32,
    pub stone_per_trip: f32,
    pub basket: f32,
    pub reach: f32,
}

/// What a town knows, what it is working on, and what it is waiting for.
///
/// A component on the settlement — knowledge belongs to the TOWN, so a scholar's
/// death costs it the progress and not the learning.
#[derive(Component, Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Studies {
    /// Finished nodes, by key. Never shrinks.
    pub known: Vec<String>,
    /// The node on the table.
    pub at_hand: Option<String>,
    /// Scholar-seconds into it. Climbs whether or not the sample is home,
    /// because time and the sample are costs on different ends of a node.
    pub progress: f32,
    /// What the town is short of to get PAST the theory of what it is reading,
    /// and where the nearest one the town knows about is.
    ///
    /// THIS IS THE WANT the whole library exists to produce.
    #[serde(default)]
    pub wanting: Option<Need>,
    /// Whether the sample has arrived and been spent, so the second half is
    /// open.
    ///
    /// A FLAG AND NOT A RE-CHECK OF THE STORES, deliberately: the sample is
    /// consumed the moment it lands, so asking the stores again a minute later
    /// would find it gone and shut a node that is halfway through its practice.
    #[serde(default)]
    pub in_hand: bool,
}

/// Which half of a node a town is in. Derived, never stored — two numbers that
/// can disagree about the same thing is how a progress bar starts lying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Reading. Time is all it wants.
    Theory,
    /// Read out, and waiting on something somebody has to go and get.
    Wanting,
    /// The thing is in hand and the work is under way.
    Practice,
}

/// A named shortfall, with directions.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Need {
    pub sample: Sample,
    /// How much more, beyond what is held.
    pub short: f32,
    /// The nearest place the town knows of. `None` means the scholars know what
    /// they want and not where to get it — which is an explorer's job, not an
    /// expedition's.
    pub toward: Option<Vec3>,
}

impl Studies {
    pub fn knows(&self, key: &str) -> bool {
        self.known.iter().any(|had| had == key)
    }

    /// Whether the town may want this building yet.
    ///
    /// A kind behind no node is always permitted, so this is safe to ask about
    /// everything and the answer for the whole survival ladder is yes.
    pub fn permits(&self, kind: BuildingKind) -> bool {
        !THE_TREE
            .iter()
            .any(|node| node.gives == Gift::Unlocks(kind) && !self.knows(node.key))
    }

    /// Everything the finished nodes add up to.
    pub fn boons(&self) -> Boons {
        let mut boons = Boons::default();
        for key in &self.known {
            let Some(node) = node(key) else { continue };
            match node.gives {
                Gift::TimberPerTree(n) => boons.timber_per_tree += n,
                Gift::HarvestPerField(n) => boons.harvest_per_field += n,
                Gift::LarderKeeps(n) => boons.larder_keeps += n,
                Gift::StonePerTrip(n) => boons.stone_per_trip += n,
                Gift::BasketSize(n) => boons.basket += n,
                Gift::Reach(n) => boons.reach += n,
                Gift::Unlocks(_) | Gift::Insight => {}
            }
        }
        boons
    }

    /// Which half of the current node the town is in, if it is reading at all.
    pub fn phase(&self) -> Option<Phase> {
        let reading = self.at_hand.as_deref().and_then(node)?;
        Some(if self.progress < reading.theory {
            Phase::Theory
        } else if reading.sample.is_some() && !self.in_hand {
            Phase::Wanting
        } else {
            Phase::Practice
        })
    }

    /// How far through the CURRENT PHASE they are, for the library's window —
    /// which wants two bars and not one, since the middle of a node is a
    /// journey and no amount of reading moves it.
    pub fn share(&self) -> f32 {
        let Some(reading) = self.at_hand.as_deref().and_then(node) else {
            return 0.0;
        };
        match self.phase() {
            Some(Phase::Theory) => (self.progress / reading.theory.max(1.0)).clamp(0.0, 1.0),
            Some(Phase::Wanting) => 1.0,
            Some(Phase::Practice) => ((self.progress - reading.theory)
                / reading.practice.max(1.0))
                .clamp(0.0, 1.0),
            None => 0.0,
        }
    }
}

/// Every node whose prerequisites are met and which is not yet known.
///
/// The FRONTIER: the town's actual choice this morning, and the thing that makes
/// this a tree rather than a queue.
pub fn frontier(known: &[String]) -> Vec<&'static Node> {
    THE_TREE
        .iter()
        .filter(|node| !known.iter().any(|had| had == node.key))
        .filter(|node| {
            node.requires
                .iter()
                .all(|need| known.iter().any(|had| had == need))
        })
        .collect()
}

/// Which node a town takes up next.
///
/// TOWNS PICK THEIR OWN PATHS, and mostly their ground picks for them: a node
/// whose sample is already on the yard is worth more than one that needs a
/// journey, so a town on clay banks finds itself walking down the brick branch
/// and a town under iron hills ends up at the forge. Nothing here names a
/// "coastal town" — the preference falls out of what the stores happen to hold.
///
/// The tie is broken by a number seeded from the settlement, so two towns on
/// identical ground still diverge. Pure, so the whole rule can be checked
/// without a world.
pub fn choose(known: &[String], have: impl Fn(Sample) -> f32, seed: u64) -> Option<&'static Node> {
    let open = frontier(known);
    open.iter()
        .max_by(|a, b| {
            let worth = |node: &Node| {
                // IN HAND, OR A JOURNEY AWAY, and this term dominates - it is
                // what makes the ground pick the path. A node whose sample is
                // already on the yard is the best kind there is: finishable this
                // week, and finishable because of where the town happens to
                // stand. A journey is not refused, it is just third in line, so
                // a town reads its way through what it can reach and then finds
                // it has to go somewhere.
                let ready = match node.sample {
                    Some((sample, wanted)) if have(sample) >= wanted => 2.5,
                    None => 1.0,
                    Some(_) => 0.75,
                };
                // Cheap before dear, GENTLY and within bounds: unclamped, a
                // short node beat a whole branch on brevity alone and the
                // ground never got a say.
                let brevity = (400.0 / (node.theory + node.practice).max(1.0)).clamp(0.6, 1.4);
                // And the town's own turn of mind. Small enough that it only
                // ever settles a near-tie, large enough that two towns on the
                // same ground do not read the same books in the same order.
                //
                // BOUNDED, and the first cut was not: it shifted a 64-bit
                // product right by 11 and divided by 2^21, which is a number in
                // the billions rather than a fraction. The jitter drowned both
                // real terms, so a town on clay banks reached for whatever the
                // hash liked and the ground never got a say at all. Taken to a
                // thousandth and scaled by hand now, so the range is on the
                // page.
                let jitter = ((seed ^ hash(node.key)).wrapping_mul(2_654_435_761) >> 33) as u32;
                let temper = (jitter % 1000) as f32 / 1000.0 * TEMPER;
                ready * brevity + temper
            };
            worth(a).total_cmp(&worth(b))
        })
        .map(|node| *node)
}

/// How far a town's own turn of mind may move a node's worth. Enough to settle
/// a tie between two nodes of the same length, never enough to beat a sample the
/// town is standing on.
const TEMPER: f32 = 0.3;

/// A small stable hash, so a town's leanings do not move between runs.
fn hash(text: &str) -> u64 {
    let mut sum = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        sum ^= *byte as u64;
        sum = sum.wrapping_mul(0x0100_0000_01b3);
    }
    sum
}

/// The scholars at their books: time accrues, samples are spent, and a settled
/// question is announced to the town that settled it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn the_scholars_study(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut chronicles: Query<&mut super::Chronicle>,
    known_world: Option<Res<super::explore::KnownWorld>>,
    deposits: Query<(&GlobalTransform, &crate::matter::Deposit)>,
    stands: Query<(&GlobalTransform, &crate::scatter::SacredFlora)>,
    libraries: Query<(&Building, &MemberOf)>,
    scholars: Query<(Entity, &Vocation, &MemberOf), (With<Villager>, Without<Corpse>)>,
    mut towns: Query<(
        Entity,
        &Settlement,
        &super::SettlementGround,
        &mut Stockpile,
        &mut Studies,
    )>,
) {
    if !clock.work_hours() {
        return;
    }
    let dt = time.delta_secs();
    for (town, settlement, ground, mut store, mut studies) in &mut towns {
        if !libraries
            .iter()
            .any(|(b, member)| b.kind == BuildingKind::Library && member.0 == town)
        {
            continue;
        }
        let hands = scholars
            .iter()
            .filter(|(_, vocation, member)| **vocation == Vocation::Scholar && member.0 == town)
            .count();
        if hands == 0 {
            continue;
        }

        // ALWAYS READING SOMETHING. A node is chosen the moment there is none,
        // and never swapped out from under itself afterward - a town that
        // reconsidered every morning would never finish anything, and the
        // half-read pages are the cost of changing your mind.
        if studies.at_hand.is_none() {
            let seed = hash(&settlement.name);
            let Some(chosen) = choose(&studies.known, |sample| sample.held(&store), seed) else {
                // The whole tree read. A rare and good problem.
                studies.wanting = None;
                continue;
            };
            info!(
                "the scholars of {} take up {}",
                settlement.name,
                chosen.name.to_lowercase()
            );
            studies.at_hand = Some(chosen.key.to_string());
            studies.progress = 0.0;
            studies.in_hand = false;
        }
        let Some(reading) = studies.at_hand.as_deref().and_then(node) else {
            // An unknown key: a save from before this node existed, or a tree
            // that has been re-authored under a running town. Drop it and pick
            // again next tick rather than reading nothing for ever.
            studies.at_hand = None;
            continue;
        };

        // PHASE ONE: reading, which nothing blocks. A second scholar nearly
        // halves the work and a sixth adds almost nothing - reading is not
        // hauling, and two people on one question argue usefully where six get
        // in each other's way.
        let hands = (hands as f32).sqrt();
        if studies.progress < reading.theory {
            studies.progress += dt * hands;
            continue;
        }

        // THE MIDDLE: read out, and waiting on something somebody has to go and
        // get. The town stands finished-but-for-the-ore and says so - by name,
        // by amount, and with directions, because the library holds the maps.
        if let Some((sample, wanted)) = reading.sample
            && !studies.in_hand
        {
            if sample.held(&store) < wanted {
                let short = wanted - sample.held(&store);
                let toward = nearest_source(
                    sample,
                    ground.center,
                    known_world.as_deref(),
                    &deposits,
                    &stands,
                );
                if studies.wanting.map(|had| had.sample) != Some(sample) {
                    info!(
                        "{} has read all it can of {}: it wants {:.0} {}{}",
                        settlement.name,
                        reading.name.to_lowercase(),
                        short,
                        sample.word(),
                        match toward {
                            Some(at) => format!(
                                " - the nearest the scholars know of is {:.0} strides out",
                                ground.center.distance(at)
                            ),
                            None => ", and nobody knows where to find any".to_string(),
                        }
                    );
                    notices.write(crate::ui::Notice::new(format!(
                        "The scholars want {} — {:.0} short for {}",
                        sample.word(),
                        short,
                        reading.name.to_lowercase()
                    )));
                }
                studies.wanting = Some(Need {
                    sample,
                    short,
                    toward,
                });
                continue;
            }
            // IT ARRIVED. Spent on the spot rather than held in reserve: a
            // question answered costs the town the ore it was answered with,
            // which makes a breakthrough an expense and not a toll.
            sample.take(&mut store, wanted);
            studies.in_hand = true;
            studies.wanting = None;
            info!(
                "{} has its {} - the {} can go on",
                settlement.name,
                sample.word(),
                reading.name.to_lowercase()
            );
            notices.write(crate::ui::Notice::new(format!(
                "The {} arrives at the library",
                sample.word()
            )));
            continue;
        }

        // PHASE TWO: the work itself, with the thing in hand.
        studies.progress += dt * hands;
        if studies.progress < reading.theory + reading.practice {
            continue;
        }

        // Settled. The sample was spent when it arrived; what finishes here is
        // the work that was done with it.
        studies.wanting = None;
        studies.in_hand = false;
        studies.known.push(reading.key.to_string());
        studies.at_hand = None;
        studies.progress = 0.0;
        info!(
            "{} has worked out {}: {}",
            settlement.name,
            reading.name.to_lowercase(),
            reading.blurb,
        );
        notices.write(crate::ui::Notice::fanfare(format!(
            "{} has learned {}",
            settlement.name, reading.name
        )));
        let day = clock.day();
        for (who, vocation, member) in &scholars {
            if member.0 == town
                && *vocation == Vocation::Scholar
                && let Ok(mut story) = chronicles.get_mut(who)
            {
                story.record(day, format!("worked out {}", reading.name.to_lowercase()));
            }
        }
    }
}

/// THE SCHOLAR POINTS THE WAY: the nearest place the town KNOWS OF that yields
/// this sample.
///
/// Brett: "Maybe the scholars can even point them in the right direction of the
/// closest place to get it?" Known ground only — the library holds maps, not
/// prophecy, so a town that has never sent anybody out gets a want with no
/// direction on it, and that is an explorer's problem before it is a party's.
fn nearest_source(
    sample: Sample,
    from: Vec3,
    known: Option<&super::explore::KnownWorld>,
    deposits: &Query<(&GlobalTransform, &crate::matter::Deposit)>,
    stands: &Query<(&GlobalTransform, &crate::scatter::SacredFlora)>,
) -> Option<Vec3> {
    // Sacred stands are chunk children, so their globals are BENT onto the
    // planet - unbend before any distance. Deposits are roots and are not.
    let mut best: Option<(f32, Vec3)> = None;
    let mut offer = |at: Vec3| {
        if known.is_some_and(|known| !known.knows(at)) {
            return;
        }
        let reach = at.distance(from);
        if best.is_none_or(|(had, _)| reach < had) {
            best = Some((reach, at));
        }
    };
    if let Some(wanted) = sample.dug_from() {
        for (at, deposit) in deposits {
            if deposit.kind == wanted && deposit.amount > 0.5 {
                offer(at.translation());
            }
        }
    }
    if let Some(wanted) = sample.picked_from() {
        for (at, stand) in stands {
            if stand.kind == wanted && stand.amount > 0.3 {
                offer(crate::globe::unbend(at.translation()));
            }
        }
    }
    best.map(|(_, at)| at)
}

/// Every town gets somewhere to keep what it knows, library or no library.
pub(crate) fn every_town_keeps_its_learning(
    mut commands: Commands,
    towns: Query<Entity, (With<Settlement>, Without<Studies>)>,
) {
    for town in &towns {
        commands.entity(town).insert(Studies::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(of: &[&'static Node]) -> Vec<&'static str> {
        of.iter().map(|node| node.key).collect()
    }

    /// THE TREE IS WHOLE.
    ///
    /// The price of authoring nodes as data with string keys is that a typo in a
    /// prerequisite is not a compile error — it is a branch nobody can ever
    /// reach, silently. This is the test that buys that back, and it has to run
    /// on every row: no duplicate keys, no dangling prerequisite, no node that
    /// requires itself.
    #[test]
    fn the_tree_is_whole() {
        let mut seen: Vec<&str> = Vec::new();
        for node in THE_TREE {
            assert!(
                !seen.contains(&node.key),
                "two nodes are called {}",
                node.key
            );
            seen.push(node.key);
            assert!(!node.name.is_empty() && !node.blurb.is_empty());
            assert!(node.theory > 0.0, "{} costs no reading at all", node.key);
            assert!(node.practice >= 0.0);
            assert_eq!(
                node.sample.is_some(),
                node.practice > 0.0,
                "{}: a node wants a sample exactly when it has a second half to \
                 spend it on - see `PRACTICE`",
                node.key
            );
            for need in node.requires {
                assert_ne!(*need, node.key, "{} requires itself", node.key);
                assert!(
                    THE_TREE.iter().any(|other| other.key == *need),
                    "{} requires {need}, which is not a node",
                    node.key
                );
            }
        }
    }

    /// AND EVERY NODE OF IT CAN BE REACHED.
    ///
    /// A cycle, or a node whose prerequisites can never all be met, is a node
    /// that may as well not exist. Walked forward from nothing: if the frontier
    /// ever empties before the tree does, something is stranded.
    #[test]
    fn every_node_can_be_reached() {
        let mut known: Vec<String> = Vec::new();
        while known.len() < THE_TREE.len() {
            let open = frontier(&known);
            assert!(
                !open.is_empty(),
                "the frontier ran dry with {} of {} nodes read - stranded: {:?}",
                known.len(),
                THE_TREE.len(),
                THE_TREE
                    .iter()
                    .map(|n| n.key)
                    .filter(|k| !known.iter().any(|had| had == k))
                    .collect::<Vec<_>>()
            );
            for node in open {
                known.push(node.key.to_string());
            }
        }
    }

    /// IT IS A TREE AND NOT A LIST. A branching structure means several things
    /// open at once, and some nodes standing on two parents.
    #[test]
    fn the_tree_actually_branches() {
        let open = frontier(&["letters".to_string()]);
        assert!(
            open.len() >= 4,
            "one node read and only {} opened - that is a queue, not a tree",
            open.len()
        );
        let joins = THE_TREE
            .iter()
            .filter(|node| node.requires.len() > 1)
            .count();
        assert!(
            joins >= 3,
            "only {joins} nodes stand on two parents - the branches never meet"
        );
    }

    /// A TOWN ALWAYS HAS SOMETHING TO READ, at every point in the walk. Brett:
    /// "they always must be researching something."
    #[test]
    fn there_is_always_something_to_read() {
        let mut known: Vec<String> = Vec::new();
        // An empty yard, so nothing is ever conveniently in hand.
        for _ in 0..THE_TREE.len() {
            let chosen = choose(&known, |_| 0.0, 7)
                .expect("a town with an empty yard still has something to read");
            known.push(chosen.key.to_string());
        }
        assert_eq!(known.len(), THE_TREE.len());
        assert!(choose(&known, |_| 0.0, 7).is_none(), "and then it is done");
    }

    /// THE GROUND PICKS THE PATH.
    ///
    /// Two towns with the same books and different yards must not reach for the
    /// same thing — that is the whole of "towns can pick their own paths", and it
    /// costs no bespoke code: nothing anywhere names a clay town. Compared
    /// between the two SAMPLE-GATED siblings, since a node the town can finish
    /// this week beats one that needs a journey either way.
    #[test]
    fn a_towns_ground_leans_its_reading() {
        // Everything read but the two nodes that hang off quarrying.
        let read: Vec<String> = THE_TREE
            .iter()
            .map(|node| node.key)
            .filter(|key| *key != "brickwork" && *key != "masonry" && *key != "milling")
            .map(str::to_string)
            .collect();
        let on_clay = choose(&read, |s| if s == Sample::Clay { 99.0 } else { 0.0 }, 7);
        assert_eq!(
            on_clay.map(|n| n.key),
            Some("brickwork"),
            "a town standing on clay banks should reach for the brick"
        );
        let on_stone = choose(&read, |s| if s == Sample::Stone { 99.0 } else { 0.0 }, 7);
        assert_eq!(
            on_stone.map(|n| n.key),
            Some("masonry"),
            "and a town with a quarry at its back should reach for the wall"
        );
    }

    /// AND TWO TOWNS ON IDENTICAL GROUND STILL DIVERGE, or every town in the
    /// world reads the same books in the same order and the tree is a queue with
    /// extra steps.
    #[test]
    fn two_towns_on_the_same_ground_read_differently() {
        let read = vec!["letters".to_string()];
        let orders: Vec<Vec<&str>> = [1u64, 2, 3, 4, 5, 6, 7, 8]
            .iter()
            .map(|seed| {
                let mut known = read.clone();
                let mut order = Vec::new();
                for _ in 0..4 {
                    let Some(next) = choose(&known, |_| 0.0, *seed) else {
                        break;
                    };
                    order.push(next.key);
                    known.push(next.key.to_string());
                }
                order
            })
            .collect();
        let first = &orders[0];
        assert!(
            orders.iter().any(|other| other != first),
            "eight towns and every one of them read {first:?} - the temper does nothing"
        );
    }

    /// THE TWO PHASES, AND THE BAR THAT SHOWS THEM.
    ///
    /// Brett: "a time gate for phase 1 and then they need to bring back iron
    /// that unlocks the time gate for phase 2." The library's window wants two
    /// bars, so `phase` and `share` have to agree with each other at every point
    /// of a node - a bar that reads full while the town is actually waiting on
    /// ore is a bar that lies.
    #[test]
    fn a_node_reads_then_waits_then_works() {
        let iron = node("ironwork").expect("iron weapons are on the tree");
        assert!(iron.sample.is_some() && iron.practice > 0.0);

        // Nothing on the table at all.
        let idle = Studies::default();
        assert_eq!(idle.phase(), None);
        assert_eq!(idle.share(), 0.0);

        // PHASE ONE: reading. The bar fills across the theory alone.
        let mut town = Studies {
            at_hand: Some("ironwork".to_string()),
            ..Default::default()
        };
        assert_eq!(town.phase(), Some(Phase::Theory));
        assert_eq!(town.share(), 0.0);
        town.progress = iron.theory / 2.0;
        assert_eq!(town.phase(), Some(Phase::Theory));
        assert!((town.share() - 0.5).abs() < 0.01, "half read");

        // THE MIDDLE: read out, and no amount of further reading moves it.
        town.progress = iron.theory;
        assert_eq!(town.phase(), Some(Phase::Wanting));
        assert_eq!(town.share(), 1.0, "the reading is genuinely finished");
        town.progress += 10_000.0;
        assert_eq!(
            town.phase(),
            Some(Phase::Wanting),
            "no scholar reads their way past a missing sample"
        );

        // PHASE TWO: the ore arrives and the second bar starts from nothing.
        town.progress = iron.theory;
        town.in_hand = true;
        assert_eq!(town.phase(), Some(Phase::Practice));
        assert_eq!(town.share(), 0.0, "the work with it in hand has not begun");
        town.progress = iron.theory + iron.practice / 2.0;
        assert!((town.share() - 0.5).abs() < 0.01, "half worked");
        town.progress = iron.theory + iron.practice;
        assert_eq!(town.share(), 1.0);
    }

    /// A NODE WITH NO SAMPLE NEVER WAITS, which is what keeps a town that can
    /// reach nothing from ever running out of things to read.
    #[test]
    fn a_node_that_wants_nothing_never_blocks() {
        let joinery = node("joinery").expect("joinery is on the tree");
        assert_eq!(joinery.sample, None);
        let town = Studies {
            at_hand: Some("joinery".to_string()),
            progress: joinery.theory,
            ..Default::default()
        };
        assert_eq!(
            town.phase(),
            Some(Phase::Practice),
            "with nothing to fetch, reading it out IS finishing it"
        );
    }

    /// The gifts add up, and only the ones the town has.
    #[test]
    fn what_is_learned_is_what_is_gained() {
        let ignorant = Studies::default();
        assert_eq!(ignorant.boons(), Boons::default());
        let handy = Studies {
            known: vec!["joinery".to_string(), "framing".to_string()],
            ..Default::default()
        };
        assert_eq!(handy.boons().timber_per_tree, 2.0, "two nodes, two logs");
        // And a key from a tree that no longer has it is ignored rather than
        // panicking - saves outlive authoring.
        let stale = Studies {
            known: vec!["phlogiston".to_string()],
            ..Default::default()
        };
        assert_eq!(stale.boons(), Boons::default());
    }

    /// THE SURVIVAL LADDER NEEDS NO PERMISSION. The rungs a village lives or
    /// dies on must never wait on a library it has not built.
    #[test]
    fn the_survival_ladder_needs_no_permission() {
        let ignorant = Studies::default();
        for kind in [
            BuildingKind::House,
            BuildingKind::Longhouse,
            BuildingKind::Well,
            BuildingKind::Dock,
            BuildingKind::Storehouse,
            BuildingKind::Sawmill,
            BuildingKind::Granary,
            BuildingKind::Smokehouse,
            BuildingKind::Mine,
            BuildingKind::Blacksmith,
            BuildingKind::Tavern,
            BuildingKind::Watchtower,
            BuildingKind::Shrine,
            BuildingKind::TownHall,
            BuildingKind::Library,
        ] {
            assert!(
                ignorant.permits(kind),
                "{} is survival, or the way out of ignorance, and must not wait on a book",
                kind.name()
            );
        }
    }

    /// And the late ambitions do wait, and stop waiting when the book is read.
    #[test]
    fn the_late_ambitions_wait_on_the_books() {
        let ignorant = Studies::default();
        let gated: Vec<BuildingKind> = THE_TREE
            .iter()
            .filter_map(|node| match node.gives {
                Gift::Unlocks(kind) => Some(kind),
                _ => None,
            })
            .collect();
        assert!(gated.len() >= 4, "the tree unlocks almost nothing");
        for kind in &gated {
            assert!(!ignorant.permits(*kind), "{} should wait", kind.name());
        }
        let learned = Studies {
            known: THE_TREE.iter().map(|n| n.key.to_string()).collect(),
            ..Default::default()
        };
        for kind in BuildingKind::every() {
            assert!(learned.permits(*kind), "a town that knows all may want all");
        }
    }

    /// A sample-gated node is a JOURNEY, and there have to be enough of them
    /// for the tree to drive anybody anywhere.
    #[test]
    fn enough_of_the_tree_is_worth_traveling_for() {
        let journeys = THE_TREE.iter().filter(|n| n.sample.is_some()).count();
        assert!(
            journeys * 4 >= THE_TREE.len(),
            "only {journeys} of {} nodes want anything fetched - the tree does not \
             send anybody anywhere",
            THE_TREE.len()
        );
    }
}

#[cfg(test)]
mod probe {
    use super::*;
    #[test]
    #[ignore]
    fn what_the_frontier_is_worth() {
        let read: Vec<String> = THE_TREE
            .iter()
            .map(|node| node.key)
            .filter(|key| *key != "brickwork" && *key != "masonry" && *key != "milling")
            .map(str::to_string)
            .collect();
        for (label, have) in [
            ("clay", Sample::Clay),
            ("stone", Sample::Stone),
        ] {
            let open = frontier(&read);
            println!("--- {label} town, frontier: {:?}", open.iter().map(|n| n.key).collect::<Vec<_>>());
            for node in &open {
                let held = if node.sample.map(|(s, _)| s) == Some(have) { 99.0 } else { 0.0 };
                println!(
                    "   {:12} sample {:?} held {held} theory {} practice {}",
                    node.key, node.sample, node.theory, node.practice
                );
            }
            println!("   chose {:?}", choose(&read, |s| if s == have { 99.0 } else { 0.0 }, 7).map(|n| n.key));
        }
    }
}
