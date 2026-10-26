//! A list of fields, each a name paired with its value or a flag, its
//! name alone, taken one by one ([`FieldList`]).

use cdnf::Element;

/// A list of fields, each a name paired with its value or a flag, its
/// name alone, taken one by one: none may be left.
pub(super) struct FieldList<'document> {
    /// The fields not taken yet: each one's name and value.
    fields: Vec<(&'document str, &'document Element)>,
    /// The flags not taken yet.
    flags: Vec<&'document str>,
}

impl<'document> FieldList<'document> {
    /// The fields `items` are: each a name paired with its value, or a
    /// flag, no name twice.
    pub(super) fn of(items: &'document [Element]) -> Result<Self, String> {
        let (mut fields, mut flags) = (Vec::new(), Vec::new());
        for item in items {
            let field_name = match item {
                Element::Text(flag) => flag,
                Element::Pair(left, _) => match &**left {
                    Element::Text(field_name) => field_name,
                    _ => return Err("a field's name is text".to_string()),
                },
                Element::List(_) => return Err("a field is its name paired with its value, or a flag, its name alone".to_string()),
            };
            if fields.iter().any(|&(taken, _)| taken == field_name) || flags.contains(&field_name.as_str()) {
                return Err(format!("{field_name}: given twice"));
            }
            match item {
                Element::Pair(_, value) => fields.push((field_name.as_str(), &**value)),
                _ => flags.push(field_name.as_str()),
            }
        }
        Ok(Self { fields, flags })
    }

    /// Whether the flag `flag_name` is given, taken.
    pub(super) fn flag(&mut self, flag_name: &str) -> bool {
        let place = self.flags.iter().position(|&each| each == flag_name);
        place.map(|place| self.flags.remove(place)).is_some()
    }

    /// The field `field_name`, taken, if there is one.
    pub(super) fn take(&mut self, field_name: &str) -> Option<&'document Element> {
        let place = self.fields.iter().position(|&(each, _)| each == field_name)?;
        Some(self.fields.remove(place).1)
    }

    /// The text of the field `field_name`, if there is one.
    pub(super) fn optional_text(&mut self, field_name: &str) -> Result<Option<&'document str>, String> {
        match self.take(field_name) {
            None => Ok(None),
            Some(Element::Text(text)) => Ok(Some(text)),
            Some(_) => Err(format!("{field_name}: text")),
        }
    }

    /// The text of the field `field_name`, which there must be.
    pub(super) fn text(&mut self, field_name: &str) -> Result<&'document str, String> {
        self.optional_text(field_name)?.ok_or_else(|| format!("{field_name}: missing"))
    }

    /// The items of the field `field_name`'s list: none if there is no
    /// such field.
    pub(super) fn optional_list(&mut self, field_name: &str) -> Result<&'document [Element], String> {
        match self.take(field_name) {
            None => Ok(&[]),
            Some(Element::List(items)) => Ok(items),
            Some(_) => Err(format!("{field_name}: a list")),
        }
    }

    /// Nothing left: or the first field or flag `taker` takes none of.
    pub(super) fn none_left(&self, taker: &str) -> Result<(), String> {
        match (self.fields.first(), self.flags.first()) {
            (None, None) => Ok(()),
            (Some((field_name, _)), _) => Err(format!("{field_name}: no field {taker} takes")),
            (None, Some(flag_name)) => Err(format!("{flag_name}: no flag {taker} takes")),
        }
    }
}
