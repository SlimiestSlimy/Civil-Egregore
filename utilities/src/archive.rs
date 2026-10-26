//! Zip archives, written ([`write_archive`]) and read ([`read_archive`])
//! whole in memory: a mod's package. Each entry is stored as it is,
//! never compressed, so an archive is its entries' bytes one after
//! another, each behind a small header, and a directory of them at the
//! end -- what every zip tool opens. Written the same every time: one
//! date, the order given, nothing else that varies.
//!
//! Every number in a zip is little-endian.

/// A local header's mark: an entry's bytes follow it.
const LOCAL_HEADER_MARK: u32 = 0x0403_4b50;

/// A directory entry's mark.
const DIRECTORY_ENTRY_MARK: u32 = 0x0201_4b50;

/// The end record's mark: where the directory is, and how long.
const END_RECORD_MARK: u32 = 0x0605_4b50;

/// A local header's length, its name aside.
const LOCAL_HEADER_LENGTH: usize = 30;

/// A directory entry's length, its name aside.
const DIRECTORY_ENTRY_LENGTH: usize = 46;

/// The end record's length, with no comment.
const END_RECORD_LENGTH: usize = 22;

/// The zip version an entry needs read, 1.0: stored, nothing else.
const VERSION_NEEDED: u16 = 10;

/// The zip version the archive is written by, 2.0.
const VERSION_MADE_BY: u16 = 20;

/// The flag of an entry's name written in UTF-8.
const NAME_IN_UTF8: u16 = 1 << 11;

/// The flag of an encrypted entry.
const ENCRYPTED: u16 = 1;

/// The one date every entry has, 1980-01-01 -- the earliest a zip says
/// -- as a zip writes a date: years since 1980, month, day.
const THE_DATE: u16 = 1 << 5 | 1;

/// The archive of `entries`, each its path -- `/` between its folders --
/// and its bytes, stored as they are, in the order given.
pub fn write_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    assert!(entries.len() < usize::from(u16::MAX), "a zip of fewer than 65535 entries");
    let (mut archive, mut directory) = (Vec::new(), Vec::new());
    for &(path, bytes) in entries {
        assert!(bytes.len() < u32::MAX as usize && archive.len() < u32::MAX as usize, "a zip under 4 GiB");
        let header_place = archive.len() as u32;
        let (crc, size, path_length) = (crc32(bytes), bytes.len() as u32, path.len() as u16);
        put_u32(&mut archive, LOCAL_HEADER_MARK);
        for half_word in [VERSION_NEEDED, NAME_IN_UTF8, 0, 0, THE_DATE] {
            put_u16(&mut archive, half_word);
        }
        for word in [crc, size, size] {
            put_u32(&mut archive, word);
        }
        put_u16(&mut archive, path_length);
        put_u16(&mut archive, 0);
        archive.extend_from_slice(path.as_bytes());
        archive.extend_from_slice(bytes);
        put_u32(&mut directory, DIRECTORY_ENTRY_MARK);
        for half_word in [VERSION_MADE_BY, VERSION_NEEDED, NAME_IN_UTF8, 0, 0, THE_DATE] {
            put_u16(&mut directory, half_word);
        }
        for word in [crc, size, size] {
            put_u32(&mut directory, word);
        }
        for half_word in [path_length, 0, 0, 0, 0] {
            put_u16(&mut directory, half_word);
        }
        put_u32(&mut directory, 0);
        put_u32(&mut directory, header_place);
        directory.extend_from_slice(path.as_bytes());
    }
    let directory_place = archive.len() as u32;
    archive.extend_from_slice(&directory);
    put_u32(&mut archive, END_RECORD_MARK);
    for half_word in [0, 0, entries.len() as u16, entries.len() as u16] {
        put_u16(&mut archive, half_word);
    }
    put_u32(&mut archive, directory.len() as u32);
    put_u32(&mut archive, directory_place);
    put_u16(&mut archive, 0);
    archive
}

