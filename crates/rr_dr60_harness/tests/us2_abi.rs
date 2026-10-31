//! Spec 022 User Stories 2 and 3 from the harness: the ABI check and the settings rule (US2
//! AS1 – AS2), the frozen pair (AS3), the core crates untouched (US3 AS1), the fifteen golden
//! guards unmoved (AS2), the header's diff limited to the listed additions (AS4), and the
//! Swift fixture pinned to the Rust rows.

use rr_dr60_ffi::*;
use rr_dr60_harness::abi_checks;
use std::process::Command;

/// US2 AS1: the check and the version, no handle.
#[test]
fn as1_check_and_version() {
    assert_eq!(rr_dr60_abi_version(), RR_DR60_ABI_VERSION);
    assert_eq!(RR_DR60_ABI_VERSION, 1);
    assert_eq!(
        rr_dr60_abi_check(1, 8, 120, 16, 40),
        RrDr60Status::Ok,
        "the right tuple"
    );
    for (t, why) in [
        ((0u32, 8usize, 120usize, 16usize, 40usize), "version 0"),
        ((u32::MAX, 8, 120, 16, 40), "version max"),
        ((1, 4, 120, 16, 40), "a 32-bit pointer"),
        ((1, 8, 121, 16, 40), "settings one above"),
        ((1, 8, 119, 16, 40), "settings one below"),
        ((1, 8, 120, 24, 40), "the event size"),
        ((1, 8, 120, 16, 48), "the entry size"),
        ((0, 4, 0, 0, 0), "all wrong"),
    ] {
        assert_eq!(
            rr_dr60_abi_check(t.0, t.1, t.2, t.3, t.4),
            RrDr60Status::AbiMismatch,
            "{why}"
        );
    }
    assert_eq!(RrDr60Status::AbiMismatch as u32, 37);
}

/// US2 AS2 (D13): the settings struct is validated whole at create, reconfigure and validate.
#[test]
fn as2_create_rule() {
    let ok = rr_dr60_settings_default(48_000);
    let mut h = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(&ok, &mut h) }, RrDr60Status::Ok);
    for size in [119u32, 121, 0, u32::MAX] {
        let mut s = ok;
        s.struct_size = size;
        let mut q = std::ptr::null_mut();
        let mut field = RrDr60SettingField::None;
        // SAFETY: valid pointers; a whole struct whatever its size field says.
        unsafe {
            assert_eq!(
                rr_dr60_create(&s, &mut q),
                RrDr60Status::InvalidArgument,
                "{size}"
            );
            assert!(q.is_null());
            assert_eq!(
                rr_dr60_reconfigure(h, &s),
                RrDr60Status::InvalidArgument,
                "{size}"
            );
            assert_eq!(
                rr_dr60_settings_validate(&s, &mut field),
                RrDr60Status::InvalidArgument
            );
        }
        assert_eq!(field, RrDr60SettingField::StructSize);
    }
    // SAFETY: a live handle; the same settings.
    assert_eq!(unsafe { rr_dr60_reconfigure(h, &ok) }, RrDr60Status::Ok);
    // SAFETY: destroying once.
    unsafe { rr_dr60_destroy(h) };
}

/// US2 AS3 (D2): the frozen pair's sizes and the `stream` name.
#[test]
fn as3_frozen_pair_pinned() {
    assert_eq!(size_of::<RrDr60VasEvent>(), RR_DR60_VAS_EVENT_SIZE as usize);
    assert_eq!(
        size_of::<RrDr60ChangeEntry>(),
        RR_DR60_CHANGE_ENTRY_SIZE as usize
    );
    assert_eq!(size_of::<rr_dr60::VasEvent>(), 16);
    assert_eq!(size_of::<rr_dr60::ChangeEntry>(), 40);
    assert_eq!(std::mem::offset_of!(RrDr60ChangeEntry, stream), 36);
    let e = RrDr60ChangeEntry {
        stream: rr_dr60::STREAM_RECORD,
        ..Default::default()
    };
    assert_eq!(e.stream, 3);
}

fn git(args: &[&str]) -> Option<std::process::Output> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
}

/// US3 AS1 (SC-003): the core crates are untouched since 0.21's merge.
#[test]
fn us3_core_untouched() {
    let Some(out) = git(&[
        "diff",
        "--quiet",
        "81f14dd",
        "--",
        "crates/rr_dr60",
        "crates/rr_dr60_detmath",
    ]) else {
        eprintln!("git unavailable: skipped");
        return;
    };
    if !out.status.success() && out.status.code() != Some(1) {
        eprintln!("git diff could not run (no 81f14dd?): skipped");
        return;
    }
    assert!(out.status.success(), "the core crates differ from 81f14dd");
}

