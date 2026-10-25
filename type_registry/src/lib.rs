//! Civil Egregore's type registry: the one `u64` namespace every type
//! in a world is drawn from, the IDs' types, and the registry a world
//! is given its types in ([`TypeRegistry`]), each by a name no other
//! has. Which types: none of its own -- initialization registers them.
//!
//! Out of protocol (2026-10-25), on its own for a while, to be made to
//! take what the manifest reader validates.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod frozen_type_registry;
mod open_bundle;
mod registered_type;
mod registration;
mod saved_types;
mod registry;
mod type_ids;

pub use frozen_type_registry::FrozenTypeRegistry;
pub use registered_type::{Drawn, RegisteredType};
pub use registration::{Declared, Hook, Library, Registration};
pub use open_bundle::OpenBundle;
pub use registry::{in_bundle, root_bundle_of, RegistryRefusal, TypeRegistry, BUNDLE_SEPARATOR};
pub use type_ids::{AttributeType, BundleType, ClientCallType, EntityRuleType, EntityType, GuiType, LayerType, RenderRuleType, TypeKind, ScaRuleType, WorldgenType, TYPE_ID_NUMBERS};
