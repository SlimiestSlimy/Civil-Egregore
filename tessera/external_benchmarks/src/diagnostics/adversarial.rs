//! Adversarial bitmaps against each external codec: for each of CCITT
//! G4, JBIG and zstd, four searches at once, one a core, for the bitmap
//! where Tessera's bits most exceed that codec's -- the library's search
//! (`tessera::diagnostics::adversarial`), scored as Tessera's bits less the codec's. The
//! worst found for each codec is kept as a PBM image in
//! `transient_data/worst/`, replaced only when beaten, and carried on
//! from by the next run; every kept bitmap is checked to round trip
//! through both Tessera and the codec. Then, for each worst bitmap, both
//! encoders' times on it. The table -- a row a codec -- is printed and
//! kept in `transient_data/measurements/external_adversarial.csv`.
//!
//! From `tessera/`, which the manifest path is relative to, in release:
//!
//! ```text
//! cargo run --release --manifest-path external_benchmarks/Cargo.toml --bin adversarial
//! cargo run --release --manifest-path external_benchmarks/Cargo.toml --bin adversarial -- 4000
//! ```
//!
//! The argument, if given, is how many changes each search tries on the
//! whole plane from each start: one long search settles deeper than many
//! short ones.

#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use tessera::diagnostics::adversarial::{worst, search_at_once, SearchEffort, BitmapScore, SEARCHES_AT_ONCE};
use tessera::corpus::corpus_seed;
use tessera::transient_data;
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;
use bitmap::Bitmap;
use external_benchmarks::codecs::g4::G4Codec;
use external_benchmarks::codecs::tessera::TesseraCodec;
use external_benchmarks::codecs::jbig::JbigCodec;
use external_benchmarks::codecs::zstd::ZstdCodec;
use external_benchmarks::codecs::BitmapCodec;
use external_benchmarks::rows::BitmapRows;
use std::time::Instant;

/// Times each worst bitmap is encoded to time it: the median is kept.
const TIMING_REPEATS: usize = 21;

/// A codec to search against: what its worst bitmap is kept under, and how to
/// make one.
struct OpposingCodec {
    /// The worst bitmap's name, `against_` and the codec.
    worst: &'static str,
    /// A fresh codec: each search gets its own.
    make_codec: fn() -> Box<dyn BitmapCodec>,
}

/// Every codec searched against.
const OPPOSING_CODECS: [OpposingCodec; 4] = [
    OpposingCodec { worst: "against_g4", make_codec: || Box::new(G4Codec::new()) },
    OpposingCodec { worst: "against_jbig", make_codec: || Box::new(JbigCodec::new()) },
    OpposingCodec { worst: "against_zstd3", make_codec: || Box::new(ZstdCodec::new(3)) },
    OpposingCodec { worst: "against_zstd19", make_codec: || Box::new(ZstdCodec::new(19)) },
];

/// Tessera's bits and the codec's, on one bitmap.
fn both_bits(tessera: &mut TesseraCodec, opposing_codec: &mut dyn BitmapCodec, bitmap: &Bitmap) -> (u64, u64) {
    let bitmap_rows = BitmapRows::from_bitmap(bitmap);
    tessera.encode_bitmap(bitmap, &bitmap_rows);
    opposing_codec.encode_bitmap(bitmap, &bitmap_rows);
    (tessera.encoded_bits() as u64, opposing_codec.encoded_bits() as u64)
}

/// Tessera's bits less the codec's: what the search maximizes.
fn score_against_codec(tessera: &mut TesseraCodec, opposing_codec: &mut dyn BitmapCodec, bitmap: &Bitmap) -> BitmapScore {
    let (tessera_bits, codec_bits) = both_bits(tessera, opposing_codec, bitmap);
    BitmapScore { gap: tessera_bits as i64 - codec_bits as i64, tessera_bits }
}

