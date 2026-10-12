//! The rules a world ticks ([`RULES`]): those the type registry has,
//! in its order, each with the function written for it here. A tick's counts by rule
//! ([`TickCounts`]), the rules picked ([`Chosen`]) by their place ([`RulePlace`])
//! (`docs/server.md`, "TickCounts").

use coordinates::CellIndex;
use actions::{RuleCounts, TickReport, Turn, COUNTS_OF_A_RULE};
use std::ops::AddAssign;
use std::time::{Duration, Instant};

/// A rule, on one superchunk's turn, with room for samples.
type RuleFunction = fn(&mut Turn, &mut Vec<CellIndex>) -> RuleCounts;

/// One rule of the world.
#[derive(Clone, Copy)]
pub struct Rule {
    /// Its row in the type registry: its name there, and its master.
    pub registered: &'static type_registry::Registered,
    /// What it counts, each named at its place in its counts.
    pub counted: &'static [&'static str],
    /// The rule, on one superchunk's turn, with room for samples.
    pub rule: RuleFunction,
}

/// What the registry cannot hold of the rule whose ID is `id`, the
/// code being above it: what it counts and its function. None of an
/// ID no rule here is written for -- and the build fails
/// ([`RULES`]) if that is a rule the registry has.
const fn written_for(id: u64) -> Option<(&'static [&'static str], RuleFunction)> {
    if id == type_registry::RULE_OF_GRASS.0 {
        Some((&sca_rules::grass::COUNTED, sca_rules::grass::rule))
    } else if id == type_registry::RULE_OF_TREES.0 {
        Some((&sca_rules::trees::COUNTED, sca_rules::trees::rule))
    } else if id == type_registry::RULE_OF_SHEEP.0 {
        Some((&entity_rules::sheep::COUNTED, |turn, _| entity_rules::sheep::rule(turn)))
    } else if id == type_registry::RULE_OF_WEATHER.0 {
        Some((&sca_rules::weather::COUNTED, sca_rules::weather::rule))
    } else {
        None
    }
}

/// The rule the registry has at `place` among its rules, with what is
/// written for it here.
///
/// # Panics
/// If the registry has no rule there, or one nothing is written for
/// -- as the server is compiled, naming it.
const fn registered_at(place: usize) -> Rule {
    let Some(registered) = type_registry::rule_at(place) else { panic!("fewer rules registered than counted") };
    let Some(id) = type_registry::id_of_rule(registered) else { panic!("a rule's row") };
    match written_for(id) {
        Some((counted, rule)) => Rule { registered, counted, rule },
        None => panic!("{}", registered.name),
    }
}

/// Every rule a world ticks: those the type registry has, as many and
/// in its order -- the order a turn runs them in, the cells'
/// (`sca_rules`) and the entities' (`entity_rules`) alike, all
/// reading the world as the tick found it. Nothing is listed here: a
/// rule registered is ticked, and the build fails, naming it, if no
/// function is written for it ([`written_for`]).
pub const RULES: [Rule; type_registry::RULES_REGISTERED] = {
    let mut rules = [registered_at(0); type_registry::RULES_REGISTERED];
    let mut place = 1;
    while place < rules.len() {
        rules[place] = registered_at(place);
        place += 1;
    }
    rules
};

impl Rule {
    /// Its master's row in the type registry: the one plane or type of
    /// entity it is of, whatever else it reads or writes.
    pub fn master(&self) -> &'static type_registry::Registered {
        type_registry::master_of(self.registered).expect("checked as the registry is built")
    }
}

/// A rule's place in [`RULES`]: what a rule is picked by, and its
/// counts read by. Made of the rule's ID in the type registry as the
/// server is compiled ([`RulePlace::of`]), so a rule that is not
/// there does not compile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RulePlace(usize);

impl RulePlace {
    /// The place of the rule whose ID in the type registry is `id`.
    ///
    /// # Panics
    /// If no rule has it -- for a constant, as it is compiled.
    pub const fn of(id: u64) -> Self {
        let mut place = 0;
        while place < RULES.len() {
            if let Some(registered) = type_registry::id_of_rule(RULES[place].registered) && registered == id {
                return Self(place);
            }
            place += 1;
        }
        panic!("no rule is registered with that ID")
    }

    /// The rule at this place.
    pub const fn rule(self) -> &'static Rule {
        &RULES[self.0]
    }
}

/// The grass's rule.
pub const GRASS_RULE: RulePlace = RulePlace::of(type_registry::RULE_OF_GRASS.0);
/// The trees' rule.
pub const TREES_RULE: RulePlace = RulePlace::of(type_registry::RULE_OF_TREES.0);
/// The sheep's rule.
pub const SHEEP_RULE: RulePlace = RulePlace::of(type_registry::RULE_OF_SHEEP.0);
/// The weather's rule.
pub const WEATHER_RULE: RulePlace = RulePlace::of(type_registry::RULE_OF_WEATHER.0);

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
