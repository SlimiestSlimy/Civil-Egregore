//! The world generation's library ([`code`]), as the type hooked into
//! generating the world exposes it: set up for each world from its
//! config -- basegame's defaults where there is none ([`WorldGeneration`])
//! -- then each superchunk generated whole, its cells and a flock on it
//! ([`flock`]).

use crate::worldgen::{generate_area, WorldGeneration};
use crate::base_types::{BaseTypes, SheepState};
use crate::entity_rules::sheep::{STEP, TIME_BETWEEN_MEALS};
use std::path::Path;
use std::sync::Arc;
use server::simulation::simulation_api::{CellCartesian, CellIndex, GeneratingSuperchunk, RandomSource, SetUpWorldgen, Write, WriteOp, WritesApplied, WorldgenCode, SUPERCHUNK_SIDE_CELLS};

/// The world generation's library, as it is registered with
/// `base_types`: set up from a world's config in `config_folder`, each
/// superchunk generated on its planes, a flock of its sheep on it.
pub fn code(base_types: BaseTypes) -> WorldgenCode {
    WorldgenCode { set_up: Arc::new(move |config_folder: &Path| Ok(set_up(base_types, WorldGeneration::DEFAULT.read_config(config_folder)?))) }
}

/// The world generation `world_generation`, set up: every superchunk
/// generated on the planes of `base_types`.
pub fn set_up(base_types: BaseTypes, world_generation: WorldGeneration) -> SetUpWorldgen {
    SetUpWorldgen {
        generate: Arc::new(move |seed, generating| {
            generate_area(&base_types, &world_generation, seed, generating);
            let sheep_count = world_generation.sheep_per_superchunk as usize;
            if sheep_count > 0 {
                // A stream of its own for the world of the seed: the same flock whenever it is made there.
                let mut superchunk_stream = RandomSource::for_stream(!seed, generating.superchunk_index().0);
                flock(&base_types, generating, sheep_count, &mut superchunk_stream);
            }
        }),
    }
}

/// Makes on `generating` `count` grown sheep of `base_types`, each some
/// way from its next meal, each on a cell of its own drawn from
/// `random_stream` -- at most half its cells' worth of them -- first
/// waking over the [`STEP`] after the superchunk turns hot, each held on
/// the collision plane. A cell held -- a tree's, a sheep's -- is drawn
/// again; fewer are made only where a superchunk has almost no free cell
/// left.
pub fn flock(base_types: &BaseTypes, generating: &mut GeneratingSuperchunk, count: usize, random_stream: &mut RandomSource) {
    let CellCartesian { x: superchunk_left_x, y: superchunk_top_y } = generating.superchunk_index().top_left().cartesian();
    let side_in_cells = SUPERCHUNK_SIDE_CELLS as u64;
    assert!(count as u64 <= side_in_cells * side_in_cells / 2, "{count} sheep on a superchunk: too many to draw a cell each");
    let first_hot_tick = generating.first_hot_tick();
    let mut writes_applied = WritesApplied::default();
    let (mut made, mut draws_left) = (0, 16 * count as u64 + side_in_cells * side_in_cells);
    while made < count && draws_left > 0 {
        draws_left -= 1;
        let drawn_cell: CellIndex = CellCartesian { x: superchunk_left_x + random_stream.draw_below(side_in_cells) as u32, y: superchunk_top_y + random_stream.draw_below(side_in_cells) as u32 }.into();
        // A cell held -- a tree's, a sheep's -- is drawn again: a cell holds one sheep.
        if generating.cells().value_at(base_types.collision, drawn_cell) == Some(1) {
            continue;
        }
        let (first_wake, hungry_at) = (STEP.part_drawn(random_stream), TIME_BETWEEN_MEALS.part_drawn(random_stream).tick_after(first_hot_tick));
        if generating.new_entity(base_types.sheep, drawn_cell, first_wake).with(base_types.sheep_state, SheepState { hungry_at, ..SheepState::default() }).make() {
            generating.cells_mut().apply(base_types.collision, Write::cell(drawn_cell, WriteOp::SetCell), &mut writes_applied);
            made += 1;
        }
    }
}
