//! Plane sampling ([`sample_plane`]): the tiles of a hot chunk's plane a
//! sca proc is called on this tick -- every tile not 0 rolled for at the
//! proc's chance. Wasteful and simple: every tile of the plane looked
//! at, a roll each.

use crate::hot_chunk::HotChunk;
use utilities::chance::Chance;
use utilities::coordinates::CellIndex;
use utilities::rng::RandomSource;

/// The hits of the plane of type ID `plane_type_id` of `chunk` this tick
/// added to `hits`, each its tile's corner cell, in Morton order: each
/// tile not 0 rolled for once at `chance`, from `random_stream`.
pub fn sample_plane(chunk: &HotChunk, plane_type_id: u64, chance: Chance, random_stream: &mut RandomSource, hits: &mut Vec<CellIndex>) {
    if chunk.non_zero_tiles(plane_type_id) == 0 {
        return;
    }
    for tile_corner in chunk.corners_of_non_zero_tiles(plane_type_id) {
        if random_stream.chance(chance) {
            hits.push(tile_corner);
        }
    }
}
