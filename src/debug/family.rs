//! The family tree: who somebody came from, and who came from them.
//!
//! Brett: "One thing I really want is for each player window to have a family
//! tree. Like with the branches and stuff... Maybe one the player could zoom on
//! and drag around?"
//!
//! WHY THIS IS WORTH BUILDING PROPERLY. The simulation records kinship as
//! ENTITIES - `Parentage { mother, father }` and `Spouse` - not as names, so the
//! graph walks as far as the world has history in both directions. And the dead
//! keep their `Person` on the headstone entity, so a tree includes ancestors who
//! have died. That is the difference between a dynasty and a snapshot, and it is
//! the whole reason a god should want to look at one.
//!
//! THE GEOMETRY IS THE RISKY PART, so it lives here as a pure function with
//! tests, and knows nothing about Bevy's UI. A tree drawn with absolutely
//! positioned nodes and hairline segments is real coordinate arithmetic:
//! branches that miss their node by two pixels, or a generation that overlaps
//! its own children, are exactly the failures a screenshot does not catch and a
//! test does. What comes out of here is a list of people with places and a list
//! of segments to draw between them; turning that into nodes is the easy half.

use bevy::prelude::Entity;

/// How a person on the tree is related to the one it was drawn for.
///
/// Carried so the drawing can mark the subject, and so a spouse can be drawn
/// beside rather than below - a wife is not her husband's descendant, and a tree
/// that files her as one is telling a lie about the family.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kin {
    /// The person whose window this is.
    Subject,
    Spouse,
    Ancestor,
    Sibling,
    Descendant,
}

/// One person, placed. Coordinates are in tree units - the drawing decides what
/// a unit is worth in pixels, which is what lets the whole thing zoom.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Seat {
    pub who: Entity,
    pub kin: Kin,
    /// The middle of the card, not its corner: every branch meets a person at
    /// their center, and a layout that stored corners would have every segment
    /// doing the same half-width arithmetic over again.
    pub x: f32,
    pub y: f32,
}

/// One straight run of branch. Horizontal when `h` is zero, vertical when `w`
/// is - the drawing needs no angles, and a tree of right angles is what reads as
/// a family tree rather than a graph.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Branch {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A plotted family: everybody's seat, every branch between them, and the
/// bounds, so the drawing can center the whole thing before anybody drags it.
#[derive(Clone, Default, Debug)]
pub(crate) struct Plot {
    pub seats: Vec<Seat>,
    pub branches: Vec<Branch>,
}

/// How far apart two people sit, side to side and generation to generation.
const APART: f32 = 1.0;
const A_GENERATION: f32 = 1.0;

impl Plot {
    /// The bounds of everything drawn, as (least x, least y, most x, most y).
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let mut least = (f32::MAX, f32::MAX);
        let mut most = (f32::MIN, f32::MIN);
        for seat in &self.seats {
            least = (least.0.min(seat.x), least.1.min(seat.y));
            most = (most.0.max(seat.x), most.1.max(seat.y));
        }
        if self.seats.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        (least.0, least.1, most.0, most.1)
    }

    fn seat(&mut self, who: Entity, kin: Kin, x: f32, y: f32) {
        self.seats.push(Seat { who, kin, x, y });
    }

    /// A right-angled run from one person down to another: down out of the
    /// parent, across, and down into the child.
    fn branch_down(&mut self, from: (f32, f32), to: (f32, f32)) {
        let midway = (from.1 + to.1) * 0.5;
        self.branches.push(Branch {
            x: from.0,
            y: from.1,
            w: 0.0,
            h: midway - from.1,
        });
        if (to.0 - from.0).abs() > f32::EPSILON {
            self.branches.push(Branch {
                x: from.0.min(to.0),
                y: midway,
                w: (to.0 - from.0).abs(),
                h: 0.0,
            });
        }
        self.branches.push(Branch {
            x: to.0,
            y: midway,
            w: 0.0,
            h: to.1 - midway,
        });
    }
}

/// The kin graph, asked rather than owned.
///
/// Closures rather than a borrowed World, so the layout can be tested against a
/// family invented in ten lines. Every one of these questions is answerable from
/// components the simulation already keeps.
pub(crate) struct Kinfolk<'a> {
    pub parents_of: &'a dyn Fn(Entity) -> Option<(Entity, Entity)>,
    pub children_of: &'a dyn Fn(Entity) -> Vec<Entity>,
    pub spouse_of: &'a dyn Fn(Entity) -> Option<Entity>,
}

