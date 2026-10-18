//! A wide plane, its width in its type ([`WidePlane`], of a [`Width`]):
//! a number a cell -- or a tile -- its bits kept as planes of a bit a
//! cell, read and written whole. The registry knows a plane by its
//! [`LayerType`] alone, the width said in it; this is what a rule
//! reads and writes it by.

use protocol::type_registry::{LayerType, RegisteredType, Registration};

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

    /// The plane `layer_type` is, if it is of its width.
    pub const fn of_layer_type(layer_type: LayerType) -> Option<Self> {
        if layer_type.width_bits() == W::BITS_A_CELL { Some(Self { layer_type, width: std::marker::PhantomData }) } else { None }
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

impl<W: Width> WidePlane<W> {
    /// Its registration, named `name`: a number of `W` a cell.
    pub fn registration(name: &str) -> Registration {
        Registration::wide_plane(name, W::BITS_A_CELL)
    }

    /// Its registration, named `name`: a number of `W` a tile
    /// `2^tile_scale` cells a side.
    pub fn registration_of_tiles(name: &str, tile_scale: u32) -> Registration {
        Registration::wide_plane_of_tiles(name, W::BITS_A_CELL, tile_scale)
    }

    /// The plane `registered` is, if it is a wide plane of `W`.
    pub fn registered(registered: &RegisteredType) -> Option<Self> {
        match registered {
            RegisteredType::WidePlane { layer_type, .. } => Self::of_layer_type(*layer_type),
            _ => None,
        }
    }
}
