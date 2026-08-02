//! Buildings carried in from the Atelier.
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
    /// "box", "wedge", "ridge" or "log" - the bench's own shapes.
    #[serde(default)]
    pub form: String,
    /// The cloth this piece was painted in, named: "wood:0.7". The
    /// house's own wall and roof cloths are re-dyed per building, so a
    /// street of one blueprint is still a street of different houses.
    #[serde(default)]
    pub cloth: String,
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
    pub name: String,
    pub half_w: f32,
    pub half_d: f32,
    #[allow(dead_code)]
    pub high: f32,
    pub boxes: Vec<Box3>,
    pub marks: Vec<Mark>,
    /// The cloths that cover the most of this building at each stage -
    /// its walls and its roof. Worked out when the file is read, by
    /// VOLUME rather than by count: a house wears more window frames
    /// than walls, and the frames are not what a village re-dyes.
    #[serde(skip)]
    pub wall_cloth: String,
    #[serde(skip)]
    pub roof_cloth: String,
}

/// The cloth covering the most of a stage, by volume.
fn dominant(boxes: &[Box3], stage: &str) -> String {
    let mut bulk: std::collections::HashMap<&str, f32> = std::collections::HashMap::new();
    for piece in boxes.iter().filter(|b| b.stage == stage) {
        let volume = piece.size[0] * piece.size[1] * piece.size[2];
        *bulk.entry(piece.cloth.as_str()).or_default() += volume;
    }
    bulk.into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(cloth, _)| cloth.to_string())
        .unwrap_or_default()
}

/// The buildings carried in, read once and held for the life of the
/// run. A plain lookup rather than a resource: the raising happens deep
/// inside helper functions that would otherwise all have to be handed
/// the thing.
static CARRIED: std::sync::OnceLock<Vec<Baked>> = std::sync::OnceLock::new();

fn carried() -> &'static Vec<Baked> {
    CARRIED.get_or_init(|| {
        let mut works: Vec<Baked> = Vec::new();
        let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
        for dir in [
            std::path::PathBuf::from(&root).join("assets/buildings"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/buildings"),
        ] {
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
                    Some(mut work) => {
                        work.wall_cloth = dominant(&work.boxes, "walls");
                        work.roof_cloth = dominant(&work.boxes, "roof");
                        info!(
                            "carried in {}: {} boxes, {} marks",
                            work.name,
                            work.boxes.len(),
                            work.marks.len()
                        );
                        works.push(work);
                    }
                    None => warn!("could not read the baked building at {}", path.display()),
                }
            }
            if !works.is_empty() {
                // read_dir's order is the filesystem's business; a world
                // seed's is ours.
                works.sort_by(|a, b| a.name.cmp(&b.name));
                break;
            }
        }
        works
    })
}

/// What a kind's drawings are named. A kind absent from this list has no
/// bench drawings and is raised by the village's own hand, the way
/// everything was before there was an Atelier.
fn called(kind: super::BuildingKind) -> Option<&'static str> {
    match kind {
        super::BuildingKind::House => Some("house"),
        super::BuildingKind::Longhouse => Some("longhouse"),
        _ => None,
    }
}

/// Every drawing carried in for this kind, in a settled order - so a
/// world seed raises the same street twice.
pub fn drawings(kind: super::BuildingKind) -> Vec<&'static Baked> {
    let Some(called) = called(kind) else {
        return Vec::new();
    };
    carried()
        .iter()
        // "longhouse1" is not a house: a kind takes only the names that
        // begin with its OWN word, and the longer word is checked by
        // being its own prefix, not by being ruled out of the shorter.
        .filter(|work| work.name.starts_with(called))
        .filter(|work| called != "house" || !work.name.starts_with("longhouse"))
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

/// A gable's prism, and the ridge cap's, cut the way the bench cuts
/// them: unit-sized, so a box's scale shapes them.
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
    // The cloths this particular house wears, rolled with its plan.
    wall_dye: Color,
    roof_dye: Color,
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
        // The house's own walls and roof take this building's cloth;
        // every frame, sill, floorboard and stick of furniture keeps
        // exactly what the maker painted it.
        let colour = if !piece.cloth.is_empty() && piece.cloth == work.wall_cloth {
            wall_dye
        } else if !piece.cloth.is_empty() && piece.cloth == work.roof_cloth {
            roof_dye
        } else {
            Color::srgb_u8(piece.rgb[0], piece.rgb[1], piece.rgb[2])
        };
        let clear = piece.alpha < 0.999;
        let mesh = match piece.form.as_str() {
            "wedge" => wedge.clone(),
            "ridge" => ridge.clone(),
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
            Transform::from_translation(Vec3::from(piece.at))
                .with_rotation(Quat::from_array(piece.turn))
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
pub fn furnish_baked(commands: &mut Commands, site: Entity, work: &Baked) {
    let mut slot = 0u8;
    let sleeps: Vec<&Mark> = work.marks.iter().filter(|m| m.mark == "sleep").collect();
    for (index, mark) in sleeps.iter().enumerate() {
        // Two sleeping places lying alongside each other are the two
        // halves of one marriage bed - the pair sleeps there and the
        // children do not, whoever set them down.
        let at = Vec3::from(mark.at);
        let double = sleeps
            .iter()
            .enumerate()
            .any(|(other, twin)| other != index && Vec3::from(twin.at).distance(at) < 1.4);
        // A mark faces its own +X, the way every mark does: for a
        // sleeper that is the way their head lies. Tipped onto its back a
        // body's head points along -Z, so the turn that carries it to the
        // pillow is read straight off that direction.
        let head_way = Quat::from_rotation_y(mark.yaw) * Vec3::X;
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
            Transform::from_translation(Vec3::from(mark.at)),
            Visibility::Hidden,
            ChildOf(site),
        ));
    }
    // Every door the maker marked, where they marked it, facing the way
    // its nose points - out of the building.
    let doors: Vec<Doorway> = work
        .marks
        .iter()
        .filter(|m| m.mark == "door")
        .map(|m| {
            let out = Quat::from_rotation_y(m.yaw) * Vec3::X;
            Doorway {
                at: Vec2::new(m.at[0], m.at[2]),
                out: Vec2::new(out.x, out.z).normalize_or(Vec2::X),
            }
        })
        .collect();
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
            assert!(
                !all.is_empty(),
                "no {kind:?} drawings found beside the game"
            );
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
            let drawn = beds(kind).expect("a carried-in roof has beds in it");
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
}
