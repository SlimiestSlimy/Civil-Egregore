//! The base game's types ([`BaseTypes`]): the handles its rules, its
//! libraries and its world generation name them by -- the layers and
//! planes of the cells, the type of entity and its attribute -- each
//! of the ID its world gave it as it was registered
//! (`crate::default_types`), never a fixed one; and what the numbers
//! some hold are made of (the climate's, a sheep's state), and how
//! wide they are ([`plane_words`]).


mod climate;
pub mod plane_words;
mod sheep_state;

pub use climate::{nearer_by_step, passes_a_slope, pressure_falls_by, Climate, Wind, DEGREES_A_WIND_CARRIES, FALL_THAT_FREES, HEAT_A_LEVEL_MORE_HELD, HIGHEST_PRESSURE, HOTTEST_TEMPERATURE, HUMIDITY_A_LEVEL_OF_CLOUD, HUMIDITY_A_LEVEL_SOONER, MOST_HUMID, PRESSURE_AT_REST, PRESSURE_GAP_AT_REST, RAINS_ABOVE_AT_THE_MOST, RAINS_ABOVE_IN_COLD_DRY_AIR, RISE_THAT_HALVES, STRONGEST_WIND, TILES_ABOUT};
pub use sheep_state::SheepState;

pub use server::simulation::simulation_api::terrain::Height;

use protocol::type_registry::{Attribute, Bits16, Bits32, Bits4, Bits8, EntityType, LayerType, WidePlane, Width};

/// The base game's types as a world registered them, each of the ID it
/// was given there: what everything of the base game that works on
/// them is made with, as it is registered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseTypes {
    /// Grass: what a superchunk is generated with in patches, where its
    /// ground is dry, and the cells' rules grow and the sheep eat. A
    /// cell without it is dirt, which has no layer.
    pub grass: LayerType,
    /// The cells a tree stands on: what a superchunk is generated with
    /// in patches of their own, and the trees' rule ages, fells and
    /// seeds.
    pub tree: LayerType,
    /// A tree's stage, 0 to [`OLDEST_TREE_STAGE`]: four bits a cell.
    pub tree_stage: WidePlane<Bits4>,
    /// A wall before a cell's step to the cell to its east: the layer
    /// of the cells no step leaves that way. A wall is one way's: the
    /// step back, the other cell's west, has its own -- a drop is
    /// stepped down and not back up.
    pub wall_east: LayerType,
    /// ...to its south.
    pub wall_south: LayerType,
    /// The collision plane: the cells something stands on that bars
    /// stepping there -- a tree, a sheep. 0 is free, 1 is taken: what
    /// a movement check asks, whatever the thing is. Who puts such a
    /// thing sets its cell, and clears it when the thing goes.
    pub collision: LayerType,
    /// ...to its north.
    pub wall_north: LayerType,
    /// ...to its west.
    pub wall_west: LayerType,
    /// All a sheep keeps: when it is next hungry, and what it is in the
    /// middle of -- carrying a lamb, bearing it, growing up, leaving
    /// thin pasture.
    pub sheep_state: Attribute<SheepState>,
    /// The sheep's type.
    pub sheep: EntityType,
    /// The cells under water, however deep: what a rule asks. How deep
    /// is [`BaseTypes::water_depth`].
    pub wet: LayerType,
    /// The clouds: a number a tile 8 by 8 cells, 0 no cloud to 15 the
    /// deepest shadow a cloud casts.
    pub cloud: WidePlane<Bits4>,
    /// The climate: thirty-two bits a tile 8 by 8 cells -- humidity,
    /// wind, temperature and pressure, a byte each, kept together: one
    /// read for all four.
    pub climate: WidePlane<Bits32>,
    /// How high the land of a tile 8 by 8 cells stands over the ocean,
    /// in heights, 255 at the most: what the air's moves are held
    /// against, read by the weather as any plane is.
    pub elevation: WidePlane<Bits8>,
    /// Every cell's height: sixteen bits a cell.
    pub height: WidePlane<Bits16>,
    /// How deep the water over a cell is: 0 on dry land, sixteen bits a
    /// cell.
    pub water_depth: WidePlane<Bits16>,
}

impl BaseTypes {
    /// The walls' layers, each the way of [`WALL_WAYS`] at its place.
    pub const fn walls(&self) -> [LayerType; 4] {
        [self.wall_east, self.wall_south, self.wall_west, self.wall_north]
    }
}

/// The neighbour each wall of [`BaseTypes::walls`] is towards: east,
/// south, west, north.
pub const WALL_WAYS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// The tile size of the weather's planes, as a scale: tiles 8 by 8
/// cells.
pub const WEATHER_TILE_SCALE: u32 = 3;
/// The depth from which water hides what is under it: a power of two.
pub const DEEP_WATER_DEPTH: u32 = 16;
/// The oldest stage a tree has: sixteen in all, what four bits hold.
pub const OLDEST_TREE_STAGE: u32 = u32::MAX >> (u32::BITS - Bits4::BITS_A_CELL);

/// The grass's rule's name.
pub const GRASS_RULE: &str = "grass rule";
/// The trees' rule's name.
pub const TREES_RULE: &str = "trees rule";
/// The sheep's rule's name.
pub const SHEEP_RULE: &str = "sheep rule";
/// The weather's rule's name.
pub const WEATHER_RULE: &str = "weather rule";
