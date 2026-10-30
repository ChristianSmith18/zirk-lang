//! Differential evidence for the browser interpreter: every valid program in
//! the native end-to-end corpus either reproduces its recorded native output
//! through the browser `run` operation, or appears in `EXCLUDED` with the
//! browser capability that explains why it cannot run there.
//!
//! The exclusion list is exhaustive in both directions. A program that starts
//! passing must leave the list, and a program that fails without being listed
//! fails this test, so the browser claim cannot drift from the evidence.

use std::fs;
use std::path::PathBuf;

use zirk_browser::contract::{Completion, FailureKind, Request};

/// Programs the browser deliberately cannot run, with the feature label the
/// capability diagnostic must name.
const EXCLUDED: &[(&str, &str)] = &[
    ("app_shutdown_grace_configured", "managed background work"),
    (
        "background_error_handler_failure_contained",
        "managed background work",
    ),
    (
        "background_error_handler_receives_throwable",
        "managed background work",
    ),
    ("atomic_managed_operations", "type Atomic"),
    ("atomic_supported_type", "type Atomic"),
    ("concurrent_dataflow", "job handles"),
    (
        "concurrent_detach_capacity_and_reuse",
        "managed background work",
    ),
    (
        "concurrent_detach_runs_and_drains",
        "managed background work",
    ),
    ("concurrent_fanout", "job handles"),
    ("concurrent_each_caller_cancel_joins", "type Job"),
    (
        "concurrent_each_fail_fast_cancels_and_joins",
        "managed completion outcomes",
    ),
    (
        "concurrent_each_invalid_before_admission",
        "managed completion outcomes",
    ),
    ("concurrent_each_ordered", "managed completion outcomes"),
    ("concurrent_each_settled_caller_cancel_joins", "type Job"),
    (
        "concurrent_each_settled_ordered",
        "managed completion outcomes",
    ),
    ("concurrent_first_all_fail_suppressed", "timers"),
    ("concurrent_first_cleanup_failure", "job handles"),
    ("concurrent_first_completion_order", "job handles"),
    ("concurrent_first_failure_then_success", "timers"),
    ("concurrent_first_tie_input_order", "job handles"),
    ("concurrent_within_first_caller_cancellation", "timers"),
    (
        "concurrent_within_first_roots_managed_winner",
        "job handles",
    ),
    ("concurrent_within_first_timeout", "job handles"),
    ("concurrent_within_first_timeout_joins_cleanup", "timers"),
    ("concurrent_within_first_winner", "job handles"),
    ("concurrent_within_negative_duration", "job handles"),
    ("concurrent_within_settled_success", "job handles"),
    ("concurrent_within_settled_timeout", "job handles"),
    ("concurrent_within_settled_timeout_joins_cleanup", "timers"),
    (
        "concurrent_settled_failure_channels",
        "managed completion outcomes",
    ),
    (
        "concurrent_settled_nested_payload",
        "managed completion outcomes",
    ),
    ("concurrent_settled_success", "managed completion outcomes"),
    ("concurrent_sibling_failure", "timers"),
    ("concurrent_spawn_loop", "structured concurrency"),
    ("concurrent_task_capacity_ceiling", "structured concurrency"),
    ("dependent_local_only", "type Dependent"),
    ("extern_call", "`unsafe` journal blocks"),
    ("instant_now", "extern `zirk_rt_instant_now`"),
    ("native_slice_basic", "`unsafe` journal blocks"),
    (
        "native_slice_bounds_error_caught",
        "`unsafe` journal blocks",
    ),
    ("mutex_with_exception_unlock", "type Mutex"),
    ("mutex_with_mutates_payload", "type Mutex"),
    ("parallel_adapter", "`parallel` regions"),
    ("parallel_cores_option", "`parallel` regions"),
    ("parallel_for", "`parallel` regions"),
    ("application_spawn_service_capacity_catch", "ServiceReady"),
    ("application_spawn_service_lambda", "ServiceStart"),
    ("application_spawn_service_named", "ServiceReady"),
    (
        "application_spawn_service_ready_lifecycle_error",
        "ServiceStart",
    ),
    ("phase_4e_surface", "type Dependent"),
    ("pin_automatic_unpin", "type Pin"),
    ("pointer_from_class_field", "`unsafe` journal blocks"),
    ("pointer_from_class_field_value", "`unsafe` journal blocks"),
    ("pointer_from_nested_class_field", "`unsafe` journal blocks"),
    ("pointer_offset_cast", "`unsafe` journal blocks"),
    ("pointer_read_write", "`unsafe` journal blocks"),
    ("slice_extent_ok", "`unsafe` journal blocks"),
    ("timer_every_stops", "structured concurrency"),
    ("timer_sleep_yields", "timers"),
    ("thread_run_managed_result", "managed native threads"),
    ("thread_run_throwable", "managed native threads"),
    ("transfer_resource", "resource transfer"),
    ("typed_channel_basic", "type Channel"),
    ("universal_to_string", "type Channel"),
    ("no_entrypoint", "file-body entrypoints"),
    (
        "unsafe_journal_commit_durable_before_commit_block",
        "`unsafe` journal blocks",
    ),
    (
        "unsafe_journal_commit_on_success",
        "`unsafe` journal blocks",
    ),
    (
        "unsafe_journal_local_declared_inside_not_journaled",
        "`unsafe` journal blocks",
    ),
    (
        "unsafe_journal_rollback_on_break",
        "`unsafe` journal blocks",
    ),
    (
        "unsafe_journal_rollback_on_continue",
        "`unsafe` journal blocks",
    ),
    (
        "unsafe_journal_rollback_on_error",
        "`unsafe` journal blocks",
    ),
    (
        "unsafe_journal_rollback_on_return",
        "`unsafe` journal blocks",
    ),
    (
        "unsafe_journal_rollback_two_writes_reverse_order",
        "`unsafe` journal blocks",
    ),
];

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../zirk-cli/tests/corpus/valid")
}

