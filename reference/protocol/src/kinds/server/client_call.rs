//! The client call kind: a function a client may call, answered by the
//! server between two ticks. The client asks over the connection, by
//! the call's type, its arguments' words in; the server answers, words
//! out -- the call's own code makes and reads them on both ends.

use super::handles::{BetweenTicks, ServerSetUp};
use crate::type_registry::{ClientCallType, FrozenTypeRegistry, RegisteredType};
use std::path::Path;

/// A call's answer, as its code set up is: its arguments' words in, its
/// answer's words out -- or why it would not take them.
pub type AnswerCall = dyn Fn(&mut dyn BetweenTicks, &[u64]) -> Result<Vec<u64>, String> + Send + Sync;

/// The code a mod adds for a type registered as a client call.
pub trait ClientCall: Send + Sync + 'static {
    /// Its answer, set up as the world starts: what it keeps gathered
    /// from `server` -- or why the world cannot run.
    fn set_up(&self, server: &ServerSetUp) -> Result<Box<AnswerCall>, String>;
}

/// A client call's code, as its type's library holds it -- until the
/// builder adds code by its type's name.
pub struct ClientCallLibrary(pub Box<dyn ClientCall>);

/// A world's client calls, each set up once as it starts, sorted by ID:
/// what the server answers a call by.
pub struct WorldClientCalls(Box<[(ClientCallType, Box<AnswerCall>)]>);

impl WorldClientCalls {
    /// Every client call of `frozen_type_registry` set up, each with its
    /// mod's (`root_bundle_of`) folder of `mod_configs`, the world's mod
    /// config; or why the world cannot run: a call whose library is no
    /// call's code, or one that would not set up.
    pub fn set_up(frozen_type_registry: &FrozenTypeRegistry, mod_configs: &Path) -> Result<Self, String> {
        let mut calls = Vec::new();
        for registered in frozen_type_registry.types() {
            let RegisteredType::ClientCall { name, call, .. } = registered else { continue };
            let library = frozen_type_registry.library_of(registered).and_then(|call_library| call_library.downcast_ref::<ClientCallLibrary>()).ok_or_else(|| format!("{name} is a client call, and its library is no call's code"))?;
            let mod_config_folder = mod_configs.join(crate::type_registry::root_bundle_of(name));
            let answer = library.0.set_up(&ServerSetUp::new(frozen_type_registry, &mod_config_folder)).map_err(|why_not| format!("{name}: {why_not}"))?;
            calls.push((*call, answer));
        }
        calls.sort_unstable_by_key(|(call, _)| call.0);
        Ok(Self(calls.into_boxed_slice()))
    }

    /// The answer of the call whose ID is `call`: or why it is refused,
    /// none of the world's being of the ID.
    pub fn find(&self, call: u64) -> Result<&AnswerCall, String> {
        self.0.binary_search_by_key(&call, |(call_type, _)| call_type.0).map(|place| &*self.0[place].1).map_err(|_| format!("no client call is of the ID {call:#018x}"))
    }
}
