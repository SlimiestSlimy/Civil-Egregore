//! A superchunk's turn in a tick's first phase: what a rule sees and
//! does there, and the outbox what it queues goes to.
//!
//! | file | what is in it |
//! |---|---|
//! | `mod` | the turn itself: its superchunk, its random numbers, sampling, cells read and writes queued; and the outbox |
//! | `entities` | the entities woken and read, and the instructions queued for them |
//!
//! A turn reads and queues, and no more: what a rule makes of it --
//! the cells about a cell, the way to one, an entity changed -- is
//! `../../../instructions`.

mod entities;

use entity_manager::{EntityReader, Instructions, SuperchunkEntities};
use crate::sampling::sample_layer;
use bitplane_manager::{NotHot, Reader, Shape, Superchunk, Window, Write, WriteQueues};
use chunk_storage::{LayerType, Wide, Width};
use coordinates::{CellIndex, SuperchunkIndex};
use utilities::chance::Chance;
use utilities::rng::Rng;

/// A superchunk's outbox slots: itself and its eight neighbours.
pub(crate) const SLOTS: usize = 9;

/// The slot of the superchunk `dx` across and `dy` down from the one
/// whose outbox it is.
pub(crate) fn slot(dx: i32, dy: i32) -> usize {
    ((dy + 1) * 3 + dx + 1) as usize
}

/// The writes and instructions a superchunk's rule queues in a tick, by
/// the superchunk they land in: itself, or one of its eight neighbours.
#[derive(Default)]
pub(crate) struct Outbox {
    /// Writes, a queue a superchunk, by [`slot`].
    pub(crate) writes: [WriteQueues; SLOTS],
    /// Instructions, a queue a superchunk, by [`slot`].
    pub(crate) instructions: [Instructions; SLOTS],
}

/// One superchunk's turn in a tick's first phase: what the rule sees and
/// does. It samples the superchunk's own cells, wakes its entities due,
/// reads any cell in reach, and queues writes and instructions, which
/// change nothing until the second phase.
pub struct Turn<'a> {
    /// The superchunk.
    pub(crate) superchunk: &'a Superchunk,
    /// The superchunk's entities.
    pub(crate) entities: &'a SuperchunkEntities,
    /// The tick running.
    pub(crate) now: u64,
    /// The thread's reader of every superchunk.
    pub(crate) reader: &'a Reader<'a>,
    /// The thread's reader of every superchunk's entities.
    pub(crate) entity_reader: &'a EntityReader<'a>,
    /// The superchunk's outbox.
    pub(crate) outbox: &'a mut Outbox,
    /// The superchunk's random numbers this tick.
    pub(crate) random: Rng,
}

impl<'a> Turn<'a> {
    /// The superchunk whose turn it is.
    pub fn superchunk(&self) -> SuperchunkIndex {
        self.superchunk.index()
    }

    /// The superchunk's random numbers: its own, going on from the last
    /// tick's.
    pub fn random(&mut self) -> &mut Rng {
        &mut self.random
    }

    /// Chooses each hot set cell of `layer_type` in this superchunk with
    /// `chance`, independently, into `samples` -- emptied first --
    /// in Morton order: how many.
    pub fn sample(&mut self, layer_type: LayerType, chance: Chance, samples: &mut Vec<CellIndex>) -> usize {
        samples.clear();
        let Some(layer) = self.superchunk.layer(layer_type) else {
            return 0;
        };
        sample_layer(self.superchunk.index(), layer, chance, &mut self.random, &mut |cell| samples.push(cell))
    }

    /// The window of `width` by `height` cells (each up to 8) whose top
    /// left cell is `origin`, of `layer_type`, row by row, as the tick
    /// found them: the cells around a cell, say, as masks.
    pub fn window(&self, layer_type: LayerType, origin: CellIndex, width: u32, height: u32) -> Window {
        self.reader.window(layer_type, origin, width, height)
    }

    /// [`Turn::window`], of each of `types` at once: grass and
    /// the cells entities stand on about a cell, say, for little more
    /// than either alone.
    pub fn windows<const N: usize>(&self, types: [LayerType; N], origin: CellIndex, width: u32, height: u32) -> [Window; N] {
        self.reader.windows(types, origin, width, height)
    }

    /// Whether `layer_type` holds at `cell`, as the tick found it.
    pub fn holds(&self, layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        self.reader.holds(layer_type, cell)
    }

    /// The number `plane` holds at `cell`, as the tick found it: one
    /// read, whatever its width -- which is its type's.
    #[inline]
    pub fn value<W: Width>(&self, plane: Wide<W>, cell: CellIndex) -> Result<u32, NotHot> {
        self.reader.value(plane, cell)
    }

    /// Whether `layer_type` holds at any cell of the tile of `scale`
    /// that `cell` is in, as the tick found it: `None` if not hot.
    pub fn any_in_tile(&self, layer_type: LayerType, cell: CellIndex, scale: u32) -> Option<bool> {
        self.reader.any_in_tile(layer_type, cell, scale)
    }

    /// Which tiles of the coarsest scale in `cell`'s chunk `layer_type`
    /// holds at any cell of, a bit each in Morton order, as the tick
    /// found them: `None` if not hot.
    pub fn tiles_holding(&self, layer_type: LayerType, cell: CellIndex) -> Option<u16> {
        self.reader.tiles_holding(layer_type, cell)
    }

    /// Queues `write` to `layer_type`'s bitplane, applied in the second
    /// phase by every superchunk it lands in. A write landing beyond the
    /// superchunks next to this one is past the speed of light, and a
    /// bug.
    pub fn queue(&mut self, layer_type: LayerType, write: Write) {
        if write.shape == Shape::Cell {
            let slot = self.slot_of({ write.at }.superchunk());
            self.outbox.writes[slot].push(layer_type, write);
            return;
        }
        for superchunk in write.superchunks() {
            let slot = self.slot_of(superchunk);
            self.outbox.writes[slot].push(layer_type, write);
        }
    }

    /// The tick running.
    pub fn now(&self) -> u64 {
        self.now
    }

    /// The outbox slot of `superchunk`: this one or a neighbour. Farther
    /// is past the speed of light, and a bug.
    fn slot_of(&self, superchunk: SuperchunkIndex) -> usize {
        if superchunk == self.superchunk.index() {
            return slot(0, 0);
        }
        let ((x, y), (to_x, to_y)) = (self.superchunk.index().cartesian(), superchunk.cartesian());
        let (dx, dy) = (to_x as i64 - x as i64, to_y as i64 - y as i64);
        assert!(dx.abs() <= 1 && dy.abs() <= 1, "a write {dx}, {dy} superchunks away: past the speed of light");
        slot(dx as i32, dy as i32)
    }
}
