//! What the client and the server send each other ([`messages`]): what
//! a client asks of the world's viewport and is answered ([`frame`]),
//! and of its terrain ([`terrain`]) -- each on the wire as words
//! (`utilities::wire`).

pub mod frame;
pub mod messages;
pub mod terrain;
