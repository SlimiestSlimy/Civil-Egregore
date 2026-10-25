//! The client's API for game content: what a type's code reaches as the
//! client runs it -- of the three public APIs, the client's, beside the
//! type registry and the simulation's: the calls of functions the
//! server's types let a client call ([`ServerCalls`]), render rules
//! ([`rendering`]) and gui types ([`gui`]). Bevy is part of it: a type's code
//! in the client is Bevy code.

pub mod gui;
pub mod rendering;

use crate::server_end::ServerLink;
use bevy::prelude::Resource;
use protocol::type_registry::ClientCallType;

/// Where a type's code in the client calls the functions the server's
/// types let a client call: by the call's type, words in, words out --
/// the call's own code makes and reads them on both ends.
#[derive(Resource, Clone)]
pub struct ServerCalls {
    /// The connection to the server the client runs with.
    server_link: ServerLink,
}

impl ServerCalls {
    /// Calls through `server_link`.
    pub(crate) fn through(server_link: ServerLink) -> Self {
        Self { server_link }
    }

    /// The answer of the function registered as `call` to `arguments`,
    /// from the world the server runs -- or why it is not answered: the
    /// call refused, or the server gone.
    pub fn call(&self, call: ClientCallType, arguments: &[u64]) -> Result<Vec<u64>, String> {
        self.server_link.call(call.0, arguments.to_vec()).unwrap_or_else(|| Err("the server is gone".to_string()))
    }
}
