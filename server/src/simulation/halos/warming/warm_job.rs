//! A superchunk's warming as its job does it, off the tick
//! ([`warm_superchunk`]): built whole from where it comes from -- its
//! image and its entities' state, or the world's generation -- as the
//! arena and the store keep a superchunk, to be made hot as it is.

use super::generating_superchunk::GeneratingSuperchunk;
use crate::bitplane_manager::Superchunk;
use crate::chunk_storage::{read_back, ImageStored, LayerCodec, LayerType, SuperchunkImage};
use crate::entity_manager::{saved, DefaultAttributes, SuperchunkEntities};
use std::sync::Arc;
use utilities::coordinates::SuperchunkIndex;

/// What generates a superchunk never made: the world's generation,
/// writing it as the arena and the store keep one.
pub type GenerateSuperchunk = Box<dyn FnOnce(&mut GeneratingSuperchunk) + Send>;

/// Where a superchunk warming comes from.
pub enum WarmSource {
    /// Gone cold a moment ago, its cells still in the arena as they went
    /// cold: only its entities' state to decode, if it has one.
    Cooled {
        /// Its entities' state, as a cold superchunk keeps it.
        entity_state: Option<Vec<u64>>,
    },
    /// Made before: its image as the cold pool has it -- read here, off
    /// the tick, if it is paged to disk -- and its entities' state, if it
    /// has one.
    Stored {
        /// Its image.
        image: ImageStored,
        /// Its entities' state, as a cold superchunk keeps it.
        entity_state: Option<Vec<u64>>,
    },
    /// Never made: generated here.
    Generated(GenerateSuperchunk),
}

/// A superchunk warmed, built whole, to be made hot as it is.
pub struct WarmedSuperchunk {
    /// Its cells, as the arena keeps a superchunk's: none of a cooled
    /// one, whose cells the arena kept.
    pub cells: Option<Superchunk>,
    /// Its entities, as the store keeps a superchunk's.
    pub entities: SuperchunkEntities,
    /// Its random numbers' state, if it had ticked.
    pub random_stream_state: Option<u64>,
    /// Its image, if it was paged to disk: read back, to be in memory
    /// again while it is hot.
    pub image_read_back: Option<SuperchunkImage>,
    /// Whether it was generated: no image of it is kept yet.
    pub generated: bool,
}

/// `superchunk` warmed from `source`, to turn hot at `first_hot_tick`: a
/// layer of each of `layer_types`, its entities of a world whose default
/// attributes are `default_attributes`, decoded with `layer_codec`.
pub fn warm_superchunk(superchunk: SuperchunkIndex, source: WarmSource, layer_types: &[LayerType], first_hot_tick: u64, default_attributes: Arc<DefaultAttributes>, layer_codec: &mut LayerCodec) -> WarmedSuperchunk {
    match source {
        WarmSource::Cooled { entity_state } => {
            let (entities, random_stream_state) = entities_of(superchunk, entity_state, first_hot_tick, &default_attributes);
            WarmedSuperchunk { cells: None, entities, random_stream_state, image_read_back: None, generated: false }
        }
        WarmSource::Stored { image, entity_state } => {
            let (cells, image_read_back) = match image {
                ImageStored::InMemory(kept_image) => (Superchunk::from_image(superchunk, layer_types, &kept_image, layer_codec), None),
                ImageStored::OnDisk(folder) => {
                    let read_image = read_back(&folder, superchunk);
                    (Superchunk::from_image(superchunk, layer_types, &read_image, layer_codec), Some(read_image))
                }
            };
            let (entities, random_stream_state) = entities_of(superchunk, entity_state, first_hot_tick, &default_attributes);
            WarmedSuperchunk { cells: Some(cells), entities, random_stream_state, image_read_back, generated: false }
        }
        WarmSource::Generated(generate) => {
            let mut generating = GeneratingSuperchunk::new(superchunk, layer_types, first_hot_tick, default_attributes);
            generate(&mut generating);
            let (cells, entities) = generating.into_parts();
            WarmedSuperchunk { cells: Some(cells), entities, random_stream_state: None, image_read_back: None, generated: true }
        }
    }
}

/// `superchunk`'s entities, of a world whose default attributes are
/// `default_attributes`, as `entity_state` keeps them -- none if it has
/// none -- warming to be hot at `first_hot_tick`; and its random numbers'
/// state, if it had one.
fn entities_of(superchunk: SuperchunkIndex, entity_state: Option<Vec<u64>>, first_hot_tick: u64, default_attributes: &Arc<DefaultAttributes>) -> (SuperchunkEntities, Option<u64>) {
    let mut entities = SuperchunkEntities::new(superchunk, Arc::clone(default_attributes));
    let random_stream_state = entity_state.and_then(|words| saved::decode_into(&words, first_hot_tick, default_attributes, &mut entities).expect("a cold superchunk's state, as it was kept"));
    (entities, random_stream_state)
}
