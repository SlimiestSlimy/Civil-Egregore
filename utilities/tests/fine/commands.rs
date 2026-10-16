//! Commands: the first word finds one, which is given the rest; a
//! parameter not given is its default.
//!
//! `cargo test`

use utilities::commands::{dispatch_command, NamedCommand, CommandParameter};

/// A command that says, as why it failed, the line it ran on and the
/// sum of its two numbers: what it was given, seen from outside.
const SUM_COMMAND: NamedCommand = NamedCommand {
    command_name: "sum",
    what_it_does: "adds two numbers",
    command_parameters: &[CommandParameter::new("first", "2"), CommandParameter::new("second", "3"), CommandParameter::new("note", "")],
    run_function: |given_command| Err(format!("{} = {}{}", given_command.resolved_line(), given_command.argument_number::<u32>("first")? + given_command.argument_number::<u32>("second")?, given_command.given_argument("note").unwrap_or(""))),
};

/// The command named is run on the words after its name, a parameter
/// not given being its default, and its line says so; a word that names
/// none, or no word, gives the usage; a number that is none is refused.
#[test]
fn the_first_word_names_the_command_and_the_rest_are_its_parameters() {
    let run_sum = |argument_words: &[&str]| dispatch_command("program crate", &[SUM_COMMAND], argument_words).expect_err("the command says what it was given");
    assert_eq!(run_sum(&["sum"]), "program crate sum 2 3 = 5");
    assert_eq!(run_sum(&["sum", "10"]), "program crate sum 10 3 = 13");
    assert_eq!(run_sum(&["sum", "10", "20", "!"]), "program crate sum 10 20 ! = 30!");
    assert_eq!(run_sum(&["sum", "ten"]), "first: `ten` is not a number");
    for unnamed_arguments in [&[][..], &["product", "1"]] {
        let usage_text = run_sum(unnamed_arguments);
        assert!(usage_text.contains("program crate sum") && usage_text.contains("[first = 2] [second = 3] [note]") && usage_text.contains("adds two numbers"), "{usage_text}");
    }
}
