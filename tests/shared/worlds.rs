//! Worlds as the tiers' tests make and read them: a folder beside no
//! `main.rs`, so no test program of its own -- a module of each tier
//! that uses it (`#[path = "../shared/worlds.rs"]`).

// A tier uses what it needs of it.
#![allow(dead_code)]

use chunk_storage::SuperchunkImage;
use coordinates::SuperchunkIndex;
use entity_manager::{set_attribute, Attribute, AttributeBlock, EntityRef, Header};
use server::{World, WorldSize, WorldStart};
use std::path::PathBuf;

/// A folder of its own for the test `test_name`, emptied: in the
/// workspace's `transient_data/test_saves/`.
pub fn folder(test_name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("test_saves").join(test_name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// Every hot cell of grass and dirt, every entity with its attributes,
/// the tick, every random stream, every cold superchunk's image and
/// kept state, and the superchunks warming and cooling with when each is due: what
/// two worlds the same hold the same.
pub type WorldContents = (Vec<u64>, Vec<(Header, Vec<AttributeBlock>)>, u64, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, SuperchunkImage, Vec<u64>)>, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, u64)>);

/// [`WorldContents`] `world` holds.
pub fn world_contents(world: &World) -> WorldContents {
    let cells = world.world_info().layers.clone().into_iter().flat_map(|layer| world.bitmap_arena().buckets_of(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
    let entities_with_attributes = world.entity_store().iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect();
    let cold = world.cold().iter().map(|(&superchunk, words)| (superchunk, SuperchunkImage::clone(&world.chunk_storage().shared_image(superchunk).expect("a cold superchunk's image")), words.clone())).collect();
    (cells, entities_with_attributes, world.entity_store().current_tick(), world.world_simulation().random_states().collect(), cold, world.warming().collect(), world.cooling().collect())
}

/// How a plain test world is started -- a rule tried alone, the
/// terrain taking no part: a plain forced hot of at least
/// `superchunks` superchunks -- a square of them -- with grass on
/// `grass_cover` of [`worldgen::FRACTION_ONE`] of its cells and
/// `sheep_per_flock` sheep on each superchunk, on `thread_count` threads.
pub fn plain_start(superchunks: u32, grass_cover: u64, sheep_per_flock: usize, thread_count: usize) -> WorldStart {
    WorldStart { world_generation: worldgen::WorldGeneration::plain(grass_cover), world_size: WorldSize::SquareOfSide { side_superchunks: coordinates::square_side(superchunks), forced_hot: true }, thread_count: Some(thread_count), sheep_per_flock, ..WorldStart::default() }
}

/// The top left superchunk of `world`'s hot ones.
pub fn first_superchunk(world: &World) -> SuperchunkIndex {
    world.bitmap_arena().superchunk_indices().into_iter().min_by_key(|superchunk| (superchunk.cartesian().1, superchunk.cartesian().0)).expect("a hot superchunk")
}

/// The cells of `layer_type` on every hot chunk of `world`, as words: what
/// two of, a tick apart, tell what the tick did to the layer
/// ([`cells_set_and_cleared`]).
pub fn layer_words(world: &World, layer_type: type_registry::LayerType) -> Vec<(coordinates::ChunkIndex, Vec<u64>)> {
    world.bitmap_arena().buckets_of(layer_type).map(|(chunk, bucket)| (chunk, bucket.cells().to_vec())).collect()
}

/// The cells set and the cells cleared from `words_before` to `words_after`,
/// two [`layer_words`] of the same hot chunks.
pub fn cells_set_and_cleared(words_before: &[(coordinates::ChunkIndex, Vec<u64>)], words_after: &[(coordinates::ChunkIndex, Vec<u64>)]) -> (u64, u64) {
    assert_eq!(words_before.len(), words_after.len(), "the same chunks hot");
    let word_pairs = words_before.iter().zip(words_after).flat_map(|((chunk, before), (same_chunk, after))| {
        assert_eq!(chunk, same_chunk);
        before.iter().zip(after)
    });
    word_pairs.fold((0, 0), |(set, cleared), (&word_before, &word_after)| (set + u64::from((word_after & !word_before).count_ones()), cleared + u64::from((word_before & !word_after).count_ones())))
}

/// Cells of grass over `world`'s hot superchunks.
pub fn cells_of_grass(world: &World) -> u64 {
    world.bitmap_arena().superchunk_indices().into_iter().map(|superchunk| world.bitmap_arena().superchunk_count(base_types::GRASS, superchunk) as u64).sum()
}

/// Turns to grass the rectangle of cells `width` by `height` whose top
/// left is `top_left_cartesian`, all of it on `world`'s hot superchunks.
pub fn plant_grass(world: &mut World, top_left_cartesian: coordinates::CellCartesian, width: u8, height: u8) {
    use bitplane_manager::{WriteShape, Write, WriteOp};
    let write_shape = if (width, height) == (1, 1) { WriteShape::Cell } else { WriteShape::RectangleOfCells { width, height } };
    let planted_writes = world.write_cells(base_types::GRASS, [Write { anchor_cell: top_left_cartesian.into(), write_op: WriteOp::SetCell, write_shape }]);
    assert_eq!(planted_writes.missed, 0, "grass planted off the hot superchunks");
}

/// Puts an entity on `world`, whole, its cell held in the collision
/// plane -- as an entity that collides holds it: there before the next
/// tick.
pub fn add_entity(world: &mut World, header: Header, attributes: &[AttributeBlock]) -> entity_manager::InstructionsApplied {
    let applied = world.add_entity(header, attributes);
    if applied.added > 0 {
        world.write_cells(base_types::COLLISION, [bitplane_manager::Write::cell(header.stood_cell, bitplane_manager::WriteOp::SetCell)]);
    }
    applied
}

/// Takes `header`'s entity off `world`, the cell it held in the collision
/// plane let go.
pub fn remove_entity(world: &mut World, header: &Header) -> entity_manager::InstructionsApplied {
    let remove_applied = world.remove_entity(header);
    if remove_applied.entities_removed > 0 {
        world.write_cells(base_types::COLLISION, [bitplane_manager::Write::cell(header.stood_cell, bitplane_manager::WriteOp::ClearCell)]);
    }
    remove_applied
}

/// A test's own attribute: a number a sheep is told from the others
/// by from a tick to the next -- an entity has no ID, and its cell
/// names it for a tick alone.
pub const SHEEP_TAG: Attribute<u64> = Attribute::new(1 << 20);

/// Gives every sheep of `world` a [`SHEEP_TAG`], each its own: taken off the
/// world and put back with it. A lamb born after has none.
pub fn tag_sheep(world: &mut World) {
    let every_sheep: Vec<(Header, Vec<AttributeBlock>)> = world.entity_store().iter().map(|one_sheep| (one_sheep.header, one_sheep.attributes.to_vec())).collect();
    for (tag_number, (header, mut attributes)) in every_sheep.into_iter().enumerate() {
        set_attribute(&mut attributes, SHEEP_TAG, tag_number as u64);
        assert_eq!(world.remove_entity(&header).entities_removed, 1);
        assert_eq!(world.add_entity(header, &attributes).added, 1);
    }
}

/// The [`SHEEP_TAG`] of `tagged_sheep`, if it was given one.
pub fn tag_of(tagged_sheep: &EntityRef) -> Option<u64> {
    tagged_sheep.attribute(SHEEP_TAG)
}

/// One tick of the rule whose type's ID is `rule_id` alone over `world`'s hot
/// superchunks, its halos left where they are: the tick's report, the
/// rule's counts alone in it.
pub fn tick_rule(world: &mut World, rule_id: u64) -> simulation::TickReport<simulation::simulation_api::RuleCounts> {
    let rule = world.rule_place(rule_id);
    let report = world.tick_only(server::ChosenRules::from_rules(&[rule]), false);
    simulation::TickReport { writes_applied: report.writes_applied, instructions_applied: report.instructions_applied, instructions_compared: report.instructions_compared, counted_when_applied: report.counted_when_applied, rules: report.rules.counts_of(rule), computing_time: report.computing_time, applying_time: report.applying_time, acts_over_a_border: report.acts_over_a_border }
}

/// One tick of the sheep's rule alone over `world`'s hot superchunks,
/// its halos left where they are.
pub fn tick_sheep(world: &mut World) -> simulation::TickReport<simulation::simulation_api::RuleCounts> {
    tick_rule(world, base_types::RULE_OF_SHEEP.0)
}

/// One tick of the grass's rule alone over `world`'s hot superchunks.
pub fn tick_grass(world: &mut World) -> simulation::TickReport<simulation::simulation_api::RuleCounts> {
    tick_rule(world, base_types::RULE_OF_GRASS.0)
}
