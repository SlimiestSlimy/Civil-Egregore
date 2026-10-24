//! The server's public API, as simulation_api: a server bundle's start
//! gets the frozen registry and the server handle, and adds rules.

pub use engine_common;
use std::sync::atomic::{AtomicU64, Ordering};

/// Ticks run, counted by the engine: a static mods read.
pub static TICKS_RUN: AtomicU64 = AtomicU64::new(0);

/// One tick as a rule sees it.
pub struct TickState {
    pub tick: u64,
    pub said: Vec<String>,
}

/// A rule: code for one tick.
pub type Rule = Box<dyn Fn(&mut TickState) + Send + Sync>;

/// What a server bundle's start is handed: where its rules go.
#[derive(Default)]
pub struct ServerHandle {
    pub rules: Vec<Rule>,
}

impl ServerHandle {
    pub fn add_rule(&mut self, rule: impl Fn(&mut TickState) + Send + Sync + 'static) {
        self.rules.push(Box::new(rule));
    }

    /// One tick: every rule, in order; what they said.
    pub fn tick(&self) -> Vec<String> {
        let mut state = TickState { tick: TICKS_RUN.fetch_add(1, Ordering::Relaxed), said: Vec::new() };
        for rule in &self.rules {
            rule(&mut state);
        }
        state.said
    }
}

/// A server bundle's start.
pub type ServerStart = fn(&engine_common::Registry, &mut ServerHandle) -> Result<(), String>;
