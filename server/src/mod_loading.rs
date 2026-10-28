//! The server's mods loaded ([`load_server_mods`]): the registry
//! filtered to the server's side, each mod's library opened --
//! dependencies first, ties by name -- and each sca proc's set up
//! called, the proc kept with what sampling it needs: its planes' type
//! IDs and its chance a tick. Nothing is checked, and nothing fails but
//! by a panic: the manifests were checked, the libraries hashed.

use crate::sca_proc::{ScaProc, ScaProcSetUp};
use protocol::type_kinds::{Kind, Side};
use protocol::type_registry::{RegisteredType, TypeRegistry};
use std::collections::HashMap;
use utilities::chance::Chance;
use utilities::dynamic_library::{library_file_name, DynamicLibrary};

/// A sca proc, loaded: its code, and what it is sampled by.
pub struct LoadedScaProc {
    /// Its type ID.
    pub type_id: u64,
    /// The type IDs of the planes it is sampled on.
    pub sampled_plane_type_ids: Vec<u64>,
    /// The chance a tile not 0 of each of them is sampled for it, a tick.
    pub chance_per_tick: Chance,
    /// What its set up made.
    pub code: Box<dyn ScaProc>,
}

/// The server's mods, loaded: the registry it sees, and the sca procs.
pub struct ServerMods {
    /// The registry, the server's side's alone.
    pub type_registry: TypeRegistry,
    /// Every sca proc, in type ID order.
    pub sca_procs: Vec<LoadedScaProc>,
}

/// The mods of `type_registry` the server runs -- the common side's and
/// the server's -- loaded.
pub fn load_server_mods(type_registry: &TypeRegistry) -> ServerMods {
    let type_registry = type_registry.of_side(Side::Server);
    // Every library opened, each after those it depends on: kept here
    // while their set ups are called, open after.
    let mut libraries: HashMap<&str, DynamicLibrary> = HashMap::new();
    for registered_mod in mods_in_load_order(type_registry.types()) {
        let mod_name = registered_mod.qualified_name();
        // A container's library is none.
        let Kind::Mod { package: Some(package), .. } = registered_mod.kind() else { continue };
        let library_bytes = package.read_entry(&library_file_name(mod_name)).unwrap_or_else(|why| panic!("{mod_name}'s library: {why}"));
        libraries.insert(mod_name, DynamicLibrary::from_bytes(mod_name, &library_bytes).unwrap_or_else(|why| panic!("{mod_name}'s library: {why}")));
    }
    // Every sca proc set up, in the registry's order: its type ID's. A
    // mod's types follow it.
    let (mut library, mut sca_procs) = (None, Vec::new());
    for registered in type_registry.types() {
        match registered.kind() {
            Kind::Mod { .. } => library = libraries.get(registered.qualified_name()),
            Kind::ScaProc { planes, mean_simulation_seconds_per_sample } => {
                let library = library.expect("a sca proc's mod has a library: no container has types");
                let set_up_name = registered.set_up_static_name().expect("a sca proc's set up: its manifest names one");
                // SAFETY: built by the same compiler, against this very
                // crate: the static the manifest names is a `ScaProcSetUp`.
                let set_up = unsafe { library.static_value::<ScaProcSetUp>(set_up_name) }.unwrap_or_else(|why| panic!("{}: {why}", registered.qualified_name()));
                let sampled_plane_type_ids = planes.iter().map(|plane| type_registry.type_id_of(&plane.to_string()).expect("a plane: its manifest checked")).collect();
                let code = set_up(&type_registry);
                sca_procs.push(LoadedScaProc { type_id: registered.type_id(), sampled_plane_type_ids, chance_per_tick: mean_simulation_seconds_per_sample.chance_per_tick(), code });
            }
            Kind::Plane { .. } => {}
        }
    }
    ServerMods { type_registry, sca_procs }
}

/// The mods among `types`, in the order they load: each after every mod
/// it depends on that is among them -- one of another side never loading
/// here -- the first by name among those ready.
fn mods_in_load_order(types: &[RegisteredType]) -> Vec<&RegisteredType> {
    let mut waiting: Vec<&RegisteredType> = types.iter().filter(|registered| matches!(registered.kind(), Kind::Mod { .. })).collect();
    waiting.sort_by_key(|registered_mod| registered_mod.qualified_name());
    let mut ordered: Vec<&RegisteredType> = Vec::new();
    while !waiting.is_empty() {
        let is_ready = |registered_mod: &&RegisteredType| {
            let Kind::Mod { depends_on, .. } = registered_mod.kind() else { unreachable!("mods alone wait") };
            depends_on.iter().all(|dependency| {
                let is_it = |other: &&RegisteredType| other.qualified_name() == dependency.mod_name;
                ordered.iter().any(is_it) || !waiting.iter().any(is_it)
            })
        };
        let ready = waiting.iter().position(is_ready).expect("a mod ready: the manifests refuse rings of dependencies");
        ordered.push(waiting.remove(ready));
    }
    ordered
}
