//! A superchunk's turn in a tick's first phase: what a rule sees and
//! does there, and the outbox what it queues goes to. `entities` holds
//! the entities read and the instructions queued
//! (`docs/simulation.md`, "What a rule is given").

pub(crate) mod conditional;
mod entities;

use entity_manager::{EntityReader, Instructions, SuperchunkEntities};
use crate::sampling::sample_layer;
use bitplane_manager::{NotHot, Reader, Superchunk, Window};
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

/// The instructions and compare-and-writes a superchunk's rule queues in a tick, by
/// the superchunk they land in: itself, or one of its eight neighbours.
#[derive(Default)]
pub(crate) struct Outbox {
    /// Instructions, a queue a superchunk, by [`slot`].
    pub(crate) instructions: [Instructions; SLOTS],
    /// What is queued under compares, a queue a superchunk, by [`slot`].
    pub(crate) conditional: [conditional::Conditional; SLOTS],
    /// Where what each author queued begins in those queues, a list a
    /// superchunk, by [`slot`], in the order queued.
    pub(crate) segments: [Vec<Segment>; SLOTS],
}

/// Where what one author queued for a superchunk begins in the
/// outbox's queues for it: all up to the next segment is that
/// author's (`docs/simulation.md`, "One order, wherever the borders
/// fall").
#[derive(Clone, Copy)]
pub(crate) struct Segment {
    /// The author: its cell's place in reading order
    /// ([`Turn::seeing_to`]), or 0 for the superchunk's rule itself.
    pub(crate) author: u64,
    /// The first of its steps.
    pub(crate) steps: u32,
    /// The first of its instructions.
    pub(crate) instructions: u32,
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
    /// Whom the rule is seeing to ([`Turn::seeing_to`]): what is
    /// queued is that author's.
    pub(crate) author: u64,
    /// The compare the entity instructions being queued are under, if
    /// any, and how many each slot of the outbox held when those not
    /// yet made a step began.
    pub(crate) comparing: Option<(conditional::Compare, [u32; SLOTS])>,
    /// The number the rule's first count is counted under when
    /// applied ([`Turn::count_under`]).
    pub(crate) counted_from: u32,
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

    /// The tick running.
    pub fn now(&self) -> u64 {
        self.now
    }

    /// Says the rule is seeing to what is on the cell `at` from here
    /// on -- an entity woken, a cell sampled: the author of all it
    /// queues until another is named. What the tick queued is applied
    /// author by author in the reading order of their cells -- rows
    /// down, then cells across -- over the whole world, whichever
    /// superchunk each is in (`docs/simulation.md`, "One order,
    /// wherever the borders fall"). What is queued with no author
    /// named is the superchunk's rule's own, applied before any
    /// author's.
    pub fn seeing_to(&mut self, at: CellIndex) {
        self.close_instructions_compared();
        let at = at.cartesian();
        self.author = (u64::from(at.y) << 32 | u64::from(at.x)) + 1;
    }

    /// [`Turn::slot_of`], for something about to be queued there:
    /// begins the author's segment in that slot's queues, if the last
    /// begun there is another's.
    pub(crate) fn slot_queued(&mut self, superchunk: SuperchunkIndex) -> usize {
        let slot = self.slot_of(superchunk);
        if self.outbox.segments[slot].last().is_none_or(|last| last.author != self.author) {
            let (steps, instructions) = (self.outbox.conditional[slot].len() as u32, self.outbox.instructions[slot].len() as u32);
            self.outbox.segments[slot].push(Segment { author: self.author, steps, instructions });
        }
        slot
    }

    /// The outbox slot of `superchunk`: this one or a neighbour. Farther
    /// is past the speed of light, and a bug.
    pub(crate) fn slot_of(&self, superchunk: SuperchunkIndex) -> usize {
        slot_between(self.superchunk.index(), superchunk).unwrap_or_else(|| panic!("a write more than a superchunk away: past the speed of light"))
    }
}

/// The outbox slot, in the outbox of the superchunk `from`, of the
/// superchunk `to`: itself or a neighbour, none farther.
pub(crate) fn slot_between(from: SuperchunkIndex, to: SuperchunkIndex) -> Option<usize> {
    if to == from {
        return Some(slot(0, 0));
    }
    let ((x, y), (to_x, to_y)) = (from.cartesian(), to.cartesian());
    let (dx, dy) = (to_x as i64 - x as i64, to_y as i64 - y as i64);
    (dx.abs() <= 1 && dy.abs() <= 1).then(|| slot(dx as i32, dy as i32))
}
