//! Two writes on one cell in a tick: a tick's counts are the cells
//! that changed, to the cell. No two sheep eat one cell; a cell of
//! grass spread onto twice is one spread, one decayed and eaten is one
//! of the two; a tree put twice is one tree put; and of two lambs put
//! on one cell the one refused is born later
//! (`docs/server.md`, "Two writes on one cell").

use protocol::server_parameters::{ParameterValue, SEED};
use crate::probe::{count_of_the_turn, entities_of_the_turn, rule_place, take_entity, words_of_the_turn, Probe};
use crate::worlds::{base, cells_set_and_cleared, first_superchunk, nothing_more, plain_start, probed_world, put_entities, set_rectangle, tick_read_at, tick_sheep};
use server::simulation::simulation_api::{cell_reads_and_writes, tick_and_random_stream, AttributeBlock, CellIndex, Header, WholeAttribute, NEVER_WAKES};
use utilities::coordinates::{CellCartesian, SuperchunkIndex};
use basegame::base_types::{GRASS_RULE, SHEEP_RULE, TREES_RULE};
use basegame::entity_rules::sheep::{SheepState, BIRTHS_COUNT, EATEN_COUNT, STEP_JITTER, STEP};
use basegame::sca_rules::{grass, trees};
use server::World;
use std::collections::HashSet;
use utilities::rng::RandomSource;

/// A plain world of one superchunk on `thread_count` threads, of
/// `grass_cover` and `sheep_per_flock`, and its probe: and the cell
/// `(x, y)` cells from its top left, as a function.
fn one_superchunk(test_name: &str, grass_cover: u64, sheep_per_flock: usize, thread_count: usize) -> (World, Probe, impl Fn(u32, u32) -> CellIndex) {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe) = probed_world(&plain_start(test_name, 1, grass_cover, sheep_per_flock, thread_count).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let superchunk: SuperchunkIndex = first_superchunk(&mut world, &probe);
    let top_left_cartesian = superchunk.top_left().cartesian();
    (world, probe, move |x, y| CellCartesian { x: top_left_cartesian.x + x, y: top_left_cartesian.y + y }.into())
}

/// No two sheep eat one cell: a crowd of hungry sheep on a square of
/// grass, the sheep's rule alone -- every tick, the cells of grass
/// cleared are the meals counted, and none is set.
#[test]
fn no_two_sheep_eat_one_cell() {
    const CROWD_SIDE: u32 = 200;
    let (mut world, probe, cell) = one_superchunk("no_two_sheep_eat_one_cell", 0, 0, 2);
    let mut test_random = RandomSource::new(crate::test_seed::TestSeedHandle::open().seed());
    set_rectangle(&mut world, &probe, base().grass, cell(400, 400).cartesian(), CROWD_SIDE, CROWD_SIDE);
    let mut taken_offsets = HashSet::new();
    let mut crowd = Vec::new();
    while taken_offsets.len() < 10_000 {
        let (x, y) = (test_random.draw_below(CROWD_SIDE as u64) as u32, test_random.draw_below(CROWD_SIDE as u64) as u32);
        if taken_offsets.insert((x, y)) {
            crowd.push((Header { entity_type: base().sheep, stood_cell: cell(400 + x, 400 + y), wake: test_random.draw_below(STEP.ticks()) }, hungry_at(0)));
        }
    }
    put_entities(&mut world, &probe, crowd);
    // Each tick's grass as the tick found it, with the sheep's rule: what the tick before did to it.
    let grass = base().grass;
    let mut ticks_read = (0..=4 * STEP.ticks()).map(|_| probe.tick_rule(&mut world, SHEEP_RULE, move |tick_state| words_of_the_turn(tick_state, grass)));
    let (first_report, grass_at_first) = ticks_read.next().expect("a tick");
    let (mut cells_eaten, mut grass_words_before, mut eaten_before) = (0, grass_at_first.clone(), first_report.rules[EATEN_COUNT]);
    for (tick, (report, grass_words)) in ticks_read.enumerate() {
        assert_eq!(cells_set_and_cleared(&grass_words_before, &grass_words), (0, eaten_before), "tick {tick}");
        cells_eaten += eaten_before;
        (grass_words_before, eaten_before) = (grass_words, report.rules[EATEN_COUNT]);
    }
    let count = |words: &[(utilities::coordinates::ChunkIndex, Vec<u64>)]| words.iter().flat_map(|(_, words)| words).map(|word| u64::from(word.count_ones())).sum::<u64>();
    assert!(cells_eaten >= 10_000, "{cells_eaten} cells eaten");
    assert_eq!(count(&grass_words_before), count(&grass_at_first) - cells_eaten);
}

