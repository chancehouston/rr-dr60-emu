# Decision review: report

**Date:** 2026-10-29 · **Engine:** `main` at 6210e28 (0.13.0) · **App:** `ic-recorder-app` at d230979 · **Handoff:** `docs/review/2026-10-21-decision-review-handoff.md`

How the evidence is marked: **(r)** means the reviewer opened the file at that line, and **(i)** means it was inferred from summaries, documentation or general knowledge. An app path written `app:` is in `/Users/chance/Code/ic-recorder-app`. This review was run as four area reviews (A architecture, B fidelity, C process, D product), each checked by a skeptic pass, then merged here. All of it ran on one model family (Claude), so it is **not** an outside-family check (§ 7).

---

## 1. Summary

- **Overall verdict:** most of the engine's decisions hold, and the engine is complete enough for a store 1.0. Of 70 decisions reviewed, by the verdict each row leads with: 42 Keep (13 of them with a narrower scope, a later part or a small change attached), 21 Change, 6 Right decision, wrong time, and 1 Can't judge (§ 2). The risk has moved to three places: the app, the engine/app boundary that no adviser reviewed, and a process that costs too much at under 5 h a week.
- **Pause engine feature work until app 1.0.** That covers specs 014 – 021, new modeled stages and new precision work. The minimum engine for app 1.0 is 0.13.0 plus a one-line commitment that stored form v2 with codec model 0.8.0 stays importable in every later release. Tag `v0.13.0` now: there are no tags at all (`git tag` is empty, r).
- **Change 1: fix the engine/app boundary, about one agent day, in the app.** Four problems:
  - A main-thread lock hold drops recorded input (`app:App/EngineHost.swift:349-368`, r).
  - Recording switches off when the microphone and output rates differ (`app:App/Playground.swift:164-172`, r).
  - Audio interruptions are not handled (r, grep).
  - The engine is an unpinned sibling checkout, and its version string cannot tell commits apart (`app:project.yml:12-16`, `crates/rr_dr60_ffi/src/lib.rs:2290-2291`, r).
