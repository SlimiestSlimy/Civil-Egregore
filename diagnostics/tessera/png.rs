//! A bitmap as a PNG image, set cells black, two pixels a cell: a PNG is
//! a zlib stream, so this writes one uncompressed, with no library.

use bitmap::Bitmap;

/// Each cell is this many pixels square.
const PIXELS_A_CELL: usize = 2;
/// The bitmap's side, in cells.
const BITMAP_SIDE_CELLS: usize = 256;
/// A set cell's grey level.
const BLACK_GREY_LEVEL: u8 = 0;
/// A clear cell's grey level.
const WHITE_GREY_LEVEL: u8 = 255;

/// The PNG format's fixed values: the file signature...
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
/// ...8 bits a pixel...
const BIT_DEPTH: u8 = 8;
/// ...the greyscale colour type...
const GREYSCALE_COLOUR_TYPE: u8 = 0;
/// ...deflate, the only compression method...
const DEFLATE_COMPRESSION: u8 = 0;
/// ...adaptive filtering, the only filter method...
const ADAPTIVE_FILTERING: u8 = 0;
/// ...no interlacing...
const NO_INTERLACE: u8 = 0;
/// ...and each row's filter type: none.
const NO_FILTER: u8 = 0;
/// CRC-32's reversed polynomial, as PNG uses it.
const CRC_POLYNOMIAL: u32 = 0xEDB8_8320;
/// A zlib header: deflate with a 32 KiB window, no dictionary, lowest
/// compression level -- stored blocks.
const ZLIB_HEADER: [u8; 2] = [0x78, 0x01];
/// Adler-32's modulus, the largest prime under 2^16.
const ADLER_MODULUS: u32 = 65521;
/// The most one stored deflate block holds.
const STORED_BLOCK: usize = u16::MAX as usize;

/// The CRC-32 PNG chunks carry.
fn chunk_crc32(checked_bytes: &[u8]) -> u32 {
    let mut running_crc = u32::MAX;
    for &next_byte in checked_bytes {
        running_crc ^= next_byte as u32;
        for _ in 0..8 {
            running_crc = if running_crc & 1 == 1 { (running_crc >> 1) ^ CRC_POLYNOMIAL } else { running_crc >> 1 };
        }
    }
    !running_crc
}

/// Adler-32, the checksum ending a zlib stream.
fn zlib_adler32(checked_bytes: &[u8]) -> u32 {
    let (mut sum_of_bytes, mut sum_of_sums) = (1u32, 0u32);
    for &next_byte in checked_bytes {
        sum_of_bytes = (sum_of_bytes + next_byte as u32) % ADLER_MODULUS;
        sum_of_sums = (sum_of_sums + sum_of_bytes) % ADLER_MODULUS;
    }
    (sum_of_sums << 16) | sum_of_bytes
}

/// Appends one PNG chunk to `png_bytes`: its length, `chunk_kind`, `chunk_data`, and the
/// CRC of the last two.
fn append_png_chunk(png_bytes: &mut Vec<u8>, chunk_kind: &[u8; 4], chunk_data: &[u8]) {
    png_bytes.extend((chunk_data.len() as u32).to_be_bytes());
    let mut kind_and_data = chunk_kind.to_vec();
    kind_and_data.extend(chunk_data);
    png_bytes.extend(&kind_and_data);
    png_bytes.extend(chunk_crc32(&kind_and_data).to_be_bytes());
}

/// A zlib stream of stored (uncompressed) blocks.
fn stored_zlib(uncompressed_data: &[u8]) -> Vec<u8> {
    let mut zlib_bytes = ZLIB_HEADER.to_vec();
    let stored_blocks: Vec<&[u8]> = uncompressed_data.chunks(STORED_BLOCK).collect();
    for (block_index, stored_block) in stored_blocks.iter().enumerate() {
        zlib_bytes.push(u8::from(block_index + 1 == stored_blocks.len()));
        zlib_bytes.extend((stored_block.len() as u16).to_le_bytes());
        zlib_bytes.extend((!(stored_block.len() as u16)).to_le_bytes());
        zlib_bytes.extend(*stored_block);
    }
    zlib_bytes.extend(zlib_adler32(uncompressed_data).to_be_bytes());
    zlib_bytes
}

/// `bitmap` as an 8-bit greyscale PNG.
pub fn bitmap_png(bitmap: &Bitmap) -> Vec<u8> {
    let side_in_pixels = BITMAP_SIDE_CELLS * PIXELS_A_CELL;
    let mut filtered_rows = Vec::with_capacity(side_in_pixels * (side_in_pixels + 1));
    for y in 0..side_in_pixels {
        filtered_rows.push(NO_FILTER);
        for x in 0..side_in_pixels {
            let cell_is_set = bitmap.get((x / PIXELS_A_CELL) as u8, (y / PIXELS_A_CELL) as u8);
            filtered_rows.push(if cell_is_set { BLACK_GREY_LEVEL } else { WHITE_GREY_LEVEL });
        }
    }
    let mut png_bytes = PNG_SIGNATURE.to_vec();
    let mut header = (side_in_pixels as u32).to_be_bytes().to_vec();
    header.extend((side_in_pixels as u32).to_be_bytes());
    header.extend([BIT_DEPTH, GREYSCALE_COLOUR_TYPE, DEFLATE_COMPRESSION, ADAPTIVE_FILTERING, NO_INTERLACE]);
    append_png_chunk(&mut png_bytes, b"IHDR", &header);
    append_png_chunk(&mut png_bytes, b"IDAT", &stored_zlib(&filtered_rows));
    append_png_chunk(&mut png_bytes, b"IEND", &[]);
    png_bytes
}
