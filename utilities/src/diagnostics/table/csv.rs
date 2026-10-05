//! A table as CSV ([`crate::csv`]), and back: the headings as the first
//! row (a stacked heading's lines joined by newlines), then one CSV row
//! per table row, and a line `---` wherever a divider goes.

use super::Table;
use crate::csv::{self, lines, Line, DIVIDER};

impl Table {
    /// The table as CSV: its headings, its rows, its dividers.
    pub fn to_csv(&self) -> String {
        let headings: Vec<String> = self.headings.iter().map(|lines| lines.join("\n")).collect();
        let mut text = csv::row(&headings);
        for (index, row) in self.rows.iter().enumerate() {
            if self.dividers.contains(&index) {
                text.push_str(&format!("{DIVIDER}\n"));
            }
            text.push_str(&csv::row(row));
        }
        if self.dividers.contains(&self.rows.len()) {
            text.push_str(&format!("{DIVIDER}\n"));
        }
        text
    }

    /// The table `lines` hold: the first row its headings, the rest
    /// its rows, and a divider at every divider line. Comments and blanks are
    /// skipped.
    pub fn from_lines(lines: &[Line]) -> Self {
        let mut rows = lines.iter().filter(|line| matches!(line, Line::Row(_) | Line::Divider));
        let headings = match rows.next() {
            Some(Line::Row(headings)) => headings.clone(),
            other => panic!("a table starts with its headings, not {other:?}"),
        };
        let mut table = Table::new(&headings.iter().map(String::as_str).collect::<Vec<_>>());
        for line in rows {
            match line {
                Line::Row(row) => table.row(row),
                _ => table.divider(),
            }
        }
        table
    }

    /// The table a CSV text holds, as [`Table::to_csv`] writes it.
    pub fn from_csv(text: &str) -> Self {
        Self::from_lines(&lines(text))
    }
}
