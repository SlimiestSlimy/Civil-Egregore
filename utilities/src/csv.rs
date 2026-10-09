//! CSV: what every text file Civil Egregore keeps is written as -- a seed, the
//! settings, the sliders, a world's file, a measurement's report -- so
//! that one reader and one writer serve them all, and any of them opens
//! in whatever reads tables. A file's first row names its columns.
//!
//! A field is quoted -- inside `"`, with any `"` in it doubled -- when it
//! holds a comma, a quote or a newline, or is a row's one field and empty, or could be read as
//! one of the lines that are not rows: a divider (`---`, a table's), or
//! a line starting with `#`, which is a note to whoever reads the file
//! and no row. Everything else is written bare.


/// The line that marks a divider.
pub const DIVIDER: &str = "---";
/// What a line that is not a row starts with: a report's note or
/// title.
pub const COMMENT: char = '#';
/// Between fields.
const SEPARATOR: char = ',';
/// Around a quoted field.
const QUOTE: char = '"';

/// A line of CSV text, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// A row: its fields.
    Row(Vec<String>),
    /// A divider.
    Divider,
    /// A line starting with [`COMMENT`], without it.
    Comment(String),
    /// An empty line.
    Blank,
}

/// `field` as CSV: bare, or quoted when it has to be.
fn field(field: &str) -> String {
    let quoted = field == DIVIDER
        || field.starts_with(COMMENT)
        || field.contains([SEPARATOR, QUOTE, '\n']);
    if quoted {
        format!("{QUOTE}{}{QUOTE}", field.replace(QUOTE, &format!("{QUOTE}{QUOTE}")))
    } else {
        field.to_string()
    }
}

/// `fields` as one CSV row, ended by a newline.
pub fn row<S: AsRef<str>>(fields: &[S]) -> String {
    // A row of one empty field would be a blank line: quoted, it is a row.
    if matches!(fields, [only] if only.as_ref().is_empty()) {
        return format!("{QUOTE}{QUOTE}\n");
    }
    let fields: Vec<String> = fields.iter().map(|text| field(text.as_ref())).collect();
    format!("{}\n", fields.join(&SEPARATOR.to_string()))
}

/// Every line of `text`: rows, dividers, comments and blanks. A quoted
/// field may run over several lines.
pub fn lines(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&first) = chars.peek() {
        if first == '\n' {
            chars.next();
            lines.push(Line::Blank);
            continue;
        }
        if first == COMMENT {
            chars.next();
            let comment: String = chars.by_ref().take_while(|&c| c != '\n').collect();
            lines.push(Line::Comment(comment));
            continue;
        }
        let mut fields = vec![String::new()];
        let mut quoted = false;
        while let Some(c) = chars.next() {
            let current = fields.last_mut().expect("a field");
            match c {
                QUOTE if quoted && chars.peek() == Some(&QUOTE) => {
                    chars.next();
                    current.push(QUOTE);
                }
                QUOTE => quoted = !quoted,
                SEPARATOR if !quoted => fields.push(String::new()),
                '\n' if !quoted => break,
                c => current.push(c),
            }
        }
        // A divider is the one bare line that reads as a lone `---`: a field
        // `---` is always quoted.
        let bare_divider = fields.len() == 1 && fields[0] == DIVIDER;
        lines.push(if bare_divider { Line::Divider } else { Line::Row(fields) });
    }
    lines
}


/// The rows of `text` alone, each its fields: what a file kept as a
/// plain table is, its notes, blanks and dividers passed over. The
/// first is the one naming the columns.
pub fn rows(text: &str) -> Vec<Vec<String>> {
    lines(text).into_iter().filter_map(|line| if let Line::Row(fields) = line { Some(fields) } else { None }).collect()
}

/// The rows of `text` after the one naming its columns.
pub fn rows_named(text: &str) -> Vec<Vec<String>> {
    rows(text).into_iter().skip(1).collect()
}

/// The rows of `text` after the one naming its columns, each its
/// fields of `columns`, in their order, whatever order the text has
/// them in: a column the text lacks, or a field a row lacks, empty.
pub fn rows_by_column(text: &str, columns: &[&str]) -> Vec<Vec<String>> {
    let mut rows = rows(text).into_iter();
    let Some(named) = rows.next() else {
        return Vec::new();
    };
    let at: Vec<Option<usize>> = columns.iter().map(|column| named.iter().position(|name| name.trim() == *column)).collect();
    rows.map(|row| at.iter().map(|at| at.and_then(|at| row.get(at)).cloned().unwrap_or_default()).collect()).collect()
}
