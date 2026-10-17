//! The types a world is given, registered as it starts
//! ([`TypeRegistry`]), then frozen ([`TypeRegistry::freeze`]): from
//! then on, as long as the world runs, no type enters. No two have one
//! name.

use crate::frozen_types::FrozenTypes;
use crate::registered_type::RegisteredType;
use crate::registration::{Library, Registration};

/// Why a type was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryRefusal {
    /// A type of its name is registered already.
    NameTaken,
    /// Its default attribute is no attribute of a fixed size registered
    /// before it.
    DefaultAmiss,
    /// Its master is nothing registered before it.
    MasterAmiss,
    /// A type it depends on is nothing registered before it.
    DependencyAmiss,
}

/// The types being registered, as a world starts: none until
/// registered -- the base game's, then a mod's -- and frozen before it
/// runs.
#[derive(Clone)]
pub struct TypeRegistry<RuleCode> {
    /// Every type registered, in the order registered.
    types: Vec<RegisteredType<RuleCode>>,
    /// The places among them of the types each depends on.
    depends_on: Vec<Box<[usize]>>,
    /// The library each exposes, if any.
    libraries: Vec<Option<Library>>,
}

impl<RuleCode> Default for TypeRegistry<RuleCode> {
    /// No type registered.
    fn default() -> Self {
        Self { types: Vec::new(), depends_on: Vec::new(), libraries: Vec::new() }
    }
}

impl<RuleCode> TypeRegistry<RuleCode> {
    /// Registers `registration`'s type, checked against every type before
    /// it: its name is no other's, its default attribute, its master and
    /// each type it depends on are registered.
    pub fn register(&mut self, registration: impl Into<Registration<RuleCode>>) -> Result<(), RegistryRefusal> {
        let Registration { registered_type, depends_on, library } = registration.into();
        if self.types.iter().any(|registered| registered.name() == registered_type.name()) {
            return Err(RegistryRefusal::NameTaken);
        }
        let attribute_of_a_fixed_size = |wanted: crate::AttributeType| self.types.iter().any(|registered| matches!(registered, RegisteredType::Attribute { attribute_type, data_words: Some(_), .. } if attribute_type.type_identity() == wanted.type_identity()));
        match &registered_type {
            RegisteredType::Entity { default_attribute: Some(default_attribute), .. } if !attribute_of_a_fixed_size(*default_attribute) => return Err(RegistryRefusal::DefaultAmiss),
            RegisteredType::ScaRule { master, .. } if !self.types.iter().any(|registered| registered.layer_type() == Some(*master)) => return Err(RegistryRefusal::MasterAmiss),
            RegisteredType::EntityRule { master, .. } if !self.types.iter().any(|registered| matches!(registered, RegisteredType::Entity { entity_type, .. } if entity_type == master)) => return Err(RegistryRefusal::MasterAmiss),
            _ => {}
        }
        let depended_on = depends_on.iter().map(|type_name| self.types.iter().position(|registered| registered.name() == type_name)).collect::<Option<Box<[usize]>>>().ok_or(RegistryRefusal::DependencyAmiss)?;
        self.types.push(registered_type);
        self.depends_on.push(depended_on);
        self.libraries.push(library);
        Ok(())
    }

    /// The types registered, frozen: what a world runs with, no type
    /// entering from then on.
    pub fn freeze(self) -> FrozenTypes<RuleCode> {
        FrozenTypes::of_registered(self.types, self.depends_on, self.libraries)
    }
}
