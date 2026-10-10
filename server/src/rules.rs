//! The rules a world ticks, in one table ([`RULES`]): a rule added is
//! a row added here, and nowhere else. A tick's counts by rule
//! ([`TickCounts`]), the rules picked by name ([`Chosen`])
//! (`docs/server.md`, "TickCounts").

use coordinates::CellIndex;
use instructions::{RuleCounts, Turn};
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
/// (`mc_rules`), then the entities' (`entity_rules`), all reading the
/// world as the tick found it.
pub const RULES: [Rule; 3] = [
    Rule { name: "grass", counted: &mc_rules::grass::COUNTED, rule: mc_rules::grass::rule },
    Rule { name: "trees", counted: &mc_rules::trees::COUNTED, rule: mc_rules::trees::rule },
    Rule { name: "sheep", counted: &entity_rules::sheep::COUNTED, rule: |turn, _| entity_rules::sheep::rule(turn) },
];

/// The place in [`RULES`] of the rule named `name`.
///
/// # Panics
/// If no rule is named so.
pub fn place_of(name: &str) -> usize {
    RULES.iter().position(|rule| rule.name == name).unwrap_or_else(|| panic!("no rule is named `{name}`"))
}

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
    /// The counts of the rule named `rule`.
    pub fn of(&self, rule: &str) -> RuleCounts {
        self.counts[place_of(rule)]
    }

    /// The time of the rule named `rule`.
    pub fn time_of(&self, rule: &str) -> Duration {
        self.times[place_of(rule)]
    }

    /// What the rule named `rule` counted under `counted`.
    ///
    /// # Panics
    /// If the rule counts nothing named so.
    pub fn count(&self, rule: &str, counted: &str) -> u64 {
        let place = place_of(rule);
        let at = RULES[place].counted.iter().position(|&name| name == counted).unwrap_or_else(|| panic!("the rule `{rule}` counts nothing named `{counted}`"));
        self.counts[place][at]
    }
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

/// Some of the rules, picked by name: what a tick of only those runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chosen {
    /// Whether the rule at each place in [`RULES`] is run.
    run: [bool; RULES.len()],
}

impl Chosen {
    /// Every rule: what a world's tick runs.
    pub const ALL: Self = Self { run: [true; RULES.len()] };

    /// The rules named in `names`, run in the table's order whatever
    /// theirs.
    ///
    /// # Panics
    /// If no rule is named as one of them.
    pub fn named(names: &[&str]) -> Self {
        let mut run = [false; RULES.len()];
        for name in names {
            run[place_of(name)] = true;
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
                done.counts[place] = (rule.rule)(turn, samples);
                done.times[place] = start.elapsed();
            }
        }
        done
    }
}
