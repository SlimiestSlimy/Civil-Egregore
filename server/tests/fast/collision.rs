//! The collision plane: what a world is generated with holds the
//! cells its trees stand on and no other, and stays so as trees
//! spread and die; and no sheep steps onto a cell it holds, nor is
//! born on one (`docs/server.md`, "The collision plane").
//!
//! `cargo test --test fast collision`

use crate::tests::{land_seed, tick_rule, tick_sheep};
use coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use server::{Start, World};
use std::collections::HashMap;
use type_registry::{COLLISION, TREE};

/// Whether the collision plane holds exactly the cells trees stand on,
/// over every hot superchunk of `world`; and how many those are.
fn trees_and_no_other(world: &World) -> (bool, u64) {
    let arena = world.arena();
    let (mut same, mut trees) = (true, 0);
    for superchunk in arena.superchunk_indices() {
        let corner = superchunk.top_left().cartesian();
        for (x, y) in (0..SUPERCHUNK_SIDE_CELLS).flat_map(|y| (0..SUPERCHUNK_SIDE_CELLS).map(move |x| (x, y))) {
            let cell: CellIndex = CellCartesian { x: corner.x + x, y: corner.y + y }.into();
            let tree = arena.holds(TREE, cell);
            same &= arena.holds(COLLISION, cell) == tree;
            trees += u64::from(tree == Ok(true));
        }
    }
    (same, trees)
}

/// A world generated with trees and a flock: the collision plane is
/// the trees' cells as it is made; the sheep ticked among them, every
/// sheep that stepped or was born stands on a cell the plane does not
/// hold; and after the trees' rule ran on it the plane is the trees'
/// cells still.
#[test]
fn trees_bar_their_cells_and_no_sheep_steps_onto_one() {
    let mut world = server::start(Start { seed: land_seed(0), sheep: 4000, ..Start::default() });
    let (same, trees) = trees_and_no_other(&world);
    assert!(same && trees > 0, "as generated: {trees} trees");
    let mut stood: HashMap<u64, CellIndex> = world.entities().iter().map(|sheep| (sheep.header.id.0, sheep.header.at)).collect();
    let (mut stepped, mut born) = (0, 0);
    for _ in 0..400 {
        tick_sheep(&mut world);
        for sheep in world.entities().iter() {
            let (id, at) = (sheep.header.id.0, sheep.header.at);
            let was = stood.insert(id, at);
            if was != Some(at) {
                assert_eq!(world.arena().holds(COLLISION, at), Ok(false), "a sheep come onto a cell the collision plane holds");
                (stepped, born) = (stepped + u64::from(was.is_some()), born + u64::from(was.is_none()));
            }
        }
    }
    assert!(stepped > 0, "{stepped} steps, {born} births");
    // The trees after the sheep: a sheep whose wake passes with its rule not run never wakes again.
    let mut done = instructions::RuleCounts::default();
    for _ in 0..2000 {
        done += tick_rule(&mut world, server::TREES_RULE).rules;
    }
    assert!(trees_and_no_other(&world).0, "after the trees spread and died");
    assert!(done[sca_rules::trees::SPREADS] > 0, "trees put");
}
