//! What an ID is of: a plane ([`PlaneType`]), an entity type ([`EntityType`]), an attribute
//! ([`AttributeType`]) -- each a `u64` from the one namespace, in a type
//! of its own so that one is never passed for another.
//!
//! Every ID is laid out alike: its first byte its [`TypeKind`], its
//! number under it. A plane's width and tile size, and an
//! attribute's blocks, are their registration's, carried beside the ID
//! in its [`PlaneType`] or [`AttributeType`].

use utilities::bitmap::{BITS_PER_WORD, WORDS};

/// What an ID is of, kept in its first byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TypeKind {
    /// An attribute of an entity.
    Attribute = 0x00,
    /// A plane: a number a cell, or a tile.
    Plane = 0x01,
    /// A type of entity.
    EntityType = 0x02,
    /// A rule of the cells, the stochastic cell automaton's: run on the
    /// cells sampled.
    ScaRule = 0x03,
    /// A rule of entities: run on an entity of a type naming it, as it wakes.
    EntityRule = 0x04,
    /// A world generation: a world's superchunks' cells, and what
    /// lives on them.
    Worldgen = 0x05,
    /// A rule of the renderer: how something is drawn.
    RenderRule = 0x06,
    /// A window of the client's, its grid's contents declared: nested
    /// in another, or on its own.
    Gui = 0x07,
    /// A function a client may call across the connection: its code the
    /// function.
    ClientCall = 0x08,
    /// A bundle: what every other type is registered in, its name the
    /// first part of theirs -- in another bundle, or on its own.
    Bundle = 0x09,
}

/// Where an ID keeps its kind: its first byte, over its number.
const KIND_SHIFT: u32 = 56;
/// Numbers an ID can have: those of the seven bytes under its kind.
pub const TYPE_ID_NUMBERS: u64 = 1 << KIND_SHIFT;
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

/// Bits a number of a plane -- a cell's, or a tile's -- takes: one of six widths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum PlaneWidth {
    /// A bit: set or clear.
    #[default]
    Bits1 = 1,
    /// Numbers 0 to 3.
    Bits2 = 2,
    /// Numbers 0 to 15.
    Bits4 = 4,
    /// Numbers 0 to 255.
    Bits8 = 8,
    /// Numbers 0 to 65,535.
    Bits16 = 16,
    /// Numbers 0 to 4,294,967,295.
    Bits32 = 32,
}

impl PlaneWidth {
    /// Its bits.
    pub const fn bits(self) -> u32 {
        self as u32
    }

    /// The width of `bits` bits, if a plane can be so wide.
    pub const fn of_bits(bits: u32) -> Option<Self> {
        match bits {
            1 => Some(Self::Bits1),
            2 => Some(Self::Bits2),
            4 => Some(Self::Bits4),
            8 => Some(Self::Bits8),
            16 => Some(Self::Bits16),
            32 => Some(Self::Bits32),
            _ => None,
        }
    }
}

/// How big a plane's tiles are: it holds one number a tile, an aligned
/// square of cells `2^scale` a side -- 1 for a plane of cells, at most
/// 256, a chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum PlaneTileSize {
    /// A number a cell.
    #[default]
    Side1 = 0,
    /// A number a 2x2 tile.
    Side2 = 1,
    /// A number a 4x4 tile.
    Side4 = 2,
    /// A number an 8x8 tile.
    Side8 = 3,
    /// A number a 16x16 tile.
    Side16 = 4,
    /// A number a 32x32 tile.
    Side32 = 5,
    /// A number a 64x64 tile.
    Side64 = 6,
    /// A number a 128x128 tile.
    Side128 = 7,
    /// A number a 256x256 tile: a chunk.
    Side256 = 8,
}

impl PlaneTileSize {
    /// Its scale: a tile is `2^scale` cells a side.
    pub const fn scale(self) -> u32 {
        self as u32
    }

    /// Cells along a side of a tile.
    pub const fn side(self) -> u32 {
        1 << self.scale()
    }

    /// How many places a cell's place in its chunk, in Morton order, is
    /// shifted by to give its tile's: two a scale.
    pub const fn shift(self) -> u32 {
        2 * self.scale()
    }

    /// The tile size of tiles `side` cells a side, if a plane's tiles can
    /// be so big.
    pub const fn of_side(side: u32) -> Option<Self> {
        if side.is_power_of_two() { Self::of_scale(side.trailing_zeros()) } else { None }
    }

    /// The tile size of scale `scale`, if a plane's tiles can be so big.
    pub const fn of_scale(scale: u32) -> Option<Self> {
        match scale {
            0 => Some(Self::Side1),
            1 => Some(Self::Side2),
            2 => Some(Self::Side4),
            3 => Some(Self::Side8),
            4 => Some(Self::Side16),
            5 => Some(Self::Side32),
            6 => Some(Self::Side64),
            7 => Some(Self::Side128),
            8 => Some(Self::Side256),
            _ => None,
        }
    }
}

/// What a plane is: an ID the registry gives a meaning
/// ([`super::TypeRegistry`]), and the plane's shape as it was
/// registered -- its width and its tile size. Sorted by its ID first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlaneType {
    /// Its ID.
    type_id: u64,
    /// Bits a number.
    width: PlaneWidth,
    /// What a number is of: a cell, or a tile.
    tile_size: PlaneTileSize,
}

impl PlaneType {
    /// The plane of number `type_number`, a number of `width` a tile of
    /// `tile_size`.
    pub const fn new(type_number: u64, width: PlaneWidth, tile_size: PlaneTileSize) -> Self {
        Self { type_id: type_id(TypeKind::Plane, type_number), width, tile_size }
    }

    /// The plane of the ID `type_id_value`, of the shape said -- as it
    /// was registered, sent over the wire: none if no plane is so.
    pub fn of_id_and_shape(type_id_value: u64, width_bits: u32, tile_scale: u32) -> Option<Self> {
        let (width, tile_size) = (PlaneWidth::of_bits(width_bits)?, PlaneTileSize::of_scale(tile_scale)?);
        is_of(type_id_value, TypeKind::Plane).then_some(Self { type_id: type_id_value, width, tile_size })
    }

    /// Its ID: its kind and its number.
    pub const fn type_id(self) -> u64 {
        self.type_id
    }

    /// Bits a number.
    pub const fn width(self) -> PlaneWidth {
        self.width
    }

    /// What a number is of: a cell, or a tile.
    pub const fn tile_size(self) -> PlaneTileSize {
        self.tile_size
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.type_id)
    }

    /// Numbers a chunk of it holds: one a cell, or a tile.
    pub const fn numbers_in_chunk(self) -> usize {
        (WORDS * BITS_PER_WORD) >> self.tile_size.shift()
    }

    /// Words a chunk of it takes, its numbers one after another in
    /// Morton order: one at the least, the rest of it clear.
    pub const fn chunk_words(self) -> usize {
        let bits = self.numbers_in_chunk() * self.width.bits() as usize;
        if bits < BITS_PER_WORD { 1 } else { bits / BITS_PER_WORD }
    }

    /// Whether each chunk of it is a bitmap: a bit a cell -- one bitmap
    /// a chunk, hot and cold alike.
    pub const fn holds_bitmaps(self) -> bool {
        matches!((self.width, self.tile_size), (PlaneWidth::Bits1, PlaneTileSize::Side1))
    }
}

/// An entity's type: a type ID like a plane's, from the one `u64`
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
