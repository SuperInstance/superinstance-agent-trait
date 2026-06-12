# superinstance-agent-trait

[![crates.io](https://img.shields.io/crates/v/superinstance-agent-trait.svg)](https://crates.io/crates/superinstance-agent-trait)
[![docs.rs](https://docs.rs/superinstance-agent-trait/badge.svg)](https://docs.rs/superinstance-agent-trait)
[![license](https://img.shields.io/crates/l/superinstance-agent-trait.svg)](https://crates.io/crates/superinstance-agent-trait)

Two-method agent trait with enforced lifecycle transitions and structural conservation.

## Quick Start

```rust
use superinstance_agent_trait::{Agent, AgentRunner, AgentReport, AgentState};
use superinstance_protocol::Bottle;

// Implement the trait — just two methods
struct EchoAgent {
    state: AgentState,
    cycles: u64,
}

impl Agent for EchoAgent {
    fn receive(&mut self, bottle: Bottle) -> Bottle {
        self.cycles += 1;
        // Echo back: swap src↔tgt, keep trits (conservation holds)
        Bottle::new_empty(&bottle.tgt, &bottle.src, "echo", bottle.trits.clone(), 300)
    }

    fn inspect(&self) -> AgentReport {
        AgentReport {
            state: self.state,
            cycle_count: self.cycles,
            last_action: "echo".into(),
            ewma_quality: 1.0,
        }
    }
}

// Wrap in AgentRunner for enforcement
let runner = AgentRunner::new(EchoAgent { state: AgentState::Init, cycles: 0 });

// Init → Active via system.init
let init = Bottle::new_empty("system", "echo", "system.init", vec![0], 300);
let _ = runner.receive(init)?;  // state is now Active
```

## Key Types

| Type | Role |
|---|---|
| `Agent` | Trait with two methods: `receive(Bottle) → Bottle` and `inspect() → AgentReport` |
| `AgentRunner<A>` | Wrapper that enforces lifecycle transitions and conservation audits |
| `AgentState` | Lifecycle states: `Init`, `Active`, `Suspended`, `Terminated` |
| `SystemAction` | Lifecycle-driving actions: `Init`, `Suspend`, `Resume`, `Terminate`, `Ping` |
| `AgentReport` | Snapshot of agent state: state, cycle count, last action, quality metric |
| `RunnerError` | Errors: invalid transitions, terminated agent, conservation violations |

Wire types (`Bottle`, `Trit`, `BottleHeader`, `BottleError`, `audit`, `audit_strict`) are re-exported from `superinstance-protocol`.

## Why

Most agent frameworks conflate the agent's business logic with lifecycle management, routing, and observability. This crate separates them:

- **The agent IS a function** from `Bottle → Bottle`. That's the entire interface.
- **Lifecycle is emergent** — states (`Init`, `Active`, `Suspended`, `Terminated`) aren't methods on the trait. They emerge from system bottles the agent receives.
- **Conservation is structural** — `AgentRunner` enforces that one bottle goes in and one bottle comes out with the same trit sum. No runtime escape hatch.

## Features

- **Two-method trait** — `receive` and `inspect`. Everything else is emergent.
- **Lifecycle enforcement** — `AgentRunner` validates state transitions (e.g., can't `Suspend` from `Init`)
- **Conservation auditing** — every `receive` call is audited: input trit sum must equal output trit sum
- **System bottle handling** — `system.init`, `system.suspend`, `system.resume`, `system.terminate`, `system.ping`
- **Termination guard** — once terminated, the runner rejects all bottles
- **Observability** — `inspect()` returns a snapshot without side effects
- **Re-exports** — wire types from `superinstance-protocol` are re-exported for convenience

## License

MIT OR Apache-2.0
