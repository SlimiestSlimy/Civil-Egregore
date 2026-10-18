//! A superchunk's timer wheel: which entities wake at which tick
//! (`docs/entity_manager.md`, "The timer wheel").

use utilities::coordinates::CellIndex;

/// Ticks the wheel holds a slot for.
pub const WHEEL_TICKS: u64 = 1024;

/// An entity to wake: the cell it stood on when its wake was filed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wake {
    /// Its cell.
    pub stood_cell: CellIndex,
}

/// A superchunk's wakes, by tick.
pub(crate) struct Wheel {
    /// The wakes of the next [`WHEEL_TICKS`] ticks, a slot a tick.
    slots: Vec<Vec<Wake>>,
    /// The wakes further off, with their ticks, in the order filed.
    later_wakes: Vec<(u64, Wake)>,
}

impl Default for Wheel {
    fn default() -> Self {
        Self { slots: (0..WHEEL_TICKS).map(|_| Vec::new()).collect(), later_wakes: Vec::new() }
    }
}

impl Wheel {
    /// The wakes filed for `tick`, which is in reach: in the order filed.
    pub(crate) fn due_wakes(&self, tick: u64) -> &[Wake] {
        &self.slots[(tick % WHEEL_TICKS) as usize]
    }

    /// Files `wake` for `tick`, no earlier than `earliest_tick`: the tick about
    /// to run between ticks, the next one during a tick's second phase,
    /// after [`Wheel::pass_tick`].
    pub(crate) fn file_wake(&mut self, earliest_tick: u64, tick: u64, wake: Wake) {
        assert!(tick >= earliest_tick, "a wake at tick {tick}, before {earliest_tick}");
        if tick - earliest_tick < WHEEL_TICKS {
            self.slots[(tick % WHEEL_TICKS) as usize].push(wake);
        } else {
            self.later_wakes.push((tick, wake));
        }
    }

    /// Passes `tick`, just run: its slot emptied for the tick
    /// [`WHEEL_TICKS`] on, and every half of that the wakes further off
    /// now in reach filed.
    pub(crate) fn pass_tick(&mut self, tick: u64) {
        self.slots[(tick % WHEEL_TICKS) as usize].clear();
        if !tick.is_multiple_of(WHEEL_TICKS / 2) {
            return;
        }
        let slots = &mut self.slots;
        self.later_wakes.retain(|&(due_tick, wake)| {
            let in_reach = due_tick - tick <= WHEEL_TICKS;
            if in_reach {
                slots[(due_tick % WHEEL_TICKS) as usize].push(wake);
            }
            !in_reach
        });
    }

    /// Sorts the wakes filed for `tick`, in reach, by cell -- the order
    /// they wake in -- each cell once: a cell filed twice for one tick,
    /// by one entity and by another that came to stand there after it,
    /// wakes whoever stands on it once.
    pub(crate) fn sort_wakes(&mut self, tick: u64) {
        let slot = &mut self.slots[(tick % WHEEL_TICKS) as usize];
        slot.sort_unstable_by_key(|wake| wake.stood_cell);
        slot.dedup();
    }
}
