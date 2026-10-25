//! A mod's manifest, data alone ([`parse_manifest`], [`manifest_text`]):
//! a CSV row a type, in the order registered. A `bundle` row opens a
//! bundle, the rows after it are in it, and an `end` row naming it
//! closes it. The first row opens the mod's own bundle, which holds its
//! bundles alone, each on its side -- common, server or client -- and
//! depending on the bundles it names; they hold the types -- one level of
//! bundles for now. Columns are read by name, in
//! any order; a field a kind does not take is refused, as is a column no
//! manifest has.

use super::registered_type::RegisteredType;
use super::registration::{BundleSide, Declared, Hook, Registration};
use super::registry::TypeRegistry;
use super::type_ids::{PlaneTileSize, PlaneWidth};
use std::fmt;
use utilities::csv;

/// A manifest's columns, in the order written.
const COLUMNS: [&str; 13] = ["type", "kind", "side", "depends on", "hooks", "width", "tile size", "default attribute", "rule", "blocks", "data words", "draw order", "parent"];

/// Where each column is among [`COLUMNS`].
mod column {
    /// The type's name.
    pub(super) const TYPE: usize = 0;
    /// Its kind's word.
    pub(super) const KIND: usize = 1;
    /// A bundle's: the side its code runs on.
    pub(super) const SIDE: usize = 2;
    /// A bundle's: the bundles it depends on.
    pub(super) const DEPENDS_ON: usize = 3;
    /// Its hooks' words.
    pub(super) const HOOKS: usize = 4;
    /// A plane's width, in bits.
    pub(super) const WIDTH: usize = 5;
    /// A plane's tile size, in cells a side.
    pub(super) const TILE_SIZE: usize = 6;
    /// An entity's default attribute.
    pub(super) const DEFAULT_ATTRIBUTE: usize = 7;
    /// An entity's rule.
    pub(super) const RULE: usize = 8;
    /// An attribute's blocks.
    pub(super) const BLOCKS: usize = 9;
    /// An attribute's data words.
    pub(super) const DATA_WORDS: usize = 10;
    /// A render rule's draw order.
    pub(super) const DRAW_ORDER: usize = 11;
    /// A gui's parent.
    pub(super) const PARENT: usize = 12;
}

/// Between the items of a list in one field.
const LIST_SEPARATOR: char = ';';

/// Each side, and its word in a manifest.
const SIDE_WORDS: [(BundleSide, &str); 3] = [(BundleSide::Common, "common"), (BundleSide::Server, "server"), (BundleSide::Client, "client")];

/// Each hook, and its word in a manifest.
const HOOK_WORDS: [(Hook, &str); 3] = [(Hook::GeneratesTheWorld, "generates the world"), (Hook::KeepsTheWorldHot, "keeps the world hot"), (Hook::ShownByDefault, "shown by default")];

/// What a manifest lists, a row each: a type -- a bundle among them,
/// opened -- or the end of the bundle open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestRow {
    /// A type, registered in the bundle open: a bundle, opened.
    Type(Registration),
    /// The end of the bundle open, of this name.
    BundleEnd(String),
}

/// Why a manifest was refused: its row -- 0 for the one naming the
/// columns -- the column, and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    /// The row, counted from the first after the columns' names.
    pub row: usize,
    /// The column.
    pub column: &'static str,
    /// What is wrong.
    pub what: String,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "row {}, {}: {}", self.row, self.column, self.what)
    }
}

impl std::error::Error for ManifestError {}

