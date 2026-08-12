//! Buildings carried in from Opificium.
//!
//! The bench and the game share no code: the bench resolves its own
//! catalogue and palette and hands over plain boxes with colours, plus
//! the marks that say what a place is FOR. This module reads those
//! files, raises the boxes stage by stage, and turns the marks into the
//! components the village already knows - beds, a shell with doors, the
//! family table.

use bevy::prelude::*;

use super::buildings::{Bed, Doorway, RoofPart, Shell, Table, WallPart};

/// One box of a baked building, in the building's own space.
#[derive(serde::Deserialize, Clone)]
pub struct Box3 {
    pub at: [f32; 3],
    pub size: [f32; 3],
    /// The turn, as the bench wrote it.
    pub turn: [f32; 4],
    pub rgb: [u8; 3],
    #[serde(default = "opaque")]
    pub alpha: f32,
    /// "box", "wedge", "ridge", "mitre" or "mitre-back" - the bench's shapes.
    #[serde(default)]
    pub form: String,
    /// footing, walls, roof, furnishing.
    pub stage: String,
}

fn opaque() -> f32 {
    1.0
}

/// A place that means something: sleep, sit, fire, smoke, door, table,
/// store, work, light.
#[derive(serde::Deserialize, Clone)]
pub struct Mark {
    pub mark: String,
    pub at: [f32; 3],
    pub yaw: f32,
}

/// A whole building as the bench baked it.
#[derive(serde::Deserialize, Clone)]
pub struct Baked {
    /// Which shape of file this is. Absent on drawings baked before the
    /// bench stamped it, which are read as the format of their day.
    #[serde(default)]
    pub format: u32,
    pub name: String,
    /// What the village should raise this AS, said outright by the maker who
    /// baked it. A drawing used to be claimed by whatever kind-word its file
    /// name happened to begin with, which is a rule a maker has to know and can
    /// get wrong in silence - `millhouse` is a mill with a surprise in it. The
    /// bench asks now, so the answer is a fact rather than a guess. Empty on the
    /// drawings baked before it asked, which fall back to the old reading.
    #[serde(default)]
    pub kind: String,
    pub half_w: f32,
    pub half_d: f32,
    #[allow(dead_code)]
    pub high: f32,
    pub boxes: Vec<Box3>,
    pub marks: Vec<Mark>,
}

/// A building can be raised as drawn or as its own reflection, and half of them
/// are. Brett: "would it be hard to have it rng between placing it and placing
/// it as a mirror for variety?" - one drawing becomes two buildings for nothing
/// but a sign change, and a street of one blueprint stops reading as a row of
/// stamps.
///
/// The mirror runs along the building's LENGTH, not across it. Every building is
/// turned so its own +X faces the square, because that is the way its front door
/// looks - so reflecting x is the one reflection that cannot be used: it carries
/// the door round to the back wall, and Brett saw it straightaway - "it mirroed
/// it but now the door is facing the wrong way". Reflecting z leaves the front
/// wall the front wall and swaps the building's hands, which is the variety that
/// was wanted.
///
/// Mirroring by SCALE would be the easy way and the wrong one besides: a
/// negative scale turns every triangle inside out, and the whole building would
/// light as though lit from within. So the reflection is done to the numbers -
/// each box's place, each box's turn, each mark's place and facing - and the
/// geometry stays wound the way it was built. Along z, no shape even changes
/// hands: a mitre's cut runs across x, and a gable prism peaks in its middle.
fn reflect_at(at: Vec3, mirrored: bool) -> Vec3 {
    if mirrored {
        Vec3::new(at.x, at.y, -at.z)
    } else {
        at
    }
}

/// A turn seen in the same mirror: the axis reflected and the angle reversed,
/// which for a quaternion is exactly this.
fn reflect_turn(turn: Quat, mirrored: bool) -> Quat {
    if mirrored {
        Quat::from_xyzw(-turn.x, -turn.y, turn.z, turn.w)
    } else {
        turn
    }
}

/// Which way a mark faces, on the other hand. A mark's nose is its own +X, so
/// reflecting the direction it points and reading the angle back off it gives
/// this - a door in the front wall stays in the front wall, and a bed's sleeper
/// lies with their head the other way about.
fn reflect_yaw(yaw: f32, mirrored: bool) -> f32 {
    if mirrored { -yaw } else { yaw }
}

/// Every door the maker marked, where they marked it, facing the way its nose
/// points - out of the building. Read in one place because three of them need
/// it: the shell a villager is routed through, the hall's doorstep, and the
/// reflection has to reach all of them or people walk at a wall.
pub fn doorways(work: &Baked, mirrored: bool) -> Vec<Doorway> {
    work.marks
        .iter()
        .filter(|m| m.mark == "door")
        .map(|m| {
            let at = reflect_at(Vec3::from(m.at), mirrored);
            let out = Quat::from_rotation_y(reflect_yaw(m.yaw, mirrored)) * Vec3::X;
            Doorway {
                at: Vec2::new(at.x, at.z),
                out: Vec2::new(out.x, out.z).normalize_or(Vec2::X),
            }
        })
        .collect()
}

/// The buildings carried in, read once and held for the life of the
/// run. A plain lookup rather than a resource: the raising happens deep
/// inside helper functions that would otherwise all have to be handed
/// the thing.
static CARRIED: std::sync::OnceLock<Vec<Baked>> = std::sync::OnceLock::new();

fn carried() -> &'static Vec<Baked> {
    CARRIED.get_or_init(|| {
        let mut works: Vec<Baked> = Vec::new();
        // What shipped, and then the maker's own on top of it. That second
        // folder is what makes the shipped pair self-sufficient: a building
        // baked out of the bench in the launcher build has somewhere to land,
        // with no source tree anywhere in the story.
        for dir in [
            crate::carried::folder("assets/buildings"),
            crate::carried::made_by_hand("buildings"),
        ]
        .into_iter()
        .flatten()
        {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                match std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Baked>(&text).ok())
                {
                    Some(work) => {
                        if let Some(why) = turned_away(&work) {
                            warn!("{}: {why}. Not carried in.", work.name);
                            continue;
                        }
                        read_the_words(&work);
                        // A maker's drawing of the same name replaces the one
                        // that shipped: they drew it second, and meant it. It
                        // says so rather than doing it quietly - a village
                        // missing a building it was expecting is a thing to be
                        // able to read in a log.
                        match works.iter().position(|had| had.name == work.name) {
                            Some(standing) => {
                                info!(
                                    "carried in {}: {} boxes, {} marks - replacing the one \
                                     already carried in under that name",
                                    work.name,
                                    work.boxes.len(),
                                    work.marks.len()
                                );
                                works[standing] = work;
                            }
                            None => {
                                info!(
                                    "carried in {}: {} boxes, {} marks",
                                    work.name,
                                    work.boxes.len(),
                                    work.marks.len()
                                );
                                works.push(work);
                            }
                        }
                    }
                    None => warn!("could not read the baked building at {}", path.display()),
                }
            }
        }
        // read_dir's order is the filesystem's business; a world seed's is ours.
        works.sort_by(|a, b| a.name.cmp(&b.name));
        works
    })
}

