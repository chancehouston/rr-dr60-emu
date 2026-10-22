# Decision review: handoff to the reviewer

**Date:** 2026-10-21 · **Owner:** Chance Houston · **Repository state:** https://github.com/chancehouston/rr-dr60-emu, branch `main` (engine 0.13.0; this handoff is in `docs/review/`) · **App:** private repo `ic-recorder-app` at d230979

You are a frontier model asked for a **final, independent review** of the decisions this project has made so far and the questions it has left open. The owner wants three answers:

1. **Did we make the right decisions?** For each major decision: keep it, change it, or revisit it later.
2. **What needs to change?** Ranked, with the cost of changing and the cost of not changing.
3. **What should happen next?** A concrete plan for the next 90 days.

The owner decides. You advise. Ask the owner questions whenever an answer would change your advice (see "How to ask questions").

---

## 1. The project in one page

**rr-dr60-emu** is an open-source (MIT) software emulator of the complete audio signal chain of the Panasonic RR-DR60, a late-1990s digital voice recorder. The chain runs from the microphone to the speaker. The device is well known among paranormal investigators as an "EVP recorder". The project's public voice is strictly neutral: it describes what the signal chain does to sound, and it neither claims nor debunks paranormal phenomena.

- **Engine:** a `no_std` Rust core (`crates/rr_dr60`), deterministic fixed-point and float DSP, real-time safe, with a stable C API (`crates/rr_dr60_ffi`, header `crates/rr_dr60_ffi/include/rr_dr60.h`) and a Swift package wrapping an XCFramework (`swift/`). The measurement harness and golden files are in `crates/rr_dr60_harness`.
- **Built so far:** 13 features through GitHub Spec-Kit (`specs/001` … `specs/013`). Ten of the fifteen signal-chain stages are modeled: the microphone and amplifier, the limiter, the codec filters and clamp, voice activation (VAS), a 4 kbit/s speech codec of the device's class (not its actual coder), a modeled memory with record and playback paths, the playback speed control, the volume control, and the speaker and earphone outputs. Stage 6 (noise reduction) is not modeled because research found no evidence that the device has it.
- **Evidence base:** a service manual and codec datasheets (`docs/hardware/sources.md`, 18 sources) and a register of 58 numbered assumptions (`docs/hardware/assumptions.md`, `A-###`). **No physical unit and no reference recordings.** On 2026-10-18 the owner put real-unit verification out of scope (constitution 1.2.0). The v1 target is a *modeled* character match.
- **Quality bar:** about 98 % line coverage (80 % gate), seven host sample rates, golden files with SHA guards, timing budgets, an iOS-device golden check (passed 2026-10-18), and CI on 12 jobs.
- **Product:** an iOS-first app (Android planned) built on the engine. One instrument with two modes. **Recorder mode** has a front panel and a "flip" to the working internals. **Investigation mode** is a field instrument that records synchronized phone sensor tracks. The free tier is generous; paid component packages are an idea, not a decision. The app exists as a prototype on the owner's phone: real-time playback through the engine, and a "Bench" tab for guided testing.
- **Who:** a solo developer, new to Spec-Kit, preparing a first App Store submission. Three advisory agents helped along the way: a Spec-Kit coach, a hardware-emulation reviewer, and a product specialist. They share one model family, so **do not treat their agreement as independent confirmation.**

## 2. Ground rules

- **Read only.** Do not change repository files. The one exception: write your report to `docs/review/2026-10-21-decision-review-report.md`, or return it as text if you cannot write files.
- **Cite what you read.** Give `path:line` or a section name for every factual claim about the project. Mark each conclusion as **read** (you checked the file) or **inferred** (you reasoned from summaries). Don't present an inference as a fact.
- **Classify each verdict.** Use one of:
  - **Keep:** right, and right now.
  - **Change:** wrong, or no longer right.
  - **Right decision, wrong time:** sound, but premature or late.
  - **Can't judge:** you need evidence the project doesn't have (say which).
- **Weigh proportion, not only correctness.** For a solo developer, a correct decision can still cost too much. Say when rigor exceeds what the product needs, and when it falls short.
- **Claims policy.** Any copy, positioning or feature advice must follow the neutral voice in `CLAUDE.md` › Tone and claims policy and constitution principle VI: describe the signal chain, never claim the device or the emulator captures, reveals or enhances anything, never debunk, and never market accuracy that hasn't been measured. If you think the policy itself is wrong for the product, say so as a finding. Don't write copy that breaks it.
- **Be specific.** Numbers, file names, dates. No generic advice ("consider your users").

