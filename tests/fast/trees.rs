//! Trees, on a plain the server makes: they grow a stage at a time to
//! the oldest and no further, spread once old enough and not where
//! they are crowded, die of age only at the oldest, and leave no stage
//! behind.

use crate::worlds::base;
use crate::worlds::{first_superchunk, plain_start, tick_rule};
use server::bitplane_manager::{Write, WriteOp};
use utilities::coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use basegame::sca_rules::trees::{DIED_COUNT, GROWN_COUNT, SAMPLED_COUNT, SEEDS_FROM, SPREADS_COUNT};
use server::World;
use basegame::base_types::OLDEST_TREE_STAGE;
use utilities::rng::RandomSource;

/// Every cell of `world`'s first superchunk, in rows.
fn cells(world: &World) -> impl Iterator<Item = CellIndex> + use<> {
    let corner_cartesian = first_superchunk(world).top_left().cartesian();
    (0..SUPERCHUNK_SIDE_CELLS).flat_map(move |y| (0..SUPERCHUNK_SIDE_CELLS).map(move |x| CellCartesian { x: corner_cartesian.x + x, y: corner_cartesian.y + y }.into()))
}

/// Puts a tree of the stage `stage` gives on each of `cells`.
fn plant_trees(world: &mut World, cells: &[CellIndex], mut stage: impl FnMut() -> u32) {
    let planted = world.write_cells(base().tree, cells.iter().map(|&planted_cell| Write::cell(planted_cell, WriteOp::SetCell)));
    let stages_written = world.write_cells(base().tree_stage.layer_type(), cells.iter().map(|&aged_cell| Write::value(base().tree_stage, aged_cell, stage())));
    let collisions_written = world.write_cells(base().collision, cells.iter().map(|&barred_cell| Write::cell(barred_cell, WriteOp::SetCell)));
    assert_eq!((planted.missed, stages_written.missed, collisions_written.missed), (0, 0, 0), "trees planted off the hot superchunks");
}

/// The stage of every tree in `world`'s first superchunk -- and no
/// cell without a tree has a stage, and the collision plane holds the
/// cells trees stand on and no other.
fn stages(world: &World) -> Vec<(CellIndex, u32)> {
    let bitmap_arena = world.bitmap_arena();
    let mut stages = Vec::new();
    for cell in cells(world) {
        let stage = bitmap_arena.value(base().tree_stage, cell).expect("hot");
        assert_eq!(bitmap_arena.layer_holds_cell(base().collision, cell), bitmap_arena.layer_holds_cell(base().tree, cell), "a tree bars its cell, and nothing else does");
        match bitmap_arena.layer_holds_cell(base().tree, cell).expect("hot") {
            true => stages.push((cell, stage)),
            false => assert_eq!(stage, 0, "a stage where no tree is"),
        }
    }
    stages
}

/// Trees of every stage scattered thin, half of them the oldest --
/// so some die in so few ticks: each tick the trees are no
/// more than were put and no fewer than died; a tree only ever grows
/// one stage or stays; one gone was at the oldest; one new starts at
/// 0, put by one old enough -- and all of it happens.
#[test]
fn trees_grow_a_stage_at_a_time_spread_and_die_of_age() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut world = crate::worlds::start_world(server::WorldStart { seed, ..plain_start(1, 0, 0, 2) });
    let mut seeded_random = RandomSource::new(seed);
    let planted_cells: Vec<CellIndex> = cells(&world).filter(|_| seeded_random.draw_below(16) == 0).collect();
    plant_trees(&mut world, &planted_cells, || if seeded_random.draw_below(2) == 0 { OLDEST_TREE_STAGE } else { seeded_random.draw_below(u64::from(OLDEST_TREE_STAGE) + 1) as u32 });
    let mut stages_before = stages(&world);
    let seeding_trees = stages_before.iter().filter(|(_, stage)| *stage >= SEEDS_FROM).count();
    assert!(seeding_trees > 0, "trees old enough to spread");
    let mut count_totals = [0; 4];
    for tick in 0..400 {
        let tick_counts = tick_rule(&mut world, crate::worlds::type_id_named(basegame::base_types::TREES_RULE)).rules;
        let stages_after = stages(&world);
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
    let mut world = crate::worlds::start_world(server::WorldStart { seed, ..plain_start(1, 0, 0, 2) });
    let every_cell: Vec<CellIndex> = cells(&world).collect();
    plant_trees(&mut world, &every_cell, || OLDEST_TREE_STAGE);
    let (mut sampled_total, mut died_total) = (0, 0);
    for tick in 0..300 {
        let tick_counts = tick_rule(&mut world, crate::worlds::type_id_named(basegame::base_types::TREES_RULE)).rules;
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
    let mut world = crate::worlds::start_world(server::WorldStart { seed, ..plain_start(1, 0, 0, 2) });
    let planted_cells: Vec<CellIndex> = cells(&world).filter(|_| seeded_random.draw_below(16) == 0).collect();
    plant_trees(&mut world, &planted_cells, || SEEDS_FROM);
    // Too few ticks for a tree to grow from there to the oldest stage.
    let ticks = 700;
    assert!(u64::from(OLDEST_TREE_STAGE - SEEDS_FROM) * basegame::sca_rules::trees::MEAN_TIME_TO_SAMPLE.ticks() > 8 * ticks);
    let (spread, died) = (0..ticks).map(|_| tick_rule(&mut world, crate::worlds::type_id_named(basegame::base_types::TREES_RULE)).rules).fold((0, 0), |spread_died_totals, tick_counts| (spread_died_totals.0 + tick_counts[SPREADS_COUNT], spread_died_totals.1 + tick_counts[DIED_COUNT]));
    assert!(spread > 20 && died == 0, "{spread} spread and {died} died");
}
