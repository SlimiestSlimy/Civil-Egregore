//! What an ID is of: a layer ([`LayerType`]), a wide plane (wide plane,
//! of a width), an entity type ([`EntityType`]), an attribute
//! ([`AttributeType`]) -- each a `u64` from the one namespace, in a type
//! of its own so that one is never passed for another.
//!
//! Every ID is laid out alike: its first byte its [`TypeKind`], its
//! number under it. A plane's bits a cell and tile size, and an
//! attribute's blocks, are their registration's, carried beside the ID
//! in its [`LayerType`] or [`AttributeType`].

/// What an ID is of, kept in its first byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TypeKind {
    /// An attribute of an entity.
    Attribute = 0x00,
    /// A layer or a wide plane: a bitplane of cells.
    Layer = 0x01,
    /// A type of entity.
    EntityType = 0x02,
    /// A rule of the cells, the stochastic cell automaton's: its
    /// master a plane.
    ScaRule = 0x03,
    /// A rule of an entity: its master a type of entity.
    EntityRule = 0x04,
    /// A world generation: a world's superchunks' cells, and what
    /// lives on them.
    Worldgen = 0x05,
    /// A rule of the renderer: how its master -- a plane or a type of
    /// entity -- is drawn.
    RenderRule = 0x06,
    /// A window of the client's, its grid's contents declared: nested
    /// in another, or on its own.
    Gui = 0x07,
    /// A function a client may call across the connection: its library
    /// the function, its master the type it belongs to.
    ClientCall = 0x08,
    /// A bundle: what every other type is registered in, its name the
    /// first part of theirs -- in another bundle, or on its own.
    Bundle = 0x09,
    /// A shader: its library the code the GPU runs for one cell or one
    /// entity, as text -- data, the same headless or windowed.
    Shader = 0x0A,
}

/// Where an ID keeps its kind: its first byte, over its number.
const KIND_SHIFT: u32 = 56;
/// Numbers an ID can have: those of the seven bytes under its kind.
pub const TYPE_ID_NUMBERS: u64 = 1 << KIND_SHIFT;
/// The most a plane's tiles' scale is: tiles 8 cells a side.
const MOST_TILE_SCALE: u32 = 3;

/// The ID of number `type_number`, of `type_kind`.
const fn type_id(type_kind: TypeKind, type_number: u64) -> u64 {
    assert!(type_number < TYPE_ID_NUMBERS, "an ID's number is under 2^56");
    (type_kind as u64) << KIND_SHIFT | type_number
}

/// The number of `type_id_value`, under its kind.
const fn number_of(type_id_value: u64) -> u64 {
    type_id_value & (TYPE_ID_NUMBERS - 1)
}

/// Whether `type_id_value` is of `type_kind`.
const fn is_of(type_id_value: u64, type_kind: TypeKind) -> bool {
    type_id_value >> KIND_SHIFT == type_kind as u64
}

/// What a layer represents: an ID the registry gives a meaning
/// ([`super::TypeRegistry`]), and the plane's shape as it was
/// registered -- 1 to 32 bits a cell, or a tile `2^tile_scale` cells a
/// side. Sorted by its ID first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerType {
    /// Its ID.
    type_id: u64,
    /// Bits a cell, or a tile.
    width_bits: u8,
    /// Its tiles' scale: 0 for a plane of cells.
    tile_scale: u8,
}

impl LayerType {
    /// The layer of number `type_number`, a bit a cell.
    pub const fn of_a_bit(type_number: u64) -> Self {
        Self::of_width(type_number, 1)
    }

    /// The type of `width_bits` bits a cell -- 1, 2, 4, 8, 16 or 32 -- whose
    /// planes are the layers of the numbers `first_number` on.
    pub const fn of_width(first_number: u64, width_bits: u32) -> Self {
        Self::of_tiles(first_number, width_bits, 0)
    }

