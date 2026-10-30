//! The writer ([`CdnfWriter`]): a document written as it is built, by a
//! typestate builder -- each state offers only what may come next there,
//! so whatever it writes parses back as it was built. No list without
//! an item, no pair without text before its colon, no element after a
//! section's subsections.
//!
//! A text is written plain where it reads back as it is, quoted where
//! not; a list's items a dash a line, its last two
//! dashes; everything on a pair's right but a text a step deeper.
//!
//! What takes a text can fail ([`CdnfError`]): a text that needs quotes
//! and ends in a backslash can't be written, nor a section's name that is
//! empty or of more than a line. So each returns a [`Result`], for `?`
//! before the next call.

use crate::tokens::COMMENT_MARK;
use crate::CdnfError;
use std::fmt::Display;

/// A step of indentation: for the eye alone -- the parser lets spaces at
/// a line's start be.
const INDENTATION: &str = "    ";

/// Whether `text` must be quoted to read back as it is: a quote first --
/// one later is text -- a comment's mark in it, or a line of it empty,
/// with spaces at an end, starting with a colon or an item's dashes, or
/// with a colon that makes a pair.
fn needs_quotes(text: &str) -> bool {
    let starts_an_item = |line: &str| {
        let after_dashes = line.trim_start_matches('-');
        (1..=2).contains(&(line.len() - after_dashes.len())) && (after_dashes.is_empty() || after_dashes.starts_with(char::is_whitespace))
    };
    let makes_a_pair = |line: &str| line.ends_with(':') || line.split(':').skip(1).any(|after_colon| after_colon.starts_with(char::is_whitespace));
    text.starts_with('"') || text.contains(COMMENT_MARK) || text.split('\n').any(|line| line.is_empty() || line.trim() != line || line.starts_with(':') || starts_an_item(line) || makes_a_pair(line))
}

/// What parts the next line from the last.
#[derive(Clone, Copy)]
enum Gap {
    /// A line's end.
    LineEnd,
    /// A blank line.
    BlankLine,
}

/// The writer's own: public for its methods' bounds alone, no name to
/// reach them by past this module.
mod own {
    use super::Gap;

    /// The text written so far, and what the next line needs.
    pub struct Written {
        /// The text.
        pub(super) text: String,
        /// What parts the next line from the last.
        pub(super) gap: Gap,
        /// The colons of the section open: none before any.
        pub(super) colons: usize,
        /// The steps the line written last is in: a plain text's later
        /// lines follow it.
        pub(super) line_depth: usize,
    }

    /// What kind of element opens.
    pub enum ElementKind {
        /// A text.
        Text,
        /// A pair.
        Pair,
        /// A list.
        List,
    }

    /// A place an element is written in: a section's level, an item of a
    /// list, or a pair's right. The last says what comes before a list
    /// there; an item, holding none, is never asked.
    pub trait ElementPlace {
        /// The state once the element is written.
        type After;
        /// What comes before an element of `kind` here, written: the steps
        /// in a pair's line is, or a list's items are.
        fn open(&mut self, written: &mut Written, kind: ElementKind) -> usize;
        /// The state once the element is written.
        fn after(self) -> Self::After;
    }

    /// A place a list may open in: any but a list's item, lists nesting
    /// through a pair's colon alone.
    pub trait HoldsLists: ElementPlace {}

    /// A state sections may open in, and the state it is in once one has.
    pub trait HoldsSections {
        /// The state once a section has opened: sections alone after.
        type Subsections;
        /// The state once a section has opened.
        fn subsections(self) -> Self::Subsections;
    }
}

use own::{ElementKind, ElementPlace, HoldsLists, HoldsSections, Written};

impl Written {
    /// A line started, `depth` steps in, after the gap due: a line's end
    /// the next one's, unless said otherwise.
    fn start_line(&mut self, depth: usize) {
        if !self.text.is_empty() {
            self.text.push_str(match self.gap {
                Gap::LineEnd => "\n",
                Gap::BlankLine => "\n\n",
            });
        }
        self.text.push_str(&INDENTATION.repeat(depth));
        (self.gap, self.line_depth) = (Gap::LineEnd, depth);
    }

    /// `what` is wrong on the line being written.
    fn refusal(&self, what: String) -> CdnfError {
        CdnfError { line: self.text.lines().count().max(1), what }
    }

