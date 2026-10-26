//! The registry's libraries loaded for one side ([`TypeRegistry::load`]):
//! every mod of the common side and that one, dependencies first, each
//! library read from its mod's package and opened -- none, a
//! container's -- and each of its types with a
//! set up function set up -- the function its manifest names, a [`TypeSetUp`], given the
//! registry and the code of the types before it, to take what it needs
//! of them once.

use super::TypeRegistry;
use crate::kinds::{Kind, ModFields, Side};
use std::any::Any;
use std::collections::HashMap;
use utilities::dynamic_library::{library_file_name, DynamicLibrary};

/// What a type's set up function makes of it: its code, or what its
/// dependents take of it -- for them, and for the side, to downcast to
/// what they know it as.
pub type Export = Box<dyn Any + Send + Sync>;

/// A type's set up function, as its mod's library exports it, by the
/// name its manifest gives: its code, made with what it takes of the
/// registry and of the types before it.
pub type TypeSetUp = fn(&TypeRegistryHandle<'_>) -> Result<Export, String>;

/// What a type is set up with: the registry -- every type's ID and full
/// name -- and the code of every type set up before it.
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

    /// What the set up function of the type of the full name `full_name`
    /// made of it, if it was set up before: to downcast.
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

impl TypeRegistry {
    /// Its libraries loaded for `side` -- the common mods' and the
    /// side's, each after every mod it depends on that loads on it, the
    /// first by name among those ready -- each mod's types set up in its
    /// order: or what went wrong.
    pub fn load(self, side: Side) -> Result<LoadedRegistry, String> {
        let loads_here = |fields: &ModFields| fields.side == Side::Common || fields.side == side;
        let mods: HashMap<&str, &ModFields> = self.types.iter().filter_map(|registered| match &registered.kind {
            Kind::Mod(fields) => Some((registered.full_name.as_str(), fields)),
            _ => None,
        }).collect();
        let mut waiting: Vec<usize> = (0..self.types.len()).filter(|&place| matches!(&self.types[place].kind, Kind::Mod(fields) if loads_here(fields))).collect();
        let (mut code, mut libraries, mut loaded) = (HashMap::new(), Vec::new(), Vec::<&str>::new());
        while !waiting.is_empty() {
            let ready = |place: &usize| {
                let Kind::Mod(fields) = &self.types[*place].kind else { unreachable!("mods alone wait") };
                // A common container carries mods of every side: those of another side never load here.
                fields.depends_on.iter().all(|(dependency, _)| loaded.contains(&dependency.as_str()) || !loads_here(mods[dependency.as_str()]))
            };
            let next = waiting.iter().filter(|place| ready(place)).min_by_key(|&&place| &self.types[place].full_name).copied().expect("a mod ready: the manifests refuse rings of dependencies");
            waiting.retain(|&place| place != next);
            let mod_type = &self.types[next];
            loaded.push(&mod_type.full_name);
            let Kind::Mod(fields) = &mod_type.kind else { unreachable!("mods alone wait") };
            // A container's library is none, nor its types.
            let Some(package) = &fields.library else { continue };
            let library_bytes = package.read_entry(&library_file_name(&mod_type.full_name))?;
            let library = DynamicLibrary::from_bytes(&mod_type.full_name, &library_bytes)?;
            // A mod's types follow it, up to the next mod.
            for registered in self.types[next + 1..].iter().take_while(|registered| !matches!(registered.kind, Kind::Mod(_))) {
                let Some(function_name) = &registered.set_up else { continue };
                // SAFETY: built by the same compiler, against this very crate: the function the manifest names is a `TypeSetUp`.
                let type_set_up = unsafe { library.function::<TypeSetUp>(function_name) }.map_err(|why| format!("{}: {why}", registered.full_name))?;
                let made = type_set_up(&TypeRegistryHandle { registry: &self, code: &code }).map_err(|why| format!("{}: {why}", registered.full_name))?;
                code.insert(registered.type_id, made);
            }
            libraries.push(library);
        }
        Ok(LoadedRegistry { registry: self, code, libraries })
    }
}
