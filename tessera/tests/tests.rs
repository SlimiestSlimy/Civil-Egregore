//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it. What every tier
//! checks of a bitmap, and that turned bitmaps take about as many bits;
//! a bitmap's examination, its tree's counts, and the saved bitmaps.

// A tier uses what it needs of it.
#![allow(dead_code)]

use utilities::bitmap::Bitmap;
use std::cell::RefCell;
use examination::BitmapExamination;
use tessera::{BitStream, Tessera};

pub mod examination;
pub mod saved;
pub mod tree_stats;

/// The raw cells: what a bitmap costs written out, one bit a cell.
pub const RAW_CELLS: usize = utilities::bitmap::WIDTH * utilities::bitmap::HEIGHT;

/// A seed drawn fresh for a test, said on standard error: a failing
/// test's output says it.
pub fn fresh_test_seed() -> u64 {
    utilities::seed::given_or_fresh_seed(None)
}

/// The most Tessera may ever spend on a bitmap: the raw cells and 1%.
pub const CAP_BITS: usize = RAW_CELLS + RAW_CELLS / 100;

/// Bytes put after a stream, packed to the byte, to check it decodes the
/// same: a stream ends itself, so streams can lie one after another.
const BYTES_AFTER: [[u8; 8]; 2] = [[0xFF; 8], [0x5A, 0xC3, 0x0F, 0x96, 0x3C, 0xA5, 0xF0, 0x69]];

/// Everything that has to hold of one bitmap: it decodes to its own
/// cells, in at most [`CAP_BITS`], whatever bytes follow its stream.
/// Tests build in debug, so the encoder
/// also checks the tree it writes is the tree it counted. One `Tessera`
/// a thread, so nothing one bitmap leaves in it may leak into the next.
pub fn check_bitmap(bitmap: &Bitmap, case_label: &str) {
    thread_local! {
        /// The thread's `Tessera`, stream and decoded bitmap.
        static TESSERA: RefCell<(Tessera, BitStream, Bitmap, BitStream)> =
            RefCell::new((Tessera::new(), BitStream::default(), Bitmap::new(), BitStream::default()));
    }
    TESSERA.with_borrow_mut(|(tessera, stream, decoded_bitmap, followed_stream)| {
        let bitmap_examination = BitmapExamination::examine_bitmap(tessera, stream, decoded_bitmap, bitmap);
        assert!(bitmap_examination.written_bits <= CAP_BITS, "{case_label}: {} bits, over the cap of {CAP_BITS}", bitmap_examination.written_bits);
        if let Some((x, y)) = bitmap_examination.first_difference {
            panic!("{case_label}: cell ({x}, {y}) comes back {} instead of {}", decoded_bitmap.get(x, y), bitmap.get(x, y));
        }
        for trailing_bytes in BYTES_AFTER {
            followed_stream.load_bytes(&[&stream.to_bytes()[..], &trailing_bytes].concat());
            tessera.decode(followed_stream, decoded_bitmap);
            assert!(decoded_bitmap.words() == bitmap.words(), "{case_label}: decodes wrong with {trailing_bytes:02X?} after its stream");
        }
    });
}

// Turned bitmaps: that they bitmaps take about as many bits, shared by the fast and
// complete tiers. Tessera is not the same every way round -- Morton order
// halves top and bottom first, copies read up and to the left, the
// last pass codes rows top down -- so a turned bitmap's tiles, copies
// and contexts differ, and so do its bits; but a set's total should
// barely move, and a bias for one orientation shows there.

/// Quarter turns in a whole turn.
const QUARTER_TURNS: usize = 4;

/// `bitmap` turned a quarter clockwise.
fn turned_a_quarter(bitmap: &Bitmap) -> Bitmap {
    let mut turned_bitmap = Bitmap::new();
    for y in 0..=u8::MAX {
        for x in 0..=u8::MAX {
            if bitmap.get(x, y) {
                turned_bitmap.set(u8::MAX - y, x);
            }
        }
    }
    turned_bitmap
}

/// Every set of `named_sets`, named, takes about as many bits turned any way
/// round: each turn's total within `most_drift_percent` of the set's
/// total as drawn.
pub fn check_turned_bits(named_sets: Vec<(String, Vec<Bitmap>)>, most_drift_percent: f64) {
    let (mut tessera, mut stream) = (Tessera::new(), BitStream::default());
    for (set_name, maps) in named_sets {
        let mut bits_by_turn = [0; QUARTER_TURNS];
        for bitmap in &maps {
            let mut turned_bitmap = bitmap.clone();
            for turn_bits in &mut bits_by_turn {
                tessera.encode(&turned_bitmap, &mut stream);
                *turn_bits += stream.len();
                turned_bitmap = turned_a_quarter(&turned_bitmap);
            }
        }
        let as_drawn = bits_by_turn[0];
        for (quarter_turns, &turn_bits) in bits_by_turn.iter().enumerate().skip(1) {
            let drift_percent = (turn_bits as f64 / as_drawn as f64 - 1.0) * 100.0;
            assert!(
                drift_percent.abs() <= most_drift_percent,
                "{set_name}: turned {} degrees, {turn_bits} bits against {as_drawn} as drawn ({drift_percent:+.2}%)",
                quarter_turns * 90
            );
        }
    }
}
