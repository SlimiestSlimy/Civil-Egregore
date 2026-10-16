//! CSV: what every text file Civil Egregore keeps is written as, a first
//! row naming its columns (`docs/utilities.md`, "CSV").


/// The line that marks a divider.
pub const DIVIDER: &str = "---";
/// What a line that is not a row starts with: a report's note or
/// title.
pub const COMMENT_MARK: char = '#';
/// Between fields.
const FIELD_SEPARATOR: char = ',';
/// Around a quoted field.
const QUOTE_MARK: char = '"';

/// A line of CSV text, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// A row: its fields.
    TableRow(Vec<String>),
    /// A divider.
    Divider,
    /// A line starting with [`COMMENT_MARK`], without it.
    CommentLine(String),
    /// An empty line.
    BlankLine,
}

/// `field_text` as CSV: bare, or quoted when it has to be.
fn field_as_csv(field_text: &str) -> String {
    let needs_quotes = field_text == DIVIDER
        || field_text.starts_with(COMMENT_MARK)
        || field_text.contains([FIELD_SEPARATOR, QUOTE_MARK, '\n']);
    if needs_quotes {
        format!("{QUOTE_MARK}{}{QUOTE_MARK}", field_text.replace(QUOTE_MARK, &format!("{QUOTE_MARK}{QUOTE_MARK}")))
    } else {
        field_text.to_string()
    }
}

/// `row_fields` as one CSV row, ended by a newline.
pub fn row_text<S: AsRef<str>>(row_fields: &[S]) -> String {
    // A row of one empty field would be a blank line: quoted, it is a row.
    if matches!(row_fields, [only_field] if only_field.as_ref().is_empty()) {
        return format!("{QUOTE_MARK}{QUOTE_MARK}\n");
    }
    let quoted_fields: Vec<String> = row_fields.iter().map(|raw_field| field_as_csv(raw_field.as_ref())).collect();
    format!("{}\n", quoted_fields.join(&FIELD_SEPARATOR.to_string()))
}

/// Every line of `csv_text`: rows, dividers, comments and blanks. A quoted
/// field may run over several lines.
pub fn lines(csv_text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut text_chars = csv_text.chars().peekable();
    while let Some(&first_char) = text_chars.peek() {
        if first_char == '\n' {
            text_chars.next();
            lines.push(Line::BlankLine);
            continue;
        }
        if first_char == COMMENT_MARK {
            text_chars.next();
            let comment_text: String = text_chars.by_ref().take_while(|&comment_char| comment_char != '\n').collect();
            lines.push(Line::CommentLine(comment_text));
            continue;
        }
        let mut row_fields = vec![String::new()];
        let mut in_quotes = false;
        while let Some(next_char) = text_chars.next() {
            let current_field = row_fields.last_mut().expect("a field");
            match next_char {
                QUOTE_MARK if in_quotes && text_chars.peek() == Some(&QUOTE_MARK) => {
                    text_chars.next();
                    current_field.push(QUOTE_MARK);
                }
                QUOTE_MARK => in_quotes = !in_quotes,
                FIELD_SEPARATOR if !in_quotes => row_fields.push(String::new()),
                '\n' if !in_quotes => break,
                plain_char => current_field.push(plain_char),
            }
        }
        // A divider is the one bare line that reads as a lone `---`: a field
        // `---` is always quoted.
        let bare_divider = row_fields.len() == 1 && row_fields[0] == DIVIDER;
        lines.push(if bare_divider { Line::Divider } else { Line::TableRow(row_fields) });
    }
    lines
}


/// The rows of `csv_text` alone, each its fields: what a file kept as a
/// plain table is, its notes, blanks and dividers passed over. The
/// first is the one naming the columns.
pub fn rows_with_header(csv_text: &str) -> Vec<Vec<String>> {
    lines(csv_text).into_iter().filter_map(|line| if let Line::TableRow(row_fields) = line { Some(row_fields) } else { None }).collect()
}

/// The rows of `csv_text` after the one naming its columns.
pub fn rows_named(csv_text: &str) -> Vec<Vec<String>> {
    rows_with_header(csv_text).into_iter().skip(1).collect()
}

/// The rows of `csv_text` after the one naming its columns, each its
/// fields of `wanted_columns`, in their order, whatever order the text has
/// them in: a column the text lacks, or a field a row lacks, empty.
pub fn rows_by_column(csv_text: &str, wanted_columns: &[&str]) -> Vec<Vec<String>> {
    let mut remaining_rows = rows_with_header(csv_text).into_iter();
    let Some(header_row) = remaining_rows.next() else {
        return Vec::new();
    };
    let column_positions: Vec<Option<usize>> = wanted_columns.iter().map(|wanted_column| header_row.iter().position(|header_name| header_name.trim() == *wanted_column)).collect();
    remaining_rows.map(|data_row| column_positions.iter().map(|column_position| column_position.and_then(|field_position| data_row.get(field_position)).cloned().unwrap_or_default()).collect()).collect()
}
