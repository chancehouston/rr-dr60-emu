//! Prints the control manifest in its canonical text form (spec 014 FR-012). The C smoke
//! program prints the same bytes (`smoke --controls`).
//!
//! Run: `cargo run -q -p rr_dr60_harness --example control_manifest`

fn main() {
    print!(
        "{}",
        rr_dr60_harness::controls_checks::render_controls(rr_dr60::controls())
    );
}
