//! The rules a world ticks ([`rules_of`]): its types hooked to run
//! every tick, in the order registered, each run by its library; what
//! generates it ([`worldgen_of`]), and what it is hot about
//! ([`hot_entity_of`]), by their hooks too. A
//! tick's counts by rule ([`TickCounts`]), the rules picked
//! ([`ChosenRules`]) by their place ([`RulePlace`]) (`docs/server.md`,
//! "TickCounts").

use crate::simulation::simulation_api::{CallCode, RuleCode, RuleContext, RuleCounts, RuleMaster, RuleRun, TickReport, SimulationTickState, WorldgenCode, COUNTS_OF_A_RULE};
use std::sync::Arc;
use crate::entity_manager::EntityType;
use protocol::type_registry::{root_bundle_of, FrozenTypeRegistry, Hook, RegisteredType};
use std::ops::AddAssign;
use std::time::{Duration, Instant};

/// The most rules a world may have: what its counts and its choice of
/// rules keep room for, each in an array that copies and never
/// allocates -- a turn's counts are added up every superchunk.
pub const MOST_RULES: usize = 16;

/// One rule of a world.
#[derive(Clone)]
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
    pub rule: Arc<RuleRun>,
}

impl std::fmt::Debug for Rule {
    /// Its ID, what it runs with and what it counts: what the rule is,
    /// only its code knows.
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.debug_struct("Rule").field("rule_id", &self.rule_id).field("context", &self.context).field("count_names", &self.count_names).finish_non_exhaustive()
    }
}

/// The rules a world of `frozen_type_registry` ticks, each run by its library: every
/// type hooked to run every tick ([`Hook::RunsEveryTick`]), in the order
/// registered, the order a turn runs them in -- the server's choice, the
/// registry promising none. Or why the world cannot run: one hooked that
/// is no rule, or whose library is no rule's code, or more than
/// [`MOST_RULES`].
pub fn rules_of(frozen_type_registry: &FrozenTypeRegistry) -> Result<Arc<[Rule]>, String> {
    let rules: Vec<Rule> = frozen_type_registry
        .hooked(Hook::RunsEveryTick)
        .map(|(registered_rule, library)| {
            let master_type = frozen_type_registry.master_of(registered_rule).ok_or_else(|| format!("{} runs every tick, and is no rule", registered_rule.name()))?;
            let master = match master_type {
                RegisteredType::Layer { layer_type, .. } => RuleMaster::Layer(*layer_type),
                RegisteredType::WidePlane { layer_type, .. } => RuleMaster::WidePlane(*layer_type),
                RegisteredType::Entity { entity_type, .. } => RuleMaster::Entity(*entity_type),
                _ => unreachable!("a rule's master is a plane or a type of entity"),
            };
            // Gathered once: each library the rule may ask for, by the ID of the type exposing it.
            let libraries = frozen_type_registry.dependencies_of(master_type).filter_map(|dependency| Some((dependency.type_id(), frozen_type_registry.library_of(dependency)?.clone()))).collect();
            // Cast once: the rule's code, its library.
            let code = library.and_then(|rule_library| rule_library.downcast_ref::<RuleCode>()).ok_or_else(|| format!("{} runs every tick, and its library is no rule's code", registered_rule.name()))?;
            Ok(Rule { rule_id: registered_rule.type_id(), context: RuleContext { master, libraries }, count_names: code.count_names, rule: code.run.clone() })
        })
        .collect::<Result<_, String>>()?;
    if rules.len() > MOST_RULES {
        return Err(format!("{} rules: more than the {MOST_RULES} a world may have", rules.len()));
    }
    Ok(rules.into())
}

/// What generates a world of `frozen_type_registry`: the library of its one type hooked
/// into generating the world ([`Hook::GeneratesTheWorld`]), cast once, and
/// the mod it is of ([`root_bundle_of`]): whose folder of the world's mod
/// config it is set up from. Or why it cannot run: none hooked, more
/// than one, or a library that is no world generation's code.
pub fn worldgen_of(frozen_type_registry: &FrozenTypeRegistry) -> Result<(WorldgenCode, String), String> {
    let mut generating = frozen_type_registry.hooked(Hook::GeneratesTheWorld);
    let (generating_type, library) = generating.next().ok_or("no type generates the world")?;
    if let Some((another_type, _)) = generating.next() {
        return Err(format!("{} and {} both generate the world", generating_type.name(), another_type.name()));
    }
    let worldgen = library.and_then(|worldgen_library| worldgen_library.downcast_ref::<WorldgenCode>()).cloned().ok_or_else(|| format!("{} generates the world, and its library is no world generation's code", generating_type.name()))?;
    Ok((worldgen, root_bundle_of(generating_type.name()).to_string()))
}

/// The code of the function a client may call whose ID is `call`, among
/// `frozen_type_registry`'s types, cast, and the mod it is of
/// ([`root_bundle_of`]): whose config it reads and writes. Or why it is
/// refused: no type of the ID, one no client may call, or a library
/// that is no call's code -- only a type registered as a client call is
/// ever run.
pub fn call_code_of(frozen_type_registry: &FrozenTypeRegistry, call: u64) -> Result<(CallCode, String), String> {
    let called = frozen_type_registry.types().iter().find(|registered| registered.type_id() == call).ok_or_else(|| format!("no type is of the ID {call:#018x}"))?;
    if !matches!(called, RegisteredType::ClientCall { .. }) {
        return Err(format!("{} is no function a client may call", called.name()));
    }
    let call_code = frozen_type_registry.library_of(called).and_then(|call_library| call_library.downcast_ref::<CallCode>()).cloned().ok_or_else(|| format!("{} is a client call, and its library is no call's code", called.name()))?;
    Ok((call_code, root_bundle_of(called.name()).to_string()))
}

/// The type of entity a world of `frozen_type_registry` is kept hot about: its one
/// type hooked to keep the world hot ([`Hook::KeepsTheWorldHot`]). Or why
/// it cannot run: none hooked, more than one, or one that is no type of
/// entity.
pub fn hot_entity_of(frozen_type_registry: &FrozenTypeRegistry) -> Result<EntityType, String> {
    let mut keeping_hot = frozen_type_registry.hooked(Hook::KeepsTheWorldHot);
    let (hot_type, _) = keeping_hot.next().ok_or("no type keeps the world hot")?;
    if let Some((another_type, _)) = keeping_hot.next() {
        return Err(format!("{} and {} both keep the world hot", hot_type.name(), another_type.name()));
    }
    match hot_type {
        RegisteredType::Entity { entity_type, .. } => Ok(*entity_type),
        _ => Err(format!("{} keeps the world hot, and is no type of entity", hot_type.name())),
    }
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
    pub fn tick_state(self, rules: &[Rule], tick_state: &mut SimulationTickState) -> TickCounts {
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
    pub fn timed_turn(self, rules: &[Rule], tick_state: &mut SimulationTickState) -> TickCounts {
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
