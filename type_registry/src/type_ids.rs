//! What an ID is of: a layer ([`LayerType`]), a wide plane ([`Wide`],
//! of a [`Width`]), an entity type ([`EntityType`]), an attribute
//! ([`AttributeType`]) -- each a `u64` from the one namespace, in a type
//! of its own so that one is never passed for another.
//!
//! Every ID is laid out alike: its first byte its [`Kind`], its
//! last its size -- bits a cell of a plane, blocks of an attribute --
//! and its number between (`docs/type_registry.md`, "The bytes of an ID").

/// What an ID is of, kept in its first byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
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
}

/// Where an ID keeps its kind: its first byte, over its number.
const KIND_SHIFT: u32 = 56;
/// Bits of a plane's ID's last byte that say its bits a cell; those
/// above say its tile size ([`LayerType::tile_scale`]).
const WIDTH_BITS: u32 = 6;
/// Bits of an ID's last byte, its size, under its number.
const SIZE_BITS: u32 = 8;
/// The most an ID's last byte holds.
const SIZE_MOST: u64 = (1 << SIZE_BITS) - 1;
/// Numbers an ID can have: those of the six bytes between.
pub const NUMBERS: u64 = 1 << (KIND_SHIFT - SIZE_BITS);

/// The ID of number `number`, of `kind`, whose size is `size`.
const fn type_id(kind: Kind, number: u64, size: u64) -> u64 {
    assert!(number < NUMBERS && size <= SIZE_MOST, "an ID's number is under 2^48, its size a byte");
    (kind as u64) << KIND_SHIFT | number << SIZE_BITS | size
}

/// The number of `id`: what its row in the registry is written with.
const fn number_of(id: u64) -> u64 {
    (id >> SIZE_BITS) & (NUMBERS - 1)
}

/// The size `id` keeps in its last byte.
const fn size_of(id: u64) -> u64 {
    id & SIZE_MOST
}

/// Whether `id` is of `kind`.
const fn is_of(id: u64, kind: Kind) -> bool {
    id >> KIND_SHIFT == kind as u64
}

/// What a layer represents: a `u64` the registry gives a meaning
/// ([`crate::REGISTRY`]). It may be wide, 2 to 32 bits a cell: its
/// last byte says how many (`docs/type_registry.md`, "Width").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerType(pub u64);

impl LayerType {
    /// The layer of number `number`, a bit a cell.
    pub const fn of_a_bit(number: u64) -> Self {
        Self::wide(number, 1)
    }

    /// The type of `bits` bits a cell -- 1, 2, 4, 8, 16 or 32 -- whose
    /// planes are the layers of the numbers `first` on.
    pub const fn wide(first: u64, bits: u32) -> Self {
        Self::of_tiles(first, bits, 0)
    }

    /// The type of `bits` bits a tile, the tiles `2^tile_scale` cells
    /// a side: a number for every such tile of the world, not for
    /// every cell (`docs/type_registry.md`, "Tile size"). A scale of
    /// 0 is a plane of cells, as every plane was.
    pub const fn of_tiles(first: u64, bits: u32, tile_scale: u32) -> Self {
        assert!(bits.is_power_of_two() && bits <= 32, "a plane is 1, 2, 4, 8, 16 or 32 bits a cell");
        assert!(tile_scale < 1 << (SIZE_BITS - WIDTH_BITS), "a plane's tile size is said in two bits");
        Self(type_id(Kind::Layer, first, (tile_scale as u64) << WIDTH_BITS | bits as u64))
    }

    /// Bits a cell -- or a tile -- from its ID's last byte: 1 unless
    /// the type is wide.
    pub const fn bits(self) -> u32 {
        (size_of(self.0) & ((1 << WIDTH_BITS) - 1)) as u32
    }

    /// Its tile size, as a scale: its tiles are `2^scale` cells a
    /// side. 0 for a plane of cells; 3 for one of 8 by 8 tiles.
    pub const fn tile_scale(self) -> u32 {
        (size_of(self.0) >> WIDTH_BITS) as u32
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
    pub const fn number(self) -> u64 {
        number_of(self.0)
    }

    /// The type of its plane `bit`, a bit a cell: what it is kept cold as.
    pub const fn plane(self, bit: u32) -> Self {
        Self::of_a_bit(self.number() + bit as u64)
    }

    /// The types of its planes, the lowest bit's first.
    pub fn planes(self) -> impl Iterator<Item = Self> {
        (0..self.bits()).map(move |bit| self.plane(bit))
    }

    /// Whether `plane`, a type of a bit a cell, is one of its planes.
    pub const fn holds(self, plane: Self) -> bool {
        plane.bits() == 1 && plane.number() >= self.number() && plane.number() < self.number() + self.bits() as u64
    }
}

/// How wide a plane is, as a type: what a [`Wide`] plane is of, so that
/// its width is known where it is read and written, and a number of
/// one width is never read as another.
pub trait Width: Copy {
    /// Bits a cell.
    const BITS: u32;
}

/// Two bits a cell: numbers 0 to 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits2;
/// Four bits a cell: numbers 0 to 15.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits4;
/// Eight bits a cell: numbers 0 to 255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits8;
/// Sixteen bits a cell: numbers 0 to 65,535.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits16;
/// Thirty-two bits a cell: numbers 0 to 4,294,967,295.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits32;

impl Width for Bits2 {
    const BITS: u32 = 2;
}
impl Width for Bits4 {
    const BITS: u32 = 4;
}
impl Width for Bits8 {
    const BITS: u32 = 8;
}
impl Width for Bits16 {
    const BITS: u32 = 16;
}
impl Width for Bits32 {
    const BITS: u32 = 32;
}

/// A wide plane, its width in its type: a cell of it holds a number,
/// read and written whole. A layer of a bit a cell is a [`LayerType`]
/// alone, set or clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wide<W: Width> {
    /// Its layer type, which says its width too ([`LayerType::wide`]).
    layer_type: LayerType,
    /// Its width.
    width: std::marker::PhantomData<W>,
}

