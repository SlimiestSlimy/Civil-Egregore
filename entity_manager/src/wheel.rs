//! A superchunk's timer wheel: which entities wake at which tick
//! (`docs/entity_manager.md`, "The timer wheel").

use crate::entity::EntityId;
use coordinates::CellIndex;

/// Ticks the wheel holds a slot for.
pub const WHEEL_TICKS: u64 = 1024;

/// An entity to wake: its ID, and the cell it stood on when its wake was
/// filed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wake {
    /// Its ID.
    pub id: EntityId,
    /// Its cell.
    pub at: CellIndex,
}

/// A superchunk's wakes, by tick.
pub(crate) struct Wheel {
    /// The wakes of the next [`WHEEL_TICKS`] ticks, a slot a tick.
    slots: Vec<Vec<Wake>>,
    /// The wakes further off, with their ticks, in the order filed.
    later: Vec<(u64, Wake)>,
}

impl Default for Wheel {
    fn default() -> Self {
        Self { slots: (0..WHEEL_TICKS).map(|_| Vec::new()).collect(), later: Vec::new() }
    }
}

impl Wheel {
    /// The wakes filed for `tick`, which is in reach: in the order filed.
    pub(crate) fn due(&self, tick: u64) -> &[Wake] {
        &self.slots[(tick % WHEEL_TICKS) as usize]
    }

    /// Files `wake` for `tick`, no earlier than `earliest`: the tick about
    /// to run between ticks, the next one during a tick's second phase,
    /// after [`Wheel::pass`].
    pub(crate) fn file(&mut self, earliest: u64, tick: u64, wake: Wake) {
        assert!(tick >= earliest, "a wake at tick {tick}, before {earliest}");
        if tick - earliest < WHEEL_TICKS {
            self.slots[(tick % WHEEL_TICKS) as usize].push(wake);
        } else {
            self.later.push((tick, wake));
        }
    }

    /// Passes `tick`, just run: its slot emptied for the tick
    /// [`WHEEL_TICKS`] on, and every half of that the wakes further off
    /// now in reach filed.
    pub(crate) fn pass(&mut self, tick: u64) {
        self.slots[(tick % WHEEL_TICKS) as usize].clear();
        if !tick.is_multiple_of(WHEEL_TICKS / 2) {
            return;
        }
        let slots = &mut self.slots;
        self.later.retain(|&(due, wake)| {
            let in_reach = due - tick <= WHEEL_TICKS;
            if in_reach {
                slots[(due % WHEEL_TICKS) as usize].push(wake);
            }
            !in_reach
        });
    }

    /// Sorts the wakes filed for `tick`, in reach, by cell then ID: the
    /// order they wake in.
    pub(crate) fn sort(&mut self, tick: u64) {
        self.slots[(tick % WHEEL_TICKS) as usize].sort_unstable_by_key(|wake| (wake.at, wake.id));
    }

    /// Wakes filed, in reach and further off, good or not.
    pub(crate) fn len(&self) -> usize {
        self.slots.iter().map(Vec::len).sum::<usize>() + self.later.len()
    }
}
