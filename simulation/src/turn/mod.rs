//! A superchunk's turn in a tick's first phase: what a rule sees and
//! does there, and the outbox what it queues goes to. `entities` holds
//! the entities read and the instructions queued
//! (`docs/simulation.md`, "What a rule is given").

pub(crate) mod apply;
pub(crate) mod conditional;
mod entities;
pub(crate) mod instruction;
pub(crate) mod landing;
pub(crate) mod one_instruction;

use entity_manager::{EntityReader, SuperchunkEntities};
use crate::sampling::sample_layer;
use bitplane_manager::{NotHot, Reader, SmallTiles, Superchunk, Window};
use chunk_storage::{LayerType, Wide, Width};
use coordinates::{CellIndex, SuperchunkIndex};
use utilities::chance::Chance;
use utilities::rng::Rng;

/// A superchunk's outbox slots: itself and its eight neighbours.
pub(crate) const SLOTS: usize = 9;
/// The outbox's queue after its slots': of the authors that queued
/// for more than one superchunk, each instruction saying where it
/// lands (`docs/simulation.md`, "An act over a border").
pub(crate) const ACROSS: usize = SLOTS;

/// The slot of the superchunk `dx` across and `dy` down from the one
/// whose outbox it is.
pub(crate) fn slot(dx: i32, dy: i32) -> usize {
    ((dy + 1) * 3 + dx + 1) as usize
}

/// The instructions a superchunk's rule queues in a tick, by the
/// superchunk they land in: itself, or one of its eight neighbours.
#[derive(Default)]
pub(crate) struct Outbox {
    /// The instructions, a queue a superchunk, by [`slot`], and
    /// after them the one of acts over a border ([`ACROSS`]).
    pub(crate) queues: [conditional::Queue; SLOTS + 1],
    /// Where what each author queued begins in those queues, a list a
    /// queue, in the order queued.
    pub(crate) segments: [Vec<Segment>; SLOTS + 1],
    /// Of each segment of the queue of acts over a border, where it
    /// lands and who has come to it.
    pub(crate) across: Vec<Across>,
    /// The entities named this tick by an author that is not the
    /// entity itself, each with that author: by the cell the tick
    /// found it on. Few: an author nearly always names itself. What
    /// is read when an entity crosses a border, to have what others
    /// queued for it follow it
    /// (`docs/simulation.md`, "An entity over a border").
    pub(crate) named_by_others: Vec<(CellIndex, u64)>,
}

/// What is kept of one author's act over a border, beside its
/// segment of the outbox's queue [`ACROSS`].
#[derive(Default)]
pub(crate) struct Across {
    /// The superchunks it lands in, a bit an outbox slot: its
    /// author's always.
    pub(crate) lands: u16,
    /// How many of those superchunks have come to it as they apply:
    /// it is applied by the last.
    pub(crate) arrived: std::sync::atomic::AtomicU8,
}

/// Where what one author queued for a superchunk begins in the
/// outbox's queues for it: all up to the next segment is that
/// author's (`docs/simulation.md`, "One order, wherever the borders
/// fall").
#[derive(Clone, Copy)]
pub(crate) struct Segment {
    /// The author: its cell's place in Morton order over the world,
    /// and one ([`Turn::seeing_to`]), or 0 for the superchunk's rule
    /// itself.
    pub(crate) author: u64,
    /// The first of its instructions.
    pub(crate) instructions: u32,
    /// The first of its compares, among what is noted of each beside
    /// the instructions.
    pub(crate) compares: u32,
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
    /// The compare the rule is queuing under, if any.
    pub(crate) comparing: Option<conditional::Compare>,
    /// The stack of compares open, the outermost first.
    pub(crate) compares_open: [conditional::CompareOpen; conditional::COMPARES_WITHIN],
    /// How many are open.
    pub(crate) compares_open_count: usize,
    /// Writes of cells queued this turn: what a compare closing
    /// counts those under it from.
    pub(crate) writes_queued: u32,
    /// The number the rule's first count is counted under when
    /// applied ([`Turn::count_under`]).
    pub(crate) counted_from: u32,
    /// Whether what the author queues goes to the queue of acts over
    /// a border: from the first thing it queues for a second
    /// superchunk, until another author is named.
    pub(crate) across: bool,
    /// Where the author began queuing, if it has: the one superchunk
    /// all it queued so far is for.
    pub(crate) author_began: Option<AuthorBegan>,
    /// The superchunk the author's last instruction landed in, the
    /// queue it went to and that superchunk's slot: the next for the
    /// same superchunk goes there with nothing looked up again.
    /// `NO_SUPERCHUNK` until the author queues, and after its
    /// instructions go across.
    pub(crate) landed_last: (SuperchunkIndex, u8, u8),
}