impl<W: Width> Wide<W> {
    /// The plane whose bits are kept cold as the types `first` on.
    pub const fn new(first: u64) -> Self {
        Self::of_tiles(first, 0)
    }

    /// The plane of a number a tile, the tiles `2^tile_scale` cells a
    /// side ([`LayerType::of_tiles`]), whose bits are kept cold as
    /// the types `first` on.
    pub const fn of_tiles(first: u64, tile_scale: u32) -> Self {
        Self { layer_type: LayerType::of_tiles(first, W::BITS, tile_scale), width: std::marker::PhantomData }
    }

    /// Its layer type: what it is made hot, and found in the arena, by.
    pub const fn layer_type(self) -> LayerType {
        self.layer_type
    }

    /// The most a cell of it holds.
    pub const fn most(self) -> u32 {
        u32::MAX >> (u32::BITS - W::BITS)
    }
}

/// An entity's type: a type ID like a layer's, from the one `u64`
/// namespace every type in Civil Egregore is drawn from. It has no
/// size: its last byte is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityType(pub u64);

impl EntityType {
    /// The entity type of number `number`.
    pub const fn from_number(number: u64) -> Self {
        Self(type_id(Kind::EntityType, number, 0))
    }

    /// Its number.
    pub const fn number(self) -> u64 {
        number_of(self.0)
    }
}

/// A rule of the cells (`docs/type_registry.md`, "Rules"): an ID
/// from the one namespace, of its own kind. It has no size: its
/// last byte is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScaRuleType(pub u64);

impl ScaRuleType {
    /// The rule of the cells of number `number`.
    pub const fn from_number(number: u64) -> Self {
        Self(type_id(Kind::ScaRule, number, 0))
    }

    /// Its number.
    pub const fn number(self) -> u64 {
        number_of(self.0)
    }
}

/// A rule of an entity (`docs/type_registry.md`, "Rules"): an ID
/// from the one namespace, of its own kind. It has no size: its
/// last byte is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityRuleType(pub u64);

impl EntityRuleType {
    /// The rule of an entity of number `number`.
    pub const fn from_number(number: u64) -> Self {
        Self(type_id(Kind::EntityRule, number, 0))
    }

    /// Its number.
    pub const fn number(self) -> u64 {
        number_of(self.0)
    }
}

/// An attribute's type, from the same namespace, laid out its own
/// way under the kind: its number in the 52 bits after it, and
/// in its last four how many blocks an attribute of it takes, less
/// one -- 1 to 16. A type of a fixed size says its size there, set
/// as it is registered. One whose size varies is registered with the
/// four bits 0, and each attribute of it keeps its own length in
/// them: so the first word of any attribute says how long it is, and
/// what tells two types apart is all but those bits
/// (`docs/type_registry.md`, "Layouts").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AttributeType(pub u64);

/// Bits of an attribute's ID that are its blocks, less one.
const ATTRIBUTE_SIZE_BITS: u32 = 4;
/// Numbers an attribute can have: those of the 52 bits between.
pub const ATTRIBUTE_NUMBERS: u64 = 1 << (KIND_SHIFT - ATTRIBUTE_SIZE_BITS);

impl AttributeType {
    /// The type of number `number` whose attributes take `blocks`
    /// blocks, 1 to 16.
    pub const fn of_blocks(number: u64, blocks: usize) -> Self {
        assert!(number < ATTRIBUTE_NUMBERS && blocks >= 1 && blocks <= crate::layouts::ATTRIBUTE_BLOCKS_MOST, "an attribute's number is under 2^52, and it is 1 to 16 blocks");
        Self((Kind::Attribute as u64) << KIND_SHIFT | number << ATTRIBUTE_SIZE_BITS | (blocks as u64 - 1))
    }

    /// The type of number `number` whose attributes vary in size: its
    /// last four bits 0, for each attribute of it to say its own.
    pub const fn of_varying_size(number: u64) -> Self {
        Self::of_blocks(number, 1)
    }

    /// It with `blocks` blocks said in its last four bits: what the
    /// first word of an attribute of it so long holds, if its size
    /// varies.
    pub const fn with_blocks(self, blocks: usize) -> Self {
        Self::of_blocks(self.number(), blocks)
    }

    /// Blocks its last four bits say: a fixed type's size, or the
    /// length of the one attribute whose first word it is.
    #[inline]
    pub const fn blocks(self) -> usize {
        (self.0 & ((1 << ATTRIBUTE_SIZE_BITS) - 1)) as usize + 1
    }

    /// What tells it from another type: all of it but its blocks.
    /// Two attributes of one type whose size varies are the same by
    /// this, however long each is.
    #[inline]
    pub const fn identity(self) -> u64 {
        self.0 >> ATTRIBUTE_SIZE_BITS
    }

    /// Whether it is an attribute's type at all: its first byte the
    /// attributes', its number not 0 -- the one namespace being
    /// everything's, and 0 nobody's.
    #[inline]
    pub const fn is_an_attributes(self) -> bool {
        is_of(self.0, Kind::Attribute) && self.number() != 0
    }

    /// Its number: what its row in the registry is written with.
    pub const fn number(self) -> u64 {
        (self.0 >> ATTRIBUTE_SIZE_BITS) & (ATTRIBUTE_NUMBERS - 1)
    }
}