/// The kinds this game can raise, as Opificium reads them.
pub fn kinds_as_json() -> String {
    let kinds: Vec<String> = super::BuildingKind::every()
        .iter()
        .map(|kind| match called(*kind) {
            // TOWN HALL on the bench's shelf; `townhall` in the file. The word
            // is the contract, the label is only what a maker reads.
            "townhall" => "    { \"word\": \"townhall\", \"label\": \"TOWN HALL\" }".to_string(),
            word => format!("    {{ \"word\": \"{word}\" }}"),
        })
        .collect();
    format!(
        "{{\n  \"format\": 1,\n  \"kinds\": [\n{}\n  ]\n}}\n",
        kinds.join(",\n")
    )
}

/// The marks this game acts on, as Opificium reads them.
pub fn widgets_as_json() -> String {
    let marks: Vec<String> = MARKS_UNDERSTOOD
        .iter()
        .map(|(mark, ramp, shade)| {
            format!("    {{ \"mark\": \"{mark}\", \"ramp\": \"{ramp}\", \"shade\": {shade} }}")
        })
        .collect();
    format!(
        "{{\n  \"format\": 1,\n  \"marks\": [\n{}\n  ]\n}}\n",
        marks.join(",\n")
    )
}

/// Keeps a maker's own Opificium project furnished with this game's truth.
///
/// The point of the whole arrangement: somebody who has never seen this
/// repository - who bought the game and downloaded the bench - opens Opificium,
/// points it at this folder, and is drawing in the game's own colours with the
/// game's own vocabulary, for the game's own VERSION. Nothing to copy, nothing
/// to keep in step, and no way to author a building against a contract the
/// installed game does not honour.
///
/// `install` points at the buildings folder the game already reads, so a bake
/// lands where the next launch finds it. The bench writes its own README into
/// the folder, and the drawings are the maker's; everything written here is
/// this game's side of the contract and nothing else.
///
/// Written every launch, but only where it differs - a file rewritten for no
/// reason is a file whose timestamp lies about when the game last changed.
pub fn furnish_the_makers_bench() {
    let Some(bench) = crate::carried::made_by_hand("opificium") else {
        return;
    };
    let data = bench.join("data");
    if std::fs::create_dir_all(&data).is_err() {
        return;
    }

    // `install` is RELATIVE to the project, and points back out at the folder
    // this game reads a maker's own buildings from.
    //
    // DO NOT DROP THIS LINE AS REDUNDANT. Opificium's default for `install` is
    // `../assets/buildings`, which is right for a project living inside a
    // game's repository and wrong for this one: this project sits in
    // Application Support beside the saves, and the buildings the game reads
    // there are `made_by_hand("buildings")` - a sibling, not an `assets`
    // folder. Left to the default, a player's bakes would land in
    // `<app support>/Divus Factus/assets/buildings`, where nothing ever
    // looks, and their building would simply never appear with no error to
    // read. This is the case `install` exists for.
    let manifest = "{\n  \"format\": 1,\n  \"name\": \"Divus Factus\",\n  \
                    \"install\": \"../buildings\"\n}\n";
    let written = [
        (bench.join("opificium.json"), manifest.to_string()),
        (data.join("palette.json"), crate::palette::as_json()),
        (data.join("kinds.json"), kinds_as_json()),
        (data.join("widgets.json"), widgets_as_json()),
    ];
    let mut fresh = 0;
    for (road, said) in written {
        let same = std::fs::read_to_string(&road).is_ok_and(|had| had == said);
        if same {
            continue;
        }
        if std::fs::write(&road, said).is_ok() {
            fresh += 1;
        }
    }
    if fresh > 0 {
        info!(
            "the maker's bench is furnished with this game's palette and words: {}",
            bench.display(),
        );
    }
}

/// The newest baked format this game can read.
///
/// A drawing from a NEWER bench is refused rather than half-read. Every field
/// this game does not recognise is dropped in silence by the reader, so a
/// format that moved something - marks into levels, a box's colour into a
/// reference - would arrive as a building with no doors and no beds rather
/// than as an error. That is a fine way to lose an afternoon on your own work
/// and an unacceptable one to treat a stranger's.
const NEWEST_FORMAT: u32 = 2;

/// What one drawing may be, at the outside.
///
/// Not a budget - a real building is nowhere near any of these - but a floor
/// under the damage a bad file can do. Every one of these is a plain JSON
/// number in a file the game did not write, and once buildings arrive from
/// the Workshop they are numbers written by strangers. A drawing with two
/// hundred thousand boxes should be refused at the door with a message, not
/// raised into a village where it becomes a frozen machine nobody can explain.
const AT_MOST_BOXES: usize = 20_000;
const AT_MOST_MARKS: usize = 2_000;
/// Metres, half-extent. The largest thing anybody has drawn is a longhouse.
const AT_MOST_ACROSS: f32 = 200.0;

/// Whether this drawing can be let in at all.
///
/// Returns why not, for saying out loud. Refusal has to be LOUD and NAMED:
/// a maker - or a modder who has never seen this source - is owed the
/// difference between "your building is wrong" and "nothing happened".
fn turned_away(work: &Baked) -> Option<String> {
    if work.format > NEWEST_FORMAT {
        return Some(format!(
            "baked in format {} by a newer Opificium than this game can read \
             (it knows up to {NEWEST_FORMAT}) - update the game, or bake it again \
             from the drawing with a matching bench",
            work.format,
        ));
    }
    if work.boxes.is_empty() {
        return Some("has no boxes in it at all".to_string());
    }
    if work.boxes.len() > AT_MOST_BOXES {
        return Some(format!(
            "has {} boxes, past the {AT_MOST_BOXES} this game will raise",
            work.boxes.len(),
        ));
    }
    if work.marks.len() > AT_MOST_MARKS {
        return Some(format!(
            "has {} marks, past the {AT_MOST_MARKS} this game will read",
            work.marks.len(),
        ));
    }
    let across = work.half_w.max(work.half_d);
    if !across.is_finite() || !work.high.is_finite() {
        return Some("measures itself in numbers that are not numbers".to_string());
    }
    if across > AT_MOST_ACROSS {
        return Some(format!(
            "measures {across:.0}m from its middle, past the {AT_MOST_ACROSS:.0}m \
             a single building may cover",
        ));
    }
    if work.half_w <= 0.0 || work.half_d <= 0.0 {
        return Some("has no footprint to stand on".to_string());
    }
    None
}