/// US3 AS2 (SC-003): the fifteen golden guards read from the golden tests' sources are the
/// 0.21 figures.
#[test]
fn us3_guards_unmoved() {
    let want = [
        (
            "golden_agc.rs", // spec 001's guard lives there
            "c8ba66679bd074b955768be43c54553a3d4ffb5ab81048a6b34df26532c8ea78",
        ),
        (
            "golden_agc.rs",
            "009cc127e695d252ac4fd85abeacdacc53543f76030a0d5ceb4836362f51a17e",
        ),
        (
            "golden_vas.rs",
            "77306208a0bc0a43c78ff1fc8a4062f31a4d29aca0dc14a34460ff3f2f46948b",
        ),
        (
            "golden_codec.rs",
            "2fed69c7ebbe1ff96a06b604543ceabc41343f8f56f0a47b5bf7af9344af05f1",
        ),
        (
            "golden_output.rs",
            "3b64ae4429cf679b8d4c39e49b7bb2d333035b8d23306b80bc4146d6c805c2f4",
        ),
        (
            "golden_output_clamp.rs",
            "62c0ae7721d8cd1f55faba1eaad2fb1fd25f585974a44151e78c3fb233c77cbd",
        ),
        (
            "golden_volume_ramp.rs",
            "d5657f0e37fe46872a3cec883a5e706e0a5d063f04aa2a8f9d96b110cf5d18e4",
        ),
        (
            "golden_mic.rs",
            "70bc609516b80ceb785c2d905f24d8b88ecf64436b30b4092d297397a101f739",
        ),
        (
            "golden_speed.rs",
            "1ae181fb80fcab0d57c95a5ee005e00521379c79795f9bdf90a4e1ab9523c432",
        ),
        (
            "golden_meters.rs",
            "1394bbb9d7912ca560bc403e62965a569a08711dbc257ea65997fef43c5d2fe3",
        ),
        (
            "golden_changes.rs",
            "4d6294d0d28dbbb144bec075d76e6ca3cd7a5f85f07a20310f5d6c9e1f07447d",
        ),
        (
            "golden_record_changes.rs",
            "789be307db6581aa8ab6a9678d91d40cad1edf0e798978531c923dfd3e1e99e0",
        ),
        (
            "golden_battery.rs",
            "ecd1072f7f0f4c1e598fac2ecae4fe9d52face92c567c45da797b6789eaa07db",
        ),
        (
            "golden_record.rs",
            "026f1ca3aa98cdcaecef584adf604938550d6eab0fb7e2cde565ea3733e4c8ec",
        ),
        (
            "golden_bypass.rs",
            "c2cb096231380ec29a0f2ca3c815e360bceee8d70164558398769214ce06b1e6",
        ),
    ];
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/");
    for (file, sha) in want {
        let src = std::fs::read_to_string(format!("{dir}{file}")).unwrap();
        assert!(src.contains(sha), "{file}: the guard moved from {sha}");
    }
}

/// US3 AS4: the header differs from 0.21's by the listed additions only: two functions, one
/// status, four defines; no struct removed, none added.
#[test]
fn us3_header_diff_listed() {
    let Some(out) = git(&["show", "81f14dd:crates/rr_dr60_ffi/include/rr_dr60.h"]) else {
        eprintln!("git unavailable: skipped");
        return;
    };
    if !out.status.success() {
        eprintln!("81f14dd unavailable: skipped");
        return;
    }
    let old = String::from_utf8(out.stdout).unwrap();
    let new = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../rr_dr60_ffi/include/rr_dr60.h"
    ))
    .unwrap();
    let names = |src: &str, prefix: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter_map(|l| {
                let rest = l.strip_prefix(prefix)?;
                let name: String = rest
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                (!name.is_empty()).then_some(name)
            })
            .collect()
    };
    // A declaration line starts with its return type at column 0 and names `rr_dr60_x(`.
    let functions = |src: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter_map(|l| {
                let first = l.chars().next()?;
                if !(first.is_alphabetic() || first == '_') {
                    return None;
                }
                let i = l.find("rr_dr60_")?;
                let name: String = l[i..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                l[i + name.len()..].starts_with('(').then_some(name)
            })
            .collect()
    };
    let structs_old = names(&old, "typedef struct ");
    let structs_new = names(&new, "typedef struct ");
    assert_eq!(structs_old.len(), 21, "the twenty structs and the handle");
    assert_eq!(structs_old, structs_new, "no struct added or removed");
    let defines_old = names(&old, "#define RR_DR60_");
    let defines_new = names(&new, "#define RR_DR60_");
    let added: Vec<_> = defines_new.difference(&defines_old).cloned().collect();
    assert_eq!(
        added,
        [
            "ABI_VERSION",
            "CHANGE_ENTRY_SIZE",
            "MAX_STRUCT_SIZE",
            "VAS_EVENT_SIZE"
        ],
        "the four defines"
    );
    assert!(
        defines_old.difference(&defines_new).next().is_none(),
        "no define removed"
    );
    let f_old = functions(&old);
    let f_new = functions(&new);
    let added: Vec<_> = f_new.difference(&f_old).cloned().collect();
    assert_eq!(
        added,
        ["rr_dr60_abi_check", "rr_dr60_abi_version"],
        "the two functions"
    );
    assert!(
        f_old.difference(&f_new).next().is_none(),
        "no function removed"
    );
    assert!(
        new.contains("RR_DR60_STATUS_ABI_MISMATCH = 37"),
        "the status"
    );
    assert!(!old.contains("ABI_MISMATCH"));
    assert!(
        new.contains("uint32_t stream;") && old.contains("uint32_t reserved;"),
        "the rename"
    );
    assert!(
        new.contains("uintptr_t flush_stored;"),
        "the flattened outcome"
    );
}

/// The Swift fixture is the Rust rows at 48 kHz (the Swift test compares its C rows to it).
#[test]
fn swift_fixture_matches_rows() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../swift/Tests/RRDR60Tests/Fixtures/abi-rows-48000.txt"
    );
    let fixture = std::fs::read_to_string(path).expect("the committed fixture");
    assert_eq!(
        fixture,
        abi_checks::rows_at(48_000),
        "regenerate the fixture: cargo run -q -p rr_dr60_harness --example abi_rows > {path}"
    );
}
