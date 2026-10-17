//! The binary count tree, written and read: the stream for sparse
//! bitmaps, the Morton order halved again and again, each run saying how
//! many of its set cells lie in its first half. `docs/tessera.md`, "The
//! stream: tree or binary count tree".
//!
//! Function by function: `docs/reference.md`, "`binary_count_tree.rs`".

use crate::bit_stream::{truncated_binary_bits, truncated_binary_shape, BitReader, BitSink};
use crate::set_cells_before_each_word::SetCellsBeforeEachWord;
use utilities::bitmap::WORDS;
use utilities::bitmap::Bitmap;

/// The counts a run of `cells` cells, `set_cells` of them set, could have in
/// its first half: the fewest, and how many there are.
const fn first_half_counts(cells: usize, set_cells: u64) -> (u64, u64) {
    let half_cells = (cells / 2) as u64;
    let fewest_first_half_set = set_cells.saturating_sub(half_cells);
    let most_first_half_set = if set_cells < half_cells { set_cells } else { half_cells };
    (fewest_first_half_set, most_first_half_set - fewest_first_half_set + 1)
}

/// The bits a run of the cells `cell_run` holds, `cells` of them, `set_cells` of
/// them set, takes inside it: what it says of its halves, and they of
/// theirs.
const fn run_bits(cell_run: u64, cells: usize, set_cells: u64) -> u64 {
    if set_cells == 0 || set_cells == cells as u64 {
        return 0;
    }
    let half_cells = cells / 2;
    let first_half = cell_run & ((1 << half_cells) - 1);
    let first_half_set = first_half.count_ones() as u64;
    let (fewest_first_half_set, counts) = first_half_counts(cells, set_cells);
    truncated_binary_bits(first_half_set - fewest_first_half_set, counts)
        + run_bits(first_half, half_cells, first_half_set)
        + run_bits(cell_run >> half_cells, half_cells, set_cells - first_half_set)
}

/// A byte of cells: a run read back by one lookup in [`BYTE_RUNS`].
const BYTE_CELLS: usize = u8::BITS as usize;

/// The most bits a run of a byte of cells takes inside it.
const MOST_BYTE_RUN_BITS: u8 = {
    let mut most_bits = 0;
    let mut byte_cell_run = 0;
    while byte_cell_run < 1 << BYTE_CELLS {
        let bits_taken = run_bits(byte_cell_run as u64, BYTE_CELLS, (byte_cell_run as u64).count_ones() as u64) as u8;
        if bits_taken > most_bits {
            most_bits = bits_taken;
        }
        byte_cell_run += 1;
    }
    most_bits
};

/// A run of a byte of cells, as read back: its cells, and how many bits
/// saying them took.
#[derive(Clone, Copy)]
struct ByteRun {
    /// The cells, bit `i` the run's cell `i`.
    cells: u8,
    /// The bits read.
    bits_read: u8,
}

/// Every run of a byte of cells as read back, by how many of its cells
/// are set and the next [`MOST_BYTE_RUN_BITS`] bits of the stream: what
/// [`read_word`] would read, made when compiling by reading the same way
/// off those bits. 18 KiB.
static BYTE_RUNS: [[ByteRun; 1 << MOST_BYTE_RUN_BITS]; BYTE_CELLS + 1] = {
    let mut byte_runs = [[ByteRun { cells: 0, bits_read: 0 }; 1 << MOST_BYTE_RUN_BITS]; BYTE_CELLS + 1];
    let mut set_cells = 0;
    while set_cells <= BYTE_CELLS {
        let mut said_bits = 0;
        while said_bits < 1 << MOST_BYTE_RUN_BITS {
            let (cells, bits_read) = read_run_off(said_bits as u64, BYTE_CELLS, set_cells as u64);
            byte_runs[set_cells][said_bits] = ByteRun { cells: cells as u8, bits_read: bits_read as u8 };
            said_bits += 1;
        }
        set_cells += 1;
    }
    byte_runs
};

/// Reads a run of `cells` cells, `set_cells` of them set, off the bits `said_bits`,
/// the next one lowest, as [`read_word`] reads it off the stream: its
/// cells, and how many bits it took.
const fn read_run_off(said_bits: u64, cells: usize, set_cells: u64) -> (u64, u32) {
    if set_cells == 0 || set_cells == cells as u64 {
        return (if set_cells == 0 { 0 } else { (1 << cells) - 1 }, 0);
    }
    let half_cells = cells / 2;
    let (fewest_first_half_set, counts) = first_half_counts(cells, set_cells);
    let (count, count_bits) = read_truncated_binary_off(said_bits, counts);
    let first_half_set = fewest_first_half_set + count;
    let (first_half, first_half_bits) = read_run_off(said_bits >> count_bits, half_cells, first_half_set);
    let (second_half, second_half_bits) = read_run_off(said_bits >> (count_bits + first_half_bits), half_cells, set_cells - first_half_set);
    (first_half | second_half << half_cells, count_bits + first_half_bits + second_half_bits)
}

