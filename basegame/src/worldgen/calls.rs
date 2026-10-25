//! The world generation's functions a client may call
//! ([`call_registrations`]): a worldgen parameter the world is reset with,
//! read and set, each by its place among the generation's parameters
//! ([`WorldGeneration::worldgen_parameters`]'s order) -- kept in the
//! config file of the world's reset -- and the world's reset, that file
//! made the world's own first: what the base game's sliders call, as
//! gui types of the client.

use crate::worldgen::{WorldGeneration, RESET_CONFIG_FILE, WORLDGEN_CONFIG_FILE};
use protocol::type_registry::{ClientCallType, RegisteredType, Registration};
use protocol::kinds::server::client_call::{AnswerCall, ClientCall, ClientCallLibrary};
use protocol::kinds::server::handles::ServerSetUp;
use std::sync::Arc;

/// The call reading a worldgen parameter the world is reset with: its
/// place in, its value out.
pub const WORLDGEN_PARAMETER_CALL: &str = "worldgen parameter";
/// The call setting a worldgen parameter the world is reset with: its
/// place and its value in, nothing out.
pub const SET_WORLDGEN_PARAMETER_CALL: &str = "set worldgen parameter";
/// The call resetting the world, generated as the worldgen parameters
/// set for its reset say: nothing in, nothing out.
pub const RESET_WORLD_CALL: &str = "reset world";

/// The world generation's calls as the world registered them: what its
/// gui types call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldgenCalls {
    /// [`WORLDGEN_PARAMETER_CALL`]'s type.
    pub parameter: ClientCallType,
    /// [`SET_WORLDGEN_PARAMETER_CALL`]'s.
    pub set_parameter: ClientCallType,
    /// [`RESET_WORLD_CALL`]'s.
    pub reset_world: ClientCallType,
}

impl WorldgenCalls {
    /// The calls of `registered`, as [`call_registrations`] were registered,
    /// in their order: none if any is not a call.
    pub fn of(registered: &[RegisteredType]) -> Option<Self> {
        let call_type = |registered_type: &RegisteredType| match registered_type {
            RegisteredType::ClientCall { call, .. } => Some(*call),
            _ => None,
        };
        let [parameter, set_parameter, reset_world] = registered else {
            return None;
        };
        Some(Self { parameter: call_type(parameter)?, set_parameter: call_type(set_parameter)?, reset_world: call_type(reset_world)? })
    }
}

/// The registrations of the world generation's calls, each its master,
/// with its code.
pub fn call_registrations(world_generation: &str) -> [Registration; 3] {
    let call = |name: &str, code: Box<dyn ClientCall>| Registration::client_call(name, world_generation).with_library(Arc::new(ClientCallLibrary(code)));
    [call(WORLDGEN_PARAMETER_CALL, Box::new(WorldgenParameter)), call(SET_WORLDGEN_PARAMETER_CALL, Box::new(SetWorldgenParameter)), call(RESET_WORLD_CALL, Box::new(ResetWorld))]
}

/// [`WORLDGEN_PARAMETER_CALL`]: the worldgen parameter at the place its
/// argument names, as the world's reset is to read it
/// ([`WorldGeneration::read_reset_config`]).
struct WorldgenParameter;

impl ClientCall for WorldgenParameter {
    /// Answering from the base game's folder of the world's mod config.
    fn set_up(&self, server: &ServerSetUp) -> Result<Box<AnswerCall>, String> {
        let config_folder = server.mod_config_folder().to_path_buf();
        Ok(Box::new(move |_, arguments| {
            let &[place] = arguments else {
                return Err(format!("{WORLDGEN_PARAMETER_CALL}: one word, the parameter's place"));
            };
            let read = WorldGeneration::DEFAULT.read_reset_config(&config_folder)?.worldgen_parameters();
            let (_, value) = parameter_at(&read, place)?;
            Ok(vec![*value])
        }))
    }
}

/// [`SET_WORLDGEN_PARAMETER_CALL`]: sets the worldgen parameter at the
/// place its first argument names to the second, for the world's reset
/// -- in its config file, the world running generated as it was.
struct SetWorldgenParameter;

impl ClientCall for SetWorldgenParameter {
    /// Writing into the base game's folder of the world's mod config.
    fn set_up(&self, server: &ServerSetUp) -> Result<Box<AnswerCall>, String> {
        let config_folder = server.mod_config_folder().to_path_buf();
        Ok(Box::new(move |_, arguments| {
            let &[place, value] = arguments else {
                return Err(format!("{SET_WORLDGEN_PARAMETER_CALL}: two words, the parameter's place and its value"));
            };
            let for_reset = WorldGeneration::DEFAULT.read_reset_config(&config_folder)?;
            let (name, _) = parameter_at(&for_reset.worldgen_parameters(), place)?.clone();
            for_reset.with_worldgen_parameters(&[(name, value)]).write_reset_config(&config_folder)?;
            Ok(Vec::new())
        }))
    }
}

/// [`RESET_WORLD_CALL`]: resets the world, the config file of its reset
/// -- if one was written -- made the world's own first.
struct ResetWorld;

impl ClientCall for ResetWorld {
    /// Its files in the base game's folder of the world's mod config.
    fn set_up(&self, server: &ServerSetUp) -> Result<Box<AnswerCall>, String> {
        let (reset_path, world_path) = (server.mod_config_folder().join(RESET_CONFIG_FILE), server.mod_config_folder().join(WORLDGEN_CONFIG_FILE));
        Ok(Box::new(move |between_ticks, arguments| {
            if !arguments.is_empty() {
                return Err(format!("{RESET_WORLD_CALL}: no words"));
            }
            if reset_path.exists() {
                std::fs::rename(&reset_path, &world_path).map_err(|error| format!("{}: {error}", reset_path.display()))?;
            }
            between_ticks.reset_world();
            Ok(Vec::new())
        }))
    }
}

/// The parameter at `place` among `worldgen_parameters`: or why there
/// is none.
fn parameter_at(worldgen_parameters: &[(String, u64)], place: u64) -> Result<&(String, u64), String> {
    usize::try_from(place).ok().and_then(|place| worldgen_parameters.get(place)).ok_or_else(|| format!("no worldgen parameter at {place}: {} of them", worldgen_parameters.len()))
}