/// What `manifest_text` lists, in order: or the first thing wrong. Every
/// type is in a bundle, every bundle ended, each by its own name.
pub fn parse_manifest(manifest_text: &str) -> Result<Vec<ManifestRow>, ManifestError> {
    let header_row = csv::rows_with_header(manifest_text).into_iter().next().unwrap_or_default();
    if let Some(unknown) = header_row.iter().find(|column_name| !COLUMNS.contains(&column_name.trim())) {
        return Err(ManifestError { row: 0, column: "columns", what: format!("no manifest has a column `{}`", unknown.trim()) });
    }
    for required in [column::TYPE, column::KIND] {
        if !header_row.iter().any(|column_name| column_name.trim() == COLUMNS[required]) {
            return Err(ManifestError { row: 0, column: "columns", what: format!("no column `{}`", COLUMNS[required]) });
        }
    }
    let (mut manifest_rows, mut bundles_open) = (Vec::new(), Vec::new());
    for (index, fields) in csv::rows_by_column(manifest_text, &COLUMNS).iter().enumerate() {
        let manifest_row = parsed_row(index + 1, fields)?;
        let amiss = |what: String| ManifestError { row: index + 1, column: COLUMNS[column::TYPE], what };
        if index > 0 && bundles_open.is_empty() {
            return Err(amiss("the mod's bundle is ended: one mod a manifest".to_string()));
        }
        match (&manifest_row, bundles_open.len()) {
            (ManifestRow::Type(Registration { name, declared: Declared::Bundle { side, depends_on }, .. }), 0 | 1) => {
                let amiss_in = |column: usize, what: &str| ManifestError { row: index + 1, column: COLUMNS[column], what: what.to_string() };
                match (bundles_open.is_empty(), side) {
                    (true, Some(_)) => return Err(amiss_in(column::SIDE, "the mod's bundle has no side: its bundles do")),
                    (true, None) if !depends_on.is_empty() => return Err(amiss_in(column::DEPENDS_ON, "the mod's bundle depends on none: its bundles do")),
                    (false, None) => return Err(amiss_in(column::SIDE, "missing: a bundle's code runs on common, server or client")),
                    _ => {}
                }
                bundles_open.push(name.clone());
            }
            (ManifestRow::Type(Registration { name, declared: Declared::Bundle { .. }, .. }), _) => return Err(amiss(format!("`{name}` is a bundle in a bundle of the mod: one level of bundles for now"))),
            (ManifestRow::Type(registration), 0) => return Err(amiss(format!("`{}` is in no bundle: the first row is the mod's", registration.name))),
            (ManifestRow::Type(registration), 1) => return Err(amiss(format!("`{}` is in the mod's own bundle, which holds bundles alone", registration.name))),
            (ManifestRow::Type(_), _) => {}
            (ManifestRow::BundleEnd(name), _) if bundles_open.last() == Some(name) => drop(bundles_open.pop()),
            (ManifestRow::BundleEnd(name), _) => return Err(amiss(format!("`{name}` is not the bundle open"))),
        }
        manifest_rows.push(manifest_row);
    }
    match bundles_open.last() {
        Some(unended) => Err(ManifestError { row: manifest_rows.len(), column: COLUMNS[column::TYPE], what: format!("the bundle `{unended}` is never ended") }),
        None if manifest_rows.is_empty() => Err(ManifestError { row: 1, column: COLUMNS[column::TYPE], what: "no row".to_string() }),
        None => Ok(manifest_rows),
    }
}

/// The row numbered `row`, its fields `fields`, one of [`COLUMNS`] each.
fn parsed_row(row: usize, fields: &[String]) -> Result<ManifestRow, ManifestError> {
    let field = |column: usize| fields[column].trim();
    let amiss = |column: usize, what: String| ManifestError { row, column: COLUMNS[column], what };
    let (name, kind) = (field(column::TYPE).to_string(), field(column::KIND));
    if name.is_empty() {
        return Err(amiss(column::TYPE, "no name".to_string()));
    }
    // What the kind takes: the fields it is made of; every other is to be empty.
    let (declared, taken): (Option<Declared>, &[usize]) = match kind {
        "end" => (None, &[]),
        "bundle" => {
            let side = optional(field(column::SIDE)).map(|side_word| SIDE_WORDS.iter().find(|(_, word)| *word == side_word).map(|&(side, _)| side).ok_or_else(|| amiss(column::SIDE, format!("`{side_word}` is no side: common, server or client")))).transpose()?;
            (Some(Declared::Bundle { side, depends_on: list(field(column::DEPENDS_ON)) }), &[column::SIDE, column::DEPENDS_ON, column::HOOKS])
        }
        "plane" => {
            let width = PlaneWidth::of_bits(number(field(column::WIDTH)).map_err(|what| amiss(column::WIDTH, what))?).ok_or_else(|| amiss(column::WIDTH, format!("{} is no plane width: 1, 2, 4, 8, 16 or 32", field(column::WIDTH))))?;
            let tile_size = PlaneTileSize::of_side(number(field(column::TILE_SIZE)).map_err(|what| amiss(column::TILE_SIZE, what))?).ok_or_else(|| amiss(column::TILE_SIZE, format!("{} is no tile size: 1, 2, 4 ... 256 cells a side", field(column::TILE_SIZE))))?;
            (Some(Declared::Plane { width, tile_size }), &[column::WIDTH, column::TILE_SIZE, column::HOOKS])
        }
        "entity" => (Some(Declared::Entity { default_attribute: optional(field(column::DEFAULT_ATTRIBUTE)), rule: optional(field(column::RULE)) }), &[column::DEFAULT_ATTRIBUTE, column::RULE, column::HOOKS]),
        "attribute" => {
            let block_count = number(field(column::BLOCKS)).map_err(|what| amiss(column::BLOCKS, what))?;
            let data_words = number(field(column::DATA_WORDS)).map_err(|what| amiss(column::DATA_WORDS, what))?;
            (Some(Declared::Attribute { block_count, data_words }), &[column::BLOCKS, column::DATA_WORDS, column::HOOKS])
        }
        "sca rule" => (Some(Declared::ScaRule), &[column::HOOKS]),
        "entity rule" => (Some(Declared::EntityRule), &[column::HOOKS]),
        "world generation" => (Some(Declared::Worldgen), &[column::HOOKS]),
        "render rule" => (Some(Declared::RenderRule { draw_order: number(field(column::DRAW_ORDER)).map_err(|what| amiss(column::DRAW_ORDER, what))? }), &[column::DRAW_ORDER, column::HOOKS]),
        "gui" => (Some(Declared::Gui { parent: optional(field(column::PARENT)) }), &[column::PARENT, column::HOOKS]),
        "client call" => (Some(Declared::ClientCall), &[column::HOOKS]),
        _ => return Err(amiss(column::KIND, format!("`{kind}` is no kind"))),
    };
    if let Some(stray) = (column::SIDE..COLUMNS.len()).filter(|column| !taken.contains(column)).find(|&column| !field(column).is_empty()) {
        return Err(amiss(stray, format!("{kind} has none")));
    }
    let Some(declared) = declared else {
        return Ok(ManifestRow::BundleEnd(name));
    };
    let hooks = list(field(column::HOOKS)).iter().map(|hook_word| HOOK_WORDS.iter().find(|(_, word)| word == hook_word).map(|&(hook, _)| hook).ok_or_else(|| amiss(column::HOOKS, format!("`{hook_word}` is no hook")))).collect::<Result<_, _>>()?;
    Ok(ManifestRow::Type(Registration { name, declared, hooks }))
}

