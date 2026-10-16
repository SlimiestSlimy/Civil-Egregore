//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use chunk_storage::SuperchunkImage;
use coordinates::SuperchunkIndex;
use entity_manager::{set_attribute, Attribute, AttributeBlock, EntityRef, Header};
use server::{transient_data, World};
use std::path::PathBuf;

/// A folder of its own for the test `name`, emptied.
pub fn folder(name: &str) -> PathBuf {
    let folder = transient_data::saves().join("tests").join(name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// Every hot cell of grass and dirt, every entity with its attributes,
/// the tick, every random stream, every cold superchunk's image and
/// kept state, and the superchunks warming and cooling with when each is due: what
/// two worlds the same hold the same.
pub type Everything = (Vec<u64>, Vec<(Header, Vec<AttributeBlock>)>, u64, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, SuperchunkImage, Vec<u64>)>, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, u64)>);

/// [`Everything`] `world` holds.
pub fn everything(world: &World) -> Everything {
    let cells = world.info().layers.clone().into_iter().flat_map(|layer| world.arena().buckets_of(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
    let all = world.entities().iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect();
    let cold = world.cold().iter().map(|(&superchunk, words)| (superchunk, SuperchunkImage::clone(&world.storage().shared_image(superchunk).expect("a cold superchunk's image")), words.clone())).collect();
    (cells, all, world.entities().now(), world.simulation().random_states().collect(), cold, world.warming().collect(), world.cooling().collect())
}

/// A seed whose world has land about its origin, the `nth` such the
/// run's tests ask for: found from the crate's seed, rolled every few
/// runs (`utilities::seed`), so that nothing passes on one seed alone.
pub fn land_seed(nth: u64) -> u64 {
    worldgen::seed_with_land(utilities::seed::counted().wrapping_add(nth.wrapping_mul(1_000_003)), &worldgen::Shape::DEFAULT, coordinates::WORLD_MIDDLE)
}

/// A plain forced hot of `superchunks` superchunks -- a square of them
/// -- made by the server, with grass on about `grass_cells` cells of
/// each and `sheep` sheep on each, ticked on `threads` threads: a rule
/// tried alone, the terrain taking no part. Its seed is the seed
/// rotation's (`utilities::seed::counted`): where the grass and the
/// sheep lie and what every rule draws change as it is rolled.
pub fn plain_world(superchunks: u32, grass_cells: u64, sheep: usize, threads: usize) -> World {
    let cells = u64::from(coordinates::SUPERCHUNK_SIDE_CELLS).pow(2);
    let start = server::diagnostics::plain_world::plain_start(superchunks, grass_cells * worldgen::ONE / cells, sheep, threads);
    server::start(server::Start { seed: utilities::seed::counted(), ..start })
}

/// The top left superchunk of `world`'s hot ones.
pub fn first_superchunk(world: &World) -> SuperchunkIndex {
    world.arena().superchunk_indices().into_iter().min_by_key(|superchunk| (superchunk.cartesian().1, superchunk.cartesian().0)).expect("a hot superchunk")
}

/// Cells of grass over `world`'s hot superchunks.
pub fn cells_of_grass(world: &World) -> u64 {
    world.arena().superchunk_indices().into_iter().map(|superchunk| world.arena().superchunk_count(type_registry::GRASS, superchunk) as u64).sum()
}

/// Turns to grass the rectangle of cells `width` by `height` whose top
/// left is `at`, all of it on `world`'s hot superchunks.
pub fn plant_grass(world: &mut World, at: coordinates::CellCartesian, width: u8, height: u8) {
    use bitplane_manager::{Shape, Write, WriteOp};
    let shape = if (width, height) == (1, 1) { Shape::Cell } else { Shape::Rect { width, height } };
    let planted = world.write_cells(type_registry::GRASS, [Write { at: at.into(), op: WriteOp::Set, shape }]);
    assert_eq!(planted.missed, 0, "grass planted off the hot superchunks");
}

/// Puts an entity on `world`, whole: there before the next tick.
pub fn put_entity(world: &mut World, header: Header, attributes: &[AttributeBlock]) {
    world.put_entity(header, attributes);
}

/// A test's own attribute: a number a sheep is told from the others
/// by from a tick to the next -- an entity has no ID, and its cell
/// names it for a tick alone.
pub const TAG: Attribute<u64> = Attribute::new(1 << 20);

/// Gives every sheep of `world` a [`TAG`], each its own: taken off the
/// world and put back with it. A lamb born after has none.
pub fn tag_sheep(world: &mut World) {
    let sheep: Vec<(Header, Vec<AttributeBlock>)> = world.entities().iter().map(|sheep| (sheep.header, sheep.attributes.to_vec())).collect();
    for (tag, (header, mut attributes)) in sheep.into_iter().enumerate() {
        set_attribute(&mut attributes, TAG, tag as u64);
        assert_eq!(world.remove_entity(&header).removes, 1);
        assert_eq!(world.put_entity(header, &attributes).puts, 1);
    }
}

/// The [`TAG`] of `sheep`, if it was given one.
pub fn tag(sheep: &EntityRef) -> Option<u64> {
    sheep.attribute(TAG)
}

/// One tick of `rule` alone over `world`'s hot
/// superchunks, its halos left where they are: the tick's report, the
/// rule's counts alone in it.
pub fn tick_rule(world: &mut World, rule: server::RulePlace) -> simulation::TickReport<actions::RuleCounts> {
    let report = world.tick_only(server::Chosen::from_rules(&[rule]), false);
    simulation::TickReport { writes_applied: report.writes_applied, instructions_applied: report.instructions_applied, instructions_compared: report.instructions_compared, counted_when_applied: report.counted_when_applied, rules: report.rules.counts_of(rule), computing: report.computing, applying: report.applying, acts_over_a_border: report.acts_over_a_border }
}

/// One tick of the sheep's rule alone over `world`'s hot superchunks,
/// its halos left where they are.
pub fn tick_sheep(world: &mut World) -> simulation::TickReport<actions::RuleCounts> {
    tick_rule(world, server::SHEEP_RULE)
}

/// One tick of the grass's rule alone over `world`'s hot superchunks.
pub fn tick_grass(world: &mut World) -> simulation::TickReport<actions::RuleCounts> {
    tick_rule(world, server::GRASS_RULE)
}
