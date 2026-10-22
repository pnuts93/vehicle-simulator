//! Checks the shipped terrain asset decodes, and that a slot turns into a solid tile once the
//! asset server has loaded it.

use avian3d::prelude::*;
use bevy::{prelude::*, state::app::StatesPlugin};
use simulator::terrain::{
    TerrainBuilt, TerrainPlugin, TerrainSlot, TerrainTile, TerrainTileReady, geometry, geotiff,
};

const ASSET: &str = "terrain/CA_center.tif";

#[test]
fn shipped_tile_decodes() {
    let bytes = std::fs::read(format!("{}/assets/{ASSET}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let tile = geotiff::decode(&bytes).unwrap();
    let map = &tile.heightmap;
    assert_eq!((map.cols(), map.rows()), (627, 591));
    assert_eq!(map.cell_size(), Vec2::splat(10.));
    assert_eq!(tile.geo.epsg, Some(32632));
    let (low, high) = map.range();
    assert!(
        low > -9999. && low < high,
        "nodata must not leak into the surface: {low}..{high}"
    );
}

/// The collider must follow the heightmap everywhere, not just on tiny square grids.
#[test]
fn shipped_tile_collider_matches_heightmap() {
    let bytes = std::fs::read(format!("{}/assets/{ASSET}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let map = geotiff::decode(&bytes).unwrap().heightmap;
    let datum = map.center_elevation();
    let collider = geometry::build_collider(&map, datum);
    for col in (1..map.cols() - 1).step_by(37) {
        for row in (1..map.rows() - 1).step_by(31) {
            // Off the vertices, where rays can slip between triangles.
            let xz = map.local_xz(col, row) + Vec2::new(0.37, 0.61);
            let hit = collider
                .cast_ray(
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::new(xz.x, 500., xz.y),
                    Vec3::NEG_Y,
                    1000.,
                    true,
                )
                .unwrap_or_else(|| panic!("hole in the collider at col {col}, row {row}"));
            let want = map.elevation_at(xz) - datum;
            assert!((500. - hit.0 - want).abs() < 3., "col {col}, row {row}");
        }
    }
}

#[test]
fn slot_becomes_solid_terrain() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        TransformPlugin,
        AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        TerrainPlugin,
    ));
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load::<TerrainTile>(ASSET);
    let slot = app.world_mut().spawn(TerrainSlot(handle)).id();

    for _ in 0..600 {
        app.update();
        if app.world().entity(slot).contains::<TerrainBuilt>() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let world = app.world();
    assert!(
        world.entity(slot).contains::<TerrainBuilt>(),
        "tile never finished loading"
    );
    assert!(world.entity(slot).contains::<Collider>());
    assert!(world.entity(slot).contains::<Mesh3d>());
    assert_eq!(world.resource::<Messages<TerrainTileReady>>().len(), 1);
}