    /// The type of `width_bits` bits a tile, the tiles `2^tile_scale` cells
    /// a side: a number for every such tile of the world, not for
    /// every cell (`docs/type_registry.md`, "Tile size"). A scale of
    /// 0 is a plane of cells, as every plane was.
    pub const fn of_tiles(first_number: u64, width_bits: u32, tile_scale: u32) -> Self {
        assert!(width_bits.is_power_of_two() && width_bits <= 32, "a plane is 1, 2, 4, 8, 16 or 32 bits a cell");
        assert!(tile_scale <= MOST_TILE_SCALE, "a plane's tiles are at most 8 cells a side");
        Self { type_id: type_id(TypeKind::Layer, first_number), width_bits: width_bits as u8, tile_scale: tile_scale as u8 }
    }

    /// The plane of the ID `type_id_value`, of the shape said -- as it
    /// was registered, sent over the wire: none if no plane is so.
    pub fn of_id_and_shape(type_id_value: u64, width_bits: u32, tile_scale: u32) -> Option<Self> {
        (is_of(type_id_value, TypeKind::Layer) && width_bits.is_power_of_two() && width_bits <= 32 && tile_scale <= MOST_TILE_SCALE).then_some(Self { type_id: type_id_value, width_bits: width_bits as u8, tile_scale: tile_scale as u8 })
    }

    /// Its ID: its kind and its number.
    pub const fn type_id(self) -> u64 {
        self.type_id
    }

    /// Bits a cell -- or a tile: 1 unless the type is wide.
    pub const fn width_bits(self) -> u32 {
        self.width_bits as u32
    }

    /// Its tile size, as a scale: its tiles are `2^scale` cells a
    /// side. 0 for a plane of cells; 3 for one of 8 by 8 tiles.
    pub const fn tile_scale(self) -> u32 {
        self.tile_scale as u32
    }

    /// Cells along a side of one of its tiles: its tile size.
    pub const fn tile_side(self) -> u32 {
        1 << self.tile_scale()
    }

    /// How many places a cell's place in its chunk, in Morton order,
    /// is shifted by to give its tile's: two a scale.
    pub const fn tile_shift(self) -> u32 {
        2 * self.tile_scale()
    }

    /// Its number: a wide plane's first, its lowest bit's layer's.
    pub const fn type_number(self) -> u64 {
        number_of(self.type_id)
    }

    /// Whether it is a plane of cells a bit a cell: one bitmap a chunk,
    /// hot and cold alike.
    pub const fn is_a_bitplane(self) -> bool {
        self.width_bits == 1 && self.tile_scale == 0
    }
}

/// An entity's type: a type ID like a layer's, from the one `u64`
/// namespace every type in Civil Egregore is drawn from. It has no
/// size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityType(pub u64);

impl EntityType {
    /// The entity type of ID `type_id_value`, if it is one's.
    pub const fn of_id(type_id_value: u64) -> Option<Self> {
        if is_of(type_id_value, TypeKind::EntityType) { Some(Self(type_id_value)) } else { None }
    }

