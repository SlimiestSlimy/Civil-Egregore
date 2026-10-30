//! The parser ([`parse`]): a text's tokens ([`crate::tokens`]) made into
//! its sections and their elements.

use crate::tokens::{tokens, Located, Token};
use crate::{CdnfError, Document, Element, Section};

/// `text` read as a document: or the first thing wrong in it.
pub fn parse(text: &str) -> Result<Document, CdnfError> {
    let mut parser = Parser { tokens: tokens(text)?, next: 0 };
    let elements = parser.elements()?;
    let sections = parser.sections(1)?;
    debug_assert!(parser.peek().is_none(), "every section read, every token read");
    Ok(Document { elements, sections })
}

/// A text's tokens being parsed.
struct Parser {
    /// The tokens.
    tokens: Vec<Located>,
    /// The token read next.
    next: usize,
}

impl Parser {
    /// The token read next, if any is left.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next).map(|located| &located.token)
    }

    /// The token read next, taken.
    fn take(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.next).map(|located| located.token.clone());
        self.next += 1;
        token
    }

    /// `what` is wrong at the token read next -- or at the last, past them all.
    fn refused<T>(&self, what: impl Into<String>) -> Result<T, CdnfError> {
        let line = self.tokens.get(self.next).or(self.tokens.last()).map_or(1, |located| located.line);
        Err(CdnfError { line, what: what.into() })
    }

    /// Past the line ends and blank lines up to what comes next.
    fn skip_line_ends(&mut self) {
        while matches!(self.peek(), Some(Token::LineEnd | Token::BlankLine)) {
            self.next += 1;
        }
    }

    /// The elements up to the next section, or the end.
    fn elements(&mut self) -> Result<Vec<Element>, CdnfError> {
        let mut elements = Vec::new();
        loop {
            self.skip_line_ends();
            if matches!(self.peek(), None | Some(Token::SectionHeader { .. })) {
                return Ok(elements);
            }
            elements.push(self.element()?);
        }
    }

    /// The next element, line ends before it let be: a list, a quoted
    /// text or a text body -- paired with the element after it if a
    /// colon follows.
    fn element(&mut self) -> Result<Element, CdnfError> {
        self.skip_line_ends();
        let first = match self.peek() {
            None => return self.refused("an element wanted, and the text ended"),
            Some(Token::SectionHeader { .. }) => return self.refused("an element wanted, and a section opens"),
            Some(Token::Colon) => return self.refused("a colon with nothing before it"),
            Some(Token::Dash | Token::LastDash) => return self.list(),
            Some(Token::Quoted(_)) => {
                let Some(Token::Quoted(quoted)) = self.take() else { unreachable!("peeked") };
                if matches!(self.peek(), Some(Token::Text(_))) {
                    return self.refused("no text body can follow a quoted text on the same line it finishes");
                }
                Element::Text(quoted)
            }
            Some(Token::Text(_)) => self.text_body(),
            Some(Token::LineEnd | Token::BlankLine) => unreachable!("line ends skipped"),
        };
        if self.peek() != Some(&Token::Colon) {
            return Ok(first);
        }
        self.next += 1;
        Ok(Element::Pair(Box::new(first), Box::new(self.element()?)))
    }

    /// A list: a dash and the element after it on its line, an item
    /// each, while a dash follows the last -- blank lines between let be
    /// -- until the item of two dashes, its last: so a list nested in an
    /// item ends, and the one it is in goes on.
    fn list(&mut self) -> Result<Element, CdnfError> {
        let mut items = Vec::new();
        while let Some(dash @ (Token::Dash | Token::LastDash)) = self.peek() {
            let last_item = *dash == Token::LastDash;
            self.next += 1;
            if matches!(self.peek(), None | Some(Token::LineEnd | Token::BlankLine)) {
                return self.refused("an empty list element");
            }
            items.push(self.element()?);
            if last_item {
                break;
            }
            self.skip_line_ends();
        }
        Ok(Element::List(items))
    }

    /// A text body: the texts of its lines, joined by line ends -- until
    /// a blank line, a dash, a section, a colon or the end.
    fn text_body(&mut self) -> Element {
        let mut body = String::new();
        while let Some(Token::Text(line_text)) = self.peek() {
            body.push_str(line_text);
            self.next += 1;
            match (self.peek(), self.tokens.get(self.next + 1).map(|located| &located.token)) {
                (Some(Token::LineEnd), Some(Token::Text(_))) => {
                    body.push('\n');
                    self.next += 1;
                }
                _ => break,
            }
        }
        Element::Text(body)
    }

    /// The sections of `depth` colons from here, each with its elements
    /// and its sections of one colon more, until one of fewer colons or
    /// the end.
    fn sections(&mut self, depth: usize) -> Result<Vec<Section>, CdnfError> {
        let mut sections = Vec::new();
        loop {
            self.skip_line_ends();
            let Some(&Token::SectionHeader { colons, .. }) = self.peek() else {
                return Ok(sections);
            };
            if colons < depth {
                return Ok(sections);
            }
            if colons > depth {
                return self.refused(format!("a section of {colons} colons, and no section of {} open", colons - 1));
            }
            let Some(Token::SectionHeader { name, .. }) = self.take() else { unreachable!("peeked") };
            let elements = self.elements()?;
            let subsections = self.sections(depth + 1)?;
            sections.push(Section { name, elements, sections: subsections });
        }
    }
}
