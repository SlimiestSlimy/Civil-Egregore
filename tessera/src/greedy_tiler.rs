//! The greedy tiler: the greedy tiling, top down, places tiles by one
//! rule; the complex tiling, bottom up, counts every tile and makes
//! complex tiles where they take fewer bits. Both are thrown away when
//! the binary count tree wins. `docs/tessera.md`, "The greedy tiling"
//! and "The complex tiling".
//!
//! Function by function: `docs/reference.md`, "`greedy_tiler.rs`".

use crate::payload_writer::cell_list_least_bits;
use crate::quadtree_writer::{node_bits, raw_resolution_fits, START_LEVEL_WIDTH};
use crate::tree::{start_level, Node, Tree, BOUND_AT_THE_TOP};
use crate::last_pass::ResidualPricing;
use crate::patterns::{homogeneous_value_of, PatternPyramid};
use crate::set_cells_before_each_word::SetCellsBeforeEachWord;
use crate::tile::{cells_in_tile, copy_offset, tiles_in_level, Tile, CELL_LEVEL, COPY_DIRECTIONS, FLOOR_LEVEL};
use bitmap::Bitmap;

/// A copy naming its children costs about 10 bits before them. A
/// homogeneous child it copies is cheap anyway; any other needs a copy
/// or a subtree of its own, 5 bits or more. So it must copy at least
/// this many children that are not the value bound above...
pub const MIN_COPIED_CHILDREN: u32 = 3;
/// ...or at least this many that are not homogeneous. Either half alone
/// measured worse.
pub const MIN_COPIED_NON_HOMOGENEOUS_CHILDREN: u32 = 2;
/// A flipping divide costs 7 bits before its named children; each child
/// it binds would otherwise be a tile of its own, 4 bits or more. So it
/// must bind at least this many.
pub const MIN_FLIPPED_CHILDREN: u32 = 2;

/// Every child named.
const ALL_CHILDREN: u8 = 0b1111;
/// A bind of the whole tile: a complex tile of its own size.
const PLAIN_TILE: Node = Node::ComplexTile { size_offset: 0 };

/// A tile the walk visits, and what it knows of it on the way down.
#[derive(Clone, Copy)]
struct TileVisit {
    /// The tile.
    tile: Tile,
    /// Its pattern number.
    pattern_number: u16,
    /// The value bound above it.
    bound_above: bool,
    /// Whether it is a child of a divide.
    child_of_divide: bool,
}

/// What the complex tiling knows of a tile once it is through everything
/// under it.
struct CountedSubtree {
    /// The fewest bits it and everything under it take.
    fewest_bits: u64,
    /// The one size every cell under it is bound at, if any.
    bound_size: Option<u8>,
}

/// The greedy tiling, the top-down pass: places tiles in `tree`, whatever
/// it held, from the whole bitmap down -- each tile's node, as placed,
/// and every child it names visited in turn; a child it does not name,
/// and one a divide leaves to the binding above, [`Node::NoNode`].
pub fn greedy_tiling(pattern_pyramid: &PatternPyramid, tree: &mut Tree) {
    let whole_bitmap_tile = Tile::WHOLE_BITMAP;
    place_subtree(pattern_pyramid, tree, TileVisit { tile: whole_bitmap_tile, pattern_number: pattern_pyramid.pattern_number(whole_bitmap_tile), bound_above: BOUND_AT_THE_TOP, child_of_divide: false });
}

/// Places at the visited tile, or divides it, then visits every child it
/// names.
fn place_subtree(pattern_pyramid: &PatternPyramid, tree: &mut Tree, tile_visit: TileVisit) {
    let tile = tile_visit.tile;
    let children_numbers = (tile.tile_level < FLOOR_LEVEL).then(|| pattern_pyramid.children_numbers(tile));
    let unplaced_node = if tile.tile_level == FLOOR_LEVEL { Node::ResidualFloorTile } else { Node::PlainDivide };
    let (node, named_children) = place(pattern_pyramid, tile_visit, children_numbers).unwrap_or((unplaced_node, ALL_CHILDREN));
    let left_to_binding_above =
        tile_visit.child_of_divide && node == PLAIN_TILE && homogeneous_value_of(tile_visit.pattern_number) == Some(tile_visit.bound_above);
    tree.set(tile, if left_to_binding_above { Node::NoNode } else { node });
    let Some(child_pattern_numbers) = children_numbers else { return };
    let bound_inside = tile_visit.bound_above != (node == Node::FlippingDivide);
    for (child_index, child_tile) in tile.child_tiles().into_iter().enumerate() {
        if named_children >> child_index & 1 == 0 {
            tree.set(child_tile, Node::NoNode);
        } else {
            let child_visit = TileVisit { tile: child_tile, pattern_number: child_pattern_numbers[child_index], bound_above: bound_inside, child_of_divide: node == Node::PlainDivide };
            place_subtree(pattern_pyramid, tree, child_visit);
        }
    }
}

