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
use crate::sampling::{sample_layer, sample_tiles_of_layer};
use bitplane_manager::{NotHot, CellReader, SmallTiles, Superchunk, Window};
use chunk_storage::{LayerType, WidePlane, Width};
use coordinates::{CellIndex, SuperchunkIndex};
use utilities::chance::Chance;
use utilities::rng::RandomSource;

/// A superchunk's outbox slots: itself and its eight neighbours.
pub(crate) const SLOTS: usize = 9;
/// The outbox's queue after its slots': of the authors that queued
/// for more than one superchunk, each instruction saying where it
/// lands (`docs/simulation.md`, "An act over a border").
pub(crate) const ACROSS_QUEUE: usize = SLOTS;

/// The slot of the superchunk `dx` across and `dy` down from the one
/// whose outbox it is.
pub(crate) fn slot_toward(dx: i32, dy: i32) -> usize {
    ((dy + 1) * 3 + dx + 1) as usize
}

/// A superchunk's own place and its eight neighbours', each its
/// outbox slot and its offset, in the order of the slots.
pub(crate) fn slots() -> impl Iterator<Item = (usize, (i32, i32))> {
    (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (slot_toward(dx, dy), (dx, dy))))
}

/// The instructions a superchunk's rule queues in a tick, by the
/// superchunk they land in: itself, or one of its eight neighbours.
#[derive(Default)]
pub(crate) struct Outbox {
    /// The instructions, a queue a superchunk, by [`slot_toward`], and
    /// after them the one of acts over a border ([`ACROSS_QUEUE`]).
    pub(crate) instruction_queues: [conditional::InstructionQueue; SLOTS + 1],
    /// Where what each author queued begins in those queues, a list a
    /// queue, in the order queued.
    pub(crate) author_segments: [Vec<AuthorSegment>; SLOTS + 1],
    /// Of each segment of the queue of acts over a border, where it
    /// lands and who has come to it.
    pub(crate) across_acts: Vec<AcrossAct>,
    /// The entities named this tick by an author that is not the
    /// entity itself, each with that author: by the cell the tick
    /// found it on. Few: an author nearly always names itself. What
    /// is read when an entity crosses a border, to have what others
    /// queued for it follow it
    /// (`docs/simulation.md`, "An entity over a border").
    pub(crate) named_by_others: Vec<(CellIndex, u64)>,
}

/// What is kept of one author's act over a border, beside its
/// segment of the outbox's queue [`ACROSS_QUEUE`].
#[derive(Default)]
pub(crate) struct AcrossAct {
    /// The superchunks it lands in, a bit an outbox slot: its
    /// author's always.
    pub(crate) lands_in_slots: u16,
    /// How many of those superchunks have come to it as they apply:
    /// it is applied by the last.
    pub(crate) arrived_count: std::sync::atomic::AtomicU8,
}

/// Where what one author queued for a superchunk begins in the
/// outbox's queues for it: all up to the next segment is that
/// author's (`docs/simulation.md`, "One order, wherever the borders
/// fall").
#[derive(Clone, Copy)]
pub(crate) struct AuthorSegment {
    /// The author: its cell's place in Morton order over the world,
    /// and one ([`Turn::seeing_to`]), or 0 for the superchunk's rule
    /// itself.
    pub(crate) author: u64,
    /// The first of its instructions.
    pub(crate) instructions: u32,
    /// The first of its compares, among what is noted of each beside
    /// the instructions.
    pub(crate) first_compare: u32,
}

/// One superchunk's turn in a tick's first phase: what the rule sees and
/// does. It samples the superchunk's own cells, wakes its entities due,
/// reads any cell in reach, and queues writes and instructions, which
/// change nothing until the second phase.
pub struct Turn<'a> {
    /// The superchunk.
    pub(crate) superchunk: &'a Superchunk,
    /// The superchunk's entities.
    pub(crate) superchunk_entities: &'a SuperchunkEntities,
    /// The tick running.
    pub(crate) current_tick: u64,
    /// The thread's reader of every superchunk.
    pub(crate) bitmap_reader: &'a CellReader<'a>,
    /// The thread's reader of every superchunk's entities.
    pub(crate) entity_reader: &'a EntityReader<'a>,
    /// The superchunk's outbox.
    pub(crate) outbox: &'a mut Outbox,
    /// The superchunk's random numbers this tick.
    pub(crate) random_stream: RandomSource,
    /// Whom the rule is seeing to ([`Turn::seeing_to`]): what is
    /// queued is that author's.
    pub(crate) author: u64,
    /// Whether the rule is queuing under a compare given by
    /// [`Turn::compare_from_here`].
    pub(crate) is_comparing: bool,
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
    pub(crate) going_across: bool,
    /// Where the author began queuing, if it has: the one superchunk
    /// all it queued so far is for.
    pub(crate) author_began: Option<AuthorBegan>,
    /// The superchunk the author's last instruction landed in, the
    /// queue it went to and that superchunk's slot: the next for the
    /// same superchunk goes there with nothing looked up again.
    /// `NO_SUPERCHUNK` until the author queues, and after its
    /// instructions go across.
    pub(crate) landed_last: (SuperchunkIndex, u8, u8),
    /// Room for the cells sampled, its thread's: filled and gone over
    /// by [`Turn::each_sampled`], kept from a turn to the next.
    pub(crate) samples: &'a mut Vec<CellIndex>,
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
    pub(crate) gated_count: u32,
    /// The blocks its queue carried.
    pub(crate) carried_count: u32,
}