/// Reads a value of `value_range` in truncated binary off the bits `said_bits`, as
/// [`BitReader::truncated_binary`] reads it off the stream: the value,
/// and how many bits it took.
const fn read_truncated_binary_off(said_bits: u64, value_range: u64) -> (u64, u32) {
    let Some((short_width, short_codes)) = truncated_binary_shape(value_range) else { return (0, 0) };
    let leading_bits = said_bits & ((1 << short_width) - 1);
    if leading_bits < short_codes {
        return (leading_bits, short_width as u32);
    }
    ((leading_bits << 1 | (said_bits >> short_width & 1)) - short_codes, short_width as u32 + 1)
}

/// Runs this short are counted by looking them up in [`SHORT_RUN_BITS`]:
/// two bytes of cells, a quarter of a word.
const SHORT_RUN_CELLS: usize = 2 * BYTE_CELLS;

/// The bits every run of [`SHORT_RUN_CELLS`] cells takes inside it, by
/// its cells, made when compiling: its own count, then each byte's bits.
/// 64 KiB.
static SHORT_RUN_BITS: [u8; 1 << SHORT_RUN_CELLS] = {
    let mut byte_bits = [0; 1 << BYTE_CELLS];
    let mut byte_cell_run = 0;
    while byte_cell_run < byte_bits.len() {
        byte_bits[byte_cell_run] = run_bits(byte_cell_run as u64, BYTE_CELLS, (byte_cell_run as u64).count_ones() as u64);
        byte_cell_run += 1;
    }
    let mut short_run_bits = [0; 1 << SHORT_RUN_CELLS];
    let mut short_cell_run = 0;
    while short_cell_run < short_run_bits.len() {
        let set_cells = (short_cell_run as u64).count_ones() as u64;
        if set_cells != 0 && set_cells != SHORT_RUN_CELLS as u64 {
            let (fewest_first_half_set, counts) = first_half_counts(SHORT_RUN_CELLS, set_cells);
            let own_count_bits = truncated_binary_bits((short_cell_run as u64 & 0xFF).count_ones() as u64 - fewest_first_half_set, counts);
            short_run_bits[short_cell_run] = (own_count_bits + byte_bits[short_cell_run & 0xFF] + byte_bits[short_cell_run >> BYTE_CELLS]) as u8;
        }
        short_cell_run += 1;
    }
    short_run_bits
};

/// Cells a word of the bitmap holds: a run of the Morton order.
const WORD_CELLS: usize = u64::BITS as usize;

/// A bitmap's words, and how many cells are set before each: what a
/// run longer than a word holds in each half is read off those.
struct Words<'a> {
    /// The bitmap's words, in Morton order.
    words: &'a [u64; WORDS],
    /// Cells set before each word.
    set_cells_before_each_word: &'a SetCellsBeforeEachWord,
}

impl Words<'_> {
    /// Cells set in the first half of the run of `count` words from
    /// `first_word`.
    fn first_half_set(&self, first_word: usize, count: usize) -> u64 {
        self.set_cells_before_each_word.in_words(first_word, count / 2)
    }

    /// Writes the run of `count` words from `first_word`, `set_cells` of its cells
    /// set: nothing if all or none is, else how many lie in its first
    /// half and each half in turn.
    fn write(&self, stream: &mut impl BitSink, first_word: usize, count: usize, set_cells: u64) {
        if set_cells == 0 || set_cells == (count * WORD_CELLS) as u64 {
            return;
        }
        if count == 1 {
            write_word(stream, self.words[first_word], WORD_CELLS, set_cells);
            return;
        }
        let (half_words, first_half_set) = (count / 2, self.first_half_set(first_word, count));
        let (fewest_first_half_set, counts) = first_half_counts(count * WORD_CELLS, set_cells);
        stream.push_truncated_binary(first_half_set - fewest_first_half_set, counts);
        self.write(stream, first_word, half_words, first_half_set);
        self.write(stream, first_word + half_words, half_words, set_cells - first_half_set);
    }
}

