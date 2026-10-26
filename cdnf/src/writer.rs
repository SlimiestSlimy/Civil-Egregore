//! The writer ([`CdnfWriter`]): a document written as it is built, by a
//! typestate builder -- each state offers only what may come next there,
//! so whatever it writes parses back as it was built. No list without
//! an item, no pair without text before its colon, no element after a
//! section's subsections.
//!
//! A text is written plain where it reads back as it is, quoted where
//! not ([`needs_quotes`]); a list's items a dash a line, its last two
//! dashes; everything on a pair's right but a text, and a list an item
//! holds, a step deeper.

use std::fmt::Display;

/// A step of indentation: for the eye alone -- the parser lets spaces at
/// a line's start be.
const INDENTATION: &str = "    ";

/// Whether `text` must be quoted to read back as it is: a quote first --
/// one later is text -- a colon or a comment's mark in it, two line ends
/// in a row, or a line of it empty, starting with a dash, or with spaces
/// at an end.
fn needs_quotes(text: &str) -> bool {
    text.starts_with('"') || text.contains([':', '#']) || text.split('\n').any(|line| line.is_empty() || line.trim() != line || line.starts_with('-'))
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
    /// list, or a pair's right.
    pub trait ElementPlace {
        /// The state once the element is written.
        type After;
        /// What comes before an element of `kind` here, written: the steps
        /// in a pair's line is, or a list's items are.
        fn open(&mut self, written: &mut Written, kind: ElementKind) -> usize;
        /// The state once the element is written.
        fn after(self) -> Self::After;
    }

    /// A state sections may open in, and the state it is in once one has.
    pub trait HoldsSections {
        /// The state once a section has opened: sections alone after.
        type Subsections;
        /// The state once a section has opened.
        fn subsections(self) -> Self::Subsections;
    }
}

use own::{ElementKind, ElementPlace, HoldsSections, Written};

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

    /// `text`, plain -- its later lines as deep as its first -- or quoted.
    fn push_text(&mut self, text: &str) {
        if needs_quotes(text) {
            self.text.push_str(&format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")));
        } else {
            self.text.push_str(&text.replace('\n', &format!("\n{}", INDENTATION.repeat(self.line_depth))));
        }
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

/// An item's dash, on a line of its own, `depth` steps in: where it is,
/// and the steps a pair's line or a list's items are in after it.
fn open_item(written: &mut Written, depth: usize, kind: ElementKind) -> (usize, usize) {
    written.start_line(depth);
    let dash = written.text.len();
    written.text.push('-');
    match kind {
        ElementKind::Text | ElementKind::Pair => {
            written.text.push(' ');
            (dash, depth)
        }
        ElementKind::List => (dash, depth + 1),
    }
}

impl<Then> ElementPlace for EmptyList<Then> {
    type After = List<Then>;
    fn open(&mut self, written: &mut Written, kind: ElementKind) -> usize {
        let (dash, depth) = open_item(written, self.depth, kind);
        self.last_dash = dash;
        depth
    }
    fn after(self) -> List<Then> {
        List { depth: self.depth, last_dash: self.last_dash, then: self.then }
    }
}

impl<Then> ElementPlace for List<Then> {
    type After = Self;
    fn open(&mut self, written: &mut Written, kind: ElementKind) -> usize {
        let (dash, depth) = open_item(written, self.depth, kind);
        self.last_dash = dash;
        depth
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
    /// A text, written plain or quoted as it needs.
    pub fn text(mut self, text: impl Display) -> CdnfWriter<Place::After> {
        self.state.open(&mut self.written, ElementKind::Text);
        self.written.push_text(&text.to_string());
        CdnfWriter { written: self.written, state: self.state.after() }
    }

    /// A pair, `left` before its colon: its right next.
    pub fn pair(mut self, left: impl Display) -> CdnfWriter<PairRight<Place::After>> {
        let depth = self.state.open(&mut self.written, ElementKind::Pair);
        self.written.push_text(&left.to_string());
        self.written.text.push_str(" :");
        CdnfWriter { written: self.written, state: PairRight { depth, then: self.state.after() } }
    }

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
    /// A section named `name`, one colon deeper: its elements next.
    ///
    /// # Panics
    ///
    /// If `name` is none a section's line can hold: empty, spaces at an
    /// end, or a colon, a comment's mark or a line's end in it -- the
    /// caller's mistake, as every name it gives is checked.
    pub fn section(mut self, name: impl Display) -> CdnfWriter<SectionElements<State::Subsections>> {
        let name = name.to_string();
        assert!(!name.is_empty() && name.trim() == name && !name.contains([':', '#', '\n']), "`{name}`: no section's name a line can hold");
        self.written.colons += 1;
        let colons = ":".repeat(self.written.colons);
        self.written.gap = Gap::BlankLine;
        self.written.start_line(0);
        self.written.text.push_str(&format!("{colons}{name}{colons}"));
        CdnfWriter { written: self.written, state: SectionElements { parent: self.state.subsections() } }
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
