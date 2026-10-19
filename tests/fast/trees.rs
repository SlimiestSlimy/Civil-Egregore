//! Trees, on a plain the server makes: they grow a stage at a time to
//! the oldest and no further, spread once old enough and not where
//! they are crowded, die of age only at the oldest, and leave no stage
//! behind.

use protocol::server_parameters::{ParameterValue, SEED};
use crate::probe::{of_the_turn, Probe};
use crate::worlds::{base, cells_of, first_superchunk, nothing_more, plain_start, probed_world, tick_rule};
use server::simulation::simulation_api::{cell_reads_and_writes, tick_and_random_stream, CellIndex};
use basegame::base_types::TREES_RULE;
use basegame::sca_rules::trees::{DIED_COUNT, GROWN_COUNT, SAMPLED_COUNT, SEEDS_FROM, SPREADS_COUNT};
use server::World;
use basegame::base_types::OLDEST_TREE_STAGE;
use utilities::rng::RandomSource;

/// A world of one superchunk, bare, and its probe: and the superchunk's
/// cells, in rows.
fn bare_world(seed: u64) -> (World, Probe, Vec<CellIndex>) {
    let (mut world, probe) = probed_world(&plain_start(1, 0, 0, 2).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let cells = cells_of(first_superchunk(&mut world, &probe)).collect();
    (world, probe, cells)
}

/// Puts a tree of the stage `stage` gives on each of `cells`, its cell
/// barred in the collision plane, in a tick of the probe alone.
fn plant_trees(world: &mut World, probe: &Probe, cells: &[CellIndex], mut stage: impl FnMut() -> u32) {
    let trees: Vec<(CellIndex, u32)> = cells.iter().map(|&planted_cell| (planted_cell, stage())).collect();
    probe.alone(world, move |tick_state| {
        let turn_superchunk = tick_and_random_stream::superchunk(tick_state);
        for &(planted_cell, stage) in trees.iter().filter(|&&(planted_cell, _)| planted_cell.superchunk() == turn_superchunk) {
            cell_reads_and_writes::set(tick_state, base().tree, planted_cell);
            cell_reads_and_writes::set_value(tick_state, base().tree_stage, planted_cell, 0, stage);
            cell_reads_and_writes::set(tick_state, base().collision, planted_cell);
        }
        Vec::<()>::new()
    });
}

/// The stage of every tree in the superchunk of `cells`, read in a tick
/// of the probe alone -- and no cell without a tree has a stage, and
/// the collision plane holds the cells trees stand on and no other.
fn stages(world: &mut World, probe: &Probe, cells: &[CellIndex]) -> Vec<(CellIndex, u32)> {
    let cells = cells.to_vec();
    // Each tree, or what is amiss where one is not: judged after the tick.
    let read: Vec<Result<(CellIndex, u32), String>> = probe.alone(world, move |tick_state| {
        cells.iter().filter(|&&cell| of_the_turn(tick_state, cell)).filter_map(|&cell| {
            let (stage, tree, barred) = (cell_reads_and_writes::value(tick_state, base().tree_stage, cell).expect("hot"), cell_reads_and_writes::layer_holds(tick_state, base().tree, cell), cell_reads_and_writes::layer_holds(tick_state, base().collision, cell));
            if barred != tree {
                Some(Err(format!("{cell:?}: a tree {tree}, barred {barred} -- a tree bars its cell, and nothing else does")))
            } else if tree {
                Some(Ok((cell, stage)))
            } else {
                (stage != 0).then(|| Err(format!("{cell:?}: a stage where no tree is")))
            }
        }).collect()
    });
    read.into_iter().map(|tree| tree.unwrap_or_else(|amiss| panic!("{amiss}"))).collect()
}

/// Trees of every stage scattered thin, half of them the oldest --
/// so some die in so few ticks: each tick the trees are no
/// more than were put and no fewer than died; a tree only ever grows
/// one stage or stays; one gone was at the oldest; one new starts at
/// 0, put by one old enough -- and all of it happens.
#[test]
fn trees_grow_a_stage_at_a_time_spread_and_die_of_age() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe, cells) = bare_world(seed);
    let mut seeded_random = RandomSource::new(seed);
    let planted_cells: Vec<CellIndex> = cells.iter().copied().filter(|_| seeded_random.draw_below(16) == 0).collect();
    plant_trees(&mut world, &probe, &planted_cells, || if seeded_random.draw_below(2) == 0 { OLDEST_TREE_STAGE } else { seeded_random.draw_below(u64::from(OLDEST_TREE_STAGE) + 1) as u32 });
    let mut stages_before = stages(&mut world, &probe, &cells);
    let seeding_trees = stages_before.iter().filter(|(_, stage)| *stage >= SEEDS_FROM).count();
    assert!(seeding_trees > 0, "trees old enough to spread");
    let mut count_totals = [0; 4];
    for tick in 0..400 {
        let tick_counts = tick_rule(&mut world, TREES_RULE).rules;
        // Read in a tick of the probe alone, the trees' rule not run in it.
        let stages_after = stages(&mut world, &probe, &cells);
        let (mut grown_seen, mut died_seen, mut new_trees) = (0, 0, 0);
        let mut earlier_stages = stages_before.iter().peekable();
        for &(cell, stage) in &stages_after {
            // Those gone before this one: `stages` is in one order every time.
            while let Some(&&(gone_cell, gone_stage)) = earlier_stages.peek().filter(|(earlier_tree, _)| earlier_tree.cartesian().y < cell.cartesian().y || (earlier_tree.cartesian().y == cell.cartesian().y && earlier_tree.cartesian().x < cell.cartesian().x)) {
                assert_eq!(gone_stage, OLDEST_TREE_STAGE, "tick {tick}: the tree on {gone_cell:?} died young");
                (died_seen, _) = (died_seen + 1, earlier_stages.next());
            }
            match earlier_stages.peek().filter(|(same_tree, _)| *same_tree == cell) {
                Some(&&(_, old_stage)) => {
                    assert!(stage == old_stage || stage == old_stage + 1, "tick {tick}: a tree went from stage {old_stage} to {stage}");
                    (grown_seen, _) = (grown_seen + u64::from(stage - old_stage), earlier_stages.next());
                }
                None => {
                    assert_eq!(stage, 0, "tick {tick}: a new tree of stage {stage}");
                    new_trees += 1;
                }
            }
        }
        for &(last_gone_cell, last_gone_stage) in earlier_stages {
            assert_eq!(last_gone_stage, OLDEST_TREE_STAGE, "tick {tick}: the tree on {last_gone_cell:?} died young");
            died_seen += 1;
        }
        assert_eq!((grown_seen, died_seen), (tick_counts[GROWN_COUNT], tick_counts[DIED_COUNT]), "tick {tick}: grown and died, seen and counted");
        assert!(new_trees <= tick_counts[SPREADS_COUNT] && tick_counts[GROWN_COUNT] + tick_counts[DIED_COUNT] + tick_counts[SPREADS_COUNT] <= tick_counts[SAMPLED_COUNT], "tick {tick}: {new_trees} new, {tick_counts:?}");
        for (count_total, count) in count_totals.iter_mut().zip([SAMPLED_COUNT, SPREADS_COUNT, GROWN_COUNT, DIED_COUNT]) {
            *count_total += tick_counts[count];
        }
        stages_before = stages_after;
    }
    assert!(count_totals.iter().all(|&count| count > 0), "sampled, spread, grown, died: {count_totals:?}");
}

