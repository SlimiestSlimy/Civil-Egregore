//! The docs a change may have left stale: every doc section the
//! changed code's comments point at that the change did not touch, as
//! a table to read -- a list, not a test.
//!
//! How it reads a change: `docs/utilities.md`, "Stale docs".

use crate::commands::{NamedCommand, GivenCommand, CommandParameter};
use crate::diagnostics::table::Table;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

/// The parameter naming the commit a change is counted from.
const FROM_COMMIT: &str = "from commit";

/// What the table says of a pointer that names no section.
const WHOLE_DOC: &str = "(the whole doc)";

/// The `docs` commands of the program.
pub const COMMANDS: [NamedCommand; 1] = [NamedCommand {
    command_name: "stale",
    what_it_does: "the doc sections the code changed since a commit points at, that the change left untouched: a list to read",
    command_parameters: &[CommandParameter::new(FROM_COMMIT, "HEAD")],
    run_function: list_stale_docs,
}];

/// A stretch of a file's lines, both ends counted, the first line 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Lines {
    /// The first line.
    first_line: usize,
    /// The last line.
    last_line: usize,
}

impl Lines {
    /// Whether the two stretches share a line.
    fn shares_line_with(self, other_stretch: Lines) -> bool {
        self.first_line <= other_stretch.last_line && other_stretch.first_line <= self.last_line
    }
}

