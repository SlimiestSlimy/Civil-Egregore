//! Civil Egregore's entities, a file a kind: its rule, run on a
//! superchunk's turn, the attributes it keeps, and how a world is
//! given some to start with -- through `instructions` alone.
//!
//! The design: `docs/entity_rules.md`; function by function:
//! `docs/reference.md`.

//! What the entities are: `docs/entity_rules.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod sheep;
