//! A type as it is registered ([`Registration`]): what it is, the types
//! it depends on by name, and the library it exposes to those that
//! depend on it ([`Library`]). Any type may expose one.

use crate::registered_type::RegisteredType;
use std::any::Any;

/// What a type exposes to the types depending on it: a handle to its
/// API -- a reference to a `static` of the type's own, so copied
/// freely -- that each is given by the type's ID and casts to what it
/// knows the API as. The registry knows nothing of what is in it.
pub type Library = &'static (dyn Any + Send + Sync);

/// A type to register, with the names of the types it depends on --
/// each registered before it -- and its library, if it has one.
#[derive(Clone)]
pub struct Registration<RuleCode> {
    /// The type.
    pub registered_type: RegisteredType<RuleCode>,
    /// The names of the types it depends on.
    pub depends_on: Vec<String>,
    /// What it exposes to those depending on it.
    pub library: Option<Library>,
}

impl<RuleCode> Registration<RuleCode> {
    /// The same, depending as well on the types named `type_names`.
    pub fn depending_on(mut self, type_names: &[&str]) -> Self {
        self.depends_on.extend(type_names.iter().map(|type_name| type_name.to_string()));
        self
    }

    /// The same, exposing `library` to the types depending on it.
    pub fn with_library(self, library: Library) -> Self {
        Self { library: Some(library), ..self }
    }
}

impl<RuleCode> From<RegisteredType<RuleCode>> for Registration<RuleCode> {
    /// `registered_type`, depending on nothing, exposing nothing.
    fn from(registered_type: RegisteredType<RuleCode>) -> Self {
        Self { registered_type, depends_on: Vec::new(), library: None }
    }
}

impl<RuleCode> RegisteredType<RuleCode> {
    /// Its registration, depending on the types named `type_names`.
    pub fn depending_on(self, type_names: &[&str]) -> Registration<RuleCode> {
        Registration::from(self).depending_on(type_names)
    }

    /// Its registration, exposing `library` to the types depending on it.
    pub fn with_library(self, library: Library) -> Registration<RuleCode> {
        Registration::from(self).with_library(library)
    }
}
