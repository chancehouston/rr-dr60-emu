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

/// US3 AS1 (SC-003): the core crates are untouched between 0.21's merge and 0.22's (restated
/// by spec 023, which moves the core: the pin reads the two revisions, not the tree).
#[test]
fn us3_core_untouched() {
    let Some(out) = git(&[
        "diff",
        "--quiet",
        "81f14dd",
        "12a3c45",
        "--",
        "crates/rr_dr60",
        "crates/rr_dr60_detmath",
    ]) else {
        eprintln!("git unavailable: skipped");
        return;
    };
    if !out.status.success() && out.status.code() != Some(1) {
        eprintln!("git diff could not run (no 81f14dd or 12a3c45?): skipped");
        return;
    }
    assert!(
        out.status.success(),
        "the core crates differ between 81f14dd and 12a3c45"
    );
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
            "9b4c7a714220a8713470eea1b3e5329f63ff92800f60e2279a624aea40bf311e", // spec 024: the record golden re-blessed once (form 2)
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

/// US3 AS4: 0.22's header differs from 0.21's by the listed additions only: two functions, one
/// status, four defines; no struct removed, none added (restated by spec 023, which appends to
/// the header: both headers are read from their merge revisions).
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
    let Some(out) = git(&["show", "12a3c45:crates/rr_dr60_ffi/include/rr_dr60.h"]) else {
        return;
    };
    if !out.status.success() {
        eprintln!("12a3c45 unavailable: skipped");
        return;
    }
    let new = String::from_utf8(out.stdout).unwrap();
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
    // Every differing non-comment line is one of the listed changes (T012, SC-003): the size
    // fields, the flattened outcome, the rename, the four defines, the two functions, the
    // status, the version, and the amended contract comment; doc comments may say anything.
    let code_lines = |src: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .map(str::trim_end)
            .filter(|l| {
                let t = l.trim_start();
                !t.is_empty() && !t.starts_with("//") && !t.starts_with('*') && !t.starts_with("/*")
            })
            .map(str::to_owned)
            .collect()
    };
    let (o, n) = (code_lines(&old), code_lines(&new));
    let allowed = [
        "struct_size",
        "flush_",
        "struct RrDr60RecordInfo flush;",
        "uint32_t stream;",
        "uint32_t reserved;",
        "RR_DR60_ABI_VERSION",
        "RR_DR60_MAX_STRUCT_SIZE",
        "RR_DR60_VAS_EVENT_SIZE",
        "RR_DR60_CHANGE_ENTRY_SIZE",
        "rr_dr60_abi_check",
        "rr_dr60_abi_version",
        "ABI_MISMATCH",
        "RR_DR60_VERSION_MINOR",
        "uintptr_t pointer_size",
        "uintptr_t settings_size",
        "uintptr_t vas_event_size",
        "uintptr_t change_entry_size",
        "sizeof(RrDr60Settings)",
    ];
    let unmatched: Vec<&String> = o
        .symmetric_difference(&n)
        .filter(|l| !allowed.iter().any(|a| l.contains(a)))
        .collect();
    assert!(
        unmatched.is_empty(),
        "header lines outside the listed changes: {unmatched:#?}"
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

/// Spec 025 D8 (tasks.md T010): the settings read moved no constant and no existing size (the
/// sizes listed here are the ones the feature sits beside; spec 022's `sizes_are_pinned` and the
/// promise script with 0.22 and 0.90 cover every other struct).
#[test]
fn spec_025_moved_nothing() {
    // Restated 2026-11-01 (spec 024 D5): 31 → 36 controls, 142 → 158; spec 025 moved neither.
    assert_eq!(rr_dr60::controls::CONTROL_COUNT, 36);
    assert_eq!(rr_dr60::changes::MAX_CHANGES_PER_CALL, 158);
    assert_eq!(RR_DR60_MAX_CHANGES_PER_CALL, 158);
    assert_eq!(RR_DR60_ABI_VERSION, 1);
    assert_eq!(size_of::<RrDr60Settings>(), 120);
    assert_eq!(size_of::<RrDr60Control>(), 120);
    assert_eq!(size_of::<RrDr60ControlOption>(), 24);
    assert_eq!(size_of::<RrDr60ControlReading>(), 24);
    assert_eq!(RrDr60Status::InvalidSpeedMode as u32, 38);
}

/// Spec 026 D7 (FR-007, SC-003): from 0.92's merge (2a170d4) to the working header, every
/// function, status and type is kept and the additions are exactly the ten reads, the one
/// struct and the five statuses; `RrDr60MeterField` stays out of the frozen `--abi` rows.
#[test]
fn spec_026_appended_only() {
    let Some(out) = git(&["show", "2a170d4:crates/rr_dr60_ffi/include/rr_dr60.h"]) else {
        eprintln!("git unavailable: skipped");
        return;
    };
    if !out.status.success() {
        eprintln!("2a170d4 unavailable: skipped");
        return;
    }
    let old = String::from_utf8(out.stdout).unwrap();
    // Restated by spec 027 (spec 023's lesson): the two revisions, 0.92's and 0.93's merges, so a
    // later feature's additions do not move this pin.
    let Some(out) = git(&["show", "3890bfe:crates/rr_dr60_ffi/include/rr_dr60.h"]) else {
        return;
    };
    if !out.status.success() {
        eprintln!("3890bfe unavailable: skipped");
        return;
    }
    let new = String::from_utf8(out.stdout).unwrap();
    let functions = |src: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter(|l| {
                l.starts_with("RrDr60Status rr_dr60_") || l.starts_with("uint32_t rr_dr60_")
            })
            .filter_map(|l| {
                let i = l.find("rr_dr60_")?;
                Some(
                    l[i..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect(),
                )
            })
            .collect()
    };
    let statuses = |src: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter_map(|l| l.trim().strip_prefix("RR_DR60_STATUS_"))
            .map(|r| {
                r.chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect()
            })
            .collect()
    };
    let types = |src: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter_map(|l| l.strip_prefix("typedef struct "))
            .map(|r| r.chars().take_while(|c| c.is_alphanumeric()).collect())
            .collect()
    };
    let added = |f: &dyn Fn(&str) -> std::collections::BTreeSet<String>| {
        let (o, n) = (f(&old), f(&new));
        assert!(
            o.is_subset(&n),
            "removed: {:?}",
            o.difference(&n).collect::<Vec<_>>()
        );
        n.difference(&o).cloned().collect::<Vec<_>>()
    };
    let mut fns = added(&functions);
    fns.sort();
    assert_eq!(
        fns,
        [
            "rr_dr60_manifest_word",
            "rr_dr60_meter_field",
            "rr_dr60_meter_field_count",
            "rr_dr60_meter_lag_samples",
            "rr_dr60_meter_reached",
            "rr_dr60_one_pass_drain_samples",
            "rr_dr60_one_pass_report_latency_samples",
            "rr_dr60_settings_one_pass_codec_runs",
            "rr_dr60_settings_one_pass_vas_runs",
            "rr_dr60_wording_first_hit",
        ]
    );
    assert_eq!(
        added(&statuses),
        [
            "INVALID_UTF8",
            "NO_SUCH_MANIFEST_WORD",
            "NO_SUCH_METER_CALL",
            "NO_SUCH_METER_FIELD",
            "NO_SUCH_WORDING_KIND",
        ]
    );
    assert_eq!(added(&types), ["RrDr60MeterField"]);
    assert_eq!(RrDr60Status::NoSuchManifestWord as u32, 48);
    let rows = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../swift/Tests/RRDR60Tests/Fixtures/abi-rows-48000.txt"
    ))
    .unwrap();
    assert!(!rows.contains("MeterField") && !rows.contains("meter_field"));
}