- **Change 2: build the missing 1.0 app basics.** The list:
  - a persistent recording library (today recordings exist only in the engine's RAM; `rr_dr60_import` is never called, r);
  - the front panel and the flip (today the screens are Forms);
  - a smaller Investigation mode: plain sensor tracks, manual marks, one timeline and a versioned case file;
  - a 0.42 headroom default;
  - the phone input treated as an uncharacterized "stage 0", with its calibration written down.
- **Change 3: cut the process to fit 5 h a week.**
  - Tier the Spec-Kit flow; the constitution already limits it to "non-trivial changes" (`.specify/memory/constitution.md:120-121`, r).
  - Move the status log out of `CLAUDE.md` (10,895 words, loaded into every session and subagent).
  - Reduce owner touchpoints to two per feature, with the owner's agreement.
  - Assert no new relative timing budgets.
  - Fix the stale agent prompts, which still say "measured character match".
- **Positioning:** drop "shows which stages are checked against a real unit" (`docs/product/decisions.md:9`, `docs/product/front-and-inside.md:88`, r). Lead with what users can check themselves: "modeled stage by stage on the service manual and codec datasheets; every assumption public; switch each stage off and hear what it does".
- **Settle before launch, because these are costly to reverse:**
  - the seller identity and Apple Developer enrolment (lead time);
  - the public app name;
  - stored-form compatibility for users' libraries;
  - a version field in the case-file format;
  - the rule that nothing shipped free later goes behind a purchase.
- **Can't judge: the VAS defaults.** The model pauses in a silent room at every sensitivity level (`docs/hardware/assumptions.md:35`, r), while some users report units that keep recording in a quiet room (`docs/hardware/research/reports/Noise reduction clock and power.md:28`, r). Do not change the engine. Instead offer presets in the app labelled "units differ".
- **Next step:** one owner session of about 2 – 3 hours. Answer the round-2 questions (§ 8), start Apple Developer enrolment, and approve one records chore. After that, the boundary fixes in the app.
- This review shares a model family with the project's earlier advisers. Its strongest findings are the ones checked against code and counts (§ 7).

---

## 2. Decision verdicts

| # | Area | Decision | Verdict | Conf. | Why | Cost to change now | Evidence |
|---|---|---|---|---|---|---|---|
| A1 | Arch | `no_std` + `alloc` core with `forbid(unsafe_code)` | Keep | high | Already paid for. The determinism rule bans the platform libm anyway, and `no_std` keeps clocks, I/O and locks out of the DSP by construction. The claim that it makes Android cheap is weak: Android would go through the C API, which uses std (i). | Nothing to gain from reverting | `crates/rr_dr60/src/lib.rs:177-183` (r); `clippy.toml:1-24` (r); `Cargo.toml:27-30` panic = unwind for the FFI (r) |
| A2 | Arch | A thin, panic-safe C API (47 extern functions, cbindgen header, drift check in CI) | Keep | high | The right surface for Swift now and JNI later. `catch_unwind` and handle poisoning are present, and CI fails on a header diff. | n/a | `crates/rr_dr60_ffi/src/lib.rs:40, 575, 615, 827` (r); `rr_dr60.h:5-16` (r); `.github/workflows/ci.yml:107-116` (r) |
| A3 | Arch | A Swift package around a locally built XCFramework, not a Swift port | Keep the mechanism. A remote artifact with a URL and checksum is Right decision, wrong time | high / med | A port would fork the model and lose bit-identity. CI never builds the XCFramework or runs `swift test`, but the owner's app builds exercise that path. A published artifact matters only once third parties consume the engine. | CI job 30 – 60 min; artifact about half a day, later | `swift/Package.swift:7,13` (r); `scripts/build-xcframework.sh:17-49` (r); `ci.yml:161-175` (r); `docs/product/decisions.md:16` (r) |
| A4 | Arch | *(Missing from the handoff)* The app consumes the engine as an unpinned sibling checkout | Change | med-high | Nothing ties an archive build to a reviewed engine commit. The logged version is only `CARGO_PKG_VERSION`, so two commits of 0.13.0 look identical. | About 1 h | `app:project.yml:12-16` (r); `app:App/BenchSession.swift:102` (r); `crates/rr_dr60_ffi/src/lib.rs:2290-2291` (r); `scripts/build-xcframework.sh:49` (r) |
| A5 | Arch | The C settings struct grows at the end, with `struct_size`, renamed reserved slots and explicit tail bytes | Right decision, wrong time (simplify only inside the ABI pass) | med | Smaller sizes are always refused, so keeping old offsets buys no compatibility. But the explicit tail bytes are what make that refusal work. The ongoing cost is small and paid in agent time. (Not every minor version broke the struct: it stayed at 88 bytes through 0.6 – 0.7 and at 112 through 0.10 – 0.11.) | About 1 day, inside the ABI pass | `crates/rr_dr60_ffi/src/lib.rs:582-603, 684-697` (r); `rr_dr60.h:452-565`, `:453`, `:522-552` (r) |
| A6 | Arch | No size field on the report structs | Right decision, wrong time | high | The gap is wider than `RrDr60PlayInfo`. Four structs grew in 0.8 (BlockInfo, RecordInfo, StopOutcome, PlayInfo), and PlayInfo grew again in 0.13. Static linking keeps a mismatch out of an installed app. Fix it once, before any prebuilt binary is published. | Half a day to a day, inside the ABI pass | `rr_dr60.h:568-683` (r); CHANGELOG 0.8 entry near `CHANGELOG.md:151` (r); `app:App/EngineHost.swift:398` (r) |
| A7 | Arch | Rust `Settings` is `#[non_exhaustive]` over a fixed pipeline | Keep | high | Fields can be added without breaking callers, and the fixed order matches the device. | n/a | `crates/rr_dr60/src/settings.rs:20,171,210` (r) |
| A8 | Arch | 6 kHz device rate inside, resampling at the boundaries | Keep | high | Sourced from the service manual (S-002). 6 kHz is the identity host rate, and the cost is low. | n/a | `rr_dr60.h:457-458` (r); timing figures from `CLAUDE.md` (i) |
| A9 | Arch | Seven host rates; the app passes `AVAudioSession.sampleRate` straight to the engine | Change (app side only) | med | The app never sets a preferred rate. When the microphone's rate differs from the output's, it switches recording off. Run the app's graph at a fixed 48 kHz and convert at the hardware boundary. No engine change, and no 24 kHz host rate. | About 2 h, app | `app:App/Playground.swift:150-175, 191, 199-211` (r); `docs/product/investigation-mode.md:261` (r) |
| A10 | Arch | Fixed-point codec, IEEE-basic float, one golden set on 6 desktop targets, the iOS simulator and one device | Keep | high | The enforcement is real. Gap: no CI job golden-tests the exact shipped feature set, because the harness always enables `__test-hooks`. Low risk if the hooks only add code paths (i). | n/a | `ci.yml:81-83, 133-171` (r); `crates/rr_dr60_harness/Cargo.toml:13` (r) |
| A11 | Arch | No Android target in CI | Right decision, wrong time | med | The first release is iOS only. The aarch64 Linux target already exercises the arm64 float path (i). Add Android when Android work starts or when case files cross platforms. | Build only 30 min; emulator golden about 1 day | `ci.yml:139-151` (r); `docs/product/investigation-mode.md:200, 240` (r) |
| A12 | Arch | 0.x semantic versioning, a breaking minor version per feature | Keep until engine 1.0 | high | With one statically linked consumer, breaks cost almost nothing. Engine 1.0 should freeze the C ABI, stored form v2 and the decoder policy together. Engine 1.0 and app 1.0 are separate milestones. | n/a | `rr_dr60.h:1158-1190` (r); `Cargo.toml:9` (r) |
| A13 | Arch | MSRV 1.85, checked by running the full test suite on 1.85 | Keep | med | Right for a library. A `check`-only job would save runner time, not owner time. The job line shows no `RUSTFLAGS=-D warnings`; check whether it is set at workflow level (not read). | 10 min if wanted | `ci.yml:177-186` (r); `Cargo.toml:7` (r) |
| A14 | Arch | *(Missing)* Thread contract: the app guards each handle with a lock that the audio callbacks only try-lock | Change (app now; an engine-side mailbox if spec 015 ever happens) | high on the mechanism, low on frequency | When the main thread holds the lock (volume, speed, route, `recordings()`/`memory()`), `receiveScaled` skips both `rr_dr60_record` and the raw capture. This breaks the app's own rule that the render path takes no locks. The `lockMisses` counter measures how often it happens. | 2 – 3 h, app | `app:App/EngineHost.swift:259-319, 344-368` (r); `app:CLAUDE.md:7` (r); `app:App/Playground.swift:295` (r) |
| A15 | Arch | *(Missing)* Recordings live in the engine handle's RAM and persist only through export/import | Keep the engine design; treat the stored form as a product file format | med | Saved recordings, case files and sharing will all depend on RRDR60RC v2 plus the codec model version. See D12 for the commitment. | A decision | `rr_dr60.h:149-152, 1180-1190` (r); `app:App/EngineHost.swift:47-58, 222-236` (r) |
| A16 | Arch | *(Missing)* No Swift wrapper; the app matches status codes by literal integer | Keep (no wrapper); Change the literals | med | A wrapper is not worth a spec for one app. Named constants make a renumbering fail at compile time instead of mislabelling an error. | 15 min | `app:App/EngineHost.swift:20-35` (r) |
| B1 | Fidelity | Limiter knee at −1 dBFS rather than +1 | Keep | med | Derived from the circuit (A-028), with +1 dBFS booked as the upper end of the range. The app already exposes the knee (−30 … +3 dBFS). | Low | `docs/hardware/assumptions.md:41` (r); `app:App/ModelView.swift:54-55` (r) |
| B2 | Fidelity | VAS defaults: −40 dBFS at level 3, 3 dB steps, 1.0 s hang, 20 ms onset, drop mode | Can't judge | low | The behaviour that most defines the device for its audience rests on unsourced timing (A-023, Low). The model pauses in silence at every level, while forum reports describe a spread between units. Judging it needs VAS behaviour from a real unit. The decision available now is app presets (change 9). | Low per number | `docs/hardware/assumptions.md:35-37` (r); `…/Noise reduction clock and power.md:28, 244` (r) |
| B3 | Fidelity | A class-match 4 kbit/s ACELP coder, not the device's coder | Keep | med | The only honest option while the device's algorithm is unknown, and calibrated against the ITU G.723.1 reference software. Still the largest fidelity gap. Behaviour on noise and non-speech has no tolerance. | High | `assumptions.md:17, 53-54` (r); `specs/008-celp-codec/spec.md:66, 68, 201` (r) |
| B4 | Fidelity | From 1.0, every codec model version keeps its decoder (FR-011) | Keep | low | With verification out of scope, a codec revision is unlikely, and keeping an old decoder is cheap. | Low before 1.0, high after | `specs/008-celp-codec/spec.md:64, 102, 234` (r); `constitution.md:26-31` (r) |
| B5 | Fidelity | Microphone hiss at about −62 dBFS (offset −18 … +12 dB) | Keep | med | Audible after decoding (about −67 dBFS, i) and it sets the VAS margin. The offset spans the credible range. | Low | `assumptions.md:61-62` (r) |
| B6 | Fidelity | Microphone amplifier: an 818 Hz high-pass and an asymmetric clipper fitted to an ngspice sweep | Keep | med | Clearly audible (−7.3 dB at 300 Hz), and checked by an independent solver. That same sweep found the first design 2.4 dB off. | None | `assumptions.md:42, 59, 64` (r) |
| B7 | Fidelity | A 20 ms smoothstep volume transition (spec 011) | Keep | high | Click-free volume changes have a physical basis (A-045). The process was out of scale for a convenience (see C1). | None | `assumptions.md:58` (r); `CLAUDE.md:105` (r) |
| B8 | Fidelity | Speed mechanism (b): pitch moves with speed; (c), which keeps pitch, is a follow-up | Keep | med | Well defined and labelled. Capture P1 will not come, so judge (c) as an app request, not as waiting for evidence. | Medium to add (c) | `assumptions.md:67` (r); `…/Noise reduction clock and power.md:268` (r) |
| B9 | Fidelity | Output clamp as an emulator option, off by default | Keep | high | Correctly labelled as not a device behaviour. The app needs a headroom default (change 10). | None in the engine | `assumptions.md:55` (r); `app:App/ModelView.swift:57` (r) |
| B10 | Fidelity | Traceability: every number traces to an S- or A- ID, enforced by the cross-check | Keep, with short-form rows from now on | high | It is what makes the project honest without a unit. 16 of 58 register rows run to 300 words or more (the file is about 15,400 words), so cap the length of new rows. | Low | `constitution.md:19-31` (r); `assumptions.md` word counts (r) |
| B11 | Fidelity | Real-unit verification out of scope; v1 is a modeled character match | Keep | med | Right under the owner's constraints. Principle VI still bars marketing accuracy. Keep the standing invitation for a lent or donated unit. | Low | `constitution.md:26-38` (r); `CLAUDE.md:31` (r) |
| B12 | Fidelity | Positioning on "modeled stage by stage, public register" | Keep | med | It persuades developers and audio enthusiasts. EVP users judge by ear, so add audible evidence within the claims policy (stage-off renders). The 2026-10-21 "validated / chip-level" wording was caught and not adopted. | Low | `README.md:3, 17-18` (r); `docs/product/brief.md:10` (r); `investigation-mode.md:204` (r) |
| B13 | Fidelity | Build order: skeleton, compressor and VAS before the service manual was read | Right decision, wrong time (historical) | med | The rework cost days, not weeks (8 kHz → 6 kHz, compressor → limiter). Lesson: read the primary source and fix the level anchor before modeling level-dependent behaviour. | None | `assumptions.md:14, 30-31, 35` (r); `CLAUDE.md:95-98` (r) |
| B14 | Fidelity | Precision beyond audibility and beyond the anchors (sub-0.1 dB pins, 70 dB resampler rejection) | Change (from now on) | med | One to two orders of magnitude finer than the anchors (A-029 ±10 dB, the Low VAS rows, the one-phone calibration). In one place the model may be cleaner than the device (A-054 suggests images 25 – 40 dB down on a 1998 DSP). | Low (a policy) | `assumptions.md:29, 42, 64, 67` (r); `app:App/Playground.swift:49-54` (r) |
| B15 | Fidelity | Earphone class-A cut-off: "only if a capture shows it" | Keep, and restate the gate | med | The predicted onset (0.4 – 0.8 of the control's travel) is above the reference position. Without a unit the gate is permanent, so reword it as "not planned; app request". | Medium if built | `assumptions.md:45, 56` (r) |
| B16 | Fidelity | Clip indicator "if the app asks" | Keep | med | The app counts clipped output itself. Per-stage indicators belong in a meters spec, if one is ever built. | None | `CLAUDE.md:134` (r); `app:App/EngineHost.swift:113-114, 384-389` (r) |
| B17 | Fidelity | µ-law quantization not modeled | Keep; close the held item | med | From the datasheet figures it sits 20 – 30 dB below the codec's error (i). | Medium churn (every golden) | `assumptions.md:16, 43` (r); `specs/008-celp-codec/spec.md:290` (r) |
| B18 | Fidelity | Stage 6 noise reduction not modeled | Keep | high | Absent from the Panasonic documents; the claim traces to an unsourced 2020 article. | None | `assumptions.md:22` (r); `…/Noise reduction clock and power.md:7-13` (r) |
| B19 | Fidelity | Clock at a nominal 6000 Hz, no deviation parameter | Keep | high | A unit records and plays on the same clock. | None | `assumptions.md:24` (r) |
| B20 | Fidelity | Battery-dependent speaker ceiling (A-056) not modeled | Right decision, wrong time | med | A natural "unit condition" control after 1.0. | Low – medium | `assumptions.md:69` (r) |
| B21 | Fidelity | Start-of-item transients (A-057) wait for capture P3 | Keep; say the gate is permanent | med | The thump bound alone spans −61 … +4 dBFS. | None | `assumptions.md:70` (r) |
| B22 | Fidelity | *(Missing)* The phone microphone and iOS input processing as an uncharacterized "stage 0" | Change | med | Every level-dependent stage lands wherever the +5 dB one-phone calibration puts it, in `.default` mode. No register row in either repo covers it. | Low (app) | `app:App/Playground.swift:49-54, 202` (r); `app:App/LiveView.swift:25` (r) |
| B23 | Fidelity | *(Missing)* Design-script figures that agree with the Rust are not independent checks | Change (records); an ngspice limiter check is optional | med | The one solver-independent check found a 2.4 dB error. The limiter's dynamics rest only on a Python model from the same family. | About half a day of agent time if done | `assumptions.md:47-48, 64` (r); `specs/012-mic-capsule-amp/plan.md:30` (r) |
| B24 | Fidelity | Records: the handoff says "ten of fifteen" stages are modeled | Change | high | It is twelve (stages 1 – 5 and 7 – 13). | Trivial | handoff `:20` (r); `README.md:17` (r); `docs/hardware/signal-chain.md:29-41` (r) |
| C1 | Process | Spec-Kit's full eight steps for every feature, plus a separate convergence PR | Change (tier it) | high | Spec 010 (the clamp) produced 27,802 words of artifacts, 48 tasks and a 49-file diff of +7,551 lines. The constitution already limits the flow to non-trivial changes; only `CLAUDE.md:55-57` says "all". | About 1 h | `specs/010-output-clamp` word count (r); `git diff --stat fe571ab cf94984` (r); `constitution.md:120-121` (r); `app:CLAUDE.md:5` (r) |
| C2 | Process | Three advisers consulted at several points per feature | Change | med-low | The strongest catches came from outside tools (ngspice, the ITU reference software). Keep a hardware review at plan time and after implementation for modeled stages; drop routine coach passes. Two prompts are stale: they say "measured character match" and "mid-1990s". | 15 min | `.claude/agents/speckit-coach.md:15`, `hw-emulation-reviewer.md:15,24`, `product-specialist.md:15` (r); 004 plan-review catch (i, memory index) |
| C3 | Process | Owner cadence: a stop per phase, a commit per command, ask before merging | Change, if the owner agrees | med | The owner confirmed it on 2026-10-12, during a burst of 347 commits in 10 active days. Since then: 6 commits in 11 days at under 5 h a week. | Trivial | `git log` (r); memory `feedback-speckit-workflow.md` (r) |
| C4 | Process | Measurement tests at seven rates (three under coverage) | Keep | med | Rate bugs live at the resampler boundary, and the coverage cost has already been cut. | None | `CLAUDE.md:48, 102` (r) |
| C5 | Process | Golden files with SHA guards on 6 targets plus the iOS simulator | Keep | high | 2 – 4 min per run; the backbone of determinism and of Swift parity. Every re-bless was an intended model change. | None | `crates/rr_dr60_harness/golden/` (r); `ci.yml:133-159` (r) |
| C6 | Process | Mutation tests through `__test-hooks` | Keep | med | 33 cheap mutants show that the tests can fail. | None | `crates/rr_dr60_harness/tests/mutation.rs` (r) |
| C7 | Process | Python design scripts whose printed figures the Rust tests pin, with ngspice and ITU gates | Keep | high | The only oracle written apart from the Rust code, and it has found real errors. | None | `tools/*/*.py` (r); `CLAUDE.md:101, 106` (r) |
| C8 | Process | Relative timing budgets of 2 – 15 % asserted in CI, five runner ceilings, A/B workflow | Change (from now on) | med | The worst block is 1.5 % of real time (3.9 % on the runner). Sub-5 % assertions protect nothing audible and flaked twice. Report relative costs; assert only the absolute real-time guards and the functional assertions. | None from now on; 1 – 2 h to convert the existing ones | `ci.yml:84-103` (r); `crates/rr_dr60_harness/tests/timing.rs:298, 354-364, 460-471` (r); `CLAUDE.md:107, 109` (r) |
| C9 | Process | 80 % coverage gate, the 1-point rule, coverage on every PR | Keep | med | At the current cadence a 42-minute job does not bind. Two fixes: the stated 15 – 30 min is stale, and CI does not enforce the 1-point rule. | 5 min | `ci.yml:119-131` (r); `CLAUDE.md:114` (r) |
| C10 | Process | `CLAUDE.md` as the living status log | Change | high | 141 lines but 10,895 words (72 KB), loaded into every session and subagent (observed in this run). Move the measurement lessons out first, then the log. | About 1 h, docs only | `wc CLAUDE.md` (r); `CLAUDE.md:85-130` (r) |
| C11 | Process | A minor version per feature, never tagged | Change | high | Zero tags, and everything sits under [Unreleased]. Tag v0.13.0, the version the device golden check passed. | 15 min | `git tag` (r); `CHANGELOG.md:9` (r); `docs/release-checklist.md` (i) |
| C12 | Process | The rigor sits in the engine; the app has little process | Change | med | 824 tests and 12 CI jobs in the engine, against 1,996 lines of Swift, one test file and no CI in the app. The app needs a pre-submission checklist, not Spec-Kit. | About 1 h | `grep '#[test]'` (r); `app:CLAUDE.md:7, 9` (r) |
| C13 | Process | Traceability IDs and the registers job | Keep | high | 6 – 53 s per run. Apply it only where a hardware number is involved. | None | `constitution.md:21-27` (r); `gh run list` (r) |
| C14 | Process | Test-first mandate | Keep | med | Already limited to measurement tests. | None | `constitution.md:40-43` (r) |
| D1 | Product | One instrument, two modes, both in the first release | Keep, with a much smaller Investigation 1.0 | med | The app has no Investigation code (no CoreMotion). The designed scope far exceeds about 65 owner-hours in 90 days. | Low (nothing built) | `docs/product/decisions.md:29, 37` (r); `app:App/ContentView.swift:7-13` (r); `investigation-mode.md:82-90, 146-172` (r) |
| D2 | Product | The flip (front and exposed back); skins later | Keep | high | The clearest answer to guideline 4.3, and the largest piece of 1.0 app work. Today's screens are Forms. | n/a | `brief.md:53-63, 203-205` (r); `app:App/RecorderView.swift:8-9` (r) |
| D3 | Product | Honest-motion rule; meters (016) | Keep the rule. A reduced "016-lite" is optional | med / low | Manifest statuses, the existing events and the app-side levels already give a 4.3-worthy inside. Build 016-lite only under a reduced process. | M engine, if built | `brief.md:128, 271` (r); `app:App/LiveView.swift:32-34` (r) |
| D4 | Product | Positioning row: "…shows which stages are checked against a real unit" | Change (copy) | high | Since 2026-10-18 no stage will be checked. Point the "help" link at the evidence issue template, not the superseded capture kit. | 1 h | `decisions.md:9, 15, 20, 36` (r); `brief.md:117, 123` (r); `front-and-inside.md:88` (r) |
| D5 | Product | Generous free tier; paid packages later; no ads, consumables or subscription | Keep the direction. Designing the paid shape is Right decision, wrong time | high | Revenue is not a year-one goal. Write down one rule now: a later purchase only adds new components. | None | `brief.md:93-106` (r); `decisions.md:32-33` (r) |
| D6 | Product | No trademark in the app name | Keep, and extend it to the subtitle and keywords | high / med | Guideline 2.3.7 covers keywords (i). A rename after launch keeps ratings (i) but loses search position. "IC Recorder" is weak for search. | Low now | `decisions.md:12` (r); `app:project.yml:27-29` (r) |
| D7 | Product | "Data not collected"; no analytics; on-device processing | Keep | high | Put the Bench tab and its "Export for Claude" zip behind a debug build flag. | Low | `decisions.md:13` (r); `app:App/BenchView.swift:104-123` (r) |
| D8 | Product | Investigation sensor design, automatic marks, case files | Keep the design. Automatic marks are Right decision, wrong time | med | The thresholds are guesses until there is self-interference data, and automatic marks are the riskiest wording surface. Ship manual marks in 1.0. | Low | `investigation-mode.md:146-170, 269-273` (r) |
| D9 | Product | AI surface: structured only, on-device, guardrails 1 – 8 | Right decision, wrong time (build); adopt rules 5 – 8 now | high | The rules are free and protect the claims policy. Building anything (014, 015, a schema) before the app has users is premature. | None | `docs/product/controls-discussion.md:186-197, 224-231` (r) |
| D10 | Product | iOS first (minimum iOS 26), Android planned | Keep | med | Already the deployment target; Android doubles the app work. | n/a | `app:project.yml:5-6` (r) |
| D11 | Product | *(Missing)* Recordings live only in engine memory; nothing reloads on launch | Change | high | `rr_dr60_import` is never called and there is no background mode, so recordings are lost when iOS kills the app. The Bench's write path can be reused. | S (app) | grep (r); `app:App/BenchSession.swift:83-85, 152-154` (r); `brief.md:207` (r) |
| D12 | Product | *(Missing)* Users' recordings must survive engine updates | Change: a one-line policy | med | Import refuses unsupported codec model versions, and the promise to keep decoders starts only at 1.0. | Low now | `rr_dr60.h:1177-1190` (r); `investigation-mode.md:199` (r) |
| D13 | Product | *(Missing)* The +5 dB phone calibration (one iPhone) | Change: record it in the app repo and plan for other phones | med | It places the −40 dBFS VAS threshold and the −1 dBFS knee in real rooms. It belongs in the app's records, not the engine's register. | S | `app:App/Playground.swift:49-64` (r); `app:App/LiveView.swift:22-31` (r) |
| D14 | Product | *(Missing)* Audio session: `.default` mode, no interruption handling | Change before submission | med | A phone call mid-recording is unhandled. Stop the item, as the design already says. Choose `.measurement` or `.default` deliberately (i). | S | `app:App/Playground.swift:199-211, 224-231` (r); `investigation-mode.md:74` (r) |
| D15 | Product | *(Missing)* Developer Program enrolment and seller identity | Change: decide now | med | TestFlight needs it. An individual account shows the legal name as seller; an organization needs a D-U-N-S number (i). The product docs never mention it. | Low now | `app:CLAUDE.md:10` (r); grep in `docs/product` (r) |
| D16 | Product | 17 O-items plus 13 owner questions across the briefs | Change: one "1.0 scope and non-goals" page | med | Too many open questions for 5 h a week. Many are moot after the 2026-10-21 answers. Add the page; do not rewrite the briefs. | Low | `brief.md:147-214` (r); `controls-discussion.md:274-289` (r) |

---

## 3. Open questions

| Question | Recommendation | Depends on | Urgency |
|---|---|---|---|
| **Largest: is the engine complete enough?** | Yes. Pause engine feature work. The minimum engine is 0.13.0 plus the stored-form commitment (D12). 016-lite is optional. The ABI pass does not block the app. | Round-2 Q5 | Before the next spec |
| O-1 Which controls are free | Every control the engine exposes, model internals included, labelled by the four parameter kinds. This departs from controls-discussion Q2's paid Workbench: with no revenue goal and an MIT engine, free is the lower-regret choice (`controls-discussion.md:282`, r). | Round-2 Q7 | Before 1.0 |
| O-2 Paid boundary | Defer. No in-app purchase in 1.0. | Answered (no revenue) | Later |
| O-3 Raw microphone audio | Keep it in both modes, 24-bit lossless, deleted with its recording, opt-in per share in case files. About 520 MB/h uncompressed, 250 – 350 MB/h compressed (i). Today's Bench files are float32, about 690 MB/h (`app:App/BenchSession.swift:153`, r). | Round-2 Q4 | Before 1.0 |
| O-4 Release sequencing | 1.0 = Recorder mode with the flip, plus Investigation-lite. 1.1 = automatic marks, "Inside this moment" cards, the meter track. | Round-2 Q3 | Before 1.0 |
| O-5 Default mode | Open in Recorder mode, with a remembered one-tap switch. | Prototype testing | Before 1.0 |
| O-6 Subscription | None. Guideline 3.1.2(a) requires ongoing value. | Nothing | Later |
| O-7 – O-14 Design | O-11: "Magnetic field (µT)", never "EMF". O-12: adopt the proposed reconciliation. O-9: changes between recordings. Settle the rest during the build. | Nothing blocking | O-11/12 before 1.0 |
| O-15 Recorder mode's audience | The plain device experience for every user, not a vintage niche (`personas.md:14`, r). | Round-2 Q7 | Before 1.0 |
| O-16 "Entertainment" disclaimer | Don't use it. Keep the copy rule and a neutral statement of what the app does. Guideline 1.1.6 says that phrase does not cure inaccurate device data (i; re-read Apple's text). | Apple's 1.1.6 wording | Before 1.0 |
| O-17 Guidelines 4.2 / 4.3 | Persistent library, instant playback, scrub, trim, share sheet; the flip and stage statuses visible within 30 s; interruptions handled; review notes pointing to the open engine. | D11, D14 | Before 1.0 |
| Controls Q1 Reopen product questions | Yes; the 2026-10-21 answers already did. Retire the "deferred until engine core is complete" rule. | Nothing | Before the next spec |
| Controls Q2 Four parameter kinds | Adopt them as UI and documentation vocabulary, not tied to a paywall. Don't build 014 for 1.0. | Nothing | Later (014) |
| Controls Q3 Record-side timing | Between recordings, as today. Measure reconfigure time on the phone instead of writing a spec. | One measurement | Later |
| Controls Q4 AI surface | Adopt rules 5 – 8 now; build after 1.0. | Nothing | Rules before 1.0 |
| Controls Q5 Spec order 014 – 021 | Pause it. Revisit when the app or users ask. | Round-2 Q5 | Before the next spec |
| Controls Q6 Battery state | Later. | App demand | Later |
| Controls Q7 Pitch-kept speed | Judge it as an app request; the crossfade (019) is post-1.0. | App request | Later |
| Investigation D1 Order 016 → 020 → 014 | No full 016 or 020 before 1.0. An app-side time map at about one 20 ms frame is enough. Apply the pre-codec latency in the input domain and the 180-sample codec delay in the kept domain, as `investigation-mode.md:76` warns, and pin it with a test against a mute-mode render. | An app-side map test | Before the next spec |
| Investigation D2 Drop or mute | Drop (the device's behaviour), mapped through the splice events. | The D1 map | Before 1.0 |
| Investigation D3 Live meter track | 1.1. | 016 | Later |
| Investigation D4 Raw audio in case files | Yes, opt-in per share. | O-3 | Before 1.0 |
| Investigation D5 Location | Not in 1.0. | Nothing | Before 1.0 |
| Investigation D6 48 kHz feed; Android | 48 kHz yes (also fixes A9). Android golden later. | Nothing | Later (Android) |
| Investigation evidence list | Items 1 – 2 before automatic marks (1.1). Items 3 – 4 only when needed. | D8 | Later |
| CLAUDE.md: earphone cut-off | Don't build it. Restate the gate as "not planned; app request". | App exposure of high volume positions | Later |
| CLAUDE.md: clip indicator | The app's own counter covers the route output. Per-stage lights only inside a meters spec. | Inside-view design | Later |
| CLAUDE.md: Android distribution | An AAR over the C API with thin JNI/Kotlin, no port; an arm64-only decision up front. | When Android starts | Later |
| CLAUDE.md: pre-1.0 ABI pass | One chore before engine 1.0 or the first published binary. Scope: one ABI-version mechanism for the settings struct and the four grown report structs; optionally drop the renamed slots; a post-1.0 compatibility rule; freeze stored form v2. | Round-2 Q1 | Before engine 1.0 (not before the app submission) |
| CLAUDE.md: µ-law modeling | Close it as expected to be inaudible with the codec on (i). | Codec-off marketing | Later |
| New: host-rate mismatch on iOS | A fixed 48 kHz graph in the app. | Bench data with Bluetooth or USB | Before app 1.0 |
| New: VAS in a silent room | Engine default unchanged. App presets labelled "units differ" (change 9). | Round-2 Q7 | Before 1.0 |

---

## 4. Changes, ranked

Conflicts resolved:
- **Engine pin vs. release tag** (A vs. C): do both. The tag names the version; `git describe --dirty` inside the XCFramework names the exact tree.
- **Stored-form freeze** (A: decide before external testers; B: keep FR-011; D: commit now): chose D's commitment now. It costs one line, and it makes the round-2 "may test recordings be discarded?" question moot.
- **Which engine specs before 1.0** (C: 014 and 016 as spec-lites; D: no 014, 016-lite optional): chose D. The app hard-codes about 15 controls, so a control manifest has no 1.0 consumer.
- **Where the phone calibration is recorded** (B: an assumption row; D: the app repo): the app repo, because it is an app setting, not a device behaviour.
- **O-1 free set** (D vs. controls-discussion Q2): chose free, for the reason given in § 3.

| Rank | Change | Why | Effort | Risk if not |
|---|---|---|---|---|
| 1 | Pause engine feature work. Write one "app 1.0 scope and non-goals" page that records the 2026-10-21 answers, retires the deferral rule, closes O-1/2/4/6 and rewrites every "wait for capture" gate as "not planned; reopens with a unit or an app request". | Success is an app people use. At about 65 owner-hours in 90 days, every hour on engine specs delays the store. | 2 – 3 h | The app stays a test bench |
| 2 | Boundary fixes in the app: volume, speed and route changes through atomics applied by the audio thread; no main-thread engine lock while recording; a fixed 48 kHz graph; interruption and media-reset handling; status constants instead of literals. | Recorded input is dropped, recording is disabled on mismatched routes, and interruptions are unhandled (A9, A14, A16, D14) | About 1 agent day | Gaps in recordings that read as VAS behaviour; crashes in review |
| 3 | A persistent recording library: reuse the Bench's `.rrdr60` write path, import on launch, add names, dates and tags. Commit in the CHANGELOG and guide that codec model 0.8.0 and form v2 stay importable. | Recordings are lost when iOS kills the app (D11, D12) | S + 1 – 2 owner h | Lost user data; one-star reviews; a 4.2 risk |
| 4 | Pin the engine: the build script writes `git describe --dirty` into the XCFramework and refuses a dirty tree for archives; tag `v0.13.0`. | Nothing ties a shipped binary to a reviewed commit (A4, C11) | About 1 h | An unreviewed engine ships |
| 5 | Process trim: tier the flow (A modeled stage / B spec-lite in the PR / C chore), run converge inside the feature PR, slim `CLAUDE.md` (lessons out first), fix the four stale agent lines, assert no new relative timing budgets, fix `CLAUDE.md:114`, note that the 1-point coverage rule is unenforced. | The agent budget and owner rounds are the binding constraint (C1, C2, C8 – C10) | About 2 – 3 h | Every small hook costs a full cycle; about 18k tokens of context in every session |
| 6 | Apple Developer enrolment (individual or organization), the public app name, an App Store Connect record. | Lead time; the seller name is public in a sensitive category (D6, D15) | 1 – 2 owner h plus a wait | Submission slips on paperwork |
| 7 | The front panel and the flip, with the inside driven by manifest statuses, events and levels. | The 4.3 differentiator (D2, D3) | L (app) | No reason to pick this app; 4.3 risk |
| 8 | Investigation-lite: magnetometer, motion, pressure and vibration tracks; MARK and "Question asked here"; one timeline; a case-file zip with `format_version` and documented columns. | The owner wants both modes at launch; a versioned format is cheap now (D1, D8) | L (app) | A late launch, or rushed automatic-mark wording |
| 9 | Stage 0 and presets: test `.measurement` against `.default`; record the calibration and its basis in the app repo; a one-step level guide; unit-variation presets (hiss offset, VAS level, knee ±1 dBFS) labelled "units of this model differ; these span the documented range". | The level-dependent behaviours users notice depend on one phone's estimate (B2, B22, D13) | About 1 agent day + 30 owner min | Pauses and limiting land several dB off on other phones |
| 10 | App output default: 0.42 headroom scaling on the speaker route, with the clamp as the user option. | The default speaker route reaches about 2.0 (B9) | Small | The phone's own clipping heard as device character |
| 11 | Copy: update the positioning rows and board header (D4); adopt AI rules 5 – 8 and O-16 option 1 as decision rows; add a short "assumed, roughly ranked by audibility" list to the README's Accuracy section and the app (the ranking itself labelled an estimate). Fix the handoff's stage count. | Stale copy promises real-unit checks; the register's honesty is invisible (B12, B24, D4, D9) | 2 – 4 h | Overstated metadata (2.3) |
| 12 | Bench behind a debug flag; an app pre-submission checklist (guidelines, privacy label, permission strings, the real-time audio rules). | Launch risk is in the app (C12, D7) | About 1 – 2 h | A rejection found at review |
| 13 | Records only: short-form register rows from now on (B10); state that design-script agreement is a consistency check (B23). | Maintenance cost; honesty about independence | Trivial | The register decays |
| 14 | Optional: an ngspice equation check of the limiter loop; a macOS CI job running `build-xcframework.sh` and `swift test`. | The only audible dynamics with no independent solver; the one untested integration path | About half a day; 30 – 60 min | A transient error goes unseen; a packaging break is found in the app |
| 15 | Later: the pre-1.0 ABI pass (§ 3). | Report structs without a size or version field | 1 – 2 days | Only matters once third parties link prebuilt binaries |

---

## 5. The next 90 days

Budget: about 65 owner-hours (under 5 h a week); agents build most of it. No deadline ("quality first"), so a gate that fails moves the date, not the bar.

| Weeks | Work | Owner time | Gate |
|---|---|---|---|
| 1 | Answer round 2. Start enrolment. Pick the public name. Approve one docs-only records chore (changes 1, 5, 11, 13; tag v0.13.0; the stored-form commitment line). | 3 – 4 h | **G0:** the 1.0 scope page is merged; enrolment has started |
| 2 – 3 | Boundary fixes and the engine pin (changes 2, 4); headroom default; Bench behind a debug flag; stage-0 mode test. | 2 – 3 h a week (phone tests) | **G1:** on the phone, `lockMisses` stays 0 across a recording with slider drags; recording stays on with a Bluetooth or USB output; a phone call mid-recording ends the item cleanly |
| 3 – 6 | Persistent library with tags, playback, scrub, trim, share sheet (change 3). Calibration guide and "units differ" presets (change 9). | 3 h a week | **G2:** a recording survives an app kill and a rebuild against the pinned engine. Decide whether 016-lite is in scope (default: no) |
| 5 – 9 | Front panel and flip (change 7). | 4 h a week (design taste) | **G3:** a new viewer finds the flip and the stage statuses within 30 s |
| 8 – 11 | Investigation-lite (change 8) with a versioned case file. | 4 h a week | **G4:** a 30-minute session exports a case file that re-opens; the time map passes its mute-mode test |
| 11 – 12 | TestFlight (internal, then a few external testers); copy review against the claims policy and guidelines 1.1.6 / 2.3 / 2.3.7 / 4.2 / 4.3 / 5.2; privacy policy and labels; the device golden check on the release commit. | 5 h a week | **G5:** the checklist is clean; no crash reports in a week of testing |
| 13 | Submit. | 2 h | Review |

If it runs late, cut Investigation-lite further first: drop pressure, then the timeline review in favour of the case-file export (i). Do not start engine specs inside the window.

**Definition of a store-ready 1.0.**
- **Engine:** 0.13.0 (or a later tag), pinned by commit, with the device golden check passed on that commit. Stored form v2 with codec model 0.8.0 committed as importable from now on.
- **Recorder mode:** front panel and flip; record, pause and stop, with the VAS behaving as modeled; instant playback; the speed control; volume; the speaker and earphone routes; headroom-safe output; a persistent library with tags, scrub, trim and share sheet; per-stage on/off; presets labelled "units differ".
- **Investigation mode:** audio plus magnetometer, motion, pressure and vibration tracks, in units, with printed context; manual marks; one timeline; a versioned case-file export; no automatic marks; nothing sent off the device.
- **Platform:** interruptions handled; the audio graph at 48 kHz; no lock in the render path; microphone and Motion & Fitness permission strings; "data not collected"; a privacy-policy URL; the Bench excluded from the release build.
- **Copy:** no trademark in the name, subtitle or keywords; the device named once in the description, with a notice; no "EMF", "detect", "authentic", "validated", "chip-level" or "reproduces exactly"; no "entertainment" disclaimer; no promise of real-unit checks; review notes pointing to the open engine and the stage manifest.
- **Account:** paid developer account, seller identity chosen, app name reserved.

---

## 6. Answers to the six questions in § 7

**1. Was the fidelity depth proportionate?** Partly. Every modeled stage has an audible reason to exist: the codec band, the 818 Hz high-pass, the codec, the VAS, the limiter, and the speaker and earphone corners (`assumptions.md:23, 35, 40, 41, 45, 53-54, 59`, r). The excess is precision. Pins of 0.01 – 0.3 dB and 70 dB resampler rejection are one to two orders of magnitude finer than the uncertainties that decide what a user hears. Those uncertainties are:
- A-029's ±10 dB level anchor (`:42`);
- the Low-confidence VAS numbers (`:35-37`);
- the class-match codec;
- the Low-confidence speaker template;
- the app's +5 dB one-phone calibration (`app:App/Playground.swift:49-54`, r).

The precision bought real regression safety, and in spec 012 it found a 2.4 dB error, so it was not wasted. Features 010 and 011 were conveniences that got the full fidelity process (`CLAUDE.md:104-105`, r). From now on: no new modeling or precision work before 1.0. Spend fidelity effort on stage 0, the headroom default and the presets.

**2. Is the process sustainable, and what would you cut?** Not at under 5 h a week in its current form. It was built in a burst of 347 commits over 10 active days and then fell to 6 commits in 11 days (git log, r). One small feature (010) produced 27,802 words and 48 tasks (r). Agents wrote most of that, so the binding costs are the agent budget and the owner's decision rounds.
- **Cut:** the full flow for small features (tiering); separate convergence PRs; routine coach passes; new asserted relative timing budgets; the status log inside `CLAUDE.md`; per-phase owner stops (if the owner agrees).
- **Keep:** determinism and goldens, the design scripts with their ngspice and ITU cross-checks, mutation tests, traceability with its sub-minute job, and coverage on PRs.
- **Add, light:** an app pre-submission checklist.

**3. Is the positioning strong enough without a unit?** It is defensible for developers and audio enthusiasts, because every number traces to a public ID (`constitution.md:19-31`, r). EVP users judge by ear and by device behaviours (pauses, clipped onsets, slowed playback). Ways to strengthen it within the policy:
1. exact "this stage off" renders (`investigation-mode.md:204`, r) and an original-microphone A/B (O-3);
2. presets labelled "units differ" that span the registered ranges;
3. a short list of assumptions ranked by estimated audibility;
4. the standing invitation for a lent or donated unit.

Drop the "checked against a real unit" half of the current line (`decisions.md:9`, r). Avoid "authentic", "accurate", "validated", "chip-level" and "reproduces exactly". The brief already rejected the last three once (`brief.md:10`, r), so keep a copy check as a release step.

**4. What is the shortest credible path to a submission that passes review?** The § 5 plan, about 13 weeks. Guideline by guideline:
- **1.1.6** (missing from the handoff's list): neutral readings in units with context are the protection; an "entertainment" label is not (i).
- **4.2:** a persistent library, instant playback, scrub, trim, share sheet, and two working modes.
- **4.3:** the flip and the stage statuses visible within 30 s, with review notes pointing to the open engine and the manifest.
- **2.3:** no real-unit-check promise, no "EMF", no "detect".
- **5.2 / 2.3.7:** no trademark in the name, subtitle or keywords; the device named once with a notice.
- **Prerequisites:** a paid account, a privacy URL, interruption handling, permission strings.

**5. Which decisions are costly to reverse after launch?**
- the seller identity (individual or organization);
- the public name (search position; ratings survive a rename, i);
- stored-form and decoder compatibility for users' libraries (`rr_dr60.h:1177-1190`, r): settle it now with the one-line commitment;
- the case-file format and its version field;
- the rule that nothing shipped free goes behind a purchase (`decisions.md:32`, r);
- the claims posture of the first listing;
- whether raw input is kept (O-3), since recordings made without it cannot be re-rendered later.

Not costly: the C ABI for the app (static linking, i), the bundle ID (permanent but never seen by users; `app:project.yml:27`, r), the minimum iOS version, the paid shape.

**6. What are we not seeing?** The common cause: every earlier adviser read the engine repo, and nobody owned the engine/app boundary.
1. A main-thread lock hold drops recorded input (`app:App/EngineHost.swift:349-368`, r).
2. Recordings do not persist (no `rr_dr60_import`; r).
3. Interruptions are not handled (r).
4. Recording switches off on a rate mismatch (`app:App/Playground.swift:164-172`, r).
5. The engine is unpinned, and its version string cannot tell commits apart (r).
6. The phone is an uncharacterized stage 0 in front of every level-dependent stage (r).
7. The VAS default sits at one end of a reported unit spread, so with drop mode a silent session is mostly empty (r).
8. Same-family "independent" checks: script-versus-Rust agreement is a consistency check. Only outside tools (ngspice, ITU) found errors.
9. `CLAUDE.md` is a context tax on every agent session (observed).
10. Stale agent prompts still carry the superseded "measured" target (r).
11. Enrolment lead time and seller name appear nowhere in the product docs (r).
12. The product plan was never checked against an hour budget, and the Investigation design's one-device-sample timeline is finer than the ±10 – 20 ms sensor alignment it serves (`investigation-mode.md:69, 234`, r): the same over-precision pattern as the engine.
13. Four report structs lack a size field, not one (r).
14. Every "wait for capture" gate is permanent under the 2026-10-18 decision.

---

## 7. Limits of this review

- **One model family.** This was run as a multi-agent review (four area reviewers, each checked by a skeptic, then this merge), all on Claude, the same family as the project's earlier advisers. It is **not** an outside-family check. The findings most likely to hold are the ones checked against code, counts and git (A4, A9, A14, C1, C10, C11, D11, D14). Judgements about proportion and product are the most exposed to shared bias.
- **Not read:** `docs/guide.md`; `docs/hardware/sources.md` in full; most specs (only cited lines of 008, 010 and 012); `docs/product/competitors.md`; `docs/release-checklist.md`; most Rust source beyond the cited ranges; the app's tests and Swift files beyond the cited ranges; any build or test run. Nothing was re-measured: timing and coverage figures come from `CLAUDE.md` and CI logs.
- **Inferred, not checked:** Apple's guideline texts (1.1.6, 2.3.7, 3.1.2(a)), rename and ratings behaviour, D-U-N-S lead time, the `.measurement` versus `.default` processing difference, which rates iOS presents per route, how often the lock misses occur, the effect of the test hooks on arithmetic, the µ-law audibility estimate, compressed raw-audio sizes, and the 90-day hour estimates.
- **Evidence that would change verdicts:**
  - Bench logs with `lockMisses` greater than zero would raise change 2's priority further. A Bluetooth or USB session would confirm or retire A9.
  - Any real-unit VAS or quiet-room capture would let B2 be judged.
  - A third-party consumer before app 1.0 would move the ABI pass and a release artifact earlier (A3, A5, A6).
  - The owner's real hours per feature would test C1 – C3.
  - Apple's current guideline text could change O-16 and D6.

---

## 8. Question log

### Round 1 (answered by the owner, 2026-10-21)

1. *App repo access?* "Yes, full read access to `/Users/chance/Code/ic-recorder-app`."
2. *Time budget?* "Under 5 hours a week of the owner's own time for the next 90 days; agent (Claude Code) time is also bounded by subscription limits and attention."
3. *Target submission date?* "None fixed; 'quality first'."
4. *Revenue in year one?* "Not a goal in year one; the app is free."

Also stated: the owner will not buy an RR-DR60 but would accept a donated or lent unit; success means "an app in the store that people use"; the first store release should have both modes (Recorder and Investigation).

### Round 2: pending

Every verdict above uses the default shown.

1. **Before app 1.0, will anyone other than you link the engine (a third-party app, a binary release, crates.io)?**
   - *Why:* decides whether the ABI pass and a release artifact come before app 1.0.
   - *Affects:* A3, A5, A6, C11, § 3 ABI pass.
   - *Default:* No. The owner's app is the only consumer through app 1.0.
2. **Will you enrol in the Apple Developer Program as an individual (your legal name shown as seller) or as an organization (needs a D-U-N-S number and lead time)?**
   - *Why:* gates TestFlight and submission, and the seller name is public in a sensitive category.
   - *Affects:* D15, gate G0.
   - *Default:* Individual, started in week 1.
3. **May Investigation 1.0 ship with plain sensor tracks (the vibration track included), manual marks, a timeline and a case file, with automatic marks, the vibration-track marks and the "Inside this moment" cards in 1.1?**
   - *Why:* the largest scope lever in the 90 days, and it partly defers your 2026-10-21 vibration-mark decision.
   - *Affects:* D1, D8, O-4.
   - *Default:* Yes.
4. **Keep the original microphone audio for every recording in both modes (24-bit lossless, about 250 – 350 MB/h, deleted with the recording), and commit that recordings made from now on stay importable after engine updates?**
   - *Why:* settles O-3, enables the original-versus-recorded A/B, and protects testers' and users' libraries.
   - *Affects:* O-3, D12, A15, B4, privacy copy.
   - *Default:* Yes to both.
5. **Do you accept pausing engine work until after app 1.0 (specs 014 – 021, new modeling, new precision; the optional ngspice limiter check shelved), with at most a reduced-process 016-lite?**
   - *Why:* frees most of the 90 days for the app.
   - *Affects:* the largest question, controls Q5, Investigation D1, B14, B23, change 1.
   - *Default:* Yes.
6. **You confirmed the phase-by-phase cadence on 2026-10-12. At under 5 h a week, would you now approve only the spec (with its decisions) and the merge, with agents stopping only on a failed gate or a new decision? And what did you usually read per feature: the whole spec, or only the decisions and review findings?**
   - *Why:* the largest saving of owner time, and it reverses a stated preference. The reading answer tests whether 20 – 42k words per feature serve you.
   - *Affects:* C1, C3, C10.
   - *Default:* Yes; you read the decisions and findings only.
7. **Three product defaults:**
   - (a) Everything shipped free in 1.0 stays free for good, and later purchases only add components.
   - (b) Recorder mode is the plain device experience for every user.
   - (c) Investigation mode offers presets labelled "units differ", including one that keeps recording in a quiet room (for example level 5 with a hiss offset near +9 dB) alongside the factory default.

   Agree?
   - *Why:* settles O-1, O-15 and what most first sessions sound like.
   - *Affects:* O-1, O-2, O-5, O-15, D5, B2, change 9.
   - *Default:* Yes to all three.

Merged or dropped from the area reviews:
- Folded into the § 5 gates as measurements instead of questions: "Have you run Bluetooth or USB audio?" and "Has `lockMisses` read above zero?".
- Dropped, defaults kept: "Is the 45-minute CI wait felt?" (default: no, keep coverage on PRs) and "a bounded analysis of public recordings" (default: no, respecting the 2026-10-18 decision; B2 stays Can't judge).

### Round 2: the owner's answers (2026-10-29)

Asked one at a time after the report was written. Where an answer differs from the default, the verdicts and the § 5 plan that depended on the default need revisiting; this log records the answers and does not rewrite them.

| # | Answer | Matches the default? |
|---|---|---|
| 1 | No: only the owner's app links the engine before app 1.0. | Yes |
| 2 | **Organization** (an LLC or similar), not an individual. A D-U-N-S number and lead time come before TestFlight. | **No** (affects D15, gate G0) |
| 3 | Yes: the smaller Investigation 1.0; automatic marks, vibration-track marks and the "Inside this moment" cards in 1.1. | Yes |
| 4 | Yes to both: keep the original microphone audio for every recording, and recordings made from now on stay importable after engine updates. | Yes |
| 5 | **Engine first, then app.** After reading what specs 014 – 021 are, the owner chose to build them (or most of them) before the app, rather than pause engine work. | **No** (affects the largest question, § 5 and change ranking) |
| 6 | **Spec, plan and merge** sign-offs (three per feature, not two); agents stop in between only on a failed check or a new decision. The owner reads the decisions and findings only. | Partly (C1, C3, C10) |
| 7 | Yes to all three: free in 1.0 stays free; Recorder mode is the plain device experience; Investigation presets labelled "units differ", including one that keeps recording in a quiet room. | Yes |