/// Writes the run of the cells `byte_cell_run` holds, `cells` of them -- a word or
/// less -- `set_cells` of them set, as [`Words::write`] a longer one; counted,
/// a run of [`SHORT_RUN_CELLS`] is looked up in [`SHORT_RUN_BITS`].
fn write_word(stream: &mut impl BitSink, cell_run: u64, cells: usize, set_cells: u64) {
    if set_cells == 0 || set_cells == cells as u64 {
        return;
    }
    if cells == SHORT_RUN_CELLS
        && let Some(bit_count) = stream.counted_bits()
    {
        *bit_count += SHORT_RUN_BITS[cell_run as usize] as u64;
        return;
    }
    let half_cells = cells / 2;
    let first_half = cell_run & ((1 << half_cells) - 1);
    let first_half_set = first_half.count_ones() as u64;
    let (fewest_first_half_set, counts) = first_half_counts(cells, set_cells);
    stream.push_truncated_binary(first_half_set - fewest_first_half_set, counts);
    write_word(stream, first_half, half_cells, first_half_set);
    write_word(stream, cell_run >> half_cells, half_cells, set_cells - first_half_set);
}

/// Writes `bitmap`'s binary count tree; `set_cells_before_each_word` are `bitmap`'s.
pub fn write(stream: &mut impl BitSink, bitmap: &Bitmap, set_cells_before_each_word: &SetCellsBeforeEachWord) {
    let words = Words { words: bitmap.words(), set_cells_before_each_word };
    stream.push_gamma(set_cells_before_each_word.total_set_cells() + 1);
    words.write(stream, 0, WORDS, set_cells_before_each_word.total_set_cells());
}

/// Reads a binary count tree into `cell_values`, which start clear.
pub fn read_binary_count_tree(stream_reader: &mut BitReader, cell_values: &mut Bitmap) {
    let set_cells = stream_reader.read_gamma() - 1;
    read_words(stream_reader, cell_values.words_mut(), set_cells);
}

/// Reads the run of `words`' words -- all of them, clear to start with
/// -- `set_cells` of its cells set: sets every cell if every one is, reads
/// nothing if none is, else reads how many lie in its first half and
/// each half in turn.
fn read_words(stream_reader: &mut BitReader, words: &mut [u64], set_cells: u64) {
    let cells = words.len() * WORD_CELLS;
    if set_cells == 0 {
        return;
    }
    if words.len() == 1 {
        words[0] = read_word(stream_reader, WORD_CELLS, set_cells);
        return;
    }
    if set_cells == cells as u64 {
        words.fill(u64::MAX);
        return;
    }
    if set_cells == 1 {
        let place = read_lone_cell_place(stream_reader, cells);
        words[place / WORD_CELLS] = 1 << (place % WORD_CELLS);
        return;
    }
    let (fewest_first_half_set, counts) = first_half_counts(cells, set_cells);
    let first_half_set = fewest_first_half_set + stream_reader.truncated_binary(counts);
    let (first_half, second_half) = words.split_at_mut(words.len() / 2);
    read_words(stream_reader, first_half, first_half_set);
    read_words(stream_reader, second_half, set_cells - first_half_set);
}

/// Reads the run of `cells` cells -- a word or less -- `set_cells` of them
/// set, as [`read_words`] a longer one, and gives its cells: bit `i` the
/// run's cell `i`.
fn read_word(stream_reader: &mut BitReader, cells: usize, set_cells: u64) -> u64 {
    if set_cells == 0 {
        return 0;
    }
    if set_cells == cells as u64 {
        return u64::MAX >> (WORD_CELLS - cells);
    }
    if set_cells == 1 {
        return 1 << read_lone_cell_place(stream_reader, cells);
    }
    if cells == BYTE_CELLS {
        let byte_run = BYTE_RUNS[set_cells as usize][stream_reader.peek_bits(MOST_BYTE_RUN_BITS) as usize];
        stream_reader.skip_bits(byte_run.bits_read);
        return byte_run.cells as u64;
    }
    let half_cells = cells / 2;
    let (fewest_first_half_set, counts) = first_half_counts(cells, set_cells);
    let first_half_set = fewest_first_half_set + stream_reader.truncated_binary(counts);
    let first_half = read_word(stream_reader, half_cells, first_half_set);
    first_half | read_word(stream_reader, half_cells, set_cells - first_half_set) << half_cells
}

/// Reads where the one set cell of a run of `cells` cells is: a bit a
/// halving, its place from the top bit down, each bit flipped.
fn read_lone_cell_place(stream_reader: &mut BitReader, cells: usize) -> usize {
    let halving_count = cells.ilog2();
    let said_bits = stream_reader.value(halving_count as u8);
    ((!said_bits).reverse_bits() >> (u64::BITS - halving_count)) as usize
}
