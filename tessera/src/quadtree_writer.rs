//! The quadtree writer, its reader beside it: every node's bits, and the
//! walks writing and reading the tree that gather the last pass's floor tile
//! plan. Counting a node's bits is writing it to a [`Counter`].
//! `docs/tessera.md`, "The quadtree grammar".
//!
//! Function by function: `docs/reference.md`, "`quadtree_writer.rs`".

use crate::bit_stream::{BitReader, BitStream, BitCounter, BitSink};
use crate::last_pass::FloorPlan;
use crate::payload_writer::{read_cell_list, read_payload, write_cell_list, write_payload};
use crate::tile::{Tile, CELL_LEVEL, CHILDREN_PER_TILE, COPY_DIRECTIONS, FLOOR_LEVEL};
use crate::tree::{Node, Tree, BOUND_AT_THE_TOP};
use bitmap::Bitmap;

/// Bits in every one-bit choice below.
pub const FLAG_WIDTH: u8 = 1;
/// A node's first bit: a leaf, a copy or a bind...
const LEAF_FLAG: u64 = 1;
/// ...or a divide -- at the 4x4 floor, a residual floor tile.
const DIVIDE_FLAG: u64 = 0;
/// After a leaf's first bit: a copy...
const COPY_FLAG: u64 = 0;
/// ...or a bind, a complex tile.
const BIND_FLAG: u64 = 1;
/// Whether a copy or a divide names its children: its child mask
/// follows. Every divide is 8x8 or coarser -- a 4x4 is a leaf or a
/// residual floor tile -- and so says it; a copy, down to 8x8 only: a 4x4's
/// children are finer than the tree.
const NAMES_CHILDREN: u64 = 1;
/// A divide naming its children flips the value bound above...
const BINDING_FLIPPED: u64 = 1;
/// A child mask's bit, a child in reading order: a node of its own,
/// following the parent's bits, or said by the parent.
const CHILD_IS_NODE: u64 = 1;
/// A bind's size offset starts with this for a plain tile, nothing
/// more, and else its size offset follows, in truncated binary over the
/// size offsets its level allows, finest first: the finest, 1x1 above
/// all, are what complex tiles are mostly made at.
const PLAIN_TILE: u64 = 0;
/// A complex tile of 1x1 resolution says its cells as a cell list,
/// else raw.
const CELL_LIST: u64 = 1;
/// Bits naming a copy's direction.
const DIRECTION_WIDTH: u8 = COPY_DIRECTIONS.trailing_zeros() as u8;
/// Bits naming the start level, the whole bitmap to the 4x4 floor.
pub const START_LEVEL_WIDTH: u8 = (u8::BITS - FLOOR_LEVEL.leading_zeros()) as u8;
/// The most bits one node takes, its payload aside: a copy naming its
/// children -- leaf, copy, far, direction, names children, a bit a
/// child.
pub const MOST_NODE_BITS: usize = (4 * FLAG_WIDTH + DIRECTION_WIDTH + CHILDREN_PER_TILE * FLAG_WIDTH) as usize;

/// Whether a copy at `tile_level` says whether it names its children.
const fn may_name_children(tile_level: u8) -> bool {
    tile_level < FLOOR_LEVEL
}

/// The largest size offset a complex tile at `tile_level` can have: down to
/// 2x2, or to 1x1 -- every cell raw, the escape for what nothing else
/// compresses -- where the truncated binary code has room for it.
const fn most_size_offset(tile_level: u8) -> u8 {
    let to_cells = CELL_LEVEL - tile_level;
    let width = u8::BITS - (to_cells - 1).leading_zeros();
    if (to_cells as u32) < 1 << width { to_cells } else { to_cells - 1 }
}

/// Whether a 1x1 resolution -- and so a cell list -- can be named at
/// `tile_level`.
pub const fn raw_resolution_fits(tile_level: u8) -> bool {
    most_size_offset(tile_level) == CELL_LEVEL - tile_level
}

/// Writes one flag.
#[inline]
fn push_flag(bit_sink: &mut impl BitSink, value: u64) {
    bit_sink.push_value(value, FLAG_WIDTH);
}

/// Reads one flag.
#[inline]
fn read_flag(stream_reader: &mut BitReader) -> u64 {
    stream_reader.value(FLAG_WIDTH)
}

