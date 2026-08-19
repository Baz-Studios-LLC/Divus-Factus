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

    // THE BASE SPREAD IS SET BY HOW DEEP WE ARE GOING, because the pedigree
    // shape halves it every generation: ask for three generations from a base of
    // two and the great-grandparents sit half a card apart, overlapping. Doubling
    // the base per generation asked for keeps the TOP row at one unit of spacing
    // whatever the depth, which is the row that decides whether a tree is
    // readable.
    let base = APART * (1u32 << up.max(1).min(6)) as f32 * 0.5;
    climb(&mut plot, subject, kin, 0.0, 0.0, base, up);
    // Children hang from THE HEARTH - the middle of the marriage line - not from
    // the subject's own card, for the same reason ancestors drop from theirs.
    let hearth = if (kin.spouse_of)(subject).is_some() {
        APART * 0.8
    } else {
        0.0
    };
    descend(&mut plot, subject, kin, hearth, 0.0, down);

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
        climb(plot, parent, kin, at, above, spread * 0.5, left - 1);
    }
    // The marriage line joining them...
    plot.branches.push(Branch {
        x: x - spread * 0.5,
        y: above,
        w: spread,
        h: 0.0,
    });
    // ...and ONE drop from the middle of it. A child comes from a COUPLE, and a
    // line run from each parent separately says something else - or worse, a
    // line from one of them says the child is theirs alone. Brett: "This is not
    // the right way to show a child of two people."
    plot.branches.push(Branch {
        x,
        y: above,
        w: 0.0,
        h: y - above,
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
        // Every VERTICAL run has to meet somebody - either a person's own x, or
        // the middle of a marriage, which is where a couple's children hang from.
        for run in plot.branches.iter().filter(|r| r.w.abs() < f32::EPSILON) {
            let on_a_person = plot.seats.iter().any(|seat| (seat.x - run.x).abs() < 0.01);
            let between_a_couple = plot.seats.iter().any(|one| {
                plot.seats.iter().any(|other| {
                    (one.y - other.y).abs() < 0.01
                        && one.x < other.x
                        && (((one.x + other.x) * 0.5) - run.x).abs() < 0.01
                })
            });
            assert!(
                on_a_person || between_a_couple,
                "{run:?} runs down to nobody"
            );
        }
    }

    /// However deep it goes, the top row stays a card's width apart.
    ///
    /// The pedigree halves its spread each generation, so the base has to double
    /// with the depth asked for or the oldest row - the one with the most people
    /// in it - is the one that overlaps.
    #[test]
    fn the_oldest_generation_still_has_room() {
        // A line of ancestors four deep: everybody has two parents.
        let mut parents = std::collections::HashMap::new();
        for who in 1u64..32 {
            parents.insert(who, (who * 2, who * 2 + 1));
        }
        let family = Family {
            parents,
            kids: HashMap::new(),
            spouses: HashMap::new(),
        };
        for depth in 1..=4 {
            let plot = family.plot(1, depth, 0);
            let oldest = plot
                .seats
                .iter()
                .filter(|seat| seat.y == -(depth as f32))
                .map(|seat| seat.x)
                .collect::<Vec<_>>();
            let mut sorted = oldest.clone();
            sorted.sort_by(f32::total_cmp);
            for pair in sorted.windows(2) {
                assert!(
                    pair[1] - pair[0] >= APART - 0.001,
                    "at {depth} deep the oldest row is {:.2} apart, which overlaps",
                    pair[1] - pair[0]
                );
            }
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

// ---------------------------------------------------------------------------
// The drawing: tree units into a canvas of cards and hairlines.
// ---------------------------------------------------------------------------

use bevy::prelude::*;

/// A card's size, and what a tree unit is worth in pixels.
///
/// One unit is a card and a gap, so the spread arithmetic above lands cards
/// beside each other rather than through each other.
const CARD: Vec2 = Vec2::new(104.0, 30.0);
const UNIT: Vec2 = Vec2::new(118.0, 76.0);

/// Marks the canvas the tree is drawn on, so dragging and zooming have
/// something to hold.
#[derive(Component)]
pub(crate) struct FamilyCanvas;

/// A card somebody can be walked to by clicking.
///
/// Brett: "Clicking on anyones name should open their panel and close the
/// current one." Which is what turns a diagram into a way of getting about - a
/// tree you can walk is how a player finds the great-grandmother nobody would
/// ever have gone looking for.
///
/// The DEAD carry none of these, and the subject does not either. A dead
/// ancestor has no panel to open - the window's own queries are `With<Villager>,
/// Without<Corpse>` - so a click would swap a full page for an empty one, and
/// the subject is already the page you are on.
#[derive(Component)]
pub(crate) struct KinCard(pub Entity);

/// Everything the drawing needs to know about one person on the tree.
pub(crate) struct Who {
    pub name: String,
    pub ink: Color,
    pub dead: bool,
}

/// Draws a plotted family into a parent, and returns the canvas.
///
/// Absolute positions inside one canvas, because branches are the point: a flex
/// layout can place cards but cannot run a line between two of them, and a tree
/// without its branches is a list with gaps. Everything is placed from the
/// plot's least corner, so the canvas is exactly the size of the family and can
/// be dragged around inside the well as one piece.
pub(crate) fn draw_the_family(
    commands: &mut Commands,
    parent: Entity,
    plot: &Plot,
    who_is: impl Fn(Entity) -> Who,
) -> Entity {
    let (least_x, least_y, most_x, most_y) = plot.bounds();
    let place = |x: f32, y: f32| Vec2::new((x - least_x) * UNIT.x, (y - least_y) * UNIT.y);

    // A VIEWPORT OF ITS OWN, and this was missing. The canvas is absolutely
    // positioned, so hung straight off the well it ignores the flow and lands on
    // top of whatever is above it - Brett's shot of the tab opening has a
    // grandfather sitting across "THE BLOOD". A relative box below the header
    // gives the tree somewhere to be, something to be CLIPPED by when it is
    // dragged, and - the part the centering needed - a real visible rectangle to
    // be centered in. The well itself is as tall as its contents, so centering
    // in that put the tree at the top.
    let viewport = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(260.0),
                overflow: Overflow::clip(),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let canvas = commands
        .spawn((
            FamilyCanvas,
            // The handle the zoom turns. Scaled from its top left rather than
            // its middle, so zooming in does not also slide the tree sideways -
            // a zoom that moves what you were looking at is a zoom that fights
            // the drag.
            UiTransform {
                scale: Vec2::splat(1.0),
                ..default()
            },
            // Hidden until it has been placed - see `FamilyView::placed`.
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px((most_x - least_x) * UNIT.x + CARD.x),
                height: Val::Px((most_y - least_y) * UNIT.y + CARD.y),
                ..default()
            },
            ChildOf(viewport),
        ))
        .id();

    // THE BRANCHES FIRST, so cards paint over them: a hairline that runs under a
    // card is invisible, and one that runs over it looks like a scratch.
    for run in &plot.branches {
        let at = place(run.x, run.y);
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                // Branches leave a person's middle, so they start half a card in.
                left: Val::Px(at.x + CARD.x * 0.5),
                top: Val::Px(at.y + CARD.y * 0.5),
                width: Val::Px((run.w * UNIT.x).max(1.0)),
                height: Val::Px((run.h.abs() * UNIT.y).max(1.0)),
                ..default()
            },
            // A run with a negative height climbs, so it is drawn from its top.
            BackgroundColor(crate::ui::theme::panel_border()),
            ChildOf(canvas),
        ));
    }

    for seat in &plot.seats {
        let at = place(seat.x, seat.y);
        let soul = who_is(seat.who);
        let card = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(at.x),
                    top: Val::Px(at.y),
                    width: Val::Px(CARD.x),
                    height: Val::Px(CARD.y),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(if seat.kin == Kin::Subject {
                    crate::ui::theme::title_bg()
                } else {
                    crate::ui::theme::panel_bg()
                }),
                // THE SUBJECT WEARS THE GOLD, so a tree of twenty faces still
                // answers "which of these is the one I opened" at a glance.
                BorderColor::all(if seat.kin == Kin::Subject {
                    crate::ui::theme::accent()
                } else {
                    crate::ui::theme::panel_border()
                }),
                ChildOf(canvas),
            ))
            .id();
        if seat.kin != Kin::Subject && !soul.dead {
            commands
                .entity(card)
                .insert((KinCard(seat.who), Interaction::default()));
        }
        commands.spawn((
            crate::ui::SerifFace,
            Text::new(if soul.dead {
                // The dagger is how a family tree has always said this.
                format!("{} \u{2020}", soul.name)
            } else {
                soul.name.clone()
            }),
            TextFont {
                font_size: bevy::text::FontSize::Px(12.0),
                ..default()
            },
            TextColor(if soul.dead {
                soul.ink.with_alpha(0.45)
            } else {
                soul.ink
            }),
            ChildOf(card),
        ));
    }
    canvas
}

