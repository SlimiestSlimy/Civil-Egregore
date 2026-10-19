//! The types a world is given, registered as it starts
//! ([`TypeRegistry`]), then frozen ([`TypeRegistry::freeze`]): from
//! then on, as long as the world runs, no type enters. No two have one
//! name. Each is given its ID as it is registered: a new world's are
//! handed out as they are asked for, a saved world's are those its
//! save gave each name ([`TypeRegistry::for_saved_world`]).
//!
//! Every type is in a bundle: one is opened ([`TypeRegistry::open_bundle`]),
//! the types registered while it is open are in it -- a bundle too --
//! and the registry writes their names whole, the bundle's name first
//! (`basegame::sheep`). In a bundle, a name with no `::` in it is one
//! of the bundle's own; another bundle's is written whole.

use super::frozen_type_registry::FrozenTypeRegistry;
use super::registered_type::RegisteredType;
use super::registration::{Declared, Hook, Library, Registration};
use super::registered_type::Drawn;
use super::type_ids::{AttributeType, BundleType, ClientCallType, EntityRuleType, EntityType, GuiType, LayerType, RenderRuleType, ScaRuleType, WorldgenType};

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
    /// The world's save gave its name an ID of another kind.
    SavedAsAnother,
    /// The window it is nested in is no window registered before it.
    ParentAmiss,
    /// It is no bundle and no bundle is open: every type is in one.
    OutsideABundle,
    /// Its name is empty or holds `::`, which the registry alone writes.
    NameAmiss,
}

/// What a whole name puts between a bundle's name and a name in it.
pub const BUNDLE_SEPARATOR: &str = "::";

/// The whole name of the type named `type_name` in the bundle whose
/// whole name is `bundle`: what a test, or a type of another bundle,
/// finds it by.
pub fn in_bundle(bundle: &str, type_name: &str) -> String {
    format!("{bundle}{BUNDLE_SEPARATOR}{type_name}")
}

/// The types being registered, as a world starts: none until
/// registered -- the base game's, then a mod's -- and frozen before it
/// runs.
#[derive(Clone)]
pub struct TypeRegistry {
    /// The IDs a world's save gave the names it holds: none for a new world.
    saved_ids: Vec<(String, u64)>,
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
    /// The whole name of the bundle open, if any: the one the types
    /// registered now are in.
    pub(super) open_bundle: Option<String>,
}

impl TypeRegistry {
    /// The registry of a new world: no type registered, every ID
    /// handed out as its type is.
    pub fn for_new_world() -> Self {
        Self::of_saved_ids(Vec::new())
    }

    /// The registry of a world whose save gave `saved_ids` -- each a
    /// type's name and ID: each type registered of a name it holds is
    /// given its ID, any other the number after them all.
    pub fn of_saved_ids(saved_ids: Vec<(String, u64)>) -> Self {
        let next_number = saved_ids.iter().map(|&(_, saved_id)| (saved_id & (super::TYPE_ID_NUMBERS - 1)) + 1).max().unwrap_or(1);
        Self { saved_ids, next_number, types: Vec::new(), depends_on: Vec::new(), libraries: Vec::new(), hooks: Vec::new(), open_bundle: None }
    }

    /// Registers `registration`'s type in the bundle open, checked
    /// against every type before it: it is in a bundle, its name is
    /// whole no other's, its default attribute, its master and each type
    /// it depends on are registered; and given its ID -- the one saved
    /// for its name, which must be of what it is declared, or the next.
    /// It comes back with its ID and its name whole: what whatever works
    /// on it is made with, its name not looked for again.
    pub fn register(&mut self, registration: Registration) -> Result<RegisteredType, RegistryRefusal> {
        let Registration { name, declared, depends_on, library, hooks } = registration;
        if name.is_empty() || name.contains(BUNDLE_SEPARATOR) {
            return Err(RegistryRefusal::NameAmiss);
        }
        if self.open_bundle.is_none() && declared != Declared::Bundle {
            return Err(RegistryRefusal::OutsideABundle);
        }
        let name = self.whole_name(&name);
        if self.types.iter().any(|registered| registered.name() == name) {
            return Err(RegistryRefusal::NameTaken);
        }
        let depended_on = depends_on.iter().map(|type_name| self.place_named(&self.whole_name(type_name))).collect::<Option<Box<[usize]>>>().ok_or(RegistryRefusal::DependencyAmiss)?;
        let saved_id = self.saved_ids.iter().find(|(saved_name, _)| *saved_name == name).map(|&(_, saved_id)| saved_id);
        // Its number: the saved ID's, or the next; checked against the saved ID once it is made.
        let number = saved_id.map_or(self.next_number, |saved_id| saved_id & (super::TYPE_ID_NUMBERS - 1));
        let registered_type = self.made(name, &declared, number)?;
        if saved_id.is_some_and(|saved_id| saved_id != registered_type.type_id()) {
            return Err(RegistryRefusal::SavedAsAnother);
        }
        if saved_id.is_none() {
            self.next_number = number + 1;
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
        if let Some((saved_name, _)) = self.saved_ids.iter().find(|(saved_name, _)| self.place_named(saved_name).is_none()) {
            return Err(format!("the world's types name `{saved_name}`, and the game registers no type of that name: nothing to load it as"));
        }
        Ok(FrozenTypeRegistry::of_registered(self.types, self.depends_on, self.libraries, self.hooks))
    }

    /// The whole name `type_name` stands for in the bundle open: itself if
    /// it is whole already -- another bundle's, written so -- or with no
    /// bundle open; else the open bundle's name, then it.
    fn whole_name(&self, type_name: &str) -> String {
        match &self.open_bundle {
            Some(bundle) if !type_name.contains(BUNDLE_SEPARATOR) => in_bundle(bundle, type_name),
            _ => type_name.to_string(),
        }
    }

    /// The place of the type whose whole name is `type_name` among those registered, if one is.
    fn place_named(&self, type_name: &str) -> Option<usize> {
        self.types.iter().position(|registered| registered.name() == type_name)
    }

    /// The type whose whole name is `name`, `declared` so, of number
    /// `number`: what it is of found among the types registered before
    /// it, by the names it gives as the bundle open reads them.
    fn made(&self, name: String, declared: &Declared, number: u64) -> Result<RegisteredType, RegistryRefusal> {
        let registered_named = |type_name: &str| self.place_named(&self.whole_name(type_name)).map(|place| &self.types[place]);
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
            Declared::RenderRule { master, render_layer } => {
                let master = match registered_named(master) {
                    Some(RegisteredType::Layer { layer_type, .. } | RegisteredType::WidePlane { layer_type, .. }) => Drawn::Plane(*layer_type),
                    Some(RegisteredType::Entity { entity_type, .. }) => Drawn::Entity(*entity_type),
                    _ => return Err(RegistryRefusal::MasterAmiss),
                };
                RegisteredType::RenderRule { name, rule: RenderRuleType::from_number(number), master, render_layer: *render_layer }
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
            Declared::Bundle => RegisteredType::Bundle { name, bundle: BundleType::from_number(number) },
        })
    }
}
