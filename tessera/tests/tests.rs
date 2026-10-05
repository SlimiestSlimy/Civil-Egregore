//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it. What every tier
//! checks of a bitmap -- judgements on what `tessera::diagnostics`
//! gathers -- and that turned bitmaps take about as many bits.

// A tier uses what it needs of it.
#![allow(dead_code)]

use bitmap::Bitmap;
use std::cell::RefCell;
use tessera::diagnostics::RAW_CELLS;
use tessera::diagnostics::examination::Examination;
use tessera::{BitStream, Tessera};

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
pub fn check(bitmap: &Bitmap, label: &str) {
    thread_local! {
        /// The thread's `Tessera`, stream and decoded bitmap.
        static TESSERA: RefCell<(Tessera, BitStream, Bitmap, BitStream)> =
            RefCell::new((Tessera::new(), BitStream::default(), Bitmap::new(), BitStream::default()));
    }
    TESSERA.with_borrow_mut(|(tessera, stream, back, followed)| {
        let examined = Examination::of(tessera, stream, back, bitmap);
        assert!(examined.written_bits <= CAP_BITS, "{label}: {} bits, over the cap of {CAP_BITS}", examined.written_bits);
        if let Some((x, y)) = examined.first_difference {
            panic!("{label}: cell ({x}, {y}) comes back {} instead of {}", back.get(x, y), bitmap.get(x, y));
        }
        for after in BYTES_AFTER {
            followed.load_bytes(&[&stream.to_bytes()[..], &after].concat());
            tessera.decode(followed, back);
            assert!(back.words() == bitmap.words(), "{label}: decodes wrong with {after:02X?} after its stream");
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
    let mut turned = Bitmap::new();
    for y in 0..=u8::MAX {
        for x in 0..=u8::MAX {
            if bitmap.get(x, y) {
                turned.set(u8::MAX - y, x);
            }
        }
    }
    turned
}

/// Every set of `sets`, named, takes about as many bits turned any way
/// round: each turn's total within `most_drift_percent` of the set's
/// total as drawn.
pub fn check_turned_bits(sets: Vec<(String, Vec<Bitmap>)>, most_drift_percent: f64) {
    let (mut tessera, mut stream) = (Tessera::new(), BitStream::default());
    for (set, maps) in sets {
        let mut bits_by_turn = [0; QUARTER_TURNS];
        for bitmap in &maps {
            let mut turned = bitmap.clone();
            for bits in &mut bits_by_turn {
                tessera.encode(&turned, &mut stream);
                *bits += stream.len();
                turned = turned_a_quarter(&turned);
            }
        }
        let as_drawn = bits_by_turn[0];
        for (quarter_turns, &bits) in bits_by_turn.iter().enumerate().skip(1) {
            let drift = (bits as f64 / as_drawn as f64 - 1.0) * 100.0;
            assert!(
                drift.abs() <= most_drift_percent,
                "{set}: turned {} degrees, {bits} bits against {as_drawn} as drawn ({drift:+.2}%)",
                quarter_turns * 90
            );
        }
    }
}
