//! What the client and the server send each other ([`messages`]): what
//! a client asks of the world's viewport and is answered ([`frame`]),
//! and of its terrain ([`terrain`]) -- each on the wire as words
//! (`utilities::wire`).

pub mod frame;
pub mod messages;
pub mod terrain;

/// The port a server listens on for its client, on this machine: one
/// server a machine at a time.
pub const SERVER_PORT: u16 = 11235;