#[test]
fn native_corpus_runs_in_the_browser_or_is_explicitly_excluded() {
    let mut programs: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("corpus directory")
        .map(|entry| entry.expect("corpus entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "zrk"))
        .collect();
    programs.sort();

    let mut problems = Vec::new();
    let mut passed = 0usize;
    let mut excluded_seen = 0usize;
    for path in programs {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let expected_path = path.with_extension("out");
        let Ok(expected) = fs::read_to_string(&expected_path) else {
            continue; // Not a runnable end-to-end program.
        };
        let source = fs::read_to_string(&path).expect("corpus source");
        let response = zirk_browser::run(Request {
            api_version: zirk_browser::API_VERSION,
            trace: None,
            source,
            virtual_name: format!("corpus/{name}.zrk"),
            limits: None,
        });
        let output: String = response
            .output
            .iter()
            .map(|event| event.text.as_str())
            .collect();
        let excluded = EXCLUDED.iter().find(|(excluded, _)| *excluded == name);
        match (excluded, &response.failure) {
            (None, None) => {
                if response.completion != Some(Completion::Completed) || output != expected {
                    problems.push(format!(
                        "{name}: browser output differs from native\nexpected: {expected:?}\nactual:   {output:?}\ncompletion: {:?}\ndiagnostics: {:?}",
                        response.completion,
                        response.diagnostics.iter().map(|diagnostic| (&diagnostic.code, &diagnostic.message)).collect::<Vec<_>>()
                    ));
                } else {
                    passed += 1;
                }
            }
            (None, Some(failure)) => problems.push(format!(
                "{name}: unlisted browser failure {:?} {}: {}",
                failure.kind, failure.code, failure.message
            )),
            (Some((_, label)), Some(failure)) => {
                if failure.kind != FailureKind::Capability || !failure.message.contains(label) {
                    problems.push(format!(
                        "{name}: expected a capability failure naming `{label}`, got {:?} {}: {}",
                        failure.kind, failure.code, failure.message
                    ));
                } else if !output.is_empty() {
                    problems.push(format!(
                        "{name}: capability failure must not run the program"
                    ));
                } else {
                    excluded_seen += 1;
                }
            }
            (Some(_), None) => problems.push(format!(
                "{name}: now runs in the browser; remove it from EXCLUDED"
            )),
        }
    }

    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    assert_eq!(excluded_seen, EXCLUDED.len());
    assert!(passed > 150, "unexpectedly small corpus: {passed}");
}

#[test]
fn structural_open_template_generic_contract_demo_runs_in_browser() {
    let response = zirk_browser::run(Request {
        api_version: zirk_browser::API_VERSION,
        trace: None,
        source: include_str!("../../../demo/42-uniones-estructurales-y-plantillas-abiertas.zrk")
            .to_owned(),
        virtual_name: "demo/42-uniones-estructurales-y-plantillas-abiertas.zrk".into(),
        limits: None,
    });
    let output = response
        .output
        .iter()
        .map(|event| event.text.as_str())
        .collect::<String>();
    assert_eq!(response.failure, None, "{:?}", response.diagnostics);
    assert_eq!(response.completion, Some(Completion::Completed));
    assert_eq!(output, "number 42\ntext ready\nflag true\nAda:8\n42\n");
}

#[test]
fn object_model_catalog_evidence_exists_and_agrees_with_the_exclusions() {
    #[derive(serde::Deserialize)]
    struct Catalog {
        #[serde(rename = "schemaVersion")]
        schema_version: u32,
        constructs: Vec<Construct>,
    }
    #[derive(serde::Deserialize)]
    struct Construct {
        id: String,
        verdict: String,
        fixtures: Vec<String>,
        corpus: Vec<String>,
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let catalog: Catalog = serde_json::from_str(
        &fs::read_to_string(root.join("oop-catalog.json")).expect("object-model catalog"),
    )
    .expect("valid catalog");
    assert_eq!(catalog.schema_version, 1);

    let mut ids = std::collections::BTreeSet::new();
    let mut problems = Vec::new();
    for construct in &catalog.constructs {
        if !ids.insert(construct.id.clone()) {
            problems.push(format!("{}: duplicate construct id", construct.id));
        }
        for fixture in &construct.fixtures {
            if !root
                .join("fixtures")
                .join(format!("{fixture}.json"))
                .is_file()
            {
                problems.push(format!("{}: missing fixture {fixture}", construct.id));
            }
        }
        for program in &construct.corpus {
            if !corpus_dir().join(format!("{program}.zrk")).is_file() {
                problems.push(format!(
                    "{}: missing corpus program {program}",
                    construct.id
                ));
            }
        }
        let excluded = |program: &String| EXCLUDED.iter().any(|(name, _)| name == program);
        match construct.verdict.as_str() {
            "supported" => {
                if construct.fixtures.is_empty() && construct.corpus.is_empty() {
                    problems.push(format!("{}: supported without evidence", construct.id));
                }
                if let Some(program) = construct.corpus.iter().find(|program| excluded(program)) {
                    problems.push(format!(
                        "{}: supported but its evidence {program} is a browser exclusion",
                        construct.id
                    ));
                }
            }
            "excluded" => {
                if !construct.fixtures.is_empty() || !construct.corpus.iter().all(excluded) {
                    problems.push(format!(
                        "{}: excluded constructs may only cite excluded corpus programs",
                        construct.id
                    ));
                }
            }
            other => problems.push(format!("{}: unknown verdict {other}", construct.id)),
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn fixture_list_covers_every_fixture_file() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut on_disk: Vec<String> = fs::read_dir(root.join("fixtures"))
        .expect("fixtures directory")
        .map(|entry| {
            entry
                .expect("fixture entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".json"))
        .collect();
    on_disk.sort();

    // `FIXTURES` in `src/lib.rs` names each fixture through `include_str!`.
    let library = fs::read_to_string(root.join("src/lib.rs")).expect("browser library source");
    let mut listed: Vec<String> = library
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("include_str!(\"../fixtures/")
                .and_then(|rest| rest.strip_suffix("\"),"))
                .map(str::to_string)
        })
        .collect();
    listed.retain(|name| name.ends_with(".json"));
    listed.sort();
    listed.dedup();
    assert_eq!(on_disk, listed);
}
