//! Tessera's diagnostics tools: each prints what `tessera::diagnostics`
//! gathers from Tessera, one tool a file. A tool that measures also
//! keeps its tables, and what they were measured on, in
//! `transient_data/measurements/<tool>.csv`, replacing the last run's --
//! the latest numbers are always there, and nowhere copied by hand.
//!
//! They are [`COMMANDS`], run by `tilesim tessera <tool>`
//! ([`dispatch`]); every tool, what it prints and what it takes are in
//! that table, and printed by `tilesim tessera`.
//!
//! The bitmaps looked at are the adversarial worst bitmaps and saved bitmaps
//! (`transient_data/worst/`, `external_benchmarks/adversarial/saved/`), plus
//! any PBM image named in `TESSERA_DIAGNOSE`.
//! Every tool stops if Tessera loses a cell.
//!
//! ```text
//! cargo run --release -- tessera <tool>
//! cargo run --release -- tessera show measurement
//! ```

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
use utilities::commands::{Command, Given, Parameter};
use utilities::diagnostics::table::report::Report;

/// How the tools are reached on the command line.
const CALLED: &str = "tilesim tessera";

/// Runs a tool that measures: it fills a report named as the tool,
/// which is printed and kept.
fn measuring(given: &Given, run: impl FnOnce(&mut Report, &Given) -> Result<(), String>) -> Result<(), String> {
    let mut report = Report::new(given.name(), &given.resolved());
    run(&mut report, given)?;
    transient_data::publish(report);
    Ok(())
}

/// A tool that measures and takes nothing.
macro_rules! measuring {
    ($run:path) => {
        |given| {
            measuring(given, |report, _| {
                $run(report);
                Ok(())
            })
        }
    };
}

/// Every tool.
pub const COMMANDS: [Command; 12] = [
    Command { name: "measurement", does: "bits a bitmap from every corpus generator, a table a generator, a row a parameter set; then what the trees hold", parameters: &[], run: measuring!(measurement::run) },
    Command { name: "census", does: "what Tessera's tree is made of, node kind by level, for each bitmap looked at", parameters: &[], run: measuring!(census::run) },
    Command { name: "per_shape", does: "Tessera's bits on every shape, plan and line set on its own", parameters: &[], run: measuring!(per_shape::run) },
    Command { name: "noise", does: "Tessera's bits on noise at several densities, against the raw cells", parameters: &[], run: measuring!(noise::run) },
    Command { name: "sparse", does: "the tree against the binary count tree on sparse bitmaps, beside the least scattered cells can take", parameters: &[], run: measuring!(sparse::run) },
    Command { name: "timing", does: "time to encode and decode a large corpus, family by family", parameters: &[Parameter::new(timing::PER_GENERATOR, "100")], run: |given| measuring(given, timing::run) },
    Command { name: "instruction_count", does: "instructions to encode and to decode a fixed corpus, counted by callgrind (needs valgrind)", parameters: &[], run: |given| measuring(given, instruction_count::run) },
    Command {
        name: instruction_count::CORPUS_TOOL,
        does: "nothing printed: encodes and decodes that corpus alone, uncounted -- what callgrind runs",
        parameters: &[],
        run: |_| {
            instruction_count::run_corpus();
            Ok(())
        },
    },
    Command {
        name: "render",
        does: "the bitmaps looked at, each written as a PNG image in transient_data/renders/",
        parameters: &[],
        run: |_| {
            render::run();
            Ok(())
        },
    },
    Command { name: "show", does: "the kept measurements, read back without measuring", parameters: &[Parameter::new(show::TOOL, "")], run: show::run },
    Command { name: "adversarial", does: "searches for the bitmaps Tessera does worst on against the raw cells, keeping the worst", parameters: &[Parameter::new(adversarial::CHANGES, "100")], run: adversarial::run },
    Command {
        name: "adversarial_save",
        does: "keeps a worst bitmap as a named bitmap no search replaces",
        parameters: &[Parameter::new(adversarial::FROM, ""), Parameter::new(adversarial::NAME, ""), Parameter::new(adversarial::DESCRIPTION, "")],
        run: adversarial::save,
    },
];

/// Runs the tool the first of `arguments` names, given the rest.
pub fn dispatch(arguments: &[&str]) -> Result<(), String> {
    utilities::commands::dispatch(CALLED, &COMMANDS, arguments)
}
