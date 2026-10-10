//! The docs a change may have left stale: every doc section the
//! changed code's comments point at that the change did not touch, as
//! a table to read -- a list, not a test.
//!
//! How it reads a change: `docs/utilities.md`, "Stale docs".

use crate::commands::{Command, Given, Parameter};
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
pub const COMMANDS: [Command; 1] = [Command {
    name: "stale",
    does: "the doc sections the code changed since a commit points at, that the change left untouched: a list to read",
    parameters: &[Parameter::new(FROM_COMMIT, "HEAD")],
    run,
}];

/// A stretch of a file's lines, both ends counted, the first line 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Lines {
    /// The first line.
    first: usize,
    /// The last line.
    last: usize,
}

impl Lines {
    /// Whether the two stretches share a line.
    fn meet(self, other: Lines) -> bool {
        self.first <= other.last && other.first <= self.last
    }
}

/// What `git`, run in `folder` with `arguments`, prints; or why it
/// did not.
fn git(folder: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = process::Command::new("git").arg("-C").arg(folder).args(arguments).output().map_err(|why| format!("git could not be run: {why}"))?;
    if !output.status.success() {
        return Err(format!("git {}: {}", arguments.join(" "), String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The lines `diff` -- what `git diff -U0` prints -- changes, file by
/// file, numbered as each file is now. Lines only removed count as the
/// two they stood between.
fn changed_lines(diff: &str) -> Vec<(String, Lines)> {
    let (mut changed, mut file, mut before) = (Vec::new(), None, "");
    for line in diff.lines() {
        if let (true, Some(path)) = (before.starts_with("--- "), line.strip_prefix("+++ ")) {
            file = path.strip_prefix("b/").map(str::to_string);
        } else if let (Some(file), Some(hunk)) = (&file, line.strip_prefix("@@ ")) {
            let now = hunk.split(' ').find_map(|part| part.strip_prefix('+')).unwrap_or_default();
            let (first, count) = now.split_once(',').unwrap_or((now, "1"));
            if let (Ok(first), Ok(count)) = (first.parse::<usize>(), count.parse::<usize>()) {
                let last = if count == 0 { first + 1 } else { first + count - 1 };
                changed.push((file.clone(), Lines { first, last }));
            }
        }
        before = line;
    }
    changed
}

/// What `line` writes between backticks.
fn spans(line: &str) -> Vec<&str> {
    line.split('`').skip(1).step_by(2).collect()
}

/// The folder of the crate the file at `path`, from the root, is in:
/// the nearest above it with a manifest.
fn crate_of(root: &Path, path: &str) -> PathBuf {
    let folders = Path::new(path).ancestors().skip(1);
    folders.map(|folder| root.join(folder)).find(|folder| folder.join("Cargo.toml").exists()).unwrap_or_else(|| root.to_path_buf())
}

/// The titles `after` -- what follows a doc's path -- quotes, a
/// title's own backticks left off: every one until the sentence ends,
/// or a bracket closes, or other code is written.
fn quoted_titles(after: &str) -> Vec<&str> {
    let (mut titles, mut parts) = (Vec::new(), after.split('"'));
    while let (Some(between), Some(title)) = (parts.next(), parts.next()) {
        if between.contains(['`', '|', ')']) || between.contains(". ") {
            break;
        }
        titles.push(title.trim_matches('`'));
    }
    titles
}

/// The docs and sections the comments of the source at `path` point
/// at, `text` being what it holds: each a doc's path from the root and
/// a section's title, none for the whole doc. A doc that is not there
/// is left out: the fine tier's to say.
fn pointers(root: &Path, path: &str, text: &str) -> Vec<(String, Option<String>)> {
    let comments: Vec<(usize, &str)> = text.lines().enumerate().filter_map(|(number, line)| line.split_once("//").map(|(_, comment)| (number, comment.trim_start_matches(['/', '!'])))).collect();
    let own = crate_of(root, path);
    let mut pointed = Vec::new();
    for (place, &(number, comment)) in comments.iter().enumerate() {
        for span in spans(comment).into_iter().filter(|span| span.ends_with(".md") && !span.contains(char::is_whitespace)) {
            let Some(doc) = [own.as_path(), root].iter().map(|from| from.join(span.trim_start_matches("../"))).find(|there| there.is_file()) else {
                continue;
            };
            let doc = doc.strip_prefix(root).unwrap_or(&doc).to_string_lossy().into_owned();
            let after = comment.split_once(&format!("`{span}`")).map_or("", |(_, after)| after);
            let next = comments.get(place + 1).filter(|(following, _)| *following == number + 1).map_or("", |&(_, next)| next);
            let after = format!("{after} {next}").split_whitespace().collect::<Vec<_>>().join(" ");
            let titles = quoted_titles(&after);
            match titles.is_empty() {
                true => pointed.push((doc, None)),
                false => pointed.extend(titles.into_iter().map(|title| (doc.clone(), Some(title.to_string())))),
            }
        }
    }
    pointed
}

/// How deep the heading `line` is: its `#`s, or none if it is not one.
fn heading_depth(line: &str) -> Option<usize> {
    Some(line.chars().take_while(|&letter| letter == '#').count()).filter(|&depth| depth > 0)
}

/// The lines of the section `title` begins in the doc `text`: from its
/// heading to the next as deep or less, or from a line led by bold to
/// the next heading or line led by bold. The whole doc if it has no
/// such section, or none is asked for.
fn section_lines(text: &str, title: Option<&str>) -> Lines {
    let lines: Vec<&str> = text.lines().collect();
    let whole = Lines { first: 1, last: lines.len().max(1) };
    let Some(title) = title else {
        return whole;
    };
    let begins = |line: &&str| (line.starts_with('#') || line.starts_with("**")) && line.trim_start_matches(['#', '*', '`', ' ']).starts_with(title);
    let Some(start) = lines.iter().position(begins) else {
        return whole;
    };
    let ends = |line: &&str| match heading_depth(lines[start]) {
        Some(depth) => heading_depth(line).is_some_and(|other| other <= depth),
        None => line.starts_with('#') || line.starts_with("**"),
    };
    let length = lines[start + 1..].iter().position(ends).unwrap_or(lines.len() - start - 1);
    Lines { first: start + 1, last: start + 1 + length }
}

/// The rows of the list: for each source the change touches, each doc
/// section its comments point at that no changed line is in -- (file
/// changed, doc, section), sorted, each once.
fn untouched_sections(root: &Path, changed_files: &str, diff: &str) -> BTreeSet<(String, String, String)> {
    let changed = changed_lines(diff);
    let mut rows = BTreeSet::new();
    for file in changed_files.lines().filter(|file| file.ends_with(".rs")) {
        // A file the change removed points at nothing any more.
        let Ok(text) = fs::read_to_string(root.join(file)) else {
            continue;
        };
        for (doc, title) in pointers(root, file, &text) {
            let section = section_lines(&fs::read_to_string(root.join(&doc)).unwrap_or_default(), title.as_deref());
            if !changed.iter().any(|(path, lines)| *path == doc && lines.meet(section)) {
                rows.insert((file.to_string(), doc, title.unwrap_or_else(|| WHOLE_DOC.to_string())));
            }
        }
    }
    rows
}

/// Prints the list for what changed since the commit given, the
/// working tree's changes among them. Fails only if git does.
fn run(given: &Given) -> Result<(), String> {
    let from = given.text(FROM_COMMIT)?;
    let root = PathBuf::from(git(Path::new("."), &["rev-parse", "--show-toplevel"])?.trim());
    let changed_files = git(&root, &["diff", "--name-only", from])?;
    let diff = git(&root, &["diff", "-U0", from])?;
    let rows = untouched_sections(&root, &changed_files, &diff);
    if rows.is_empty() {
        println!("Since {from}: no doc section the changed code points at was left untouched.");
        return Ok(());
    }
    println!("Since {from}: {} doc sections the changed code points at, untouched. Read each: update it, or say in the commit why it still holds.\n", rows.len());
    let mut table = Table::new(&["file changed", "doc", "section"]).left_aligned(&["doc", "section"]);
    for (file, doc, section) in &rows {
        table.row(&[file, doc, section]);
    }
    table.print();
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
        let diff = "--- a/one.md\n+++ b/one.md\n@@ -3 +3,2 @@ heading\n+new\n+++ more\n@@ -9,2 +10,0 @@\n-gone\n--- a/two.md\n+++ /dev/null\n@@ -1,4 +0,0 @@\n";
        assert_eq!(changed_lines(diff), [("one.md".to_string(), Lines { first: 3, last: 4 }), ("one.md".to_string(), Lines { first: 10, last: 11 })]);
    }

    /// A section runs from its heading to the next as deep; an entry
    /// led by bold to the next such; one not there is the whole doc.
    #[test]
    fn a_section_is_its_heading_to_the_next() {
        let doc = "# Doc\n\n## First\ntext\n### Within\ntext\n## `second.rs`: more\n**`entry`**: said\nmore\n**other**: said\n";
        assert_eq!(section_lines(doc, Some("First")), Lines { first: 3, last: 6 });
        assert_eq!(section_lines(doc, Some("second.rs")), Lines { first: 7, last: 10 });
        assert_eq!(section_lines(doc, Some("entry")), Lines { first: 8, last: 9 });
        assert_eq!(section_lines(doc, Some("absent")), Lines { first: 1, last: 10 });
        assert_eq!(section_lines(doc, None), Lines { first: 1, last: 10 });
    }
}
