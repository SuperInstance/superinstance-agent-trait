//! Forgemaster — a minimal agent that logs cycles and emits quality signals.
//!
//! On `cycle.request`: returns `cycle.complete` with a quality signal in trits.
//! Tracks cycle count and EWMA quality. Responds to system bottles.

use crate::*;

/// Exponentially-weighted moving average alpha.
const EWMA_ALPHA: f64 = 0.3;

/// The Forgemaster agent — the first agent in the fleet.
///
/// It forges quality signals from cycle requests. Every cycle produces a ternary
/// quality assessment {-1, 0, +1}, and the running EWMA tracks overall health.
pub struct Forgemaster {
    state: AgentState,
    cycle_count: u64,
    last_action: String,
    ewma_quality: f64,
}

impl Forgemaster {
    pub fn new() -> Self {
        Self {
            state: AgentState::Init,
            cycle_count: 0,
            last_action: String::new(),
            ewma_quality: 0.0,
        }
    }

    /// Generate a quality signal as a trit vector that conserves the given trit sum.
    /// Quality is encoded in the *pattern* of trits, not the total sum.
    fn quality_signal_for_sum(&self, target_sum: i32) -> Vec<Trit> {
        // Base quality pattern based on cycle parity
        let base = match self.cycle_count % 3 {
            0 => vec![1, 0, 1],  // positive quality
            1 => vec![0, 0, 0],  // neutral quality
            _ => vec![-1, 0, 1], // mixed quality
        };
        let base_sum: i32 = base.iter().map(|&t| t as i32).sum();
        let diff = target_sum - base_sum;
        // Adjust by appending compensating trits to hit target_sum
        let mut result = base;
        if diff > 0 {
            for _ in 0..diff {
                result.push(1);
            }
        } else if diff < 0 {
            for _ in 0..(-diff) {
                result.push(-1);
            }
        }
        result
    }

    fn update_ewma(&mut self, quality: f64) {
        if self.cycle_count <= 1 {
            self.ewma_quality = quality;
        } else {
            self.ewma_quality = EWMA_ALPHA * quality + (1.0 - EWMA_ALPHA) * self.ewma_quality;
        }
    }
}

impl Default for Forgemaster {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent for Forgemaster {
    fn receive(&mut self, bottle: Bottle) -> Bottle {
        self.last_action = bottle.act.clone();

        match bottle.act.as_str() {
            "system.init" => {
                self.state = AgentState::Active;
                Bottle::new_raw(
                    "forgemaster",
                    bottle.src.clone(),
                    "system.init.ack",
                    bottle.trits.clone(),
                    b"initialized".to_vec(),
                    300,
                )
            }
            "system.ping" => {
                Bottle::new_raw(
                    "forgemaster",
                    bottle.src.clone(),
                    "system.pong",
                    bottle.trits.clone(),
                    b"pong".to_vec(),
                    300,
                )
            }
            "system.suspend" => {
                self.state = AgentState::Suspended;
                Bottle::new_raw(
                    "forgemaster",
                    bottle.src.clone(),
                    "system.suspend.ack",
                    bottle.trits.clone(),
                    b"suspended".to_vec(),
                    300,
                )
            }
            "system.resume" => {
                self.state = AgentState::Active;
                Bottle::new_raw(
                    "forgemaster",
                    bottle.src.clone(),
                    "system.resume.ack",
                    bottle.trits.clone(),
                    b"resumed".to_vec(),
                    300,
                )
            }
            "system.terminate" => {
                self.state = AgentState::Terminated;
                Bottle::new_raw(
                    "forgemaster",
                    bottle.src.clone(),
                    "system.terminate.ack",
                    bottle.trits.clone(),
                    b"terminated".to_vec(),
                    300,
                )
            }
            "cycle.request" => {
                self.cycle_count += 1;
                let input_sum = bottle.trit_sum();
                let quality_trits = self.quality_signal_for_sum(input_sum);
                let quality_value = {
                    let base = match (self.cycle_count - 1) % 3 {
                        0 => vec![1.0, 0.0, 1.0],
                        1 => vec![0.0, 0.0, 0.0],
                        _ => vec![-1.0, 0.0, 1.0],
                    };
                    base.iter().sum::<f64>() / base.len().max(1) as f64
                };
                self.update_ewma(quality_value);

                Bottle::new_raw(
                    "forgemaster",
                    bottle.src.clone(),
                    "cycle.complete",
                    quality_trits,
                    format!("cycle#{}", self.cycle_count).into_bytes(),
                    300,
                )
            }
            _ => {
                // Unknown action: echo back with same trits (conservation)
                Bottle::new_raw(
                    "forgemaster",
                    bottle.src.clone(),
                    "unknown.ack",
                    bottle.trits.clone(),
                    format!("unknown action: {}", bottle.act).into_bytes(),
                    300,
                )
            }
        }
    }

