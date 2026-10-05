//! Commands: what a command line asks for, found by its first word and
//! handed the rest -- once, for every crate.
//!
//! A crate is a library: its diagnostics tools are functions, not
//! programs. It lists them as [`Command`]s, each with the
//! [`Parameter`]s it takes -- a name and what it is if not given --
//! and [`dispatch`] runs the one the first word names. The one program,
//! `tilesim`, hands a crate everything after the crate's name and knows
//! nothing of its tools.
//!
//! A parameter is declared once: its default is what the usage table
//! shows, what the command reads ([`Given::number`]) and what its
//! report says it ran on ([`Given::resolved`]).

use crate::diagnostics::table::Table;
use std::str::FromStr;

/// One thing a command takes, by its place after the command's name.
#[derive(Clone, Copy, Debug)]
pub struct Parameter {
    /// What it is, as the usage says it.
    pub name: &'static str,
    /// What it is if not given; empty, and it has no default.
    pub default: &'static str,
}

impl Parameter {
    /// The parameter `name`, `default` if not given.
    pub const fn new(name: &'static str, default: &'static str) -> Self {
        Self { name, default }
    }
}

/// One command: the word that names it, and what it runs.
#[derive(Clone, Copy)]
pub struct Command {
    /// The word that names it.
    pub name: &'static str,
    /// What it does, as the usage says it.
    pub does: &'static str,
    /// What it takes after its name, in order.
    pub parameters: &'static [Parameter],
    /// What it runs, given the rest of the line.
    pub run: fn(&Given) -> Result<(), String>,
}

/// A command as it was called: the words before it, and those after.
pub struct Given<'a> {
    /// The words the command was reached by, the program's first.
    called: &'a str,
    /// The command.
    command: &'a Command,
    /// The words after the command's name.
    arguments: &'a [&'a str],
}

impl Given<'_> {
    /// The command's name.
    pub fn name(&self) -> &'static str {
        self.command.name
    }

    /// The words after the command's name, as they were given.
    pub fn arguments(&self) -> &[&str] {
        self.arguments
    }

    /// The words between the program's name and the command's: what a
    /// program run again must be given before the command.
    pub fn route(&self) -> impl Iterator<Item = &str> {
        self.called.split_whitespace().skip(1)
    }

    /// The parameter `name`, if given.
    pub fn given(&self, name: &str) -> Option<&str> {
        let place = self.command.parameters.iter().position(|parameter| parameter.name == name).unwrap_or_else(|| panic!("`{}` takes no `{name}`", self.command.name));
        self.arguments.get(place).copied()
    }

    /// The parameter `name`: as given, or its default.
    pub fn text(&self, name: &str) -> Result<&str, String> {
        let default = self.command.parameters.iter().find(|parameter| parameter.name == name).map_or("", |parameter| parameter.default);
        match self.given(name).unwrap_or(default) {
            "" => Err(format!("{} needs its {name}\n{}", self.command.name, self.usage())),
            text => Ok(text),
        }
    }

    /// The parameter `name` as a number: as given, or its default.
    pub fn number<T: FromStr>(&self, name: &str) -> Result<T, String> {
        let text = self.text(name)?;
        text.parse().map_err(|_| format!("{name}: `{text}` is not a number"))
    }

    /// How the command is called: the words it is reached by, its name
    /// and its parameters.
    pub fn usage(&self) -> String {
        line(self.called, self.command)
    }

    /// The command as it ran: the words it was reached by, its name,
    /// and every parameter as given or by default -- what a report says
    /// it was measured with.
    pub fn resolved(&self) -> String {
        let parameters = self.command.parameters.iter().enumerate().map(|(place, parameter)| self.arguments.get(place).copied().unwrap_or(parameter.default));
        let words: Vec<&str> = [self.called, self.command.name].into_iter().chain(parameters).filter(|word| !word.is_empty()).collect();
        words.join(" ")
    }
}

/// `command`'s parameters as the usage shows them: `[name]`, and its
/// default after it if it has one.
fn parameters(command: &Command) -> String {
    let shown = |parameter: &Parameter| if parameter.default.is_empty() { format!("[{}]", parameter.name) } else { format!("[{} = {}]", parameter.name, parameter.default) };
    command.parameters.iter().map(shown).collect::<Vec<_>>().join(" ")
}

/// How `command` is called, reached by `called`.
fn line(called: &str, command: &Command) -> String {
    format!("{called} {} {}", command.name, parameters(command)).trim_end().to_string()
}

/// Every command of `commands`, reached by `called`, as a table: its
/// name, what it takes and what it does.
pub fn usage(called: &str, commands: &[Command]) -> String {
    let mut table = Table::new(&["command", "takes", "does"]).left_aligned(&["command", "takes", "does"]);
    for command in commands {
        table.row(&[format!("{called} {}", command.name), parameters(command), command.does.to_string()]);
    }
    table.rendered()
}

/// Runs the command of `commands` that the first of `arguments` names,
/// given the rest: reached by `called`, the words before it on the
/// command line. None named, or none so named: the usage, as why not.
pub fn dispatch(called: &str, commands: &[Command], arguments: &[&str]) -> Result<(), String> {
    let named = arguments.split_first().and_then(|(name, rest)| commands.iter().find(|command| command.name == *name).map(|command| (command, rest)));
    match named {
        Some((command, arguments)) => (command.run)(&Given { called, command, arguments }),
        None => Err(usage(called, commands)),
    }
}
