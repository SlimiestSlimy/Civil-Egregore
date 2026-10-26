//! The registry's libraries loaded for one side ([`TypeRegistry::load`]):
//! every bundle of the common side and that one, dependencies first,
//! each library opened and its bundle set up ([`BUNDLE_SET_UP`]) -- given
//! the registry and the code of the bundles before it, to take what it
//! needs of them once -- its code matched to the bundle's types as the
//! manifests say them: a name it has no type of, a type of a kind with
//! code it gives none, refused.

use super::TypeRegistry;
use crate::kinds::{Kind, Side};
use std::any::Any;
use std::collections::HashMap;
use utilities::dynamic_library::DynamicLibrary;

/// What a bundle's library makes of a type: its code, or what its
/// dependents take of it -- for them, and for the side, to downcast to
/// what they know it as.
pub type Export = Box<dyn Any + Send + Sync>;

/// The function every bundle's library exports as [`BUNDLE_SET_UP`]: its
/// types' code, each by its name in the bundle, made with what it takes
/// of the registry and of the bundles before it.
pub type BundleSetUp = fn(&TypeRegistryHandle<'_>) -> Result<Vec<(&'static str, Export)>, String>;

/// The name a bundle's library exports its [`BundleSetUp`] as.
pub const BUNDLE_SET_UP: &str = "bundle_set_up";

/// What a bundle is set up with: the registry -- every type's ID and full
/// name -- and the code of every bundle set up before it.
pub struct TypeRegistryHandle<'loading> {
    /// The registry.
    registry: &'loading TypeRegistry,
    /// The code made so far, by type ID.
    code: &'loading HashMap<u64, Export>,
}

impl TypeRegistryHandle<'_> {
    /// The registry.
    pub fn registry(&self) -> &TypeRegistry {
        self.registry
    }

    /// The ID of the type of the full name `full_name`, if there is one.
    pub fn type_id_of(&self, full_name: &str) -> Option<u64> {
        self.registry.types.iter().find(|registered| registered.full_name == full_name).map(|registered| registered.type_id)
    }

    /// What the bundle of the type of the full name `full_name` made of
    /// it, if it was set up before and made anything: to downcast.
    pub fn code_of(&self, full_name: &str) -> Option<&(dyn Any + Send + Sync)> {
        self.code.get(&self.type_id_of(full_name)?).map(|export| &**export)
    }
}

/// The registry, its libraries loaded for one side, and the code they
/// made: every type's, by its ID, for the side to downcast.
pub struct LoadedRegistry {
    /// The registry.
    registry: TypeRegistry,
    /// The code made, by type ID.
    code: HashMap<u64, Export>,
    /// The libraries opened: kept open, the code theirs.
    libraries: Vec<DynamicLibrary>,
}

impl LoadedRegistry {
    /// The registry.
    pub fn registry(&self) -> &TypeRegistry {
        &self.registry
    }

    /// What was made of the type of ID `type_id`, if anything.
    pub fn code_of(&self, type_id: u64) -> Option<&(dyn Any + Send + Sync)> {
        self.code.get(&type_id).map(|export| &**export)
    }

    /// How many libraries were opened.
    pub fn library_count(&self) -> usize {
        self.libraries.len()
    }
}

/// Whether a type of `kind` has code its bundle must make.
fn has_code(kind: &Kind) -> bool {
    matches!(kind, Kind::EntityProc { .. } | Kind::ScaProc | Kind::Worldgen | Kind::RenderProc { .. } | Kind::Gui { .. })
}

impl TypeRegistry {
    /// Its libraries loaded for `side` -- the common bundles' and the
    /// side's, each after every bundle it depends on, the first by name
    /// among those ready -- each bundle set up: or what went wrong.
    pub fn load(self, side: Side) -> Result<LoadedRegistry, String> {
        let mut waiting: Vec<usize> = (0..self.types.len()).filter(|&place| matches!(&self.types[place].kind, Kind::Bundle(fields) if fields.side == Side::Common || fields.side == side)).collect();
        let (mut code, mut libraries, mut set_up) = (HashMap::new(), Vec::new(), Vec::<String>::new());
        while !waiting.is_empty() {
            let ready = |place: &usize| {
                let Kind::Bundle(fields) = &self.types[*place].kind else { unreachable!("bundles alone wait") };
                fields.depends_on.iter().all(|dependency| set_up.contains(&dependency.to_string()))
            };
            let Some(next) = waiting.iter().filter(|place| ready(place)).min_by_key(|&&place| &self.types[place].full_name).copied() else {
                return Err("bundles depending on one another in a ring, or on a bundle of another side".to_string());
            };
            waiting.retain(|&place| place != next);
            let bundle = &self.types[next];
            let Kind::Bundle(fields) = &bundle.kind else { unreachable!("bundles alone wait") };
            let library = DynamicLibrary::open(&fields.library)?;
            // SAFETY: built by the same compiler, against this very crate: the library's `bundle_set_up` is a `BundleSetUp`.
            let bundle_set_up = unsafe { library.function::<BundleSetUp>(BUNDLE_SET_UP) }?;
            let made = bundle_set_up(&TypeRegistryHandle { registry: &self, code: &code }).map_err(|why| format!("{}: {why}", bundle.full_name))?;
            let prefix = format!("{}{}", bundle.full_name, crate::type_names::NAME_SEPARATOR);
            let mut made_for = Vec::new();
            for (type_name, export) in made {
                let full_name = format!("{prefix}{type_name}");
                let Some(registered) = self.types.iter().find(|registered| registered.full_name == full_name) else {
                    return Err(format!("{}: code for {type_name}, a type the manifest does not give it", bundle.full_name));
                };
                if code.insert(registered.type_id, export).is_some() {
                    return Err(format!("{}: code for {type_name} made twice", bundle.full_name));
                }
                made_for.push(full_name);
            }
            if let Some(without) = self.types.iter().find(|registered| registered.full_name.starts_with(&prefix) && has_code(&registered.kind) && !made_for.contains(&registered.full_name)) {
                return Err(format!("{}: no code for {}", bundle.full_name, without.full_name));
            }
            set_up.push(bundle.full_name.clone());
            libraries.push(library);
        }
        Ok(LoadedRegistry { registry: self, code, libraries })
    }
}
