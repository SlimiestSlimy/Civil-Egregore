//! Morton (Z) order: a square plane's cells numbered by interleaving
//! their coordinates' bits, `x` in the even ones (`docs/bitmap.md`,
//! "Morton order").

/// Every byte with its bits spread to every other bit: bit `i` to bit
/// `2i`. A table, since this is asked of every coordinate read.
const SPREAD: [u16; 256] = {
    let mut table = [0u16; 256];
    let mut byte = 0;
    while byte < 256 {
        let mut spread = byte;
        spread = (spread | spread << 4) & 0x0f0f;
        spread = (spread | spread << 2) & 0x3333;
        spread = (spread | spread << 1) & 0x5555;
        table[byte] = spread as u16;
        byte += 1;
    }
    table
};

/// The Morton index of `(x, y)`.
pub const fn morton_index(x: u8, y: u8) -> usize {
    SPREAD[x as usize] as usize | (SPREAD[y as usize] as usize) << 1
}

/// The even bits of `index` gathered into the low byte: the inverse of
/// [`SPREAD`], one coordinate of a Morton index.
const fn compact(index: usize) -> u8 {
    let mut gathered = index & 0x5555;
    gathered = (gathered | gathered >> 1) & 0x3333;
    gathered = (gathered | gathered >> 2) & 0x0f0f;
    gathered = (gathered | gathered >> 4) & 0x00ff;
    gathered as u8
}

/// The `(x, y)` whose Morton index is `index`.
pub const fn morton_coordinates(index: usize) -> (u8, u8) {
    (compact(index), compact(index >> 1))
}
