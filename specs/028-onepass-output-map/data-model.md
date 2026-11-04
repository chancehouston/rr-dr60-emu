# Data model: the one-pass output map

No new stored data and no new C struct.

- **`InputAlignment`** (spec 020, `timeline`; C `RrDr60InputAlignment`, sized): `centre`, `first`, `last` (host input samples since the stream's last start), `clamped` (an end pulled to the stream's start). The one-pass read has no end clamp.
- **`VasEvent`** (spec 003; C `RrDr60VasEvent`, frozen 16 bytes): `output_position` on the one-pass's report clock (before the speech coder), `input_length` (drop: host input samples removed; mute: the region's length). Unchanged; its documentation is corrected.
- **`OnePassClock`** (crate-private, built per call): `l` (`latency_samples()`), `c` (the coder's delay when it runs, else 0), `drop`, `half` (the decimator's half-length), `codec` (the coder runs), and the rate plan's `l`/`m`.