/// Plots a family around one person: their line up, their line down, and the
/// people who stand beside them.
///
/// Ancestors HALVE their spread each generation, which is the pedigree shape
/// everybody recognizes - two parents over the middle of the child, four
/// grandparents over the middle of each parent - and it is also what keeps the
/// branches from crossing without any collision work at all.
///
/// Descendants do the opposite: children are laid out side by side and the
/// parent is centered OVER them, so a family of six does not have five of them
/// stacked behind the sixth.
pub(crate) fn plot_the_family(
    subject: Entity,
    kin: &Kinfolk,
    up: usize,
    down: usize,
) -> Plot {
    let mut plot = Plot::default();
    plot.seat(subject, Kin::Subject, 0.0, 0.0);

    // A spouse stands beside, and their line is not drawn upward: this is the
    // subject's family, and a spouse's own ancestors are their own window.
    if let Some(spouse) = (kin.spouse_of)(subject) {
        plot.seat(spouse, Kin::Spouse, APART * 1.6, 0.0);
        plot.branches.push(Branch {
            x: 0.0,
            y: 0.0,
            w: APART * 1.6,
            h: 0.0,
        });
    }

    climb(&mut plot, subject, kin, 0.0, 0.0, APART * 2.0, up);
    descend(&mut plot, subject, kin, 0.0, 0.0, down);

    // Brothers and sisters: everybody the parents had who is not the subject.
    if let Some((mother, _)) = (kin.parents_of)(subject) {
        let mut beside = -APART * 1.6;
        for kid in (kin.children_of)(mother) {
            if kid == subject {
                continue;
            }
            plot.seat(kid, Kin::Sibling, beside, 0.0);
            beside -= APART * 1.6;
        }
    }
    plot
}

/// The line upward, halving its spread each generation.
fn climb(
    plot: &mut Plot,
    who: Entity,
    kin: &Kinfolk,
    x: f32,
    y: f32,
    spread: f32,
    left: usize,
) {
    if left == 0 {
        return;
    }
    let Some((mother, father)) = (kin.parents_of)(who) else {
        return;
    };
    let above = y - A_GENERATION;
    for (parent, at) in [(father, x - spread * 0.5), (mother, x + spread * 0.5)] {
        plot.seat(parent, Kin::Ancestor, at, above);
        plot.branch_down((at, above), (x, y));
        climb(plot, parent, kin, at, above, spread * 0.5, left - 1);
    }
    // The line joining a couple, drawn once rather than per parent.
    plot.branches.push(Branch {
        x: x - spread * 0.5,
        y: above,
        w: spread,
        h: 0.0,
    });
}

