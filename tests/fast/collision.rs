//! The collision plane: between ticks it holds the cells trees and
//! sheep stand on and no other -- as a world is generated and flocked,
//! as the sheep step, are born and die, as the trees spread and die --
//! and no sheep comes onto a tree's cell
//! (`docs/server.md`, "The collision plane").
//!
//! `cargo test --test fast collision`

use protocol::server_parameters::{CleanServerParameters, ParameterValue, SEED};
use crate::probe::{entities_of_the_turn, Probe};
use crate::worlds::{base, cells_of, one_flock_world, sheep_tag, sheep_tag_registered, tag_of, tag_sheep, tick_rule};
use server::simulation::simulation_api::{cell_reads_and_writes, tick_and_random_stream, CellIndex, SimulationTickState};
use basegame::base_types::{SHEEP_RULE, TREES_RULE};
use server::World;
use std::collections::HashMap;

/// Whether the collision plane holds exactly the cells trees and
/// entities stand on, over the turn's superchunk; and how many trees
/// those are.
fn what_stands_and_no_other(tick_state: &SimulationTickState) -> (bool, u64) {
    let stood_on: std::collections::HashSet<CellIndex> = entities_of_the_turn(tick_state).iter().map(|entity| entity.header.stood_cell).collect();
    let (mut collision_matches, mut trees) = (true, 0);
    for cell in cells_of(tick_and_random_stream::superchunk(tick_state)) {
        let tree = cell_reads_and_writes::layer_holds(tick_state, base().tree, cell);
        collision_matches &= cell_reads_and_writes::layer_holds(tick_state, base().collision, cell) == (tree || stood_on.contains(&cell));
        trees += u64::from(tree);
    }
    (collision_matches, trees)
}

/// [`what_stands_and_no_other`] over every hot superchunk of `world`,
/// read in a tick of the probe alone.
fn over_the_world(world: &mut World, probe: &Probe) -> (bool, u64) {
    probe.alone(world, |tick_state| vec![what_stands_and_no_other(tick_state)]).into_iter().fold((true, 0), |(all_match, all_trees), (matches, trees)| (all_match && matches, all_trees + trees))
}

/// A sheep as the tick found it: its tag if it has one, its cell, and
/// whether a tree stands there.
type SeenSheep = (Option<u64>, CellIndex, bool);

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
    let (mut world, probe) = one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 4_000, sheep_tag_registered);
    let (collision_matches, trees) = over_the_world(&mut world, &probe);
    assert!(collision_matches && trees > 0, "as generated: {trees} trees");
    // Each sheep told from the others by a tag; a lamb, born after, has none.
    tag_sheep(&mut world, &probe);
    let sheep_tag = sheep_tag(&world);
    let read_sheep = move |tick_state: &mut SimulationTickState| -> Vec<SeenSheep> { entities_of_the_turn(tick_state).iter().map(|seen_sheep| (tag_of(sheep_tag, seen_sheep), seen_sheep.header.stood_cell, cell_reads_and_writes::layer_holds(tick_state, base().tree, seen_sheep.header.stood_cell))).collect() };
    let tagged = probe.alone(&mut world, read_sheep);
    assert!(tagged.iter().all(|&(tag, _, on_tree)| tag.is_some() && !on_tree), "every sheep tagged, and none put on a tree");
    let mut stood_cell_by_tag: HashMap<u64, CellIndex> = tagged.iter().map(|&(tag, stood_cell, _)| (tag.expect("tagged"), stood_cell)).collect();
    let (mut steps_seen, mut lamb_ticks) = (0, 0);
    // A stretch at the least; and on, to the first step: within two meals, as no grass grows back with the sheep's rule alone run.
    let (stretch_ticks, mut tick) = (400, 0);
    while tick < stretch_ticks || steps_seen == 0 && tick < 2 * basegame::entity_rules::sheep::TIME_BETWEEN_MEALS.ticks() {
        // The sheep as the tick found them -- as the one before left them -- and the collision plane every 50th.
        let checks_plane = tick % 50 == 0 && tick > 0 && tick <= stretch_ticks;
        let (_, seen) = probe.tick_rule(&mut world, SHEEP_RULE, move |tick_state| vec![(read_sheep(tick_state), checks_plane.then(|| what_stands_and_no_other(tick_state).0))]);
        for (seen_sheep, plane_matches) in seen {
            assert!(plane_matches.unwrap_or(true), "after tick {tick} of the sheep");
            for (tag, stood_cell, on_tree) in seen_sheep {
                let Some(tag_number) = tag else {
                    assert!(!on_tree, "a lamb on a tree's cell");
                    lamb_ticks += 1;
                    continue;
                };
                if stood_cell_by_tag.insert(tag_number, stood_cell) != Some(stood_cell) {
                    assert!(!on_tree, "a sheep come onto a tree's cell");
                    steps_seen += 1;
                }
            }
        }
        tick += 1;
    }
    assert!(steps_seen > 0, "{steps_seen} steps, {lamb_ticks} lamb-ticks in {tick} ticks");
    assert!(over_the_world(&mut world, &probe).0, "after {tick} ticks of the sheep");
    // The trees after the sheep: a sheep whose wake passes with its rule not run never wakes again.
    let mut trees_counts = server::simulation::simulation_api::RuleCounts::default();
    for _ in 0..2000 {
        trees_counts += tick_rule(&mut world, TREES_RULE).rules;
    }
    assert!(over_the_world(&mut world, &probe).0, "after the trees spread and died");
    assert!(trees_counts[basegame::sca_rules::trees::SPREADS_COUNT] > 0, "trees put");
}
