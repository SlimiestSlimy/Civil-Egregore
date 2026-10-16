//! Commands: what a command line asks for, found by its first word and
//! handed the rest -- once, for every crate (`docs/utilities.md`,
//! "Commands").

use crate::diagnostics::table::Table;
use std::str::FromStr;

/// One thing a command takes, by its place after the command's name.
#[derive(Clone, Copy, Debug)]
pub struct CommandParameter {
    /// What it is, as the usage says it.
    pub parameter_name: &'static str,
    /// What it is if not given; empty, and it has no default.
    pub default_value: &'static str,
}

impl CommandParameter {
    /// The parameter `parameter_name`, `default_value` if not given.
    pub const fn new(parameter_name: &'static str, default_value: &'static str) -> Self {
        Self { parameter_name, default_value }
    }
}

/// One command: the word that names it, and what it runs.
#[derive(Clone, Copy)]
pub struct NamedCommand {
    /// The word that names it.
    pub command_name: &'static str,
    /// What it does, as the usage says it.
    pub what_it_does: &'static str,
    /// What it takes after its name, in order.
    pub command_parameters: &'static [CommandParameter],
    /// What it runs, given the rest of the line.
    pub run_function: fn(&GivenCommand) -> Result<(), String>,
}

/// A command as it was called: the words before it, and those after.
pub struct GivenCommand<'a> {
    /// The words the command was reached by, the program's first.
    reached_by: &'a str,
    /// The command.
    called_command: &'a NamedCommand,
    /// The words after the command's name.
    words_after_name: &'a [&'a str],
}

impl GivenCommand<'_> {
    /// The command's name.
    pub fn command_name(&self) -> &'static str {
        self.called_command.command_name
    }

    /// The words after the command's name, as they were given.
    pub fn argument_words(&self) -> &[&str] {
        self.words_after_name
    }

    /// The words between the program's name and the command's: what a
    /// program run again must be given before the command.
    pub fn route_words(&self) -> impl Iterator<Item = &str> {
        self.reached_by.split_whitespace().skip(1)
    }

    /// The parameter `parameter_name`, if given.
    pub fn given_argument(&self, parameter_name: &str) -> Option<&str> {
        let place = self.called_command.command_parameters.iter().position(|declared_parameter| declared_parameter.parameter_name == parameter_name).unwrap_or_else(|| panic!("`{}` takes no `{parameter_name}`", self.called_command.command_name));
        self.words_after_name.get(place).copied()
    }

    /// The parameter `parameter_name`: as given, or its default.
    pub fn argument_text(&self, parameter_name: &str) -> Result<&str, String> {
        let default_text = self.called_command.command_parameters.iter().find(|declared_parameter| declared_parameter.parameter_name == parameter_name).map_or("", |found_parameter| found_parameter.default_value);
        match self.given_argument(parameter_name).unwrap_or(default_text) {
            "" => Err(format!("{} needs its {parameter_name}\n{}", self.called_command.command_name, self.command_usage())),
            given_text => Ok(given_text),
        }
    }

    /// The parameter `parameter_name` as a number: as given, or its default.
    pub fn argument_number<T: FromStr>(&self, parameter_name: &str) -> Result<T, String> {
        let number_text = self.argument_text(parameter_name)?;
        number_text.parse().map_err(|_| format!("{parameter_name}: `{number_text}` is not a number"))
    }

    /// How the command is called: the words it is reached by, its name
    /// and its parameters.
    pub fn command_usage(&self) -> String {
        usage_line(self.reached_by, self.called_command)
    }

    /// The command as it ran: the words it was reached by, its name,
    /// and every parameter as given or by default -- what a report says
    /// it was measured with.
    pub fn resolved_line(&self) -> String {
        let parameter_values = self.called_command.command_parameters.iter().enumerate().map(|(place, declared_parameter)| self.words_after_name.get(place).copied().unwrap_or(declared_parameter.default_value));
        let words: Vec<&str> = [self.reached_by, self.called_command.command_name].into_iter().chain(parameter_values).filter(|word| !word.is_empty()).collect();
        words.join(" ")
    }
}