    /// The entity type of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::EntityType, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A rule of the cells (`docs/type_registry.md`, "Rules"): an ID
/// from the one namespace, of its own kind. It has no
/// size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScaRuleType(pub u64);

impl ScaRuleType {
    /// The rule of the cells of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::ScaRule, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A world generation: an ID from the one namespace, of its own kind.
/// It has no size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorldgenType(pub u64);

impl WorldgenType {
    /// The world generation of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::Worldgen, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A rule of the renderer: an ID from the one namespace, of its own
/// kind. It has no size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderRuleType(pub u64);

impl RenderRuleType {
    /// The rule of the renderer of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::RenderRule, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A window of the client's: an ID from the one namespace, of its own
/// kind. It has no size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GuiType(pub u64);

impl GuiType {
    /// The window of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::Gui, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A function a client may call across the connection: an ID from the
/// one namespace, of its own kind -- what a call names on the wire. It
/// has no size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientCallType(pub u64);

impl ClientCallType {
    /// The call of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::ClientCall, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A bundle: an ID from the one namespace, of its own kind. It has no
/// size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BundleType(pub u64);

impl BundleType {
    /// The bundle of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::Bundle, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A shader: an ID from the one namespace, of its own kind. It has no
/// size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShaderType(pub u64);

impl ShaderType {
    /// The shader of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::Shader, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A rule of an entity (`docs/type_registry.md`, "Rules"): an ID
/// from the one namespace, of its own kind. It has no
/// size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityRuleType(pub u64);

impl EntityRuleType {
    /// The rule of an entity of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::EntityRule, type_number))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// An attribute's type: an ID from the same namespace, of its own kind,
/// and how many blocks an attribute of it takes, as it was registered --
/// 1 to 16. One whose size varies is registered as one block, and each
/// attribute of it says its own length: an attribute's first word, its
/// header ([`AttributeType::header_word`]), says its number and its
/// blocks, so the first word of any attribute says how long it is, and
/// what tells two types apart is their IDs (`docs/type_registry.md`,
/// "Layouts"). Sorted by its ID first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AttributeType {
    /// Its ID.
    type_id: u64,
    /// Blocks an attribute of it takes: a fixed type's size, or the
    /// length of one attribute of a type whose size varies.
    block_count: u8,
}

/// Bits of an attribute's header word that are its blocks, less one.
const HEADER_BLOCK_BITS: u32 = 4;

impl AttributeType {
    /// The most blocks an attribute takes: what its header's last four
    /// bits hold.
    pub const MOST_BLOCKS: usize = 1 << HEADER_BLOCK_BITS;

    /// The type of number `type_number` whose attributes take `block_count`
    /// blocks, 1 to 16.
    pub const fn of_blocks(type_number: u64, block_count: usize) -> Self {
        assert!(block_count >= 1 && block_count <= Self::MOST_BLOCKS, "an attribute is 1 to 16 blocks");
        Self { type_id: type_id(TypeKind::Attribute, type_number), block_count: block_count as u8 }
    }

    /// The type of number `type_number` whose attributes vary in size:
    /// registered as one block, for each attribute of it to say its own.
    pub const fn of_varying_size(type_number: u64) -> Self {
        Self::of_blocks(type_number, 1)
    }

    /// The attribute's type an attribute's first word, `header_word`,
    /// says: its number and its blocks. Any word says one; whether it
    /// is an attribute's at all, [`AttributeType::is_an_attributes`].
    #[inline]
    pub const fn of_header_word(header_word: u64) -> Self {
        Self { type_id: header_word >> HEADER_BLOCK_BITS, block_count: (header_word & ((1 << HEADER_BLOCK_BITS) - 1)) as u8 + 1 }
    }

    /// What an attribute's first word holds: its number, and its blocks
    /// less one in the last four bits.
    #[inline]
    pub const fn header_word(self) -> u64 {
        self.type_id << HEADER_BLOCK_BITS | (self.block_count as u64 - 1)
    }

    /// It with `block_count` blocks: what the first word of an attribute
    /// of it so long says, if its size varies.
    pub const fn with_blocks(self, block_count: usize) -> Self {
        Self::of_blocks(self.type_number(), block_count)
    }

    /// Its ID: its kind and its number.
    pub const fn type_id(self) -> u64 {
        self.type_id
    }

    /// Blocks an attribute of it takes: a fixed type's size, or the
    /// length of the one attribute whose first word it is.
    #[inline]
    pub const fn block_count(self) -> usize {
        self.block_count as usize
    }

    /// What tells it from another type: its ID. Two attributes of one
    /// type whose size varies are the same by this, however long each is.
    #[inline]
    pub const fn type_identity(self) -> u64 {
        self.type_id
    }

    /// Whether it is an attribute's type at all: an ID of the
    /// attributes' kind, its number not 0 -- the one namespace being
    /// everything's, and 0 nobody's.
    #[inline]
    pub const fn is_an_attributes(self) -> bool {
        is_of(self.type_id, TypeKind::Attribute) && self.type_number() != 0
    }

    /// Its number, under its kind.
    pub const fn type_number(self) -> u64 {
        number_of(self.type_id)
    }
}
