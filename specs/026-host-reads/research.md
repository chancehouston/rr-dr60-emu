# Research: Host reads

Each decision, its rationale, and the alternatives considered. The full detail is in [plan.md](plan.md)'s R-items.

- **R-01 (D1)**: one `const fn` per rule, called by `paths.rs` itself. *Alternative*: duplicate the expression (rejected: two copies drift). The report latency is the single rounding without the coder's term: exact.
- **R-02 (D2)**: the summed bound with 4·D (two converters), the device-grid + 1, and the SVF's ⌈15.3 / −ln r⌉ + 4 with a ramp box bound, computed on read. *Alternatives*:
  - ⌈ln 1e-5 / ln r⌉: up to 1.44× short near Q 0.5;
  - `max(current, target)`: not a bound for staggered ramps;
  - cached on landing: stale mid-ramp.
- **R-03 (D3)**: two tiers, a Unicode `\w` word class, ASCII folding, span results, and the regex oracle kept. *Alternative*: one list (rejected: the guide's legitimate words).
- **R-04 (D4)**: reach and lag are calls, not struct fields: they depend on the call, the live route and the speed level.
- **R-05 (D5)**: the Swift slot loop as the harness's, and the frame-floored fill.
- **R-06 (D6)**: tokens and labels; the README's "—" kept by the generator's table rule.
- **R-08 (C)**: statuses 44 – 48, 022's refusal orders, and 025's field-local settings reads.
