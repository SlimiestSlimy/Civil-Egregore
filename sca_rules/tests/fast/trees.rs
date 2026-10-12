//! Trees, on a plain the server makes: they grow a stage at a time to
//! the oldest and no further, spread once old enough and not where
//! they are crowded and as the air is humid, die of age only at the
//! oldest and of a harsh climate at any stage, and leave no stage
//! behind.

use crate::tests::{first_superchunk, plain_world, tick_rule};
use bitplane_manager::{Write, WriteOp};
use coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use sca_rules::trees::{DIED, GROWN, SAMPLED, SEEDS_FROM, SPREADS};
use server::World;
use crate::grass::put_climate;
use type_registry::{Climate, Wind, COLLISION, HOTTEST, MOST_HUMID, OLDEST_TREE_STAGE, PRESSURE_AT_REST, TREE, TREE_STAGE};
use utilities::rng::Rng;

/// Every cell of `world`'s first superchunk, in rows.
fn cells(world: &World) -> impl Iterator<Item = CellIndex> + use<> {
    let corner = first_superchunk(world).top_left().cartesian();
    (0..SUPERCHUNK_SIDE_CELLS).flat_map(move |y| (0..SUPERCHUNK_SIDE_CELLS).map(move |x| CellCartesian { x: corner.x + x, y: corner.y + y }.into()))
}

/// Puts a tree of the stage `stage` gives on each of `cells`.
fn plant(world: &mut World, cells: &[CellIndex], mut stage: impl FnMut() -> u32) {
    let put = world.write_cells(TREE, cells.iter().map(|&at| Write::cell(at, WriteOp::Set)));
    let aged = world.write_cells(TREE_STAGE.layer_type(), cells.iter().map(|&at| Write::value(TREE_STAGE, at, stage())));
    let barred = world.write_cells(COLLISION, cells.iter().map(|&at| Write::cell(at, WriteOp::Set)));
    assert_eq!((put.missed, aged.missed, barred.missed), (0, 0, 0), "trees planted off the hot superchunks");
}

/// The stage of every tree in `world`'s first superchunk -- and no
/// cell without a tree has a stage, and the collision plane holds the
/// cells trees stand on and no other.
fn stages(world: &World) -> Vec<(CellIndex, u32)> {
    let arena = world.arena();
    let mut stages = Vec::new();
    for cell in cells(world) {
        let stage = arena.value(TREE_STAGE, cell).expect("hot");
        assert_eq!(arena.holds(COLLISION, cell), arena.holds(TREE, cell), "a tree bars its cell, and nothing else does");
        match arena.holds(TREE, cell).expect("hot") {
            true => stages.push((cell, stage)),
            false => assert_eq!(stage, 0, "a stage where no tree is"),
        }
    }
    stages
}

/// The mildest climate: the most humid, the coldest.
const MILDEST: Climate = Climate { humidity: MOST_HUMID, wind: Wind::CALM, temperature: 0, pressure: PRESSURE_AT_REST };