    /// `text`, plain -- its later lines as deep as its first -- or quoted:
    /// or refused, needing quotes and ending in a backslash, which would
    /// escape the closing one.
    fn push_text(&mut self, text: &str) -> Result<(), CdnfError> {
        if !needs_quotes(text) {
            self.text.push_str(&text.replace('\n', &format!("\n{}", INDENTATION.repeat(self.line_depth))));
            return Ok(());
        }
        if text.ends_with('\\') {
            return Err(self.refusal(format!("`{text}`: a text that needs quotes can't end in a backslash")));
        }
        self.text.push_str(&format!("\"{}\"", text.replace('"', "\\\"")));
        Ok(())
    }
}

/// A document being written: what is written so far, and the state it
/// is in.
pub struct CdnfWriter<State> {
    /// What is written so far.
    written: Written,
    /// The state.
    state: State,
}

/// The document's own level: its elements, before any section.
pub struct Document;

/// The document's sections: no element of its own after them.
pub struct DocumentSections;

/// A section's elements, its parent's state `Parent` once it ends.
pub struct SectionElements<Parent> {
    /// The state once it ends.
    parent: Parent,
}

/// A section's subsections: no element of its own after them.
pub struct SectionSubsections<Parent> {
    /// The state once it ends.
    parent: Parent,
}

/// A list of no item yet: one there must be.
pub struct EmptyList<Then> {
    /// The steps its items are in.
    depth: usize,
    /// Where its first item's dash is, once written.
    last_dash: usize,
    /// The state once it ends.
    then: Then,
}

/// A list of an item or more, that may end.
pub struct List<Then> {
    /// The steps its items are in.
    depth: usize,
    /// Where its last item's dash is: the last made two.
    last_dash: usize,
    /// The state once it ends.
    then: Then,
}

/// A pair's right: one element, then the pair is written.
pub struct PairRight<Then> {
    /// The steps the pair's line is in.
    depth: usize,
    /// The state once the pair is written.
    then: Then,
}

impl CdnfWriter<Document> {
    /// A document, nothing written yet.
    pub fn new() -> Self {
        Self { written: Written { text: String::new(), gap: Gap::LineEnd, colons: 0, line_depth: 0 }, state: Document }
    }
}

impl Default for CdnfWriter<Document> {
    fn default() -> Self {
        Self::new()
    }
}

/// An element at a section's level, or the document's: a blank line
/// before it.
fn open_at_section_level(written: &mut Written, kind: ElementKind) -> usize {
    written.gap = Gap::BlankLine;
    if !matches!(kind, ElementKind::List) {
        written.start_line(0);
    }
    0
}

impl ElementPlace for Document {
    type After = Self;
    fn open(&mut self, written: &mut Written, kind: ElementKind) -> usize {
        open_at_section_level(written, kind)
    }
    fn after(self) -> Self {
        self
    }
}

impl<Parent> ElementPlace for SectionElements<Parent> {
    type After = Self;
    fn open(&mut self, written: &mut Written, kind: ElementKind) -> usize {
        open_at_section_level(written, kind)
    }
    fn after(self) -> Self {
        self
    }
}

/// An item's dash, on a line of its own, `depth` steps in: where it is.
fn open_item(written: &mut Written, depth: usize) -> usize {
    written.start_line(depth);
    let dash = written.text.len();
    written.text.push_str("- ");
    dash
}

impl<Then> ElementPlace for EmptyList<Then> {
    type After = List<Then>;
    fn open(&mut self, written: &mut Written, _kind: ElementKind) -> usize {
        self.last_dash = open_item(written, self.depth);
        self.depth
    }
    fn after(self) -> List<Then> {
        List { depth: self.depth, last_dash: self.last_dash, then: self.then }
    }
}

impl<Then> ElementPlace for List<Then> {
    type After = Self;
    fn open(&mut self, written: &mut Written, _kind: ElementKind) -> usize {
        self.last_dash = open_item(written, self.depth);
        self.depth
    }
    fn after(self) -> Self {
        self
    }
}

impl<Then> ElementPlace for PairRight<Then> {
    type After = Then;
    fn open(&mut self, written: &mut Written, kind: ElementKind) -> usize {
        match kind {
            ElementKind::Text => {
                written.text.push(' ');
                self.depth
            }
            ElementKind::Pair => {
                written.start_line(self.depth + 1);
                self.depth + 1
            }
            ElementKind::List => self.depth + 1,
        }
    }
    fn after(self) -> Then {
        self.then
    }
}

