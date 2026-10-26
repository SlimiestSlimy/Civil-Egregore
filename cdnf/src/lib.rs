//! The Colon Dash and Newline Format: natural text a program reads too
//! -- its rules, written in it: `cdnf.cdnf`, at this crate's root. A text
//! read ([`parse`]); a document written as it is built
//! ([`writer::CdnfWriter`]).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod parse;
mod tokens;
pub mod writer;
pub use parse::parse;

use std::fmt;

/// Why a text is no document: the line, counted from 1, and what is
/// wrong there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CdnfError {
    /// The line.
    pub line: usize,
    /// What is wrong.
    pub what: String,
}

impl fmt::Display for CdnfError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.what)
    }
}

impl std::error::Error for CdnfError {}

/// A document: the elements before its first section, and its sections
/// of one colon.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    /// What comes before any section.
    pub elements: Vec<Element>,
    /// Its sections of one colon, in order.
    pub sections: Vec<Section>,
}

/// A section: its name, its elements, and its sections of one colon
/// more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// Its name, between its colons.
    pub name: String,
    /// Its elements, in order.
    pub elements: Vec<Element>,
    /// Its subsections, in order.
    pub sections: Vec<Section>,
}

/// One element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Element {
    /// A text body, or a quoted text: the two read alike.
    Text(String),
    /// A list: its items, each an element.
    List(Vec<Element>),
    /// What came before a colon, paired with the next element.
    Pair(Box<Element>, Box<Element>),
}