/// Trees of every stage scattered thin, half of them the oldest --
/// so some die in so few ticks: each tick the trees are no
/// more than were put and no fewer than died; a tree only ever grows
/// one stage or stays; one gone was at the oldest; one new starts at
/// 0, put by one old enough -- and all of it happens.
#[test]
fn trees_grow_a_stage_at_a_time_spread_and_die_of_age() {
    let mut world = plain_world(1, 0, 0, 2);
    let mut random = Rng::new(utilities::seed::counted());
    // The mildest climate: no tree dies of it, and the air always lets one spread.
    put_climate(&mut world, MILDEST);
    let planted: Vec<CellIndex> = cells(&world).filter(|_| random.below(16) == 0).collect();
    plant(&mut world, &planted, || if random.below(2) == 0 { OLDEST_TREE_STAGE } else { random.below(u64::from(OLDEST_TREE_STAGE) + 1) as u32 });
    let mut before = stages(&world);
    let seeding = before.iter().filter(|(_, stage)| *stage >= SEEDS_FROM).count();
    assert!(seeding > 0, "trees old enough to spread");
    let mut all = [0; 4];
    for tick in 0..400 {
        let done = tick_rule(&mut world, server::TREES_RULE).rules;
        let after = stages(&world);
        let (mut grown, mut died, mut new) = (0, 0, 0);
        let mut was = before.iter().peekable();
        for &(cell, stage) in &after {
            // Those gone before this one: `stages` is in one order every time.
            while let Some(&&(gone, old)) = was.peek().filter(|(earlier, _)| earlier.cartesian().y < cell.cartesian().y || (earlier.cartesian().y == cell.cartesian().y && earlier.cartesian().x < cell.cartesian().x)) {
                assert_eq!(old, OLDEST_TREE_STAGE, "tick {tick}: the tree on {gone:?} died young");
                (died, _) = (died + 1, was.next());
            }
            match was.peek().filter(|(same, _)| *same == cell) {
                Some(&&(_, old)) => {
                    assert!(stage == old || stage == old + 1, "tick {tick}: a tree went from stage {old} to {stage}");
                    (grown, _) = (grown + u64::from(stage - old), was.next());
                }
                None => {
                    assert_eq!(stage, 0, "tick {tick}: a new tree of stage {stage}");
                    new += 1;
                }
            }
        }
        for &(gone, old) in was {
            assert_eq!(old, OLDEST_TREE_STAGE, "tick {tick}: the tree on {gone:?} died young");
            died += 1;
        }
        assert_eq!((grown, died), (done[GROWN], done[DIED]), "tick {tick}: grown and died, seen and counted");
        assert!(new <= done[SPREADS] && done[GROWN] + done[DIED] + done[SPREADS] <= done[SAMPLED], "tick {tick}: {new} new, {done:?}");
        for (sum, count) in all.iter_mut().zip([SAMPLED, SPREADS, GROWN, DIED]) {
            *sum += done[count];
        }
        before = after;
    }
    assert!(all.iter().all(|&count| count > 0), "sampled, spread, grown, died: {all:?}");
}

/// Trees on every cell: none ever spreads, however old -- each has
/// more about it than crowd it, and stays so as some die.
#[test]
fn crowded_trees_never_spread() {
    let mut world = plain_world(1, 0, 0, 2);
    let everywhere: Vec<CellIndex> = cells(&world).collect();
    plant(&mut world, &everywhere, || OLDEST_TREE_STAGE);
    let (mut sampled, mut died) = (0, 0);
    for tick in 0..300 {
        let done = tick_rule(&mut world, server::TREES_RULE).rules;
        assert_eq!(done[SPREADS], 0, "tick {tick}");
        (sampled, died) = (sampled + done[SAMPLED], died + done[DIED]);
    }
    assert!(sampled > 1_000 && died > 100, "{sampled} sampled, {died} died");
}

/// The climate over trees: young ones scattered thin, old enough to
/// spread and none at the oldest stage, so none dies of age. In the
/// mildest climate none dies and they spread; in the driest and
/// hottest they die young and none spreads; and in a hot climate of a
/// humidity drawn between they die less than in the harshest.
#[test]
fn trees_spread_as_the_air_is_humid_and_die_as_the_climate_is_harsh() {
    let mut random = Rng::new(utilities::seed::counted());
    let between = random.between(64, 192) as u32;
    let harshest = Climate { humidity: 0, temperature: HOTTEST, ..MILDEST };
    let [mild, some, harsh] = [MILDEST, Climate { humidity: between, ..harshest }, harshest].map(|climate| {
        let mut world = plain_world(1, 0, 0, 2);
        put_climate(&mut world, climate);
        let planted: Vec<CellIndex> = cells(&world).filter(|_| random.below(16) == 0).collect();
        plant(&mut world, &planted, || SEEDS_FROM);
        // Too few ticks for a tree to grow from there to the oldest stage.
        let ticks = 700;
        assert!(u64::from(OLDEST_TREE_STAGE - SEEDS_FROM) * sca_rules::trees::MEAN_TIME_TO_SAMPLE.ticks() > 8 * ticks);
        (0..ticks).map(|_| tick_rule(&mut world, server::TREES_RULE).rules).fold((0, 0), |sum, done| (sum.0 + done[SPREADS], sum.1 + done[DIED]))
    });
    assert!(mild.0 > 20 && mild.1 == 0, "the mildest: {mild:?} spread and died");
    assert!(harsh.0 == 0 && harsh.1 > 20, "the harshest: {harsh:?} spread and died");
    assert!(some.0 > 0 && some.0 < mild.0 && some.1 > 0 && some.1 < harsh.1, "humidity {between}, hot: {some:?} spread and died, between {mild:?} and {harsh:?}");
}
