//! What an ID is of: a layer ([`LayerType`]), a wide plane ([`Wide`],
//! of a [`Width`]), an entity type ([`EntityType`]), an attribute
//! ([`AttributeType`]) -- each a `u64` from the one namespace, in a type
//! of its own so that one is never passed for another.

/// What a layer represents: a `u64` the registry gives a meaning
/// ([`crate::REGISTRY`]). It may be wide, 2 to 16 bits a cell
/// (`docs/type_registry.md`, "Width").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerType(pub u64);

impl LayerType {
    /// Where a type keeps how wide it is: the power of two, over its number.
    const WIDTH_SHIFT: u32 = 56;

    /// The type of `bits` bits a cell -- 1, 2, 4, 8 or 16 -- whose
    /// planes are the types `first` on; of one bit, `LayerType(first)`.
    pub const fn wide(first: u64, bits: u32) -> Self {
        assert!(bits.is_power_of_two() && bits <= 16 && first >> Self::WIDTH_SHIFT == 0, "a plane is 1, 2, 4, 8 or 16 bits a cell");
        Self(first | (bits.trailing_zeros() as u64) << Self::WIDTH_SHIFT)
    }

    /// Bits a cell: 1 unless the type is wide.
    pub const fn bits(self) -> u32 {
        1 << (self.0 >> Self::WIDTH_SHIFT)
    }

    /// The type of its plane `bit`, a bit a cell: what it is kept cold as.
    pub const fn plane(self, bit: u32) -> Self {
        Self((self.0 & ((1 << Self::WIDTH_SHIFT) - 1)) + bit as u64)
    }

    /// The types of its planes, the lowest bit's first.
    pub fn planes(self) -> impl Iterator<Item = Self> {
        (0..self.bits()).map(move |bit| self.plane(bit))
    }

    /// Whether `plane`, a type of a bit a cell, is one of its planes.
    pub const fn holds(self, plane: Self) -> bool {
        plane.0 >= self.plane(0).0 && plane.0 < self.plane(0).0 + self.bits() as u64
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
/// namespace every type in Civil Egregore is drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityType(pub u64);

/// An attribute's type, from the same namespace. Its top byte says how
/// many blocks an attribute of it takes -- none, and it is no
/// attribute's (`docs/type_registry.md`, "Layouts").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AttributeType(pub u64);

impl AttributeType {
    /// Where a type keeps how many blocks it takes: over its number.
    const BLOCKS_SHIFT: u32 = 56;
    /// What it keeps there if its size varies: its attribute's first
    /// block then says its block length.
    const SIZE_VARIES: u64 = 0xFF;

    /// The type of number `number` whose attributes take `blocks`
    /// blocks, 1 to 254.
    pub const fn of_blocks(number: u64, blocks: usize) -> Self {
        assert!(number >> Self::BLOCKS_SHIFT == 0 && blocks >= 1 && (blocks as u64) < Self::SIZE_VARIES, "an attribute is 1 to 254 blocks, its number under 2^56");
        Self(number | (blocks as u64) << Self::BLOCKS_SHIFT)
    }

    /// The type of number `number` whose attributes vary in size.
    pub const fn of_varying_size(number: u64) -> Self {
        assert!(number >> Self::BLOCKS_SHIFT == 0, "an attribute's number is under 2^56");
        Self(number | Self::SIZE_VARIES << Self::BLOCKS_SHIFT)
    }

    /// Blocks an attribute of it takes, or none if its size varies:
    /// 0 for an ID that is no attribute's.
    #[inline]
    pub const fn blocks(self) -> Option<usize> {
        match self.0 >> Self::BLOCKS_SHIFT {
            Self::SIZE_VARIES => None,
            blocks => Some(blocks as usize),
        }
    }

    /// Whether it is an attribute's type at all: an ID whose top byte
    /// is 0 is of something else, the one namespace being everything's.
    #[inline]
    pub const fn is_an_attributes(self) -> bool {
        self.0 >> Self::BLOCKS_SHIFT != 0
    }

    /// Its number: its ID without its top byte, what its row in the
    /// registry is written with.
    pub const fn number(self) -> u64 {
        self.0 & ((1 << Self::BLOCKS_SHIFT) - 1)
    }
}