/// What `git`, run in `folder` with `git_arguments`, prints; or why it
/// did not.
fn git_output(folder: &Path, git_arguments: &[&str]) -> Result<String, String> {
    let git_process_output = process::Command::new("git").arg("-C").arg(folder).args(git_arguments).output().map_err(|spawn_error| format!("git could not be run: {spawn_error}"))?;
    if !git_process_output.status.success() {
        return Err(format!("git {}: {}", git_arguments.join(" "), String::from_utf8_lossy(&git_process_output.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&git_process_output.stdout).into_owned())
}

/// The lines `diff_text` -- what `git diff -U0` prints -- changes, file by
/// file, numbered as each file is now. Lines only removed count as the
/// two they stood between.
fn changed_lines(diff_text: &str) -> Vec<(String, Lines)> {
    let (mut changed_stretches, mut current_file, mut previous_line) = (Vec::new(), None, "");
    for line in diff_text.lines() {
        if let (true, Some(path)) = (previous_line.starts_with("--- "), line.strip_prefix("+++ ")) {
            current_file = path.strip_prefix("b/").map(str::to_string);
        } else if let (Some(hunk_file), Some(hunk_header)) = (&current_file, line.strip_prefix("@@ ")) {
            let new_range = hunk_header.split(' ').find_map(|part| part.strip_prefix('+')).unwrap_or_default();
            let (first_text, count) = new_range.split_once(',').unwrap_or((new_range, "1"));
            if let (Ok(first_line), Ok(count)) = (first_text.parse::<usize>(), count.parse::<usize>()) {
                let last_line = if count == 0 { first_line + 1 } else { first_line + count - 1 };
                changed_stretches.push((hunk_file.clone(), Lines { first_line, last_line }));
            }
        }
        previous_line = line;
    }
    changed_stretches
}

/// What `line` writes between backticks.
fn backtick_spans(line: &str) -> Vec<&str> {
    line.split('`').skip(1).step_by(2).collect()
}

/// The folder of the crate the file at `path`, from the root, is in:
/// the nearest above it with a manifest.
fn crate_of(root_folder: &Path, path: &str) -> PathBuf {
    let folders = Path::new(path).ancestors().skip(1);
    folders.map(|folder| root_folder.join(folder)).find(|folder| folder.join("Cargo.toml").exists()).unwrap_or_else(|| root_folder.to_path_buf())
}

/// The titles `text_after_path` -- what follows a doc's path -- quotes, a
/// title's own backticks left off: every one until the sentence ends,
/// or a bracket closes, or other code is written.
fn quoted_titles(text_after_path: &str) -> Vec<&str> {
    let (mut found_titles, mut parts) = (Vec::new(), text_after_path.split('"'));
    while let (Some(text_between), Some(quoted_title)) = (parts.next(), parts.next()) {
        if text_between.contains(['`', '|', ')']) || text_between.contains(". ") {
            break;
        }
        found_titles.push(quoted_title.trim_matches('`'));
    }
    found_titles
}

/// The docs and sections the comments of the source at `path` point
/// at, `source_text` being what it holds: each a doc's path from the root and
/// a section's title, none for the whole doc. A doc that is not there
/// is left out: the fine tier's to say.
fn doc_pointers(root_folder: &Path, path: &str, source_text: &str) -> Vec<(String, Option<String>)> {
    let comment_lines: Vec<(usize, &str)> = source_text.lines().enumerate().filter_map(|(line_index, line)| line.split_once("//").map(|(_, comment_text)| (line_index, comment_text.trim_start_matches(['/', '!'])))).collect();
    let own_crate = crate_of(root_folder, path);
    let mut pointed_sections = Vec::new();
    for (place, &(comment_line_index, line_comment)) in comment_lines.iter().enumerate() {
        for doc_span in backtick_spans(line_comment).into_iter().filter(|candidate_span| candidate_span.ends_with(".md") && !candidate_span.contains(char::is_whitespace)) {
            let Some(found_doc) = [own_crate.as_path(), root_folder].iter().map(|start_folder| start_folder.join(doc_span.trim_start_matches("../"))).find(|candidate_path| candidate_path.is_file()) else {
                continue;
            };
            let doc_path = found_doc.strip_prefix(root_folder).unwrap_or(&found_doc).to_string_lossy().into_owned();
            let text_after_span = line_comment.split_once(&format!("`{doc_span}`")).map_or("", |(_, split_after)| split_after);
            let next_comment = comment_lines.get(place + 1).filter(|(following_index, _)| *following_index == comment_line_index + 1).map_or("", |&(_, following_comment)| following_comment);
            let joined_after = format!("{text_after_span} {next_comment}").split_whitespace().collect::<Vec<_>>().join(" ");
            let section_titles = quoted_titles(&joined_after);
            match section_titles.is_empty() {
                true => pointed_sections.push((doc_path, None)),
                false => pointed_sections.extend(section_titles.into_iter().map(|section_title| (doc_path.clone(), Some(section_title.to_string())))),
            }
        }
    }
    pointed_sections
}

/// How deep the heading `line` is: its `#`s, or none if it is not one.
fn heading_depth(line: &str) -> Option<usize> {
    Some(line.chars().take_while(|&heading_char| heading_char == '#').count()).filter(|&hash_count| hash_count > 0)
}

/// The lines of the section `wanted_title` begins in the doc `doc_text`: from its
/// heading to the next as deep or less, or from a line led by bold to
/// the next heading or line led by bold. The whole doc if it has no
/// such section, or none is asked for.
fn section_lines(doc_text: &str, wanted_title: Option<&str>) -> Lines {
    let lines: Vec<&str> = doc_text.lines().collect();
    let whole_doc = Lines { first_line: 1, last_line: lines.len().max(1) };
    let Some(section_title) = wanted_title else {
        return whole_doc;
    };
    let begins_section = |line: &&str| (line.starts_with('#') || line.starts_with("**")) && line.trim_start_matches(['#', '*', '`', ' ']).starts_with(section_title);
    let Some(start_index) = lines.iter().position(begins_section) else {
        return whole_doc;
    };
    let ends_section = |line: &&str| match heading_depth(lines[start_index]) {
        Some(start_depth) => heading_depth(line).is_some_and(|line_depth| line_depth <= start_depth),
        None => line.starts_with('#') || line.starts_with("**"),
    };
    let section_length = lines[start_index + 1..].iter().position(ends_section).unwrap_or(lines.len() - start_index - 1);
    Lines { first_line: start_index + 1, last_line: start_index + 1 + section_length }
}

/// The rows of the list: for each source the change touches, each doc
/// section its comments point at that no changed line is in -- (file
/// changed, doc, section), sorted, each once.
fn untouched_sections(root_folder: &Path, changed_files: &str, diff_text: &str) -> BTreeSet<(String, String, String)> {
    let changed_stretches = changed_lines(diff_text);
    let mut untouched_rows = BTreeSet::new();
    for changed_file in changed_files.lines().filter(|listed_file| listed_file.ends_with(".rs")) {
        // A file the change removed points at nothing any more.
        let Ok(source_text) = fs::read_to_string(root_folder.join(changed_file)) else {
            continue;
        };
        for (doc_path, section_title) in doc_pointers(root_folder, changed_file, &source_text) {
            let section_stretch = section_lines(&fs::read_to_string(root_folder.join(&doc_path)).unwrap_or_default(), section_title.as_deref());
            if !changed_stretches.iter().any(|(path, lines)| *path == doc_path && lines.shares_line_with(section_stretch)) {
                untouched_rows.insert((changed_file.to_string(), doc_path, section_title.unwrap_or_else(|| WHOLE_DOC.to_string())));
            }
        }
    }
    untouched_rows
}

/// Prints the list for what changed since the commit given, the
/// working tree's changes among them. Fails only if git does.
fn list_stale_docs(given_command: &GivenCommand) -> Result<(), String> {
    let from_commit = given_command.argument_text(FROM_COMMIT)?;
    let root_folder = PathBuf::from(git_output(Path::new("."), &["rev-parse", "--show-toplevel"])?.trim());
    let changed_files = git_output(&root_folder, &["diff", "--name-only", from_commit])?;
    let diff_text = git_output(&root_folder, &["diff", "-U0", from_commit])?;
    let untouched_rows = untouched_sections(&root_folder, &changed_files, &diff_text);
    if untouched_rows.is_empty() {
        println!("Since {from_commit}: no doc section the changed code points at was left untouched.");
        return Ok(());
    }
    println!("Since {from_commit}: {} doc sections the changed code points at, untouched. Read each: update it, or say in the commit why it still holds.\n", untouched_rows.len());
    let mut table = Table::new(&["file changed", "doc", "section"]).left_aligned(&["doc", "section"]);
    for (changed_file, doc_path, section_title) in &untouched_rows {
        table.add_row(&[changed_file, doc_path, section_title]);
    }
    table.print_rendered();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{changed_lines, quoted_titles, section_lines, Lines};

    /// The titles quoted after a path are a comment's sections, to the
    /// end of its sentence, with or without backticks of their own.
    #[test]
    fn the_titles_after_a_path_are_the_sections() {
        assert_eq!(quoted_titles(", \"`one.rs`\" and \"Two\"). Not \"this\""), ["one.rs", "Two"]);
        assert_eq!(quoted_titles(". Then \"this\""), [""; 0]);
        assert_eq!(quoted_titles(" and `other.md`, \"its\""), [""; 0]);
    }

    /// A diff's hunks are read as the lines the file now has: added
    /// ones as themselves, removed ones as the two about them, and a
    /// removed file as nothing.
    #[test]
    fn a_diff_is_read_as_the_lines_changed() {
        let diff_text = "--- a/one.md\n+++ b/one.md\n@@ -3 +3,2 @@ heading\n+new\n+++ more\n@@ -9,2 +10,0 @@\n-gone\n--- a/two.md\n+++ /dev/null\n@@ -1,4 +0,0 @@\n";
        assert_eq!(changed_lines(diff_text), [("one.md".to_string(), Lines { first_line: 3, last_line: 4 }), ("one.md".to_string(), Lines { first_line: 10, last_line: 11 })]);
    }

    /// A section runs from its heading to the next as deep; an entry
    /// led by bold to the next such; one not there is the whole doc.
    #[test]
    fn a_section_is_its_heading_to_the_next() {
        let doc_text = "# Doc\n\n## First\ntext\n### Within\ntext\n## `second.rs`: more\n**`entry`**: said\nmore\n**other**: said\n";
        assert_eq!(section_lines(doc_text, Some("First")), Lines { first_line: 3, last_line: 6 });
        assert_eq!(section_lines(doc_text, Some("second.rs")), Lines { first_line: 7, last_line: 10 });
        assert_eq!(section_lines(doc_text, Some("entry")), Lines { first_line: 8, last_line: 9 });
        assert_eq!(section_lines(doc_text, Some("absent")), Lines { first_line: 1, last_line: 10 });
        assert_eq!(section_lines(doc_text, None), Lines { first_line: 1, last_line: 10 });
    }
}
