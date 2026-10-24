//! A function a client may call across the connection, as its type's
//! library holds it ([`CallCode`]), and what it is answered against
//! ([`CallContext`]): the world running, between two ticks -- its mod's
//! folder of the world's mod config, to read and write as it will, and
//! the world's reset, which a call may ask for. A call's arguments and
//! its answer are words, which the function's own code makes and reads
//! on both ends -- the client and the server have the same types, and
//! so the same code.

use std::path::Path;
use std::sync::Arc;

/// A function a client may call, as its code is: words in, words out
/// -- or why it would not take them.
type CallFunction = dyn Fn(&mut CallContext, &[u64]) -> Result<Vec<u64>, String> + Send + Sync;

/// What a function a client may call is answered against: the world
/// running, as far as a call is let see it, between two ticks; its mod's
/// config; and its reset -- the world made again from its start in its
/// folder, of the same types, seed and config.
pub struct CallContext<'world> {
    /// The world's seed.
    pub(crate) seed: u64,
    /// The tick it is at.
    pub(crate) tick: u64,
    /// The call's mod's folder of the world's mod config.
    pub(crate) mod_config_folder: &'world Path,
    /// Whether the call asked for the world to be reset.
    pub(crate) reset_asked: bool,
}

impl CallContext<'_> {
    /// The world's seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The tick the world is at: the next to run.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The call's mod's folder of the world's mod config: its files the
    /// mod's own, read and written as it will -- the server reads none.
    pub fn mod_config_folder(&self) -> &Path {
        self.mod_config_folder
    }

    /// Resets the world once the call is answered: made again from its
    /// start in its folder, of the same types and seed, generated as its
    /// config then says -- everything else of it gone.
    pub fn reset_world(&mut self) {
        self.reset_asked = true;
    }
}

/// The code of a function a client may call: its arguments' words in,
/// its answer's words out -- or why it would not take them. The
/// library of a type registered as a client call
/// (`protocol::type_registry::Registration::client_call`).
#[derive(Clone)]
pub struct CallCode {
    /// The function.
    answer: Arc<CallFunction>,
}

impl CallCode {
    /// The code answering a call with `answer`.
    pub fn new(answer: impl Fn(&mut CallContext, &[u64]) -> Result<Vec<u64>, String> + Send + Sync + 'static) -> Self {
        Self { answer: Arc::new(answer) }
    }

    /// Its answer to `arguments`, against `call_context`.
    pub(crate) fn answer(&self, call_context: &mut CallContext, arguments: &[u64]) -> Result<Vec<u64>, String> {
        (self.answer)(call_context, arguments)
    }
}
