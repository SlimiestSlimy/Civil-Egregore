//! The rules a world ticks, in one table ([`RULES`]): a rule added is
//! a row added here, and nowhere else. A tick's counts by rule
//! ([`TickCounts`]), the rules picked ([`Chosen`]) by their place ([`RulePlace`])
//! (`docs/server.md`, "TickCounts").

use coordinates::CellIndex;
use instructions::{RuleCounts, TickReport, Turn, COUNTS_OF_A_RULE};
use std::ops::AddAssign;
use std::time::{Duration, Instant};

/// One rule of the world.
pub struct Rule {
    /// Its name: what it is picked by.
    pub name: &'static str,
    /// What it counts, each named at its place in its counts.
    pub counted: &'static [&'static str],
    /// The rule, on one superchunk's turn, with room for samples.
    pub rule: fn(&mut Turn, &mut Vec<CellIndex>) -> RuleCounts,
}

/// Every rule a world ticks, in the order a turn runs them: the cells'
/// (`sca_rules`), then the entities' (`entity_rules`), all reading the
/// world as the tick found it.
pub const RULES: [Rule; 3] = [
    Rule { name: "grass", counted: &sca_rules::grass::COUNTED, rule: sca_rules::grass::rule },
    Rule { name: "trees", counted: &sca_rules::trees::COUNTED, rule: sca_rules::trees::rule },
    Rule { name: "sheep", counted: &entity_rules::sheep::COUNTED, rule: |turn, _| entity_rules::sheep::rule(turn) },
];

/// A rule's place in [`RULES`]: what a rule is picked by, and its
/// counts read by. Made of a name as the server is compiled
/// ([`RulePlace::named`]), so a rule that is not there does not compile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RulePlace(usize);

impl RulePlace {
    /// The place of the rule named `name`.
    ///
    /// # Panics
    /// If no rule is named so -- for a constant, as it is compiled.
    pub const fn named(name: &str) -> Self {
        let names = {
            let mut names = [""; RULES.len()];
            let mut place = 0;
            while place < RULES.len() {
                names[place] = RULES[place].name;
                place += 1;
            }
            names
        };
        Self(instructions::place_counted(&names, name))
    }

    /// The rule at this place.
    pub const fn rule(self) -> &'static Rule {
        &RULES[self.0]
    }
}

/// The grass's rule.
pub const GRASS_RULE: RulePlace = RulePlace::named("grass");
/// The trees' rule.
pub const TREES_RULE: RulePlace = RulePlace::named("trees");
/// The sheep's rule.
pub const SHEEP_RULE: RulePlace = RulePlace::named("sheep");

/// What the rules did, in a tick or added up over many: each rule's
/// counts at its place in [`RULES`], and how long each took, every
/// thread's time added up -- none, unless the turns were timed
/// ([`Chosen::timed_turn`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickCounts {
    /// Each rule's counts.
    pub counts: [RuleCounts; RULES.len()],
    /// Each rule's time.
    pub times: [Duration; RULES.len()],
}

impl TickCounts {
    /// The counts of `rule`, each at the place the rule names it by.
    pub fn of(&self, rule: RulePlace) -> RuleCounts {
        self.counts[rule.0]
    }

    /// The time of `rule`.
    pub fn time_of(&self, rule: RulePlace) -> Duration {
        self.times[rule.0]
    }
}

/// `report`, each rule's counts with what the tick counted for it as
/// it applied ([`Chosen::turn`] gives each rule its numbers): a write
/// refused, or a group, is counted only if it happened
/// (`docs/server.md`, "TickCounts").
pub(crate) fn with_counts_applied(mut report: TickReport<TickCounts>) -> TickReport<TickCounts> {
    for (place, counts) in report.rules.counts.iter_mut().enumerate() {
        for (count, applied) in counts.0.iter_mut().zip(&report.counted_when_applied[place * COUNTS_OF_A_RULE..]) {
            *count += applied;
        }
    }
    report
}

impl AddAssign for TickCounts {
    /// Every rule's counts and time added to its like.
    fn add_assign(&mut self, other: Self) {
        for place in 0..RULES.len() {
            self.counts[place] += other.counts[place];
            self.times[place] += other.times[place];
        }
    }
}

/// Some of the rules: what a tick of only those runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chosen {
    /// Whether the rule at each place in [`RULES`] is run.
    run: [bool; RULES.len()],
}

impl Chosen {
    /// Every rule: what a world's tick runs.
    pub const ALL: Self = Self { run: [true; RULES.len()] };

    /// The rules `rules`, run in the table's order whatever theirs.
    pub fn of(rules: &[RulePlace]) -> Self {
        let mut run = [false; RULES.len()];
        for rule in rules {
            run[rule.0] = true;
        }
        Self { run }
    }

    /// One superchunk's turn of these rules: what `Simulation::tick`
    /// is given.
    #[inline]
    pub fn turn(self, turn: &mut Turn, samples: &mut Vec<CellIndex>) -> TickCounts {
        let mut done = TickCounts::default();
        for (place, rule) in RULES.iter().enumerate() {
            if self.run[place] {
                turn.count_under((place * COUNTS_OF_A_RULE) as u32);
                done.counts[place] = (rule.rule)(turn, samples);
            }
        }
        done
    }

    /// One superchunk's turn of these rules, each one's time taken.
    pub fn timed_turn(self, turn: &mut Turn, samples: &mut Vec<CellIndex>) -> TickCounts {
        let mut done = TickCounts::default();
        for (place, rule) in RULES.iter().enumerate() {
            if self.run[place] {
                let start = Instant::now();
                turn.count_under((place * COUNTS_OF_A_RULE) as u32);
                done.counts[place] = (rule.rule)(turn, samples);
                done.times[place] = start.elapsed();
            }
        }
        done
    }
}
