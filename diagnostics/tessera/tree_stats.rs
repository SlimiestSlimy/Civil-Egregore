//! What a tree holds: tiles, complex tiles and the tiles of their
//! resolutions they say, nodes naming children, and cell lists.

use crate::tree::{Node, Tree};
use crate::tile::Tile;

/// The counts, over one tree or added up over many.
#[derive(Default, Clone, Debug)]
pub struct TreeStats {
    /// Whole binds that are nodes: complex tiles at size offset 0.
    pub tiles: usize,
    /// Complex tiles finer than size offset 0, cell lists aside.
    pub complex_tiles: usize,
    /// Tiles of a complex tile's resolution said in its payload: one bit
    /// each.
    pub payload_values: usize,
    /// Copies naming some of their children.
    pub copies_naming_children: usize,
    /// Flipping divides.
    pub flipping_divides: usize,
    /// Cell lists.
    pub cell_lists: usize,
}

impl TreeStats {
    /// The counts of one tree.
    pub fn measure_tree(tree: &Tree) -> Self {
        let mut tree_stats = Self::default();
        tree_stats.count(tree, Tile::WHOLE_BITMAP);
        tree_stats
    }

    /// Adds `other_stats`'s counts to these.
    pub fn add_counts(&mut self, other_stats: &TreeStats) {
        self.tiles += other_stats.tiles;
        self.complex_tiles += other_stats.complex_tiles;
        self.payload_values += other_stats.payload_values;
        self.copies_naming_children += other_stats.copies_naming_children;
        self.flipping_divides += other_stats.flipping_divides;
        self.cell_lists += other_stats.cell_lists;
    }

    /// Counts `tile`'s node and everything under it.
    fn count(&mut self, tree: &Tree, tile: Tile) {
        match tree.get(tile) {
            Node::CopiedTile { names_children: true, .. } => {
                self.copies_naming_children += 1;
                self.count_children(tree, tile);
            }
            Node::FlippingDivide => {
                self.flipping_divides += 1;
                self.count_children(tree, tile);
            }
            Node::PlainDivide => self.count_children(tree, tile),
            Node::ComplexTile { size_offset: 0 } => self.tiles += 1,
            Node::ComplexTile { size_offset } => {
                self.complex_tiles += 1;
                self.payload_values += 1 << (2 * size_offset);
            }
            Node::CellList => self.cell_lists += 1,
            Node::CopiedTile { names_children: false, .. } | Node::ResidualFloorTile | Node::NoNode => {}
        }
    }

    /// Counts every child of `tile` that is a node.
    fn count_children(&mut self, tree: &Tree, tile: Tile) {
        for child_tile in tile.child_tiles() {
            self.count(tree, child_tile);
        }
    }
}