/// The items of a list field, each trimmed: none if it is empty.
fn list(field: &str) -> Vec<String> {
    field.split(LIST_SEPARATOR).map(str::trim).filter(|item| !item.is_empty()).map(str::to_string).collect()
}

/// The name a field gives, if any.
fn optional(field: &str) -> Option<String> {
    (!field.is_empty()).then(|| field.to_string())
}

/// The number a field holds, or why it holds none.
fn number<N: std::str::FromStr>(field: &str) -> Result<N, String> {
    field.parse().map_err(|_| if field.is_empty() { "missing".to_string() } else { format!("`{field}` is no number") })
}

/// `rows` as a manifest's text: [`parse_manifest`] undone.
pub fn manifest_text(rows: &[ManifestRow]) -> String {
    let row_fields = |manifest_row: &ManifestRow| {
        let mut fields: [String; COLUMNS.len()] = Default::default();
        let registration = match manifest_row {
            ManifestRow::Type(registration) => registration,
            ManifestRow::BundleEnd(name) => {
                (fields[column::TYPE], fields[column::KIND]) = (name.clone(), "end".to_string());
                return csv::row_text(&fields);
            }
        };
        fields[column::TYPE] = registration.name.clone();
        fields[column::HOOKS] = registration.hooks.iter().map(|hook| HOOK_WORDS.iter().find(|(word_hook, _)| word_hook == hook).expect("every hook a word").1).collect::<Vec<_>>().join("; ");
        fields[column::KIND] = match &registration.declared {
            Declared::Bundle { side, depends_on } => {
                fields[column::SIDE] = side.map(|side| SIDE_WORDS.iter().find(|(word_side, _)| *word_side == side).expect("every side a word").1.to_string()).unwrap_or_default();
                fields[column::DEPENDS_ON] = depends_on.join("; ");
                "bundle"
            }
            Declared::Plane { width, tile_size } => {
                (fields[column::WIDTH], fields[column::TILE_SIZE]) = (width.bits().to_string(), tile_size.side().to_string());
                "plane"
            }
            Declared::Entity { default_attribute, rule } => {
                (fields[column::DEFAULT_ATTRIBUTE], fields[column::RULE]) = (default_attribute.clone().unwrap_or_default(), rule.clone().unwrap_or_default());
                "entity"
            }
            Declared::Attribute { block_count, data_words } => {
                (fields[column::BLOCKS], fields[column::DATA_WORDS]) = (block_count.to_string(), data_words.to_string());
                "attribute"
            }
            Declared::ScaRule => "sca rule",
            Declared::EntityRule => "entity rule",
            Declared::Worldgen => "world generation",
            Declared::RenderRule { draw_order } => {
                fields[column::DRAW_ORDER] = draw_order.to_string();
                "render rule"
            }
            Declared::Gui { parent } => {
                fields[column::PARENT] = parent.clone().unwrap_or_default();
                "gui"
            }
            Declared::ClientCall => "client call",
        }
        .to_string();
        csv::row_text(&fields)
    };
    std::iter::once(csv::row_text(&COLUMNS)).chain(rows.iter().map(row_fields)).collect()
}

impl TypeRegistry {
    /// Registers what `manifest_text` lists, in order, each type in the
    /// bundle open as it is listed: or the first thing wrong, the row it is
    /// on -- a type refused among them. No bundle is to be open.
    pub fn register_manifest(&mut self, manifest_text: &str) -> Result<(), ManifestError> {
        debug_assert!(self.open_bundle.is_none(), "a manifest's bundles are opened at the top: none open");
        let mut bundles_open: Vec<String> = Vec::new();
        let registered = parse_manifest(manifest_text)?.into_iter().enumerate().try_for_each(|(index, manifest_row)| {
            self.open_bundle = bundles_open.last().cloned();
            match manifest_row {
                ManifestRow::Type(registration) => match self.register(registration) {
                    Ok(RegisteredType::Bundle { name, .. }) => bundles_open.push(name),
                    Ok(_) => {}
                    Err(refusal) => return Err(ManifestError { row: index + 1, column: COLUMNS[column::TYPE], what: format!("refused: {refusal:?}") }),
                },
                ManifestRow::BundleEnd(_) => drop(bundles_open.pop()),
            }
            Ok(())
        });
        self.open_bundle = None;
        registered
    }
}
