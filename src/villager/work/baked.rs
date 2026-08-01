//! Buildings carried in from the Atelier.
//!
//! The bench and the game share no code: the bench resolves its own
//! catalogue and palette and hands over plain boxes with colours, plus
//! the marks that say what a place is FOR. This module reads those
//! files, raises the boxes stage by stage, and turns the marks into the
//! components the village already knows - beds, a shell with doors, the
//! family table.

use bevy::prelude::*;

use super::buildings::{Bed, Doorway, RoofPart, Shell, Table};

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
                break;
            }
        }
        works
    })
}

/// The house the village raises, if one has been carried in.
pub fn house() -> Option<&'static Baked> {
    carried().iter().find(|work| work.name.starts_with("house"))
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
fn stage_of(stage: &str) -> u8 {
    match stage {
        "footing" | "frame" => 0,
        "walls" => 1,
        _ => 2,
    }
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
    for piece in work.boxes.iter().filter(|b| stage_of(&b.stage) == stage) {
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
        // The roof lifts for the cutaway, the way it always has.
        if piece.stage == "roof" {
            raised.insert(RoofPart);
        }
    }
}

/// Turns the marks into what the village reads: beds it can claim, a
/// shell with doors to walk through, the family table.
pub fn furnish_baked(commands: &mut Commands, site: Entity, work: &Baked) {
    let mut slot = 0u8;
    let sleeps: Vec<&Mark> = work.marks.iter().filter(|m| m.mark == "sleep").collect();
    for (index, mark) in sleeps.iter().enumerate() {
        // Two sleeping places within arm's reach of each other are one
        // bed made for two - the marriage bed, whoever placed it.
        let at = Vec3::from(mark.at);
        let double = sleeps
            .iter()
            .enumerate()
            .any(|(other, twin)| other != index && Vec3::from(twin.at).distance(at) < 0.8);
        // A sleeper's head lies toward the bed's own +Z.
        let head_way = Quat::from_rotation_y(mark.yaw) * Vec3::Z;
        let along_x = head_way.x.abs() > head_way.z.abs();
        let head = if along_x {
            head_way.x.signum()
        } else {
            head_way.z.signum()
        };
        commands.spawn((
            Bed {
                slot,
                along_x,
                head,
                double,
            },
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
    commands.entity(site).insert(Shell {
        half_w: work.half_w,
        half_d: work.half_d,
        doors: if doors.is_empty() {
            vec![Doorway::on_x_wall(work.half_w, 0.0)]
        } else {
            doors
        },
    });
}