/// Trees on every cell: none ever spreads, however old -- each has
/// more about it than crowd it, and stays so as some die.
#[test]
fn crowded_trees_never_spread() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe, every_cell) = bare_world(seed);
    plant_trees(&mut world, &probe, &every_cell, || OLDEST_TREE_STAGE);
    let (mut sampled_total, mut died_total) = (0, 0);
    for tick in 0..300 {
        let tick_counts = tick_rule(&mut world, TREES_RULE).rules;
        assert_eq!(tick_counts[SPREADS_COUNT], 0, "tick {tick}");
        (sampled_total, died_total) = (sampled_total + tick_counts[SAMPLED_COUNT], died_total + tick_counts[DIED_COUNT]);
    }
    assert!(sampled_total > 1_000 && died_total > 100, "{sampled_total} sampled, {died_total} died");
}

/// Young trees scattered thin, old enough to spread and none at the
/// oldest stage: they spread, and in too few ticks for any to grow to
/// the oldest, none dies.
#[test]
fn young_trees_spread_and_none_dies() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    let (mut world, probe, cells) = bare_world(seed);
    let planted_cells: Vec<CellIndex> = cells.iter().copied().filter(|_| seeded_random.draw_below(16) == 0).collect();
    plant_trees(&mut world, &probe, &planted_cells, || SEEDS_FROM);
    // Too few ticks for a tree to grow from there to the oldest stage.
    let ticks = 700;
    assert!(u64::from(OLDEST_TREE_STAGE - SEEDS_FROM) * basegame::sca_rules::trees::MEAN_TIME_TO_SAMPLE.ticks() > 8 * ticks);
    let (spread, died) = (0..ticks).map(|_| tick_rule(&mut world, TREES_RULE).rules).fold((0, 0), |spread_died_totals, tick_counts| (spread_died_totals.0 + tick_counts[SPREADS_COUNT], spread_died_totals.1 + tick_counts[DIED_COUNT]));
    assert!(spread > 20 && died == 0, "{spread} spread and {died} died");
}
