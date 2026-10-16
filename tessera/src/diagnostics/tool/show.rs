//! Prints the reports kept in `transient_data/measurements/` -- every one, or the one
//! named after the tool -- read back from their files, without
//! measuring anything again.

use crate::transient_data::measurements;
use utilities::commands::Given;
use utilities::diagnostics::table::report::{saved_reports, Report};

/// The parameter: the tool whose report is printed; every one if not given.
pub const TOOL: &str = "a tool's name";

/// Prints the kept report of the tool named, or every one.
pub fn run(given: &Given) -> Result<(), String> {
    let names = match given.given(TOOL) {
        Some(name) => vec![name.to_string()],
        None => saved_reports(&measurements()),
    };
    for name in names {
        let report = Report::read(&measurements(), &name).ok_or_else(|| format!("no report kept as {name}: one of {}", saved_reports(&measurements()).join(", ")))?;
        report.print();
    }
    Ok(())
}
