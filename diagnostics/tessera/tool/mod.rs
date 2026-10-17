//! Tessera's diagnostics tools ([`COMMANDS`]), run by `Civil_Egregore
//! tessera <tool>`: each prints what `tessera::diagnostics` gathers,
//! and a tool that measures keeps its tables
//! (`docs/testing_protocol.md`, "The diagnostics tool").

mod adversarial;
mod census;
mod instruction_count;
mod measurement;
mod noise;
mod per_shape;
mod render;
mod show;
mod sparse;
mod timing;

use crate::transient_data;
use utilities::commands::{NamedCommand, GivenCommand, CommandParameter};
use utilities::diagnostics::table::report::Report;

/// Runs a tool that measures: it fills a report named as the tool,
/// which is printed and kept.
fn run_measuring_tool(given_parameters: &GivenCommand, fill_report: impl FnOnce(&mut Report, &GivenCommand) -> Result<(), String>) -> Result<(), String> {
    let mut report = Report::new(given_parameters.command_name(), &given_parameters.resolved_line());
    fill_report(&mut report, given_parameters)?;
    transient_data::publish_report(report);
    Ok(())
}

/// A tool that measures and takes nothing.
macro_rules! measuring_tool {
    ($run:path) => {
        |given| {
            run_measuring_tool(given, |report, _| {
                $run(report);
                Ok(())
            })
        }
    };
}

/// Every tool.
pub const COMMANDS: [NamedCommand; 12] = [
    NamedCommand { command_name: "measurement", what_it_does: "bits a bitmap from every corpus generator, a table a generator, a row a parameter set; then what the trees hold", command_parameters: &[], run_function: measuring_tool!(measurement::run_measurement_tool) },
    NamedCommand { command_name: "census", what_it_does: "what Tessera's tree is made of, node kind by level, for each bitmap looked at", command_parameters: &[], run_function: measuring_tool!(census::run_census_tool) },
    NamedCommand { command_name: "per_shape", what_it_does: "Tessera's bits on every shape, plan and line set on its own", command_parameters: &[], run_function: measuring_tool!(per_shape::run_per_shape_tool) },
    NamedCommand { command_name: "noise", what_it_does: "Tessera's bits on noise at several densities, against the raw cells", command_parameters: &[], run_function: measuring_tool!(noise::run_noise_tool) },
    NamedCommand { command_name: "sparse", what_it_does: "the tree against the binary count tree on sparse bitmaps, beside the least scattered cells can take", command_parameters: &[], run_function: measuring_tool!(sparse::run_sparse_tool) },
    NamedCommand { command_name: "timing", what_it_does: "time to encode and decode a large corpus, family by family", command_parameters: &[CommandParameter::new(timing::PER_GENERATOR, "100")], run_function: |given_parameters| run_measuring_tool(given_parameters, timing::run_timing_tool) },
    NamedCommand { command_name: "instruction_count", what_it_does: "instructions to encode and to decode a fixed corpus, counted by callgrind (needs valgrind)", command_parameters: &[], run_function: |given_parameters| run_measuring_tool(given_parameters, instruction_count::run_instruction_count_tool) },
    NamedCommand {
        command_name: instruction_count::CORPUS_TOOL,
        what_it_does: "nothing printed: encodes and decodes that corpus alone, uncounted -- what callgrind runs",
        command_parameters: &[],
        run_function: |_| {
            instruction_count::run_corpus();
            Ok(())
        },
    },
    NamedCommand {
        command_name: "render",
        what_it_does: "the bitmaps looked at, each written as a PNG image in transient_data/renders/",
        command_parameters: &[],
        run_function: |_| {
            render::run_render_tool();
            Ok(())
        },
    },
    NamedCommand { command_name: "show", what_it_does: "the kept measurements, read back without measuring", command_parameters: &[CommandParameter::new(show::TOOL_PARAMETER, "")], run_function: show::run_show_tool },
    NamedCommand { command_name: "adversarial", what_it_does: "searches for the bitmaps Tessera does worst on against the raw cells, keeping the worst", command_parameters: &[CommandParameter::new(adversarial::CHANGES_PARAMETER, "100")], run_function: adversarial::run_adversarial_tool },
    NamedCommand {
        command_name: "adversarial_save",
        what_it_does: "keeps a worst bitmap as a named bitmap no search replaces",
        command_parameters: &[CommandParameter::new(adversarial::FROM_PARAMETER, ""), CommandParameter::new(adversarial::NAME_PARAMETER, ""), CommandParameter::new(adversarial::DESCRIPTION_PARAMETER, "")],
        run_function: adversarial::save,
    },
];
