//! The last pass, both directions: the floor tiles the floor plan names, in
//! Morton order -- each copied floor tile copied from its source, each
//! residual floor tile's cells range-coded at the odds of their contexts --
//! and the pricing of residual floor tiles for the complex tiling, by the same
//! floor tile coder. `docs/tessera.md`, "The last pass".
//!
//! Function by function: `docs/reference.md`, "`last_pass.rs`".

pub(crate) mod context_odds;
mod context_window;
mod floor_plan;

pub use context_odds::ContextOdds;
pub use floor_plan::FloorPlan;

use context_odds::FRACTION_BITS;
use context_window::{WINDOW_PLACES, Window};

use crate::arithmetic::{RangeDecoder, RangeEncoder, FINISHING_BITS};
use crate::bit_stream::{BitReader, BitStream};
use crate::tile::{cells_in_tile, tiles_in_level, Tile, CELLS, FLOOR_LEVEL};
use utilities::bitmap::Bitmap;
use utilities::fixed_list::FixedList;

/// Where a cell's context reads, relative to it, `(dx, dy)`: top left,
/// above and left, then the same two cells away -- each before it in
/// Morton order.
pub const CONTEXT_CELLS: [(i8, i8); 6] = [(-1, -1), (0, -1), (-1, 0), (-2, -2), (0, -2), (-2, 0)];
/// Contexts: one for every value the context cells can hold.
const CONTEXT_COUNT: usize = 1 << CONTEXT_CELLS.len();

/// The most bits the pass takes over one a residual cell
/// (`docs/tessera.md`, "The odds").
pub const MOST_EXTRA_BITS: usize = CONTEXT_COUNT * (CELLS.ilog2() as usize / 2 + 1) + (CELLS >> 10) + (CELLS >> 12) + FINISHING_BITS;

/// Floor tiles in the bitmap: the 4x4 floor's tiles. A copy is 4x4 or
/// coarser, and so is every child a copy naming children copies, so a
/// copy's own cells are always whole floor tiles.
pub const FLOOR_TILES: usize = tiles_in_level(FLOOR_LEVEL);
/// Cells in a floor tile: one run of the bitmap, in Morton order.
const FLOOR_TILE_CELLS: usize = cells_in_tile(FLOOR_LEVEL);
/// A floor tile's side, in cells.
const FLOOR_TILE_SIDE: u32 = FLOOR_TILE_CELLS.isqrt() as u32;
/// Words of one bit a floor tile.
const FLOOR_SET_WORDS: usize = FLOOR_TILES.div_ceil(u64::BITS as usize);
/// One bit a floor tile, by Morton index.
type FloorSet = [u64; FLOOR_SET_WORDS];

/// A floor tile, by its Morton index among the floor tiles.
type FloorIndex = u16;
/// The source of a floor tile no copy covers, or one already copied.
const NO_SOURCE: FloorIndex = FloorIndex::MAX;
const _: () = assert!(FLOOR_TILES <= NO_SOURCE as usize, "every floor tile has an index, and none is NO_SOURCE");

/// A floor tile's Morton index's `x` bits, the even ones...
const FLOOR_X_BITS: usize = 0x5555_5555 & (FLOOR_TILES - 1);
/// ...and its `y` bits, the odd ones: a neighbour's index is one of the
/// two fields stepped in place.
const FLOOR_Y_BITS: usize = FLOOR_X_BITS << 1;

/// Whether `floor_tile_index` is in `floor_set`.
fn contains(floor_set: &FloorSet, floor_tile_index: usize) -> bool {
    floor_set[floor_tile_index / u64::BITS as usize] >> (floor_tile_index % u64::BITS as usize) & 1 == 1
}

/// Adds `floor_tile_index` to `floor_set`.
fn insert(floor_set: &mut FloorSet, floor_tile_index: usize) {
    floor_set[floor_tile_index / u64::BITS as usize] |= 1 << (floor_tile_index % u64::BITS as usize);
}

/// Takes `floor_tile_index` out of `floor_set`.
fn remove(floor_set: &mut FloorSet, floor_tile_index: usize) {
    floor_set[floor_tile_index / u64::BITS as usize] &= !(1 << (floor_tile_index % u64::BITS as usize));
}

/// Every floor tile of `floor_set_word`, in Morton order, each read off it when its
/// turn comes: one taken out of it before then is passed over.
fn each_floor_tile(floor_set_word: impl Fn(usize) -> u64, mut visit_floor_tile: impl FnMut(usize)) {
    for word_index in 0..FLOOR_SET_WORDS {
        let mut floor_tiles = floor_set_word(word_index);
        while floor_tiles != 0 {
            visit_floor_tile(word_index * u64::BITS as usize + floor_tiles.trailing_zeros() as usize);
            floor_tiles &= floor_tiles - 1;
        }
    }
}

