//! A tree's nodes, counted by kind and level.

use crate::tile::{Tile, CELL_LEVEL, FLOOR_LEVEL};
use crate::tree::{Node, Tree};
use std::collections::BTreeMap;

/// The node at `tile`'s kind, as `docs/tessera.md` names it.
pub fn node_kind_name(tree: &Tree, tile: Tile, node: Node) -> &'static str {
    match node {
        Node::ComplexTile { size_offset } if tile.tile_level + size_offset == CELL_LEVEL => "raw",
        Node::ComplexTile { size_offset: 0 } => "tile",
        Node::ComplexTile { .. } => "complex tile",
        Node::FlippingDivide => "flipping divide",
        Node::PlainDivide if tree.child_elements(tile).contains(&Node::NoNode) => "divide naming children",
        Node::PlainDivide => "divide",
        Node::CopiedTile { names_children: false, .. } => "copy",
        Node::CopiedTile { names_children: true, .. } => "copy naming children",
        Node::ResidualFloorTile => "residual",
        Node::CellList => "cell list",
        Node::NoNode => "absent",
    }
}

/// How many nodes of each kind a tree has at each level, whole bitmap
/// to the 4x4 floor.
pub type Census = BTreeMap<&'static str, [usize; FLOOR_LEVEL as usize + 1]>;

/// `tree`'s census, walked from the whole bitmap down: what lies under
/// a complex tile is not in the tree.
pub fn census(tree: &Tree) -> Census {
    let mut counts = Census::new();
    count(tree, Tile::WHOLE_BITMAP, &mut counts);
    counts
}

/// Counts `tile`'s node and every node under it.
fn count(tree: &Tree, tile: Tile, counts: &mut Census) {
    let node = tree.get(tile);
    counts.entry(node_kind_name(tree, tile, node)).or_default()[tile.tile_level as usize] += 1;
    if node.has_children() {
        for child_tile in tile.child_tiles() {
            if tree.get(child_tile) != Node::NoNode {
                count(tree, child_tile, counts);
            }
        }
    }
}
