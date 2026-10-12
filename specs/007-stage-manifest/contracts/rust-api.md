# Contract: Rust API additions (feature 007)

Additive (0.6 → 0.7; nothing released). No existing item changes.

## Module `rr_dr60::manifest` (public) and root re-exports

```text
pub fn manifest() -> &'static Manifest;                       // also rr_dr60::manifest()

#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manifest { pub header: Header, pub stages: [StageEntry; 15] }
impl Manifest {
    pub const STAGE_COUNT: usize = 15;                        // signal-chain.md rows
    pub fn header(&self) -> &Header;
    pub fn stages(&self) -> &[StageEntry];                    // in number order
    pub fn stage(&self, number: u32) -> Option<&StageEntry>;  // None for 0 or > 15
}

#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub library_version: &'static str,   // == rr_dr60::VERSION
    pub device_rate_hz: u32,             // 6000 (A-026)
    pub stage_count: u32,                // 15
    pub counts: StatusCounts,
    pub basis: &'static str,
    pub evidence_link: &'static str,
    pub assumptions_link: &'static str,
    pub sources_link: &'static str,
}
#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StatusCounts { pub not_modeled: u32, pub assumed: u32, pub sourced: u32, pub checked: u32 }

#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageEntry {
    pub number: u8,
    pub identifier: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub status: StageStatus,
    pub path: StagePath,
    pub confidence: Confidence,
    pub sources: &'static [&'static str],
    pub assumptions: &'static [&'static str],
    pub coverage: &'static str,          // "" when nothing is left out or the stage is not modeled
    pub controls: &'static str,          // "" when the engine offers no control or read-out
    pub model_version: &'static str,     // "" iff status == NotModeled
}

#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum StageStatus { NotModeled, Assumed, Sourced, Checked }
#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum StagePath { Record, Playback, Both }
#[non_exhaustive] #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Confidence { NotApplicable, Low, Medium, High }
```

Guarantees: `manifest()` returns the same `&'static` on every call; reading it allocates nothing, performs no I/O and takes no lock (it is a `static`); every field is identical on every supported platform (plain literals; the library version from `CARGO_PKG_VERSION`); the strings are `'static`; `Manifest`, `Header` and `StageEntry` are `Copy`, so a host (or the harness) can copy and mutate a value but never construct one (`#[non_exhaustive]`).

Hidden, for the FFI crate only: `#[doc(hidden)] pub mod __c { pub static HEADER: HeaderC; pub static STAGES: [StageEntryC; 15]; }` with every string as a NUL-terminated `&'static [u8]` and the ID lists as `&'static [&'static [u8]]`. Not part of the public API; may change without notice.

## Stability

- Stage numbers, identifiers and existing fields: breaking to renumber, rename or remove (MAJOR after 1.0).
- Adding a field to `Header` or `StageEntry`, or a variant to an enum: minor (`#[non_exhaustive]`; hosts match with a wildcard arm).
- Changing a name, description, note, status, confidence, IDs or model version: minor, recorded in the CHANGELOG (FR-015), and a re-bless of `golden/manifest-v1.txt` and the README table.

## Rustdoc

Every public item documents its spec rule; the module docs carry the claims-policy sentence and the four status definitions, and the crate docs gain one paragraph and the README's read loop as a doctest:

```rust
let m = rr_dr60::manifest();
println!("{} stages, {} checked against a real unit", m.header.stage_count, m.header.counts.checked);
for s in m.stages() {
    println!("{} {}: {:?} ({:?}) {}", s.number, s.name, s.status, s.confidence, s.description);
}
```
