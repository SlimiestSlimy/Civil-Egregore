//! The types a world was made with, as its save holds them
//! ([`FrozenTypeRegistry::to_text`]): each one's name and ID -- and a
//! plane's shape -- in the order registered. Read again ([`TypeRegistry::for_saved_world`]), each
//! name is given its ID back as its type is registered -- the name
//! joins it to its code.

use super::frozen_type_registry::FrozenTypeRegistry;
use super::registered_type::RegisteredType;
use super::registry::TypeRegistry;
use super::type_ids::numbers_taken;
use utilities::csv;

/// The columns of the types' text: a plane's shape blank for any other type.
const TYPES_COLUMNS: [&str; 4] = ["type", "id", "bits a cell", "tile scale"];

/// A type as a save holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SavedType {
    /// Its name.
    pub(crate) name: String,
    /// Its ID.
    pub(crate) type_id: u64,
    /// A plane's bits a cell and tiles' scale; none for any other type.
    pub(crate) plane_shape: Option<(u32, u32)>,
}

impl SavedType {
    /// The numbers its ID takes ([`RegisteredType::numbers`]).
    pub(crate) fn numbers(&self) -> std::ops::Range<u64> {
        numbers_taken(self.type_id, self.plane_shape)
    }

    /// Whether `registered` is it: of its ID, and a plane of its shape.
    pub(crate) fn is(&self, registered: &RegisteredType) -> bool {
        registered.type_id() == self.type_id && registered.plane_shape() == self.plane_shape
    }
}

impl FrozenTypeRegistry {
    /// As a save holds them: a row a type, its name, its ID in
    /// hexadecimal and a plane's shape, in the order registered.
    pub fn to_text(&self) -> String {
        let types = self.types().iter().map(|registered| {
            let (width_bits, tile_scale) = registered.plane_shape().map_or((String::new(), String::new()), |(width_bits, tile_scale)| (width_bits.to_string(), tile_scale.to_string()));
            csv::row_text(&[registered.name().to_string(), format!("{:016x}", registered.type_id()), width_bits, tile_scale])
        });
        std::iter::once(csv::row_text(&TYPES_COLUMNS)).chain(types).collect()
    }

    /// Whether these are the types of the world whose types `types_text`
    /// names: each it names here of the ID and shape it was saved with
    /// -- what a world opened with them must be. Or what is wrong.
    pub fn agree_with_saved(&self, types_text: &str) -> Result<(), String> {
        for saved_type in saved_types_of(types_text)? {
            let type_name = &saved_type.name;
            match self.types().iter().find(|registered| registered.name() == type_name) {
                Some(registered) if saved_type.is(registered) => {}
                Some(registered) => return Err(format!("`{type_name}` saved as {:016x} {:?}, frozen as {:016x} {:?}", saved_type.type_id, saved_type.plane_shape, registered.type_id(), registered.plane_shape())),
                None => return Err(format!("`{type_name}` saved, and not among the types frozen")),
            }
        }
        Ok(())
    }
}

impl TypeRegistry {
    /// The registry of the world whose types `types_text` names
    /// ([`FrozenTypeRegistry::to_text`]): each name given its ID as its type is
    /// registered ([`TypeRegistry::of_saved_types`]); or what is wrong
    /// with the text -- a name said twice, or two of its IDs sharing a
    /// number, are.
    pub fn for_saved_world(types_text: &str) -> Result<Self, String> {
        Ok(Self::of_saved_types(saved_types_of(types_text)?))
    }
}

/// Each type `types_text` names, with its ID and a plane's shape; or
/// what is wrong with the text -- a name said twice, or two of its IDs
/// sharing a number, are.
fn saved_types_of(types_text: &str) -> Result<Vec<SavedType>, String> {
    let mut csv_rows = csv::rows_with_header(types_text).into_iter();
    if csv_rows.next().is_none_or(|first_row| first_row != TYPES_COLUMNS) {
        return Err(format!("does not start with `{}`", TYPES_COLUMNS.join(",")));
    }
    let saved_types = csv_rows
        .map(|csv_row| {
            let wrong_row = || format!("`{}` is not a type's name, its ID in hexadecimal, and a plane's bits a cell and tile scale", csv_row.join(","));
            let [type_name, type_id, width_bits, tile_scale] = &csv_row[..] else {
                return Err(wrong_row());
            };
            let type_id = u64::from_str_radix(type_id.trim(), 16).map_err(|_| wrong_row())?;
            let plane_shape = match (width_bits.trim(), tile_scale.trim()) {
                ("", "") => None,
                (width_bits, tile_scale) => Some((width_bits.parse().map_err(|_| wrong_row())?, tile_scale.parse().map_err(|_| wrong_row())?)),
            };
            Ok(SavedType { name: type_name.trim().to_string(), type_id, plane_shape })
        })
        .collect::<Result<Vec<_>, String>>()?;
    for (place, saved_type) in saved_types.iter().enumerate() {
        let shares_with = |other: &&SavedType| other.name == saved_type.name || { let (numbers, other_numbers) = (saved_type.numbers(), other.numbers()); numbers.start < other_numbers.end && other_numbers.start < numbers.end };
        if let Some(other) = saved_types[place + 1..].iter().find(shares_with) {
            return Err(format!("`{}` and `{}` share a name or a number", saved_type.name, other.name));
        }
    }
    Ok(saved_types)
}