/// Every mark this game acts on, and the colour the bench should draw it.
///
/// THE GAME IS THE AUTHORITY ON WHAT A WORD MEANS. Opificium offers only the
/// marks listed in a project's `data/widgets.json`, passes the word through
/// untouched, and cannot check it against anything - it has never seen this
/// source. So a mark the bench offers that this list does not carry is a trap:
/// a maker places it, it bakes, it arrives, and nothing happens, with no error
/// anywhere to say why.
///
/// Which is exactly what had happened. The bench was offering `sit`, `fire`,
/// `smoke`, `work`, `store` and `light`, none of which anything here reads -
/// and two of them were already standing in authored buildings, doing nothing.
/// Meanwhile `table`, which this game DOES act on, was not offered at all, so
/// no maker could place one.
///
/// `export_widgets_for_opificium` writes this list out as the project's
/// widgets file, the same way the palette is exported. Add a mark here, teach
/// the reader below to act on it, re-run the export, and the bench offers it.
/// That is the whole ceremony, and it cannot drift.
pub(crate) const MARKS_UNDERSTOOD: &[(&str, &str, f32)] = &[
    // A doorway: where a wall may be walked through.
    ("door", "cloth-green", 0.6),
    // A bed: one villager sleeps here, and a house holds as many as it has.
    ("sleep", "cloth-blue", 0.7),
    // A table: where the household eats.
    ("table", "wood", 0.6),
];

/// Says so when a drawing carries a word this game does not know.
///
/// Both directions of the contract fail silently otherwise - an unknown KIND
/// means the drawing is never offered to any village, and an unknown MARK
/// means whatever it was placed for simply never happens. Neither is
/// detectable from the bench, so the game is the only place that can complain,
/// and it should complain loudly and name what it does understand.
fn read_the_words(work: &Baked) {
    if claimed_by(work).is_none() {
        let known: Vec<&str> = super::BuildingKind::every()
            .iter()
            .map(|kind| called(*kind))
            .collect();
        // Two ways to arrive unclaimed, and the second one is quieter and
        // more likely. A STATED kind that matches nothing is a typo or a
        // building this game does not have. NO stated kind - every drawing
        // baked before the bench started asking - falls back to "the kind is
        // whatever word begins the file name", and a perfectly good drawing
        // called `guard-tower1` matches nothing at all, because the word here
        // is `watchtower`. It bakes, it is valid, it is simply never raised.
        match work.kind.is_empty() {
            false => warn!(
                "{}: baked as \"{}\", which this game has no building for - it will \
                 never be raised. Known kinds: {}",
                work.name,
                work.kind,
                known.join(", "),
            ),
            true => warn!(
                "{}: says no kind, and no known kind begins its name, so nothing \
                 will ever raise it. Bake it again from a bench that asks what it \
                 is, or rename it to begin with one of: {}",
                work.name,
                known.join(", "),
            ),
        }
    }
    let mut complained: Vec<&str> = Vec::new();
    for mark in &work.marks {
        let known = MARKS_UNDERSTOOD.iter().any(|(word, ..)| *word == mark.mark);
        if known || complained.contains(&mark.mark.as_str()) {
            continue;
        }
        complained.push(&mark.mark);
        warn!(
            "{}: carries the mark \"{}\", which this game does not act on - it will \
             do nothing. Marks understood: {}",
            work.name,
            mark.mark,
            MARKS_UNDERSTOOD
                .iter()
                .map(|(word, ..)| *word)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
}

/// What a kind's drawings are named.
///
/// EVERY kind has a word now. Brett: "When I have at least one building for a
/// building type the defaults can be erased" - so a drawing saved as
/// `tavern-corner.baz` becomes the tavern the moment it is carried in, with
/// nothing to register and no code to change. A kind nobody has drawn falls back
/// to the village's own hand, exactly as everything did before there was an
/// Opificium; a kind somebody HAS drawn never falls back again.
fn called(kind: super::BuildingKind) -> &'static str {
    use super::BuildingKind as K;
    match kind {
        K::House => "house",
        K::Longhouse => "longhouse",
        K::Sawmill => "sawmill",
        K::Blacksmith => "blacksmith",
        K::Tavern => "tavern",
        K::TownHall => "townhall",
        K::Storehouse => "storehouse",
        K::Granary => "granary",
        K::Well => "well",
        K::Smokehouse => "smokehouse",
        K::Mill => "mill",
        K::Bakery => "bakery",
        K::Weaver => "weaver",
        K::Herbalist => "herbalist",
        K::Watchtower => "watchtower",
        K::Shrine => "shrine",
        K::Dock => "dock",
        K::Mine => "mine",
    }
}

/// Every word a drawing could be claimed by, longest first.
///
/// Which is the whole trick, and it used to be a special case written out by
/// hand for the one collision anybody had noticed: "longhouse1" is not a house.
/// There are three of those pairs now - mill inside sawmill, house inside
/// longhouse, mill inside smokehouse - and a rule that ruled out one of them by
/// name would leave the others to be found the hard way, in a village where the
/// sawmill had quietly become a mill.
///
/// So a drawing belongs to the LONGEST word that begins its name, and every kind
/// is checked against that one answer.
fn claimed_by(work: &Baked) -> Option<super::BuildingKind> {
    // What the maker SAID, first and last.
    if !work.kind.is_empty() {
        return super::BuildingKind::every()
            .iter()
            .copied()
            .find(|kind| called(*kind) == work.kind);
    }
    claimed_by_name(&work.name)
}

/// The older reading, for a drawing baked before the bench asked: the kind is
/// whatever word begins its name.
fn claimed_by_name(name: &str) -> Option<super::BuildingKind> {
    let mut kinds: Vec<super::BuildingKind> = super::BuildingKind::every().to_vec();
    kinds.sort_by_key(|kind| std::cmp::Reverse(called(*kind).len()));
    kinds
        .into_iter()
        .find(|kind| name.starts_with(called(*kind)))
}

/// Every drawing carried in for this kind, in a settled order - so a
/// world seed raises the same street twice.
pub fn drawings(kind: super::BuildingKind) -> Vec<&'static Baked> {
    carried()
        .iter()
        .filter(|work| claimed_by(work) == Some(kind))
        .collect()
}

/// The drawing a given plan follows. Plans are rolled per building, so a
/// village of one blueprint became a village of however many the maker
/// has drawn - and the roll survives a save, because it lives in the
/// blueprint rather than being worked out again from the entity.
pub fn drawing_at(kind: super::BuildingKind, plan: usize) -> Option<&'static Baked> {
    let all = drawings(kind);
    (!all.is_empty()).then(|| all[plan % all.len()])
}

