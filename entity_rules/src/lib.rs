//! Civil Egregore's entities: every kind of entity the game has, a file each --
//! its rule, run on a superchunk's turn in a tick's first phase, the
//! attributes it keeps, and how a world is given some to start with.
//!
//! What an entity *is* -- its header and attributes, the bucket a chunk
//! that keeps it, the timer wheel that wakes it, the instructions it
//! queues -- is the
//! simulation's (`entity_manager/src/`), and knows no kind of
//! entity. What each kind *does* is here, and knows no other rule: the
//! game (`src/`, `Civil_Egregore`) ticks them together with the rules of the
//! cells.
//!
//! | module | entity |
//! |---|---|
//! | `sheep` | sheep eating the grass, breeding, walking, starving: the first entity |
//!
//! A rule sees the simulation through `instructions` alone, so it holds
//! no world: the worlds its rules are tested and measured on are the
//! server's (`server/tests/fast/sheep.rs`, `server/src/diagnostics/`).

//! What the entities are: `docs/entity_rules.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod sheep;