/// No superchunk: what [`Turn::landed_last`] holds before an author
/// queues anything.
pub(crate) const NO_SUPERCHUNK: SuperchunkIndex = SuperchunkIndex(u64::MAX);

/// Where an author began queuing: the slot of the superchunk, and
/// how much that slot's queue held before it -- all after is the
/// author's, until it queues for another superchunk.
#[derive(Clone, Copy)]
pub(crate) struct AuthorBegan {
    /// The outbox slot of the superchunk.
    pub(crate) slot: u8,
    /// The instructions its queue held.
    pub(crate) instructions: u32,
    /// The compares its queue held, by what is noted of each.
    pub(crate) gated: u32,
    /// The blocks its queue carried.
    pub(crate) carried: u32,
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

    /// The four small tiles, two by two, from the one that begins at
    /// `first`, of each of `types`, as the tick found them: those
    /// `wanted` has a cell in, the cells in Morton order -- what is
    /// about a cell as a word of bits, nothing turned.
    #[inline]
    pub fn small_tiles<const N: usize>(&self, types: [LayerType; N], first: CellIndex, wanted: u64) -> [SmallTiles; N] {
        self.reader.small_tiles(types, first, wanted)
    }

    /// The words of the aligned tile `side` cells a side (8 to a
    /// chunk's) that `cell` is in, of `layer_type`, as the tick found
    /// them, in Morton order: none if its bitmap is not hot.
    pub fn aligned_tile(&self, layer_type: LayerType, cell: CellIndex, side: u32) -> Option<&[u64]> {
        self.reader.aligned_tile(layer_type, cell, side)
    }

