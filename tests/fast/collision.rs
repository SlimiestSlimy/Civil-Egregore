//! The collision plane: between ticks it holds the cells trees and
//! sheep stand on and no other -- as a world is generated and flocked,
//! as the sheep step, are born and die, as the trees spread and die --
//! and no sheep comes onto a tree's cell
//! (`docs/server.md`, "The collision plane").
//!
//! `cargo test --test fast collision`

use crate::worlds::base;
use crate::worlds::{tag_of, tag_sheep, tick_rule, tick_sheep};
use utilities::coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use server::{WorldStart, World};
use std::collections::HashMap;

/// Whether the collision plane holds exactly the cells trees and
/// entities stand on, over every hot superchunk of `world`; and how
/// many trees those are.
fn what_stands_and_no_other(world: &World) -> (bool, u64) {
    let bitmap_arena = world.bitmap_arena();
    let stood_on: std::collections::HashSet<CellIndex> = world.entity_store().iter().map(|entity| entity.header.stood_cell).collect();
    let (mut collision_matches, mut trees) = (true, 0);
    for superchunk in bitmap_arena.superchunk_indices() {
        let top_left_cartesian = superchunk.top_left().cartesian();
        for (x, y) in (0..SUPERCHUNK_SIDE_CELLS).flat_map(|y| (0..SUPERCHUNK_SIDE_CELLS).map(move |x| (x, y))) {
            let cell: CellIndex = CellCartesian { x: top_left_cartesian.x + x, y: top_left_cartesian.y + y }.into();
            let tree = bitmap_arena.layer_holds_cell(base().tree, cell);
            collision_matches &= bitmap_arena.layer_holds_cell(base().collision, cell) == Ok(tree == Ok(true) || stood_on.contains(&cell));
            trees += u64::from(tree == Ok(true));
        }
    }
    (collision_matches, trees)
}

/// A world generated with trees and a flock: the collision plane is
/// the trees' and the sheep's cells as it is made, no sheep put on a
/// tree; the sheep ticked among them, it is so after every stretch,
/// and every sheep that stepped or was born stands on no tree's cell;
/// and it is so still after the trees' rule ran. The sheep are ticked
/// until some have stepped, however few are hungry off grass at first
/// on the seed's land: one that ate has no grass under it, and steps
/// when it is hungry again, a meal later.
#[test]
fn trees_bar_their_cells_and_no_sheep_steps_onto_one() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut world = crate::worlds::one_flock_world(WorldStart { seed, ..WorldStart::default() }, 4_000);
    let (collision_matches, trees) = what_stands_and_no_other(&world);
    assert!(collision_matches && trees > 0, "as generated: {trees} trees");
    assert!(world.entity_store().iter().all(|put_sheep| world.bitmap_arena().layer_holds_cell(base().tree, put_sheep.header.stood_cell) == Ok(false)), "a sheep put on a tree");
    // Each sheep told from the others by a tag; a lamb, born after, has none.
    tag_sheep(&mut world);
    let mut stood_cell_by_tag: HashMap<u64, CellIndex> = world.entity_store().iter().map(|tagged_sheep| (tag_of(&tagged_sheep).expect("tagged"), tagged_sheep.header.stood_cell)).collect();
    let (mut steps_seen, mut lamb_ticks) = (0, 0);
    // A stretch at the least; and on, to the first step: within two meals, as no grass grows back with the sheep's rule alone run.
    let (stretch_ticks, mut tick) = (400, 0);
    while tick < stretch_ticks || steps_seen == 0 && tick < 2 * basegame::entity_rules::sheep::TIME_BETWEEN_MEALS.ticks() {
        tick_sheep(&mut world);
        tick += 1;
        if tick % 50 == 0 && tick <= stretch_ticks {
            assert!(what_stands_and_no_other(&world).0, "after tick {tick} of the sheep");
        }
        for ticked_sheep in world.entity_store().iter() {
            let stood_cell = ticked_sheep.header.stood_cell;
            let Some(sheep_tag) = tag_of(&ticked_sheep) else {
                assert_eq!(world.bitmap_arena().layer_holds_cell(base().tree, stood_cell), Ok(false), "a lamb on a tree's cell");
                lamb_ticks += 1;
                continue;
            };
            if stood_cell_by_tag.insert(sheep_tag, stood_cell) != Some(stood_cell) {
                assert_eq!(world.bitmap_arena().layer_holds_cell(base().tree, stood_cell), Ok(false), "a sheep come onto a tree's cell");
                steps_seen += 1;
            }
        }
    }
    assert!(steps_seen > 0, "{steps_seen} steps, {lamb_ticks} lamb-ticks in {tick} ticks");
    assert!(what_stands_and_no_other(&world).0, "after {tick} ticks of the sheep");
    // The trees after the sheep: a sheep whose wake passes with its rule not run never wakes again.
    let mut trees_counts = server::simulation::simulation_api::RuleCounts::default();
    for _ in 0..2000 {
        trees_counts += tick_rule(&mut world, crate::worlds::type_id_named(basegame::base_types::TREES_RULE)).rules;
    }
    assert!(what_stands_and_no_other(&world).0, "after the trees spread and died");
    assert!(trees_counts[basegame::sca_rules::trees::SPREADS_COUNT] > 0, "trees put");
}
