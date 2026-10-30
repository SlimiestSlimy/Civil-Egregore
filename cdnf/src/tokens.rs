//! The tokenizer ([`tokens`]): a text into its tokens, each with its
//! line -- the marks of the format, and the text between them. Comments
//! and the spaces at a line's start mean nothing, and are let go. A colon
//! or a dash is a mark only with a space or a line's end after it.
//! Every mark is ASCII, so what lies between two is whole UTF-8.

use crate::CdnfError;

/// What opens a comment, anywhere outside a quoted text.
pub(crate) const COMMENT_MARK: &str = "<:->";

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
    /// Two dashes at a line's start: a list's last item opens.
    LastDash,
    /// A colon, not at a line's start, that makes a pair.
    Colon,
    /// Text on one line between two marks, its ends' spaces let go.
    Text(String),
    /// A quoted text, its escapes read: `\"` a quote.
    Quoted(String),
    /// One line's end between two lines with something on them.
    LineEnd,
    /// Empty lines, one or more, between two lines with something on
    /// them: a line of a comment alone is no line at all.
    BlankLine,
}

/// A token, and the line it starts on, counted from 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Located {
    /// The token.
    pub(crate) token: Token,
    /// Its line.
    pub(crate) line: usize,
}

/// The tokens of `text`, in order, a byte order mark at its start let
/// go: or the first thing wrong in it -- a section's line not right, or
/// a quote never closed.
pub(crate) fn tokens(text: &str) -> Result<Vec<Located>, CdnfError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut tokenizer = Tokenizer { text, at: 0, line: 1, tokens: Vec::new() };
    tokenizer.read_all()?;
    Ok(tokenizer.tokens)
}

