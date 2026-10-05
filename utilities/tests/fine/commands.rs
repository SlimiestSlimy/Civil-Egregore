//! Commands: the first word finds one, which is given the rest; a
//! parameter not given is its default.
//!
//! `cargo test`

use utilities::commands::{dispatch, Command, Parameter};

/// A command that says, as why it failed, the line it ran on and the
/// sum of its two numbers: what it was given, seen from outside.
const SUM: Command = Command {
    name: "sum",
    does: "adds two numbers",
    parameters: &[Parameter::new("first", "2"), Parameter::new("second", "3"), Parameter::new("note", "")],
    run: |given| Err(format!("{} = {}{}", given.resolved(), given.number::<u32>("first")? + given.number::<u32>("second")?, given.given("note").unwrap_or(""))),
};

/// The command named is run on the words after its name, a parameter
/// not given being its default, and its line says so; a word that names
/// none, or no word, gives the usage; a number that is none is refused.
#[test]
fn the_first_word_names_the_command_and_the_rest_are_its_parameters() {
    let run = |arguments: &[&str]| dispatch("program crate", &[SUM], arguments).expect_err("the command says what it was given");
    assert_eq!(run(&["sum"]), "program crate sum 2 3 = 5");
    assert_eq!(run(&["sum", "10"]), "program crate sum 10 3 = 13");
    assert_eq!(run(&["sum", "10", "20", "!"]), "program crate sum 10 20 ! = 30!");
    assert_eq!(run(&["sum", "ten"]), "first: `ten` is not a number");
    for unnamed in [&[][..], &["product", "1"]] {
        let usage = run(unnamed);
        assert!(usage.contains("program crate sum") && usage.contains("[first = 2] [second = 3] [note]") && usage.contains("adds two numbers"), "{usage}");
    }
}
