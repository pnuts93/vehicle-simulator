//! Heightmap to render mesh and physics collider. Both are built centred on the grid, with
//! `datum` subtracted from every elevation, so they line up exactly under one entity transform.

use avian3d::{
    parry::{shape::SharedShape, utils::Array2},
    prelude::Collider,
};
use bevy::{
    asset::RenderAssetUsages,
    math::Vec3,
    mesh::{Indices, Mesh, PrimitiveTopology},
};

use super::heightmap::Heightmap;

pub fn build_mesh(map: &Heightmap, datum: f32) -> Mesh {
    let (cols, rows) = (map.cols(), map.rows());
    let mut positions = Vec::with_capacity(cols * rows);
    let mut normals = Vec::with_capacity(cols * rows);
    let mut uvs = Vec::with_capacity(cols * rows);
    for row in 0..rows {
        for col in 0..cols {
            let xz = map.local_xz(col, row);
            positions.push([xz.x, map.at(col, row) - datum, xz.y]);
            normals.push(map.normal(col, row));
            uvs.push([
                col as f32 / (cols - 1) as f32,
                row as f32 / (rows - 1) as f32,
            ]);
        }
    }

    // Two triangles per cell, wound counter-clockwise seen from above.
    let mut indices = Vec::with_capacity((cols - 1) * (rows - 1) * 6);
    for row in 0..rows - 1 {
        for col in 0..cols - 1 {
            let nw = (row * cols + col) as u32;
            let ne = nw + 1;
            let sw = nw + cols as u32;
            let se = sw + 1;
            indices.extend([nw, sw, ne, ne, sw, se]);
        }
    }

    // The collider is built from the heightmap, so the mesh only needs to live on the GPU.
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// A heightfield collider: far cheaper than a trimesh of the same surface, and stable for wheels
/// rolling over it.
pub fn build_collider(map: &Heightmap, datum: f32) -> Collider {
    // Parry wants a column-major array indexed (z, x), i.e. all rows of column 0, then column 1,
    // and so on. Avian's `Collider::heightfield` takes nested vectors and builds an array with
    // the wrong dimensions from them, which only comes out right for square grids.
    let elevations = (0..map.cols())
        .flat_map(|col| (0..map.rows()).map(move |row| (col, row)))
        .map(|(col, row)| map.at(col, row) - datum)
        .collect();
    let heights = Array2::new(map.rows(), map.cols(), elevations);
    let extent = map.extent();
    Collider::from(SharedShape::heightfield(
        heights,
        Vec3::new(extent.x, 1., extent.y),
    ))
}

#[cfg(test)]
mod tests {
    use bevy::math::{Quat, Vec2};

    use super::*;

    /// 5 columns x 3 rows, 10 m apart; elevation is 1 m per column plus 10 m per row. The grid is
    /// deliberately not square: square grids hide mixed-up axes.
    fn tilted() -> Heightmap {
        let elevations = (0..3)
            .flat_map(|row| (0..5).map(move |col| col as f32 + 10. * row as f32))
            .collect();
        Heightmap::new(5, 3, Vec2::splat(10.), elevations).unwrap()
    }

    #[test]
    fn mesh_has_a_triangle_pair_per_cell() {
        let mesh = build_mesh(&tilted(), 0.);
        assert_eq!(mesh.count_vertices(), 15);
        assert_eq!(mesh.indices().unwrap().len(), 8 * 6);
    }

    /// Drops a ray from above onto the collider: it must land where the heightmap says the
    /// ground is, which pins down the axis orientation of the transpose above.
    #[test]
    fn collider_matches_heightmap_orientation() {
        let map = tilted();
        let collider = build_collider(&map, 5.);
        for xz in [
            Vec2::new(-10., -10.),
            Vec2::new(10., -10.),
            Vec2::new(-10., 10.),
        ] {
            let hit = collider
                .cast_ray(
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::new(xz.x, 100., xz.y),
                    Vec3::NEG_Y,
                    200.,
                    true,
                )
                .expect("ray should hit the terrain")
                .0;
            let ground = 100. - hit;
            assert!(
                (ground - (map.elevation_at(xz) - 5.)).abs() < 1e-3,
                "at {xz}: collider {ground}, heightmap {}",
                map.elevation_at(xz) - 5.
            );
        }
    }
}
