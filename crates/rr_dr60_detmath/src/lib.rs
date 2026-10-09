//! Deterministic math for the RR-DR60 emulator.
//!
//! Every function in this crate is built only from IEEE-754 basic operations
//! (`+ − × ÷` and comparisons), which are correctly rounded on every supported
//! platform. The results are therefore bit-identical on Linux, macOS, Windows
//! and iOS, on both x86-64 and ARM64 (research.md R-04, R-06; spec FR-014).
//!
//! The platform math library (`f64::sin`, `f64::exp`, …) is deliberately not
//! used: its results differ in the last bits between operating systems, which
//! would break the single set of golden files.
#![no_std]
#![forbid(unsafe_code)]
