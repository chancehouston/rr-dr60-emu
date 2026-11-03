# Data model: Host reads

- **`wording::Lists`** `{ stems: &[&str], phrases: &[&str], suffixes: &[&str] }`, with the constants `ENGINE_STRINGS` and `CLAIMS`.
- **`wording::Hit`** `{ entry: &'static str, start: usize, end: usize }`: the list entry and the matched byte range.
- **`meters::MeterField`** (`#[non_exhaustive]`) `{ identifier, unit, sentence: &'static str, kind: MeterKind, side: MeterSide }`.
  - `MeterKind`: `Maximum`, `Count` or `State`.
  - `MeterSide`: `Input` or `Output`.
  - `FIELDS: [MeterField; 13]` in `Meters`'s declaration order.
- **`meters::MeterCall`**: `OnePass`, `Record` or `Play`.
- **`StageStatus`, `StagePath`, `Confidence`**: `token()` and `label()`, both `&'static str`.