/// The complex tiling, bottom up over the placed `tree`: the tree's bits,
/// residual floor tiles at their prices in `residual_pricing`, and its start level.
pub fn complex_tiling(bitmap: &Bitmap, set_cells_before_each_word: &SetCellsBeforeEachWord, tree: &mut Tree, residual_pricing: &mut ResidualPricing) -> (u64, u8) {
    residual_pricing.clear();
    let fewest_bits = count_subtree(bitmap, set_cells_before_each_word, tree, residual_pricing, Tile::WHOLE_BITMAP).fewest_bits;
    // The divides above the start level are never written.
    let start_level = start_level(tree);
    let trunk_bits: u64 = (0..start_level).map(|tile_level| tiles_in_level(tile_level) as u64 * node_bits(tree, bitmap, Tile { tile_level, x: 0, y: 0 }, Node::PlainDivide)).sum();
    (START_LEVEL_WIDTH as u64 + fewest_bits - trunk_bits, start_level)
}

/// Counts `tile`'s node, placed, after every child that is a node, and
/// makes it one complex tile if that takes fewer bits.
fn count_subtree(bitmap: &Bitmap, set_cells_before_each_word: &SetCellsBeforeEachWord, tree: &mut Tree, residual_pricing: &mut ResidualPricing, tile: Tile) -> CountedSubtree {
    let node = tree.get(tile);
    let (mut fewest_bits, mut bound_size) = (0, None);
    if node == Node::ResidualFloorTile {
        fewest_bits = residual_pricing.price_residual_floor_tile(bitmap, tile);
        bound_size = all_2x2s_homogeneous(bitmap, tile).then_some(FLOOR_LEVEL + 1);
    } else if node == PLAIN_TILE {
        bound_size = Some(tile.tile_level);
    } else if node.has_children() {
        let mut child_bound_sizes = [None; 4];
        for (child_index, child_tile) in tile.child_tiles().into_iter().enumerate() {
            if tree.get(child_tile) == Node::NoNode {
                // A divide's child left to the binding above is bound whole.
                child_bound_sizes[child_index] = Some(child_tile.tile_level);
            } else {
                let counted_child = count_subtree(bitmap, set_cells_before_each_word, tree, residual_pricing, child_tile);
                fewest_bits += counted_child.fewest_bits;
                child_bound_sizes[child_index] = counted_child.bound_size;
            }
        }
        if node == Node::PlainDivide && child_bound_sizes.iter().all(|&child_bound_size| child_bound_size == child_bound_sizes[0]) {
            bound_size = child_bound_sizes[0];
        }
    }
    fewest_bits += node_bits(tree, bitmap, tile, node);
    if (node == Node::PlainDivide || node == Node::ResidualFloorTile)
        && let Some((complex_tile, complex_tile_bits)) = best_complex_tile(bitmap, set_cells_before_each_word, tree, tile, bound_size, fewest_bits)
    {
        tree.set(tile, complex_tile);
        fewest_bits = complex_tile_bits;
    }
    CountedSubtree { fewest_bits, bound_size }
}

/// `tile`'s cheapest complex tile, if one takes fewer than `to_beat`
/// bits: at `bound_size`, then raw, then as a cell list, the first of
/// the fewest bits kept.
fn best_complex_tile(bitmap: &Bitmap, set_cells_before_each_word: &SetCellsBeforeEachWord, tree: &Tree, tile: Tile, bound_size: Option<u8>, to_beat: u64) -> Option<(Node, u64)> {
    let mut best_so_far = (None, to_beat);
    let consider_node = |best_found: &mut (Option<Node>, u64), node: Node, candidate_bits: u64| {
        if candidate_bits < best_found.1 {
            *best_found = (Some(node), candidate_bits);
        }
    };
    if let Some(bound_level) = bound_size {
        let node = Node::ComplexTile { size_offset: bound_level - tile.tile_level };
        consider_node(&mut best_so_far, node, node_bits(tree, bitmap, tile, node));
    }
    if raw_resolution_fits(tile.tile_level) {
        let raw_node = Node::ComplexTile { size_offset: CELL_LEVEL - tile.tile_level };
        let raw_bits = node_bits(tree, bitmap, tile, raw_node);
        consider_node(&mut best_so_far, raw_node, raw_bits);
        // A cell list's bits before its cells are a raw complex
        // tile's; its cells are counted only if the fewest they could
        // take beat everything so far.
        let header = raw_bits - cells_in_tile(tile.tile_level) as u64;
        if header + cell_list_least_bits(tile.tile_level, set_cells_before_each_word.in_tile(tile)) < best_so_far.1 {
            consider_node(&mut best_so_far, Node::CellList, node_bits(tree, bitmap, tile, Node::CellList));
        }
    }
    best_so_far.0.map(|node| (node, best_so_far.1))
}

