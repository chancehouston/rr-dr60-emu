# celp-design

The design tool of the speech codec model (spec 008, signal-chain stages 7 and 9). It prints every design constant of the model and generates `crates/rr_dr60/src/codec/tables.rs`.

Everything it prints is a **design constant of the model**, modeled on the G.729 / G.723.1 coder family (S-009, S-008; secondary sources for the class). None of it is a measurement of, or a claim about, the RR-DR60's own coder, whose algorithm is unidentified (A-004, A-040, A-041).

```sh
uv run tools/celp-design/design_celp.py            # every figure: rates, latency table, allocation, tables
uv run tools/celp-design/design_celp.py --check    # the 80-bit budget and the bit order
uv run tools/celp-design/design_celp.py --write    # regenerate tables.rs (byte-deterministic)
uv run tools/celp-design/design_celp.py --lsp-mode gaps   # the alternative LSP quantizer (research R-13)
```

After `--write`, `git diff --exit-code crates/rr_dr60/src/codec/tables.rs` proves the committed tables match the tool. A table change is a model change: a CHANGELOG entry and, after 1.0, a kept decoder (spec FR-011, FR-021).
