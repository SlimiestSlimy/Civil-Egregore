//! A chunk, hot ([`HotChunk`]): what a thread ticks, its planes held
//! whole -- only those with a tile that is not 0, each one dense over the
//! chunk, counting its tiles that are not 0, what sampling goes by -- and
//! its random stream, where its tick draws from.

use utilities::coordinates::{CellIndex, ChunkIndex, CELLS_IN_CHUNK};
use utilities::rng::RandomSource;

/// One chunk, hot: its planes with a tile that is not 0, by type.
pub struct HotChunk {
    /// Which chunk it is.
    chunk_index: ChunkIndex,
    /// Its random stream's whole state, as its last tick left it.
    random_state: u64,
    /// Its planes, sorted by type ID: one emptied stays until the tick's
    /// end ([`HotChunk::drop_empty_planes`]).
    planes: Vec<HotPlane>,
}

impl HotChunk {
    /// `chunk_index` of the world of seed `world_seed`, every tile of
    /// every plane 0, its random stream the chunk's own of the seed.
    pub fn new(chunk_index: ChunkIndex, world_seed: u64) -> Self {
        Self { chunk_index, random_state: RandomSource::for_stream(world_seed, chunk_index.0).whole_state(), planes: Vec::new() }
    }

    /// Its random stream, as it stands: taken for a tick, and given back
    /// ([`HotChunk::give_back_random_numbers`]) drawn from.
    pub fn take_random_numbers(&self) -> RandomSource {
        RandomSource::new(self.random_state)
    }

    /// `random_numbers`, its stream taken for a tick, kept as drawn.
    pub fn give_back_random_numbers(&mut self, random_numbers: RandomSource) {
        self.random_state = random_numbers.whole_state();
    }

    /// Which chunk it is.
    pub fn chunk_index(&self) -> ChunkIndex {
        self.chunk_index
    }

    /// The number the plane of ID `plane` holds at the tile holding
    /// `cell`, a cell of this chunk: 0 if it keeps no such plane.
    pub fn read_tile(&self, plane: u64, cell: CellIndex) -> u32 {
        debug_assert_eq!(cell.chunk(), self.chunk_index, "a cell of another chunk");
        self.plane(plane).map_or(0, |hot_plane| hot_plane.read(cell.place()))
    }

    /// Makes `value` the number the plane of ID `plane` -- numbers of
    /// `width_bits`, tiles `tile_size` cells a side, as its type says --
    /// holds at the tile holding `cell`, a cell of this chunk. `value` fits
    /// the plane's numbers.
    pub fn write_tile(&mut self, plane: u64, width_bits: u32, tile_size: u32, cell: CellIndex, value: u32) {
        debug_assert_eq!(cell.chunk(), self.chunk_index, "a cell of another chunk");
        let place = match self.planes.binary_search_by_key(&plane, |hot_plane| hot_plane.type_id) {
            Ok(place) => place,
            Err(_) if value == 0 => return,
            Err(place) => {
                self.planes.insert(place, HotPlane::new(plane, width_bits, tile_size));
                place
            }
        };
        self.planes[place].write(cell.place(), value);
    }

    /// How many tiles of the plane of ID `plane` are not 0.
    pub fn non_zero_tiles(&self, plane: u64) -> u32 {
        self.plane(plane).map_or(0, |hot_plane| hot_plane.non_zero_tiles)
    }

    /// The first cell of each tile not 0 of the plane of ID `plane`, in
    /// Morton order: every tile of it looked at.
    pub fn first_cells_of_non_zero_tiles(&self, plane: u64) -> impl Iterator<Item = CellIndex> + '_ {
        let chunk_index = self.chunk_index;
        self.plane(plane).into_iter().flat_map(move |hot_plane| {
            (0..CELLS_IN_CHUNK).step_by(hot_plane.tile_cells).filter(|&place| hot_plane.read(place) != 0).map(move |place| CellIndex::from_chunk_place(chunk_index, place))
        })
    }

    /// The planes whose every tile is 0 let go: the tick's end.
    pub fn drop_empty_planes(&mut self) {
        self.planes.retain(|hot_plane| hot_plane.non_zero_tiles > 0);
    }

    /// The plane of ID `plane`, if it keeps one.
    fn plane(&self, plane: u64) -> Option<&HotPlane> {
        self.planes.binary_search_by_key(&plane, |hot_plane| hot_plane.type_id).ok().map(|place| &self.planes[place])
    }
}

/// One plane over one chunk, dense: every tile's number, packed by
/// its width, in Morton order -- a tile's cells one after another in
/// it, so a tile is its cells' place over its cells' count.
struct HotPlane {
    /// The plane's type's ID.
    type_id: u64,
    /// Bits a number: 1, 2, 4, 8, 16 or 32.
    width_bits: u32,
    /// Cells a tile.
    tile_cells: usize,
    /// The numbers, `64 / width_bits` a word.
    words: Vec<u64>,
    /// How many tiles are not 0.
    non_zero_tiles: u32,
}

impl HotPlane {
    /// The plane of ID `type_id`, numbers of `width_bits`, tiles
    /// `tile_size` cells a side, every tile 0.
    fn new(type_id: u64, width_bits: u32, tile_size: u32) -> Self {
        let tile_cells = (tile_size * tile_size) as usize;
        let bits = CELLS_IN_CHUNK / tile_cells * width_bits as usize;
        Self { type_id, width_bits, tile_cells, words: vec![0; bits.div_ceil(u64::BITS as usize)], non_zero_tiles: 0 }
    }

    /// The word and the bit in it the number of the tile holding the cell
    /// at `place` in the chunk starts at.
    fn word_and_bit(&self, place: usize) -> (usize, u32) {
        let bit = place / self.tile_cells * self.width_bits as usize;
        (bit / u64::BITS as usize, (bit % u64::BITS as usize) as u32)
    }

    /// A number's bits, all set.
    fn mask(&self) -> u64 {
        u64::MAX >> (u64::BITS - self.width_bits)
    }

    /// The number of the tile holding the cell at `place`.
    fn read(&self, place: usize) -> u32 {
        let (word, bit) = self.word_and_bit(place);
        (self.words[word] >> bit & self.mask()) as u32
    }

    /// Makes `value` the number of the tile holding the cell at `place`,
    /// counting the tiles not 0.
    fn write(&mut self, place: usize, value: u32) {
        debug_assert!(u64::from(value) <= self.mask(), "{value} is wider than {} bits", self.width_bits);
        let (word, bit) = self.word_and_bit(place);
        let held = (self.words[word] >> bit & self.mask()) as u32;
        self.words[word] = self.words[word] & !(self.mask() << bit) | u64::from(value) << bit;
        match (held == 0, value == 0) {
            (true, false) => self.non_zero_tiles += 1,
            (false, true) => self.non_zero_tiles -= 1,
            _ => {}
        }
    }
}