/// The entries of the archive `archive`, each its path and its bytes,
/// borrowed from it, in the directory's order: or what is wrong with it.
/// Stored entries alone, each checked against its CRC; no path twice,
/// none absolute, none through `..`.
pub fn read_archive(archive: &[u8]) -> Result<Vec<(&str, &[u8])>, String> {
    let end_place = (0..=archive.len().saturating_sub(END_RECORD_LENGTH)).rev().take(usize::from(u16::MAX) + 1).find(|&place| u32_at(archive, place) == Ok(END_RECORD_MARK)).ok_or("no zip: no end record")?;
    let (disk, directory_disk) = (u16_at(archive, end_place + 4)?, u16_at(archive, end_place + 6)?);
    let (entries_here, entry_count) = (u16_at(archive, end_place + 8)?, u16_at(archive, end_place + 10)?);
    let directory_place = u32_at(archive, end_place + 16)? as usize;
    if disk != 0 || directory_disk != 0 || entries_here != entry_count || entry_count == u16::MAX || directory_place == u32::MAX as usize {
        return Err("a zip in many parts, or a zip64: neither read".to_string());
    }
    let mut entries: Vec<(&str, &[u8])> = Vec::new();
    let mut place = directory_place;
    for _ in 0..entry_count {
        if u32_at(archive, place)? != DIRECTORY_ENTRY_MARK {
            return Err(format!("no directory entry at byte {place}"));
        }
        let (flags, method) = (u16_at(archive, place + 8)?, u16_at(archive, place + 10)?);
        let (crc, size, unpacked_size) = (u32_at(archive, place + 16)?, u32_at(archive, place + 20)? as usize, u32_at(archive, place + 24)? as usize);
        let (path_length, extra_length, comment_length) = (usize::from(u16_at(archive, place + 28)?), usize::from(u16_at(archive, place + 30)?), usize::from(u16_at(archive, place + 32)?));
        let header_place = u32_at(archive, place + 42)? as usize;
        let path = std::str::from_utf8(bytes_at(archive, place + DIRECTORY_ENTRY_LENGTH, path_length)?).map_err(|_| format!("an entry's path at byte {place}: not UTF-8"))?;
        place += DIRECTORY_ENTRY_LENGTH + path_length + extra_length + comment_length;
        if method != 0 || flags & ENCRYPTED != 0 || size != unpacked_size {
            return Err(format!("{path}: compressed or encrypted, stored entries alone read"));
        }
        if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
            return Err(format!("`{path}`: no path of a file inside the archive"));
        }
        if entries.iter().any(|&(read_before, _)| read_before == path) {
            return Err(format!("{path}: twice"));
        }
        if u32_at(archive, header_place)? != LOCAL_HEADER_MARK {
            return Err(format!("{path}: no header at byte {header_place}"));
        }
        let bytes_place = header_place + LOCAL_HEADER_LENGTH + usize::from(u16_at(archive, header_place + 26)?) + usize::from(u16_at(archive, header_place + 28)?);
        let bytes = bytes_at(archive, bytes_place, size)?;
        if crc32(bytes) != crc {
            return Err(format!("{path}: damaged, its CRC not its bytes'"));
        }
        entries.push((path, bytes));
    }
    Ok(entries)
}

/// `half_word` put at the end of `bytes`.
fn put_u16(bytes: &mut Vec<u8>, half_word: u16) {
    bytes.extend_from_slice(&half_word.to_le_bytes());
}

/// `word` put at the end of `bytes`.
fn put_u32(bytes: &mut Vec<u8>, word: u32) {
    bytes.extend_from_slice(&word.to_le_bytes());
}

/// The `length` bytes of `archive` at `place`: or that it ends before.
fn bytes_at(archive: &[u8], place: usize, length: usize) -> Result<&[u8], String> {
    archive.get(place..place.saturating_add(length)).ok_or_else(|| format!("the zip ends before byte {}", place.saturating_add(length)))
}

/// The half word of `archive` at `place`.
fn u16_at(archive: &[u8], place: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(bytes_at(archive, place, 2)?.try_into().expect("two bytes")))
}

/// The word of `archive` at `place`.
fn u32_at(archive: &[u8], place: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(bytes_at(archive, place, 4)?.try_into().expect("four bytes")))
}

/// The CRC-32 of each byte alone, by the zip's polynomial.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut byte = 0;
    while byte < 256 {
        let mut crc = byte as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 { crc >> 1 ^ 0xEDB8_8320 } else { crc >> 1 };
            bit += 1;
        }
        table[byte] = crc;
        byte += 1;
    }
    table
};

/// The CRC-32 of `bytes`, as a zip says it.
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0, |crc, &byte| crc >> 8 ^ CRC_TABLE[usize::from(crc as u8 ^ byte)])
}
