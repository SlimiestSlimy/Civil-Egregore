//! The types a world is given, registered as it starts
//! ([`TypeRegistry`]), then frozen ([`TypeRegistry::freeze`]): from
//! then on, as long as the world runs, no type enters. No two have one
//! name. Each is given its ID as it is registered: a new world's are
//! handed out as they are asked for, a saved world's are those its
//! save gave each name ([`TypeRegistry::for_saved_world`]).

use super::frozen_type_registry::FrozenTypeRegistry;
use super::registered_type::RegisteredType;
use super::registration::{Declared, Hook, Library, Registration};
use super::registered_type::Drawn;
use super::saved_types::SavedType;
use super::type_ids::{AttributeType, ClientCallType, EntityRuleType, EntityType, GuiType, LayerType, RenderRuleType, ScaRuleType, WorldgenType};

/// Why a type was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryRefusal {
    /// A type of its name is registered already.
    NameTaken,
    /// Its default attribute is no attribute registered before it.
    DefaultAmiss,
    /// Its master is nothing registered before it.
    MasterAmiss,
    /// A type it depends on is nothing registered before it.
    DependencyAmiss,
    /// The world's save gave its name an ID of another kind, or a
    /// plane of another shape.
    SavedAsAnother,
    /// The window it is nested in is no window registered before it.
    ParentAmiss,
}

/// The types being registered, as a world starts: none until
/// registered -- the base game's, then a mod's -- and frozen before it
/// runs.
#[derive(Clone)]
pub struct TypeRegistry {
    /// The types a world's save holds, by name: none for a new world.
    saved_types: Vec<SavedType>,
    /// The number the next type not saved is given: past every number
    /// taken. 0 is nobody's.
    next_number: u64,
    /// Every type registered, in the order registered.
    types: Vec<RegisteredType>,
    /// The places among them of the types each depends on.
    depends_on: Vec<Box<[usize]>>,
    /// The library each exposes, if any.
    libraries: Vec<Option<Library>>,
    /// What each is hooked into.
    hooks: Vec<Box<[Hook]>>,
}

impl TypeRegistry {
    /// The registry of a new world: no type registered, every ID
    /// handed out as its type is.
    pub fn for_new_world() -> Self {
        Self::of_saved_types(Vec::new())
    }

    /// The registry of a world whose save holds `saved_types`: each type
    /// registered of a name among them is given its ID, any other one
    /// past them all.
    pub(crate) fn of_saved_types(saved_types: Vec<SavedType>) -> Self {
        let next_number = saved_types.iter().map(|saved_type| saved_type.numbers().end).max().unwrap_or(0).max(1);
        Self { saved_types, next_number, types: Vec::new(), depends_on: Vec::new(), libraries: Vec::new(), hooks: Vec::new() }
    }

    /// Registers `registration`'s type, checked against every type before
    /// it: its name is no other's, its default attribute, its master and
    /// each type it depends on are registered; and given its ID -- the
    /// one saved for its name, which must be of what it is declared, or
    /// the next. It comes back with its ID: what whatever works on it
    /// is made with, its name not looked for again.
    pub fn register(&mut self, registration: Registration) -> Result<RegisteredType, RegistryRefusal> {
        let Registration { name, declared, depends_on, library, hooks } = registration;
        if self.types.iter().any(|registered| registered.name() == name) {
            return Err(RegistryRefusal::NameTaken);
        }
        let depended_on = depends_on.iter().map(|type_name| self.place_named(type_name)).collect::<Option<Box<[usize]>>>().ok_or(RegistryRefusal::DependencyAmiss)?;
        let saved_type = self.saved_types.iter().find(|saved_type| saved_type.name == name).cloned();
        // Its number: the saved ID's, or the next; checked against the saved type once it is made.
        let number = saved_type.as_ref().map_or(self.next_number, |saved_type| saved_type.numbers().start);
        let registered_type = self.made(name, &declared, number)?;
        if saved_type.as_ref().is_some_and(|saved_type| !saved_type.is(&registered_type)) {
            return Err(RegistryRefusal::SavedAsAnother);
        }
        if saved_type.is_none() {
            self.next_number = registered_type.numbers().end;
        }
        self.types.push(registered_type.clone());
        self.depends_on.push(depended_on);
        self.libraries.push(library);
        self.hooks.push(hooks.into_boxed_slice());
        Ok(registered_type)
    }