/// Grass and sheep together: every tick, the cells set are the spreads
/// counted and the cells cleared the decays and the meals counted
/// together, exactly -- a write is counted as it is applied, and one
/// that finds its cell changed since its rule saw it is refused, and
/// counts nothing. Two writes meet on a cell seldom: the meeting is
/// made to happen in the simulation's own tests and the sheep's
/// (`a_meal_lost_to_a_decay_leaves_the_sheep_hungry`).
#[test]
fn grass_spread_decayed_and_eaten_changes_once_a_cell() {
    let (mut world, probe, _) = one_superchunk("grass_spread_decayed_and_eaten_changes_once_a_cell", basegame::worldgen::FRACTION_ONE / 2, 20_000, 2);
    let (grass_rule, sheep_rule, grass) = (rule_place(&world, GRASS_RULE), rule_place(&world, SHEEP_RULE), base().grass);
    // Each tick's grass as the tick found it, with both rules: what the tick before did to it.
    let (mut grass_words_before, mut report_before): (Option<crate::worlds::ChunkWords>, _) = (None, None);
    for tick in 0..=600 {
        let (report, grass_words) = probe.tick(&mut world, &[GRASS_RULE, SHEEP_RULE], move |tick_state| words_of_the_turn(tick_state, grass));
        if let (Some(grass_words_before), Some(report_before)) = (&grass_words_before, &report_before) {
            let report_before: &server::simulation::simulation_api::TickReport<server::TickCounts> = report_before;
            let (grass_counts, sheep_counts) = (report_before.rules.counts_of(grass_rule), report_before.rules.counts_of(sheep_rule));
            let (cells_set, cells_cleared) = cells_set_and_cleared(grass_words_before, &grass_words);
            assert_eq!(cells_set, grass_counts[grass::SPREADS_COUNT], "tick {tick}: cells set, spreads counted");
            let (grass_decays, cells_eaten) = (grass_counts[grass::DECAYS_COUNT], sheep_counts[EATEN_COUNT]);
            assert_eq!(cells_cleared, grass_decays + cells_eaten, "tick {tick}: cells cleared, {grass_decays} decayed and {cells_eaten} eaten");
            assert_eq!(report_before.writes_applied.writes as u64, report_before.writes_applied.cells_changed + report_before.writes_applied.writes_refused, "tick {tick}: a write changes its cell, or is refused");
        }
        (grass_words_before, report_before) = (Some(grass_words), Some(report));
    }
}

