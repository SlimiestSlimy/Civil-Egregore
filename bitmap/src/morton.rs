//! Morton (Z) order: a square plane's cells numbered by interleaving
//! their coordinates' bits, `x` in the even ones (`docs/bitmap.md`,
//! "Morton order").

/// Every byte with its bits spread to every other bit: bit `i` to bit
/// `2i`. A table, since this is asked of every coordinate read.
const SPREAD_BYTE_TABLE: [u16; 256] = {
    let mut table = [0u16; 256];
    let mut byte_value = 0;
    while byte_value < 256 {
        let mut spread_bits = byte_value;
        spread_bits = (spread_bits | spread_bits << 4) & 0x0f0f;
        spread_bits = (spread_bits | spread_bits << 2) & 0x3333;
        spread_bits = (spread_bits | spread_bits << 1) & 0x5555;
        table[byte_value] = spread_bits as u16;
        byte_value += 1;
    }
    table
};

/// The Morton index of `(x, y)`.
pub const fn morton_index(x: u8, y: u8) -> usize {
    SPREAD_BYTE_TABLE[x as usize] as usize | (SPREAD_BYTE_TABLE[y as usize] as usize) << 1
}

/// The even bits of `cell_morton_index` gathered into the low byte: the inverse of
/// [`SPREAD_BYTE_TABLE`], one coordinate of a Morton index.
const fn gather_even_bits(cell_morton_index: usize) -> u8 {
    let mut gathered_bits = cell_morton_index & 0x5555;
    gathered_bits = (gathered_bits | gathered_bits >> 1) & 0x3333;
    gathered_bits = (gathered_bits | gathered_bits >> 2) & 0x0f0f;
    gathered_bits = (gathered_bits | gathered_bits >> 4) & 0x00ff;
    gathered_bits as u8
}

/// The `(x, y)` whose Morton index is `cell_morton_index`.
pub const fn morton_coordinates(cell_morton_index: usize) -> (u8, u8) {
    (gather_even_bits(cell_morton_index), gather_even_bits(cell_morton_index >> 1))
}