    /// Registers `registration`'s type as [`Self::register`] does, its
    /// library made of it registered (`library_of`): what a type whose
    /// library works on the type itself -- on its own ID -- is
    /// registered by.
    pub fn register_with_own_library(&mut self, registration: Registration, library_of: impl FnOnce(&RegisteredType) -> Library) -> Result<RegisteredType, RegistryRefusal> {
        let registered_type = self.register(registration)?;
        *self.libraries.last_mut().expect("a type just registered") = Some(library_of(&registered_type));
        Ok(registered_type)
    }

    /// The types registered, frozen: what a world runs with, no type
    /// entering from then on; or what is
    /// wrong: a type the world's save holds that none registered is.
    pub fn freeze(self) -> Result<FrozenTypeRegistry, String> {
        if let Some(SavedType { name: saved_name, .. }) = self.saved_types.iter().find(|saved_type| self.place_named(&saved_type.name).is_none()) {
            return Err(format!("the type `{saved_name}` was saved, none of its name is registered"));
        }
        Ok(FrozenTypeRegistry::of_registered(self.types, self.depends_on, self.libraries, self.hooks))
    }

    /// The place of the type named `type_name` among those registered, if one is.
    fn place_named(&self, type_name: &str) -> Option<usize> {
        self.types.iter().position(|registered| registered.name() == type_name)
    }

    /// The type named `name`, `declared` so, of number `number`: what it
    /// is of found among the types registered before it.
    fn made(&self, name: String, declared: &Declared, number: u64) -> Result<RegisteredType, RegistryRefusal> {
        let registered_named = |type_name: &str| self.place_named(type_name).map(|place| &self.types[place]);
        Ok(match declared {
            Declared::Layer => RegisteredType::Layer { name, layer_type: LayerType::of_a_bit(number) },
            Declared::WidePlane { width_bits, tile_scale } => RegisteredType::WidePlane { name, layer_type: LayerType::of_tiles(number, *width_bits, *tile_scale) },
            Declared::Entity { default_attribute } => {
                let default_attribute = match default_attribute {
                    None => None,
                    Some(attribute_name) => match registered_named(attribute_name) {
                        Some(RegisteredType::Attribute { attribute_type, .. }) => Some(*attribute_type),
                        _ => return Err(RegistryRefusal::DefaultAmiss),
                    },
                };
                RegisteredType::Entity { name, entity_type: EntityType::from_number(number), default_attribute }
            }
            Declared::Attribute { block_count, data_words } => RegisteredType::Attribute { name, attribute_type: AttributeType::of_blocks(number, *block_count), data_words: Some(*data_words) },
            Declared::ScaRule { master } => {
                let master = registered_named(master).and_then(RegisteredType::layer_type).ok_or(RegistryRefusal::MasterAmiss)?;
                RegisteredType::ScaRule { name, rule: ScaRuleType::from_number(number), master }
            }
            Declared::EntityRule { master } => match registered_named(master) {
                Some(RegisteredType::Entity { entity_type, .. }) => RegisteredType::EntityRule { name, rule: EntityRuleType::from_number(number), master: *entity_type },
                _ => return Err(RegistryRefusal::MasterAmiss),
            },
            Declared::Worldgen => RegisteredType::Worldgen { name, worldgen: WorldgenType::from_number(number) },
            Declared::RenderRule { master } => {
                let master = match registered_named(master) {
                    Some(RegisteredType::Layer { layer_type, .. } | RegisteredType::WidePlane { layer_type, .. }) => Drawn::Plane(*layer_type),
                    Some(RegisteredType::Entity { entity_type, .. }) => Drawn::Entity(*entity_type),
                    _ => return Err(RegistryRefusal::MasterAmiss),
                };
                RegisteredType::RenderRule { name, rule: RenderRuleType::from_number(number), master }
            }
            Declared::Gui { parent } => {
                let parent = match parent {
                    None => None,
                    Some(parent_name) => match registered_named(parent_name) {
                        Some(RegisteredType::Gui { gui, .. }) => Some(*gui),
                        _ => return Err(RegistryRefusal::ParentAmiss),
                    },
                };
                RegisteredType::Gui { name, gui: GuiType::from_number(number), parent }
            }
            Declared::ClientCall { master } => {
                let master = registered_named(master).map(RegisteredType::type_id).ok_or(RegistryRefusal::MasterAmiss)?;
                RegisteredType::ClientCall { name, call: ClientCallType::from_number(number), master }
            }
        })
    }
}