/// The drawing a standing building follows: BY NAME where it has one.
///
/// A blueprint's `plan` is an index into the drawings for its kind, and an
/// index only means anything while the list behind it holds still. Carry one
/// new house into the folder and the list grows: every standing house's
/// `plan % len` lands somewhere else, and a village loaded from a save comes
/// back as different buildings - standing on ground that was terraced for the
/// footprints of the old ones. Which is precisely what a maker does, over and
/// over, all afternoon.
///
/// So the name is the answer where there is one, and the index is kept for the
/// saves written before there was. A named drawing that has since been deleted
/// falls back the same way rather than leaving a hole in the village.
pub fn drawing_of(kind: super::BuildingKind, plan: usize, named: &str) -> Option<&'static Baked> {
    if !named.is_empty()
        && let Some(known) = drawings(kind).into_iter().find(|work| work.name == named)
    {
        return Some(known);
    }
    drawing_at(kind, plan)
}

/// The widest drawing of a kind. Ground is broken before a plan is
/// rolled, so the plot has to be cut for whichever one turns up.
pub fn widest(kind: super::BuildingKind) -> Option<f32> {
    drawings(kind)
        .iter()
        .map(|work| work.half_w.max(work.half_d))
        .fold(None, |most: Option<f32>, reach| {
            Some(most.map_or(reach, |m| m.max(reach)))
        })
}

/// The fewest beds any drawing of this kind holds - what the planner may
/// safely promise before it knows which one will be rolled. A carried-in
/// building sleeps the beds its maker drew, not a constant: the bench
/// longhouse holds ten, and a village that believed eight would break
/// ground for a second hall it did not need.
pub fn beds(kind: super::BuildingKind) -> Option<usize> {
    drawings(kind)
        .iter()
        .map(|work| work.marks.iter().filter(|m| m.mark == "sleep").count())
        .filter(|beds| *beds > 0)
        .min()
}

/// A truncated pyramid: four faces sloping in from the foot to a flat top.
///
/// The bench's hip roof. Written out here corner for corner as well as there,
/// like every other shape the two programs share - see `FORMATS.md`. The two
/// numbers are how much of the box the flat top keeps along each axis.
fn hip(keep_x: f32, keep_z: f32) -> Mesh {
    let (tx, tz) = (keep_x.clamp(0.0, 1.0) * 0.5, keep_z.clamp(0.0, 1.0) * 0.5);
    let foot = [
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, 0.5),
        Vec3::new(-0.5, -0.5, 0.5),
    ];
    let deck = [
        Vec3::new(-tx, 0.5, -tz),
        Vec3::new(tx, 0.5, -tz),
        Vec3::new(tx, 0.5, tz),
        Vec3::new(-tx, 0.5, tz),
    ];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut face = |corners: [Vec3; 4]| {
        let first = positions.len() as u32;
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or(Vec3::Y);
        for corner in corners {
            positions.push(corner.to_array());
            normals.push(normal.to_array());
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    };
    // The deck looks UP and the underside looks DOWN; wound the other way each
    // is culled, and a roof with no top reads as a sunken tray.
    face([deck[3], deck[2], deck[1], deck[0]]);
    face([foot[0], deck[0], deck[1], foot[1]]);
    face([foot[1], deck[1], deck[2], foot[2]]);
    face([foot[2], deck[2], deck[3], foot[3]]);
    face([foot[3], deck[3], deck[0], foot[0]]);
    face([foot[1], foot[2], foot[3], foot[0]]);
    let uvs: Vec<[f32; 2]> = positions.iter().map(|_| [0.0, 0.0]).collect();
    Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}

/// A gable's prism, and the ridge cap's, cut the way the bench cuts
/// them: unit-sized, so a box's scale shapes them.
/// A right-angle prism: a box with one end cut clean through at an angle.
///
/// The saw's own shape, and the newest word in the file contract. A wedge is a
/// GABLE's prism, two slopes meeting at a peak; this is the far commoner cut,
/// and without it a beam meeting a roof had to stop square and stand off it.
///
/// The vertices are Opificium's, corner for corner. The two programs share no
/// code and never will, so the only thing keeping a shape the same shape in both
/// is that somebody wrote it out twice and said so - see `FORMATS.md`.
/// A unit box with its top or bottom face cut back at each end.
///
/// `low` and `high` are RUNS as fractions of the box's own length: how far along
/// it the saw travels while crossing its full height, at the -X end and the +X
/// end. Nought is a square end. A POSITIVE run cuts the top face back, a
/// NEGATIVE one cuts the bottom.
///
/// One shape for every angled end there is. There used to be two - a mitre and
/// its mirror - because a beam can be cut at one end or the other, and neither
/// could do both at once. A run of one takes a face all the way to the far
/// corner, which IS the old full mitre, so nothing that could be drawn before
/// has stopped being drawable.
///
/// The signs are what make a brace possible. Cut the top at one end and the
/// bottom at the other and the two ends come out PARALLEL - a parallelogram,
/// which is what a diagonal brace is, since both of its ends meet horizontal
/// timber.
fn cut_mesh(low: f32, high: f32) -> Mesh {
    let (low, high) = (low.clamp(-1.0, 1.0), high.clamp(-1.0, 1.0));
    // A run may be NEGATIVE, and that is what lets a brace exist. A positive
    // run cuts the top face back; a negative one cuts the bottom. Cut the top at
    // one end and the bottom at the other by the same amount and the two ends
    // come out PARALLEL - a parallelogram, which is what a diagonal brace is,
    // because both of its ends meet horizontal timber. With the top cut at both
    // ends the ends converge instead, and a brace would sit in its bay like a
    // wedge.
    let inset = |run: f32| (run.max(0.0), (-run).max(0.0));
    let (top_low, foot_low) = inset(low);
    let (top_high, foot_high) = inset(high);
    // Cuts that would cross each other leave a face inside out; share the
    // length between them instead.
    let share = |a: f32, b: f32| {
        if a + b > 1.0 {
            (a / (a + b), b / (a + b))
        } else {
            (a, b)
        }
    };
    let (top_low, top_high) = share(top_low, top_high);
    let (foot_low, foot_high) = share(foot_low, foot_high);

    let (ta, tb) = (-0.5 + top_low, 0.5 - top_high);
    let (fa, fb) = (-0.5 + foot_low, 0.5 - foot_high);
    let top_peak = tb <= ta;
    let foot_peak = fb <= fa;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Normals from the corners themselves, and wound to face outward.
    //
    // The shape is convex and centred on the origin, so a face's own middle
    // points the way that face does - which settles both the normal's sign and
    // the winding without anyone having to reason about which end is cut. The
    // slanted ends are where doing it by hand goes wrong, and they are the
    // whole point of this mesh.
    let mut face = |corners: &[Vec3]| {
        if corners.len() < 3 {
            return;
        }
        let middle = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        let Some(normal) = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .try_normalize()
        else {
            return;
        };
        let (normal, flip) = if normal.dot(middle) < 0.0 {
            (-normal, true)
        } else {
            (normal, false)
        };
        let first = positions.len() as u32;
        let mut corners = corners.to_vec();
        if flip {
            corners.reverse();
        }
        for corner in &corners {
            positions.push(corner.to_array());
            normals.push(normal.to_array());
        }
        for step in 1..(corners.len() as u32 - 1) {
            indices.extend_from_slice(&[first, first + step, first + step + 1]);
        }
    };

    let at = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);

    // The underside and the top, each shortened by whatever was cut from it.
    if !foot_peak {
        face(&[
            at(fa, -0.5, -0.5),
            at(fb, -0.5, -0.5),
            at(fb, -0.5, 0.5),
            at(fa, -0.5, 0.5),
        ]);
    }
    if !top_peak {
        face(&[
            at(ta, 0.5, -0.5),
            at(tb, 0.5, -0.5),
            at(tb, 0.5, 0.5),
            at(ta, 0.5, 0.5),
        ]);
    }
    // The sides, walked round in order so a collapsed edge simply drops out.
    for z in [-0.5f32, 0.5] {
        let mut corners = vec![at(fa, -0.5, z)];
        if !foot_peak {
            corners.push(at(fb, -0.5, z));
        }
        corners.push(at(tb, 0.5, z));
        if !top_peak {
            corners.push(at(ta, 0.5, z));
        }
        face(&corners);
    }
    // And the ends themselves, square where nothing was cut and leaning where
    // something was.
    face(&[
        at(fa, -0.5, -0.5),
        at(fa, -0.5, 0.5),
        at(ta, 0.5, 0.5),
        at(ta, 0.5, -0.5),
    ]);
    face(&[
        at(fb, -0.5, -0.5),
        at(fb, -0.5, 0.5),
        at(tb, 0.5, 0.5),
        at(tb, 0.5, -0.5),
    ]);
    Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}