/// Codes the residual floor tile at `floor_tile_index` in Morton order, each cell at its
/// context's odds in `cells`, which `context_odds` learn; `code_cell` encodes, decodes
/// or prices a cell and says whether it is set. The floor tile's cells, as
/// one run.
fn code_residual_floor_tile(context_odds: &mut [ContextOdds; CONTEXT_COUNT], cells: &Bitmap, floor_tile_index: usize, code_cell: &mut impl FnMut(ContextOdds, usize) -> bool) -> u64 {
    let mut window = Window::around(cells, floor_tile_index);
    let mut floor_tile_run = 0;
    for place in 0..FLOOR_TILE_CELLS {
        let cell_context_odds = &mut context_odds[window.cell_context(place)];
        let cell_is_set = code_cell(*cell_context_odds, floor_tile_index * FLOOR_TILE_CELLS + place);
        if cell_is_set {
            window.0 |= 1 << WINDOW_PLACES[place];
            floor_tile_run |= 1 << place;
        }
        cell_context_odds.learn_cell(cell_is_set);
    }
    floor_tile_run
}

/// What the last pass takes for each residual floor tile, priced as the
/// greedy tiler reaches it, in Morton order, the pass's.
pub struct ResidualPricing {
    /// Each context's odds, as learned so far.
    context_odds: [ContextOdds; CONTEXT_COUNT],
    /// Each floor tile's bits, rounded to the nearest, as the counts the
    /// greedy tiler makes are whole bits.
    floor_tile_prices: Box<[u16; FLOOR_TILES]>,
}

impl ResidualPricing {
    /// No floor tile priced.
    pub fn new() -> Self {
        Self { context_odds: [ContextOdds::UNSEEN_CONTEXT; CONTEXT_COUNT], floor_tile_prices: Box::new([0; FLOOR_TILES]) }
    }

    /// Forgets every floor tile priced: before a bitmap's walk.
    pub fn clear(&mut self) {
        self.context_odds = [ContextOdds::UNSEEN_CONTEXT; CONTEXT_COUNT];
    }

    /// Prices the residual floor tile `floor tile` of `bitmap`, every residual
    /// floor tile before it in Morton order priced: its bits.
    pub fn price_residual_floor_tile(&mut self, bitmap: &Bitmap, floor_tile: Tile) -> u64 {
        let mut fixed_point_bits = 0;
        code_residual_floor_tile(&mut self.context_odds, bitmap, floor_tile.place_in_level(), &mut |cell_context_odds, cell_index| {
            let cell_is_set = bitmap.morton_run(cell_index, 1) == 1;
            fixed_point_bits += cell_context_odds.cell_cost(cell_is_set);
            cell_is_set
        });
        let rounded_price = ((fixed_point_bits + (1 << (FRACTION_BITS - 1))) >> FRACTION_BITS) as u16;
        self.floor_tile_prices[floor_tile.place_in_level()] = rounded_price;
        rounded_price as u64
    }

    /// The bits the residual floor tile at Morton index `floor_tile_index` was priced
    /// at.
    pub fn price_of(&self, floor_tile_index: usize) -> u64 {
        self.floor_tile_prices[floor_tile_index] as u64
    }
}

/// Room for the last pass, allocated once.
pub struct LastPass {
    /// How far the pass is.
    pass_state: PassState,
    /// Encoding: the cells as decoding has them.
    cells_as_decoded: Bitmap,
}

/// Copies waiting on their sources, and the contexts' odds.
struct PassState {
    /// A copy waiting on its source, and that source on its own: a
    /// chain, never longer than there are floor tiles.
    waiting_chain: FixedList<FloorIndex, FLOOR_TILES>,
    /// Copies whose source was a residual floor tile not yet coded, copied at
    /// the end.
    pending_copies: FixedList<FloorIndex, FLOOR_TILES>,
    /// Each context's odds.
    context_odds: [ContextOdds; CONTEXT_COUNT],
}

impl LastPass {
    /// Room for the pass.
    pub fn new() -> Self {
        let pass_state = PassState { waiting_chain: FixedList::new(), pending_copies: FixedList::new(), context_odds: [ContextOdds::UNSEEN_CONTEXT; CONTEXT_COUNT] };
        Self { pass_state, cells_as_decoded: Bitmap::new() }
    }

