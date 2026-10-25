//! Civil Egregore's type registry: the one `u64` namespace every type
//! in a world is drawn from, the IDs' types, and the registry a world
//! is given its types in ([`TypeRegistry`]), each by a name no other
//! has. Which types: none of its own -- initialization registers them.
//!
//! The design: `docs/type_registry.md`; item by item:
//! `docs/type_registry_reference.md`.

mod frozen_type_registry;
mod manifest_file;
mod open_bundle;
mod registered_type;
mod registration;
mod saved_types;
mod registry;
mod type_ids;

pub use frozen_type_registry::FrozenTypeRegistry;
pub use manifest_file::{manifest_text, parse_manifest, ManifestError, ManifestRow};
pub use registered_type::RegisteredType;
pub use registration::{BundleSide, Declared, Hook, Registration};
pub use open_bundle::OpenBundle;
pub use registry::{bundle_of, in_bundle, RegistryRefusal, TypeRegistry, BUNDLE_SEPARATOR};
pub use type_ids::{AttributeType, BundleType, ClientCallType, EntityRuleType, EntityType, GuiType, PlaneTileSize, PlaneType, PlaneWidth, RenderRuleType, TypeKind, ScaRuleType, WorldgenType, TYPE_ID_NUMBERS};
