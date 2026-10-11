//! What an ID is of: a layer ([`LayerType`]), a wide plane ([`Wide`],
//! of a [`Width`]), an entity type ([`EntityType`]), an attribute
//! ([`AttributeType`]) -- each a `u64` from the one namespace, in a type
//! of its own so that one is never passed for another.
//!
//! Every ID is laid out alike: its first byte its [`Supertype`], its
//! last its size -- bits a cell of a plane, blocks of an attribute --
//! and its number between (`docs/type_registry.md`, "The bytes of an ID").

/// What an ID is of, kept in its first byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Supertype {
    /// An attribute of an entity.
    Attribute = 0x00,
    /// A layer or a wide plane: a bitplane of cells.
    Layer = 0x01,
    /// A type of entity.
    EntityType = 0x02,
}

/// Where an ID keeps its supertype: its first byte, over its number.
const SUPERTYPE_SHIFT: u32 = 56;
/// Bits of an ID's last byte, its size, under its number.
const SIZE_BITS: u32 = 8;
/// The most an ID's last byte holds.
const SIZE_MOST: u64 = (1 << SIZE_BITS) - 1;
/// Numbers an ID can have: those of the six bytes between.
pub const NUMBERS: u64 = 1 << (SUPERTYPE_SHIFT - SIZE_BITS);

/// The ID of number `number`, of `supertype`, whose size is `size`.
const fn type_id(supertype: Supertype, number: u64, size: u64) -> u64 {
    assert!(number < NUMBERS && size <= SIZE_MOST, "an ID's number is under 2^48, its size a byte");
    (supertype as u64) << SUPERTYPE_SHIFT | number << SIZE_BITS | size
}

/// The number of `id`: what its row in the registry is written with.
const fn number_of(id: u64) -> u64 {
    (id >> SIZE_BITS) & (NUMBERS - 1)
}

/// The size `id` keeps in its last byte.
const fn size_of(id: u64) -> u64 {
    id & SIZE_MOST
}

/// Whether `id` is of `supertype`.
const fn is_of(id: u64, supertype: Supertype) -> bool {
    id >> SUPERTYPE_SHIFT == supertype as u64
}

/// What a layer represents: a `u64` the registry gives a meaning
/// ([`crate::REGISTRY`]). It may be wide, 2 to 16 bits a cell: its
/// last byte says how many (`docs/type_registry.md`, "Width").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerType(pub u64);

impl LayerType {
    /// The layer of number `number`, a bit a cell.
    pub const fn of_a_bit(number: u64) -> Self {
        Self::wide(number, 1)
    }

    /// The type of `bits` bits a cell -- 1, 2, 4, 8 or 16 -- whose
    /// planes are the layers of the numbers `first` on.
    pub const fn wide(first: u64, bits: u32) -> Self {
        assert!(bits.is_power_of_two() && bits <= 16, "a plane is 1, 2, 4, 8 or 16 bits a cell");
        Self(type_id(Supertype::Layer, first, bits as u64))
    }

    /// Bits a cell, its ID's last byte: 1 unless the type is wide.
    pub const fn bits(self) -> u32 {
        size_of(self.0) as u32
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
        Self { layer_type: LayerType::wide(first, W::BITS), width: std::marker::PhantomData }
    }

    /// Its layer type: what it is made hot, and found in the arena, by.
    pub const fn layer_type(self) -> LayerType {
        self.layer_type
    }

    /// The most a cell of it holds.
    pub const fn most(self) -> u32 {
        (1 << W::BITS) - 1
    }
}

/// An entity's type: a type ID like a layer's, from the one `u64`
/// namespace every type in Civil Egregore is drawn from. It has no
/// size: its last byte is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityType(pub u64);

impl EntityType {
    /// The entity type of number `number`.
    pub const fn of(number: u64) -> Self {
        Self(type_id(Supertype::EntityType, number, 0))
    }

    /// Its number.
    pub const fn number(self) -> u64 {
        number_of(self.0)
    }
}

/// An attribute's type, from the same namespace. Its last byte says
/// how many blocks an attribute of it takes -- none, and it is no
/// attribute's (`docs/type_registry.md`, "Layouts").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AttributeType(pub u64);

impl AttributeType {
    /// What its last byte holds if its size varies: its attribute's
    /// first block then says its block length.
    const SIZE_VARIES: u64 = SIZE_MOST;

    /// The type of number `number` whose attributes take `blocks`
    /// blocks, 1 to 16.
    pub const fn of_blocks(number: u64, blocks: usize) -> Self {
        assert!(blocks >= 1 && blocks <= crate::layouts::ATTRIBUTE_BLOCKS_MOST, "an attribute is 1 to 16 blocks");
        Self(type_id(Supertype::Attribute, number, blocks as u64))
    }

    /// The type of number `number` whose attributes vary in size.
    pub const fn of_varying_size(number: u64) -> Self {
        Self(type_id(Supertype::Attribute, number, Self::SIZE_VARIES))
    }

    /// Blocks an attribute of it takes, or none if its size varies:
    /// 0 for an ID that is no attribute's.
    #[inline]
    pub const fn blocks(self) -> Option<usize> {
        match size_of(self.0) {
            _ if !is_of(self.0, Supertype::Attribute) => Some(0),
            Self::SIZE_VARIES => None,
            blocks => Some(blocks as usize),
        }
    }

    /// Whether it is an attribute's type at all: its first byte the
    /// attributes', its last not 0 -- the one namespace being everything's.
    #[inline]
    pub const fn is_an_attributes(self) -> bool {
        is_of(self.0, Supertype::Attribute) && size_of(self.0) != 0
    }

    /// Its number: its ID without its first byte and its last, what
    /// its row in the registry is written with.
    pub const fn number(self) -> u64 {
        number_of(self.0)
    }
}
