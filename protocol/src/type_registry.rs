//! Civil Egregore's type registry: the one `u64` namespace every type
//! in a world is drawn from, the IDs' types, and the registry a world
//! is given its types in ([`TypeRegistry`]), each by a name no other
//! has. Which types: none of its own -- initialization registers them.
//!
//! The design: `docs/type_registry.md`; item by item:
//! `docs/type_registry_reference.md`.

mod frozen_type_registry;
mod registered_type;
mod registration;
mod saved_types;
mod registry;
mod type_ids;

pub use frozen_type_registry::FrozenTypeRegistry;
pub use registered_type::RegisteredType;
pub use registration::{Declared, Hook, Library, Registration};
pub use registry::{RegistryRefusal, TypeRegistry};
pub use type_ids::{AttributeType, EntityRuleType, EntityType, LayerType, TypeKind, ScaRuleType, WorldgenType, ATTRIBUTE_NUMBERS, TYPE_ID_NUMBERS};
