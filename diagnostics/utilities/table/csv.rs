//! A table as CSV ([`crate::csv`]), and back: the headings as the first
//! row (a stacked heading's lines joined by newlines), then one CSV row
//! per table row, and a line `---` wherever a divider goes.

use super::Table;
use crate::csv::{self, lines, Line, DIVIDER};

impl Table {
    /// The table as CSV: its headings, its rows, its dividers.
    pub fn to_csv(&self) -> String {
        let heading_fields: Vec<String> = self.column_headings.iter().map(|lines| lines.join("\n")).collect();
        let mut csv_text = csv::row_text(&heading_fields);
        for (row_index, table_row) in self.table_rows.iter().enumerate() {
            if self.dividers.contains(&row_index) {
                csv_text.push_str(&format!("{DIVIDER}\n"));
            }
            csv_text.push_str(&csv::row_text(table_row));
        }
        if self.dividers.contains(&self.table_rows.len()) {
            csv_text.push_str(&format!("{DIVIDER}\n"));
        }
        csv_text
    }

    /// The table `lines` hold: the first row its headings, the rest
    /// its rows, and a divider at every divider line. Comments and blanks are
    /// skipped.
    pub fn from_lines(lines: &[Line]) -> Self {
        let mut row_lines = lines.iter().filter(|line| matches!(line, Line::TableRow(_) | Line::Divider));
        let heading_row = match row_lines.next() {
            Some(Line::TableRow(first_row)) => first_row.clone(),
            first_line => panic!("a table starts with its headings, not {first_line:?}"),
        };
        let mut table = Table::new(&heading_row.iter().map(String::as_str).collect::<Vec<_>>());
        for line in row_lines {
            match line {
                Line::TableRow(row_fields) => table.add_row(row_fields),
                _ => table.divider(),
            }
        }
        table
    }

    /// The table a CSV text holds, as [`Table::to_csv`] writes it.
    pub fn from_csv(csv_text: &str) -> Self {
        Self::from_lines(&lines(csv_text))
    }
}
