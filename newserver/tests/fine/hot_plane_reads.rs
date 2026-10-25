//! Planes read across the hot superchunks: every shape of read -- a cell,
//! a window, small tiles, an aligned tile, whether a tile holds anything
//! -- is what the cells it covers hold, across chunk and superchunk
//! edges, and a superchunk not hot reads as not hot.

use crate::hot_superchunks::hot_store::HotStore;
use crate::hot_superchunks::hot_superchunk::HotSuperchunk;
use crate::hot_superchunks::hot_planes::hot_plane_reads::{HotPlaneReader, SMALL_TILE_CELLS, SMALL_TILE_SIDE};
use crate::hot_superchunks::hot_planes::NotHot;
use crate::registered_planes::TestPlanes;
use protocol::type_registry::PlaneType;
use utilities::coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use utilities::rng::RandomSource;

/// Cells either way of the corner the four hot superchunks meet at, where
/// the cells are set and the reads are drawn.
const NEAR_CORNER: u32 = 300;

/// The cell of the four hot superchunks' meeting corner, and a store of
/// those four -- the one to their right not hot -- with cells drawn set
/// on two planes about the corner.
fn store_about_a_corner(planes: &TestPlanes, seeded_random: &mut RandomSource) -> (CellCartesian, HotStore) {
    let (middle_x, middle_y) = WORLD_MIDDLE.cartesian();
    let mut hot_store = HotStore::new(planes.every().to_vec());
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        hot_store.adopt(HotSuperchunk::new(utilities::coordinates::SuperchunkIndex::from_cartesian(middle_x + dx, middle_y + dy)));
    }
    let corner = CellCartesian { x: (middle_x + 1) * SUPERCHUNK_SIDE_CELLS, y: (middle_y + 1) * SUPERCHUNK_SIDE_CELLS };
    for _ in 0..40_000 {
        let cell = near_corner(corner, seeded_random);
        let plane_type = [planes.one_bit, planes.other_bit][seeded_random.draw_below(2) as usize];
        if let Some(hot_superchunk) = hot_store.superchunk_mut(CellIndex::from(cell).superchunk()) {
            hot_superchunk.set_value(plane_type, cell.into(), 1);
        }
    }
    (corner, hot_store)
}

/// A cell drawn within [`NEAR_CORNER`] of `corner`, either way.
fn near_corner(corner: CellCartesian, seeded_random: &mut RandomSource) -> CellCartesian {
    let offset = |random: &mut RandomSource| random.draw_below(2 * u64::from(NEAR_CORNER)) as u32;
    CellCartesian { x: corner.x - NEAR_CORNER + offset(seeded_random), y: corner.y - NEAR_CORNER + offset(seeded_random) }
}

/// What `reader` says of `cell` read alone: whether it is hot, and
/// whether `plane_type` holds there.
fn cell_alone(reader: &HotPlaneReader, plane_type: PlaneType, cell: CellCartesian) -> (bool, bool) {
    match reader.holds(plane_type, cell.into()) {
        Ok(holds) => (true, holds),
        Err(NotHot(_)) => (false, false),
    }
}

/// A cell read alone holds what was set there; a cell of the superchunk
/// not hot is not hot, and none of its planes is read.
#[test]
fn a_cell_reads_what_was_set_and_not_hot_where_its_superchunk_is_not() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random) = (TestPlanes::registered(seed), RandomSource::new(seed));
    let (corner, hot_store) = store_about_a_corner(&planes, &mut seeded_random);
    let reader = hot_store.reader();
    for _ in 0..2000 {
        let cell = near_corner(corner, &mut seeded_random);
        let index = CellIndex::from(cell);
        match hot_store.superchunk(index.superchunk()) {
            Some(hot_superchunk) => assert_eq!(reader.holds(planes.one_bit, index), Ok(hot_superchunk.value_at(planes.one_bit, index) == Some(1))),
            None => assert_eq!(reader.value(planes.four_bits, index), Err(NotHot(index.superchunk()))),
        }
    }
}

