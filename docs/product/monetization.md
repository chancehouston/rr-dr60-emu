# Monetization

> **Partly superseded 2026-10-15.** The current position is in [brief.md](brief.md) › Value and › Open questions. Still in force: the principle that paid content is only what the device cannot do, no ads, no consumables, the models table, and the copy rules. Superseded: the single "Workbench" purchase at $7.99 and its fixed contents (paid expansion is now packages of extra components with example chains and presets; the shape and price are open, O-2), and "bypass and parameter controls" as paid (the engine's intended user adjustments are free, O-1). The free tier now also includes both modes and sensor sessions. This page is kept as the 2026-10-11 record.

Decision as of 2026-10-11, to be revisited after the first review mining and interviews (see personas.md).

## Principle

Fidelity is the product. Everything the real RR-DR60 does is free, with the real device's limits. The paid tier contains only what the device could never do.

## Free tier: the device

- The full signal chain, recording and playback, through the modeled speaker or the modeled earphone output.
- Voice-activated pausing, the five sensitivity levels, SLOW/FAST playback speed, 10-second review (all documented device features; S-001, S-002). *(Engine 0.11.0 models neither SLOW/FAST nor the 10-second review; brief.md › Engine capabilities 1.)*
- The device's memory: 60 minutes and 99 recordings (S-001, S-002: 16 Mbit flash).
- Share one recording through the system share sheet (the device-rate render).
- The inside view: the chain rendered live, with each stage's status (sourced, assumed, not modeled, checked against a real unit) and a flat text fallback; it replaces a separate "About this model" screen (front-and-inside.md).
- No ads, no account, no analytics SDK at launch.

## Paid tier: "Workbench" (working name)

*Superseded 2026-10-15 as a catalog and a price; where each item now belongs is open (brief.md O-2, O-3).*

One non-consumable in-app purchase, $7.99 (to be tested between $4.99 and $9.99):

- Expanded memory beyond the device's 60 minutes and 99 recordings.
- Stage taps and a mixer (raw microphone, after the limiter, codec only, decoded before the speaker, earphone), including "Listen here" in a stage's card on the inside view. Combinations the device cannot produce are labelled as such.
- Retention of each recording's raw microphone capture, which "Listen here" needs: shown in the recording's details with a delete option and listed on the privacy label. The free tier keeps only the device-rate render.
- Bypass and parameter controls for each stage.
- Folders, search and bulk tagging.
- Export of stems and of the device-rate file.
- Review tools: A-B loop, band filter, zoomable waveform.
- Later, if evidence supports it: unit-to-unit variation (clock offset, noise seed).

## Models considered

| Model | Verdict | Reason |
|---|---|---|
| One-time non-consumable | adopted | matches a product with no content feed; the category's one-time apps sell at $9.99 |
| Subscription | not now | App Store Review Guideline 3.1.2(a) requires "ongoing value"; there is none until a second emulated device or cloud features exist |
| Ads | no | the category's loudest complaint; interstitials also interrupt the audio session |
| Consumables (credits, packs) | no | the refund pattern seen in weekly-plan competitors |

## What would change the decision

- Casual users hit the 60-minute cap quickly and leave: raise the cap to 60 minutes per session and gate the library instead.
- Creators will only pay monthly (review mining or the landing-page test says so): offer the Workbench as a cheap annual plan alongside the one-time price.
- A second emulated device ships: a bundle or an annual plan becomes honest.

## Copy rules for every paid or free feature

- Describe signal processing only: band-limiting, limiting, gating, codec artifacts, speaker response.
- Say "modeled on" until a stage is checked against a real unit; never "authentic", "identical", "measured", "closely matches".
- Never "capture", "reveal", "enhance", "communicate"; never "fake", "debunk", "entertainment only".
- Name tools by what they do ("band filter"), not by what a user might hope they do.
