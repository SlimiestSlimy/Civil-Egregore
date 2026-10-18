//! The types a world was made with, as its save holds them
//! ([`FrozenTypes::to_text`]): each one's name and ID, in the order
//! registered. Loaded again ([`TypeRegistry::of_saved_text`]), each is
//! the type registered under its name -- the name joins it to its code
//! -- its ID checked to be the same.

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
    /// The types `types_text` names ([`FrozenTypes::to_text`]), in its
    /// order, each the one of `available` of its name, registered and
    /// checked as any type is; or what is wrong, naming the type: none
    /// of its name, or one of another ID.
    pub fn of_saved_text(available: &FrozenTypes, types_text: &str) -> Result<Self, String> {
        let mut csv_rows = csv::rows_with_header(types_text).into_iter();
        if csv_rows.next().is_none_or(|first_row| first_row != TYPES_COLUMNS) {
            return Err(format!("does not start with `{}`", TYPES_COLUMNS.join(",")));
        }
        let mut type_registry = Self::default();
        for csv_row in csv_rows {
            let wrong_row = || format!("`{}` is not a type's name and its ID in hexadecimal", csv_row.join(","));
            let [type_name, type_id] = &csv_row[..] else {
                return Err(wrong_row());
            };
            let (type_name, saved_id) = (type_name.trim(), u64::from_str_radix(type_id.trim(), 16).map_err(|_| wrong_row())?);
            let registered = available.named(type_name).ok_or_else(|| format!("the type `{type_name}` was saved, none of its name is registered"))?;
            if registered.type_id() != saved_id {
                return Err(format!("the type `{type_name}` was saved as {saved_id:016x}, is {:016x}", registered.type_id()));
            }
            type_registry.register(available.registration_of(registered)).map_err(|refusal| format!("the type `{type_name}`: {refusal:?}"))?;
        }
        Ok(type_registry)
    }
}