/// A window of any size up to 8x8, of two planes at once, is the cells
/// it covers read alone, row by row -- wherever it falls over chunk and
/// superchunk edges, its cells of the superchunk not hot not hot.
#[test]
fn windows_are_their_cells_read_alone() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random) = (TestPlanes::registered(seed), RandomSource::new(seed));
    let (corner, hot_store) = store_about_a_corner(&planes, &mut seeded_random);
    let reader = hot_store.reader();
    for _ in 0..3000 {
        let origin = near_corner(corner, &mut seeded_random);
        let (width, height) = (1 + seeded_random.draw_below(8) as u32, 1 + seeded_random.draw_below(8) as u32);
        let windows = reader.windows([planes.one_bit, planes.other_bit], origin.into(), width, height);
        for (read_window, plane_type) in windows.iter().zip([planes.one_bit, planes.other_bit]) {
            let (mut cells_set, mut hot) = (0, 0);
            for (y, x) in (0..height).flat_map(|y| (0..width).map(move |x| (y, x))) {
                let (is_hot, holds) = cell_alone(&reader, plane_type, CellCartesian { x: origin.x + x, y: origin.y + y });
                hot |= u64::from(is_hot) << (8 * y + x);
                cells_set |= u64::from(holds) << (8 * y + x);
            }
            assert_eq!((read_window.cells_set, read_window.hot), (cells_set, hot), "the window {width}x{height} at {origin:?}");
        }
    }
}

/// Four small tiles, two by two, are the cells they cover read alone,
/// in Morton order a tile at a time -- only those asked for.
#[test]
fn small_tiles_are_their_cells_read_alone() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random) = (TestPlanes::registered(seed), RandomSource::new(seed));
    let (corner, hot_store) = store_about_a_corner(&planes, &mut seeded_random);
    let reader = hot_store.reader();
    let side = SMALL_TILE_SIDE;
    for _ in 0..3000 {
        let drawn = near_corner(corner, &mut seeded_random);
        let first_cell = CellCartesian { x: drawn.x / side * side, y: drawn.y / side * side };
        let wanted_cells = seeded_random.next_draw();
        let [read_tiles] = reader.small_tiles([planes.one_bit], first_cell.into(), wanted_cells);
        let (mut cells_set, mut hot) = (0, 0);
        for bit in (0..u64::BITS).filter(|&bit| wanted_cells >> bit & 1 == 1) {
            let (tile, in_tile) = (bit / SMALL_TILE_CELLS, bit % SMALL_TILE_CELLS);
            // A small tile's cells in Morton order: x in the even bits, y in the odd.
            let (x, y) = ((in_tile & 1) | (in_tile >> 1 & 2), (in_tile >> 1 & 1) | (in_tile >> 2 & 2));
            let (is_hot, holds) = cell_alone(&reader, planes.one_bit, CellCartesian { x: first_cell.x + side * (tile & 1) + x, y: first_cell.y + side * (tile >> 1) + y });
            hot |= u64::from(is_hot) << bit;
            cells_set |= u64::from(holds) << bit;
        }
        assert_eq!((read_tiles.cells_set, read_tiles.hot), (cells_set, hot), "small tiles at {first_cell:?}");
    }
}

/// An aligned tile of any side from 8 to a chunk's is its cells read
/// alone, in Morton order; whether a tile of any scale holds anything,
/// and which coarsest tiles of a chunk do, are what its cells say.
#[test]
fn aligned_tiles_and_what_holds_anything_are_their_cells() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random) = (TestPlanes::registered(seed), RandomSource::new(seed));
    let (corner, hot_store) = store_about_a_corner(&planes, &mut seeded_random);
    let reader = hot_store.reader();
    let any_in = |cell: CellIndex, cells_in_tile: u64| (0..cells_in_tile).any(|place| cell_alone(&reader, planes.one_bit, CellIndex((cell.0 & !(cells_in_tile - 1)) + place).cartesian()).1);
    for _ in 0..300 {
        let cell = CellIndex::from(near_corner(corner, &mut seeded_random));
        let hot = hot_store.superchunk(cell.superchunk()).is_some();
        let side = 8 << seeded_random.draw_below(6);
        let cells_in_tile = u64::from(side * side);
        let expected_tile = hot.then(|| (0..cells_in_tile / 64).map(|word| (0..64).fold(0u64, |bits, bit| bits | u64::from(cell_alone(&reader, planes.one_bit, CellIndex((cell.0 & !(cells_in_tile - 1)) + word * 64 + bit).cartesian()).1) << bit)).collect::<Vec<_>>());
        assert_eq!(reader.aligned_tile(planes.one_bit, cell, side).map(<[u64]>::to_vec), expected_tile, "the aligned tile {side} a side at {cell:?}");
        let scale = seeded_random.draw_below(7) as u32;
        assert_eq!(reader.any_in_tile(planes.one_bit, cell, scale), hot.then(|| any_in(cell, 1 << (2 * scale))), "anything in the tile of scale {scale}");
        let chunk_first = CellIndex(cell.0 & !0xffff);
        let holding = (0..16).fold(0u16, |tiles, tile| tiles | u16::from(any_in(CellIndex(chunk_first.0 + tile * 4096), 4096)) << tile);
        assert_eq!(reader.tiles_holding(planes.one_bit, cell), hot.then_some(holding), "the coarsest tiles holding anything");
    }
}
