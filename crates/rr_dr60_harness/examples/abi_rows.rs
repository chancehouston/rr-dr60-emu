//! Prints the ABI rows as the C smoke program prints them (`smoke --abi`; spec 022 research
//! R-07, SC-001): the 64-bit size table, the four defines, and one row per struct the library
//! fills, read on a ±0.125 square at 48 kHz through the default engine with the codec bypassed.
//! The first figure after a row's name is the struct's size as the C side reads it back (D11).
//!
//! Run: `cargo run -q -p rr_dr60_harness --example abi_rows`

fn main() {
    print!("{}", rr_dr60_harness::abi_checks::rows_at(48_000));
}