impl<Place: ElementPlace> CdnfWriter<Place> {
    /// A text, written plain or quoted as it needs: or refused, needing
    /// quotes and ending in a backslash.
    pub fn text(mut self, text: impl Display) -> Result<CdnfWriter<Place::After>, CdnfError> {
        self.state.open(&mut self.written, ElementKind::Text);
        self.written.push_text(&text.to_string())?;
        Ok(CdnfWriter { written: self.written, state: self.state.after() })
    }

    /// A pair, `left` before its colon: its right next. Or refused,
    /// `left` needing quotes and ending in a backslash.
    pub fn pair(mut self, left: impl Display) -> Result<CdnfWriter<PairRight<Place::After>>, CdnfError> {
        let depth = self.state.open(&mut self.written, ElementKind::Pair);
        self.written.push_text(&left.to_string())?;
        self.written.text.push_str(" :");
        Ok(CdnfWriter { written: self.written, state: PairRight { depth, then: self.state.after() } })
    }

}

impl HoldsLists for Document {}

impl<Parent> HoldsLists for SectionElements<Parent> {}

impl<Then> HoldsLists for PairRight<Then> {}

impl<Place: HoldsLists> CdnfWriter<Place> {
    /// A list: its first item next.
    pub fn list(mut self) -> CdnfWriter<EmptyList<Place::After>> {
        let depth = self.state.open(&mut self.written, ElementKind::List);
        CdnfWriter { written: self.written, state: EmptyList { depth, last_dash: 0, then: self.state.after() } }
    }
}

impl<Then> CdnfWriter<List<Then>> {
    /// The list ended: its last item's dash made two.
    pub fn end_list(mut self) -> CdnfWriter<Then> {
        self.written.text.insert(self.state.last_dash, '-');
        CdnfWriter { written: self.written, state: self.state.then }
    }
}

impl HoldsSections for Document {
    type Subsections = DocumentSections;
    fn subsections(self) -> DocumentSections {
        DocumentSections
    }
}

impl HoldsSections for DocumentSections {
    type Subsections = Self;
    fn subsections(self) -> Self {
        self
    }
}

impl<Parent> HoldsSections for SectionElements<Parent> {
    type Subsections = SectionSubsections<Parent>;
    fn subsections(self) -> SectionSubsections<Parent> {
        SectionSubsections { parent: self.parent }
    }
}

impl<Parent> HoldsSections for SectionSubsections<Parent> {
    type Subsections = Self;
    fn subsections(self) -> Self {
        self
    }
}

impl<State: HoldsSections> CdnfWriter<State> {
    /// A section named `name`, one colon deeper: its elements next. Or
    /// refused: a name empty or of more than a line, none a section's
    /// line can hold, or needing quotes and ending in a backslash.
    pub fn section(mut self, name: impl Display) -> Result<CdnfWriter<SectionElements<State::Subsections>>, CdnfError> {
        let name = name.to_string();
        self.written.colons += 1;
        let colons = ":".repeat(self.written.colons);
        self.written.gap = Gap::BlankLine;
        self.written.start_line(0);
        self.written.text.push_str(&colons);
        if name.is_empty() || name.contains('\n') {
            return Err(self.written.refusal(format!("`{name}`: a section's name is one line, and not empty")));
        }
        self.written.push_text(&name)?;
        self.written.text.push_str(&colons);
        Ok(CdnfWriter { written: self.written, state: SectionElements { parent: self.state.subsections() } })
    }
}

impl<Parent> CdnfWriter<SectionElements<Parent>> {
    /// The section ended.
    pub fn end_section(mut self) -> CdnfWriter<Parent> {
        self.written.colons -= 1;
        CdnfWriter { written: self.written, state: self.state.parent }
    }
}

impl<Parent> CdnfWriter<SectionSubsections<Parent>> {
    /// The section ended.
    pub fn end_section(mut self) -> CdnfWriter<Parent> {
        self.written.colons -= 1;
        CdnfWriter { written: self.written, state: self.state.parent }
    }
}

impl CdnfWriter<Document> {
    /// The document written.
    pub fn finish(self) -> String {
        finished(self.written)
    }
}

impl CdnfWriter<DocumentSections> {
    /// The document written.
    pub fn finish(self) -> String {
        finished(self.written)
    }
}

/// `written`'s text, its last line ended.
fn finished(written: Written) -> String {
    let mut text = written.text;
    if !text.is_empty() {
        text.push('\n');
    }
    text
}