fn prism(lengthwise: bool) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut face = |corners: &[[f32; 3]], normal: [f32; 3]| {
        let first = positions.len() as u32;
        for corner in corners {
            positions.push(*corner);
            normals.push(normal);
        }
        for step in 1..(corners.len() as u32 - 1) {
            indices.extend_from_slice(&[first, first + step, first + step + 1]);
        }
    };
    let slope = (2.0f32 / 5.0f32.sqrt(), 1.0 / 5.0f32.sqrt());
    face(
        &[[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.0, 0.5, 0.5]],
        [0.0, 0.0, 1.0],
    );
    face(
        &[[0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [0.0, 0.5, -0.5]],
        [0.0, 0.0, -1.0],
    );
    face(
        &[
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, -0.5, -0.5],
            [-0.5, -0.5, -0.5],
        ],
        [0.0, -1.0, 0.0],
    );
    face(
        &[
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.0, 0.5, 0.5],
            [0.0, 0.5, -0.5],
        ],
        [-slope.0, slope.1, 0.0],
    );
    face(
        &[
            [0.5, -0.5, 0.5],
            [0.5, -0.5, -0.5],
            [0.0, 0.5, -0.5],
            [0.0, 0.5, 0.5],
        ],
        [slope.0, slope.1, 0.0],
    );
    if lengthwise {
        for corner in &mut positions {
            *corner = [corner[2], corner[1], corner[0]];
        }
        for normal in &mut normals {
            *normal = [normal[2], normal[1], normal[0]];
        }
        for triangle in indices.chunks_mut(3) {
            triangle.swap(1, 2);
        }
    }
    let uvs: Vec<[f32; 2]> = positions.iter().map(|_| [0.0, 0.0]).collect();
    Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}

/// Which of the game's three build stages a baked stage belongs to.
fn stage_of(stage: &str, framed: bool) -> u8 {
    // A house drawn with no frame rises in three steps, not four, so its
    // walls and roof must move DOWN a step to match - or the last step
    // is one the build never reaches, and the roof and the furniture
    // never arrive at all.
    match (stage, framed) {
        ("footing", _) => 0,
        ("frame", _) => 1,
        ("walls", true) => 2,
        ("walls", false) => 1,
        (_, true) => 3,
        (_, false) => 2,
    }
}

/// Whether a carried-in building has a frame worth showing on its own -
/// posts standing on the footing before a single wall goes up.
pub fn has_frame(work: &Baked) -> bool {
    work.boxes.iter().any(|piece| piece.stage == "frame")
}

/// Raises one stage of a carried-in building under its site.
pub fn raise_baked(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    site: Entity,
    stage: u8,
    work: &Baked,
    mirrored: bool,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let wedge = meshes.add(prism(false));
    let ridge = meshes.add(prism(true));
    let framed = has_frame(work);
    for piece in work
        .boxes
        .iter()
        .filter(|b| stage_of(&b.stage, framed) == stage)
    {
        // Exactly the colour it was painted, everywhere, always.
        //
        // The village used to re-dye each house's dominant wall and roof cloth
        // with a colour rolled from its plan, so a street of one blueprint was
        // still a street of different houses. That was the right answer while a
        // drawing arrived in whatever colours the catalogue happened to hold and
        // nobody had chosen them. There is a brush on the bench now, and Brett
        // used it - "we can remove the old painting system now that I can paint
        // in atelier" - so the roll had become a thing that painted OVER the
        // maker, and on precisely the pieces they cared most about.
        let colour = Color::srgb_u8(piece.rgb[0], piece.rgb[1], piece.rgb[2]);
        let clear = piece.alpha < 0.999;
        let mesh = match piece.form.as_str() {
            "wedge" => wedge.clone(),
            "ridge" => ridge.clone(),
            // A cut carries its two runs, as fractions of the piece's own
            // length - "cut:0.0000x0.3300". One form for every angled end.
            //
            // "mitre" and "mitre-back" were the same thing said two ways, and
            // each could only cut ALL of one end; they are still read, as the
            // full cuts they always were, so a drawing made before this still
            // opens.
            "mitre" => meshes.add(cut_mesh(0.0, 1.0)),
            "mitre-back" => meshes.add(cut_mesh(1.0, 0.0)),
            cutting if cutting.starts_with("cut:") => {
                let mut runs = cutting.trim_start_matches("cut:").split('x');
                let low = runs.next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
                let high = runs.next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
                meshes.add(cut_mesh(low, high))
            }
            // A hip roof carries its own proportions in its form - "hip:0.5x0.6"
            // - because a truncated pyramid is a different mesh at every deck
            // size and a name alone cannot say which. See opificium/FORMATS.md.
            hipped if hipped.starts_with("hip:") => {
                let mut parts = hipped.trim_start_matches("hip:").split('x');
                let keep_x = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0.5);
                let keep_z = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0.5);
                meshes.add(hip(keep_x, keep_z))
            }
            _ => cube.clone(),
        };
        let mut raised = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: colour.with_alpha(piece.alpha),
                perceptual_roughness: 0.95,
                alpha_mode: if clear {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Opaque
                },
                ..default()
            })),
            Transform::from_translation(reflect_at(Vec3::from(piece.at), mirrored))
                .with_rotation(reflect_turn(Quat::from_array(piece.turn), mirrored))
                .with_scale(Vec3::from(piece.size)),
            ChildOf(site),
        ));
        // The roof lifts for the cutaway, and the walls come down after
        // it - windows, doors and their frames with them, since they
        // are set in the walls.
        if piece.stage == "roof" {
            raised.insert(RoofPart);
        } else if piece.stage == "walls" || piece.stage == "frame" {
            raised.insert(WallPart);
        }
    }
}