/// Spec 027 D8 (FR-008, SC-003): from 0.93's merge (3890bfe) to 0.94's merge (ad1ba8d), every
/// function, status and type is kept and the additions are exactly the session's functions,
/// statuses and types; `RrDr60MeterField` stays out of the frozen `--abi` rows. Restated by
/// spec 028 (spec 023's lesson): two revisions, so the next feature's additions do not move it.
#[test]
fn spec_027_appended_only() {
    let Some(out) = git(&["show", "3890bfe:crates/rr_dr60_ffi/include/rr_dr60.h"]) else {
        eprintln!("git unavailable: skipped");
        return;
    };
    if !out.status.success() {
        eprintln!("3890bfe unavailable: skipped");
        return;
    }
    let old = String::from_utf8(out.stdout).unwrap();
    let Some(new) = git(&[
        "show",
        "ad1ba8d401228c91480f12bd8ff3df68ac489e7f:crates/rr_dr60_ffi/include/rr_dr60.h",
    ])
    .filter(|o| o.status.success()) else {
        eprintln!("ad1ba8d unavailable: skipped");
        return;
    };
    let new = String::from_utf8(new.stdout).unwrap();
    let functions = |src: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter(|l| {
                l.starts_with("RrDr60Status rr_dr60_") || l.starts_with("uint32_t rr_dr60_")
            })
            .filter_map(|l| {
                let i = l.find("rr_dr60_")?;
                Some(
                    l[i..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect(),
                )
            })
            .collect()
    };
    let statuses = |src: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter_map(|l| l.trim().strip_prefix("RR_DR60_STATUS_"))
            .map(|r| {
                r.chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect()
            })
            .collect()
    };
    let types = |src: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter_map(|l| l.strip_prefix("typedef struct "))
            .map(|r| r.chars().take_while(|c| c.is_alphanumeric()).collect())
            .collect()
    };
    let added = |f: &dyn Fn(&str) -> std::collections::BTreeSet<String>| {
        let (o, n) = (f(&old), f(&new));
        assert!(
            o.is_subset(&n),
            "removed: {:?}",
            o.difference(&n).collect::<Vec<_>>()
        );
        n.difference(&o).cloned().collect::<Vec<_>>()
    };
    let mut fns = added(&functions);
    fns.sort();
    let want: Vec<String> = [
        "rr_dr60_fill_to_remaining",
        "rr_dr60_rerender_destroy",
        "rr_dr60_rerender_export",
        "rr_dr60_rerender_export_len",
        "rr_dr60_rerender_feed_f32",
        "rr_dr60_rerender_feed_i24",
        "rr_dr60_rerender_finish",
        "rr_dr60_rerender_into_pipeline",
        "rr_dr60_rerender_meters",
        "rr_dr60_rerender_new",
        "rr_dr60_rerender_plan_clone",
        "rr_dr60_rerender_plan_component",
        "rr_dr60_rerender_plan_control_value",
        "rr_dr60_rerender_plan_counter",
        "rr_dr60_rerender_plan_destroy",
        "rr_dr60_rerender_plan_edited",
        "rr_dr60_rerender_plan_flags",
        "rr_dr60_rerender_plan_from_record",
        "rr_dr60_rerender_plan_insert_row",
        "rr_dr60_rerender_plan_parameter",
        "rr_dr60_rerender_plan_remaining_samples",
        "rr_dr60_rerender_plan_remove_row",
        "rr_dr60_rerender_plan_row",
        "rr_dr60_rerender_plan_row_count",
        "rr_dr60_rerender_plan_seed",
        "rr_dr60_rerender_plan_set_collect_run",
        "rr_dr60_rerender_plan_set_component",
        "rr_dr60_rerender_plan_set_control_value",
        "rr_dr60_rerender_plan_set_counter",
        "rr_dr60_rerender_plan_set_parameter",
        "rr_dr60_rerender_plan_set_remaining_samples",
        "rr_dr60_rerender_plan_set_row",
        "rr_dr60_rerender_plan_set_seed",
        "rr_dr60_rerender_progress",
        "rr_dr60_rerender_record_len",
        "rr_dr60_rerender_record_write",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    // `void` destroys are not `RrDr60Status`/`uint32_t` declarations: read them separately.
    let voids: Vec<String> = new
        .lines()
        .filter_map(|l| l.strip_prefix("void rr_dr60_rerender_"))
        .map(|r| format!("rr_dr60_rerender_{}", r.split('(').next().unwrap()))
        .collect();
    let mut got = fns.clone();
    got.extend(voids);
    got.sort();
    got.dedup();
    assert_eq!(got, want);
    assert_eq!(
        added(&statuses),
        [
            "FEED_KIND_MIXED",
            "HOST_RATE_FIXED",
            "IMPORTED_RECORD",
            "INPUT_SHORT",
            "INVALID_SAMPLE",
            "RERENDER_STATE",
            "RUN_NOT_COLLECTED",
            "RUN_TOO_LARGE",
            "UNKNOWN_COMPONENT",
        ]
    );
    assert_eq!(
        added(&types),
        [
            "RrDr60ReRender",
            "RrDr60ReRenderFed",
            "RrDr60ReRenderOutcome",
            "RrDr60ReRenderPlan"
        ]
    );
    assert_eq!(RrDr60Status::UnknownComponent as u32, 57);
    let rows = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../swift/Tests/RRDR60Tests/Fixtures/abi-rows-48000.txt"
    ))
    .unwrap();
    assert!(!rows.contains("MeterField") && !rows.contains("meter_field"));
}

