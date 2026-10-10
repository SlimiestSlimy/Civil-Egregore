//! Trees, on a plain the server makes: they grow a stage at a time to
//! the oldest and no further, spread once old enough and not where
//! they are crowded, die only at the oldest, and leave no stage behind.

use crate::tests::{first_superchunk, plain_world, tick_rule};
use bitplane_manager::{Write, WriteOp};
use coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use sca_rules::trees::{DIED, GROWN, SAMPLED, SEEDS_FROM, SPREADS};
use server::World;
use type_registry::{OLDEST_TREE_STAGE, TREE, TREE_STAGE};
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
    assert_eq!((put.missed, aged.missed), (0, 0), "trees planted off the hot superchunks");
}

/// The stage of every tree in `world`'s first superchunk -- and no
/// cell without a tree has a stage.
fn stages(world: &World) -> Vec<(CellIndex, u32)> {
    let arena = world.arena();
    let mut stages = Vec::new();
    for cell in cells(world) {
        let stage = arena.value(TREE_STAGE, cell).expect("hot");
        match arena.holds(TREE, cell).expect("hot") {
            true => stages.push((cell, stage)),
            false => assert_eq!(stage, 0, "a stage where no tree is"),
        }
    }
    stages
}

/// Trees of every stage scattered thin: each tick the trees are no
/// more than were put and no fewer than died; a tree only ever grows
/// one stage or stays; one gone was at the oldest; one new starts at
/// 0, put by one old enough -- and all of it happens.
#[test]
fn trees_grow_a_stage_at_a_time_spread_and_die_of_age() {
    let mut world = plain_world(1, 0, 0, 2);
    let mut random = Rng::new(utilities::seed::counted());
    let planted: Vec<CellIndex> = cells(&world).filter(|_| random.below(16) == 0).collect();
    plant(&mut world, &planted, || random.below(u64::from(OLDEST_TREE_STAGE) + 1) as u32);
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
