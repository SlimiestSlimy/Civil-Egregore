//! Trees, on a plain the server makes: they grow a stage at a time to
//! the oldest and no further, spread once old enough and not where
//! they are crowded and as the air is humid, die of age only at the
//! oldest and of a harsh climate at any stage, and leave no stage
//! behind.

use crate::tests::{first_superchunk, plain_world, tick_rule};
use bitplane_manager::{Write, WriteOp};
use coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use sca_rules::trees::{DIED_COUNT, GROWN_COUNT, SAMPLED_COUNT, SEEDS_FROM, SPREADS_COUNT};
use server::World;
use crate::grass::put_climate;
use type_registry::{Climate, Wind, COLLISION, HOTTEST_TEMPERATURE, MOST_HUMID, OLDEST_TREE_STAGE, PRESSURE_AT_REST, TREE, TREE_STAGE};
use utilities::rng::RandomSource;

/// Every cell of `world`'s first superchunk, in rows.
fn cells(world: &World) -> impl Iterator<Item = CellIndex> + use<> {
    let corner_cartesian = first_superchunk(world).top_left().cartesian();
    (0..SUPERCHUNK_SIDE_CELLS).flat_map(move |y| (0..SUPERCHUNK_SIDE_CELLS).map(move |x| CellCartesian { x: corner_cartesian.x + x, y: corner_cartesian.y + y }.into()))
}

/// Puts a tree of the stage `stage` gives on each of `cells`.
fn plant_trees(world: &mut World, cells: &[CellIndex], mut stage: impl FnMut() -> u32) {
    let put = world.write_cells(TREE, cells.iter().map(|&put_cell| Write::cell(put_cell, WriteOp::SetCell)));
    let stages_written = world.write_cells(TREE_STAGE.layer_type(), cells.iter().map(|&aged_cell| Write::value(TREE_STAGE, aged_cell, stage())));
    let collisions_written = world.write_cells(COLLISION, cells.iter().map(|&barred_cell| Write::cell(barred_cell, WriteOp::SetCell)));
    assert_eq!((put.missed, stages_written.missed, collisions_written.missed), (0, 0, 0), "trees planted off the hot superchunks");
}

/// The stage of every tree in `world`'s first superchunk -- and no
/// cell without a tree has a stage, and the collision plane holds the
/// cells trees stand on and no other.
fn stages(world: &World) -> Vec<(CellIndex, u32)> {
    let bitmap_arena = world.bitmap_arena();
    let mut stages = Vec::new();
    for cell in cells(world) {
        let stage = bitmap_arena.value(TREE_STAGE, cell).expect("hot");
        assert_eq!(bitmap_arena.layer_holds_cell(COLLISION, cell), bitmap_arena.layer_holds_cell(TREE, cell), "a tree bars its cell, and nothing else does");
        match bitmap_arena.layer_holds_cell(TREE, cell).expect("hot") {
            true => stages.push((cell, stage)),
            false => assert_eq!(stage, 0, "a stage where no tree is"),
        }
    }
    stages
}

/// The mildest climate: the most humid, the coldest.
const MILDEST_CLIMATE: Climate = Climate { humidity: MOST_HUMID, wind: Wind::CALM_AIR, temperature: 0, pressure: PRESSURE_AT_REST };

