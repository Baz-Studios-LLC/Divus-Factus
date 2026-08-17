//! Accumulates transformed boxes into a single mesh.
//!
//! Scenery used to be one entity per box: a tree was a trunk plus three canopy
//! slabs, each its own entity, and a chunk held a few hundred of them. Across a
//! streamed view that reached 186,000 entities, and the per-frame cost of
//! transform propagation and visibility culling over that many — not the cost of
//! generating them — is what pinned the frame rate at 30.
//!
//! Baking a chunk's scenery into one mesh trades individual control for roughly two
//! orders of magnitude fewer entities. Anything that needs to be picked up, eaten or
//! animated stays a real entity; anything that is only ever looked at goes in here.

use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

/// A mesh under construction.
#[derive(Default)]
pub struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

/// The eight corners of a unit cube centered on the origin.
const CORNERS: [Vec3; 8] = [
    Vec3::new(-0.5, -0.5, -0.5),
    Vec3::new(0.5, -0.5, -0.5),
    Vec3::new(0.5, 0.5, -0.5),
    Vec3::new(-0.5, 0.5, -0.5),
    Vec3::new(-0.5, -0.5, 0.5),
    Vec3::new(0.5, -0.5, 0.5),
    Vec3::new(0.5, 0.5, 0.5),
    Vec3::new(-0.5, 0.5, 0.5),
];

/// Each face as four corner indices plus its outward normal.
const FACES: [([usize; 4], Vec3); 6] = [
    ([0, 3, 2, 1], Vec3::NEG_Z),
    ([4, 5, 6, 7], Vec3::Z),
    ([0, 4, 7, 3], Vec3::NEG_X),
    ([1, 2, 6, 5], Vec3::X),
    ([3, 7, 6, 2], Vec3::Y),
    ([0, 1, 5, 4], Vec3::NEG_Y),
];

impl MeshBuilder {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Adds a box, transformed by `transform`, in a flat color.
    pub fn push_box(&mut self, transform: Transform, color: Color) {
        let matrix = transform.to_matrix();
        // Normals need the rotation but not the scale, or a non-uniform scale tilts
        // the lighting.
        let rotation = transform.rotation;
        let linear = color.to_linear();
        let rgba = [linear.red, linear.green, linear.blue, 1.0];

        for (corners, normal) in FACES {
            let base = self.positions.len() as u32;
            let world_normal = rotation * normal;

            for corner in corners {
                let p = matrix.transform_point3(CORNERS[corner]);
                self.positions.push([p.x, p.y, p.z]);
                self.normals
                    .push([world_normal.x, world_normal.y, world_normal.z]);
                self.colors.push(rgba);
            }

            self.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    pub fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::MAIN_WORLD
                | bevy::asset::RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// Accumulates grass blades: single triangles with a bend weight in `uv.x`
/// (0 at the roots, 1 at the tip) and a color gradient from root to tip.
///
/// Kept separate from [`MeshBuilder`] because blades are not boxes: they are the
/// one thing in the game whose vertices the GPU moves every frame, and the wind
/// shader's contract — what `uv` means — lives here.
#[derive(Default)]
pub struct BladeBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
}

impl BladeBuilder {
    /// Adds one blade standing at `base`.
    #[allow(clippy::too_many_arguments)]
    pub fn push_blade(
        &mut self,
        base: Vec3,
        height: f32,
        half_width: f32,
        yaw: f32,
        phase: f32,
        root_color: [f32; 4],
        tip_color: [f32; 4],
    ) {
        let across = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * half_width;
        // The tip leans a little off vertical so blades are not soldiers.
        let lean = Vec3::new(yaw.sin(), 0.0, -yaw.cos()) * height * 0.18;

        let root_left = base - across;
        let root_right = base + across;
        let tip = base + Vec3::Y * height + lean;

        for (position, uv, color) in [
            (root_left, [0.0, phase], root_color),
            (root_right, [0.0, phase], root_color),
            (tip, [1.0, phase], tip_color),
        ] {
            self.positions.push([position.x, position.y, position.z]);
            // Up-facing normals: blades shade like the meadow they stand in, rather
            // than each catching its own highlight and shimmering.
            self.normals.push([0.0, 1.0, 0.0]);
            self.uvs.push(uv);
            self.colors.push(color);
        }
    }

    /// The finished mesh, or `None` if no blades were added.
    pub fn build(self) -> Option<Mesh> {
        if self.positions.is_empty() {
            return None;
        }
        Some(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::MAIN_WORLD
                    | bevy::asset::RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_builder_reports_empty() {
        assert!(MeshBuilder::default().is_empty());
    }

    #[test]
    fn a_box_contributes_six_quads() {
        let mut builder = MeshBuilder::default();
        builder.push_box(Transform::default(), Color::WHITE);
        assert!(!builder.is_empty());

        let mesh = builder.build();
        assert_eq!(mesh.count_vertices(), 24, "six faces of four vertices");
        assert_eq!(mesh.indices().map(|i| i.len()), Some(36));
    }

    #[test]
    fn boxes_accumulate_without_overlapping_indices() {
        // A second box must reference its own vertices, not the first box's.
        let mut builder = MeshBuilder::default();
        builder.push_box(Transform::default(), Color::WHITE);
        builder.push_box(Transform::from_xyz(5.0, 0.0, 0.0), Color::BLACK);

        let mesh = builder.build();
        assert_eq!(mesh.count_vertices(), 48);

        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("expected u32 indices");
        };
        assert!(indices.iter().all(|i| (*i as usize) < 48));
        assert!(
            indices[36..].iter().all(|i| *i >= 24),
            "second box reindexed"
        );
    }

    #[test]
    fn transforms_place_and_scale_geometry() {
        let mut builder = MeshBuilder::default();
        builder.push_box(
            Transform::from_xyz(10.0, 2.0, -3.0).with_scale(Vec3::new(2.0, 4.0, 6.0)),
            Color::WHITE,
        );
        let mesh = builder.build();

        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };

        let xs: Vec<f32> = positions.iter().map(|p| p[0]).collect();
        let ys: Vec<f32> = positions.iter().map(|p| p[1]).collect();
        let min_x = xs.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_x = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min_y = ys.iter().cloned().fold(f32::INFINITY, f32::min);

        assert!((min_x - 9.0).abs() < 1e-4, "min x was {min_x}");
        assert!((max_x - 11.0).abs() < 1e-4, "max x was {max_x}");
        assert!((min_y - 0.0).abs() < 1e-4, "min y was {min_y}");
    }

    #[test]
    fn rotation_carries_into_normals() {
        let mut builder = MeshBuilder::default();
        builder.push_box(
            Transform::default().with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Color::WHITE,
        );
        let mesh = builder.build();

        let Some(bevy::mesh::VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("no normals");
        };

        // Every normal must stay unit length after rotation.
        for n in normals {
            let length = Vec3::from_array(*n).length();
            assert!((length - 1.0).abs() < 1e-4, "normal length {length}");
        }

        // The former +Y face should now point along +Z.
        assert!(
            normals
                .iter()
                .any(|n| Vec3::from_array(*n).dot(Vec3::Z) > 0.99),
            "rotation did not reach the normals",
        );
    }
}