/// Trees alone: every tick, the trees there are grow by exactly those
/// counted put, less those that died -- two put on one cell are one
/// tree, and one put counted.
#[test]
fn a_tree_written_twice_is_one_tree() {
    let (mut world, probe, cell) = one_superchunk("a_tree_written_twice_is_one_tree", 0, 0, 2);
    // A tree every fourth cell each way, each old enough to spread and not crowded.
    let planted: Vec<CellIndex> = (0..256).flat_map(|y| (0..256).map(move |x| (4 * x, 4 * y))).map(|(x, y)| cell(x, y)).collect();
    probe.alone(&mut world, move |tick_state| {
        let turn_superchunk = tick_and_random_stream::superchunk(tick_state);
        for &tree_cell in planted.iter().filter(|&&tree_cell| tree_cell.superchunk() == turn_superchunk) {
            cell_reads_and_writes::set(tick_state, base().tree, tree_cell);
            cell_reads_and_writes::set_value(tick_state, base().tree_stage, tree_cell, 0, trees::SEEDS_FROM);
        }
        Vec::<()>::new()
    });
    let (trees_rule, tree) = (rule_place(&world, TREES_RULE), base().tree);
    // Each tick's trees as the tick found them, with the trees' rule: what the tick before did to them.
    let (mut spread, mut before) = (0, None);
    for tick in 0..=2_000 {
        let (report, trees_there) = probe.tick(&mut world, &[TREES_RULE], move |tick_state| vec![count_of_the_turn(tick_state, tree)]);
        let trees_there: u64 = trees_there.into_iter().sum();
        if let Some((trees_before, trees_counts)) = before {
            let trees_counts: server::simulation::simulation_api::RuleCounts = trees_counts;
            let trees_set = trees_there + trees_counts[trees::DIED_COUNT] - trees_before;
            assert_eq!(trees_set, trees_counts[trees::SPREADS_COUNT], "tick {tick}: trees more, trees put");
            spread += trees_set;
        }
        before = Some((trees_there, report.rules.counts_of(trees_rule)));
    }
    assert!(spread > 0, "no tree spread");
}

/// The attributes of a sheep next hungry at `hungry_at`, in the middle of nothing.
fn hungry_at(hungry_at: u64) -> Vec<AttributeBlock> {
    WholeAttribute::holding_value(base().sheep_state, SheepState { hungry_at, ..SheepState::default() }).to_vec()
}

/// The attributes of a sheep next hungry at `hungry_at` whose lamb is
/// due at `due_at`.
fn carrying_lamb(hungry_at: u64, due_at: u64) -> Vec<AttributeBlock> {
    WholeAttribute::holding_value(base().sheep_state, SheepState { hungry_at, lamb_due_at: due_at, ..SheepState::default() }).to_vec()
}

/// The lambs and the pregnant sheep of the turn, as the tick found them.
fn lambs_and_pregnant(tick_state: &mut server::simulation::simulation_api::SimulationTickState) -> Vec<(usize, usize)> {
    let states: Vec<SheepState> = entities_of_the_turn(tick_state).iter().map(|sheep_entity| sheep_entity.attribute(base().sheep_state).expect("a sheep's state, always")).collect();
    vec![(states.iter().filter(|sheep_state| sheep_state.grown_at != 0).count(), states.iter().filter(|sheep_state| sheep_state.lamb_due_at != 0).count())]
}

/// Lambs and pregnant sheep added up over the turns.
fn summed(turns: Vec<(usize, usize)>) -> (usize, usize) {
    turns.into_iter().fold((0, 0), |(lambs, pregnant), (turn_lambs, turn_pregnant)| (lambs + turn_lambs, pregnant + turn_pregnant))
}

/// Two mothers on `(x, y)` and two cells east, whose lambs are due as
/// they wake, and a blocker asleep for good on every cell beside either
/// but `free_cells`, put in a tick of the probe alone; then a tick of
/// the sheep with nobody awake. The blockers.
fn mothers_among_blockers(world: &mut World, probe: &Probe, cell: &impl Fn(u32, u32) -> CellIndex, (x, y): (u32, u32), free_cells: &[(u32, u32)], hungry_after: u64) -> Vec<Header> {
    // Put the tick after the one read at; awake, and due, the tick after the one with nobody awake.
    let due_at = tick_read_at(world, probe) + 3;
    let mothers = [x, x + 2].map(|mother_x| (Header { entity_type: base().sheep, stood_cell: cell(mother_x, y), wake: due_at }, carrying_lamb(due_at + hungry_after, due_at)));
    let blockers: Vec<Header> = (x - 1..=x + 3).flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y))).filter(|beside_cell| !free_cells.contains(beside_cell)).map(|(x, y)| Header { entity_type: base().sheep, stood_cell: cell(x, y), wake: NEVER_WAKES }).collect();
    put_entities(world, probe, mothers.into_iter().chain(blockers.iter().map(|&blocker| (blocker, hungry_at(0)))).collect());
    assert_eq!(tick_sheep(world).rules[BIRTHS_COUNT], 0, "nobody awake");
    blockers
}