    /// The four aligned tiles `side` cells a side, two by two from the
    /// one beginning at `first`, of each of `types`, as the tick found
    /// them: each tile's words as they lie, none where not hot.
    #[inline]
    pub fn aligned_tiles_two_by_two<const N: usize>(&self, types: [LayerType; N], first: CellIndex, side: u32) -> [[Option<&[u64]>; 4]; N] {
        self.reader.aligned_tiles_two_by_two(types, first, side)
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
    /// author by author in the Morton order of their cells over the
    /// whole world -- an entity's name in the tick is its place --
    /// whichever superchunk each is in (`docs/simulation.md`, "One
    /// order, wherever the borders fall"). What is queued with no author
    /// named is the superchunk's rule's own, applied before any
    /// author's.
    pub fn seeing_to(&mut self, at: CellIndex) {
        self.compare_end();
        debug_assert_eq!(self.compares_open_count, 0, "an author named with a compare open");
        self.author = at.0 + 1;
        self.across = false;
        self.author_began = None;
        self.landed_last.0 = NO_SUPERCHUNK;
    }

    /// The queue of the outbox the author's next instruction goes
    /// to, landing in `superchunk`, and that superchunk's slot: the
    /// slot's own queue while all the author queued is for that one
    /// superchunk, the queue of acts over a border from the first
    /// thing it queues for a second ([`Turn::go_across`]), whatever
    /// the instruction. Begins the author's segment there, if the
    /// last begun is another's.
    #[inline]
    pub(crate) fn queue_landing_in(&mut self, superchunk: SuperchunkIndex) -> (usize, usize) {
        // Most of what an author queues lands where the last did: a compare, its write, its end.
        if superchunk == self.landed_last.0 {
            return (self.landed_last.1 as usize, self.landed_last.2 as usize);
        }
        let slot = self.slot_of(superchunk);
        if !self.across {
            match self.author_began {
                Some(began) if began.slot as usize != slot => self.go_across(began),
                Some(_) => {}
                None => {
                    let queue = &self.outbox.queues[slot];
                    self.author_began = Some(AuthorBegan { slot: slot as u8, instructions: queue.len() as u32, gated: queue.compares() as u32, carried: queue.carried.len() as u32 });
                }
            }
        }
        let queue = if self.across { ACROSS } else { slot };
        if self.outbox.segments[queue].last().is_none_or(|last| last.author != self.author) {
            let (instructions, compares) = (self.outbox.queues[queue].len() as u32, self.outbox.queues[queue].compares() as u32);
            self.outbox.segments[queue].push(Segment { author: self.author, instructions, compares });
            if queue == ACROSS {
                self.outbox.across.push(Across { lands: 1 << slot_home(), ..Across::default() });
            }
        }
        if queue == ACROSS {
            self.outbox.across.last_mut().expect("the segment's").lands |= 1 << slot;
        }
        self.landed_last = (superchunk, queue as u8, slot as u8);
        (queue, slot)
    }

    /// Makes what the author queues an act over a border: all it
    /// queued so far -- for the one superchunk it `began` in -- is
    /// moved to the queue of acts over a border, in the order queued,
    /// each instruction keeping where it lands; and what it queues
    /// from here goes there too. So nothing of one author is split
    /// between two superchunks' queues.
    #[cold]
    fn go_across(&mut self, began: AuthorBegan) {
        let from = began.slot as usize;
        let (slots, across) = self.outbox.queues.split_at_mut(ACROSS);
        let (was, is) = (&mut slots[from], &mut across[0]);
        let (instructions, gated, carried) = (is.instructions.len() as u32, is.gated.len() as u32, is.carried.len() as u32);
        if self.outbox.segments[ACROSS].last().is_none_or(|last| last.author != self.author) {
            self.outbox.segments[ACROSS].push(Segment { author: self.author, instructions, compares: gated });
            self.outbox.across.push(Across { lands: 1 << slot_home(), ..Across::default() });
        }
        self.outbox.across.last_mut().expect("the segment's").lands |= 1 << from;
        let moved = i64::from(carried) - i64::from(began.carried);
        is.instructions.extend(was.instructions.drain(began.instructions as usize..).map(|instruction| instruction.carried_moved_by(moved)));
        is.gated.extend(was.gated.drain(began.gated as usize..));
        is.carried.extend(was.carried.drain(began.carried as usize..));
        // A segment begun there for nothing but what was moved is no segment: the superchunk would stop at it between two acts.
        if self.outbox.segments[from].last().is_some_and(|last| last.author == self.author && last.instructions as usize == was.instructions.len()) {
            self.outbox.segments[from].pop();
        }
        // The compares open are the author's, so in the queue it began in: they lie where they were moved to.
        for open in &mut self.compares_open[..self.compares_open_count] {
            debug_assert_eq!(open.queue as usize, from);
            *open = conditional::CompareOpen { instruction: open.instruction - began.instructions + instructions, gated: open.gated - began.gated + gated, carried_before: open.carried_before - began.carried + carried, queue: ACROSS as u8, ..*open };
        }
        self.across = true;
        self.landed_last.0 = NO_SUPERCHUNK;
    }

    /// The outbox slot of `superchunk`: this one or a neighbour. Farther
    /// is past the speed of light, and a bug.
    pub(crate) fn slot_of(&self, superchunk: SuperchunkIndex) -> usize {
        slot_between(self.superchunk.index(), superchunk).unwrap_or_else(|| panic!("a write more than a superchunk away: past the speed of light"))
    }
}

/// The outbox slot of the superchunk whose outbox it is.
pub(crate) fn slot_home() -> usize {
    slot(0, 0)
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