// ---------------------------------------------------------------------------
// Taking hold of it: drag to pan, wheel to zoom.
// ---------------------------------------------------------------------------

/// Where the tree has been dragged to and how far in it has been zoomed.
///
/// A RESOURCE, not a component, because the page is rebuilt from nothing every
/// time the tab changes or the family does - and a view the player had arranged
/// must not snap back to the middle because a grandchild was born. Reset when a
/// different villager's window is opened, since their tree is a different shape
/// and somebody else's framing means nothing on it.
#[derive(Resource)]
pub(crate) struct FamilyView {
    pub held: Vec2,
    pub close: f32,
    /// Whose tree this framing belongs to.
    pub of: Option<Entity>,
    /// Whether the opening framing has been worked out yet.
    ///
    /// A canvas is spawned before its parent's rectangle exists, so anything
    /// placed on the frame it is built lands at the window's corner and snaps
    /// into place on the next one. Brett: "when you open the panel the tree is in
    /// the upper left corner and then pops to the center?" It stays hidden until
    /// this is true, so the first frame anybody sees is the placed one.
    pub placed: bool,
}

impl Default for FamilyView {
    fn default() -> Self {
        FamilyView {
            held: Vec2::ZERO,
            close: 1.0,
            of: None,
            placed: false,
        }
    }
}

