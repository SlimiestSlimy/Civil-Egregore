//! Command lines as flags: `--flag_name`, then its parameter, or its
//! parameters in square brackets -- `--seed 0x1a2b --check [2000 100]`.
//! The words are read into tokens once; one parser handles them with the
//! flags it was given, each flag's handler reading its parameters from
//! the same parser -- and, within brackets, flags again, the parse
//! running on in the handler. The first terminal flag met is the
//! command: its parameters end the line, and any flag within them is
//! general. Simple over fast: it runs once a program
//! (`docs/reference.md`, "flags.rs").

use crate::server_parameters::CleanServerParameters;
use std::fmt;

/// A word of the command line, as the parser reads it. The shell has
/// already joined what was quoted into one word.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandLineToken {
    /// `[`: a flag's parameters begin.
    OpeningBracket,
    /// `]`: they end.
    ClosingBracket,
    /// `--flag_name`.
    Flag(String),
    /// `0x` and hexadecimal digits: a seed.
    Hex(u64),
    /// A whole number.
    Integer(i64),
    /// A number with a fraction.
    Float(f64),
    /// Anything else.
    Text(String),
}

impl fmt::Display for CommandLineToken {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CommandLineToken::OpeningBracket => write!(formatter, "`[`"),
            CommandLineToken::ClosingBracket => write!(formatter, "`]`"),
            CommandLineToken::Flag(flag_name) => write!(formatter, "`--{flag_name}`"),
            CommandLineToken::Hex(hex_number) => write!(formatter, "`{hex_number:#x}`"),
            CommandLineToken::Integer(integer) => write!(formatter, "`{integer}`"),
            CommandLineToken::Float(float) => write!(formatter, "`{float}`"),
            CommandLineToken::Text(text) => write!(formatter, "`{text}`"),
        }
    }
}

/// The tokens of the words a program was given: each word one, its
/// leading `[`s and trailing `]`s split off as the shell leaves them
/// glued on.
pub fn command_line_tokens(words: &[String]) -> Vec<CommandLineToken> {
    let mut tokens = Vec::new();
    for word in words {
        let inner_word = word.trim_start_matches('[').trim_end_matches(']');
        let (opening_count, closing_count) = (word.len() - word.trim_start_matches('[').len(), word.len() - word.trim_end_matches(']').len());
        tokens.extend(std::iter::repeat_n(CommandLineToken::OpeningBracket, opening_count));
        if !inner_word.is_empty() || opening_count + closing_count == 0 {
            tokens.push(token_of_word(inner_word));
        }
        tokens.extend(std::iter::repeat_n(CommandLineToken::ClosingBracket, closing_count));
    }
    tokens
}

/// The token a word with no brackets is: a flag, hex, a whole number, a
/// number with a fraction -- one with a digit in it, so `inf` and `nan`
/// stay text -- or text.
fn token_of_word(word: &str) -> CommandLineToken {
    let is_flag_name = |flag_name: &str| !flag_name.is_empty() && flag_name.chars().all(|name_char| name_char.is_ascii_alphanumeric() || name_char == '_' || name_char == '-');
    if let Some(flag_name) = word.strip_prefix("--").filter(|flag_name| is_flag_name(flag_name)) {
        return CommandLineToken::Flag(flag_name.to_string());
    }
    if let Some(hex_number) = word.strip_prefix("0x").and_then(|hex_digits| u64::from_str_radix(hex_digits, 16).ok()) {
        return CommandLineToken::Hex(hex_number);
    }
    if let Ok(integer) = word.parse::<i64>() {
        return CommandLineToken::Integer(integer);
    }
    match word.parse::<f64>() {
        Ok(float) if word.contains(|word_char: char| word_char.is_ascii_digit()) => CommandLineToken::Float(float),
        _ => CommandLineToken::Text(word.to_string()),
    }
}

/// What a flag is to the parse.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FlagKind {
    /// Read by whatever wants it, before the command: `--seed`.
    General,
    /// The command: its parameters, and nothing after them -- unless
    /// another terminal flag came first, within whose parameters it is
    /// general.
    Terminal,
}

/// A flag: `--flag_name`, its help, its kind, and the function that
/// parses it -- reading its parameters from the parser, and putting into
/// the parameters what they mean.
pub struct Flag {
    /// What follows `--`: letters, digits, `_` and `-`.
    pub flag_name: &'static str,
    /// What it does and what it takes, as `--help` says it.
    pub flag_help: &'static str,
    /// General or terminal.
    pub flag_kind: FlagKind,
    /// Reads its parameters, and puts into the parameters what they mean.
    pub parse_flag: fn(&mut FlagParser, &mut CleanServerParameters) -> Result<(), FlagError>,
}

/// Why a parse stopped before the line's end.
#[derive(Debug)]
pub enum FlagError {
    /// `--help` was met: the flags, said. Nothing is to run.
    HelpShown(String),
    /// The line is wrong: why, and the flags.
    WrongLine(String),
}

/// The tokens of a command line, read once, and the flags it may hold.
pub struct FlagParser {
    /// Every token.
    tokens: Vec<CommandLineToken>,
    /// The first token not yet read.
    next_token: usize,
    /// Every flag the line may hold.
    flags: Vec<Flag>,
    /// Whether the command -- the first terminal flag -- has been met.
    command_met: bool,
}

impl FlagParser {
    /// A parser of the words a program was given, with `flags`.
    pub fn new(words: &[String], flags: Vec<Flag>) -> Self {
        Self { tokens: command_line_tokens(words), next_token: 0, flags, command_met: false }
    }

