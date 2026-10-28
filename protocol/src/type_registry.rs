//! Civil Egregore's type registry: the mods' manifests, read and checked
//! already ([`crate::manifests::read_mods`]), made into one flat list
//! of types ([`TypeRegistry::of_manifests`]) -- each mod a type too,
//! before its own -- each with its ID and its qualified name. Nothing is
//! checked here: the manifests were.

use crate::type_kinds::{Hook, Kind, ModFields};
use crate::manifest_format::DeclaredDependency;
use crate::manifests::Manifest;
use crate::type_names::QualifiedTypeName;

mod library_loading;
pub use library_loading::{TypeCode, LoadedRegistry, TypeRegistryHandle, TypeSetUp};

/// Where an ID keeps its kind: its first byte, over its number.
pub const KIND_SHIFT: u32 = 56;

/// Numbers an ID can have: those of the seven bytes under its kind.
pub const TYPE_ID_NUMBERS: u64 = 1 << KIND_SHIFT;

/// The byte an ID of `kind` has first.
pub fn kind_byte(kind: &Kind) -> u8 {
    match kind {
        Kind::Attribute { .. } => 0x00,
        Kind::Plane { .. } => 0x01,
        Kind::Entity { .. } => 0x02,
        Kind::ScaProc { .. } => 0x03,
        Kind::EntityProc { .. } => 0x04,
        Kind::Worldgen => 0x05,
        Kind::RenderProc { .. } => 0x06,
        Kind::Gui { .. } => 0x07,
        Kind::ClientCall => 0x08,
        Kind::Mod(_) => 0x09,
    }
}

/// A type as the registry keeps it: its ID, its qualified name, its kind, its
/// hooks and its set up function's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredType {
    /// Its ID: its kind's byte over its number.
    type_id: u64,
    /// Its qualified name: `mod` a mod's, `mod>type` a type's.
    qualified_name: String,
    /// Its kind, and the kind's fields.
    kind: Kind,
    /// Its hooks.
    hooks: Vec<Hook>,
    /// The name its mod's library exports its set up function as, if it
    /// has one.
    set_up_function_name: Option<String>,
}

impl RegisteredType {
    /// Its ID.
    pub fn type_id(&self) -> u64 {
        self.type_id
    }

    /// Its qualified name.
    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
    }

    /// Its kind, and the kind's fields.
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// Its hooks.
    pub fn hooks(&self) -> &[Hook] {
        &self.hooks
    }

    /// The name its set up function is exported as, if it has one.
    pub fn set_up_function_name(&self) -> Option<&str> {
        self.set_up_function_name.as_deref()
    }
}

/// Every type of a world's mods, one flat list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeRegistry {
    /// The types, in the order given out: their numbers counting up.
    types: Vec<RegisteredType>,
}

impl TypeRegistry {
    /// The types of `manifests`, in their order: each mod first and then
    /// its types, numbered one after another from 0 whatever their kind
    /// -- as a world's are when it is made.
    pub fn of_manifests(manifests: &[Manifest]) -> Self {
        let mut types = Vec::new();
        let mut push_type = |qualified_name: String, kind: Kind, hooks: Vec<Hook>, set_up: Option<&str>| {
            let type_id = u64::from(kind_byte(&kind)) << KIND_SHIFT | types.len() as u64;
            types.push(RegisteredType { type_id, qualified_name, kind, hooks, set_up_function_name: set_up.map(str::to_string) });
        };
        for manifest in manifests {
            let document = manifest.document();
            let fields = ModFields { side: document.side, depends_on: document.depends_on.iter().map(DeclaredDependency::dependency).collect(), package: (!document.is_container).then(|| manifest.package().clone()) };
            push_type(document.name.clone(), Kind::Mod(fields), Vec::new(), None);
            for mod_type in &document.types {
                let type_name = QualifiedTypeName { mod_name: document.name.clone(), type_name: mod_type.name().to_string() };
                push_type(type_name.to_string(), mod_type.kind().clone(), mod_type.hooks().to_vec(), mod_type.set_up_function_name());
            }
        }
        Self { types }
    }

    /// Every type, in the order given out.
    pub fn types(&self) -> &[RegisteredType] {
        &self.types
    }

    /// The type of ID `type_id`, an ID the registry gave out: its number
    /// its place in the list.
    pub fn type_of(&self, type_id: u64) -> &RegisteredType {
        &self.types[(type_id % TYPE_ID_NUMBERS) as usize]
    }
}
