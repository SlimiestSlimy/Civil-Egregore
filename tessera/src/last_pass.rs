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

use crate::arithmetic::{Decoder, Encoder, FINISHING_BITS};
use crate::bit_stream::{BitReader, BitStream};
use crate::tile::{cells_in_tile, tiles_in_level, Tile, CELLS, FLOOR_LEVEL};
use bitmap::Bitmap;
use utilities::fixed_list::FixedList;

/// Where a cell's context reads, relative to it, `(dx, dy)`: top left,
/// above and left, then the same two cells away -- each before it in
/// Morton order.
pub const CONTEXT_CELLS: [(i8, i8); 6] = [(-1, -1), (0, -1), (-1, 0), (-2, -2), (0, -2), (-2, 0)];
/// Contexts: one for every value the context cells can hold.
const CONTEXTS: usize = 1 << CONTEXT_CELLS.len();

/// The most bits the pass takes over one a residual cell
/// (`docs/tessera.md`, "The odds").
pub const MOST_EXTRA_BITS: usize = CONTEXTS * (CELLS.ilog2() as usize / 2 + 1) + (CELLS >> 10) + (CELLS >> 12) + FINISHING_BITS;

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

/// Whether `index` is in `set`.
fn contains(set: &FloorSet, index: usize) -> bool {
    set[index / u64::BITS as usize] >> (index % u64::BITS as usize) & 1 == 1
}

/// Adds `index` to `set`.
fn insert(set: &mut FloorSet, index: usize) {
    set[index / u64::BITS as usize] |= 1 << (index % u64::BITS as usize);
}

/// Takes `index` out of `set`.
fn remove(set: &mut FloorSet, index: usize) {
    set[index / u64::BITS as usize] &= !(1 << (index % u64::BITS as usize));
}

/// Every floor tile of `set`, in Morton order, each read off it when its
/// turn comes: one taken out of it before then is passed over.
fn each_floor_tile(set: impl Fn(usize) -> u64, mut visit: impl FnMut(usize)) {
    for word_index in 0..FLOOR_SET_WORDS {
        let mut floor_tiles = set(word_index);
        while floor_tiles != 0 {
            visit(word_index * u64::BITS as usize + floor_tiles.trailing_zeros() as usize);
            floor_tiles &= floor_tiles - 1;
        }
    }
}

/// Codes the residual floor tile at `index` in Morton order, each cell at its
/// context's odds in `cells`, which `odds` learn; `code` encodes, decodes
/// or prices a cell and says whether it is set. The floor tile's cells, as
/// one run.
fn code_residual_floor_tile(odds: &mut [ContextOdds; CONTEXTS], cells: &Bitmap, index: usize, code: &mut impl FnMut(ContextOdds, usize) -> bool) -> u64 {
    let mut window = Window::around(cells, index);
    let mut floor_tile_run = 0;
    for place in 0..FLOOR_TILE_CELLS {
        let context = &mut odds[window.context(place)];
        let set = code(*context, index * FLOOR_TILE_CELLS + place);
        if set {
            window.0 |= 1 << WINDOW_PLACES[place];
            floor_tile_run |= 1 << place;
        }
        context.learn(set);
    }
    floor_tile_run
}

/// What the last pass takes for each residual floor tile, priced as the
/// greedy tiler reaches it, in Morton order, the pass's.
pub struct Pricing {
    /// Each context's odds, as learned so far.
    odds: [ContextOdds; CONTEXTS],
    /// Each floor tile's bits, rounded to the nearest, as the counts the
    /// greedy tiler makes are whole bits.
    prices: Box<[u16; FLOOR_TILES]>,
}

impl Pricing {
    /// No floor tile priced.
    pub fn new() -> Self {
        Self { odds: [ContextOdds::UNSEEN; CONTEXTS], prices: Box::new([0; FLOOR_TILES]) }
    }

    /// Forgets every floor tile priced: before a bitmap's walk.
    pub fn clear(&mut self) {
        self.odds = [ContextOdds::UNSEEN; CONTEXTS];
    }

    /// Prices the residual floor tile `floor tile` of `bitmap`, every residual
    /// floor tile before it in Morton order priced: its bits.
    pub fn price(&mut self, bitmap: &Bitmap, floor_tile: Tile) -> u64 {
        let mut bits = 0;
        code_residual_floor_tile(&mut self.odds, bitmap, floor_tile.index(), &mut |odds, cell_index| {
            let set = bitmap.morton_run(cell_index, 1) == 1;
            bits += odds.cost(set);
            set
        });
        let price = ((bits + (1 << (FRACTION_BITS - 1))) >> FRACTION_BITS) as u16;
        self.prices[floor_tile.index()] = price;
        price as u64
    }

    /// The bits the residual floor tile at Morton index `index` was priced
    /// at.
    pub fn of(&self, index: usize) -> u64 {
        self.prices[index] as u64
    }
}