impl<'a> Turn<'a> {
    /// The turn of `superchunk` and its `superchunk_entities` in the tick `current_tick`,
    /// nothing queued yet: what it queues goes to `outbox`, what it
    /// draws comes from `random_stream`, what it samples is kept in `samples`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(superchunk: &'a Superchunk, superchunk_entities: &'a SuperchunkEntities, current_tick: u64, bitmap_reader: &'a CellReader<'a>, entity_reader: &'a EntityReader<'a>, outbox: &'a mut Outbox, random_stream: RandomSource, samples: &'a mut Vec<CellIndex>) -> Self {
        let compares_open = [conditional::CompareOpen::NONE_OPEN; conditional::COMPARES_WITHIN];
        Self { superchunk, superchunk_entities, current_tick, bitmap_reader, entity_reader, outbox, random_stream, author: 0, is_comparing: false, compares_open, compares_open_count: 0, writes_queued: 0, counted_from: 0, going_across: false, author_began: None, landed_last: (NO_SUPERCHUNK, 0, 0), samples }
    }

    /// The superchunk whose turn it is.
    pub fn superchunk(&self) -> SuperchunkIndex {
        self.superchunk.superchunk_index()
    }

    /// The superchunk's random numbers: its own, going on from the last
    /// tick's.
    pub fn random_stream(&mut self) -> &mut RandomSource {
        &mut self.random_stream
    }

    /// Runs `rule` on every hot set cell of `layer_type` in this
    /// superchunk chosen with `chance`, independently, in Morton
    /// order -- each named the author of what `rule` queues for it
    /// ([`Turn::seeing_to`]) -- with `counts`, what the rule counts:
    /// how many were chosen. A rule of the cells is the function on
    /// one cell sampled; drawing the cells and going over them is
    /// done here.
    #[inline]
    pub fn each_sampled<C>(&mut self, layer_type: LayerType, chance: Chance, counts: &mut C, rule: impl FnMut(&mut Self, CellIndex, &mut C)) -> usize {
        let mut samples = std::mem::take(self.samples);
        samples.clear();
        if let Some(layer) = self.superchunk.layer(layer_type) {
            sample_layer(self.superchunk.superchunk_index(), layer, chance, &mut self.random_stream, &mut |cell| samples.push(cell));
        }
        self.each_of(samples, counts, rule)
    }

    /// [`Turn::each_sampled`] on every tile of `layer_type` -- a plane
    /// of tiles -- in this superchunk's hot chunks chosen with
    /// `chance`, whatever number it holds: `rule` is handed the
    /// tile's first cell.
    #[inline]
    pub fn each_tile_sampled<C>(&mut self, layer_type: LayerType, chance: Chance, counts: &mut C, rule: impl FnMut(&mut Self, CellIndex, &mut C)) -> usize {
        let mut samples = std::mem::take(self.samples);
        samples.clear();
        if let Some(layer) = self.superchunk.layer(layer_type) {
            sample_tiles_of_layer(self.superchunk.superchunk_index(), layer, layer_type.tile_shift(), chance, &mut self.random_stream, &mut |cell| samples.push(cell));
        }
        self.each_of(samples, counts, rule)
    }

    /// Runs `rule` on each of `samples`, in their order, each named
    /// the author of what is queued for it, and keeps their room for
    /// the next sampling: how many they were.
    #[inline]
    fn each_of<C>(&mut self, samples: Vec<CellIndex>, counts: &mut C, mut rule: impl FnMut(&mut Self, CellIndex, &mut C)) -> usize {
        for &sampled_cell in &samples {
            self.seeing_to(sampled_cell);
            rule(self, sampled_cell, counts);
        }
        let sampled_count = samples.len();
        *self.samples = samples;
        sampled_count
    }

    /// The window of `width` by `height` cells (each up to 8) whose top
    /// left cell is `origin`, of `layer_type`, row by row, as the tick
    /// found them: the cells around a cell, say, as masks.
    pub fn window(&self, layer_type: LayerType, origin: CellIndex, width: u32, height: u32) -> Window {
        self.bitmap_reader.window(layer_type, origin, width, height)
    }

    /// [`Turn::window`], of each of `layer_types` at once: grass and
    /// the cells entities stand on about a cell, say, for little more
    /// than either alone.
    pub fn windows<const N: usize>(&self, layer_types: [LayerType; N], origin: CellIndex, width: u32, height: u32) -> [Window; N] {
        self.bitmap_reader.windows(layer_types, origin, width, height)
    }

    /// The four small tiles, two by two, from the one that begins at
    /// `first_tile_cell`, of each of `layer_types`, as the tick found them: those
    /// `wanted_cells` has a cell in, the cells in Morton order -- what is
    /// about a cell as a word of bits, nothing turned.
    #[inline]
    pub fn small_tiles<const N: usize>(&self, layer_types: [LayerType; N], first_tile_cell: CellIndex, wanted_cells: u64) -> [SmallTiles; N] {
        self.bitmap_reader.small_tiles(layer_types, first_tile_cell, wanted_cells)
    }

    /// The words of the aligned tile `side_in_cells` cells a side (8 to a
    /// chunk's) that `cell` is in, of `layer_type`, as the tick found
    /// them, in Morton order: none if its bitmap is not hot.
    pub fn aligned_tile(&self, layer_type: LayerType, cell: CellIndex, side_in_cells: u32) -> Option<&[u64]> {
        self.bitmap_reader.aligned_tile(layer_type, cell, side_in_cells)
    }

    /// The four aligned tiles `side_in_cells` cells a side, two by two from the
    /// one beginning at `first_tile_cell`, of each of `layer_types`, as the tick found
    /// them: each tile's words as they lie, none where not hot.
    #[inline]
    pub fn aligned_tiles_two_by_two<const N: usize>(&self, layer_types: [LayerType; N], first_tile_cell: CellIndex, side_in_cells: u32) -> [[Option<&[u64]>; 4]; N] {
        self.bitmap_reader.aligned_tiles_two_by_two(layer_types, first_tile_cell, side_in_cells)
    }

    /// Whether `layer_type` holds at `cell`, as the tick found it.
    pub fn layer_holds(&self, layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        self.bitmap_reader.layer_holds_cell(layer_type, cell)
    }

    /// The number `wide_plane` holds at `cell`, as the tick found it: one
    /// read, whatever its width -- which is its type's.
    #[inline]
    pub fn value<W: Width>(&self, wide_plane: WidePlane<W>, cell: CellIndex) -> Result<u32, NotHot> {
        self.bitmap_reader.value(wide_plane, cell)
    }

    /// Whether `layer_type` holds at any cell of the tile of `scale`
    /// that `cell` is in, as the tick found it: `None` if not hot.
    pub fn any_in_tile(&self, layer_type: LayerType, cell: CellIndex, scale: u32) -> Option<bool> {
        self.bitmap_reader.any_in_tile(layer_type, cell, scale)
    }

    /// Which tiles of the coarsest scale in `cell`'s chunk `layer_type`
    /// holds at any cell of, a bit each in Morton order, as the tick
    /// found them: `None` if not hot.
    pub fn tiles_holding(&self, layer_type: LayerType, cell: CellIndex) -> Option<u16> {
        self.bitmap_reader.tiles_holding(layer_type, cell)
    }

    /// The tick running.
    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    /// Says the rule is seeing to what is on the cell `author_cell` from here
    /// on -- an entity woken, a cell sampled: the author of all it
    /// queues until another is named. What the tick queued is applied
    /// author by author in the Morton order of their cells over the
    /// whole world -- an entity's name in the tick is its place --
    /// whichever superchunk each is in (`docs/simulation.md`, "One
    /// order, wherever the borders fall"). What is queued with no author
    /// named is the superchunk's rule's own, applied before any
    /// author's.
    pub fn seeing_to(&mut self, author_cell: CellIndex) {
        self.compare_end();
        debug_assert_eq!(self.compares_open_count, 0, "an author named with a compare open");
        self.author = author_cell.0 + 1;
        self.going_across = false;
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
        if !self.going_across {
            match self.author_began {
                Some(author_start) if author_start.slot as usize != slot => self.go_across(author_start),
                Some(_) => {}
                None => {
                    let slot_queue = &self.outbox.instruction_queues[slot];
                    self.author_began = Some(AuthorBegan { slot: slot as u8, instructions: slot_queue.instructions.len() as u32, gated_count: slot_queue.gated_by_compares.len() as u32, carried_count: slot_queue.carried_blocks.len() as u32 });
                }
            }
        }
        let queue_index = if self.going_across { ACROSS_QUEUE } else { slot };
        self.begin_segment(queue_index, slot);
        self.landed_last = (superchunk, queue_index as u8, slot as u8);
        (queue_index, slot)
    }

    /// Begins the author's segment of the queue `queue_index` of the
    /// outbox, if the last begun there is another's; and, of the
    /// queue of acts over a border, notes that the act lands in the
    /// superchunk of `slot`.
    #[inline]
    fn begin_segment(&mut self, queue_index: usize, slot: usize) {
        let outbox = &mut *self.outbox;
        if outbox.author_segments[queue_index].last().is_none_or(|last_segment| last_segment.author != self.author) {
            let (instructions, first_compare) = (outbox.instruction_queues[queue_index].instructions.len() as u32, outbox.instruction_queues[queue_index].gated_by_compares.len() as u32);
            outbox.author_segments[queue_index].push(AuthorSegment { author: self.author, instructions, first_compare });
            if queue_index == ACROSS_QUEUE {
                outbox.across_acts.push(AcrossAct { lands_in_slots: 1 << slot_home(), ..AcrossAct::default() });
            }
        }
        if queue_index == ACROSS_QUEUE {
            outbox.across_acts.last_mut().expect("the segment's").lands_in_slots |= 1 << slot;
        }
    }

    /// Makes what the author queues an act over a border: all it
    /// queued so far -- for the one superchunk it `author_start` in -- is
    /// moved to the queue of acts over a border, in the order queued,
    /// each instruction keeping where it lands; and what it queues
    /// from here goes there too. So nothing of one author is split
    /// between two superchunks' queues.
    #[cold]
    fn go_across(&mut self, author_start: AuthorBegan) {
        let from_slot = author_start.slot as usize;
        self.begin_segment(ACROSS_QUEUE, from_slot);
        let (slots, across_queues) = self.outbox.instruction_queues.split_at_mut(ACROSS_QUEUE);
        let (was_queue, across_queue) = (&mut slots[from_slot], &mut across_queues[0]);
        let (instructions, across_gated, across_carried) = (across_queue.instructions.len() as u32, across_queue.gated_by_compares.len() as u32, across_queue.carried_blocks.len() as u32);
        let carried_shift = i64::from(across_carried) - i64::from(author_start.carried_count);
        across_queue.instructions.extend(was_queue.instructions.drain(author_start.instructions as usize..).map(|instruction| instruction.carried_moved_by(carried_shift)));
        across_queue.gated_by_compares.extend(was_queue.gated_by_compares.drain(author_start.gated_count as usize..));
        across_queue.carried_blocks.extend(was_queue.carried_blocks.drain(author_start.carried_count as usize..));
        // A segment begun there for nothing but what was moved is no segment: the superchunk would stop at it between two acts.
        if self.outbox.author_segments[from_slot].last().is_some_and(|last_segment| last_segment.author == self.author && last_segment.instructions as usize == was_queue.instructions.len()) {
            self.outbox.author_segments[from_slot].pop();
        }
        // The compares open are the author's, so in the queue it began in: they lie where they were moved to.
        for open_compare in &mut self.compares_open[..self.compares_open_count] {
            debug_assert_eq!(open_compare.queue_index as usize, from_slot);
            *open_compare = conditional::CompareOpen { instruction: open_compare.instruction - author_start.instructions + instructions, gated_index: open_compare.gated_index - author_start.gated_count + across_gated, carried_before: open_compare.carried_before - author_start.carried_count + across_carried, queue_index: ACROSS_QUEUE as u8, ..*open_compare };
        }
        self.going_across = true;
        self.landed_last.0 = NO_SUPERCHUNK;
    }

    /// The outbox slot of `superchunk`: this one or a neighbour. Farther
    /// is past the speed of light, and a bug.
    pub(crate) fn slot_of(&self, superchunk: SuperchunkIndex) -> usize {
        slot_between(self.superchunk.superchunk_index(), superchunk).unwrap_or_else(|| panic!("a write more than a superchunk away: past the speed of light"))
    }
}

/// The outbox slot of the superchunk whose outbox it is.
pub(crate) fn slot_home() -> usize {
    slot_toward(0, 0)
}

/// The outbox slot, in the outbox of the superchunk `from_superchunk`, of the
/// superchunk `to_superchunk`: itself or a neighbour, none farther.
pub(crate) fn slot_between(from_superchunk: SuperchunkIndex, to_superchunk: SuperchunkIndex) -> Option<usize> {
    if to_superchunk == from_superchunk {
        return Some(slot_toward(0, 0));
    }
    let ((x, y), (to_x, to_y)) = (from_superchunk.cartesian(), to_superchunk.cartesian());
    let (dx, dy) = (to_x as i64 - x as i64, to_y as i64 - y as i64);
    (dx.abs() <= 1 && dy.abs() <= 1).then(|| slot_toward(dx as i32, dy as i32))
}