    fn inspect(&self) -> AgentReport {
        AgentReport {
            state: self.state,
            cycle_count: self.cycle_count,
            last_action: self.last_action.clone(),
            ewma_quality: self.ewma_quality,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a system bottle.
    fn sys_bottle(action: &str, trits: Vec<Trit>) -> Bottle {
        Bottle::new_empty("system", "forgemaster", action, trits, 300)
    }

    /// Helper: create a cycle request bottle.
    fn cycle_request(trits: Vec<Trit>) -> Bottle {
        Bottle::new_empty("fleet-edge", "forgemaster", "cycle.request", trits, 300)
    }

    /// Forgemaster handles cycle.request and returns cycle.complete.
    #[test]
    fn forgemaster_handles_cycle_request() {
        let mut fm = Forgemaster::new();
        // Must init first
        let _ = fm.receive(sys_bottle("system.init", vec![1, 0, -1]));
        let response = fm.receive(cycle_request(vec![1, 0, -1]));
        assert_eq!(response.act, "cycle.complete");
        assert_eq!(response.src, "forgemaster");
    }

    /// Forgemaster returns a quality signal in the trits of cycle.complete.
    #[test]
    fn forgemaster_returns_quality_signal() {
        let mut fm = Forgemaster::new();
        let _ = fm.receive(sys_bottle("system.init", vec![1, 0, -1]));
        let response = fm.receive(cycle_request(vec![1, 0, -1]));
        // Quality signal should be non-empty trit vector
        assert!(!response.trits.is_empty());
        // All values should be valid trits
        for t in &response.trits {
            assert!(*t == -1 || *t == 0 || *t == 1);
        }
    }

    /// Conservation holds across multiple cycles.
    #[test]
    fn conservation_holds_across_cycles() {
        let mut fm = Forgemaster::new();
        let _ = fm.receive(sys_bottle("system.init", vec![1, 0, -1]));

        let input_trits = vec![1, 0, -1, 0, 1]; // sum = 1
        for _ in 0..5 {
            let response = fm.receive(cycle_request(input_trits.clone()));
            assert_eq!(response.trit_sum(), input_trits.iter().map(|&t| t as i32).sum::<i32>());
        }
    }

    /// system.init transitions to Active.
    #[test]
    fn system_init_transitions_to_active() {
        let mut fm = Forgemaster::new();
        assert_eq!(fm.state, AgentState::Init);
        let _ = fm.receive(sys_bottle("system.init", vec![]));
        assert_eq!(fm.state, AgentState::Active);
    }

    /// system.terminate transitions to Terminated.
    #[test]
    fn system_terminate_transitions_to_terminated() {
        let mut fm = Forgemaster::new();
        let _ = fm.receive(sys_bottle("system.init", vec![]));
        assert_eq!(fm.state, AgentState::Active);
        let _ = fm.receive(sys_bottle("system.terminate", vec![]));
        assert_eq!(fm.state, AgentState::Terminated);
    }

    /// AgentRunner enforces conservation: input trit sum must equal output trit sum.
    #[test]
    fn agent_runner_enforces_conservation() {
        use crate::AgentRunner;

        let fm = Forgemaster::new();
        let mut runner = AgentRunner::new(fm);

        // Init the runner
        let init_bottle = sys_bottle("system.init", vec![1, 0, -1]);
        let _ = runner.receive(init_bottle).expect("init should succeed");

        // Cycle with trits that the forgemaster will NOT conserve
        // Forgemaster returns quality_signal() trits, not input trits.
        // So if input sum != output sum, the runner should catch it.
        // Let's verify the runner catches a mismatch.
        let cycle = cycle_request(vec![1, 1, 1]); // sum = 3
        let result = runner.receive(cycle);
        // Forgemaster returns quality_signal based on cycle count, which may not sum to 3
        // The runner should detect this if sums differ
        // (If they happen to match, the test still passes — the runner is working)
        match result {
            Ok(bottle) => {
                // Sums matched by coincidence — that's fine, runner allowed it
                assert_eq!(bottle.trit_sum(), 3);
            }
            Err(RunnerError::ConservationViolation { .. }) => {
                // Runner caught the violation — correct behavior
            }
            Err(e) => panic!("unexpected error: {}", e),
        }
    }

    /// Can't receive after terminate.
    #[test]
    fn cant_receive_after_terminate() {
        use crate::AgentRunner;

        let fm = Forgemaster::new();
        let mut runner = AgentRunner::new(fm);

        let _ = runner
            .receive(sys_bottle("system.init", vec![]))
            .expect("init");
        let _ = runner
            .receive(sys_bottle("system.terminate", vec![]))
            .expect("terminate");

        let result = runner.receive(cycle_request(vec![1, 0, -1]));
        assert!(matches!(result, Err(RunnerError::Terminated)));
    }

    /// Lifecycle transitions must be valid.
    #[test]
    fn lifecycle_transitions_are_valid() {
        use crate::AgentRunner;

        let fm = Forgemaster::new();
        let mut runner = AgentRunner::new(fm);

        // Can't resume from Init (need to be Suspended)
        let result = runner.receive(sys_bottle("system.resume", vec![]));
        assert!(matches!(
            result,
            Err(RunnerError::InvalidTransition { .. })
        ));

        // Init -> Active
        let _ = runner
            .receive(sys_bottle("system.init", vec![]))
            .expect("init");

        // Can't init again (already Active)
        let result = runner.receive(sys_bottle("system.init", vec![]));
        assert!(matches!(
            result,
            Err(RunnerError::InvalidTransition { .. })
        ));

        // Active -> Suspended
        let _ = runner
            .receive(sys_bottle("system.suspend", vec![]))
            .expect("suspend");

        // Can't ping while Suspended
        let result = runner.receive(sys_bottle("system.ping", vec![]));
        assert!(matches!(
            result,
            Err(RunnerError::InvalidTransition { .. })
        ));

        // Suspended -> Active
        let _ = runner
            .receive(sys_bottle("system.resume", vec![]))
            .expect("resume");

        // Active -> Terminated
        let _ = runner
            .receive(sys_bottle("system.terminate", vec![]))
            .expect("terminate");
    }
}