/// `listed_command`'s parameters as the usage shows them: `[name]`, and its
/// default after it if it has one.
fn parameters_usage(listed_command: &NamedCommand) -> String {
    let shown_parameter = |declared_parameter: &CommandParameter| if declared_parameter.default_value.is_empty() { format!("[{}]", declared_parameter.parameter_name) } else { format!("[{} = {}]", declared_parameter.parameter_name, declared_parameter.default_value) };
    listed_command.command_parameters.iter().map(shown_parameter).collect::<Vec<_>>().join(" ")
}

/// How `listed_command` is called, reached by `reached_by`.
fn usage_line(reached_by: &str, listed_command: &NamedCommand) -> String {
    format!("{reached_by} {} {}", listed_command.command_name, parameters_usage(listed_command)).trim_end().to_string()
}

/// Every command of `crate_commands`, reached by `reached_by`, as a table: its
/// name, what it takes and what it does.
pub fn usage_table(reached_by: &str, crate_commands: &[NamedCommand]) -> String {
    let mut table = Table::new(&["command", "takes", "does"]).left_aligned(&["command", "takes", "does"]);
    for listed_command in crate_commands {
        table.add_row(&[format!("{reached_by} {}", listed_command.command_name), parameters_usage(listed_command), listed_command.what_it_does.to_string()]);
    }
    table.rendered_text()
}

/// Runs the command of `crate_commands` that the first of `argument_words` names,
/// given the rest: reached by `reached_by`, the words before it on the
/// command line. None named, or none so named: the usage, as why not.
pub fn dispatch_command(reached_by: &str, crate_commands: &[NamedCommand], argument_words: &[&str]) -> Result<(), String> {
    let command_and_rest = argument_words.split_first().and_then(|(first_word, rest_words)| crate_commands.iter().find(|candidate_command| candidate_command.command_name == *first_word).map(|matched_command| (matched_command, rest_words)));
    match command_and_rest {
        Some((chosen_command, rest_arguments)) => (chosen_command.run_function)(&GivenCommand { reached_by, called_command: chosen_command, words_after_name: rest_arguments }),
        None => Err(usage_table(reached_by, crate_commands)),
    }
}

/// A crate with commands, as the program knows it: its name, and where
/// to hand the rest of the line.
#[derive(Clone, Copy)]
pub struct CommandCrate {
    /// The word that names it.
    pub crate_name: &'static str,
    /// What its commands are for, as the usage says it.
    pub what_it_does: &'static str,
    /// Its commands.
    pub crate_commands: &'static [NamedCommand],
}

/// The words that ask for help.
pub const HELP_WORDS: [&str; 3] = ["help", "--help", "-h"];

/// Every command of every crate of `command_crates`, a table a crate: what
/// `reached_by help` prints.
pub fn help_text(reached_by: &str, command_crates: &[CommandCrate]) -> String {
    let mut printed_help = format!("  {reached_by} <crate> <command> [what it takes]: what is not given is what follows its `=`\n");
    for listed_crate in command_crates {
        printed_help += &format!("\n  {}: {}\n{}", listed_crate.crate_name, listed_crate.what_it_does, usage_table(&format!("{reached_by} {}", listed_crate.crate_name), listed_crate.crate_commands));
    }
    printed_help
}

/// The whole of a program called `program_name`: hands `argument_words` to the
/// crate of `command_crates` its first word names; with [`HELP_WORDS`], no word or
/// an unknown one, every command there is ([`help_text`]).
pub fn run_program(program_name: &str, command_crates: &[CommandCrate], argument_words: &[&str]) -> Result<(), String> {
    let crate_and_rest = argument_words.split_first().and_then(|(first_word, rest_words)| command_crates.iter().find(|candidate_crate| candidate_crate.crate_name == *first_word).map(|matched_crate| (matched_crate, rest_words)));
    match crate_and_rest {
        Some((chosen_crate, rest_arguments)) => dispatch_command(&format!("{program_name} {}", chosen_crate.crate_name), chosen_crate.crate_commands, rest_arguments),
        None if argument_words.first().is_some_and(|word| HELP_WORDS.contains(word)) => {
            print!("{}", help_text(program_name, command_crates));
            Ok(())
        }
        None => Err(help_text(program_name, command_crates)),
    }
}
