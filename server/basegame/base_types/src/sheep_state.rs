//! What a sheep keeps ([`SheepState`]): its default attribute's layout.

use type_registry::{Layout, Member};

/// All a sheep keeps, in the three blocks of [`SHEEP_STATE`](crate::SHEEP_STATE):
/// a word a member, in the order written. A tick of 0 is none: the
/// sheep is not in the middle of that.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SheepState {
    /// The tick it is next hungry at.
    pub hungry_at: u64,
    /// The tick its lamb is due at, if it is pregnant.
    pub lamb_due_at: u64,
    /// The tick it is grown at, if it is a lamb.
    pub grown_at: u64,
    /// The tick it roams until, if it is leaving thin pasture. Set
    /// once, at the meal, with the neighbour: a step on the way
    /// changes nothing.
    pub roaming_until: u64,
    /// The neighbour it steps to while it roams, by its way: which
    /// of the nine cells about it.
    pub roaming_neighbour: u32,
}

impl Layout for SheepState {
    const WORDS: usize = 5;
    const MEMBERS: &'static [Member] = &[Member::word(0), Member::word(1), Member::word(2), Member::word(3), Member::within_word(4, 0, 4)];

    fn write(self, attribute_data: &mut [u64]) {
        attribute_data[..5].copy_from_slice(&[self.hungry_at, self.lamb_due_at, self.grown_at, self.roaming_until, self.roaming_neighbour as u64]);
    }

    fn read_data(attribute_data: &[u64]) -> Self {
        Self { hungry_at: attribute_data[0], lamb_due_at: attribute_data[1], grown_at: attribute_data[2], roaming_until: attribute_data[3], roaming_neighbour: attribute_data[4] as u32 }
    }
}
