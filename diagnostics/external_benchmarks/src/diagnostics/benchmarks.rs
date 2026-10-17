//! Tessera against existing bitmap compressors -- CCITT Group 4, JBIG
//! (jbigkit) and zstd -- on the same large corpus the diagnostics tool's `timing` uses:
//! every generator, `corpus::TIMING_PER_GENERATOR` distinct bitmaps each. For
//! each family and codec: the mean encoded size, and the mean time to
//! encode and to decode, each bitmap encoded once and decoded once and
//! checked. Run in release, on its own -- no profiler, nothing else
//! busy:
//!
//! From `tessera/`, which the manifest path is relative to; the corpus bitmaps
//! use the seed every other run does (`transient_data/seed.csv`):
//!
//! ```text
//! cargo run --release --manifest-path external_benchmarks/Cargo.toml
//! cargo run --release --manifest-path external_benchmarks/Cargo.toml -- 400
//! ```
//!
//! The argument, if given, is how many bitmaps each generator makes.
//! Every bitmap is turned into rows before anything is timed.

#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use tessera::diagnostics::RAW_CELLS;
use tessera::corpus::{corpus_families, HowMany, TIMING_PER_GENERATOR};
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

/// zstd's default level, and a high one: how much more a general
/// compressor finds when given the time.
const ZSTD_LEVELS: [i32; 2] = [3, 19];

/// One family's bitmaps, named, each also in rows.
struct NamedFamily {
    /// What the family is called.
    family_name: String,
    /// Its bitmaps, each with its rows.
    bitmaps: Vec<(Bitmap, BitmapRows)>,
}

impl NamedFamily {
    /// A family of these bitmaps.
    fn from_named_bitmaps(family_name: String, bitmaps: impl Iterator<Item = Bitmap>) -> Self {
        Self { family_name, bitmaps: bitmaps.map(|bitmap| { let bitmap_rows = BitmapRows::from_bitmap(&bitmap); (bitmap, bitmap_rows) }).collect() }
    }
}

/// One codec's totals over a family.
#[derive(Default, Clone, Copy)]
struct CodecTotals {
    /// Bitmaps.
    bitmap_count: usize,
    /// Encoded bits.
    total_bits: usize,
    /// Seconds encoding.
    encode_seconds: f64,
    /// Seconds decoding.
    decode_seconds: f64,
}

impl CodecTotals {
    /// These and `other_totals` together.
    fn plus_totals(self, other_totals: CodecTotals) -> CodecTotals {
        CodecTotals {
            bitmap_count: self.bitmap_count + other_totals.bitmap_count,
            total_bits: self.total_bits + other_totals.total_bits,
            encode_seconds: self.encode_seconds + other_totals.encode_seconds,
            decode_seconds: self.decode_seconds + other_totals.decode_seconds,
        }
    }
}

/// Builds the corpus, runs every codec over every family, and prints a
/// table a family, then one for them all.
fn main() {
    let per_generator = std::env::args().nth(1).map_or(TIMING_PER_GENERATOR, |count| count.parse().expect("a count"));
    let named_families: Vec<NamedFamily> =
        corpus_families(HowMany::SameCountEach(per_generator)).into_iter().map(|(family_name, bitmaps)| NamedFamily::from_named_bitmaps(family_name, bitmaps.into_iter())).collect();
    let mut every_codec: Vec<Box<dyn BitmapCodec>> = vec![Box::new(TesseraCodec::new()), Box::new(G4Codec::new()), Box::new(JbigCodec::new())];
    every_codec.extend(ZSTD_LEVELS.map(|zstd_level| Box::new(ZstdCodec::new(zstd_level)) as Box<dyn BitmapCodec>));

    // One pass of each codec over one bitmap first, so the first timed one
    // pays for nothing extra.
    let (bitmap, bitmap_rows) = &named_families[0].bitmaps[0];
    for warm_up_codec in every_codec.iter_mut() {
        warm_up_codec.encode_bitmap(bitmap, bitmap_rows);
        warm_up_codec.decode_last_encoding();
    }

    let mut report = Report::new("external_benchmarks", "cargo run --release --manifest-path external_benchmarks/Cargo.toml");
    report.add_note(format!("{per_generator} bitmaps a generator"));
    let mut overall_totals = vec![CodecTotals::default(); every_codec.len()];
    for named_family in &named_families {
        let totals_by_codec: Vec<CodecTotals> = every_codec.iter_mut().map(|each_codec| time_codec_on_family(each_codec.as_mut(), named_family)).collect();
        add_table(&mut report, &named_family.family_name, &every_codec, &totals_by_codec);
        for (overall_sum, family_totals) in overall_totals.iter_mut().zip(&totals_by_codec) {
            *overall_sum = overall_sum.plus_totals(*family_totals);
        }
    }
    add_table(&mut report, "all", &every_codec, &overall_totals);
    transient_data::publish_report(report);
}

/// Encodes and decodes every bitmap of `named_family` with `timed_codec`, checking
/// each comes back whole, and totals the sizes and times.
fn time_codec_on_family(timed_codec: &mut dyn BitmapCodec, named_family: &NamedFamily) -> CodecTotals {
    let mut codec_totals = CodecTotals::default();
    for (bitmap, bitmap_rows) in &named_family.bitmaps {
        let encode_start = Instant::now();
        timed_codec.encode_bitmap(bitmap, bitmap_rows);
        codec_totals.encode_seconds += encode_start.elapsed().as_secs_f64();
        codec_totals.total_bits += timed_codec.encoded_bits();
        let decode_start = Instant::now();
        timed_codec.decode_last_encoding();
        codec_totals.decode_seconds += decode_start.elapsed().as_secs_f64();
        assert!(timed_codec.decoded_matches(bitmap_rows), "{} did not round trip a bitmap of {}", timed_codec.codec_name(), named_family.family_name);
        codec_totals.bitmap_count += 1;
    }
    codec_totals
}

/// Adds one family's table to `report`: a row a codec, its mean bits
/// (and as a share of raw), encode and decode microseconds a bitmap.
fn add_table(report: &mut Report, table_name: &str, every_codec: &[Box<dyn BitmapCodec>], totals_by_codec: &[CodecTotals]) {
    let mut table = Table::new(&["codec", "bits\na bitmap", "of the\nraw cells", "encode us\na bitmap", "decode us\na bitmap"]);
    for (each_codec, codec_totals) in every_codec.iter().zip(totals_by_codec) {
        let count = codec_totals.bitmap_count as f64;
        let mean_bits = codec_totals.total_bits as f64 / count;
        table.add_row(&[
            each_codec.codec_name().to_string(),
            format!("{mean_bits:.0}"),
            format!("{:.1}%", 100.0 * mean_bits / RAW_CELLS as f64),
            format!("{:.1}", codec_totals.encode_seconds / count * 1e6),
            format!("{:.1}", codec_totals.decode_seconds / count * 1e6),
        ]);
    }
    report.add_table(format!("{table_name}, {} bitmaps", totals_by_codec[0].bitmap_count), table);
}
