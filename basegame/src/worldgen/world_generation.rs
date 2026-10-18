//! The world generation's library ([`WORLD_GENERATION`]), as the type
//! hooked into generating the world exposes it: each superchunk's
//! cells, and a flock on each queued on its turn the first tick it is
//! hot ([`flock`]) -- both as the world's numbers say ([`WorldGeneration`]);
//! the numbers off a window's sliders; a seed with land about the
//! middle, where the sheep start; the terrain asked of the server.

use crate::worldgen::terrain_asked::{drawn_heights_of, heights, map};
use crate::worldgen::{generate_area, has_land_about, WorldGeneration};
use crate::base_types::{SheepState, COLLISION, SHEEP, SHEEP_STATE};
use crate::entity_rules::sheep::{STEP, TIME_BETWEEN_MEALS};
use crate::libraries::collision::{Collision, COLLISION as COLLISION_LIBRARY};
use server::simulation::simulation_api::terrain::{DrawnHeightRange, Height, HeightsAsk, MapAnswer, MapAsk};
use server::simulation::simulation_api::{cell_reads_and_writes, tick_and_random_stream, CellCartesian, CellIndex, GeneratedArea, GenerationNumbers, RandomSource, SimulationTickState, WholeAttribute, WorldgenCode, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE_CELL};
use std::collections::HashSet;
use server::simulation::simulation_api::tuning::Tuning;

/// The world generation's library.
pub static WORLD_GENERATION: WorldgenCode = WorldgenCode { generate_cells, populate, numbers_read, numbers_of_tuning, fit_to_start, drawn_heights, heights: heights_asked, map: map_asked };

/// Writes the cells of `generated_area`'s area of the world of `seed`, as
/// `generation_numbers` say ([`generate_area`]).
fn generate_cells(generation_numbers: &GenerationNumbers, seed: u64, generated_area: &mut GeneratedArea) {
    generate_area(&WorldGeneration::of_numbers(generation_numbers), seed, generated_area);
}

/// The numbers as [`WorldGeneration`] reads them.
fn numbers_read(generation_numbers: &GenerationNumbers) -> Vec<(String, u64)> {
    WorldGeneration::of_numbers(generation_numbers).numbers()
}

/// The numbers a window's sliders say ([`WorldGeneration::from_tuning`]).
fn numbers_of_tuning(tuning: &Tuning) -> Vec<(String, u64)> {
    WorldGeneration::from_tuning(tuning).numbers()
}

/// Whether the world of `seed` has land about the middle, where the
/// sheep start ([`has_land_about`]).
fn fit_to_start(generation_numbers: &GenerationNumbers, seed: u64) -> bool {
    has_land_about(seed, &WorldGeneration::of_numbers(generation_numbers).land_shape, WORLD_MIDDLE_CELL)
}

/// The heights the world is drawn between ([`drawn_heights_of`]).
fn drawn_heights(generation_numbers: &GenerationNumbers) -> DrawnHeightRange {
    drawn_heights_of(&WorldGeneration::of_numbers(generation_numbers))
}

/// The heights `heights_ask` asks for, of the world of `seed` ([`heights`]).
fn heights_asked(generation_numbers: &GenerationNumbers, seed: u64, heights_ask: &HeightsAsk) -> Vec<Height> {
    heights(seed, &WorldGeneration::of_numbers(generation_numbers), heights_ask)
}

/// The map `map_ask` asks for, of the world of `seed` ([`map`]).
fn map_asked(generation_numbers: &GenerationNumbers, seed: u64, map_ask: MapAsk) -> MapAnswer {
    map(seed, &WorldGeneration::of_numbers(generation_numbers), map_ask)
}

/// A flock of as many sheep as `generation_numbers` say, queued on `tick_state`, its
/// superchunk's turn, drawn from a random stream of its own for the world of
/// `seed`: the same flock whenever it is made there.
fn populate(tick_state: &mut SimulationTickState, generation_numbers: &GenerationNumbers, seed: u64) {
    let sheep_count = WorldGeneration::of_numbers(generation_numbers).sheep_per_superchunk as usize;
    if sheep_count > 0 {
        let mut superchunk_stream = RandomSource::for_stream(!seed, tick_state.superchunk().0);
        flock(tick_state, &COLLISION_LIBRARY, sheep_count, &mut superchunk_stream);
    }
}

/// Queues on `tick_state`, its superchunk's turn, `count` grown sheep, each some
/// way from its next meal, each on a cell of its own drawn from
/// `random_stream` -- at most half its cells' worth of them -- waking over the next
/// [`STEP`], each made through `collision`: under compares, as any
/// entity is. A cell seen held -- a tree's -- is drawn again; fewer are
/// made only where a superchunk has almost no free cell left, or one
/// is taken by the time it is applied.
pub fn flock(tick_state: &mut SimulationTickState, collision: &Collision, count: usize, random_stream: &mut RandomSource) {
    let CellCartesian { x: superchunk_left_x, y: superchunk_top_y } = tick_state.superchunk().top_left().cartesian();
    let side_in_cells = SUPERCHUNK_SIDE_CELLS as u64;
    assert!(count as u64 <= side_in_cells * side_in_cells / 2, "{count} sheep on a superchunk: too many to draw a cell each");
    let tick_now = tick_and_random_stream::current_tick(tick_state);
    let mut cells_drawn = HashSet::with_capacity(count);
    let (mut made, mut draws_left) = (0, 16 * count as u64 + side_in_cells * side_in_cells);
    while made < count && draws_left > 0 {
        draws_left -= 1;
        let drawn_cell: CellIndex = CellCartesian { x: superchunk_left_x + random_stream.draw_below(side_in_cells) as u32, y: superchunk_top_y + random_stream.draw_below(side_in_cells) as u32 }.into();
        // A cell drawn twice, or held, is drawn again: a cell holds one sheep.
        if !cells_drawn.insert(drawn_cell) || cell_reads_and_writes::layer_holds(tick_state, COLLISION, drawn_cell) {
            continue;
        }
        // Woken the next tick at the soonest: a wake filed in a tick is for one after it.
        let (wake, hungry_at) = (STEP.part_drawn(random_stream).tick_after(tick_now + 1), TIME_BETWEEN_MEALS.part_drawn(random_stream).tick_after(tick_now));
        drop(collision.begin_making_entity(tick_state, SHEEP, drawn_cell, wake, &WholeAttribute::holding_value(SHEEP_STATE, SheepState { hungry_at, ..SheepState::default() })));
        made += 1;
    }
}
