//! The world generation's functions a client may call
//! ([`call_registrations`]): a worldgen parameter the world is reset with,
//! read and set, each by its place among the generation's parameters
//! ([`WorldGeneration::worldgen_parameters`]'s order) -- and the
//! world's reset: what the base game's sliders will call, as gui types
//! of the client.

use crate::worldgen::WorldGeneration;
use protocol::type_registry::{ClientCallType, RegisteredType, Registration};
use server::simulation::simulation_api::{CallCode, CallContext};
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
    [
        Registration::client_call(WORLDGEN_PARAMETER_CALL, world_generation).with_library(Arc::new(CallCode::new(worldgen_parameter))),
        Registration::client_call(SET_WORLDGEN_PARAMETER_CALL, world_generation).with_library(Arc::new(CallCode::new(set_worldgen_parameter))),
        Registration::client_call(RESET_WORLD_CALL, world_generation).with_library(Arc::new(CallCode::new(reset_world))),
    ]
}

/// The worldgen parameter at the place `arguments` names, as the
/// world's reset is to read it: or why not.
fn worldgen_parameter(call_context: &mut CallContext, arguments: &[u64]) -> Result<Vec<u64>, String> {
    let &[place] = arguments else {
        return Err(format!("{WORLDGEN_PARAMETER_CALL}: one word, the parameter's place"));
    };
    let read = WorldGeneration::of_worldgen_parameters(call_context.worldgen_parameters_for_reset()).worldgen_parameters();
    let (_, value) = parameter_at(&read, place)?;
    Ok(vec![*value])
}

/// Sets the worldgen parameter at the place `arguments` names to the
/// value after it, for the world's reset: or why not.
fn set_worldgen_parameter(call_context: &mut CallContext, arguments: &[u64]) -> Result<Vec<u64>, String> {
    let &[place, value] = arguments else {
        return Err(format!("{SET_WORLDGEN_PARAMETER_CALL}: two words, the parameter's place and its value"));
    };
    let read = WorldGeneration::DEFAULT.worldgen_parameters();
    let (name, _) = parameter_at(&read, place)?;
    call_context.set_worldgen_parameter_for_reset(name, value);
    Ok(Vec::new())
}

/// Resets the world: nothing taken, nothing answered.
fn reset_world(call_context: &mut CallContext, arguments: &[u64]) -> Result<Vec<u64>, String> {
    if !arguments.is_empty() {
        return Err(format!("{RESET_WORLD_CALL}: no words"));
    }
    call_context.reset_world();
    Ok(Vec::new())
}

/// The parameter at `place` among `worldgen_parameters`: or why there
/// is none.
fn parameter_at(worldgen_parameters: &[(String, u64)], place: u64) -> Result<&(String, u64), String> {
    usize::try_from(place).ok().and_then(|place| worldgen_parameters.get(place)).ok_or_else(|| format!("no worldgen parameter at {place}: {} of them", worldgen_parameters.len()))
}
