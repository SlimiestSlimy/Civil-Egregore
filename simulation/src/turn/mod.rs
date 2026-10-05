//! A superchunk's turn in a tick's first phase: what a rule sees and
//! does there, and the outbox what it queues goes to.
//!
//! | file | what is in it |
//! |---|---|
//! | `mod` | the turn itself: its superchunk, its random numbers, sampling, the going over cells and entities, cells read and writes queued; and the outbox |
//! | `area` | the cells about a cell: the 3x3 around it, the area of 16x16, the tiles further off, the cells entities stand on |
//! | `entities` | the entities woken and read, and the instructions queued for them |

mod area;
mod entities;

pub use area::{Area, AREA_CENTRE, AREA_SIDE, FARTHEST_SCALE};

use crate::entity_store::{EntityReader, EntityRef, Instructions, SuperchunkEntities};
use crate::sampling::sample_layer;
use bitplane_manager::{NotHot, Reader, Shape, Superchunk, Window, Write, WriteQueues};
use chunk_storage::{LayerType, Wide, Width};
use coordinates::{CellIndex, SuperchunkIndex};
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
    /// `probability`, independently, into `samples` -- emptied first --
    /// in Morton order: how many.
    pub fn sample(&mut self, layer_type: LayerType, probability: f64, samples: &mut Vec<CellIndex>) -> usize {
        samples.clear();
        let Some(layer) = self.superchunk.layer(layer_type) else {
            return 0;
        };
        sample_layer(self.superchunk.index(), layer, probability, &mut self.random, &mut |cell| samples.push(cell))
    }

    /// Runs `each` on every cell [`Turn::sample`] chooses of
    /// `layer_type`, in Morton order, with what it counts: how many
    /// were chosen, and the counts. A rule of the cells is written for
    /// one cell; the going over them is here.
    #[inline]
    pub fn each_sampled<C: Default>(&mut self, layer_type: LayerType, probability: f64, samples: &mut Vec<CellIndex>, mut each: impl FnMut(&mut Self, CellIndex, &mut C)) -> (usize, C) {
        let (sampled, mut counts) = (self.sample(layer_type, probability, samples), C::default());
        for &cell in samples.iter() {
            each(self, cell, &mut counts);
        }
        (sampled, counts)
    }

    /// Runs `each` on every entity of the superchunk waking this tick
    /// ([`Turn::woken_reading`], of `layers`), with `state`: what the
    /// rule counts, and whatever it keeps from one entity to the next.
    /// A rule of the entities is written for one entity; the going
    /// over them is here.
    #[inline]
    pub fn each_woken<const N: usize, S>(&mut self, layers: [LayerType; N], state: &mut S, mut each: impl FnMut(&mut Self, EntityRef<'a>, &mut S)) {
        for entity in self.woken_reading(layers) {
            each(self, entity, state);
        }
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

    /// The number kept at `cell` over `planes`, a bit a bitplane, the
    /// lowest first, as the tick found it: for a number kept over
    /// separate layers, a read each -- the water's depth, for now. A
    /// wide plane holds its number in one ([`Turn::value`]).
    pub fn level<const N: usize>(&self, planes: [LayerType; N], cell: CellIndex) -> Result<u32, NotHot> {
        let mut level = 0;
        for (bit, plane) in planes.into_iter().enumerate() {
            level |= (self.holds(plane, cell)? as u32) << bit;
        }
        Ok(level)
    }

    /// The number `plane` holds at `cell`, as the tick found it: one
    /// read, whatever its width -- which is its type's.
    #[inline]
    pub fn value<W: Width>(&self, plane: Wide<W>, cell: CellIndex) -> Result<u32, NotHot> {
        self.reader.value(plane, cell)
    }

    /// Queues the write that makes `value` the number `plane` holds at
    /// `cell`.
    pub fn queue_value<W: Width>(&mut self, plane: Wide<W>, cell: CellIndex, value: u32) {
        self.queue(plane.layer_type(), Write::value(plane, cell, value));
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