/// The bits `node` takes at `tile` itself: [`write_node`], counted.
pub fn node_bits(tree: &Tree, bitmap: &Bitmap, tile: Tile, node: Node) -> u64 {
    let mut bit_counter = BitCounter::default();
    write_node(&mut bit_counter, tree, bitmap, tile, node);
    bit_counter.0
}

/// Writes `node`, at `tile`, whose children's nodes are in `tree`: its
/// own bits, child mask and payload -- everything but its children's
/// nodes.
pub fn write_node(bit_sink: &mut impl BitSink, tree: &Tree, bitmap: &Bitmap, tile: Tile, node: Node) {
    match node {
        Node::PlainDivide | Node::FlippingDivide => {
            push_flag(bit_sink, DIVIDE_FLAG);
            let names_children = node == Node::FlippingDivide || tree.child_elements(tile).contains(&Node::NoNode);
            push_flag(bit_sink, names_children as u64);
            if names_children {
                push_flag(bit_sink, (node == Node::FlippingDivide) as u64);
                write_child_mask(bit_sink, tree, tile);
            }
        }
        Node::CopiedTile { is_far_copy: far, copy_direction: direction, names_children } => {
            push_flag(bit_sink, LEAF_FLAG);
            push_flag(bit_sink, COPY_FLAG);
            push_flag(bit_sink, far as u64);
            bit_sink.push_value(direction as u64, DIRECTION_WIDTH);
            if may_name_children(tile.tile_level) {
                push_flag(bit_sink, names_children as u64);
            }
            if names_children {
                write_child_mask(bit_sink, tree, tile);
            }
        }
        Node::ComplexTile { size_offset } => {
            write_complex_tile_header(bit_sink, tile.tile_level, size_offset, false);
            write_payload(bit_sink, bitmap, tile, size_offset);
        }
        Node::CellList => {
            write_complex_tile_header(bit_sink, tile.tile_level, CELL_LEVEL - tile.tile_level, true);
            write_cell_list(bit_sink, bitmap, tile);
        }
        Node::ResidualFloorTile => push_flag(bit_sink, DIVIDE_FLAG),
        Node::NoNode => unreachable!("an absent node is never written"),
    }
}

/// A child mask: a bit a child, whether it is a node of its own.
fn write_child_mask(bit_sink: &mut impl BitSink, tree: &Tree, tile: Tile) {
    for child_node in tree.child_elements(tile) {
        push_flag(bit_sink, (child_node != Node::NoNode) as u64);
    }
}

/// Reads a child mask.
fn read_child_mask(stream_reader: &mut BitReader) -> [bool; 4] {
    [(); 4].map(|_| read_flag(stream_reader) == CHILD_IS_NODE)
}

/// A complex tile's bits before its payload, at `tile_level`, of
/// `size_offset`: leaf, bind, its size offset and, at 1x1 resolution,
/// whether it is a cell list.
fn write_complex_tile_header(bit_sink: &mut impl BitSink, tile_level: u8, size_offset: u8, cell_list: bool) {
    push_flag(bit_sink, LEAF_FLAG);
    push_flag(bit_sink, BIND_FLAG);
    push_flag(bit_sink, (size_offset != 0) as u64);
    if size_offset != 0 {
        bit_sink.push_truncated_binary((most_size_offset(tile_level) - size_offset) as u64, most_size_offset(tile_level) as u64);
    }
    if tile_level + size_offset == CELL_LEVEL {
        push_flag(bit_sink, cell_list as u64);
    }
}

/// Writes `tree`, which starts at `start_level`, for `bitmap`: the start
/// level, then every node from there -- and fills `floor_plan`, whatever it
/// held, with the floor tiles the tree leaves to the last pass.
pub fn write_tree_and_plan_last_pass(stream: &mut BitStream, tree: &Tree, bitmap: &Bitmap, floor_plan: &mut FloorPlan, start_level: u8) {
    floor_plan.clear();
    stream.push_value(start_level as u64, START_LEVEL_WIDTH);
    for tile in Tile::all_of_level(start_level) {
        write_subtree_and_plan_last_pass(stream, tree, bitmap, floor_plan, tile);
    }
}

