//! Spec 029: prints the canonical text of the noise, bursts and silence report (D1), or with
//! `--write-noise <dir>` writes the steady noise rows at 8 kHz as s16le for the G.723.1 desk
//! check (plan R-07), or with `--rate <hz>` one rate's section.
use std::io::Write;

use rr_dr60_harness::nonspeech_checks as ns;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--write-noise" {
        let dir = std::path::Path::new(&args[2]);
        std::fs::create_dir_all(dir).unwrap();
        for st in [ns::Stimulus::A35, ns::Stimulus::A50, ns::Stimulus::A55] {
            let x = st.samples(8000);
            let mut f =
                std::fs::File::create(dir.join(format!("nonspeech_{}_8k.s16", st.id()))).unwrap();
            for v in x {
                f.write_all(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
                    .unwrap();
            }
        }
        return;
    }
    if args.len() == 3 && args[1] == "--rate" {
        print!("{}", ns::rate_text(args[2].parse().unwrap()));
        return;
    }
    print!("{}", ns::canonical_text());
}
