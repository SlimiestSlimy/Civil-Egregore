//! What an ID is of: a layer ([`LayerType`]), a wide plane ([`WidePlane`],
//! of a [`Width`]), an entity type ([`EntityType`]), an attribute
//! ([`AttributeType`]) -- each a `u64` from the one namespace, in a type
//! of its own so that one is never passed for another.
//!
//! Every ID is laid out alike: its first byte its [`TypeKind`], its
//! last its size -- bits a cell of a plane, blocks of an attribute --
//! and its number between (`docs/type_registry.md`, "The bytes of an ID").

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
pub const TYPE_ID_NUMBERS: u64 = 1 << (KIND_SHIFT - SIZE_BITS);

/// The ID of number `type_number`, of `type_kind`, whose size is `size_byte`.
const fn type_id(type_kind: TypeKind, type_number: u64, size_byte: u64) -> u64 {
    assert!(type_number < TYPE_ID_NUMBERS && size_byte <= SIZE_MOST, "an ID's number is under 2^48, its size a byte");
    (type_kind as u64) << KIND_SHIFT | type_number << SIZE_BITS | size_byte
}

/// The number of `type_id_value`, between its kind and its size.
const fn number_of(type_id_value: u64) -> u64 {
    (type_id_value >> SIZE_BITS) & (TYPE_ID_NUMBERS - 1)
}

/// The size `type_id_value` keeps in its last byte.
const fn size_of(type_id_value: u64) -> u64 {
    type_id_value & SIZE_MOST
}

/// Whether `type_id_value` is of `type_kind`.
const fn is_of(type_id_value: u64, type_kind: TypeKind) -> bool {
    type_id_value >> KIND_SHIFT == type_kind as u64
}

/// What a layer represents: a `u64` the registry gives a meaning
/// ([`crate::TypeRegistry`]). It may be wide, 2 to 32 bits a cell: its
/// last byte says how many (`docs/type_registry.md`, "Width").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerType(pub u64);

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
        assert!(tile_scale < 1 << (SIZE_BITS - WIDTH_BITS), "a plane's tile size is said in two bits");
        Self(type_id(TypeKind::Layer, first_number, (tile_scale as u64) << WIDTH_BITS | width_bits as u64))
    }

    /// Bits a cell -- or a tile -- from its ID's last byte: 1 unless
    /// the type is wide.
    pub const fn width_bits(self) -> u32 {
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
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }

    /// The type of its plane `bit_index`, a bit a cell: what it is kept cold as.
    pub const fn bitplane_of(self, bit_index: u32) -> Self {
        Self::of_a_bit(self.type_number() + bit_index as u64)
    }

    /// The types of its planes, the lowest bit's first.
    pub fn each_bitplane(self) -> impl Iterator<Item = Self> {
        (0..self.width_bits()).map(move |bit_index| self.bitplane_of(bit_index))
    }

    /// Whether `asked_bitplane`, a type of a bit a cell, is one of its planes.
    pub const fn holds_bitplane(self, asked_bitplane: Self) -> bool {
        asked_bitplane.width_bits() == 1 && asked_bitplane.type_number() >= self.type_number() && asked_bitplane.type_number() < self.type_number() + self.width_bits() as u64
    }
}

/// How wide a plane is, as a type: what a [`WidePlane`] plane is of, so that
/// its width is known where it is read and written, and a number of
/// one width is never read as another.
pub trait Width: Copy {
    /// Bits a cell.
    const BITS_A_CELL: u32;
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
    const BITS_A_CELL: u32 = 2;
}
impl Width for Bits4 {
    const BITS_A_CELL: u32 = 4;
}
impl Width for Bits8 {
    const BITS_A_CELL: u32 = 8;
}
impl Width for Bits16 {
    const BITS_A_CELL: u32 = 16;
}
impl Width for Bits32 {
    const BITS_A_CELL: u32 = 32;
}

/// A wide plane, its width in its type: a cell of it holds a number,
/// read and written whole. A layer of a bit a cell is a [`LayerType`]
/// alone, set or clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidePlane<W: Width> {
    /// Its layer type, which says its width too ([`LayerType::of_width`]).
    layer_type: LayerType,
    /// Its width.
    width: std::marker::PhantomData<W>,
}