/// What the rule places at the visited tile, if anything, and the
/// children it names, bit `i` for child `i`; `children_numbers` its
/// children's pattern numbers, for a tile coarser than 4x4, which
/// may name its children.
fn place(pattern_pyramid: &PatternPyramid, tile_visit: TileVisit, children_numbers: Option<[u16; 4]>) -> Option<(Node, u8)> {
    if homogeneous_value_of(tile_visit.pattern_number).is_some() {
        return Some((PLAIN_TILE, 0));
    }
    if let Some((is_far_copy, copy_direction)) = pattern_pyramid.copy_source(tile_visit.tile, tile_visit.pattern_number) {
        return Some((Node::CopiedTile { is_far_copy, copy_direction, names_children: false }, 0));
    }
    let child_pattern_numbers = children_numbers?;
    copy_naming_children(pattern_pyramid, tile_visit, child_pattern_numbers).or_else(|| flipping_divide(child_pattern_numbers, tile_visit.bound_above))
}

/// The copy of the visited tile copying the most children, naming the
/// rest, if it copies enough: the first on a tie.
fn copy_naming_children(pattern_pyramid: &PatternPyramid, tile_visit: TileVisit, child_pattern_numbers: [u16; 4]) -> Option<(Node, u8)> {
    let values = child_pattern_numbers.map(homogeneous_value_of);
    let child_level = tile_visit.tile.tile_level + 1;
    // Only a child whose pattern another tile holds can be copied.
    let can_match: [bool; 4] = std::array::from_fn(|child_index| pattern_pyramid.pattern_repeats(child_level, child_pattern_numbers[child_index]));
    let adds_copied: [u32; 4] = std::array::from_fn(|child_index| (can_match[child_index] && values[child_index] != Some(tile_visit.bound_above)) as u32);
    let adds_non_homogeneous: [u32; 4] = std::array::from_fn(|child_index| (can_match[child_index] && values[child_index].is_none()) as u32);
    let worth_it = |copied_total: u32, non_homogeneous: u32| copied_total >= MIN_COPIED_CHILDREN || non_homogeneous >= MIN_COPIED_NON_HOMOGENEOUS_CHILDREN;
    if !worth_it(adds_copied.iter().sum(), adds_non_homogeneous.iter().sum()) {
        return None;
    }
    let mut best_copy: Option<(u32, Node, u8)> = None;
    for is_far_copy in [false, true] {
        for copy_direction in 0..COPY_DIRECTIONS {
            let Some(source_tile) = tile_visit.tile.offset_by(copy_offset(is_far_copy, copy_direction)) else { continue };
            let source_numbers = pattern_pyramid.children_numbers(source_tile);
            let (mut named_children, mut copied_count, mut non_homogeneous) = (0u8, 0, 0);
            for child_index in 0..child_pattern_numbers.len() {
                if can_match[child_index] && source_numbers[child_index] == child_pattern_numbers[child_index] {
                    copied_count += adds_copied[child_index];
                    non_homogeneous += adds_non_homogeneous[child_index];
                } else {
                    named_children |= 1 << child_index;
                }
            }
            if worth_it(copied_count, non_homogeneous) && best_copy.is_none_or(|(most_copied, ..)| copied_count > most_copied) {
                best_copy = Some((copied_count, Node::CopiedTile { is_far_copy, copy_direction, names_children: true }, named_children));
            }
        }
    }
    best_copy.map(|(_, node, named_children)| (node, named_children))
}

/// A flipping divide of a tile whose children's pattern numbers are
/// `child_pattern_numbers`, naming the children not homogeneous with the value not
/// bound above, if it binds at least [`MIN_FLIPPED_CHILDREN`].
fn flipping_divide(child_pattern_numbers: [u16; 4], bound_above: bool) -> Option<(Node, u8)> {
    let named_children = (0..child_pattern_numbers.len()).filter(|&child_index| homogeneous_value_of(child_pattern_numbers[child_index]) != Some(!bound_above)).fold(0u8, |named_so_far, child_index| named_so_far | 1 << child_index);
    (child_pattern_numbers.len() as u32 - named_children.count_ones() >= MIN_FLIPPED_CHILDREN).then_some((Node::FlippingDivide, named_children))
}

/// Whether each of the 4x4 `tile`'s 2x2s is homogeneous: its cells, one
/// run of the bitmap whose four quarters are its 2x2s.
fn all_2x2s_homogeneous(bitmap: &Bitmap, tile: Tile) -> bool {
    /// A 2x2's cells, all set.
    const QUARTER_ALL_SET: u64 = (1 << cells_in_tile(FLOOR_LEVEL + 1)) - 1;
    let cells = bitmap.morton_run(tile.first_cell(), cells_in_tile(FLOOR_LEVEL));
    (0..4).all(|quarter_index| matches!(cells >> (quarter_index * cells_in_tile(FLOOR_LEVEL + 1)) & QUARTER_ALL_SET, 0 | QUARTER_ALL_SET))
}