### Fixed, unless you give a strong reason

- The neutral claims policy (above).
- MIT license and an open-source engine.
- Determinism (same input, settings and seed give bit-identical output on every platform) and real-time safety. Much of the test strategy relies on them.

### Open to challenge (everything else), including:

- Taking real-unit verification out of scope (2026-10-18).
- Spec-Kit's full eight-step flow for every feature, the three advisory agents, and the documentation volume.
- The 80 % coverage gate, seven host rates, golden files and timing guards: their cost compared with their value.
- Building the engine almost to completion before the app.
- The modeling choices themselves: for example the class-match codec, the speed mechanism, the defaults.
- Every product decision in `docs/product/decisions.md` and `docs/product/brief.md`.

## 3. How to ask questions

The owner expects questions and will answer them in the chat.

- **Ask in rounds.** At most 7 questions per round, numbered. Aim for 3 rounds or fewer.
- **Each question:** why you're asking, which verdicts it affects, and your default answer if the owner skips it.
- **Round 1, ask first:**
  - Can you read the private app repository, or only the engine repository?
  - What is the owner's time budget per week?
  - What is the target date for a first App Store submission, if there is one?
  - Is revenue a goal for year one?
- **Don't block.** If a question goes unanswered, continue with your stated default and mark the verdicts that depend on it.

## 4. Reading guide (in priority order)

The repository is large: 13 specs, each with a spec, plan, research notes and tasks. Read in this order, and skim specs only where a verdict needs them.

| # | File | Why |
|---|---|---|
| 1 | `CLAUDE.md` | Mission, policies, and a dated status log of every feature, with measurement lessons and open decisions. The densest single source. |
| 2 | `.specify/memory/constitution.md` | The seven principles and their governance (v1.2.0). |
| 3 | `README.md`, `docs/guide.md` | The public face and the per-stage user guide. |
| 4 | `docs/hardware/signal-chain.md` | The 15 stages and the status of each: known, assumed, not modeled. |
| 5 | `docs/hardware/assumptions.md`, `docs/hardware/sources.md` | The evidence registers: what is known and how confident the project is. |
| 6 | `docs/product/brief.md` | The current product source: agreed, standing and open items, O-1 – O-17. |
| 7 | `docs/product/decisions.md` | The product decision log, with alternatives and reversal costs. |
| 8 | `docs/product/controls-discussion.md` | Parameter kinds, the AI-control guardrails, a proposed spec order 014 – 021, seven owner questions (§ 6). |
| 9 | `docs/product/investigation-mode.md` | The sensor-session design and six owner questions (§ 6). |
| 10 | `docs/product/competitors.md`, `personas.md`, `monetization.md`, `front-and-inside.md` | Market context and the older product notes. Superseded parts are marked. |
| 11 | `CHANGELOG.md` | What changed in each version, including every assumption change. |
| 12 | `docs/hardware/research/reports/` | The research on stages 6, 14 and 15, with the owner's decisions. |
| 13 | `specs/NNN-*/spec.md` and `research.md` | Only where you need a feature's reasoning. 005 (limiter), 008 (codec) and 013 (speed) carry the most modeling judgment. |
| 14 | App repo `ic-recorder-app`: `README.md`, `CLAUDE.md`, `project.yml`, `App/` | The prototype, if you have access. |

## 5. The decisions to review

Group your verdicts by these areas. The list is a starting point. Add decisions you find that are missing from it.

### A. Architecture and engineering
- A `no_std` + `alloc` Rust core; a C API as the integration surface; a Swift package around an XCFramework instead of a Swift port.
- A fixed pipeline with a `#[non_exhaustive]` settings struct; a 120-byte C settings struct grown by appended fields; no struct-size field on `RrDr60PlayInfo` (a pre-1.0 ABI pass is pending).
- Device rate 6 kHz inside, seven host rates with resampling at the boundaries.
- Fixed-point integer arithmetic in the codec; determinism across six desktop targets and iOS. No Android target in CI yet.
- Semantic versioning at 0.x with frequent breaking changes; MSRV 1.85.

