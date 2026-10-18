//! The types a world was made with, as its save holds them
//! ([`FrozenTypes::to_text`]): each one's name and ID, in the order
//! registered. Read again ([`TypeRegistry::for_saved_world`]), each
//! name is given its ID back as its type is registered -- the name
//! joins it to its code.

use super::frozen_types::FrozenTypes;
use super::registry::TypeRegistry;
use utilities::csv;

/// The columns of the types' text.
const TYPES_COLUMNS: [&str; 2] = ["type", "id"];

impl FrozenTypes {
    /// As a save holds them: a row a type, its name and its ID in
    /// hexadecimal, in the order registered.
    pub fn to_text(&self) -> String {
        let types = self.types().iter().map(|registered| csv::row_text(&[registered.name().to_string(), format!("{:016x}", registered.type_id())]));
        std::iter::once(csv::row_text(&TYPES_COLUMNS)).chain(types).collect()
    }
}

impl TypeRegistry {
    /// The registry of the world whose types `types_text` names
    /// ([`FrozenTypes::to_text`]): each name given its ID as its type is
    /// registered ([`TypeRegistry::of_saved_ids`]); or what is wrong
    /// with the text.
    pub fn for_saved_world(types_text: &str) -> Result<Self, String> {
        let mut csv_rows = csv::rows_with_header(types_text).into_iter();
        if csv_rows.next().is_none_or(|first_row| first_row != TYPES_COLUMNS) {
            return Err(format!("does not start with `{}`", TYPES_COLUMNS.join(",")));
        }
        let saved_ids = csv_rows
            .map(|csv_row| {
                let wrong_row = || format!("`{}` is not a type's name and its ID in hexadecimal", csv_row.join(","));
                let [type_name, type_id] = &csv_row[..] else {
                    return Err(wrong_row());
                };
                Ok((type_name.trim().to_string(), u64::from_str_radix(type_id.trim(), 16).map_err(|_| wrong_row())?))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self::of_saved_ids(saved_ids))
    }
}
