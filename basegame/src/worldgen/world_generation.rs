//! The world generation's library ([`code`]), as the type
//! hooked into generating the world exposes it: each superchunk's
//! cells, and a flock on each queued on its turn the first tick it is
//! hot ([`flock`]) -- both as the world's worldgen parameters say ([`WorldGeneration`]);
//! and a seed with land about the middle, where the sheep start.

use crate::worldgen::{generate_area, has_land_about, WorldGeneration};
use crate::base_types::{BaseTypes, SheepState};
use crate::entity_rules::sheep::{STEP, TIME_BETWEEN_MEALS};
use crate::libraries::collision::Collision;
use std::sync::Arc;
use server::simulation::simulation_api::{cell_reads_and_writes, tick_and_random_stream, CellCartesian, CellIndex, GeneratedArea, WorldgenParameters, RandomSource, SimulationTickState, WholeAttribute, WorldgenCode, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE_CELL};
use std::collections::HashSet;

/// The world generation's library, as it is registered with
/// `base_types`: the cells generated on its planes, and the flocks made
/// of its sheep, through the collision plane.
pub fn code(base_types: BaseTypes) -> WorldgenCode {
    let collision = Collision { plane: base_types.collision };
    WorldgenCode {
        generate_cells: Arc::new(move |worldgen_parameters, seed, generated_area| generate_cells(&base_types, worldgen_parameters, seed, generated_area)),
        populate: Arc::new(move |tick_state, worldgen_parameters, seed| populate(&base_types, &collision, tick_state, worldgen_parameters, seed)),
        parameters_read,
        fit_to_start,
    }
}

/// Writes the cells of `generated_area`'s area of the world of `seed`, on
/// the planes of `base_types`, as `worldgen_parameters` say ([`generate_area`]).
fn generate_cells(base_types: &BaseTypes, worldgen_parameters: &WorldgenParameters, seed: u64, generated_area: &mut GeneratedArea) {
    generate_area(base_types, &WorldGeneration::of_worldgen_parameters(worldgen_parameters), seed, generated_area);
}

/// The worldgen parameters as [`WorldGeneration`] reads them.
fn parameters_read(worldgen_parameters: &WorldgenParameters) -> Vec<(String, u64)> {
    WorldGeneration::of_worldgen_parameters(worldgen_parameters).worldgen_parameters()
}

/// Whether the world of `seed` has land about the middle, where the
/// sheep start ([`has_land_about`]).
fn fit_to_start(worldgen_parameters: &WorldgenParameters, seed: u64) -> bool {
    has_land_about(seed, &WorldGeneration::of_worldgen_parameters(worldgen_parameters).land_shape, WORLD_MIDDLE_CELL)
}

/// A flock of as many sheep as `worldgen_parameters` say, queued on `tick_state`, its
/// superchunk's turn, drawn from a random stream of its own for the world of
/// `seed`: the same flock whenever it is made there -- of the sheep of
/// `base_types`, each made through `collision`.
fn populate(base_types: &BaseTypes, collision: &Collision, tick_state: &mut SimulationTickState, worldgen_parameters: &WorldgenParameters, seed: u64) {
    let sheep_count = WorldGeneration::of_worldgen_parameters(worldgen_parameters).sheep_per_superchunk as usize;
    if sheep_count > 0 {
        let mut superchunk_stream = RandomSource::for_stream(!seed, tick_state.superchunk().0);
        flock(base_types, tick_state, collision, sheep_count, &mut superchunk_stream);
    }
}

/// Queues on `tick_state`, its superchunk's turn, `count` grown sheep of
/// `base_types`, each some
/// way from its next meal, each on a cell of its own drawn from
/// `random_stream` -- at most half its cells' worth of them -- waking over the next
/// [`STEP`], each made through `collision`: under compares, as any
/// entity is. A cell seen held -- a tree's -- is drawn again; fewer are
/// made only where a superchunk has almost no free cell left, or one
/// is taken by the time it is applied.
pub fn flock(base_types: &BaseTypes, tick_state: &mut SimulationTickState, collision: &Collision, count: usize, random_stream: &mut RandomSource) {
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
        if !cells_drawn.insert(drawn_cell) || cell_reads_and_writes::layer_holds(tick_state, base_types.collision, drawn_cell) {
            continue;
        }
        // Woken the next tick at the soonest: a wake filed in a tick is for one after it.
        let (wake, hungry_at) = (STEP.part_drawn(random_stream).tick_after(tick_now + 1), TIME_BETWEEN_MEALS.part_drawn(random_stream).tick_after(tick_now));
        drop(collision.begin_making_entity(tick_state, base_types.sheep, drawn_cell, wake, &WholeAttribute::holding_value(base_types.sheep_state, SheepState { hungry_at, ..SheepState::default() })));
        made += 1;
    }
}
