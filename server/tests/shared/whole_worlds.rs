//! Whole worlds as the server's tests make, save and read them: of the
//! test game ([`crate::test_game`]), in folders of the crate's
//! `transient_data/`.

// A tier uses what it needs of it.
#![allow(dead_code)]

use crate::chunk_storage::SuperchunkImage;
use crate::entity_manager::InstructionsApplied;
use protocol::server_types::entity::EntityBaseData;
use protocol::server_types::attributes::AttributeBlock;
use crate::test_game::TestGame;
use crate::{DiskError, World, WorldNotMade};
use protocol::server_parameters::{CleanServerParameters, ParameterValue};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use utilities::coordinates::SuperchunkIndex;

/// A folder of its own for the test `test_name`, emptied: in the
/// crate's `transient_data/test_saves/`.
pub fn folder(test_name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("test_saves").join(test_name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// A parameter given with nothing of its own -- forced hot, the camera
/// loading -- as a flag `[]` gives one.
pub fn given() -> ParameterValue {
    ParameterValue::Nested(CleanServerParameters::default())
}

/// The world of the test game saved in `folder`, run again -- its types
/// frozen first as its types file says -- or what of the save is not
/// read.
pub fn opened(folder: &Path) -> Result<World, DiskError> {
    let frozen_type_registry = Arc::new(crate::read_types(folder, TestGame::register_types)?);
    crate::run_world(folder, &frozen_type_registry, &CleanServerParameters::default()).map_err(|why_not| match why_not {
        WorldNotMade::Disk(disk_error) => disk_error,
        WorldNotMade::Refused(why_refused) => panic!("refused: {why_refused}"),
    })
}

/// Every hot cell, every entity with its attributes, the tick, every
/// random stream, every cold superchunk's image and kept state, and the
/// superchunks warming and cooling with when each is due: what two
/// worlds the same hold the same.
pub type WorldContents = (Vec<u64>, Vec<(EntityBaseData, Vec<AttributeBlock>)>, u64, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, SuperchunkImage, Vec<u64>)>, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, u64)>);

/// [`WorldContents`] `world` holds.
pub fn world_contents(world: &World) -> WorldContents {
    let cells = world.world_info().layers.clone().into_iter().flat_map(|layer| world.bitmap_arena().buckets_of(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
    let entities_with_attributes = world.entity_store().iter().map(|entity| (entity.base_data, entity.attributes.to_vec())).collect();
    let cold = world.cold().iter().map(|(&superchunk, words)| (superchunk, SuperchunkImage::clone(&world.chunk_storage().shared_image(superchunk).expect("a cold superchunk's image")), words.clone())).collect();
    (cells, entities_with_attributes, world.entity_store().current_tick(), world.world_simulation().random_states().collect(), cold, world.warming().collect(), world.cooling().collect())
}

/// Puts an entity on `world` between ticks, whole, its cell held in the
/// test game's collision plane: there before the next tick.
pub fn add_entity(world: &mut World, test_game: TestGame, base_data: EntityBaseData, attributes: &[AttributeBlock]) -> InstructionsApplied {
    use crate::bitplane_manager::{Write, WriteOp};
    let applied = world.add_entity(base_data, attributes);
    if applied.added > 0 {
        world.write_cells(test_game.collision, [Write::cell(base_data.stood_cell, WriteOp::SetCell)]);
    }
    applied
}

/// Takes `base_data`'s entity off `world` between ticks, the cell it held
/// in the test game's collision plane let go.
pub fn remove_entity(world: &mut World, test_game: TestGame, base_data: &EntityBaseData) -> InstructionsApplied {
    use crate::bitplane_manager::{Write, WriteOp};
    let applied = world.remove_entity(base_data);
    if applied.entities_removed > 0 {
        world.write_cells(test_game.collision, [Write::cell(base_data.stood_cell, WriteOp::ClearCell)]);
    }
    applied
}
