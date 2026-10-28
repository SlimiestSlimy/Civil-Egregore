//! Civil Egregore's type registry: the mods' manifests, read and checked
//! already ([`crate::manifests::read_mods`]), made into one flat list
//! of types ([`TypeRegistry::of_manifests`]) -- each mod a type too,
//! before its own -- each with its ID and its qualified name -- and
//! filtered to one side's ([`TypeRegistry::of_side`]), for that side to
//! load. Nothing is checked here: the manifests were.

use crate::type_kinds::{Kind, Side};
use crate::manifest_format::DeclaredDependency;
use crate::manifests::Manifest;
use crate::type_names::QualifiedTypeName;

/// Where an ID keeps its kind: its first byte, over its number.
pub const KIND_SHIFT: u32 = 56;

/// Numbers an ID can have: those of the seven bytes under its kind.
pub const TYPE_ID_NUMBERS: u64 = 1 << KIND_SHIFT;

/// The byte an ID of `kind` has first.
pub fn kind_byte(kind: &Kind) -> u8 {
    match kind {
        Kind::Plane { .. } => 0x01,
        Kind::ScaProc { .. } => 0x03,
        Kind::Mod { .. } => 0x09,
    }
}

/// A type as the registry keeps it: its ID, its qualified name, its kind
/// and its set up's static's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredType {
    /// Its ID: its kind's byte over its number.
    type_id: u64,
    /// Its qualified name: `mod` a mod's, `mod>type` a type's.
    qualified_name: String,
    /// Its kind, and the kind's fields.
    kind: Kind,
    /// The name its mod's library exports its set up's static as, if it
    /// has one.
    set_up_static_name: Option<String>,
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

    /// The name its set up's static is exported as, if it has one.
    pub fn set_up_static_name(&self) -> Option<&str> {
        self.set_up_static_name.as_deref()
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
        let mut push_type = |qualified_name: String, kind: Kind, set_up: Option<&str>| {
            let type_id = u64::from(kind_byte(&kind)) << KIND_SHIFT | types.len() as u64;
            types.push(RegisteredType { type_id, qualified_name, kind, set_up_static_name: set_up.map(str::to_string) });
        };
        for manifest in manifests {
            let document = manifest.document();
            let kind = Kind::Mod { side: document.side, depends_on: document.depends_on.iter().map(DeclaredDependency::dependency).collect(), package: (!document.is_container).then(|| manifest.package().clone()) };
            push_type(document.name.clone(), kind, None);
            for mod_type in &document.types {
                let type_name = QualifiedTypeName { mod_name: document.name.clone(), type_name: mod_type.name().to_string() };
                push_type(type_name.to_string(), mod_type.kind().clone(), mod_type.set_up_static_name());
            }
        }
        Self { types }
    }

    /// Every type, in the order given out.
    pub fn types(&self) -> &[RegisteredType] {
        &self.types
    }

    /// The mods of the common side and of `side`, each with its types:
    /// what `side` loads, every type's ID as given.
    pub fn of_side(&self, side: Side) -> TypeRegistry {
        let mut kept = true;
        let types = self.types.iter().filter(|registered| {
            if let Kind::Mod { side: mod_side, .. } = registered.kind {
                kept = mod_side == Side::Common || mod_side == side;
            }
            kept
        });
        Self { types: types.cloned().collect() }
    }

    /// The ID of the type of the qualified name `qualified_name`, if one
    /// is kept: a name gives an ID alone.
    pub fn type_id_of(&self, qualified_name: &str) -> Option<u64> {
        self.types.iter().find(|registered| registered.qualified_name == qualified_name).map(|registered| registered.type_id)
    }

    /// The type of ID `type_id`, an ID the registry gave out and kept:
    /// found by its number, the list counting up.
    pub fn type_of(&self, type_id: u64) -> &RegisteredType {
        let number = |id: u64| id % TYPE_ID_NUMBERS;
        let place = self.types.binary_search_by_key(&number(type_id), |registered| number(registered.type_id)).unwrap_or_else(|_| panic!("{type_id:#x}: no type kept of that ID"));
        &self.types[place]
    }
}
