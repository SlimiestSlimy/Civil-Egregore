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
use super::registration::{Declared, Hook, Registration};
use super::type_ids::{AttributeType, BundleType, ClientCallType, EntityRuleType, EntityType, GuiType, PlaneType, RenderRuleType, ScaRuleType, WorldgenType};

/// Why a type was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryRefusal {
    /// A type of its name is registered already.
    NameTaken,
    /// Its default attribute is no attribute registered before it.
    DefaultAmiss,
    /// Its rule is no entity rule registered before it.
    RuleAmiss,
    /// A bundle it depends on is no bundle registered before it.
    DependencyAmiss,
    /// A type it names is of a bundle its own does not depend on.
    OutsideItsDependencies,
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

/// The whole name of the bundle the type whose whole name is `type_name`
/// is in: what is before its last separator -- none, of a type in no
/// bundle.
pub fn bundle_of(type_name: &str) -> &str {
    type_name.rsplit_once(BUNDLE_SEPARATOR).map_or("", |(bundle, _)| bundle)
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
        Self { saved_ids, next_number, types: Vec::new(), hooks: Vec::new(), open_bundle: None }
    }

    /// Registers `registration`'s type in the bundle open, checked
    /// against every type before it: it is in a bundle, its name is
    /// whole no other's, each type it names is registered, of its own mod
    /// or one its mod depends on; and given its ID -- the one saved for
    /// its name, which must be of what it is declared, or the next.
    /// It comes back with its ID and its name whole: what whatever works
    /// on it is made with, its name not looked for again.
    pub fn register(&mut self, registration: Registration) -> Result<RegisteredType, RegistryRefusal> {
        let Registration { name, declared, hooks } = registration;
        if name.is_empty() || name.contains(BUNDLE_SEPARATOR) {
            return Err(RegistryRefusal::NameAmiss);
        }
        if self.open_bundle.is_none() && !matches!(declared, Declared::Bundle { .. }) {
            return Err(RegistryRefusal::OutsideABundle);
        }
        let name = self.whole_name(&name);
        if self.types.iter().any(|registered| registered.name() == name) {
            return Err(RegistryRefusal::NameTaken);
        }
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
        self.hooks.push(hooks.into_boxed_slice());
        Ok(registered_type)
    }

    /// The types registered, frozen: what a world runs with, no type
    /// entering from then on; or what is
    /// wrong: a type the world's save holds that none registered is.
    pub fn freeze(self) -> Result<FrozenTypeRegistry, String> {
        if let Some((saved_name, _)) = self.saved_ids.iter().find(|(saved_name, _)| self.place_named(saved_name).is_none()) {
            return Err(format!("the world's types name `{saved_name}`, and the game registers no type of that name: nothing to load it as"));
        }
        Ok(FrozenTypeRegistry::of_registered(self.types, self.hooks))
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

    /// The bundle registered of the whole name `bundle_name`, if one is.
    pub(super) fn bundle_named(&self, bundle_name: &str) -> Option<(BundleType, &[BundleType])> {
        self.place_named(bundle_name).and_then(|place| match &self.types[place] {
            RegisteredType::Bundle { bundle, depends_on, .. } => Some((*bundle, &depends_on[..])),
            _ => None,
        })
    }

    /// The type `type_name` names in the bundle open, if one is registered:
    /// refused if it is of a bundle neither the open one nor one it
    /// depends on.
    fn named(&self, type_name: &str) -> Result<Option<&RegisteredType>, RegistryRefusal> {
        let Some(place) = self.place_named(&self.whole_name(type_name)) else {
            return Ok(None);
        };
        let (named_bundle, own_bundle) = (bundle_of(self.types[place].name()), self.open_bundle.as_deref().unwrap_or(""));
        let depended_on = || self.bundle_named(own_bundle).zip(self.bundle_named(named_bundle)).is_some_and(|((_, own_depends_on), (named_bundle, _))| own_depends_on.contains(&named_bundle));
        if named_bundle == own_bundle || depended_on() { Ok(Some(&self.types[place])) } else { Err(RegistryRefusal::OutsideItsDependencies) }
    }

    /// The type whose whole name is `name`, `declared` so, of number
    /// `number`: the types it names found among those registered before
    /// it, by the names it gives as the bundle open reads them.
    fn made(&self, name: String, declared: &Declared, number: u64) -> Result<RegisteredType, RegistryRefusal> {
        Ok(match declared {
            Declared::Plane { width, tile_size } => RegisteredType::Plane { name, plane_type: PlaneType::new(number, *width, *tile_size) },
            Declared::Entity { default_attribute, rule } => {
                let default_attribute = match default_attribute {
                    None => None,
                    Some(attribute_name) => match self.named(attribute_name)? {
                        Some(RegisteredType::Attribute { attribute_type, .. }) => Some(*attribute_type),
                        _ => return Err(RegistryRefusal::DefaultAmiss),
                    },
                };
                let rule = match rule {
                    None => None,
                    Some(rule_name) => match self.named(rule_name)? {
                        Some(RegisteredType::EntityRule { rule, .. }) => Some(*rule),
                        _ => return Err(RegistryRefusal::RuleAmiss),
                    },
                };
                RegisteredType::Entity { name, entity_type: EntityType::from_number(number), default_attribute, rule }
            }
            Declared::Attribute { block_count, data_words } => RegisteredType::Attribute { name, attribute_type: AttributeType::of_blocks(number, *block_count), data_words: Some(*data_words) },
            Declared::ScaRule => RegisteredType::ScaRule { name, rule: ScaRuleType::from_number(number) },
            Declared::EntityRule => RegisteredType::EntityRule { name, rule: EntityRuleType::from_number(number) },
            Declared::Worldgen => RegisteredType::Worldgen { name, worldgen: WorldgenType::from_number(number) },
            Declared::RenderRule { draw_order } => RegisteredType::RenderRule { name, rule: RenderRuleType::from_number(number), draw_order: *draw_order },
            Declared::Gui { parent } => {
                let parent = match parent {
                    None => None,
                    Some(parent_name) => match self.named(parent_name)? {
                        Some(RegisteredType::Gui { gui, .. }) => Some(*gui),
                        _ => return Err(RegistryRefusal::ParentAmiss),
                    },
                };
                RegisteredType::Gui { name, gui: GuiType::from_number(number), parent }
            }
            Declared::ClientCall => RegisteredType::ClientCall { name, call: ClientCallType::from_number(number) },
            Declared::Bundle { side, depends_on } => {
                let depends_on = depends_on.iter().map(|bundle_name| self.bundle_named(&self.whole_name(bundle_name)).map(|(bundle, _)| bundle)).collect::<Option<_>>().ok_or(RegistryRefusal::DependencyAmiss)?;
                RegisteredType::Bundle { name, bundle: BundleType::from_number(number), side: *side, depends_on }
            }
        })
    }
}
