//! The base game's planes as a frame brings them (`protocol::pipe::frame`):
//! how wide a cell or a tile of each is, and a tile read off their
//! words -- the clouds, the climates, which tiles rain.

use crate::base_types::{Climate, CLIMATE, CLOUD, TREE_STAGE};

/// Bits a word holds.
const WORD_BITS: usize = u64::BITS as usize;

/// Bits a cell's stage takes in a frame's stages.
pub const STAGE_BITS: usize = TREE_STAGE.layer_type().width_bits() as usize;

/// Bits a tile's cloud takes in a frame's clouds: the cloud plane's,
/// as the type registry has it.
pub const CLOUD_BITS: usize = CLOUD.layer_type().width_bits() as usize;
/// How many places a cell's place in its superchunk is shifted by to
/// give its tile's in the cloud plane: the plane's tile size, as the
/// type registry has it.
pub const CLOUD_TILE_SHIFT: u32 = CLOUD.layer_type().tile_shift();
/// The most cloud a tile holds: the deepest shadow.
pub const DEEPEST_CLOUD: u32 = CLOUD.most_value();

/// How many places a cell's place in its chunk is shifted by to give
/// its climate's tile's, as the type registry has it.
pub const CLIMATE_TILE_SHIFT: u32 = CLIMATE.layer_type().tile_shift();
/// Bits a tile's climate takes in a frame's climates: the climate
/// plane's.
pub const CLIMATE_BITS: usize = CLIMATE.layer_type().width_bits() as usize;

/// The climate of the tile at `tile` of a superchunk -- its tiles in
/// Morton order, as a frame's climates hold them: none where no
/// climate was sent.
pub fn climate_over(climates: &[u64], tile: usize) -> Option<Climate> {
    let first_bit = tile * CLIMATE_BITS;
    climates.get(first_bit / WORD_BITS).map(|word| Climate::of_number((word >> (first_bit % WORD_BITS)) as u32 & CLIMATE.most_value()))
}

/// The cloud over the tile at `tile` of a superchunk -- its tiles in
/// Morton order, as a frame's clouds hold them: 0 for none, and where
/// no cloud was sent.
pub fn cloud_over(clouds: &[u64], tile: usize) -> u32 {
    let first_bit = tile * CLOUD_BITS;
    clouds.get(first_bit / WORD_BITS).map_or(0, |word| (word >> (first_bit % WORD_BITS)) as u32 & DEEPEST_CLOUD)
}

/// The tiles of a superchunk that are raining, a bit a tile in the
/// order of `clouds`, its tiles' clouds: those whose cloud is above
/// what their climate -- `climates`, the climate plane's words for
/// the same tiles -- lets the air hold (`Climate::rains_above`).
/// Empty if none is.
pub fn raining_tiles_of(clouds: &[u64], climates: &[u64]) -> Vec<u64> {
    let mut raining_bits = vec![0u64; (clouds.len() * WORD_BITS / CLOUD_BITS).div_ceil(WORD_BITS)];
    for tile in (0..clouds.len() * WORD_BITS / CLOUD_BITS).filter(|&tile| cloud_over(clouds, tile) != 0) {
        if climate_over(climates, tile).unwrap_or(Climate::of_number(0)).rains(cloud_over(clouds, tile)) {
            raining_bits[tile / WORD_BITS] |= 1 << (tile % WORD_BITS);
        }
    }
    if raining_bits.iter().any(|&word| word != 0) { raining_bits } else { Vec::new() }
}
