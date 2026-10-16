//! Which thread goes on with which superchunk in a tick's second
//! phase: superchunks that wait on one another at acts over a border
//! come to be one thread's, since passing a superchunk between two
//! threads at every act costs more than the act; a superchunk with
//! work of its own is anyone's; a thread with nothing to do rests,
//! and then sleeps (`docs/simulation.md`, "An act over a border").

use super::Phase;
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
    superchunk_list: Mutex<Vec<usize>>,
    /// How many there are: read without the lock.
    lying_count: AtomicUsize,
}

impl LetGoOn {
    /// Empties it: of those a thread waiting for them took up from
    /// under it, which it still lists.
    pub(super) fn empty_out(&mut self) {
        self.superchunk_list.get_mut().expect("the superchunks let go on").clear();
        *self.lying_count.get_mut() = 0;
    }

    /// Puts `superchunk` among them.
    fn put(&self, superchunk: usize) {
        self.superchunk_list.lock().expect("the superchunks let go on").push(superchunk);
        self.lying_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Takes one of them off the list, if there is one: not taken up
    /// yet.
    fn pop(&self) -> Option<usize> {
        if self.lying_count.load(Ordering::SeqCst) == 0 {
            return None;
        }
        let popped_superchunk = self.superchunk_list.lock().expect("the superchunks let go on").pop()?;
        self.lying_count.fetch_sub(1, Ordering::SeqCst);
        Some(popped_superchunk)
    }

    /// Takes one of them up, if there is one that `lying_to_take_up` says no
    /// thread waiting for it took up first.
    fn take_up_lying(&self, lying_to_take_up: &[AtomicBool]) -> Option<usize> {
        loop {
            let popped_superchunk = self.pop()?;
            if lying_to_take_up[popped_superchunk].swap(false, Ordering::SeqCst) {
                return Some(popped_superchunk);
            }
        }
    }
}

impl Phase<'_, '_> {
    /// The next superchunk for the thread `part` to apply to, if there
    /// is one: one it let go on itself, else one not begun, else one
    /// that is anyone's.
    pub(super) fn next_for(&self, part: usize) -> Option<usize> {
        let next_superchunk = self.let_go_on[part].take_up_lying(self.lying_to_take_up).or_else(|| self.take_unbegun()).or_else(|| self.anyones_list().take_up_lying(self.lying_to_take_up))?;
        self.claimed_by[next_superchunk].store(part, Ordering::SeqCst);
        Some(next_superchunk)
    }

    /// Takes up the superchunk at `superchunk` for the thread `part`
    /// if it lies to be: so, whether the thread has it now.
    fn take_up(&self, superchunk: usize, part: usize) -> bool {
        let was_lying = self.lying_to_take_up[superchunk].swap(false, Ordering::SeqCst);
        if was_lying {
            self.claimed_by[superchunk].store(part, Ordering::SeqCst);
        }
        was_lying
    }

    /// Lets the superchunk at `superchunk` lie in the list `list_index`, to
    /// be taken up.
    pub(super) fn let_lie(&self, superchunk: usize, list_index: usize) {
        self.claimed_by[superchunk].store(list_index, Ordering::SeqCst);
        self.lying_to_take_up[superchunk].store(true, Ordering::SeqCst);
        self.let_go_on[list_index].put(superchunk);
    }

    /// What the thread `part` goes on with when a superchunk of its
    /// waits at an act over a border for one of `waited_for`: one of
    /// those, if it is the thread's own to take up or anyone's. Else
    /// another thread has what is waited for, and is given all this
    /// one kept: what waits on one another is one thread's, since
    /// passing it between two at every act costs more than the act.
    pub(super) fn after_waiting(&self, waited_for: Vec<usize>, part: usize) -> Option<usize> {
        let anyones_list_index = self.let_go_on.len() - 1;
        let mut maybe_another_list = None;
        for waited_superchunk in waited_for {
            let whose_list = self.claimed_by[waited_superchunk].load(Ordering::SeqCst);
            if whose_list != part && whose_list != anyones_list_index {
                maybe_another_list = Some(whose_list);
            } else if self.take_up(waited_superchunk, part) {
                return Some(waited_superchunk);
            }
        }
        let another_list = maybe_another_list?;
        let mut gave_any = false;
        while let Some(kept_superchunk) = self.let_go_on[part].pop() {
            // One taken up from under the list is no longer there to give.
            if self.lying_to_take_up[kept_superchunk].load(Ordering::SeqCst) {
                self.claimed_by[kept_superchunk].store(another_list, Ordering::SeqCst);
                self.let_go_on[another_list].put(kept_superchunk);
                gave_any = true;
            }
        }
        if gave_any {
            self.wake(true);
        }
        None
    }

    /// The superchunks let go on that are anyone's.
    pub(super) fn anyones_list(&self) -> &LetGoOn {
        &self.let_go_on[self.let_go_on.len() - 1]
    }

    /// Whether all is applied, or there is a superchunk for the
    /// thread `part` to take up.
    fn is_something_to_do(&self, part: usize) -> bool {
        let any_lying = [&self.let_go_on[part], self.anyones_list()].into_iter().any(|go_on_list| go_on_list.lying_count.load(Ordering::SeqCst) > 0);
        any_lying || self.superchunks_done.load(Ordering::SeqCst) == self.applying_superchunks.len() || self.to_begin.load(Ordering::SeqCst) > 0
    }

    /// Sleeps until there may be something to do: a thread spinning
    /// takes from the one applying beside it on the same core.
    pub(super) fn sleep(&self, part: usize) {
        let asleep_guard = self.asleep_lock.lock().expect("the threads asleep");
        self.sleeping_threads.fetch_add(1, Ordering::SeqCst);
        // Looked at once more, counted as sleeping: whoever puts a superchunk there after this sees the count and wakes it.
        if !self.is_something_to_do(part) {
            drop(self.wake_signal.wait(asleep_guard).expect("the threads asleep"));
        }
        self.sleeping_threads.fetch_sub(1, Ordering::SeqCst);
    }

    /// Wakes the threads asleep, if any are: all of them, or one.
    pub(super) fn wake(&self, wake_all: bool) {
        if self.sleeping_threads.load(Ordering::SeqCst) > 0 {
            // Held so that one going to sleep is asleep before it is woken.
            drop(self.asleep_lock.lock().expect("the threads asleep"));
            if wake_all {
                self.wake_signal.notify_all();
            } else {
                self.wake_signal.notify_one();
            }
        }
    }

    /// Makes anyone's the superchunks the thread `part` kept to go on
    /// with itself: it has come to something that keeps it.
    pub(super) fn give_up_those_kept(&self, part: usize) {
        while let Some(kept_superchunk) = self.let_go_on[part].pop() {
            if self.lying_to_take_up[kept_superchunk].load(Ordering::SeqCst) {
                self.claimed_by[kept_superchunk].store(self.let_go_on.len() - 1, Ordering::SeqCst);
                self.anyones_list().put(kept_superchunk);
                self.wake(false);
            }
        }
    }

    /// A superchunk not begun, if there is one.
    fn take_unbegun(&self) -> Option<usize> {
        if self.to_begin.load(Ordering::SeqCst) == 0 {
            return None;
        }
        loop {
            let unbegun_superchunk = self.to_go_on.lock().expect("the superchunks to begin with").pop()?;
            self.to_begin.fetch_sub(1, Ordering::SeqCst);
            // Unless a thread waiting for it took it up first.
            if self.lying_to_take_up[unbegun_superchunk].swap(false, Ordering::SeqCst) {
                return Some(unbegun_superchunk);
            }
        }
    }
}