/// Turns the marks into what the village reads: beds it can claim, a
/// shell with doors to walk through, the family table.
pub fn furnish_baked(commands: &mut Commands, site: Entity, work: &Baked, mirrored: bool) {
    let mut slot = 0u8;
    let sleeps: Vec<&Mark> = work.marks.iter().filter(|m| m.mark == "sleep").collect();
    for (index, mark) in sleeps.iter().enumerate() {
        // Two sleeping places lying alongside each other are the two
        // halves of one marriage bed - the pair sleeps there and the
        // children do not, whoever set them down.
        let at = reflect_at(Vec3::from(mark.at), mirrored);
        let double = sleeps.iter().enumerate().any(|(other, twin)| {
            other != index && Vec3::from(twin.at).distance(Vec3::from(mark.at)) < 1.4
        });
        // A mark faces its own +X, the way every mark does: for a
        // sleeper that is the way their head lies. Tipped onto its back a
        // body's head points along -Z, so the turn that carries it to the
        // pillow is read straight off that direction.
        let head_way = Quat::from_rotation_y(reflect_yaw(mark.yaw, mirrored)) * Vec3::X;
        let lie = super::buildings::lie_toward(head_way);
        commands.spawn((
            Bed { slot, lie, double },
            Transform::from_translation(at),
            Visibility::Hidden,
            ChildOf(site),
        ));
        slot += 1;
    }
    for mark in work.marks.iter().filter(|m| m.mark == "table") {
        commands.spawn((
            Table,
            Transform::from_translation(reflect_at(Vec3::from(mark.at), mirrored)),
            Visibility::Hidden,
            ChildOf(site),
        ));
    }
    let doors = doorways(work, mirrored);
    // The shell is the WALLS, not the whole building. The file's own
    // half-extents take in the roof, which on a house with eaves reaches
    // most of a metre past the wall it shelters - and a doorway that
    // measures as INSIDE the shell is a doorway nobody can be routed
    // through. They walk at the wall beside it instead.
    let (mut half_w, mut half_d) = (0.0_f32, 0.0_f32);
    for piece in work
        .boxes
        .iter()
        .filter(|b| b.stage == "walls" || b.stage == "frame")
    {
        let turn = Quat::from_array(piece.turn);
        let half = Vec3::from(piece.size) * 0.5;
        // A turned box's reach along each world axis.
        let reach = (turn * Vec3::new(half.x, 0.0, 0.0)).abs()
            + (turn * Vec3::new(0.0, half.y, 0.0)).abs()
            + (turn * Vec3::new(0.0, 0.0, half.z)).abs();
        half_w = half_w.max(piece.at[0].abs() + reach.x);
        half_d = half_d.max(piece.at[2].abs() + reach.z);
    }
    if half_w <= 0.0 || half_d <= 0.0 {
        half_w = work.half_w;
        half_d = work.half_d;
    }
    commands.entity(site).insert(Shell {
        half_w,
        half_d,
        doors: if doors.is_empty() {
            vec![Doorway::on_x_wall(half_w, 0.0)]
        } else {
            doors
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::BuildingKind;

    #[test]
    fn every_drawing_in_the_folder_is_carried_and_dealt_in_turn() {
        for kind in [BuildingKind::House, BuildingKind::Longhouse] {
            let all = drawings(kind);
            // NO DRAWINGS IS A LEGITIMATE WORLD. A kind nobody has drawn
            // falls back to the village's own hand, exactly as everything
            // did before there was an Opificium - so this asserts what is
            // true OF carried drawings rather than that any exist. It used
            // to demand them, which made an empty folder a test failure
            // rather than a fresh start.
            if all.is_empty() {
                continue;
            }
            // A settled order, so a world seed raises the same street twice.
            let mut names: Vec<&str> = all.iter().map(|w| w.name.as_str()).collect();
            let given = names.clone();
            names.sort_unstable();
            assert_eq!(given, names, "the {kind:?} drawings came back out of order");
            // Plans cycle, and a plan past the end wraps rather than panics.
            for plan in 0..all.len() * 2 + 3 {
                let picked = drawing_at(kind, plan).expect("a plan always finds a drawing");
                assert_eq!(picked.name, all[plan % all.len()].name);
            }
            // The plot is cut for whichever one turns up.
            let widest = widest(kind).expect("a drawing has a width");
            assert!(all.iter().all(|w| w.half_w.max(w.half_d) <= widest + 1e-3));
        }
    }

    /// A reflected box has to end up where its reflection is, not merely
    /// somewhere on the other side. Turning the place and forgetting the turn
    /// puts every leaning piece - every rafter, every mitred beam - back the way
    /// it was, in a building that has moved out from under it.
    #[test]
    fn a_mirrored_box_stands_where_the_mirror_puts_it() {
        let corners: Vec<Vec3> = (0..8)
            .map(|i| {
                Vec3::new(
                    if i & 1 == 0 { -0.5 } else { 0.5 },
                    if i & 2 == 0 { -0.5 } else { 0.5 },
                    if i & 4 == 0 { -0.5 } else { 0.5 },
                )
            })
            .collect();
        // A leaning, turned, off-centre piece: the case a sign flip gets wrong.
        let at = Vec3::new(1.7, 2.3, -0.9);
        let size = Vec3::new(3.0, 0.25, 0.8);
        let turn = Quat::from_rotation_y(0.7) * Quat::from_rotation_x(-0.45);

        let mut stood: Vec<Vec3> = corners
            .iter()
            .map(|c| at + turn * (*c * size))
            .map(|w| Vec3::new(w.x, w.y, -w.z))
            .collect();
        let mirror_at = reflect_at(at, true);
        let mirror_turn = reflect_turn(turn, true);
        let mut mirrored: Vec<Vec3> = corners
            .iter()
            .map(|c| mirror_at + mirror_turn * (*c * size))
            .collect();
        let order = |a: &Vec3, b: &Vec3| {
            a.x.total_cmp(&b.x)
                .then(a.y.total_cmp(&b.y))
                .then(a.z.total_cmp(&b.z))
        };
        stood.sort_by(order);
        mirrored.sort_by(order);
        for (want, got) in stood.iter().zip(&mirrored) {
            assert!(
                want.distance(*got) < 1e-4,
                "the mirrored box stands at {got} where its reflection is {want}"
            );
        }
    }

    /// A mirrored building still fronts the same way. The village turns every
    /// building so its +X looks at the square, so a reflection that moved the
    /// door round the back would leave every mirrored house facing away - which
    /// is what the first one did, and what Brett saw within a minute of it.
    #[test]
    fn a_mirrored_building_keeps_its_front_door_in_front() {
        for work in drawings(BuildingKind::House)
            .into_iter()
            .chain(drawings(BuildingKind::Longhouse))
        {
            let plain = doorways(work, false);
            let mirrored = doorways(work, true);
            assert_eq!(plain.len(), mirrored.len(), "{}", work.name);
            for (door, seen) in plain.iter().zip(&mirrored) {
                assert!(
                    (door.out.x - seen.out.x).abs() < 1e-4,
                    "{}: a door facing {} faces {} in the mirror - it has turned \
                     round rather than moved along its wall",
                    work.name,
                    door.out,
                    seen.out
                );
                // It may slide along the wall; it may not leave it.
                assert!(
                    (door.at.x - seen.at.x).abs() < 1e-4,
                    "{}: a door in the wall at x={} came out at x={}",
                    work.name,
                    door.at.x,
                    seen.at.x
                );
            }
        }
    }

    /// A maker's stated kind outranks whatever their file name begins with. The
    /// whole point of the bench asking is that `millhouse-tavern` can be a
    /// tavern if that is what it was drawn as.
    #[test]
    fn a_stated_kind_beats_the_name() {
        let named = |name: &str, kind: &str| Baked {
            format: super::NEWEST_FORMAT,
            name: name.to_string(),
            kind: kind.to_string(),
            half_w: 1.0,
            half_d: 1.0,
            high: 1.0,
            boxes: Vec::new(),
            marks: Vec::new(),
        };
        assert_eq!(
            claimed_by(&named("millhouse-of-mine", "tavern")),
            Some(BuildingKind::Tavern),
            "the maker said tavern"
        );
        // And with nothing said, the longest word that begins the name wins -
        // which is what every drawing baked before the card existed relies on.
        assert_eq!(
            claimed_by(&named("longhouse1-10people", "")),
            Some(BuildingKind::Longhouse)
        );
        assert_eq!(
            claimed_by(&named("sawmill2", "")),
            Some(BuildingKind::Sawmill),
            "sawmill is not a mill"
        );
        assert_eq!(claimed_by(&named("cathedral", "")), None);
    }
    #[test]
    fn a_longhouse_is_never_dealt_out_as_a_house() {
        // "longhouse1" begins with neither more nor less than its own
        // word. A prefix test that forgot this handed the hall out as a
        // family home, and every roof in the village became a hall.
        for house in drawings(BuildingKind::House) {
            assert!(
                !house.name.starts_with("longhouse"),
                "{} was dealt out as a house",
                house.name
            );
        }
        for hall in drawings(BuildingKind::Longhouse) {
            assert!(
                hall.name.starts_with("longhouse"),
                "{} is no hall",
                hall.name
            );
        }
    }

    #[test]
    fn a_carried_in_roof_sleeps_the_beds_its_maker_drew() {
        // The planner asks the KIND how many it shelters, and the answer
        // has to be the drawing's own count or it breaks ground for
        // roofs nobody needs.
        for kind in [BuildingKind::House, BuildingKind::Longhouse] {
            // Only where somebody has drawn one - see above. With no drawing
            // the planner keeps its own promise and there is nothing to hold
            // it against.
            let Some(drawn) = beds(kind) else {
                continue;
            };
            assert_eq!(kind.sleeps(), drawn);
            for work in drawings(kind) {
                let mine = work.marks.iter().filter(|m| m.mark == "sleep").count();
                assert!(
                    mine >= drawn,
                    "{} sleeps {mine}, under the {drawn} the planner promises",
                    work.name
                );
            }
        }
        assert!(
            BuildingKind::Longhouse.sleeps() > BuildingKind::House.sleeps(),
            "a hall must shelter more than a family home"
        );
    }

    /// Writes the two vocabulary files out for Opificium.
    ///
    /// The same words the game furnishes a player's bench with at every
    /// launch - this only puts them where THIS repository's project keeps
    /// them, so the contract is reviewable in a diff.
    ///
    /// `cargo test export_vocabulary_for_opificium -- --ignored`
    #[test]
    #[ignore = "a hand-run export, not a check"]
    fn export_vocabulary_for_opificium() {
        std::fs::create_dir_all("opificium/data").expect("opificium/data");
        std::fs::write("opificium/data/kinds.json", super::kinds_as_json()).expect("kinds");
        std::fs::write("opificium/data/widgets.json", super::widgets_as_json()).expect("widgets");
    }

    /// Every colour the vocabulary asks the bench to paint with must be a ramp
    /// the game actually exports, or the bench falls back to its own and the
    /// marks come out the wrong colour for reasons nobody can see.
    #[test]
    fn every_mark_paints_from_a_real_ramp() {
        for (mark, ramp, shade) in super::MARKS_UNDERSTOOD {
            assert!(
                crate::palette::ramp_named(ramp).is_some(),
                "the mark \"{mark}\" paints from \"{ramp}\", which is not a ramp the game has",
            );
            assert!(
                (0.0..=1.0).contains(shade),
                "the mark \"{mark}\" asks for shade {shade}, outside the ramp",
            );
        }
    }

    /// A drawing from a newer bench is refused, not half-read.
    ///
    /// This is the one that matters once buildings arrive from strangers.
    /// Serde drops every field it does not recognise without a word, so a
    /// format that MOVED something - marks into levels, colour into a
    /// reference - would be read as a building with no doors and no beds
    /// rather than as an error.
    #[test]
    fn a_drawing_from_a_newer_bench_is_turned_away() {
        let mut work = a_plain_work();
        work.format = super::NEWEST_FORMAT + 1;
        let why = super::turned_away(&work).expect("a newer format must be refused");
        assert!(why.contains("newer"), "{why}");

        // And the formats it does know are let in, including the drawings
        // baked before the bench stamped one at all.
        for format in [0, 1, super::NEWEST_FORMAT] {
            work.format = format;
            assert!(
                super::turned_away(&work).is_none(),
                "format {format} should be readable",
            );
        }
    }

    /// A stranger's numbers are still numbers, and some of them are absurd.
    #[test]
    fn an_absurd_drawing_is_turned_away_by_name() {
        let bad: &[(&str, fn(&mut super::Baked))] = &[
            ("no boxes", |w| w.boxes.clear()),
            ("too many boxes", |w| {
                w.boxes = (0..super::AT_MOST_BOXES + 1)
                    .map(|_| w.boxes[0].clone())
                    .collect()
            }),
            ("too wide", |w| w.half_w = super::AT_MOST_ACROSS + 1.0),
            ("no footprint", |w| w.half_d = 0.0),
            ("not a number", |w| w.high = f32::NAN),
        ];
        for (what, spoil) in bad {
            let mut work = a_plain_work();
            spoil(&mut work);
            assert!(
                super::turned_away(&work).is_some(),
                "a drawing that is {what} should be refused",
            );
        }
        // And an ordinary one is not.
        assert!(super::turned_away(&a_plain_work()).is_none());
    }

    /// What the game furnishes a maker's bench with must be what the bench
    /// can actually read - a project it cannot parse is worse than none.
    #[test]
    fn the_bench_is_furnished_with_readable_words() {
        let kinds: serde_json::Value =
            serde_json::from_str(&super::kinds_as_json()).expect("kinds.json is JSON");
        assert_eq!(kinds["format"], 1);
        assert_eq!(
            kinds["kinds"].as_array().expect("a list").len(),
            BuildingKind::every().len(),
            "every kind the game raises must be offered",
        );

        let widgets: serde_json::Value =
            serde_json::from_str(&super::widgets_as_json()).expect("widgets.json is JSON");
        assert_eq!(widgets["format"], 1);
        let offered: Vec<&str> = widgets["marks"]
            .as_array()
            .expect("a list")
            .iter()
            .map(|m| m["mark"].as_str().expect("a word"))
            .collect();
        for (mark, ..) in super::MARKS_UNDERSTOOD {
            assert!(offered.contains(mark), "{mark} is read but never offered");
        }

        let palette: serde_json::Value =
            serde_json::from_str(&crate::palette::as_json()).expect("palette.json is JSON");
        assert!(!palette["ramps"].as_array().expect("a list").is_empty());
    }

    /// A plain, believable drawing to spoil in the tests above.
    fn a_plain_work() -> super::Baked {
        super::Baked {
            format: super::NEWEST_FORMAT,
            name: "house-test".into(),
            kind: "house".into(),
            half_w: 3.0,
            half_d: 4.0,
            high: 5.0,
            boxes: vec![super::Box3 {
                at: [0.0, 1.0, 0.0],
                size: [4.0, 2.0, 0.25],
                turn: [0.0, 0.0, 0.0, 1.0],
                rgb: [110, 92, 70],
                alpha: 1.0,
                form: "box".into(),
                stage: String::new(),
            }],
            marks: vec![],
        }
    }

    /// A standing building keeps its drawing when a new one is carried in.
    ///
    /// The maker's actual afternoon: play, bake a second house into the
    /// folder, restart, load the save. By index alone every standing house
    /// would re-point - `plan % 1` and `plan % 2` are not the same house -
    /// and each would come back as a different building on ground terraced
    /// for the one before it.
    #[test]
    fn a_new_drawing_does_not_rebuild_the_village() {
        let all = super::drawings(BuildingKind::House);
        if all.len() < 2 {
            // With one drawing carried in there is nothing to re-point TO,
            // so the property is trivially held and the test says so rather
            // than pretending to have checked it.
            assert!(
                super::drawing_of(BuildingKind::House, 7, "house1-1couple-2kids").is_some()
                    || all.is_empty(),
                "a named drawing that exists must be found by name",
            );
            return;
        }
        // Every drawing must be findable by its own name, from any index -
        // which is what makes the stored name authoritative over the roll.
        for (roll, work) in all.iter().enumerate() {
            let found = super::drawing_of(BuildingKind::House, roll + 1, &work.name)
                .expect("a named drawing is found");
            assert_eq!(found.name, work.name, "the name must win over the index");
        }
    }

    /// Saves written before there was a name still raise their buildings, and
    /// a name that has since been deleted does not leave a hole.
    #[test]
    fn a_nameless_or_missing_drawing_still_finds_one() {
        if super::drawings(BuildingKind::House).is_empty() {
            return;
        }
        assert!(
            super::drawing_of(BuildingKind::House, 3, "").is_some(),
            "an old save carries no name and must still build",
        );
        assert!(
            super::drawing_of(BuildingKind::House, 3, "a-house-nobody-drew").is_some(),
            "a deleted drawing must fall back, not vanish",
        );
    }

    /// A format 2 drawing, `levels` and all, is read as the building it is.
    ///
    /// The Opificium thread's word is that `levels` rides ALONGSIDE the
    /// top-level `boxes` and `marks`, which still hold the base building
    /// finished exactly as before - so this game reads what it always read
    /// and ignores the upgrades until it has a use for them. Worth pinning
    /// rather than believing: `Baked` does not deny unknown fields today,
    /// and the day somebody adds `#[serde(deny_unknown_fields)]` for
    /// tidiness, every format 2 building in the world stops loading.
    #[test]
    fn a_format_two_drawing_reads_as_its_base_building() {
        let said = r#"{
            "format": 2,
            "name": "house9-test", "kind": "house",
            "half_w": 3.6, "half_d": 4.2, "high": 7.6,
            "boxes": [ { "at": [0,1.25,0], "size": [4,2.5,0.25], "turn": [0,0,0,1],
                         "form": "box", "rgb": [110,92,70], "alpha": 1.0,
                         "stage": "walls" } ],
            "marks": [ { "mark": "door", "at": [3.6,0.4,0.0], "yaw": 0.0 } ],
            "levels": [ { "name": "", "half_w": 3.6, "half_d": 4.2, "high": 7.6,
                          "phases": [ { "boxes": [] } ], "marks": [] } ]
        }"#;
        let work: super::Baked = serde_json::from_str(said).expect("format 2 must parse");
        assert_eq!(work.format, 2);
        assert_eq!(work.boxes.len(), 1, "the base building is still the top level");
        assert_eq!(work.marks.len(), 1);
        assert!(
            super::turned_away(&work).is_none(),
            "a current-format building must be let in",
        );
        assert_eq!(
            super::claimed_by(&work),
            Some(BuildingKind::House),
            "and raised as what it says it is",
        );
    }
}