### B. Fidelity and modeling
- Each modeled stage and its defaults: the limiter at −1 dBFS, VAS at −40 dBFS, the class-match ACELP codec at 4 kbit/s, the microphone hiss at about −62 dBFS, the 20 ms volume transition, the speed control's resampling mechanism, and an output clamp offered as an emulator convenience, off by default.
- The traceability rule: every number traces to a source or a registered assumption.
- **Real-unit verification out of scope.** Is "modeled character match" a sufficient basis for the product's positioning?
- The order the stages were built in, and whether any modeled stage is beyond what users can hear.

### C. Process
- Spec-Kit's eight steps for every feature, with coach, hardware and product reviews, often at several points per feature.
- The test strategy: measurement tests at seven rates, golden files, mutation tests, design scripts in Python whose printed figures the Rust tests pin, and timing guards with ceilings for the CI runner.
- CI cost: the coverage job takes 30 – 48 minutes. Documentation volume: `CLAUDE.md` alone is a long status log.

### D. Product
- One instrument, two modes; the flip; skins later.
- A generous free tier; paid component packages (shape and price open); no ads, no consumables, no subscription at launch.
- No trademark in the app name; "data not collected" privacy labels; on-device processing.
- Investigation mode's sensor design, automatic-mark rules and case files.
- The AI-control surface: structured only, on-device, no audio sent to the model.
- iOS first (minimum iOS 26), Android planned.

## 6. The open questions to advise on

For each, give a recommendation and say what it depends on. The owner will decide.

- **`brief.md` O-1 – O-17,** especially the starred ones:
  - O-1: which controls are free;
  - O-2: the paid boundary;
  - O-3: raw microphone audio;
  - O-4: release sequencing;
  - O-15: Recorder mode's audience;
  - O-16: an "entertainment" disclaimer versus the copy rules;
  - O-17: App Review guidelines 4.2 and 4.3.
- **`controls-discussion.md` § 6, questions 1 – 7:** whether to reopen product questions now, the four parameter kinds, record-side timing, the AI surface, the order of specs 014 – 021, battery state, and pitch-preserving speed.
- **`investigation-mode.md` § 6, decisions 1 – 6,** and its evidence list.
- **`CLAUDE.md` › Open decisions:** the earphone follower's cut-off, a clip indicator, Android distribution, the pre-1.0 ABI pass, and µ-law modeling.
- **The largest question:** is the engine "complete enough"? The owner's earlier rule deferred product decisions until engine core development is complete. Should engine work stop, pause or continue, and what is the minimum engine for a store-ready 1.0?

## 7. Questions the owner especially wants answered

1. Was the depth of engine fidelity work proportionate to what users will hear and to the product's goals?
2. Is the process sustainable for one person, and what would you cut or simplify?
3. With no real unit, is the positioning ("modeled stage by stage on the service manual, with a public sources and assumptions register") strong enough against competitors who claim an emulation without evidence? What else would strengthen it within the claims policy?
4. What is the shortest credible path to a first App Store submission that passes review: guideline 4.2 (minimum functionality), 4.3 (spam), 2.3 (accurate metadata), 5.2 (trademarks)?
5. Which decisions are costly to reverse after launch, and need settling before it? Examples: the app name, pricing shape, what is free, the stored and case-file formats, the C ABI.
6. What are we not seeing? Risks, missing decisions, blind spots that came from relying on advisers of one model family.

## 8. The report you return

Use this outline. Lead with the answer and keep prose short. Tables are welcome.

1. **Summary:** at most 10 bullets. The overall verdict, the three most important changes, the next step.
2. **Decision verdicts:** one table.

   | # | Area | Decision | Verdict | Confidence (high/med/low) | Why, in one or two sentences | Cost to change now | Evidence (`path:line`, read or inferred) |
   |---|---|---|---|---|---|---|---|

3. **Open questions:** one row each: the recommendation, what it depends on, and its urgency (before 1.0, before the next spec, later).
4. **Changes, ranked:** what to change, why, the effort, and the risk of not changing.
5. **The next 90 days:** a sequence with gates and a definition of a store-ready 1.0.
6. **Answers to the six questions in § 7.**
7. **Limits of this review:** what you didn't read, what you inferred, what evidence would change your verdicts.
8. **Question log:** the questions you asked, the owner's answers, and the defaults you used.

## 9. After the review

The owner and Claude Code will evaluate your report together. They will check your citations against the files, and they will decide which recommendations become decision-log rows, brief updates or new specs. Write for that: every recommendation should be traceable to a reason and to a file the owner can check.
