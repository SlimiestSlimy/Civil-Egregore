//! The types a world runs with ([`FrozenTypeRegistry`]): frozen as it
//! starts, constant as long as it runs -- what is worked out of them
//! once stays true. Changed only between worlds.

use super::registered_type::RegisteredType;
use super::registration::Hook;
use super::type_ids::{AttributeType, EntityType, PlaneType};

/// The types a world runs with, in the order registered: the rules
/// tick in it. Read only: no type enters after
/// [`super::TypeRegistry::freeze`].
#[derive(Clone)]
pub struct FrozenTypeRegistry {
    /// Every type, in the order registered.
    types: Box<[RegisteredType]>,
    /// What each is hooked into.
    hooks: Box<[Box<[Hook]>]>,
}

impl FrozenTypeRegistry {
    /// `types`, registered and checked, frozen.
    pub(crate) fn of_registered(types: Vec<RegisteredType>, hooks: Vec<Box<[Hook]>>) -> Self {
        Self { types: types.into_boxed_slice(), hooks: hooks.into_boxed_slice() }
    }

    /// The place among them of `registered`, one of theirs.
    fn place_of(&self, registered: &RegisteredType) -> usize {
        self.types.iter().position(|of_these| std::ptr::eq(of_these, registered)).expect("one of these types")
    }

    /// What `registered`, one of these, is hooked into.
    pub fn hooks_of(&self, registered: &RegisteredType) -> &[Hook] {
        &self.hooks[self.place_of(registered)]
    }

    /// The types hooked into `hook`, in the order registered.
    pub fn hooked(&self, hook: Hook) -> impl Iterator<Item = &RegisteredType> {
        self.types.iter().zip(&self.hooks).filter(move |(_, hooks)| hooks.contains(&hook)).map(|(registered, _)| registered)
    }

    /// Every type, in the order registered.
    pub fn types(&self) -> &[RegisteredType] {
        &self.types
    }

    /// The type of number `type_number`, if any.
    pub fn of_number(&self, type_number: u64) -> Option<&RegisteredType> {
        self.types.iter().find(|registered| registered.type_number() == type_number)
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

    /// Every plane type a world's cells have. Dirt has none: it is
    /// a cell with nothing on it.
    pub fn plane_types(&self) -> Vec<PlaneType> {
        self.types.iter().filter_map(RegisteredType::plane_type).collect()
    }
}

impl std::fmt::Debug for FrozenTypeRegistry {
    /// Every type, in the order registered.
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.debug_list().entries(self.types.iter()).finish()
    }
}