/// Room for the last pass, allocated once.
pub struct LastPass {
    /// How far the pass is.
    state: PassState,
    /// Encoding: the cells as decoding has them.
    cells_as_decoded: Bitmap,
}

/// Copies waiting on their sources, and the contexts' odds.
struct PassState {
    /// A copy waiting on its source, and that source on its own: a
    /// chain, never longer than there are floor tiles.
    waiting: FixedList<FloorIndex, FLOOR_TILES>,
    /// Copies whose source was a residual floor tile not yet coded, copied at
    /// the end.
    pending: FixedList<FloorIndex, FLOOR_TILES>,
    /// Each context's odds.
    odds: [ContextOdds; CONTEXTS],
}

impl LastPass {
    /// Room for the pass.
    pub fn new() -> Self {
        let state = PassState { waiting: FixedList::new(), pending: FixedList::new(), odds: [ContextOdds::UNSEEN; CONTEXTS] };
        Self { state, cells_as_decoded: Bitmap::new() }
    }

    /// Writes the pass of `plan` for `bitmap` to `stream`, after its
    /// tree.
    pub fn encode(&mut self, plan: &mut FloorPlan, bitmap: &Bitmap, stream: &mut BitStream) {
        // The cells as decoding has them after the tree: none of a floor tile
        // the tree leaves unsaid.
        self.cells_as_decoded.copy_from(bitmap);
        let unsaid = &plan.unsaid;
        each_floor_tile(|word_index| unsaid[word_index], |index| self.cells_as_decoded.clear_morton_run(index * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS));
        // A pass coding no cell writes nothing: not even the coder's end.
        let codes_any_cell = plan.residual != [0; FLOOR_SET_WORDS];
        let mut encoder = Encoder::default();
        self.state.run_pass(plan, &mut self.cells_as_decoded, &mut |odds, cell_index| {
            let set = bitmap.morton_run(cell_index, 1) == 1;
            encoder.encode(set, odds.clear_probability(), stream);
            set
        });
        if codes_any_cell {
            encoder.finish(stream);
        }
    }

    /// Reads the pass of `plan` into `cells`, which hold what the tree
    /// said.
    pub fn decode(&mut self, plan: &mut FloorPlan, cells: &mut Bitmap, reader: &mut BitReader) {
        // A pass coding no cell has nothing after it: the coder's start
        // reads past the stream's end, all 0, and nothing more.
        let mut decoder = Decoder::new(reader);
        self.state.run_pass(plan, cells, &mut |odds, _| decoder.decode(odds.clear_probability(), reader));
    }
}

impl PassState {
    /// The pass itself, on `cells`: every floor tile copied or coded, in
    /// Morton order, then the copies that waited.
    fn run_pass(&mut self, plan: &mut FloorPlan, cells: &mut Bitmap, code: &mut impl FnMut(ContextOdds, usize) -> bool) {
        self.odds = [ContextOdds::UNSEEN; CONTEXTS];
        self.pending.clear();
        let unsaid = plan.unsaid;
        each_floor_tile(|word_index| unsaid[word_index], |index| {
            // A floor tile copied as the source of one before it has no source
            // left when its turn comes, and is passed over.
            if plan.sources[index] != NO_SOURCE {
                if !self.copy_floor_tile_chain(plan, index, cells) {
                    self.pending.push(index as FloorIndex);
                }
            } else if contains(&plan.residual, index) {
                let run = code_residual_floor_tile(&mut self.odds, cells, index, code);
                cells.set_in_morton_run(index * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS, run);
                remove(&mut plan.residual, index);
            }
        });
        for pending_index in 0..self.pending.len() {
            let copied = self.copy_floor_tile_chain(plan, self.pending[pending_index] as usize, cells);
            debug_assert!(copied, "every source is final by the end");
        }
    }

    /// Copies the floor tile at `index`, and first its source when that is a
    /// floor tile a copy covers not copied yet, and so on down the chain --
    /// unless the chain ends at a residual floor tile not coded yet: then
    /// nothing. Whether it copied.
    fn copy_floor_tile_chain(&mut self, plan: &mut FloorPlan, index: usize, cells: &mut Bitmap) -> bool {
        self.waiting.clear();
        self.waiting.push(index as FloorIndex);
        while let Some(&waiting) = self.waiting.last() {
            let source = plan.sources[waiting as usize];
            if source == NO_SOURCE {
                self.waiting.pop();
            } else if contains(&plan.residual, source as usize) {
                return false;
            } else if plan.sources[source as usize] != NO_SOURCE {
                self.waiting.push(source);
            } else {
                let run = cells.morton_run(source as usize * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS);
                cells.set_in_morton_run(waiting as usize * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS, run);
                plan.sources[waiting as usize] = NO_SOURCE;
                self.waiting.pop();
            }
        }
        true
    }
}