/// Writes `tile`'s node and everything under it.
fn write_subtree_and_plan_last_pass(stream: &mut BitStream, tree: &Tree, bitmap: &Bitmap, floor_plan: &mut FloorPlan, tile: Tile) {
    let node = tree.get(tile);
    write_node(stream, tree, bitmap, tile, node);
    match node {
        Node::ResidualFloorTile => floor_plan.add_residual_floor_tile(tile),
        Node::CopiedTile { is_far_copy: far, copy_direction: direction, names_children: false } => floor_plan.add_copied_floor_tiles(tile, tile, far, direction),
        _ => {}
    }
    if node.has_children() {
        for (child_tile, child_node) in tile.child_tiles().into_iter().zip(tree.child_elements(tile)) {
            match (child_node, node) {
                (Node::NoNode, Node::CopiedTile { is_far_copy: far, copy_direction: direction, .. }) => floor_plan.add_copied_floor_tiles(tile, child_tile, far, direction),
                (Node::NoNode, _) => {}
                _ => write_subtree_and_plan_last_pass(stream, tree, bitmap, floor_plan, child_tile),
            }
        }
    }
}

/// Reads back what [`write_tree_and_plan_last_pass`] wrote into `cells`, which start
/// clear: every cell the tree says -- and fills `floor_plan`, whatever it
/// held, as writing did.
pub fn read_tree_and_plan_last_pass(stream_reader: &mut BitReader, cells: &mut Bitmap, floor_plan: &mut FloorPlan) {
    floor_plan.clear();
    let start_level = stream_reader.value(START_LEVEL_WIDTH) as u8;
    for tile in Tile::all_of_level(start_level) {
        read_subtree_and_plan_last_pass(stream_reader, cells, floor_plan, tile, BOUND_AT_THE_TOP);
    }
}

/// Reads `tile`'s node and everything under it, `bound_above` the value
/// bound above it.
fn read_subtree_and_plan_last_pass(stream_reader: &mut BitReader, cells: &mut Bitmap, floor_plan: &mut FloorPlan, tile: Tile, bound_above: bool) {
    if read_flag(stream_reader) == DIVIDE_FLAG {
        if tile.tile_level == FLOOR_LEVEL {
            floor_plan.add_residual_floor_tile(tile);
        } else if read_flag(stream_reader) != NAMES_CHILDREN {
            for child_tile in tile.child_tiles() {
                read_subtree_and_plan_last_pass(stream_reader, cells, floor_plan, child_tile, bound_above);
            }
        } else {
            let bound_inside = bound_above != (read_flag(stream_reader) == BINDING_FLIPPED);
            for (child_tile, is_node) in tile.child_tiles().into_iter().zip(read_child_mask(stream_reader)) {
                if is_node {
                    read_subtree_and_plan_last_pass(stream_reader, cells, floor_plan, child_tile, bound_inside);
                } else if bound_inside {
                    cells.set_tile(child_tile.top_left_cell(), child_tile.side_in_cells());
                }
            }
        }
    } else if read_flag(stream_reader) == COPY_FLAG {
        let is_far_copy = stream_reader.read_bit();
        let copy_direction = stream_reader.value(DIRECTION_WIDTH) as u8;
        if !may_name_children(tile.tile_level) || read_flag(stream_reader) != NAMES_CHILDREN {
            floor_plan.add_copied_floor_tiles(tile, tile, is_far_copy, copy_direction);
            return;
        }
        for (child_tile, is_node) in tile.child_tiles().into_iter().zip(read_child_mask(stream_reader)) {
            if is_node {
                read_subtree_and_plan_last_pass(stream_reader, cells, floor_plan, child_tile, bound_above);
            } else {
                floor_plan.add_copied_floor_tiles(tile, child_tile, is_far_copy, copy_direction);
            }
        }
    } else {
        let size_offset =
            if read_flag(stream_reader) == PLAIN_TILE { 0 } else { most_size_offset(tile.tile_level) - stream_reader.truncated_binary(most_size_offset(tile.tile_level) as u64) as u8 };
        if tile.tile_level + size_offset == CELL_LEVEL && read_flag(stream_reader) == CELL_LIST {
            read_cell_list(stream_reader, tile, cells);
        } else {
            read_payload(stream_reader, cells, tile, size_offset);
        }
    }
}
