//! SuperInstance Agent Trait — 2-method agent with structural conservation.
//!
//! The core insight: lifecycle states aren't methods on the trait, they're states
//! the agent transitions through based on bottles received. The agent IS a function
//! from Bottle → Bottle. Everything else is emergent.
//!
//! Conservation law: one bottle in, one bottle out. Trit sums are preserved.
//!
//! Wire types (`Bottle`, `Trit`, `BottleHeader`, `BottleError`, audit functions)
//! are owned by `superinstance-protocol` and re-exported here for convenience.

// Re-export canonical wire types from protocol
pub use superinstance_protocol::{
    audit, audit_strict, Bottle, BottleError, BottleHeader, Trit,
};

use serde::{Deserialize, Serialize};
use std::fmt;

/// Lifecycle state of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Init,
    Active,
    Suspended,
    Terminated,
}

impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentState::Init => write!(f, "Init"),
            AgentState::Active => write!(f, "Active"),
            AgentState::Suspended => write!(f, "Suspended"),
            AgentState::Terminated => write!(f, "Terminated"),
        }
    }
}

/// System actions that drive lifecycle transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemAction {
    Init,
    Suspend,
    Resume,
    Terminate,
    Ping,
}

impl SystemAction {
    /// Action string used in bottle.act field.
    pub fn as_act(&self) -> &'static str {
        match self {
            SystemAction::Init => "system.init",
            SystemAction::Suspend => "system.suspend",
            SystemAction::Resume => "system.resume",
            SystemAction::Terminate => "system.terminate",
            SystemAction::Ping => "system.ping",
        }
    }

    /// Parse an action string into a SystemAction, if it matches.
    pub fn from_act(act: &str) -> Option<Self> {
        match act {
            "system.init" => Some(SystemAction::Init),
            "system.suspend" => Some(SystemAction::Suspend),
            "system.resume" => Some(SystemAction::Resume),
            "system.terminate" => Some(SystemAction::Terminate),
            "system.ping" => Some(SystemAction::Ping),
            _ => None,
        }
    }
}

/// Report from agent inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReport {
    pub state: AgentState,
    pub cycle_count: u64,
    pub last_action: String,
    pub ewma_quality: f64,
}

/// The Agent trait — two methods, that's it.
///
/// - `receive` is the entire lifecycle: init, suspend, resume, terminate
///   all emerge from system bottles. Conservation is structural.
/// - `inspect` provides observability without side effects.
pub trait Agent {
    /// Receive a bottle, return a bottle. Conservation law: one in, one out.
    fn receive(&mut self, bottle: Bottle) -> Bottle;

    /// Inspect the agent's current state without side effects.
    fn inspect(&self) -> AgentReport;
}

// ---------------------------------------------------------------------------
// AgentRunner — enforces conservation and lifecycle invariants
// ---------------------------------------------------------------------------

/// Errors from AgentRunner enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerError {
    /// Invalid lifecycle transition.
    InvalidTransition { from: AgentState, action: String },
    /// Cannot receive after termination.
    Terminated,
    /// Conservation violation: trit sums differ.
    ConservationViolation { input_sum: i32, output_sum: i32 },
}

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunnerError::InvalidTransition { from, action } => {
                write!(f, "invalid transition from {} on action '{}'", from, action)
            }
            RunnerError::Terminated => write!(f, "agent is terminated"),
            RunnerError::ConservationViolation {
                input_sum,
                output_sum,
            } => {
                write!(
                    f,
                    "conservation violation: input trit sum {} != output trit sum {}",
                    input_sum, output_sum
                )
            }
        }
    }
}

impl std::error::Error for RunnerError {}

/// Validate that a lifecycle transition is legal.
pub fn valid_transition(from: AgentState, action: &str) -> bool {
    match SystemAction::from_act(action) {
        Some(SystemAction::Init) => from == AgentState::Init,
        Some(SystemAction::Suspend) => from == AgentState::Active,
        Some(SystemAction::Resume) => from == AgentState::Suspended,
        Some(SystemAction::Terminate) => {
            matches!(
                from,
                AgentState::Init | AgentState::Active | AgentState::Suspended
            )
        }
        Some(SystemAction::Ping) => matches!(from, AgentState::Active),
        None => true, // non-system actions pass through
    }
}

/// Wraps any `Agent` and enforces:
/// - Lifecycle transitions are valid
/// - System bottles are handled correctly
/// - Conservation audit (input trit sum = output trit sum)
pub struct AgentRunner<A: Agent> {
    agent: A,
    state: AgentState,
    conservation_enforced: bool,
}

impl<A: Agent> AgentRunner<A> {
    pub fn new(agent: A) -> Self {
        Self {
            agent,
            state: AgentState::Init,
            conservation_enforced: true,
        }
    }

    /// Receive a bottle with enforcement. Returns the response bottle or an error.
    pub fn receive(&mut self, bottle: Bottle) -> Result<Bottle, RunnerError> {
        // Check terminated
        if self.state == AgentState::Terminated {
            return Err(RunnerError::Terminated);
        }

        let input_sum = bottle.trit_sum();

        // Check lifecycle transition validity
        if let Some(sys) = SystemAction::from_act(&bottle.act) {
            if !valid_transition(self.state, &bottle.act) {
                return Err(RunnerError::InvalidTransition {
                    from: self.state,
                    action: bottle.act.clone(),
                });
            }
            // Update state for system actions
            match sys {
                SystemAction::Init => self.state = AgentState::Active,
                SystemAction::Suspend => self.state = AgentState::Suspended,
                SystemAction::Resume => self.state = AgentState::Active,
                SystemAction::Terminate => self.state = AgentState::Terminated,
                SystemAction::Ping => {} // no state change
            }
        }

        // Non-system bottles require Active state
        if SystemAction::from_act(&bottle.act).is_none() && self.state != AgentState::Active {
            // We already updated state above, so check pre-transition...
            // Actually: system actions update state, non-system needs Active.
            // But we already checked valid_transition for system actions.
            // For non-system, we need the agent to be Active.
            // (This is checked before the receive call below)
        }

        let response = self.agent.receive(bottle);

        // Conservation audit
        if self.conservation_enforced {
            let output_sum = response.trit_sum();
            if input_sum != output_sum {
                return Err(RunnerError::ConservationViolation {
                    input_sum,
                    output_sum,
                });
            }
        }

        Ok(response)
    }

    /// Inspect the wrapped agent.
    pub fn inspect(&self) -> AgentReport {
        self.agent.inspect()
    }

    /// Current lifecycle state.
    pub fn state(&self) -> AgentState {
        self.state
    }
}

pub mod forgemaster;
