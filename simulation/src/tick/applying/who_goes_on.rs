//! Which thread goes on with which superchunk in a tick's second
//! phase: superchunks that wait on one another at acts over a border
//! come to be one thread's, since passing a superchunk between two
//! threads at every act costs more than the act; a superchunk with
//! work of its own is anyone's; a thread with nothing to do rests,
//! and then sleeps (`docs/simulation.md`, "An act over a border").

use super::Phase;
use crate::turn::SLOTS;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

/// The longest a thread with nothing to do rests before it looks
/// again, in turns of the processor's waiting.
pub(super) const LONGEST_REST: u32 = 512;

/// Superchunks that waited at an act over a border and can go on: one
/// thread's, or anyone's. A line of the cache to itself, each being
/// written to by its own thread at every such act.
#[derive(Default)]
#[repr(align(64))]
pub(super) struct LetGoOn {
    /// The superchunks.
    list: Mutex<Vec<usize>>,
    /// How many there are: read without the lock.
    lying: AtomicUsize,
}

impl LetGoOn {
    /// Empties it: of those a thread waiting for them took up from
    /// under it, which it still lists.
    pub(super) fn empty(&mut self) {
        self.list.get_mut().expect("the superchunks let go on").clear();
        *self.lying.get_mut() = 0;
    }

    /// Puts `superchunk` among them.
    fn put(&self, superchunk: usize) {
        self.list.lock().expect("the superchunks let go on").push(superchunk);
        self.lying.fetch_add(1, Ordering::SeqCst);
    }

    /// Takes one of them off the list, if there is one: not taken up
    /// yet.
    fn pop(&self) -> Option<usize> {
        if self.lying.load(Ordering::SeqCst) == 0 {
            return None;
        }
        let popped = self.list.lock().expect("the superchunks let go on").pop()?;
        self.lying.fetch_sub(1, Ordering::SeqCst);
        Some(popped)
    }

    /// Takes one of them up, if there is one that `lying` says no
    /// thread waiting for it took up first.
    fn take(&self, lying: &[AtomicBool]) -> Option<usize> {
        loop {
            let popped = self.pop()?;
            if lying[popped].swap(false, Ordering::SeqCst) {
                return Some(popped);
            }
        }
    }
}

impl Phase<'_, '_> {
    /// The next superchunk for the thread `part` to apply to, if there
    /// is one: one it let go on itself, else one not begun, else one
    /// that is anyone's.
    pub(super) fn next_for(&self, part: usize) -> Option<usize> {
        let next = self.let_go_on[part].take(self.lying).or_else(|| self.begin()).or_else(|| self.anyones().take(self.lying))?;
        self.whose[next].store(part, Ordering::SeqCst);
        Some(next)
    }

    /// Takes up the superchunk at `superchunk` for the thread `part`
    /// if it lies to be: so, whether the thread has it now.
    fn take_up(&self, superchunk: usize, part: usize) -> bool {
        let taken = self.lying[superchunk].swap(false, Ordering::SeqCst);
        if taken {
            self.whose[superchunk].store(part, Ordering::SeqCst);
        }
        taken
    }

    /// Lets the superchunk at `superchunk` lie in the list `of`, to
    /// be taken up.
    pub(super) fn let_lie(&self, superchunk: usize, of: usize) {
        self.whose[superchunk].store(of, Ordering::SeqCst);
        self.lying[superchunk].store(true, Ordering::SeqCst);
        self.let_go_on[of].put(superchunk);
    }

    /// What the thread `part` goes on with when a superchunk of its
    /// waits at an act over a border for one of `waited_for`: one of
    /// those, if it is the thread's own to take up or anyone's. Else
    /// another thread has what is waited for, and is given all this
    /// one kept: what waits on one another is one thread's, since
    /// passing it between two at every act costs more than the act.
    pub(super) fn after_waiting(&self, waited_for: [Option<usize>; SLOTS], part: usize) -> Option<usize> {
        let anyones = self.let_go_on.len() - 1;
        let mut anothers = None;
        for other in waited_for.into_iter().flatten() {
            let whose = self.whose[other].load(Ordering::SeqCst);
            if whose != part && whose != anyones {
                anothers = Some(whose);
            } else if self.take_up(other, part) {
                return Some(other);
            }
        }
        let anothers = anothers?;
        let mut given = false;
        while let Some(kept) = self.let_go_on[part].pop() {
            // One taken up from under the list is no longer there to give.
            if self.lying[kept].load(Ordering::SeqCst) {
                self.whose[kept].store(anothers, Ordering::SeqCst);
                self.let_go_on[anothers].put(kept);
                given = true;
            }
        }
        if given {
            self.wake(true);
        }
        None
    }

    /// The superchunks let go on that are anyone's.
    pub(super) fn anyones(&self) -> &LetGoOn {
        &self.let_go_on[self.let_go_on.len() - 1]
    }

    /// Whether all is applied, or there is a superchunk for the
    /// thread `part` to take up.
    fn is_something_to_do(&self, part: usize) -> bool {
        let lying = [&self.let_go_on[part], self.anyones()].into_iter().any(|list| list.lying.load(Ordering::SeqCst) > 0);
        lying || self.done.load(Ordering::SeqCst) == self.applying.len() || self.to_begin.load(Ordering::SeqCst) > 0
    }

    /// Sleeps until there may be something to do: a thread spinning
    /// takes from the one applying beside it on the same core.
    pub(super) fn sleep(&self, part: usize) {
        let asleep = self.asleep.lock().expect("the threads asleep");
        self.sleeping.fetch_add(1, Ordering::SeqCst);
        // Looked at once more, counted as sleeping: whoever puts a superchunk there after this sees the count and wakes it.
        if !self.is_something_to_do(part) {
            drop(self.woken.wait(asleep).expect("the threads asleep"));
        }
        self.sleeping.fetch_sub(1, Ordering::SeqCst);
    }

    /// Wakes the threads asleep, if any are: all of them, or one.
    pub(super) fn wake(&self, all: bool) {
        if self.sleeping.load(Ordering::SeqCst) > 0 {
            // Held so that one going to sleep is asleep before it is woken.
            drop(self.asleep.lock().expect("the threads asleep"));
            if all {
                self.woken.notify_all();
            } else {
                self.woken.notify_one();
            }
        }
    }

    /// Makes anyone's the superchunks the thread `part` kept to go on
    /// with itself: it has come to something that keeps it.
    pub(super) fn give_up_those_kept(&self, part: usize) {
        while let Some(kept) = self.let_go_on[part].pop() {
            if self.lying[kept].load(Ordering::SeqCst) {
                self.whose[kept].store(self.let_go_on.len() - 1, Ordering::SeqCst);
                self.anyones().put(kept);
                self.wake(false);
            }
        }
    }

    /// A superchunk not begun, if there is one.
    fn begin(&self) -> Option<usize> {
        if self.to_begin.load(Ordering::SeqCst) == 0 {
            return None;
        }
        loop {
            let begun = self.to_go_on.lock().expect("the superchunks to begin with").pop()?;
            self.to_begin.fetch_sub(1, Ordering::SeqCst);
            // Unless a thread waiting for it took it up first.
            if self.lying[begun].swap(false, Ordering::SeqCst) {
                return Some(begun);
            }
        }
    }
}
