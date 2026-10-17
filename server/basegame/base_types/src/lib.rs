//! The base game's types: each a constant, the handle a rule names it
//! by, at its fixed number -- the layers and planes of the cells, the
//! types of entities and their attributes, the rules -- and what the
//! numbers some hold are made of (the climate's, a sheep's state).
//! Registered by `protocol::default_types`; nothing but the base
//! game's rules, and that, depends on it.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod climate;
mod sheep_state;

pub use climate::{nearer_by_step, passes_a_slope, pressure_falls_by, Climate, Wind, DEGREES_A_WIND_CARRIES, FALL_THAT_FREES, HEAT_A_LEVEL_MORE_HELD, HIGHEST_PRESSURE, HOTTEST_TEMPERATURE, HUMIDITY_A_LEVEL_OF_CLOUD, HUMIDITY_A_LEVEL_SOONER, MOST_HUMID, PRESSURE_AT_REST, PRESSURE_GAP_AT_REST, RAINS_ABOVE_AT_THE_MOST, RAINS_ABOVE_IN_COLD_DRY_AIR, RISE_THAT_HALVES, STRONGEST_WIND, TILES_ABOUT};
pub use sheep_state::SheepState;

use type_registry::{Attribute, Bits32, Bits4, Bits8, EntityRuleType, EntityType, LayerType, ScaRuleType, WidePlane, WorldgenType};

/// Grass: what a superchunk is generated with in patches, where its
/// ground is dry, and the cells' rules grow and the sheep eat. A cell
/// without it is dirt, which has no layer.
pub const GRASS: LayerType = LayerType::of_a_bit(2);
/// The cells a tree stands on: what a superchunk is generated with in
/// patches of their own, and the trees' rule ages, fells and seeds.
pub const TREE: LayerType = LayerType::of_a_bit(3);
/// A tree's stage, 0 to [`OLDEST_TREE_STAGE`]: a plane four bits a cell
/// wide, kept cold as the four layer types from 4 on, a bit each.
pub const TREE_STAGE: WidePlane<Bits4> = WidePlane::new(4);
/// A wall before a cell's step to the cell to its east: the layer of
/// the cells no step leaves that way. A wall is one way's: the step
/// back, the other cell's west, has its own -- a drop is stepped down
/// and not back up.
pub const WALL_EAST: LayerType = LayerType::of_a_bit(8);
/// ...to its south.
pub const WALL_SOUTH: LayerType = LayerType::of_a_bit(9);
/// The collision plane: the cells something stands on that bars
/// stepping there -- a tree. 0 is free, 1 is taken: what a
/// movement check asks, whatever the thing is. Who puts such a
/// thing sets its cell, and clears it when the thing goes.
pub const COLLISION: LayerType = LayerType::of_a_bit(10);
/// A wall before a cell's step to the cell to its north
/// ([`WALL_EAST`])...
pub const WALL_NORTH: LayerType = LayerType::of_a_bit(11);
/// ...to its west.
pub const WALL_WEST: LayerType = LayerType::of_a_bit(12);
/// The sheep's type.
pub const SHEEP: EntityType = EntityType::from_number(16);
/// All a sheep keeps: when it is next hungry, and what it is in the
/// middle of -- carrying a lamb, bearing it, growing up, leaving thin
/// pasture.
pub const SHEEP_STATE: Attribute<SheepState> = Attribute::new(17);
/// The cells under water, however deep: what a rule asks. How deep is
/// kept a map a chunk with water, in the superchunk's image
/// (`chunk_storage::SuperchunkImage::depth`).
pub const WET: LayerType = LayerType::of_a_bit(24);

/// The tile size of the weather's planes, as a scale: tiles 8 by 8
/// cells.
pub const WEATHER_TILE_SCALE: u32 = 3;
/// The clouds: a number a tile 8 by 8 cells, 0 no cloud to 15 the
/// deepest shadow a cloud casts.
pub const CLOUD: WidePlane<Bits4> = WidePlane::of_tiles(40, WEATHER_TILE_SCALE);
/// The climate: thirty-two bits a tile 8 by 8 cells -- humidity,
/// wind, temperature and pressure, a byte each, kept together: one
/// read for all four.
pub const CLIMATE: WidePlane<Bits32> = WidePlane::of_tiles(44, WEATHER_TILE_SCALE);
/// How high the land of a tile 8 by 8 cells stands over the ocean,
/// in heights, 255 at the most: what the air's moves are held
/// against, read by the weather as any plane is.
pub const ELEVATION: WidePlane<Bits8> = WidePlane::of_tiles(80, WEATHER_TILE_SCALE);

/// The grass's rule: its master the grass.
pub const RULE_OF_GRASS: ScaRuleType = ScaRuleType::from_number(32);
/// The trees' rule: its master the cells a tree stands on.
pub const RULE_OF_TREES: ScaRuleType = ScaRuleType::from_number(33);
/// The sheep's rule: its master the sheep.
pub const RULE_OF_SHEEP: EntityRuleType = EntityRuleType::from_number(34);
/// The weather's rule: its master the climate, whose tiles it samples.
pub const RULE_OF_WEATHER: ScaRuleType = ScaRuleType::from_number(35);
/// The world generation: terrain, what grows on it, and a flock on each
/// superchunk.
pub const WORLD_GENERATION: WorldgenType = WorldgenType::from_number(36);

/// The grass's rule's name: what it is found by.
pub const GRASS_RULE: &str = "grass rule";
/// The trees' rule's name.
pub const TREES_RULE: &str = "trees rule";
/// The sheep's rule's name.
pub const SHEEP_RULE: &str = "sheep rule";
/// The weather's rule's name.
pub const WEATHER_RULE: &str = "weather rule";

/// The oldest stage a tree has: sixteen in all.
pub const OLDEST_TREE_STAGE: u32 = TREE_STAGE.most_value();
