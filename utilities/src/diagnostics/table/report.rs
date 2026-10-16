//! A measurement's report: its tables, each titled, and notes on what
//! they were measured on -- printed, and kept as one CSV file
//! (`docs/utilities.md`, "Tables and reports").

use crate::csv::{lines, Line, COMMENT_MARK};
use super::Table;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A report file's extension.
const REPORT_EXTENSION: &str = "csv";

/// A measurement's tables and notes.
pub struct Report {
    /// What the measurement is called: its file's name.
    report_name: String,
    /// Notes on what it was measured on.
    report_notes: Vec<String>,
    /// Its tables, each titled.
    tables: Vec<(String, Table)>,
}

/// The file the report named `report_name` is kept in, in `folder`.
pub fn path(folder: &Path, report_name: &str) -> PathBuf {
    folder.join(format!("{report_name}.{REPORT_EXTENSION}"))
}

/// The names of every report kept in `folder`, in name order.
pub fn saved_reports(folder: &Path) -> Vec<String> {
    let mut report_names: Vec<String> = fs::read_dir(folder)
        .map(|folder_entries| {
            folder_entries
                .filter_map(|folder_entry| {
                    let path = folder_entry.ok()?.path();
                    (path.extension()? == REPORT_EXTENSION).then(|| path.file_stem()?.to_str().map(str::to_string))?
                })
                .collect()
        })
        .unwrap_or_default();
    report_names.sort();
    report_names
}

/// The commit the checkout holding `folder` is at, and whether it had
/// changes not yet committed, as git says; `None` outside a git
/// checkout.
fn checkout_commit(folder: &Path) -> Option<String> {
    let run_git = |git_arguments: &[&str]| {
        let git_output = Command::new("git").args(git_arguments).current_dir(folder).output().ok()?;
        git_output.status.success().then(|| String::from_utf8_lossy(&git_output.stdout).trim().to_string())
    };
    let commit_hash = run_git(&["rev-parse", "--short", "HEAD"])?;
    let has_changes = run_git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|status_text| !status_text.is_empty());
    Some(if has_changes { format!("{commit_hash}, with uncommitted changes") } else { commit_hash })
}

impl Report {
    /// An empty report named `report_name`, made by `rerun_command`: what reruns it.
    pub fn new(report_name: &str, rerun_command: &str) -> Self {
        Self { report_name: report_name.to_string(), report_notes: vec![format!("written by `{rerun_command}`")], tables: Vec::new() }
    }

    /// Adds a note.
    pub fn add_note(&mut self, note_text: impl Into<String>) {
        self.report_notes.push(note_text.into());
    }

    /// Adds `table`, titled `table_title`.
    pub fn add_table(&mut self, table_title: impl Into<String>, table: Table) {
        self.tables.push((table_title.into(), table));
    }

    /// Prints the report's name, its notes, then every table under its
    /// title.
    pub fn print_report(&self) {
        println!("\n  == {}", self.report_name);
        for report_note in &self.report_notes {
            println!("  {report_note}");
        }
        for (table_title, table) in &self.tables {
            println!("\n  {table_title}");
            table.print_rendered();
        }
    }

    /// The report as its file holds it.
    pub fn to_text(&self) -> String {
        let mut report_text: String = self.report_notes.iter().map(|report_note| format!("{COMMENT_MARK} {report_note}\n")).collect();
        for (table_title, table) in &self.tables {
            report_text.push_str(&format!("\n{COMMENT_MARK}{COMMENT_MARK} {table_title}\n"));
            report_text.push_str(&table.to_csv());
        }
        report_text
    }

    /// The report named `report_name` a file's `report_text` holds.
    pub fn from_text(report_name: &str, report_text: &str) -> Self {
        let mut report = Self { report_name: report_name.to_string(), report_notes: Vec::new(), tables: Vec::new() };
        let mut table_lines: Vec<Line> = Vec::new();
        let mut current_title: Option<String> = None;
        for line in lines(report_text) {
            match line {
                Line::CommentLine(comment_text) => match comment_text.strip_prefix(COMMENT_MARK) {
                    Some(next_title) => {
                        report.finish_table(current_title.take(), &mut table_lines);
                        current_title = Some(next_title.trim().to_string());
                    }
                    None => report.report_notes.push(comment_text.trim().to_string()),
                },
                Line::BlankLine => {}
                line => table_lines.push(line),
            }
        }
        report.finish_table(current_title, &mut table_lines);
        report
    }

    /// Ends the table titled `table_title`, if any, read from `table_lines`,
    /// and empties them for the next.
    fn finish_table(&mut self, table_title: Option<String>, table_lines: &mut Vec<Line>) {
        if let Some(finished_title) = table_title {
            self.tables.push((finished_title, Table::from_lines(table_lines)));
        }
        table_lines.clear();
    }

    /// The report kept as `report_name` in `folder`, if there is one.
    pub fn read_kept(folder: &Path, report_name: &str) -> Option<Self> {
        fs::read_to_string(path(folder, report_name)).ok().map(|kept_text| Self::from_text(report_name, &kept_text))
    }

    /// Notes the commit the numbers were measured on, then prints the
    /// report and keeps it in its file in `folder`, replacing the last
    /// run's. Anything else it was measured on -- a seed, say -- the
    /// caller notes first.
    pub fn print_and_keep(mut self, folder: &Path) {
        self.note_commit(folder);
        self.print_report();
        let path = self.write(folder);
        // Where it went, from where the run started, when it is under it.
        let current_folder = std::env::current_dir().unwrap_or_default();
        println!("\n  kept in {}", path.strip_prefix(&current_folder).unwrap_or(&path).display());
    }

    /// [`Report::print_and_keep`] without printing anything -- for a run whose
    /// standard output is something else, a video say: the report kept
    /// in its file in `folder`, and where.
    pub fn write_to_folder(mut self, folder: &Path) -> PathBuf {
        self.note_commit(folder);
        self.write(folder)
    }

    /// Notes the commit the numbers were measured on, if `folder` is in
    /// a git checkout.
    fn note_commit(&mut self, folder: &Path) {
        fs::create_dir_all(folder).expect("the measurements folder");
        if let Some(commit_text) = checkout_commit(folder) {
            self.add_note(format!("commit {commit_text}"));
        }
    }

    /// Writes the report into its file in `folder`, replacing the last
    /// run's: where.
    fn write(&self, folder: &Path) -> PathBuf {
        let path = path(folder, &self.report_name);
        fs::write(&path, self.to_text()).expect("the report written");
        path
    }
}
