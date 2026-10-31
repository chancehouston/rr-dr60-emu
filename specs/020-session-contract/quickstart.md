# Quickstart: Processing Record (spec 020)

- Core: `cargo test -p rr_dr60 record digest timeline counter_tests` (the grammar, the FNV vectors, the maps' arithmetic, the counter constructor).
- Story 1: `cargo test -p rr_dr60_harness --test us1_record_contract -- --nocapture` (the record written and re-rendered at the measurement rates; the effective changes and the playback twin; the round trip; the dirty-engine counter identity).
- Story 2: `cargo test -p rr_dr60_harness --test us2_record_read -- --nocapture` (the doctored set: every refusal and flag; the imported origin; the incomplete run).
- Story 3: `cargo test -p rr_dr60_harness --test us3_record_timeline -- --nocapture` (the burst markers, the VAS-bypassed oracle, map (c) against the trace).
- Seven rates: `cargo test -p rr_dr60_harness --test record_contract_matrix -- --nocapture`; edge cases `--test record_contract_edge_cases`; mutants `--test mutation record_contract::`; goldens `--test golden_record` (`RR_DR60_BLESS=record` once); `--test ffi_parity record_parity`, `--test alloc_free record`, `--test determinism record`.
- The recipe: `cargo run -p rr_dr60_harness --example re_render -- <record.txt> <raw.i24> ` prints both digests; the C smoke `crates/rr_dr60_ffi/tests/c/run_smoke.sh` (`smoke --record`); `cd swift && swift test`.
- Cost: `scripts/ab-timing.sh 01a61ca record_contract` (solo; 3 %, `RR_DR60_RECORD_CONTRACT_COST_CEILING` 0.05 on the label job).
- Records: the thirteen audio guards and every smoke hash unmoved; `RR_DR60_BLESS=manifest …` once for the version line; the guide scan `cargo test -p rr_dr60_harness --test us2_cross_check guide_record`.
