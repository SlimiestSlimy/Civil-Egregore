//! The types a world runs with ([`FrozenTypes`]): frozen as it
//! starts, constant as long as it runs -- what is worked out of them
//! once stays true. Changed only between worlds.

use crate::registered_type::RegisteredType;
use crate::registration::{Hook, Library, Registration};
use crate::type_ids::{AttributeType, EntityType, LayerType};

/// The types a world runs with, in the order registered: the rules
/// tick in it. Read only: no type enters after
/// [`crate::TypeRegistry::freeze`].
#[derive(Clone)]
pub struct FrozenTypes {
    /// Every type, in the order registered.
    types: Box<[RegisteredType]>,
    /// The places among them of the types each depends on.
    depends_on: Box<[Box<[usize]>]>,
    /// The library each exposes, if any.
    libraries: Box<[Option<Library>]>,
    /// What each is hooked into.
    hooks: Box<[Box<[Hook]>]>,
}

impl FrozenTypes {
    /// `types`, registered and checked, frozen.
    pub(crate) fn of_registered(types: Vec<RegisteredType>, depends_on: Vec<Box<[usize]>>, libraries: Vec<Option<Library>>, hooks: Vec<Box<[Hook]>>) -> Self {
        Self { types: types.into_boxed_slice(), depends_on: depends_on.into_boxed_slice(), libraries: libraries.into_boxed_slice(), hooks: hooks.into_boxed_slice() }
    }

    /// The place among them of `registered`, one of theirs.
    fn place_of(&self, registered: &RegisteredType) -> usize {
        self.types.iter().position(|of_these| std::ptr::eq(of_these, registered)).expect("one of these types")
    }

    /// The types `registered`, one of these, depends on, in the order it
    /// names them.
    pub fn dependencies_of(&self, registered: &RegisteredType) -> impl Iterator<Item = &RegisteredType> {
        self.depends_on[self.place_of(registered)].iter().map(|&place| &self.types[place])
    }

    /// The library `registered`, one of these, exposes, if any.
    pub fn library_of(&self, registered: &RegisteredType) -> Option<&Library> {
        self.libraries[self.place_of(registered)].as_ref()
    }

    /// `registered`, one of these, as it was registered: what registers
    /// it again among other types.
    pub fn registration_of(&self, registered: &RegisteredType) -> Registration
    {
        let depends_on = self.dependencies_of(registered).map(|dependency| dependency.name().to_string()).collect();
        Registration { registered_type: registered.clone(), depends_on, library: self.library_of(registered).copied(), hooks: self.hooks_of(registered).to_vec() }
    }

    /// What `registered`, one of these, is hooked into.
    pub fn hooks_of(&self, registered: &RegisteredType) -> &[Hook] {
        &self.hooks[self.place_of(registered)]
    }

    /// The types hooked into `hook`, in the order registered, each with its
    /// library, if it has one.
    pub fn hooked(&self, hook: Hook) -> impl Iterator<Item = (&RegisteredType, Option<Library>)> {
        self.types.iter().zip(&self.hooks).zip(&self.libraries).filter(move |((_, hooks), _)| hooks.contains(&hook)).map(|((registered, _), library)| (registered, *library))
    }

    /// Every type, in the order registered.
    pub fn types(&self) -> &[RegisteredType] {
        &self.types
    }

    /// The type named `type_name`, if one is.
    pub fn named(&self, type_name: &str) -> Option<&RegisteredType> {
        self.types.iter().find(|registered| registered.name() == type_name)
    }

    /// The type `type_number` is one of the numbers of, if any: a wide
    /// plane's for any of its cold planes'.
    pub fn of_number(&self, type_number: u64) -> Option<&RegisteredType> {
        self.types.iter().find(|registered| registered.numbers().contains(&type_number))
    }

    /// The type of entity `entity_type`, if it is registered.
    pub fn entity(&self, entity_type: EntityType) -> Option<&RegisteredType> {
        self.types.iter().find(|registered| matches!(registered, RegisteredType::Entity { entity_type: registered_entity, .. } if *registered_entity == entity_type))
    }

    /// The words the data of the attribute `attribute_type` is, if it is
    /// registered and of a fixed size.
    pub fn data_words_of(&self, attribute_type: AttributeType) -> Option<usize> {
        self.types.iter().find_map(|registered| match registered {
            RegisteredType::Attribute { attribute_type: registered_attribute, data_words, .. } if registered_attribute.type_identity() == attribute_type.type_identity() => *data_words,
            _ => None,
        })
    }

    /// Every rule, of the cells and of entities, in the order they tick.
    pub fn rules(&self) -> impl Iterator<Item = &RegisteredType> {
        self.types.iter().filter(|registered| registered.is_a_rule())
    }

    /// The rules whose master is `master`: those of a plane, or of a
    /// type of entity.
    pub fn rules_of<'registry>(&'registry self, master: &'registry RegisteredType) -> impl Iterator<Item = &'registry RegisteredType> {
        self.rules().filter(move |rule| match rule {
            RegisteredType::ScaRule { master: rule_master, .. } => master.layer_type() == Some(*rule_master),
            RegisteredType::EntityRule { master: rule_master, .. } => matches!(master, RegisteredType::Entity { entity_type, .. } if entity_type == rule_master),
            _ => false,
        })
    }

    /// The master of `rule`: the plane or the type of entity it is of.
    pub fn master_of(&self, rule: &RegisteredType) -> Option<&RegisteredType> {
        match rule {
            RegisteredType::ScaRule { master, .. } => self.types.iter().find(|registered| registered.layer_type() == Some(*master)),
            RegisteredType::EntityRule { master, .. } => self.entity(*master),
            _ => None,
        }
    }

    /// Every type of entity with a default attribute, each with that
    /// attribute's type and the words its data is: what keeping an
    /// entity asks of the registry, copied out once.
    pub fn default_attributes(&self) -> Vec<(EntityType, AttributeType, usize)> {
        self.types
            .iter()
            .filter_map(|registered| match registered {
                RegisteredType::Entity { entity_type, default_attribute: Some(default_attribute), .. } => Some((*entity_type, *default_attribute, self.data_words_of(*default_attribute)?)),
                _ => None,
            })
            .collect()
    }

    /// Every layer type a world's cells have, as the hot bitmaps know
    /// them: each layer, and each wide plane whole. Dirt has none: it is
    /// a cell with nothing on it.
    pub fn layer_types(&self) -> Vec<LayerType> {
        self.types.iter().filter_map(RegisteredType::layer_type).collect()
    }
}
