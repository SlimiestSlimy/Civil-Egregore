//! A function a client may call across the connection, as its type's
//! library holds it ([`CallCode`]), and what it is answered against
//! ([`CallContext`]): the world running, between two ticks -- and the
//! world's reset, which a call may set up and ask for. A call's
//! arguments and its answer are words, which the function's own code
//! makes and reads on both ends -- the client and the server have the
//! same types, and so the same code.

use std::sync::Arc;

/// A function a client may call, as its code is: words in, words out
/// -- or why it would not take them.
type CallFunction = dyn Fn(&mut CallContext, &[u64]) -> Result<Vec<u64>, String> + Send + Sync;

/// What a function a client may call is answered against: the world
/// running, as far as a call is let see it, between two ticks; and its
/// reset -- the world made again from its start, of the same types and
/// seed, with the worldgen parameters set for it.
pub struct CallContext<'world> {
    /// The world's seed.
    pub(crate) seed: u64,
    /// The tick it is at.
    pub(crate) tick: u64,
    /// Its worldgen parameters, each by name.
    pub(crate) worldgen_parameters: &'world [(String, u64)],
    /// The worldgen parameters it is reset with, each by name.
    pub(crate) worldgen_parameters_for_reset: &'world mut Vec<(String, u64)>,
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

    /// The world's worldgen parameters, each by name: those it was
    /// generated with.
    pub fn worldgen_parameters(&self) -> &[(String, u64)] {
        self.worldgen_parameters
    }

    /// The worldgen parameters the world is reset with, each by name:
    /// its own, until one is set ([`CallContext::set_worldgen_parameter_for_reset`]).
    pub fn worldgen_parameters_for_reset(&self) -> &[(String, u64)] {
        self.worldgen_parameters_for_reset
    }

    /// Sets the worldgen parameter `name` the world is reset with to
    /// `value`: the world running is generated as it was, until it is
    /// reset ([`CallContext::reset_world`]). A name the world's
    /// generation does not read is dropped as it is reset.
    pub fn set_worldgen_parameter_for_reset(&mut self, name: &str, value: u64) {
        match self.worldgen_parameters_for_reset.iter_mut().find(|(parameter_name, _)| parameter_name == name) {
            Some((_, kept_value)) => *kept_value = value,
            None => self.worldgen_parameters_for_reset.push((name.to_string(), value)),
        }
    }

    /// Resets the world once the call is answered: made again from its
    /// start, of the same types and seed, with the worldgen parameters
    /// set for its reset -- everything else of it gone.
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