/// Two lambs put on one cell in a tick: the second is refused, and
/// its mother with it -- pregnant still, no birth counted that was
/// none -- and bears it once a cell beside her is free.
#[test]
fn a_lamb_refused_its_cell_is_born_later() {
    let (mut world, probe, cell) = one_superchunk("a_lamb_refused_its_cell_is_born_later", 0, 0, 1);
    let (x, y) = (500, 500);
    // Every cell beside either taken, but the one between them.
    let blocker_headers = mothers_among_blockers(&mut world, &probe, &cell, (x, y), &[(x, y), (x + 1, y), (x + 2, y)], 1);
    // Both bear on the one cell, neither with another to bear on: one lamb is made, its mother's the birth, in that tick; the other
    // is refused whole, and has no cell to try again on.
    let report = tick_sheep(&mut world);
    // As that tick left them, read in the next of the sheep -- which frees a cell beside each: the other's lamb is born, a step's wait on.
    let (next_report, after_births) = probe.tick_rule(&mut world, SHEEP_RULE, move |tick_state| {
        let turn_superchunk = tick_and_random_stream::superchunk(tick_state);
        for freed_blocker in blocker_headers.iter().filter(|freed_blocker| freed_blocker.stood_cell.superchunk() == turn_superchunk) {
            take_entity(tick_state, freed_blocker);
        }
        lambs_and_pregnant(tick_state)
    });
    assert_eq!((report.rules[BIRTHS_COUNT], report.instructions_applied.adds_refused, summed(after_births)), (1, 1, (1, 1)));
    let (mut births_counted, mut most_lambs) = (1 + next_report.rules[BIRTHS_COUNT], 1);
    for _ in 1..2 * (STEP.ticks() + STEP_JITTER.ticks()) {
        // Seen as it is born: a lamb may die of old age at its first wake.
        let (report, lambs_now) = probe.tick_rule(&mut world, SHEEP_RULE, lambs_and_pregnant);
        births_counted += report.rules[BIRTHS_COUNT];
        most_lambs = most_lambs.max(summed(lambs_now).0);
    }
    let (lambs_at_last, pregnant_at_last) = summed(probe.alone(&mut world, lambs_and_pregnant));
    assert_eq!((births_counted, most_lambs.max(lambs_at_last), pregnant_at_last), (2, 2, 0));
}

/// Two mothers bearing in one tick, two cells free between them:
/// whichever cell each drew, both lambs are born -- at once, or, the
/// two having drawn one cell, the second a step's wait on.
#[test]
fn a_lamb_whose_cell_was_taken_is_born_the_tick_after() {
    let (mut world, probe, cell) = one_superchunk("a_lamb_whose_cell_was_taken_is_born_the_tick_after", 0, 0, 1);
    let (x, y) = (500, 500);
    // Every cell beside either taken, but two between them. Not hungry for long: neither walks onto the cell the other's lamb is to have.
    mothers_among_blockers(&mut world, &probe, &cell, (x, y), &[(x, y), (x + 1, y), (x + 1, y + 1), (x + 2, y)], 5000);
    let report = tick_sheep(&mut world);
    // As that tick left them, read in the next of the sheep.
    let (next_report, after_births) = probe.tick_rule(&mut world, SHEEP_RULE, lambs_and_pregnant);
    let lambs_made = summed(after_births).0;
    // A lamb made is a birth counted, in the tick it is made in.
    assert_eq!((report.instructions_applied.adds_refused + lambs_made, report.rules[BIRTHS_COUNT] as usize), (2, lambs_made), "each made, or refused its cell");
    let births_counted: u64 = report.rules[BIRTHS_COUNT] + next_report.rules[BIRTHS_COUNT] + (1..2 * (STEP.ticks() + STEP_JITTER.ticks())).map(|_| tick_sheep(&mut world).rules[BIRTHS_COUNT]).sum::<u64>();
    assert_eq!(births_counted, 2);
    assert_eq!(summed(probe.alone(&mut world, lambs_and_pregnant)).1, 0, "no sheep pregnant still");
}
