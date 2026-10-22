//! Terrain from elevation rasters.
//!
//! The pieces are layered so each can change independently:
//!
//! - [`heightmap`]: the in-memory elevation grid. Pure data and maths.
//! - [`geotiff`]: file format to heightmap. Knows nothing about the engine.
//! - [`asset`]: plugs the decoder into Bevy's asset server (`.tif` loads as a [`TerrainTile`]).
//! - [`geometry`]: heightmap to render mesh and physics collider.
//! - this module: a [`TerrainSlot`] entity says *which* tile goes *where*; [`TerrainPlugin`]
//!   turns it into a solid, visible entity once the tile has loaded.
//!
//! Deciding which tiles are needed is deliberately not part of this module. Today the app spawns
//! one slot by hand; a future provider that streams tiles around the player only has to spawn and
//! despawn slots, positioning them with [`TerrainTile::geo`] and a shared [`ElevationDatum`].

pub mod asset;
pub mod geometry;
pub mod geotiff;
pub mod heightmap;

use avian3d::prelude::RigidBody;
use bevy::prelude::*;

pub use asset::{TerrainTile, TerrainTileLoader};
pub use heightmap::Heightmap;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<TerrainTile>()
            .init_asset_loader::<TerrainTileLoader>()
            .add_message::<TerrainTileReady>()
            .add_systems(Update, build_loaded_slots);
    }
}

/// Request for a tile at this entity's transform. The tile is centred on the entity, and its
/// elevations are placed according to the [`ElevationDatum`].
///
/// Add a `MeshMaterial3d` and a [`Surface`](crate::surface::Surface) to choose how the terrain
/// looks and grips; they are left untouched.
#[derive(Component)]
#[require(Transform, ElevationDatum)]
pub struct TerrainSlot(pub Handle<TerrainTile>);

/// Which elevation sits at the slot's own height.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub enum ElevationDatum {
    /// The tile's centre is at the slot's height. Convenient for a single tile.
    #[default]
    TileCenter,
    /// This absolute elevation (metres) is at the slot's height. Adjacent tiles share a fixed
    /// datum so their edges meet.
    Fixed(f32),
}

impl ElevationDatum {
    fn resolve(self, map: &Heightmap) -> f32 {
        match self {
            Self::TileCenter => map.center_elevation(),
            Self::Fixed(elevation) => elevation,
        }
    }
}

/// Marks a slot whose mesh and collider have been built.
#[derive(Component)]
pub struct TerrainBuilt;

/// Sent when a slot's terrain has become solid.
#[derive(Message)]
pub struct TerrainTileReady(pub Entity);

fn build_loaded_slots(
    mut commands: Commands,
    tiles: Res<Assets<TerrainTile>>,
    mut meshes: ResMut<Assets<Mesh>>,
    slots: Query<(Entity, &TerrainSlot, &ElevationDatum), Without<TerrainBuilt>>,
    mut ready: MessageWriter<TerrainTileReady>,
) {
    for (entity, slot, datum) in &slots {
        // Not loaded yet (or failed, which the asset server already logs).
        let Some(tile) = tiles.get(&slot.0) else {
            continue;
        };
        let map = &tile.heightmap;
        let datum = datum.resolve(map);
        commands.entity(entity).insert((
            Mesh3d(meshes.add(geometry::build_mesh(map, datum))),
            geometry::build_collider(map, datum),
            RigidBody::Static,
            TerrainBuilt,
        ));
        ready.write(TerrainTileReady(entity));
    }
}
