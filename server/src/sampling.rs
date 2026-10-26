//! Sampling ([`sample_the_plane`]): the tiles of a hot chunk's plane a
//! sca proc is called on this tick -- every tile not 0 rolled for at the
//! proc's chance. Wasteful and simple: every tile of the plane looked
//! at, a roll each.

use crate::hot_chunk::HotChunk;
use utilities::chance::Chance;
use utilities::coordinates::CellIndex;
use utilities::rng::RandomSource;

/// The hits of the plane of ID `plane` of `chunk` this tick added to
/// `hits`, each its tile's first cell, in Morton order: each tile not 0
/// rolled for once at `chance`, from `random_numbers`.
pub fn sample_the_plane(chunk: &HotChunk, plane: u64, chance: Chance, random_numbers: &mut RandomSource, hits: &mut Vec<CellIndex>) {
    if chunk.non_zero_tiles(plane) == 0 {
        return;
    }
    for tile in chunk.first_cells_of_non_zero_tiles(plane) {
        if random_numbers.chance(chance) {
            hits.push(tile);
        }
    }
}