/// Spec 028 D6 (FR-006, SC-004): from 0.94's merge (ad1ba8d) to the working header, every
/// function, status and type is kept and the additions are exactly the two one-pass map reads,
/// with no status and no type.
#[test]
fn spec_028_onepass_map_appended_only() {
    let Some(out) = git(&[
        "show",
        "ad1ba8d401228c91480f12bd8ff3df68ac489e7f:crates/rr_dr60_ffi/include/rr_dr60.h",
    ])
    .filter(|o| o.status.success()) else {
        eprintln!("ad1ba8d unavailable: skipped");
        return;
    };
    let old = String::from_utf8(out.stdout).unwrap();
    let new = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../rr_dr60_ffi/include/rr_dr60.h"
    ))
    .unwrap();
    let names = |src: &str, prefix: &str| -> std::collections::BTreeSet<String> {
        src.lines()
            .filter_map(|l| {
                let i = l.find(prefix)? + prefix.len();
                Some(
                    l[i..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect(),
                )
            })
            .collect()
    };
    for prefix in ["rr_dr60_", "RR_DR60_STATUS_", "typedef struct "] {
        let (o, n) = (names(&old, prefix), names(&new, prefix));
        assert!(
            o.is_subset(&n),
            "removed: {:?}",
            o.difference(&n).collect::<Vec<_>>()
        );
        let added: Vec<_> = n.difference(&o).cloned().collect();
        let want: &[&str] = if prefix == "rr_dr60_" {
            &[
                "one_pass_input_at_output",
                "one_pass_splice_output_position",
            ]
        } else {
            &[]
        };
        assert_eq!(added, want, "{prefix}");
    }
}
