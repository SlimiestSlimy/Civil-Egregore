//! The table printer's text forms: a table and a report written out and
//! read back.
//!
//! `cargo test`

use utilities::csv::{lines, Line};
use utilities::diagnostics::table::report::Report;
use utilities::transient_data::TRANSIENT_DATA;
use utilities::diagnostics::table::Table;

/// A table with every awkward field -- a comma, a quote, a newline, an
/// empty one, one reading as a comment or a divider -- and dividers comes back
/// from CSV field for field, alone and inside a report.
#[test]
fn a_table_round_trips_through_csv() {
    let awkward_fields = ["a, b", "say \"so\"", "two\nlines", "", "# not a note", "---"];
    let mut table = Table::new(&["name", "stacked\nheading"]);
    for awkward_field in awkward_fields {
        table.add_row(&[awkward_field, "1"]);
        table.divider();
    }
    let csv_text = table.to_csv();
    assert_eq!(Table::from_csv(&csv_text).to_csv(), csv_text);
    let first_fields: Vec<String> = lines(&csv_text)
        .into_iter()
        .filter_map(|line| match line {
            Line::TableRow(row_fields) => Some(row_fields[0].clone()),
            _ => None,
        })
        .collect();
    assert_eq!(first_fields, ["name"].into_iter().chain(awkward_fields).collect::<Vec<_>>());

    let mut report = Report::new("round trip", "a test");
    report.add_note("a note");
    report.add_table("first", table);
    report.add_table("second", Table::from_csv(&csv_text));
    let report_text = report.to_text();
    assert_eq!(Report::from_text("round trip", &report_text).to_text(), report_text);

    // Kept in the transient data's measurements and read back from there:
    // the same, with the commit it was measured on noted.
    let folder = TRANSIENT_DATA.measurements_folder();
    let kept_path = Report::from_text("round trip", &report_text).write_to_folder(&folder);
    assert!(kept_path.starts_with(TRANSIENT_DATA.path_under("")) && kept_path.exists());
    let read_back = Report::read_kept(&folder, "round trip").expect("kept").to_text();
    let without_commit: String = read_back.lines().filter(|line| !line.starts_with("# commit ")).map(|line| format!("{line}\n")).collect();
    assert_eq!(without_commit, report_text);
}
