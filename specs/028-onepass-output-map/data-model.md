# Data model: the one-pass output map

No new stored data and no new C struct.

- **`InputAlignment`** (spec 020, `timeline`; C `RrDr60InputAlignment`, sized): `centre`, `first`, `last` (host input samples since the stream's last start), `clamped` (an end pulled to the stream's start). The one-pass read has no end clamp.
- **`VasEvent`** (spec 003; C `RrDr60VasEvent`, frozen 16 bytes): `output_position` where the sample after the splice (or the region's first muted sample) enters the playback half; heard `c + l_post` later, `input_length` (drop: host input samples removed; mute: the region's length). Unchanged; its documentation is corrected.
- **`OnePassClock`** (crate-private, built per call): `latency` (`latency_samples()`), `c` (the coder's delay when it runs, else 0), `l_pre` (the record path's latency to the encoder's input), `l_post` (`latency − c − l_pre`, the playback half; equal to `playback_latency_samples()`), `drop`, `half` (the decimator's half-length), `codec` (the coder runs), `rate_l`/`rate_m` (the rate plan's ratio). The heard point of a pause report is `S(e) = e.output_position + c + l_post`.
