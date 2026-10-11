# Product decisions

A log of product decisions with the alternatives considered, the evidence, and the cost of reversing. Add a row when a decision is made or changed. Engineering decisions live in the specs.

| Date | Decision | Alternatives | Evidence | Reversal cost |
|---|---|---|---|---|
| 2026-10-11 | Positioning: "reproduces the RR-DR60's signal chain stage by stage, from the service manual, and shows which stages are verified" | "DR60 emulator" (taken by GhostTube EVP); generic EVP recorder | competitors.md; the sources and assumptions registers | low; copy only |
| 2026-10-11 | Free tier is the full device with the device's limits (60 min, 99 recordings); paid tier is only what the device cannot do | gate part of the chain; unlimited free; subscription | monetization.md; category review themes | raising a cap is cheap, lowering one is not |
| 2026-10-11 | One non-consumable purchase; no subscription, ads or consumables at launch | subscription; ads | App Store Review Guideline 3.1.2(a); competitor reviews | one-time to subscription for new content is possible; the reverse is not |
| 2026-10-11 | App name contains no trademark ("Panasonic", "RR-DR60", "DR60") and nothing confusingly similar; the device is named once in the description with a trademark notice | name with "DR60" | App Store Review Guidelines 2.3.7, 5.2.1; Google Play "Misleading claims" | renaming after launch loses ratings and search position |
| 2026-10-11 | No analytics or advertising SDK at launch; App Privacy and Data safety both "data not collected" | third-party analytics | Apple App Privacy details; Play Data safety (on-device processing is not collection) | low |
| 2026-10-11 | Live monitoring during recording is not offered as a device behaviour (the speaker amp is muted while recording, S-002); if offered later it is a labelled Workbench extra | live monitor in the free tier | S-002; Android latency variance | low |
| 2026-10-11 | The evidence program (community test recordings, a loaned unit, purchase as a last resort) is a product priority | engineering-only | without a verified stage the pitch is weaker than competitors' unsupported claims | n/a |
| 2026-10-11 | The engine is distributed as a Swift Package wrapping an XCFramework (and later an Android archive), not as a source port | source port | adoption speed is the library's value; a port forks the model | low now, high after the app exists |
| 2026-10-11 | The engine's next product-driven feature is a split between recording into an encoded, device-sized recording and playing it back through the speaker or earphone path | keep the single mic-to-speaker pass | the app's core loop is record, stop, play, share; the device mutes the speaker while recording | grows with every stage added before the split |
| 2026-10-11 | The vintage-gear audience stays in the copy but is not built for until evidence of it appears | build a dedicated mode | no evidence found | none |
| 2026-10-11 | Year-one goal is credibility: 1.0 ships fully free with the device's limits; the Workbench purchase follows in 1.1 after review mining | revenue from launch | monetization.md; the category's one-star themes are all about trust | low; adding a purchase later is routine |
| 2026-10-11 | Evidence program runs the free routes first (a published capture kit, requests to creators who own units); a purchase is considered only if those fail within about eight weeks | buy a unit now; never buy | second-hand asking prices EUR 1,700–3,500 (competitors.md) | none |
