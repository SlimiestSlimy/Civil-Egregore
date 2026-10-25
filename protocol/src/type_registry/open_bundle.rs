//! A bundle open in a registry ([`OpenBundle`]): every type registered
//! through it is in the bundle, its name written whole by the registry,
//! until it is dropped and the bundle it was opened in is open again.

use super::registered_type::RegisteredType;
use super::registration::{BundleSide, Registration};
use super::registry::{RegistryRefusal, TypeRegistry};
use super::type_ids::BundleType;

/// A bundle open in a registry: the registry itself to register through,
/// each type registered in the bundle, and once dropped, the bundle it
/// was opened in open again -- or none.
pub struct OpenBundle<'registry> {
    /// The registry the bundle is open in.
    type_registry: &'registry mut TypeRegistry,
    /// The bundle open before it, open again once it is dropped.
    outer_bundle: Option<String>,
    /// The bundle, registered.
    bundle: BundleType,
}

impl TypeRegistry {
    /// Registers the bundle named `bundle_name` -- in the bundle open, if
    /// any -- its code on `side`, depending on the bundles named
    /// `depends_on`, and opens it: the types registered through what comes
    /// back are in it, until it is dropped. Refused as any type would be.
    pub fn open_bundle(&mut self, bundle_name: &str, side: Option<BundleSide>, depends_on: &[&str]) -> Result<OpenBundle<'_>, RegistryRefusal> {
        let registered = self.register(Registration::bundle(bundle_name, side, depends_on))?;
        let RegisteredType::Bundle { name, bundle, .. } = registered else { unreachable!("a bundle registered as one") };
        let outer_bundle = self.open_bundle.replace(name);
        Ok(OpenBundle { type_registry: self, outer_bundle, bundle })
    }
}

impl OpenBundle<'_> {
    /// The bundle, as registered.
    pub fn bundle(&self) -> BundleType {
        self.bundle
    }
}

impl std::ops::Deref for OpenBundle<'_> {
    type Target = TypeRegistry;

    fn deref(&self) -> &TypeRegistry {
        self.type_registry
    }
}

impl std::ops::DerefMut for OpenBundle<'_> {
    fn deref_mut(&mut self) -> &mut TypeRegistry {
        self.type_registry
    }
}

impl Drop for OpenBundle<'_> {
    fn drop(&mut self) {
        self.type_registry.open_bundle = self.outer_bundle.take();
    }
}
