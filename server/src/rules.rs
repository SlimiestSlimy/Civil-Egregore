//! The rules a world ticks ([`rules_of`]): those its types have, in
//! the order registered, each run by the code registered with it. A
//! tick's counts by rule ([`TickCounts`]), the rules picked
//! ([`ChosenRules`]) by their place ([`RulePlace`]) (`docs/server.md`,
//! "TickCounts").

use simulation::simulation_api::{RuleCode, RuleContext, RuleCounts, RuleMaster, TickReport, SuperchunkTickState, COUNTS_OF_A_RULE};
use std::sync::Arc;
use type_registry::{FrozenTypes, RegisteredType};
use std::ops::AddAssign;
use std::time::{Duration, Instant};

/// The most rules a world may have: what its counts and its choice of
/// rules keep room for, each in an array that copies and never
/// allocates -- a turn's counts are added up every superchunk.
pub const MOST_RULES: usize = 16;

/// One rule of a world.
#[derive(Clone, Debug)]
pub struct Rule {
    /// Its type's ID, whole: what the server knows it by.
    pub rule_id: u64,
    /// What it runs with: the one plane or type of entity it is of,
    /// whatever else it reads or writes, and the libraries of what that
    /// depends on.
    pub context: RuleContext,
    /// What it counts, each named at its place in its counts.
    pub count_names: &'static [&'static str],
    /// The rule, on one superchunk's turn.
    pub rule: fn(&mut SuperchunkTickState, &RuleContext) -> RuleCounts,
}

/// The rules a world of `frozen_types` ticks, each run by its code:
/// every rule the types have, in the order registered, the order a turn
/// runs them in -- the server's choice, the registry promising none. Or
/// why the world cannot run: more than [`MOST_RULES`].
pub fn rules_of(frozen_types: &FrozenTypes<RuleCode>) -> Result<Arc<[Rule]>, String> {
    let rules: Vec<Rule> = frozen_types
        .rules()
        .map(|registered_rule| {
            let master_type = frozen_types.master_of(registered_rule).expect("a rule's master is checked as it is registered");
            let master = match master_type {
                RegisteredType::Layer { layer_type, .. } => RuleMaster::Layer(*layer_type),
                RegisteredType::WidePlane { layer_type, .. } => RuleMaster::WidePlane(*layer_type),
                RegisteredType::Entity { entity_type, .. } => RuleMaster::Entity(*entity_type),
                _ => unreachable!("a rule's master is a plane or a type of entity"),
            };
            // Gathered once: each library the rule may ask for, by the ID of the type exposing it.
            let libraries = frozen_types.dependencies_of(master_type).filter_map(|dependency| Some((dependency.type_id(), *frozen_types.library_of(dependency)?))).collect();
            let code = registered_rule.rule_code().expect("the types' rules are rules");
            Rule { rule_id: registered_rule.type_id(), context: RuleContext { master, libraries }, count_names: code.count_names, rule: code.run }
        })
        .collect();
    if rules.len() > MOST_RULES {
        return Err(format!("{} rules: more than the {MOST_RULES} a world may have", rules.len()));
    }
    Ok(rules.into())
}

/// A rule's place among a world's rules: what a rule is picked by, and
/// its counts read by ([`place_of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RulePlace(usize);

/// The place among `rules` of the rule whose type's ID is `rule_id`,
/// if one is.
pub fn place_of(rules: &[Rule], rule_id: u64) -> Option<RulePlace> {
    rules.iter().position(|rule| rule.rule_id == rule_id).map(RulePlace)
}

/// What the rules did, in a tick or added up over many: each rule's
/// counts at its place among the world's rules, and how long each took, every
/// thread's time added up -- none, unless the turns were timed
/// ([`ChosenRules::timed_turn`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickCounts {
    /// Each rule's counts.
    pub counts: [RuleCounts; MOST_RULES],
    /// Each rule's time.
    pub rule_times: [Duration; MOST_RULES],
}

impl TickCounts {
    /// The counts of `rule`, each at the place the rule names it by.
    pub fn counts_of(&self, rule: RulePlace) -> RuleCounts {
        self.counts[rule.0]
    }

    /// The time of `rule`.
    pub fn time_of(&self, rule: RulePlace) -> Duration {
        self.rule_times[rule.0]
    }
}

/// `report`, each rule's counts with what the tick counted for it as
/// it applied ([`ChosenRules::turn`] gives each rule its numbers): a write
/// refused, or a group, is counted only if it happened
/// (`docs/server.md`, "TickCounts").
pub(crate) fn with_counts_applied(mut report: TickReport<TickCounts>) -> TickReport<TickCounts> {
    for (place, counts) in report.rules.counts.iter_mut().enumerate() {
        for (count, applied_count) in counts.0.iter_mut().zip(&report.counted_when_applied[place * COUNTS_OF_A_RULE..]) {
            *count += applied_count;
        }
    }
    report
}

impl AddAssign for TickCounts {
    /// Every rule's counts and time added to its like.
    fn add_assign(&mut self, added_counts: Self) {
        for place in 0..MOST_RULES {
            self.counts[place] += added_counts.counts[place];
            self.rule_times[place] += added_counts.rule_times[place];
        }
    }
}

/// Some of the rules: what a tick of only those runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChosenRules {
    /// Whether the rule at each place among the world's rules is run.
    runs_rule: [bool; MOST_RULES],
}

impl ChosenRules {
    /// Every rule: what a world's tick runs.
    pub const EVERY_RULE: Self = Self { runs_rule: [true; MOST_RULES] };

    /// The rules `rules`, run in the table's order whatever theirs.
    pub fn from_rules(rules: &[RulePlace]) -> Self {
        let mut runs_rule = [false; MOST_RULES];
        for rule in rules {
            runs_rule[rule.0] = true;
        }
        Self { runs_rule }
    }

    /// One superchunk's turn of these of `rules`: what `Simulation::tick`
    /// is given.
    #[inline]
    pub fn tick_state(self, rules: &[Rule], tick_state: &mut SuperchunkTickState) -> TickCounts {
        let mut turn_counts = TickCounts::default();
        for (place, rule) in rules.iter().enumerate() {
            if self.runs_rule[place] {
                tick_state.count_under((place * COUNTS_OF_A_RULE) as u32);
                turn_counts.counts[place] = (rule.rule)(tick_state, &rule.context);
            }
        }
        turn_counts
    }

    /// One superchunk's turn of these of `rules`, each one's time taken.
    pub fn timed_turn(self, rules: &[Rule], tick_state: &mut SuperchunkTickState) -> TickCounts {
        let mut turn_counts = TickCounts::default();
        for (place, rule) in rules.iter().enumerate() {
            if self.runs_rule[place] {
                let rule_began = Instant::now();
                tick_state.count_under((place * COUNTS_OF_A_RULE) as u32);
                turn_counts.counts[place] = (rule.rule)(tick_state, &rule.context);
                turn_counts.rule_times[place] = rule_began.elapsed();
            }
        }
        turn_counts
    }
}
