//! Prints the reports kept in `transient_data/measurements/` -- every one, or the one
//! named after the tool -- read back from their files, without
//! measuring anything again.

use crate::transient_data::measurements_folder;
use utilities::commands::GivenCommand;
use utilities::diagnostics::table::report::{saved_reports, Report};

/// The parameter: the tool whose report is printed; every one if not given.
pub const TOOL_PARAMETER: &str = "a tool's name";

/// Prints the kept report of the tool named, or every one.
pub fn run_show_tool(given_parameters: &GivenCommand) -> Result<(), String> {
    let report_names = match given_parameters.given_argument(TOOL_PARAMETER) {
        Some(given_name) => vec![given_name.to_string()],
        None => saved_reports(&measurements_folder()),
    };
    for report_name in report_names {
        let report = Report::read_kept(&measurements_folder(), &report_name).ok_or_else(|| format!("no report kept as {report_name}: one of {}", saved_reports(&measurements_folder()).join(", ")))?;
        report.print_report();
    }
    Ok(())
}