    /// Handles the whole line, flag after flag, each by its handler.
    pub fn parse(&mut self, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
        self.parse_flags(clean_parameters, false)
    }

    /// A handler's flags within its brackets: handled until the `]`,
    /// which is left for [`Self::bracketed`] to read.
    pub fn parse_flags_within(&mut self, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
        self.parse_flags(clean_parameters, true)
    }

    /// Flag after flag until the line's end -- or, `within_brackets`, a
    /// `]`. The command's parameters end the line.
    fn parse_flags(&mut self, clean_parameters: &mut CleanServerParameters, within_brackets: bool) -> Result<(), FlagError> {
        while let Some(token) = self.tokens.get(self.next_token).cloned() {
            let flag_name = match token {
                CommandLineToken::ClosingBracket if within_brackets => return Ok(()),
                CommandLineToken::Flag(flag_name) => flag_name,
                other_token => return Err(self.wrong_line(&format!("{other_token} is no flag here"))),
            };
            if flag_name == "help" {
                return Err(FlagError::HelpShown(self.help_text()));
            }
            let Some((flag_kind, parse_flag)) = self.flags.iter().find(|flag| flag.flag_name == flag_name).map(|flag| (flag.flag_kind, flag.parse_flag)) else {
                return Err(self.wrong_line(&format!("`--{flag_name}` is no flag")));
            };
            self.next_token += 1;
            let is_the_command = flag_kind == FlagKind::Terminal && !self.command_met;
            self.command_met |= is_the_command;
            parse_flag(self, clean_parameters)?;
            if is_the_command && !within_brackets && let Some(left_over) = self.tokens.get(self.next_token) {
                return Err(self.wrong_line(&format!("{left_over}: nothing follows `--{flag_name}`'s parameters")));
            }
        }
        Ok(())
    }

    /// A handler's parameters in square brackets: `[`, what
    /// `read_within` reads, `]`.
    pub fn bracketed<Read>(&mut self, read_within: impl FnOnce(&mut Self) -> Result<Read, FlagError>) -> Result<Read, FlagError> {
        self.expect_token(&CommandLineToken::OpeningBracket)?;
        let read = read_within(self)?;
        self.expect_token(&CommandLineToken::ClosingBracket)?;
        Ok(read)
    }

    /// A handler's parameter in hexadecimal.
    pub fn hex_parameter(&mut self) -> Result<u64, FlagError> {
        match self.next_parameter("a number in hexadecimal")? {
            CommandLineToken::Hex(hex_number) => Ok(hex_number),
            other_token => Err(self.wrong_line(&format!("{other_token} is not a number in hexadecimal"))),
        }
    }

    /// A handler's parameter as a whole number.
    pub fn integer_parameter(&mut self) -> Result<i64, FlagError> {
        match self.next_parameter("a whole number")? {
            CommandLineToken::Integer(integer) => Ok(integer),
            other_token => Err(self.wrong_line(&format!("{other_token} is not a whole number"))),
        }
    }

    /// A handler's parameter as a number, a whole one too.
    pub fn float_parameter(&mut self) -> Result<f64, FlagError> {
        match self.next_parameter("a number")? {
            CommandLineToken::Float(float) => Ok(float),
            CommandLineToken::Integer(integer) => Ok(integer as f64),
            other_token => Err(self.wrong_line(&format!("{other_token} is not a number"))),
        }
    }

    /// A handler's parameter as text: any word that is no flag or bracket.
    pub fn text_parameter(&mut self) -> Result<String, FlagError> {
        match self.next_parameter("a word")? {
            CommandLineToken::Text(text) => Ok(text),
            CommandLineToken::Hex(hex_number) => Ok(format!("{hex_number:#x}")),
            CommandLineToken::Integer(integer) => Ok(integer.to_string()),
            CommandLineToken::Float(float) => Ok(float.to_string()),
            other_token => Err(self.wrong_line(&format!("{other_token} is not a word"))),
        }
    }

    /// The next token, read, if it is a parameter: no flag or bracket.
    fn next_parameter(&mut self, wanted: &str) -> Result<CommandLineToken, FlagError> {
        match self.tokens.get(self.next_token).cloned() {
            Some(CommandLineToken::Flag(_) | CommandLineToken::OpeningBracket | CommandLineToken::ClosingBracket) | None => {
                let found = self.tokens.get(self.next_token).map_or("the line's end".to_string(), ToString::to_string);
                Err(self.wrong_line(&format!("{wanted} wanted, not {found}")))
            }
            Some(parameter_token) => {
                self.next_token += 1;
                Ok(parameter_token)
            }
        }
    }

    /// The next token, which must be `wanted_token`.
    fn expect_token(&mut self, wanted_token: &CommandLineToken) -> Result<(), FlagError> {
        if self.tokens.get(self.next_token) == Some(wanted_token) {
            self.next_token += 1;
            return Ok(());
        }
        Err(self.wrong_line(&format!("{wanted_token} wanted")))
    }

    /// The line is wrong, `why_wrong`, and the flags said after.
    fn wrong_line(&self, why_wrong: &str) -> FlagError {
        FlagError::WrongLine(format!("{why_wrong}\n{}", self.help_text()))
    }

    /// Every flag, a line each: name, whether it is a command, help.
    fn help_text(&self) -> String {
        self.flags.iter().map(|flag| format!("  --{:<20} {}{}\n", flag.flag_name, if flag.flag_kind == FlagKind::Terminal { "(command) " } else { "" }, flag.flag_help)).collect()
    }
}