/// A text being tokenized.
struct Tokenizer<'text> {
    /// The text.
    text: &'text str,
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
        self.text.as_bytes().get(self.at).copied()
    }

    /// `token`, starting on the line read.
    fn push(&mut self, token: Token) {
        self.tokens.push(Located { token, line: self.line });
    }

    /// `what` is wrong on `line`.
    fn refused<T>(line: usize, what: impl Into<String>) -> Result<T, CdnfError> {
        Err(CdnfError { line, what: what.into() })
    }

    /// How many of `byte` come here, one after another.
    fn run_of(&self, byte: u8) -> usize {
        self.text.as_bytes()[self.at..].iter().take_while(|&&next| next == byte).count()
    }

    /// Past the spaces up to the next mark or the line's end.
    fn skip_spaces(&mut self) {
        let rest = &self.text[self.at..];
        self.at += rest.len() - rest.trim_start_matches(|character: char| character.is_whitespace() && character != '\n').len();
    }

    /// Whether a comment starts here.
    fn at_comment(&self) -> bool {
        self.text.as_bytes()[self.at..].starts_with(COMMENT_MARK.as_bytes())
    }

    /// Past a comment, if one starts here, up to its line's end.
    fn skip_comment(&mut self) {
        if self.at_comment() {
            while !matches!(self.peek(), None | Some(b'\n')) {
                self.at += 1;
            }
        }
    }

    /// Whether a colon here makes a pair: a space, the line's end or a
    /// comment after it.
    fn at_pair_colon(&self) -> bool {
        self.peek() == Some(b':') && {
            let after = &self.text[self.at + 1..];
            after.is_empty() || after.starts_with(char::is_whitespace) || after.starts_with(COMMENT_MARK)
        }
    }

    /// How many dashes open an item here: 1 or 2, a space or the line's
    /// end after them; else 0, and they are text.
    fn item_dashes(&self) -> usize {
        let dashes = self.run_of(b'-');
        let after = &self.text[self.at + dashes..];
        if (1..=2).contains(&dashes) && (after.is_empty() || after.starts_with(char::is_whitespace)) { dashes } else { 0 }
    }

    /// Whether an element may start here: a quote opens a quoted text
    /// only there -- at the text's start, or after a blank line, a
    /// section's line, a dash or a colon, line ends between let be.
    fn element_may_start(&self) -> bool {
        let last_mark = self.tokens.iter().rev().map(|located| &located.token).find(|&token| *token != Token::LineEnd);
        matches!(last_mark, None | Some(Token::BlankLine | Token::SectionHeader { .. } | Token::Dash | Token::LastDash | Token::Colon))
    }

    /// Every line, one after another: each line with something on it
    /// after the first parted from the one before by a line end, or a
    /// blank line if any line between was empty -- one of a comment alone
    /// not counted.
    fn read_all(&mut self) -> Result<(), CdnfError> {
        let (mut anything_yet, mut empty_between) = (false, 0);
        while self.peek().is_some() {
            let line_began = self.at;
            self.skip_spaces();
            let indented = self.at != line_began;
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
                self.push(if empty_between == 0 { Token::LineEnd } else { Token::BlankLine });
            }
            (anything_yet, empty_between) = (true, 0);
            self.read_line(indented)?;
        }
        Ok(())
    }

    /// One line with something on it, from its first mark to past its
    /// line's end: a section's line -- at the line's very start, none
    /// `indented` -- or its tokens, a dash first, or two, if it starts
    /// with them.
    fn read_line(&mut self, indented: bool) -> Result<(), CdnfError> {
        match (self.peek(), self.item_dashes()) {
            (Some(b':'), _) if indented => return Self::refused(self.line, "a section's line starts at the line's very start: no space before its colons"),
            (Some(b':'), _) => self.section_header()?,
            (_, 1) => {
                self.push(Token::Dash);
                self.at += 1;
            }
            (_, 2) => {
                self.push(Token::LastDash);
                self.at += 2;
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
                Some(b':') if self.at_pair_colon() => {
                    self.push(Token::Colon);
                    self.at += 1;
                }
                Some(b'"') if self.element_may_start() => {
                    let opened_on = self.line;
                    let quoted = self.quoted()?;
                    self.tokens.push(Located { token: Token::Quoted(quoted), line: opened_on });
                }
                Some(_) => {
                    let run = self.text_run();
                    self.push(Token::Text(run));
                }
            }
        }
    }

    /// A section's line: its name a text like any other, quoted or
    /// plain, as many colons closing it as open it, then the line's end.
    fn section_header(&mut self) -> Result<(), CdnfError> {
        let colons = self.run_of(b':');
        self.at += colons;
        self.skip_spaces();
        let name = if self.peek() == Some(b'"') {
            self.quoted()?
        } else {
            let run = self.text_run();
            let name = run.trim_end_matches(':');
            self.at -= run.len() - name.len();
            name.trim_end().to_string()
        };
        self.skip_spaces();
        let closing = self.run_of(b':');
        if closing != colons {
            return Self::refused(self.line, format!("a section opened with {colons} colons and closed with {closing}"));
        }
        if name.is_empty() || name.contains('\n') {
            return Self::refused(self.line, "a section's name is one line, and not empty");
        }
        self.at += closing;
        self.skip_spaces();
        self.skip_comment();
        if !matches!(self.peek(), None | Some(b'\n')) {
            return Self::refused(self.line, "nothing comes after a section's closing colons");
        }
        self.push(Token::SectionHeader { colons, name });
        Ok(())
    }

    /// Text up to the next mark or the line's end, its ends' spaces let
    /// go: a quote within it is text.
    fn text_run(&mut self) -> String {
        let run_began = self.at;
        while !matches!(self.peek(), None | Some(b'\n')) && !self.at_pair_colon() && !self.at_comment() {
            self.at += 1;
        }
        self.text[run_began..self.at].trim_end().to_string()
    }

    /// A quoted text: what lies between its quotes as written, line ends
    /// and marks with it, but its escapes.
    fn quoted(&mut self) -> Result<String, CdnfError> {
        let opened_on = self.line;
        self.at += 1;
        let mut quoted = Vec::new();
        loop {
            match self.peek() {
                None => return Self::refused(opened_on, "a quote never closed"),
                Some(b'"') => break,
                Some(b'\\') if self.text.as_bytes().get(self.at + 1) == Some(&b'"') => {
                    quoted.push(b'"');
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
        Ok(String::from_utf8_lossy(&quoted).into_owned())
    }
}
