//! A type as it is registered ([`Registration`]): what it is, the types
//! it depends on by name, the library it exposes ([`Library`]) -- any
//! type may expose one -- and what it is hooked into ([`Hook`]): what
//! the server does with it, and its library, beyond keeping it.

use crate::registered_type::RegisteredType;
use std::any::Any;

/// What a type exposes to the types depending on it: a handle to its
/// API -- a reference to a `static` of the type's own, so copied
/// freely -- that each is given by the type's ID and casts to what it
/// knows the API as. The registry knows nothing of what is in it.
pub type Library = &'static (dyn Any + Send + Sync);

/// What a type is hooked into: what the simulation does with it, and
/// with its library, beyond keeping it. One more is one more variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// Its library generates the world: the cells of each superchunk,
    /// and what lives on it. A world has one.
    GeneratesTheWorld,
    /// Its library is run on every hot superchunk every tick: a rule's,
    /// of its master.
    RunsEveryTick,
    /// It is a type of entity the world is kept hot about: its halos
    /// follow these. A world has one for now, unless forced hot.
    KeepsTheWorldHot,
}

/// A type to register, with the names of the types it depends on --
/// each registered before it -- its library, if it has one, and what it
/// is hooked into.
#[derive(Clone)]
pub struct Registration {
    /// The type.
    pub registered_type: RegisteredType,
    /// The names of the types it depends on.
    pub depends_on: Vec<String>,
    /// What it exposes to those depending on it, and runs it if it is
    /// hooked into anything.
    pub library: Option<Library>,
    /// What it is hooked into.
    pub hooks: Vec<Hook>,
}

impl Registration {
    /// The same, depending as well on the types named `type_names`.
    pub fn depending_on(mut self, type_names: &[&str]) -> Self {
        self.depends_on.extend(type_names.iter().map(|type_name| type_name.to_string()));
        self
    }

    /// The same, exposing `library` to the types depending on it.
    pub fn with_library(self, library: Library) -> Self {
        Self { library: Some(library), ..self }
    }

    /// The same, hooked into `hooks` as well.
    pub fn with_hooks(mut self, hooks: &[Hook]) -> Self {
        self.hooks.extend_from_slice(hooks);
        self
    }
}

impl From<RegisteredType> for Registration {
    /// `registered_type`, depending on nothing, exposing nothing.
    fn from(registered_type: RegisteredType) -> Self {
        Self { registered_type, depends_on: Vec::new(), library: None, hooks: Vec::new() }
    }
}

impl RegisteredType {
    /// Its registration, depending on the types named `type_names`.
    pub fn depending_on(self, type_names: &[&str]) -> Registration {
        Registration::from(self).depending_on(type_names)
    }

    /// Its registration, exposing `library` to the types depending on it.
    pub fn with_library(self, library: Library) -> Registration {
        Registration::from(self).with_library(library)
    }

    /// Its registration, hooked into `hooks`.
    pub fn with_hooks(self, hooks: &[Hook]) -> Registration {
        Registration::from(self).with_hooks(hooks)
    }
}