/// The line downward, each parent centered over the children they had.
fn descend(plot: &mut Plot, who: Entity, kin: &Kinfolk, x: f32, y: f32, left: usize) -> f32 {
    if left == 0 {
        return x;
    }
    let kids = (kin.children_of)(who);
    if kids.is_empty() {
        return x;
    }
    let below = y + A_GENERATION;
    // Laid out from the parent's own x, spread evenly either side.
    let span = APART * 1.6 * (kids.len() as f32 - 1.0);
    let mut at = x - span * 0.5;
    for kid in kids {
        plot.seat(kid, Kin::Descendant, at, below);
        plot.branch_down((x, y), (at, below));
        descend(plot, kid, kin, at, below, left - 1);
        at += APART * 1.6;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A family invented for the test, answered through the same closures the
    /// game answers through.
    struct Family {
        parents: HashMap<u64, (u64, u64)>,
        kids: HashMap<u64, Vec<u64>>,
        spouses: HashMap<u64, u64>,
    }

    fn who(bits: u64) -> Entity {
        Entity::from_bits((1u64 << 32) | bits)
    }

    /// The small number this entity was invented from.
    fn tag(who: Entity) -> u64 {
        who.to_bits() & 0xFFFF_FFFF
    }

    impl Family {
        fn plot(&self, subject: u64, up: usize, down: usize) -> Plot {
            let parents = |e: Entity| {
                self.parents
                    .get(&tag(e))
                    .map(|(m, f)| (who(*m), who(*f)))
            };
            let children = |e: Entity| {
                self.kids
                    .get(&tag(e))
                    .map(|list| list.iter().map(|k| who(*k)).collect())
                    .unwrap_or_default()
            };
            let spouse = |e: Entity| self.spouses.get(&tag(e)).map(|s| who(*s));
            plot_the_family(
                who(subject),
                &Kinfolk {
                    parents_of: &parents,
                    children_of: &children,
                    spouse_of: &spouse,
                },
                up,
                down,
            )
        }
    }

    /// Three generations, and nobody standing on anybody.
    ///
    /// THE FAILURE THIS CATCHES is two people plotted to the same place, which
    /// on screen is one card with another exactly behind it - a family that
    /// silently loses a grandmother.
    #[test]
    fn nobody_shares_a_seat() {
        let family = Family {
            parents: HashMap::from([(1, (2, 3)), (2, (4, 5)), (3, (6, 7))]),
            kids: HashMap::from([(2, vec![1]), (1, vec![8, 9, 10])]),
            spouses: HashMap::from([(1, 11)]),
        };
        let plot = family.plot(1, 2, 1);
        for (i, one) in plot.seats.iter().enumerate() {
            for other in &plot.seats[i + 1..] {
                let same = (one.x - other.x).abs() < 0.01 && (one.y - other.y).abs() < 0.01;
                assert!(!same, "{one:?} sits on top of {other:?}");
            }
        }
        // Everybody who should be there: the subject, a spouse, two parents,
        // four grandparents and three children.
        assert_eq!(plot.seats.len(), 1 + 1 + 2 + 4 + 3, "{:?}", plot.seats);
    }

    /// Ancestors go up, descendants go down, spouses stay level.
    #[test]
    fn the_generations_run_the_right_way() {
        let family = Family {
            parents: HashMap::from([(1, (2, 3))]),
            kids: HashMap::from([(1, vec![4])]),
            spouses: HashMap::from([(1, 5)]),
        };
        let plot = family.plot(1, 1, 1);
        for seat in &plot.seats {
            match seat.kin {
                Kin::Subject => assert_eq!(seat.y, 0.0),
                Kin::Spouse | Kin::Sibling => {
                    assert_eq!(seat.y, 0.0, "a spouse or sibling is not a generation away")
                }
                Kin::Ancestor => assert!(seat.y < 0.0, "an ancestor is drawn above"),
                Kin::Descendant => assert!(seat.y > 0.0, "a descendant is drawn below"),
            }
        }
    }

    /// Every branch is a straight run, and every run meets somebody.
    ///
    /// A branch with both a width and a height would draw as a filled block, and
    /// a branch ending in empty air is the two-pixel miss that makes a tree look
    /// broken without anybody being able to say why.
    #[test]
    fn every_branch_is_square_and_lands_on_a_person() {
        let family = Family {
            parents: HashMap::from([(1, (2, 3)), (2, (4, 5))]),
            kids: HashMap::from([(1, vec![6, 7])]),
            spouses: HashMap::new(),
        };
        let plot = family.plot(1, 2, 1);
        assert!(!plot.branches.is_empty());
        for run in &plot.branches {
            let square = run.w.abs() < f32::EPSILON || run.h.abs() < f32::EPSILON;
            assert!(square, "{run:?} is a block, not a branch");
        }
        // Every VERTICAL run has to start or finish on somebody's x, or it is
        // hanging in the middle of the tree.
        for run in plot.branches.iter().filter(|r| r.w.abs() < f32::EPSILON) {
            let met = plot.seats.iter().any(|seat| (seat.x - run.x).abs() < 0.01);
            assert!(met, "{run:?} runs down to nobody");
        }
    }

    /// A founder has no line upward, and that is not an error.
    #[test]
    fn the_first_generation_stands_alone() {
        let family = Family {
            parents: HashMap::new(),
            kids: HashMap::new(),
            spouses: HashMap::new(),
        };
        let plot = family.plot(1, 3, 3);
        assert_eq!(plot.seats.len(), 1);
        assert!(plot.branches.is_empty());
        assert_eq!(plot.bounds(), (0.0, 0.0, 0.0, 0.0));
    }
}