/// How far in and out the tree may be zoomed.
///
/// Out to a third, because a dynasty six generations wide is the case that
/// needs it; in to double, because the names are set at twelve pixels and past
/// double it is a poster rather than a tree.
const NEAREST: f32 = 2.0;
const FURTHEST: f32 = 0.34;

/// Drags the tree about and zooms it, while the pointer is over it.
///
/// Hit-tested by geometry rather than `Interaction`, the same as the wheel over
/// a scrollable pane and for the same reason: the cards ARE the tree, so hover
/// would be captured by whichever card the cursor rested on and the tree would
/// pan only in the gaps between them.
pub(crate) fn drag_the_family(
    mouse: Res<ButtonInput<MouseButton>>,
    moved: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    primary: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut view: ResMut<FamilyView>,
    // The well the tree sits in, for centering it on opening.
    wells: Query<&ComputedNode>,
    mut canvas: Query<
        (
            &mut Node,
            &mut UiTransform,
            &mut Visibility,
            &ComputedNode,
            &UiGlobalTransform,
            &ChildOf,
        ),
        // WITHOUT THIS IT IS EVERY NODE IN THE GAME. An unfiltered `&mut Node`
        // query matches the HUD, the codex and the hotbar, and this system would
        // have dragged the whole interface around.
        With<FamilyCanvas>,
    >,
) {
    let Ok(primary) = primary.single() else {
        return;
    };
    let cursor = primary.cursor_position();
    for (mut node, mut transform, mut shown, computed, at, parent) in &mut canvas {
        let scale = computed.inverse_scale_factor();
        // CENTERED ON OPENING, once there is a parent rectangle to center in.
        // Brett: "this is probably the right zoom level when you open the panel" -
        // so nothing else about the opening view is touched.
        // THE GATE IS THE CANVAS'S OWN, not the resource's. The page is rebuilt
        // from nothing whenever the family changes - and every rebuild spawns a
        // fresh hidden canvas, so a flag on the resource would stay true and
        // leave the new one hidden for good. Which would have broken clicking a
        // name, the feature added an hour ago, in the least obvious way.
        if *shown == Visibility::Hidden {
            // The FRAMING is worked out once per person; a rebuild reuses it, so
            // a grandchild being born does not undo a drag.
            if !view.placed {
                let mine = computed.size() * scale;
                let room = wells
                    .get(parent.parent())
                    .map(|well| well.size() * well.inverse_scale_factor())
                    .unwrap_or(mine);
                view.held = ((room - mine) * 0.5).max(Vec2::ZERO);
                view.placed = true;
            }
            node.left = Val::Px(view.held.x);
            node.top = Val::Px(view.held.y);
            transform.scale = Vec2::splat(view.close);
            // Only now is it worth looking at.
            *shown = Visibility::Inherited;
            continue;
        }
        // Tested against the canvas's own visible box: it is the size of the
        // family, so this is generous, which is right - a tree is dragged from
        // wherever the hand happens to be resting on it.
        let over = cursor.is_some_and(|cursor| {
            let center = Vec2::new(at.translation.x, at.translation.y) * scale;
            let half = computed.size() * scale * 0.5;
            (cursor.x - center.x).abs() <= half.x && (cursor.y - center.y).abs() <= half.y
        });
        if !over {
            continue;
        }
        if mouse.pressed(MouseButton::Left) && moved.delta != Vec2::ZERO {
            view.held += moved.delta;
        }
        if wheel.delta.y != 0.0 {
            view.close = (view.close * (1.0 + wheel.delta.y * 0.12)).clamp(FURTHEST, NEAREST);
        }
        node.left = Val::Px(view.held.x);
        node.top = Val::Px(view.held.y);
        transform.scale = Vec2::splat(view.close);
    }
}

/// Walks the tree: a click on a name opens that person instead.
///
/// The tab is DELIBERATELY LEFT WHERE IT IS. Opening somebody from the world
/// starts them on their overview, which is right - you asked about a person. But
/// a click inside the tree is a step through the family, and being thrown to an
/// overview every step would make walking a dynasty a matter of five clicks per
/// generation.
pub(crate) fn walk_the_family(
    mut cards: Query<(&Interaction, &KinCard, &mut BorderColor), Changed<Interaction>>,
    mut selected: ResMut<crate::debug::people::SelectedPerson>,
    mut view: ResMut<FamilyView>,
) {
    for (how, card, mut border) in &mut cards {
        match how {
            Interaction::Pressed => {
                selected.0 = Some(card.0);
                // Their tree is a different shape, so it opens framed afresh.
                *view = FamilyView {
                    of: Some(card.0),
                    ..Default::default()
                };
            }
            // The only hint that a name can be walked to. Cheap, and it is what
            // makes the feature discoverable at all.
            Interaction::Hovered => {
                *border = BorderColor::all(crate::ui::theme::accent());
            }
            Interaction::None => {
                *border = BorderColor::all(crate::ui::theme::panel_border());
            }
        }
    }
}
