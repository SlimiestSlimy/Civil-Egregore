//! Zip archives: written and read back the same, the CRC the zip's, and
//! what is no archive of ours refused.
//!
//! `cargo test`

use utilities::archive::{crc32, read_archive, write_archive};
use utilities::rng::RandomSource;

/// The CRC of the nine digits is the one every CRC-32 is checked by.
#[test]
fn the_crc_is_the_zips() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

/// Entries of drawn lengths and bytes, an empty one and a nested
/// archive among them, read back as written, in order; written twice,
/// the same bytes.
#[test]
fn entries_read_back_as_written() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random = RandomSource::new(seed);
    let drawn: Vec<Vec<u8>> = (0..5).map(|_| (0..random.next_draw() % 4096).map(|_| random.next_draw() as u8).collect()).collect();
    let nested = write_archive(&[("manifest.cdnf", &drawn[0])]);
    let entries: Vec<(&str, &[u8])> = vec![("manifest.cdnf", &drawn[1]), ("libmeadow.so", &drawn[2]), ("empty", &[]), ("dependencies/basegame.zip", &nested), ("dependencies/more/deeper", &drawn[3])];
    let archive = write_archive(&entries);
    assert_eq!(read_archive(&archive).expect("an archive"), entries);
    assert_eq!(write_archive(&entries), archive, "written the same every time");
    let (_, nested_read) = read_archive(&archive).expect("an archive")[3];
    assert_eq!(read_archive(nested_read).expect("a nested archive"), [("manifest.cdnf", &drawn[0][..])]);
}

/// A damaged byte, a compressed entry, a path out of the archive, a
/// path twice and no archive at all, each refused.
#[test]
fn what_is_no_archive_of_ours_refused() {
    let archive = write_archive(&[("manifest.cdnf", b"bytes of the manifest")]);
    let mut damaged = archive.clone();
    damaged[30 + "manifest.cdnf".len()] ^= 1;
    assert!(read_archive(&damaged).expect_err("damaged").contains("damaged"));
    let mut compressed = archive.clone();
    let directory_place = archive.len() - 22 - 46 - "manifest.cdnf".len();
    compressed[directory_place + 10] = 8;
    assert!(read_archive(&compressed).expect_err("compressed").contains("stored entries alone"));
    for path in ["/etc/passwd", "../outside", "a//b", "a\\b", ""] {
        let refusal = read_archive(&write_archive(&[(path, b"")])).expect_err("a path out of the archive");
        assert!(refusal.contains("no path of a file"), "{path}: {refusal}");
    }
    assert!(read_archive(&write_archive(&[("twice", b""), ("twice", b"")])).expect_err("twice").contains("twice"));
    assert!(read_archive(b"no zip at all").expect_err("no zip").contains("no end record"));
}