impl<W: Width> WidePlane<W> {
    /// The plane whose bits are kept cold as the types `first_number` on.
    pub const fn new(first_number: u64) -> Self {
        Self::of_tiles(first_number, 0)
    }

    /// The plane of a number a tile, the tiles `2^tile_scale` cells a
    /// side ([`LayerType::of_tiles`]), whose bits are kept cold as
    /// the types `first_number` on.
    pub const fn of_tiles(first_number: u64, tile_scale: u32) -> Self {
        Self { layer_type: LayerType::of_tiles(first_number, W::BITS_A_CELL, tile_scale), width: std::marker::PhantomData }
    }

    /// Its layer type: what it is made hot, and found in the arena, by.
    pub const fn layer_type(self) -> LayerType {
        self.layer_type
    }

    /// The most a cell of it holds.
    pub const fn most_value(self) -> u32 {
        u32::MAX >> (u32::BITS - W::BITS_A_CELL)
    }
}

/// An entity's type: a type ID like a layer's, from the one `u64`
/// namespace every type in Civil Egregore is drawn from. It has no
/// size: its last byte is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityType(pub u64);

impl EntityType {
    /// The entity type of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::EntityType, type_number, 0))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A rule of the cells (`docs/type_registry.md`, "Rules"): an ID
/// from the one namespace, of its own kind. It has no size: its
/// last byte is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScaRuleType(pub u64);

impl ScaRuleType {
    /// The rule of the cells of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::ScaRule, type_number, 0))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
        number_of(self.0)
    }
}

/// A rule of an entity (`docs/type_registry.md`, "Rules"): an ID
/// from the one namespace, of its own kind. It has no size: its
/// last byte is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityRuleType(pub u64);

impl EntityRuleType {
    /// The rule of an entity of number `type_number`.
    pub const fn from_number(type_number: u64) -> Self {
        Self(type_id(TypeKind::EntityRule, type_number, 0))
    }

    /// Its number.
    pub const fn type_number(self) -> u64 {
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
    /// The type of number `type_number` whose attributes take `block_count`
    /// blocks, 1 to 16.
    pub const fn of_blocks(type_number: u64, block_count: usize) -> Self {
        assert!(type_number < ATTRIBUTE_NUMBERS && block_count >= 1 && block_count <= crate::layouts::ATTRIBUTE_BLOCKS_MOST, "an attribute's number is under 2^52, and it is 1 to 16 blocks");
        Self((TypeKind::Attribute as u64) << KIND_SHIFT | type_number << ATTRIBUTE_SIZE_BITS | (block_count as u64 - 1))
    }

    /// The type of number `type_number` whose attributes vary in size: its
    /// last four bits 0, for each attribute of it to say its own.
    pub const fn of_varying_size(type_number: u64) -> Self {
        Self::of_blocks(type_number, 1)
    }

    /// It with `block_count` blocks said in its last four bits: what the
    /// first word of an attribute of it so long holds, if its size
    /// varies.
    pub const fn with_blocks(self, block_count: usize) -> Self {
        Self::of_blocks(self.type_number(), block_count)
    }

    /// Blocks its last four bits say: a fixed type's size, or the
    /// length of the one attribute whose first word it is.
    #[inline]
    pub const fn block_count(self) -> usize {
        (self.0 & ((1 << ATTRIBUTE_SIZE_BITS) - 1)) as usize + 1
    }

    /// What tells it from another type: all of it but its blocks.
    /// Two attributes of one type whose size varies are the same by
    /// this, however long each is.
    #[inline]
    pub const fn type_identity(self) -> u64 {
        self.0 >> ATTRIBUTE_SIZE_BITS
    }

    /// Whether it is an attribute's type at all: its first byte the
    /// attributes', its number not 0 -- the one namespace being
    /// everything's, and 0 nobody's.
    #[inline]
    pub const fn is_an_attributes(self) -> bool {
        is_of(self.0, TypeKind::Attribute) && self.type_number() != 0
    }

    /// Its number, between its kind and its size.
    pub const fn type_number(self) -> u64 {
        (self.0 >> ATTRIBUTE_SIZE_BITS) & (ATTRIBUTE_NUMBERS - 1)
    }
}
