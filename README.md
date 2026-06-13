# superinstance-agent-trait

The **core Agent trait** for the SuperInstance ecosystem — a 2-method trait with structural conservation. Every agent in the fleet implements `receive(Bottle) → Bottle` and `inspect() → AgentReport`. Lifecycle, conservation, and quality emerge from the message protocol, not from trait complexity.

## Why It Matters

The genius of this design is its **radical minimalism**: two methods capture the entire agent lifecycle. There are no `init()`, `start()`, `stop()`, `suspend()`, `resume()`, or `handle_request()` methods. Instead:

- **`receive(bottle) → bottle`** — the agent IS a pure function from input to output
- **`inspect() → report`** — observability without side effects

Everything else — lifecycle transitions, quality tracking, conservation enforcement — is **emergent behavior** driven by the content of bottles, not by the type system.

### Design Philosophy

| Traditional Agent API | SuperInstance Agent |
|----------------------|---------------------|
| 10+ trait methods | **2 methods** |
| Explicit lifecycle via types | Emergent lifecycle via bottle actions |
| Framework controls flow | Agent controls flow |
| State in framework structs | State in agent struct |
| Error handling via `Result` | Errors encoded in bottles (ternary $\{-1\}$) |

This is the **Unix philosophy** applied to agents: simple, composable, text-based (trit-based) interfaces.

## How It Works

### The Agent Trait

```rust
pub trait Agent {
    fn receive(&mut self, bottle: Bottle) -> Bottle;
    fn inspect(&self) -> AgentReport;
}
```

That's it. One input, one output. Conservation is structural.

### Lifecycle as Emergent Behavior

Agents transition through four states based on the `act` field of received bottles:

```
Init ──system.init──→ Active
  │                      │
  │                  ┌───┴───┐
  │          system.suspend  system.terminate
  │                ↓         ↓
  │           Suspended   Terminated
  │              │
  │       system.resume
  │              ↓
  └────→ Active ←┘
```

Valid transitions:

| From | Action | To |
|------|--------|-----|
| Init | `system.init` | Active |
| Active | `system.suspend` | Suspended |
| Suspended | `system.resume` | Active |
| Any | `system.terminate` | Terminated |
| Active | `system.ping` | Active (no change) |
| Active | any non-system | Active (agent handles) |

Invalid transitions return `RunnerError::InvalidTransition`.

### Conservation Law: γ + η = C

The **AgentRunner** enforces that every `receive` call preserves the trit sum:

$$\text{trit\_sum}(\text{bottle}_{\text{in}}) = \text{trit\_sum}(\text{bottle}_{\text{out}})$$

If the sums differ, the runner returns `ConservationViolation`:

```rust
let input_sum = bottle.trit_sum();
let response = agent.receive(bottle);
let output_sum = response.trit_sum();
assert_eq!(input_sum, output_sum);  // conservation invariant
```

This means agents **cannot create or destroy information** — they can only transform it. The trit vector is a conserved quantity, like charge in physics.

### Forgemaster: Reference Implementation

The `Forgemaster` agent demonstrates the trait:

| Action | Response | Quality |
|--------|----------|---------|
| `system.init` | `system.init.ack` | — |
| `cycle.request` | `cycle.complete` | EWMA-tracked |
| `system.ping` | `system.pong` | — |
| Unknown | `unknown.ack` (echo trits) | — |

Quality signal is generated as a ternary pattern based on cycle count modulo 3:

$$\text{quality}(n) = \begin{cases} [+1, 0, +1] & n \bmod 3 = 0 \\ [0, 0, 0] & n \bmod 3 = 1 \\ [-1, 0, +1] & n \bmod 3 = 2 \end{cases}$$

The quality EWMA uses $\alpha = 0.3$:

$$\text{EWMA}_{t+1} = 0.3 \cdot q_t + 0.7 \cdot \text{EWMA}_t$$

### Complexity

| Operation | Time | Space |
|-----------|------|-------|
| `receive` | $O(|\text{trits}|)$ (sum + transform) | $O(|\text{trits}|)$ |
| `inspect` | $O(1)$ | $O(1)$ |
| Runner enforcement | $O(|\text{trits}|)$ (audit) | $O(1)$ |
| Lifecycle validation | $O(1)$ | $O(1)$ |

## Quick Start

```rust
use superinstance_agent_trait::{Agent, AgentRunner, Bottle, Forgemaster};

fn main() {
    let fm = Forgemaster::new();
    let mut runner = AgentRunner::new(fm);

    // Initialize
    let init = Bottle::new_empty("system", "forgemaster", "system.init", vec![1, 0, -1], 300);
    let _ = runner.receive(init).unwrap();

    // Run a cycle
    let cycle = Bottle::new_empty("fleet-edge", "forgemaster", "cycle.request", vec![1, 0, -1], 300);
    let response = runner.receive(cycle).unwrap();
    assert_eq!(response.act, "cycle.complete");

    // Inspect state
    let report = runner.inspect();
    println!("State: {}, Cycles: {}, Quality: {:.3}",
        report.state, report.cycle_count, report.ewma_quality);
}
```

```bash
cargo build
cargo test
```

## API

### `Agent` Trait

| Method | Signature | Description |
|--------|-----------|-------------|
| `receive` | `fn receive(&mut self, bottle: Bottle) -> Bottle` | Process bottle, return bottle (conservation enforced) |
| `inspect` | `fn inspect(&self) -> AgentReport` | Observe state without side effects |

### `AgentRunner<A: Agent>`

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(agent: A) -> Self` | Wrap agent with enforcement |
| `receive` | `fn receive(&mut self, bottle: Bottle) -> Result<Bottle, RunnerError>` | Enforce conservation + lifecycle |
| `inspect` | `fn inspect(&self) -> AgentReport` | Delegate to agent |
| `state` | `fn state(&self) -> AgentState` | Current lifecycle state |

### `AgentState`

`Init` → `Active` → `Suspended` ↔ `Active` → `Terminated`

### `RunnerError`

| Variant | Description |
|---------|-------------|
| `InvalidTransition { from, action }` | Illegal lifecycle transition |
| `Terminated` | Cannot receive after termination |
| `ConservationViolation { input_sum, output_sum }` | Trit sums don't match |

## Architecture Notes

This crate is **the heart of the SuperInstance ecosystem**. The conservation law $\gamma + \eta = C$ is not merely documented here — it is **mechanically enforced** by `AgentRunner::receive()`. Every agent in the fleet, from the Forgemaster to future specialized agents, passes through this enforcement layer. The 2-method trait ensures that conservation is **structural** (a property of the type system) rather than **conventional** (a property that developers must remember to check).

The `AgentRunner` is the **conservation boundary**: inside the runner, agents can manipulate trits freely; outside, the runner guarantees that what goes in must come out with the same sum. This is analogous to a physicist's control volume in thermodynamics — the boundary where we account for all energy entering and leaving.

## References

1. Booch, G. et al. (2007). *Object-Oriented Analysis and Design with Applications.* (Agent trait design.)
2. Hoare, C. (1978). *"Communicating Sequential Processes."* Communications of the ACM. (Bottle-passing semantics.)
3. Armstrong, J. (2003). *Making Reliable Distributed Systems in the Presence of Software Errors.* (Erlang's actor model.)
4. Noether, E. (1918). *"Invariante Variationsprobleme."* (Conservation laws from symmetry — the mathematical foundation of γ + η = C.)

## License

MIT
