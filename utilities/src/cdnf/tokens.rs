//! The tokenizer ([`tokens`]): a text into its tokens, each with its
//! line -- the marks of the format, and the text between them. Comments
//! and the spaces at a line's start mean nothing, and are let go.
//! Every mark is ASCII, so what lies between two is whole UTF-8.

use crate::cdnf::CdnfError;

/// One token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Token {
    /// A line of a section's name between colons, as many on each side:
    /// how deep the section is, and its name.
    SectionHeader {
        /// The colons on each side.
        colons: usize,
        /// The name.
        name: String,
    },
    /// A dash at a line's start: an item opens.
    Dash,
    /// A colon anywhere but at a line's start: a pair.
    Colon,
    /// Text on one line between two marks, its ends' spaces let go.
    Text(String),
    /// A quoted text, its escapes read: `\"` a quote, `\\` a backslash.
    Quoted(String),
    /// One line's end between two lines with something on them.
    LineEnd,
    /// One empty line between two lines with something on them: a line
    /// of a comment alone is no line at all.
    BlankLine,
    /// Two empty lines or more between two lines with something on them
    /// -- a triple newline -- which ends a list always, and is a blank
    /// line besides.
    ListEnd,
}

/// A token, and the line it starts on, counted from 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Located {
    /// The token.
    pub(crate) token: Token,
    /// Its line.
    pub(crate) line: usize,
}

/// The tokens of `text`, in order: or the first thing wrong in it -- a
/// section's colons not matching, a section with no name, or a quote
/// never closed.
pub(crate) fn tokens(text: &str) -> Result<Vec<Located>, CdnfError> {
    let mut tokenizer = Tokenizer { text: text.as_bytes(), at: 0, line: 1, tokens: Vec::new() };
    tokenizer.read_all()?;
    Ok(tokenizer.tokens)
}

/// A text being tokenized.
struct Tokenizer<'text> {
    /// The text.
    text: &'text [u8],
    /// The byte read next.
    at: usize,
    /// Its line, counted from 1.
    line: usize,
    /// The tokens made so far.
    tokens: Vec<Located>,
}

impl Tokenizer<'_> {
    /// The byte read next, if the text has not ended.
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    /// `token`, starting on the line read.
    fn push(&mut self, token: Token) {
        self.tokens.push(Located { token, line: self.line });
    }

    /// `what` is wrong on `line`.
    fn refused<T>(line: usize, what: impl Into<String>) -> Result<T, CdnfError> {
        Err(CdnfError { line, what: what.into() })
    }

    /// The text from `began` to the byte read next.
    fn text_from(&self, began: usize) -> String {
        String::from_utf8_lossy(&self.text[began..self.at]).into_owned()
    }

    /// Past the spaces up to the next mark or the line's end.
    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r')) {
            self.at += 1;
        }
    }

    /// Past a comment, if one starts here, up to its line's end.
    fn skip_comment(&mut self) {
        if self.peek() == Some(b'#') {
            while !matches!(self.peek(), None | Some(b'\n')) {
                self.at += 1;
            }
        }
    }

    /// Whether an element may start here: a quote opens a quoted text
    /// only there -- at the text's start, or after a blank line, a
    /// section's line, a dash or a colon, line ends between let be.
    fn element_may_start(&self) -> bool {
        let last_mark = self.tokens.iter().rev().map(|located| &located.token).find(|&token| *token != Token::LineEnd);
        matches!(last_mark, None | Some(Token::BlankLine | Token::ListEnd | Token::SectionHeader { .. } | Token::Dash | Token::Colon))
    }

    /// Every line, one after another: each line with something on it
    /// after the first parted from the one before by a line end, a blank
    /// line if one line between was empty, or a list's end if more were
    /// -- one of a comment alone not counted.
    fn read_all(&mut self) -> Result<(), CdnfError> {
        let (mut anything_yet, mut empty_between) = (false, 0);
        while self.peek().is_some() {
            self.skip_spaces();
            let comment_began = self.at;
            self.skip_comment();
            let only_a_comment = self.at != comment_began;
            match self.peek() {
                None => break,
                Some(b'\n') => {
                    empty_between += usize::from(anything_yet && !only_a_comment);
                    self.at += 1;
                    self.line += 1;
                    continue;
                }
                Some(_) => {}
            }
            if anything_yet {
                self.push(match empty_between {
                    0 => Token::LineEnd,
                    1 => Token::BlankLine,
                    _ => Token::ListEnd,
                });
            }
            (anything_yet, empty_between) = (true, 0);
            self.read_line()?;
        }
        Ok(())
    }

    /// One line with something on it, from its first mark to past its
    /// line's end: a section's line, or its tokens -- a dash first if
    /// it starts with one.
    fn read_line(&mut self) -> Result<(), CdnfError> {
        match self.peek() {
            Some(b':') => self.section_header()?,
            Some(b'-') => {
                self.push(Token::Dash);
                self.at += 1;
            }
            _ => {}
        }
        loop {
            self.skip_spaces();
            self.skip_comment();
            match self.peek() {
                None => return Ok(()),
                Some(b'\n') => {
                    self.at += 1;
                    self.line += 1;
                    return Ok(());
                }
                Some(b':') => {
                    self.push(Token::Colon);
                    self.at += 1;
                }
                Some(b'"') if self.element_may_start() => self.quoted()?,
                Some(_) => self.text_run(),
            }
        }
    }

    /// A section's line: as many colons closing its name as open it, then
    /// the line's end.
    fn section_header(&mut self) -> Result<(), CdnfError> {
        let colons_ahead = |tokenizer: &Self| tokenizer.text[tokenizer.at..].iter().take_while(|&&byte| byte == b':').count();
        let colons = colons_ahead(self);
        self.at += colons;
        let name_began = self.at;
        while !matches!(self.peek(), None | Some(b':' | b'\n' | b'#')) {
            self.at += 1;
        }
        let name = self.text_from(name_began).trim().to_string();
        let closing = colons_ahead(self);
        if closing != colons {
            return Self::refused(self.line, format!("a section opened with {colons} colons and closed with {closing}"));
        }
        if name.is_empty() {
            return Self::refused(self.line, "a section with no name");
        }
        self.push(Token::SectionHeader { colons, name });
        self.at += closing;
        Ok(())
    }

    /// Text up to the next mark or the line's end, its ends' spaces let
    /// go: a quote within it is text.
    fn text_run(&mut self) {
        let run_began = self.at;
        while !matches!(self.peek(), None | Some(b'\n' | b':' | b'#')) {
            self.at += 1;
        }
        let run = self.text_from(run_began).trim_end().to_string();
        self.push(Token::Text(run));
    }

    /// A quoted text: what lies between its quotes as written, line ends
    /// and marks with it, but its escapes.
    fn quoted(&mut self) -> Result<(), CdnfError> {
        let opened_on = self.line;
        self.at += 1;
        let mut quoted = Vec::new();
        loop {
            match self.peek() {
                None => return Self::refused(opened_on, "a quote never closed"),
                Some(b'"') => break,
                Some(b'\\') if matches!(self.text.get(self.at + 1), Some(b'"' | b'\\')) => {
                    quoted.push(self.text[self.at + 1]);
                    self.at += 2;
                    continue;
                }
                Some(byte) => {
                    self.line += usize::from(byte == b'\n');
                    quoted.push(byte);
                }
            }
            self.at += 1;
        }
        self.at += 1;
        self.tokens.push(Located { token: Token::Quoted(String::from_utf8_lossy(&quoted).into_owned()), line: opened_on });
        Ok(())
    }
}