    /// Writes the pass of `floor_plan` for `bitmap` to `stream`, after its
    /// tree.
    pub fn encode_last_pass(&mut self, floor_plan: &mut FloorPlan, bitmap: &Bitmap, stream: &mut BitStream) {
        // The cells as decoding has them after the tree: none of a floor tile
        // the tree leaves unsaid.
        self.cells_as_decoded.copy_from(bitmap);
        let unsaid_floor_tiles = &floor_plan.unsaid_floor_tiles;
        each_floor_tile(|word_index| unsaid_floor_tiles[word_index], |floor_tile_index| self.cells_as_decoded.clear_morton_run(floor_tile_index * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS));
        // A pass coding no cell writes nothing: not even the coder's end.
        let codes_any_cell = floor_plan.residual_floor_tiles_left != [0; FLOOR_SET_WORDS];
        let mut range_encoder = RangeEncoder::default();
        self.pass_state.run_pass(floor_plan, &mut self.cells_as_decoded, &mut |cell_context_odds, cell_index| {
            let cell_is_set = bitmap.morton_run(cell_index, 1) == 1;
            range_encoder.encode_bit(cell_is_set, cell_context_odds.clear_probability(), stream);
            cell_is_set
        });
        if codes_any_cell {
            range_encoder.finish_stream(stream);
        }
    }

    /// Reads the pass of `floor_plan` into `cells`, which hold what the tree
    /// said.
    pub fn decode_last_pass(&mut self, floor_plan: &mut FloorPlan, cells: &mut Bitmap, stream_reader: &mut BitReader) {
        // A pass coding no cell has nothing after it: the coder's start
        // reads past the stream's end, all 0, and nothing more.
        let mut range_decoder = RangeDecoder::new(stream_reader);
        self.pass_state.run_pass(floor_plan, cells, &mut |cell_context_odds, _| range_decoder.decode_bit(cell_context_odds.clear_probability(), stream_reader));
    }
}

impl PassState {
    /// The pass itself, on `cells`: every floor tile copied or coded, in
    /// Morton order, then the copies that waited.
    fn run_pass(&mut self, floor_plan: &mut FloorPlan, cells: &mut Bitmap, code_cell: &mut impl FnMut(ContextOdds, usize) -> bool) {
        self.context_odds = [ContextOdds::UNSEEN_CONTEXT; CONTEXT_COUNT];
        self.pending_copies.clear();
        let unsaid_floor_tiles = floor_plan.unsaid_floor_tiles;
        each_floor_tile(|word_index| unsaid_floor_tiles[word_index], |floor_tile_index| {
            // A floor tile copied as the source of one before it has no source
            // left when its turn comes, and is passed over.
            if floor_plan.copy_sources[floor_tile_index] != NO_SOURCE {
                if !self.copy_floor_tile_chain(floor_plan, floor_tile_index, cells) {
                    self.pending_copies.push(floor_tile_index as FloorIndex);
                }
            } else if contains(&floor_plan.residual_floor_tiles_left, floor_tile_index) {
                let floor_tile_run = code_residual_floor_tile(&mut self.context_odds, cells, floor_tile_index, code_cell);
                cells.set_in_morton_run(floor_tile_index * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS, floor_tile_run);
                remove(&mut floor_plan.residual_floor_tiles_left, floor_tile_index);
            }
        });
        for pending_index in 0..self.pending_copies.len() {
            let chain_copied = self.copy_floor_tile_chain(floor_plan, self.pending_copies[pending_index] as usize, cells);
            debug_assert!(chain_copied, "every source is final by the end");
        }
    }

    /// Copies the floor tile at `floor_tile_index`, and first its source when that is a
    /// floor tile a copy covers not copied yet, and so on down the chain --
    /// unless the chain ends at a residual floor tile not coded yet: then
    /// nothing. Whether it copied.
    fn copy_floor_tile_chain(&mut self, floor_plan: &mut FloorPlan, floor_tile_index: usize, cells: &mut Bitmap) -> bool {
        self.waiting_chain.clear();
        self.waiting_chain.push(floor_tile_index as FloorIndex);
        while let Some(&waiting_floor_tile) = self.waiting_chain.last() {
            let source_floor_tile = floor_plan.copy_sources[waiting_floor_tile as usize];
            if source_floor_tile == NO_SOURCE {
                self.waiting_chain.pop();
            } else if contains(&floor_plan.residual_floor_tiles_left, source_floor_tile as usize) {
                return false;
            } else if floor_plan.copy_sources[source_floor_tile as usize] != NO_SOURCE {
                self.waiting_chain.push(source_floor_tile);
            } else {
                let source_run = cells.morton_run(source_floor_tile as usize * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS);
                cells.set_in_morton_run(waiting_floor_tile as usize * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS, source_run);
                floor_plan.copy_sources[waiting_floor_tile as usize] = NO_SOURCE;
                self.waiting_chain.pop();
            }
        }
        true
    }
}