/// Trees of every stage scattered thin, half of them the oldest --
/// so some die in so few ticks: each tick the trees are no
/// more than were put and no fewer than died; a tree only ever grows
/// one stage or stays; one gone was at the oldest; one new starts at
/// 0, put by one old enough -- and all of it happens.
#[test]
fn trees_grow_a_stage_at_a_time_spread_and_die_of_age() {
    let mut world = plain_world(1, 0, 0, 2);
    let mut seeded_random = RandomSource::new(utilities::seed::counted_seed());
    // The mildest climate: no tree dies of it, and the air always lets one spread.
    put_climate(&mut world, MILDEST_CLIMATE);
    let planted_cells: Vec<CellIndex> = cells(&world).filter(|_| seeded_random.draw_below(16) == 0).collect();
    plant_trees(&mut world, &planted_cells, || if seeded_random.draw_below(2) == 0 { OLDEST_TREE_STAGE } else { seeded_random.draw_below(u64::from(OLDEST_TREE_STAGE) + 1) as u32 });
    let mut stages_before = stages(&world);
    let seeding_trees = stages_before.iter().filter(|(_, stage)| *stage >= SEEDS_FROM).count();
    assert!(seeding_trees > 0, "trees old enough to spread");
    let mut count_totals = [0; 4];
    for tick in 0..400 {
        let tick_counts = tick_rule(&mut world, server::TREES_RULE).rules;
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
    let mut world = plain_world(1, 0, 0, 2);
    let every_cell: Vec<CellIndex> = cells(&world).collect();
    plant_trees(&mut world, &every_cell, || OLDEST_TREE_STAGE);
    let (mut sampled_total, mut died_total) = (0, 0);
    for tick in 0..300 {
        let tick_counts = tick_rule(&mut world, server::TREES_RULE).rules;
        assert_eq!(tick_counts[SPREADS_COUNT], 0, "tick {tick}");
        (sampled_total, died_total) = (sampled_total + tick_counts[SAMPLED_COUNT], died_total + tick_counts[DIED_COUNT]);
    }
    assert!(sampled_total > 1_000 && died_total > 100, "{sampled_total} sampled, {died_total} died");
}

/// The climate over trees: young ones scattered thin, old enough to
/// spread and none at the oldest stage, so none dies of age. In the
/// mildest climate none dies and they spread; in the driest and
/// hottest they die young and none spreads; and in a hot climate of a
/// humidity drawn between they die less than in the harshest.
#[test]
fn trees_spread_as_the_air_is_humid_and_die_as_the_climate_is_harsh() {
    let mut seeded_random = RandomSource::new(utilities::seed::counted_seed());
    let humidity_between = seeded_random.draw_between(64, 192) as u32;
    let harshest_climate = Climate { humidity: 0, temperature: HOTTEST_TEMPERATURE, ..MILDEST_CLIMATE };
    let [mild_outcome, between_outcome, harsh_outcome] = [MILDEST_CLIMATE, Climate { humidity: humidity_between, ..harshest_climate }, harshest_climate].map(|climate| {
        let mut world = plain_world(1, 0, 0, 2);
        put_climate(&mut world, climate);
        let planted_cells: Vec<CellIndex> = cells(&world).filter(|_| seeded_random.draw_below(16) == 0).collect();
        plant_trees(&mut world, &planted_cells, || SEEDS_FROM);
        // Too few ticks for a tree to grow from there to the oldest stage.
        let ticks = 700;
        assert!(u64::from(OLDEST_TREE_STAGE - SEEDS_FROM) * sca_rules::trees::MEAN_TIME_TO_SAMPLE.ticks() > 8 * ticks);
        (0..ticks).map(|_| tick_rule(&mut world, server::TREES_RULE).rules).fold((0, 0), |spread_died_totals, tick_counts| (spread_died_totals.0 + tick_counts[SPREADS_COUNT], spread_died_totals.1 + tick_counts[DIED_COUNT]))
    });
    assert!(mild_outcome.0 > 20 && mild_outcome.1 == 0, "the mildest: {mild_outcome:?} spread and died");
    assert!(harsh_outcome.0 == 0 && harsh_outcome.1 > 20, "the harshest: {harsh_outcome:?} spread and died");
    assert!(between_outcome.0 > 0 && between_outcome.0 < mild_outcome.0 && between_outcome.1 > 0 && between_outcome.1 < harsh_outcome.1, "humidity {humidity_between}, hot: {between_outcome:?} spread and died, between {mild_outcome:?} and {harsh_outcome:?}");
}
