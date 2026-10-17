//! One table printer for every measurement, so that a column means and
//! looks the same wherever it is printed (`docs/utilities.md`, "Tables
//! and reports").

pub mod csv;
pub mod report;

/// A table being built. The first column is left aligned and named
/// rather than numbered; the rest are right aligned figures, but for
/// any [`Table::left_aligned`] names as words.
pub struct Table {
    /// Each column's heading, one entry a line of it.
    column_headings: Vec<Vec<String>>,
    /// Whether each column is left aligned: words, rather than figures.
    is_left_aligned: Vec<bool>,
    /// Each row's fields, in column order.
    table_rows: Vec<Vec<String>>,
    /// Where dividers go: before the row at each of these indices.
    dividers: Vec<usize>,
}

impl Table {
    /// A table with these column headings. A heading may hold newlines,
    /// and then it stacks over as many lines as it needs.
    pub fn new(heading_texts: &[&str]) -> Self {
        Self {
            column_headings: heading_texts.iter().map(|heading_text| heading_text.split('\n').map(str::to_string).collect()).collect(),
            is_left_aligned: (0..heading_texts.len()).map(|column_index| column_index == 0).collect(),
            table_rows: Vec::new(),
            dividers: Vec::new(),
        }
    }

    /// The columns headed `left_headings` hold words, not figures: left
    /// aligned, as the first column is. Each must be one of the table's
    /// headings, as given to [`Table::new`].
    pub fn left_aligned(mut self, left_headings: &[&str]) -> Self {
        for left_heading in left_headings {
            let column_index = self
                .column_headings
                .iter()
                .position(|lines| lines.join("\n") == *left_heading)
                .unwrap_or_else(|| panic!("no column headed {left_heading:?}"));
            self.is_left_aligned[column_index] = true;
        }
        self
    }

    /// One row. Must have as many fields as there are headings.
    pub fn add_row<S: AsRef<str>>(&mut self, row_fields: &[S]) {
        assert_eq!(row_fields.len(), self.column_headings.len(), "a row must fill every column");
        self.table_rows.push(row_fields.iter().map(|row_field| row_field.as_ref().to_string()).collect());
    }

    /// A divider under the row last added, for a total or a group.
    pub fn divider(&mut self) {
        self.dividers.push(self.table_rows.len());
    }

    /// The width each column needs: the widest of its heading lines and
    /// its fields.
    fn column_widths(&self) -> Vec<usize> {
        (0..self.column_headings.len())
            .map(|column_index| {
                let heading_width = self.column_headings[column_index].iter().map(|line| line.chars().count()).max().unwrap_or(0);
                let field_width = self.table_rows.iter().map(|table_row| table_row[column_index].chars().count()).max().unwrap_or(0);
                heading_width.max(field_width)
            })
            .collect()
    }

    /// One printed line of `line_fields`, each column as wide as
    /// `column_widths` says, and aligned as [`Table::is_left_aligned`] says.
    fn printed_line(&self, column_widths: &[usize], line_fields: &[String]) -> String {
        let mut line = String::from("  ");
        for (column_index, &width) in column_widths.iter().enumerate() {
            let column_field = line_fields.get(column_index).map_or("", String::as_str);
            if column_index > 0 {
                line.push_str(" | ");
            }
            if self.is_left_aligned[column_index] {
                line.push_str(&format!("{column_field:<width$}"));
            } else {
                line.push_str(&format!("{column_field:>width$}"));
            }
        }
        // Right hand padding goes, but never a column bar: a heading
        // line that ends in blank cells still has to show where its
        // columns are.
        let after_last_bar = line.rfind('|').map_or(0, |bar| bar + 1);
        let (with_bars, trailing_padding) = line.split_at(after_last_bar);
        format!("{with_bars}{}", trailing_padding.trim_end())
    }

    /// A divider across every column, each as wide as `column_widths` says.
    fn divider_line(column_widths: &[usize]) -> String {
        let column_dashes: Vec<String> = column_widths.iter().map(|&width| "-".repeat(width)).collect();
        format!("  {}", column_dashes.join("-+-"))
    }

    /// Prints the table, on standard output: [`Table::rendered_text`].
    pub fn print_rendered(&self) {
        print!("{}", self.rendered_text());
    }

    /// The table as printed: the headings in a ruled block, then the
    /// rows, every line ended.
    pub fn rendered_text(&self) -> String {
        let column_widths = self.column_widths();
        let heading_lines = self.column_headings.iter().map(Vec::len).max().unwrap_or(1);
        let mut lines = vec![Self::divider_line(&column_widths)];
        // Headings sit at the bottom of their stack, so a one line
        // heading lines up with the last line of a taller one.
        for heading_line in 0..heading_lines {
            let heading_fields: Vec<String> = self
                .column_headings
                .iter()
                .map(|stacked_heading| {
                    let blank_lines_above = heading_lines - stacked_heading.len();
                    heading_line.checked_sub(blank_lines_above).map_or(String::new(), |line| stacked_heading[line].clone())
                })
                .collect();
            lines.push(self.printed_line(&column_widths, &heading_fields));
        }
        lines.push(Self::divider_line(&column_widths));

        for (row_index, table_row) in self.table_rows.iter().enumerate() {
            lines.push(self.printed_line(&column_widths, table_row));
            if self.dividers.contains(&(row_index + 1)) {
                lines.push(Self::divider_line(&column_widths));
            }
        }
        lines.iter().map(|line| format!("{line}\n")).collect()
    }
}