/// The median time, in microseconds, `encode_once` takes over [`TIMING_REPEATS`] runs.
fn median_micros(mut encode_once: impl FnMut()) -> f64 {
    let mut encode_times: Vec<f64> = (0..TIMING_REPEATS)
        .map(|_| {
            let encode_start = Instant::now();
            encode_once();
            encode_start.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    encode_times.sort_by(f64::total_cmp);
    encode_times[TIMING_REPEATS / 2]
}

/// Searches against every codec, keeps any new worst, and reports the
/// worst bitmaps with both encoders' bits and times.
fn main() {
    let seed = corpus_seed();
    // The changes tried on the plane from each start: the first argument, if given.
    let whole_bitmap_changes = std::env::args().nth(1).map_or(SearchEffort::default().whole_bitmap, |argument_text| argument_text.parse().expect("a number of changes"));
    let search_effort = SearchEffort { whole_bitmap: whole_bitmap_changes, ..SearchEffort::default() };
    let mut table = Table::new(&[
        "against",
        "worst gap\nthis run",
        "kept gap\nbefore",
        "kept gap\nnow",
        "worst\nreplaced",
        "Tessera\nbits",
        "codec\nbits",
        "Tessera\nencode us",
        "codec\nencode us",
    ]);
    for (opponent_index, opponent_entry) in OPPOSING_CODECS.iter().enumerate() {
        let kept_worst = worst::read_worst(opponent_entry.worst);
        let opponent_seed = seed.wrapping_add(opponent_index as u64 * SEARCHES_AT_ONCE);
        let search_outcomes = search_at_once(opponent_seed, kept_worst.clone(), search_effort, &|| {
            let (mut tessera, mut search_codec) = (TesseraCodec::new(), (opponent_entry.make_codec)());
            move |bitmap: &Bitmap, _| score_against_codec(&mut tessera, search_codec.as_mut(), bitmap)
        });
        let worst = search_outcomes.iter().map(|search_outcome| &search_outcome.worst).max_by_key(|found_worst| found_worst.bitmap_score.gap).expect("a search");

        let (mut tessera, mut timed_codec) = (TesseraCodec::new(), (opponent_entry.make_codec)());
        let kept_gap = kept_worst.as_ref().map(|bitmap| score_against_codec(&mut tessera, timed_codec.as_mut(), bitmap).gap);
        let kept_beaten = kept_gap.is_none_or(|gap| worst.bitmap_score.gap > gap);
        if kept_beaten {
            worst::write(opponent_entry.worst, &worst.bitmap, &format!("{}: Tessera {} bits over {}", opponent_entry.worst, worst.bitmap_score.gap, timed_codec.codec_name()));
        }

        // The worst bitmap, whichever it is now: both encoders must give it
        // back, and both are timed on it.
        let bitmap = worst::read_worst(opponent_entry.worst).expect("kept");
        let bitmap_rows = BitmapRows::from_bitmap(&bitmap);
        for round_trip_codec in [&mut tessera as &mut dyn BitmapCodec, timed_codec.as_mut()] {
            round_trip_codec.encode_bitmap(&bitmap, &bitmap_rows);
            round_trip_codec.decode_last_encoding();
            assert!(round_trip_codec.decoded_matches(&bitmap_rows), "{} does not round trip {}", round_trip_codec.codec_name(), opponent_entry.worst);
        }
        let (tessera_bits, codec_bits) = both_bits(&mut tessera, timed_codec.as_mut(), &bitmap);
        let tessera_micros = median_micros(|| tessera.encode_bitmap(&bitmap, &bitmap_rows));
        let codec_micros = median_micros(|| timed_codec.encode_bitmap(&bitmap, &bitmap_rows));
        table.add_row(&[
            timed_codec.codec_name(),
            worst.bitmap_score.gap.to_string(),
            kept_gap.map_or("none".to_string(), |gap| gap.to_string()),
            (tessera_bits as i64 - codec_bits as i64).to_string(),
            if kept_beaten { "yes" } else { "no" }.to_string(),
            tessera_bits.to_string(),
            codec_bits.to_string(),
            format!("{tessera_micros:.0}"),
            format!("{codec_micros:.0}"),
        ]);
    }
    let mut report = Report::new(
        "external_adversarial",
        "cargo run --release --manifest-path external_benchmarks/Cargo.toml --bin adversarial",
    );
    report.add_note(format!(
        "{SEARCHES_AT_ONCE} searches a codec, {} changes a window start, {} a plane start; gap: Tessera's bits less the codec's",
        search_effort.window, search_effort.whole_bitmap
    ));
    report.add_note(format!("times: the median of {TIMING_REPEATS} encodings of the worst bitmap"));
    report.add_table("the worst bitmaps, each round tripping through both", table);
    transient_data::publish_report(report);
}
