//! The types a world was made with, as its save holds them
//! ([`FrozenTypeRegistry::to_text`]): each one's name and ID, in the order
//! registered. Read again ([`TypeRegistry::for_saved_world`]), each
//! name is given its ID back as its type is registered -- the name
//! joins it to its code.

use super::frozen_type_registry::FrozenTypeRegistry;
use super::registry::TypeRegistry;
use super::type_ids::numbers_taken;
use utilities::csv;

/// The columns of the types' text.
const TYPES_COLUMNS: [&str; 2] = ["type", "id"];

impl FrozenTypeRegistry {
    /// As a save holds them: a row a type, its name and its ID in
    /// hexadecimal, in the order registered.
    pub fn to_text(&self) -> String {
        let types = self.types().iter().map(|registered| csv::row_text(&[registered.name().to_string(), format!("{:016x}", registered.type_id())]));
        std::iter::once(csv::row_text(&TYPES_COLUMNS)).chain(types).collect()
    }

    /// Whether these are the types of the world whose types `types_text`
    /// names: each it names here of the ID it was saved with -- what a
    /// world opened with them must be. Or what is wrong.
    pub fn agree_with_saved(&self, types_text: &str) -> Result<(), String> {
        for (type_name, saved_id) in saved_ids_of(types_text)? {
            match self.types().iter().find(|registered| registered.name() == type_name) {
                Some(registered) if registered.type_id() == saved_id => {}
                Some(registered) => return Err(format!("`{type_name}` saved as {saved_id:016x}, frozen as {:016x}", registered.type_id())),
                None => return Err(format!("`{type_name}` saved, and not among the types frozen")),
            }
        }
        Ok(())
    }
}

impl TypeRegistry {
    /// The registry of the world whose types `types_text` names
    /// ([`FrozenTypeRegistry::to_text`]): each name given its ID as its type is
    /// registered ([`TypeRegistry::of_saved_ids`]); or what is wrong
    /// with the text -- a name said twice, or two of its IDs sharing a
    /// number, are.
    pub fn for_saved_world(types_text: &str) -> Result<Self, String> {
        Ok(Self::of_saved_ids(saved_ids_of(types_text)?))
    }
}

/// Each type `types_text` names, with its ID; or what is wrong with the
/// text -- a name said twice, or two of its IDs sharing a number, are.
fn saved_ids_of(types_text: &str) -> Result<Vec<(String, u64)>, String> {
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
    for (place, (type_name, type_id)) in saved_ids.iter().enumerate() {
        let shares_with = |(other_name, other_id): &&(String, u64)| *other_name == *type_name || { let (numbers, other_numbers) = (numbers_taken(*type_id), numbers_taken(*other_id)); numbers.start < other_numbers.end && other_numbers.start < numbers.end };
        if let Some((other_name, _)) = saved_ids[place + 1..].iter().find(shares_with) {
            return Err(format!("`{type_name}` and `{other_name}` share a name or a number"));
        }
    }
    Ok(saved_ids)
}
