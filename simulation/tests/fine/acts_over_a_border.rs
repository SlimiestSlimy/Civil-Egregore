//! An act over a border: what an author queues for more than one
//! superchunk is applied in one piece -- what is under a compare
//! whole or not at all -- in the one order of the world, however the superchunks wait on one
//! another (`docs/simulation.md`, "An act over a border").
//!
//! `cargo test --test fine acts_over_a_border`

use crate::tests::{cell, walker, world, STONE, TAG, WOKEN};
use bitplane_manager::BitmapArena;
use coordinates::{CellIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{AttributeBlock, Entities, NEVER};
use simulation::{Compare, Simulation, Turn};
use std::collections::BTreeSet;
use utilities::rng::Rng;

/// An entity beside a border -- any of the four that meet at the
/// corner of four superchunks, on either side of it, anywhere along
/// it: drawn from the seed -- makes another over it and counts that
/// on itself, under the one compare of the cell being free: free,
/// both happen; taken, neither -- and it sleeps either way.
#[test]
fn an_act_over_a_border_happens_whole_or_not_at_all() {
    let mut random = Rng::new(utilities::seed::counted());
    for taken in [false, true, false, true] {
        let (mut arena, mut entities) = world(2);
        // Along the border, and the two cells either side of it: which the maker stands on, and whether the border runs north to south.
        let (along, side, north_to_south) = (random.below(u64::from(2 * SUPERCHUNK_SIDE_CELLS)) as u32, random.below(2) as u32, random.below(2) == 0);
        let across = |side: u32| if north_to_south { cell(SUPERCHUNK_SIDE_CELLS - 1 + side, along) } else { cell(along, SUPERCHUNK_SIDE_CELLS - 1 + side) };
        let (maker, beside) = (walker(across(side), 0), across(1 - side));
        entities.queue_put(maker, &[]);
        if taken {
            entities.queue_put(walker(beside, NEVER), &[]);
        }
        entities.apply();
        let report = Simulation::new(2).tick(&mut arena, &mut entities, random.draw(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            for entity in turn.woken() {
                turn.seeing_to(entity.header.at);
                turn.make(walker(beside, NEVER), &[AttributeBlock::holding(TAG, 5)]);
                turn.set_attribute(&entity.header, WOKEN, None, 1);
                turn.made();
                turn.step(&entity.header, entity.header.at, NEVER);
            }
            0
        });
        assert_eq!(report.acts_over_a_border, 1, "taken: {taken}");
        let (made, itself) = (entities.get(beside).and_then(|made| made.attribute(TAG)), entities.get(maker.at).expect("where it stood"));
        assert_eq!((made, itself.attribute(WOKEN), itself.header.wake), if taken { (None, None, NEVER) } else { (Some(5), Some(1), NEVER) }, "taken: {taken}");
    }
}

/// The most cells each way from the corner four superchunks meet at
/// that the crowd stands on.
const CROWD: u32 = 12;
/// The most cells along each border from the corner that a line of
/// the crowd stands on.
const LINE: u32 = 200;
/// Ticks the crowd is run.
const TICKS: u64 = 31;

/// Where the crowd stands, drawn from `seed`: about half the cells of
/// a square about the corner of four superchunks, and of two lines
/// along the borders from it, a cell each side -- the square and the
/// lines as large as drawn, never less than half the most.
fn crowd(seed: u64) -> BTreeSet<(u32, u32)> {
    let (mut random, corner) = (Rng::new(seed), SUPERCHUNK_SIDE_CELLS);
    let (crowd, line) = (CROWD / 2 + random.below(u64::from(CROWD / 2) + 1) as u32, LINE / 2 + random.below(u64::from(LINE / 2) + 1) as u32);
    let square = (corner - crowd..corner + crowd).flat_map(|y| (corner - crowd..corner + crowd).map(move |x| (x, y)));
    let along = (corner..corner + line).flat_map(|along| [(corner - 1, along), (corner, along), (along, corner - 1), (along, corner)]);
    square.chain(along).collect::<BTreeSet<_>>().into_iter().filter(|_| random.below(2) == 0).collect()
}

/// One superchunk's turn of the crowd: each, one tick in three drawn
/// from `seed` by where it stands, flips the stone east of it, south of it, and
/// south-east past that, as it saw them -- the third only if the
/// second is still as seen, all only if the first is -- and counts on
/// itself, under the first compare too. A cell is three entities' to
/// flip, so of those acting in a tick the first in the world's order
/// has it and the others are refused: in up to four superchunks at
/// once.
fn flip_what_is_about(turn: &mut Turn, seed: u64) -> u64 {
    let now = turn.now();
    for entity in turn.woken() {
        let (header, at) = (entity.header, entity.header.at.cartesian());
        turn.seeing_to(header.at);
        if Rng::for_stream(seed ^ now, u64::from(at.y) << 32 | u64::from(at.x)).below(3) != 0 {
            turn.step(&header, header.at, now + 1);
            continue;
        }
        let seen = |turn: &Turn, x: u32, y: u32| (CellIndex::from(coordinates::CellCartesian { x, y }), u32::from(turn.holds(STONE, coordinates::CellCartesian { x, y }.into()) == Ok(true)));
        let (east, south, past) = (seen(turn, at.x + 1, at.y), seen(turn, at.x, at.y + 1), seen(turn, at.x + 1, at.y + 2));
        turn.compare_from_here(Compare::Cell { layer_type: STONE, at: east.0, seen: east.1 as u16 });
        turn.queue_if(Compare::Cell { layer_type: STONE, at: south.0, seen: south.1 as u16 }, STONE, past.0, past.1, 1 - past.1, None);
        turn.queue_seen(STONE, south.0, south.1, 1 - south.1, None);
        turn.queue_seen(STONE, east.0, east.1, 1 - east.1, None);
        let counted = entity.attribute(WOKEN);
        turn.set_attribute(&header, WOKEN, counted, counted.unwrap_or(0) + 1);
        turn.compare_end();
        turn.step(&header, header.at, now + 1);
    }
    0
}

/// What came of a crowd: the cells of stone about it, and each of it
/// by its cell with what it counted.
type Came = (Vec<(u32, u32)>, Vec<(u64, Option<u64>)>);

/// What came of the crowd: the cells of stone about it, and what each
/// of it counted.
fn came(arena: &BitmapArena, entities: &Entities) -> Came {
    let about = SUPERCHUNK_SIDE_CELLS - CROWD - 2..SUPERCHUNK_SIDE_CELLS + LINE + 4;
    let stone = about.clone().flat_map(|y| about.clone().map(move |x| (x, y))).filter(|&(x, y)| arena.holds(STONE, coordinates::CellCartesian { x: cell(0, 0).cartesian().x + x, y: cell(0, 0).cartesian().y + y }.into()) == Ok(true)).collect();
    let mut counted: Vec<_> = entities.iter().map(|entity| (entity.header.at.0, entity.attribute(WOKEN))).collect();
    counted.sort_unstable();
    (stone, counted)
}

/// The worst there is for superchunks waiting on one another -- a
/// crowd drawn from the seed about the corner of four and along their
/// borders, those of it by a border acting over it, up to three
/// contending for each cell -- ends, and the same on one thread as on
/// any number: every cell of stone and every count.
#[test]
fn a_crowd_acting_over_every_border_comes_to_the_same_on_any_threads() {
    let (mut first, seed) = (None, utilities::seed::counted());
    for threads in [1, 2, 3, 4, 16] {
        let (mut arena, mut entities) = world(2);
        let crowd = crowd(seed);
        for &(x, y) in &crowd {
            entities.queue_put(walker(cell(x, y), 0), &[]);
        }
        entities.apply();
        let mut simulation = Simulation::new(threads);
        let (mut over_a_border, mut refused) = (0, 0);
        for _ in 0..TICKS {
            let report = simulation.tick(&mut arena, &mut entities, seed, |turn: &mut Turn, _: &mut Vec<CellIndex>| flip_what_is_about(turn, seed));
            over_a_border += report.acts_over_a_border;
            refused += report.instructions_compared.1;
        }
        // Of those by a border a third act over it every tick: more acts than there are of the crowd, whatever was drawn.
        assert!(over_a_border > crowd.len() as u64 && refused > 0, "seed {seed:#x}: {over_a_border} acts over a border by a crowd of {}, {refused} compares not holding", crowd.len());
        let came = (came(&arena, &entities), over_a_border, refused);
        // Stone is left, and most of the crowd counted an act of its own: those of the lines too, all of whose acts are over a border.
        let counting = came.0 .1.iter().filter(|counted| counted.1.is_some()).count();
        assert!(!came.0 .0.is_empty() && counting > came.0 .1.len() / 2, "seed {seed:#x}: {} cells of stone, {counting} of {} counting; {over_a_border} acts, {refused} refused", came.0 .0.len(), came.0 .1.len());
        match &first {
            None => first = Some(came),
            Some(first) => assert!(*first == came, "seed {seed:#x}, on {threads} threads: {} cells of stone and {} acts, on one {} and {}", came.0 .0.len(), came.1, first.0 .0.len(), first.1),
        }
    }
}
