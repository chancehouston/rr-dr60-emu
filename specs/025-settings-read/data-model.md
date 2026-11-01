# Data Model: The Settings Read and Control Access by Index

- **Control mapping** (`control_map`): `const TARGETS` in manifest order, index (0 … 30) → `Target::Setting(SettingAccess)` (26 variants, one per settings field) or `Target::Live(LiveControl)` (route, volume, speed level, speed mode, battery). Exhaustive both ways (R-01).
- **ControlReading**: `Switch(bool)`, `Integer(u64)`, `Continuous(f32)`, `Choice(u32)`; C 24 bytes (R-02). The value type equals the manifest entry's.
- **Representability** (D3): type matches; switch 0/1; choice one of the options; continuous finite (−0.0 → 0.0); integer within the Rust field (u8 sensitivity, u32 memory fields, u64 seed); the host rate is a choice.
- **Applied settings** (D1): `Pipeline::settings()`; the last value set or landed on any stream.
- **Refusals** (D7, R-06): ordered; statuses 39 `NOT_A_SETTING`, 40 `NOT_A_PLAIN_LIVE_CONTROL`; `INVALID_ARGUMENT` for representability (`Error::InvalidControlValue`).
