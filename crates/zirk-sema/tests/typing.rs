//! Type-system tests.
//!
//! `ZIRK_SPEC_FINAL.md` section 8 requires one valid and one invalid case per
//! rule. Tests are grouped by rule so that correspondence stays verifiable.

use zirk_ast::{Expr, Ident, ModuleDecl, Mutability, Stmt};
use zirk_diagnostics::{DiagnosticSink, RenderStyle, Severity, SourceFile, SourceMap};
use zirk_lexer::tokenize;
use zirk_parser::{codes as parser_codes, parse};
use zirk_sema::{Base, ProjectGlobalSpec, Type, check, check_with_globals, codes};

/// Standard stream calls (`stdout.println`, `stderr.print`, ...) require an
/// explicit `std.io` import (see `add-stdin-stdout-standard-library`). Source
/// using one is automatically prefixed with the import and checked alongside
/// the compiler-bundled `std.io` modules, mirroring how
/// `zirk-browser::check_source` merges them for a program with no
/// filesystem-backed package loader.
fn parse_and_check(source_text: &str) -> (SourceMap, DiagnosticSink) {
    let needs_streams = ["stdout.", "stderr.", "stdin."]
        .iter()
        .any(|prefix| source_text.contains(prefix));
    let source_text = if needs_streams {
        format!("import {{ stdout, stderr, stdin }} from std.io;\n{source_text}")
    } else {
        source_text.to_string()
    };

    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("test.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let mut program = parse(source, &tokens, &mut sink);
    if needs_streams {
        let standard_package = bundled_standard_package(&mut sources, &mut sink);
        let standard_modules = bundled_standard_modules(&mut sources, &mut sink);
        program.enums.extend(standard_package.enums);
        program.classes.extend(standard_package.classes);
        program.contracts.extend(standard_package.contracts);
        program.type_aliases.extend(standard_package.type_aliases);
        for module in &standard_modules {
            program.enums.extend(module.body.enums.iter().cloned());
            program.classes.extend(module.body.classes.iter().cloned());
            program
                .contracts
                .extend(module.body.contracts.iter().cloned());
            program
                .functions
                .extend(module.body.functions.iter().cloned());
            program
                .type_aliases
                .extend(module.body.type_aliases.iter().cloned());
        }
        program.modules.extend(standard_modules);
    }
    check(&sources, &program, &mut sink);
    (sources, sink)
}

#[test]
fn project_globals_are_resolved_against_the_merged_program_types() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "app/src/main.zrk",
        "class Settings { construct() { } }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let globals = [ProjectGlobalSpec {
        name: "settings".into(),
        type_name: "Settings".into(),
        type_source: None,
        readonly: true,
        has_explicit_default: false,
        default_function: None,
        span: None,
    }];

    let checked = check_with_globals(&sources, &program, &globals, &mut sink);
    assert!(
        !sink.has_errors(),
        "global type should resolve from the project source graph: {}",
        sink.render(RenderStyle::Human)
    );
    let resolved = checked.project_globals.get("settings").unwrap();
    assert!(resolved.readonly);
    let Base::Class(class_id) = resolved.ty.base else {
        panic!("expected Settings class type, got {:?}", resolved.ty);
    };
    assert_eq!(checked.classes[class_id as usize].name, "Settings");
}

#[test]
fn project_global_default_selects_one_constructor_and_reports_ambiguity() {
    fn check_global(source_text: &str) -> (zirk_sema::CheckedProgram, DiagnosticSink) {
        let mut sources = SourceMap::new();
        sources.add(SourceFile::new("app/src/main.zrk", source_text));
        let source = sources.entry();
        let mut sink = DiagnosticSink::new();
        let tokens = tokenize(source, &mut sink);
        let program = parse(source, &tokens, &mut sink);
        let checked = check_with_globals(
            &sources,
            &program,
            &[ProjectGlobalSpec {
                name: "person".into(),
                type_name: "Person".into(),
                type_source: None,
                readonly: false,
                has_explicit_default: false,
                default_function: None,
                span: None,
            }],
            &mut sink,
        );
        (checked, sink)
    }

    let (checked, sink) = check_global(
        "class Person { name: String; construct(name: String) { this.name = name; } }",
    );
    assert!(!sink.has_errors(), "{}", sink.render(RenderStyle::Human));
    assert_eq!(
        checked.project_globals["person"].default_constructor,
        Some(0)
    );

    let (checked, sink) = check_global(
        "class Address { construct(city: String) { } } class Person { construct(address: Address) { } }",
    );
    assert!(
        !sink.has_errors(),
        "nested constructor parameters derive their own defaults: {}",
        sink.render(RenderStyle::Human)
    );
    assert_eq!(
        checked.project_globals["person"].default_constructor,
        Some(0)
    );

    let (checked, sink) =
        check_global("class Person { construct(name: String = \"anonymous\") { } }");
    assert!(
        !sink.has_errors(),
        "a declared constructor-parameter default is eligible: {}",
        sink.render(RenderStyle::Human)
    );
    assert_eq!(
        checked.project_globals["person"].default_constructor,
        Some(0)
    );

    let (_checked, sink) =
        check_global("class Person { construct(name: String) { } construct(age: Int32) { } }");
    let diagnostics = sink.render(RenderStyle::Human);
    assert!(sink.has_errors());
    assert!(
        diagnostics.contains("more than one constructor of `Person`"),
        "{diagnostics}"
    );

    let (_checked, sink) = check_global("class Person { construct(name: Char) { } }");
    let diagnostics = sink.render(RenderStyle::Human);
    assert!(
        diagnostics.contains("no constructor of `Person` has parameters with derivable defaults"),
        "a type without a default cannot satisfy the constructor: {diagnostics}"
    );
}

#[test]
fn project_global_get_and_set_are_typed_from_manifest_keys() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "app/src/main.zrk",
        "fn counter(): Int32 { return Global.get(\"counter\"); }\n\
         fn replace(): Void { Global.set(\"counter\", 7); }\n\
         fn frozen(): String { return Global.get(\"frozen\"); }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check_with_globals(
        &sources,
        &program,
        &[
            ProjectGlobalSpec {
                name: "counter".into(),
                type_name: "Int32".into(),
                type_source: None,
                readonly: false,
                has_explicit_default: false,
                default_function: None,
                span: None,
            },
            ProjectGlobalSpec {
                name: "frozen".into(),
                type_name: "String".into(),
                type_source: None,
                readonly: true,
                has_explicit_default: false,
                default_function: None,
                span: None,
            },
        ],
        &mut sink,
    );

    assert!(
        !sink.has_errors(),
        "manifest-typed reads and a mutable replacement should check: {}",
        sink.render(RenderStyle::Human)
    );
    let calls = checked.project_global_calls.values().collect::<Vec<_>>();
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().any(|call| {
        call.name == "counter"
            && call.operation == zirk_sema::ProjectGlobalOperation::Get
            && call.ty == Type::INT32
    }));
    assert!(calls.iter().any(|call| {
        call.name == "counter"
            && call.operation == zirk_sema::ProjectGlobalOperation::Set
            && call.ty == Type::INT32
    }));
    assert!(calls.iter().any(|call| {
        call.name == "frozen"
            && call.operation == zirk_sema::ProjectGlobalOperation::Get
            && call.ty == Type::STRING
    }));
}

#[test]
fn project_global_calls_reject_wrong_types_dynamic_unknown_and_readonly_writes() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "app/src/main.zrk",
        "fn wrong_read(): String { return Global.get(\"counter\"); }\n\
         fn wrong_set(): Void { Global.set(\"counter\", \"seven\"); }\n\
         fn explicit_override(): String { return Global.get<String>(\"counter\"); }\n\
         fn dynamic(key: String): Int32 { return Global.get(key); }\n\
         fn unknown(): Int32 { return Global.get(\"missing\"); }\n\
         fn readonly_write(): Void { Global.set(\"frozen\", \"new\"); }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let _checked = check_with_globals(
        &sources,
        &program,
        &[
            ProjectGlobalSpec {
                name: "counter".into(),
                type_name: "Int32".into(),
                type_source: None,
                readonly: false,
                has_explicit_default: false,
                default_function: None,
                span: None,
            },
            ProjectGlobalSpec {
                name: "frozen".into(),
                type_name: "String".into(),
                type_source: None,
                readonly: true,
                has_explicit_default: false,
                default_function: None,
                span: None,
            },
        ],
        &mut sink,
    );
    let diagnostics = sink.render(RenderStyle::Human);

    assert!(
        sink.has_errors(),
        "invalid global operations must fail sema"
    );
    assert!(
        diagnostics.contains("cannot override the manifest type"),
        "{diagnostics}"
    );
    assert!(
        diagnostics.contains("requires a literal manifest key"),
        "{diagnostics}"
    );
    assert!(
        diagnostics.contains("project global `missing` is not declared"),
        "{diagnostics}"
    );
    assert!(diagnostics.contains("read-only"), "{diagnostics}");
    assert!(
        diagnostics.contains(codes::TYPE_MISMATCH.as_str()),
        "{diagnostics}"
    );
}

#[test]
fn project_global_snapshots_reject_types_without_deep_clone_semantics() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "app/src/main.zrk",
        "class Holder { pointer: Pointer<Int32>?; }\n\
         fn value(): Holder { return Global.get(\"holder\"); }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let _checked = check_with_globals(
        &sources,
        &program,
        &[ProjectGlobalSpec {
            name: "holder".into(),
            type_name: "Holder".into(),
            type_source: None,
            readonly: false,
            has_explicit_default: false,
            default_function: None,
            span: None,
        }],
        &mut sink,
    );

    let diagnostics = sink.render(RenderStyle::Human);
    assert!(
        diagnostics.contains("cannot be used as an immutable snapshot"),
        "mutable global values need a sound deep-clone contract: {diagnostics}"
    );
}

#[test]
fn project_global_watch_infers_subscription_element_type() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "app/src/main.zrk",
        "share class GlobalSnapshot<T> { value: T; revision: UInt64; }\n\
         share class GlobalChange<T> { previous: T; current: T; revision: UInt64; }\n\
         share enum GlobalWatchError { Lagged, Closed, RuntimeFailure }\n\
         share abstract class GlobalSubscription<T> {}\n\
         fn subscribe(): GlobalSubscription<Int32> { return Global.watch(\"counter\"); }\n\
         fn current(subscription: GlobalSubscription<Int32>): GlobalSnapshot<Int32> { return subscription.snapshot(); }\n\
         fn change(subscription: GlobalSubscription<Int32>): Result<GlobalChange<Int32>?, GlobalWatchError> { return subscription.next(); }\n\
         fn stop(subscription: GlobalSubscription<Int32>): Void { subscription.close(); }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check_with_globals(
        &sources,
        &program,
        &[ProjectGlobalSpec {
            name: "counter".into(),
            type_name: "Int32".into(),
            type_source: None,
            readonly: false,
            has_explicit_default: false,
            default_function: None,
            span: None,
        }],
        &mut sink,
    );

    let diagnostics = sink.render(RenderStyle::Human);
    assert_eq!(
        sink.diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.severity == zirk_diagnostics::Severity::Error)
            .count(),
        0,
        "the typed watch API should check without diagnostics: {diagnostics}"
    );

    let (call_span, call) = checked
        .project_global_calls
        .iter()
        .next()
        .expect("Global.watch should retain its manifest-resolved call metadata");
    assert_eq!(call.operation, zirk_sema::ProjectGlobalOperation::Watch);
    assert_eq!(call.name, "counter");
    assert_eq!(call.ty, Type::INT32);
    let Type {
        base: zirk_sema::Base::Instance(instance),
        ..
    } = checked.expr_types[call_span]
    else {
        panic!(
            "watch should return a generic subscription, got {:?}",
            checked.expr_types[call_span]
        );
    };
    let instance = &checked.generic_instances[instance as usize];
    assert_eq!(
        checked.classes[instance.class as usize].name,
        "GlobalSubscription"
    );
    assert_eq!(instance.args, vec![Type::INT32]);

    let operations: std::collections::HashSet<_> = checked
        .project_global_subscription_calls
        .values()
        .map(|call| call.operation)
        .collect();
    assert_eq!(
        operations,
        [
            zirk_sema::ProjectGlobalSubscriptionOperation::Snapshot,
            zirk_sema::ProjectGlobalSubscriptionOperation::Next,
            zirk_sema::ProjectGlobalSubscriptionOperation::Close,
        ]
        .into_iter()
        .collect()
    );
}

fn bundled_standard_package(
    sources: &mut SourceMap,
    sink: &mut DiagnosticSink,
) -> zirk_ast::Program {
    let file = sources.add(SourceFile::new(
        "std.io/package.zrk",
        include_str!("../../../stdlib/std/io/package.zrk"),
    ));
    let tokens = tokenize(sources.file(file), sink);
    parse(sources.file(file), &tokens, sink)
}

fn bundled_standard_modules(sources: &mut SourceMap, sink: &mut DiagnosticSink) -> Vec<ModuleDecl> {
    [
        ("stdin", include_str!("../../../stdlib/std/io/stdin.zrk")),
        ("stdout", include_str!("../../../stdlib/std/io/stdout.zrk")),
        ("stderr", include_str!("../../../stdlib/std/io/stderr.zrk")),
    ]
    .into_iter()
    .map(|(name, source)| {
        let file = sources.add(SourceFile::new(format!("std.io/{name}.zrk"), source));
        let tokens = tokenize(sources.file(file), sink);
        let body = parse(sources.file(file), &tokens, sink);
        ModuleDecl {
            name: Ident::new(name, body.span),
            shared: true,
            span: body.span,
            body: Box::new(body),
        }
    })
    .collect()
}

/// Checks a full program expecting no errors.
fn accepted(source_text: &str) {
    let (_sources, sink) = parse_and_check(source_text);

    assert!(
        !sink.has_errors(),
        "no errors were expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}

/// Checks a full program expecting errors, returning them rendered.
fn rejected(source_text: &str) -> String {
    let (_sources, sink) = parse_and_check(source_text);

    assert!(sink.has_errors(), "an error was expected and none occurred");
    sink.render(RenderStyle::Human)
}

/// Checks the sema contract while source lowering remains explicitly gated.
fn rejected_mutex_with(source_text: &str) -> String {
    rejected(source_text)
}

/// Wraps a body inside a valid `main`.
fn in_main(body: &str) -> String {
    format!("fn main(): Void {{\n{body}\n}}")
}

fn accepted_body(body: &str) {
    accepted(&in_main(body));
}

/// Checks a full program expecting no errors, returning the rendered
/// diagnostics so warnings can be asserted on.
fn accepted_with_warnings(source_text: &str) -> String {
    let (_sources, sink) = parse_and_check(source_text);

    let rendered = sink.render(RenderStyle::Human);
    assert!(!sink.has_errors(), "no errors were expected:\n{rendered}");
    rendered
}

fn rejected_body(body: &str) -> String {
    rejected(&in_main(body))
}

fn suspension_summary(source_text: &str, function_name: &str) -> zirk_sema::EffectSummary {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("suspension.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let function = program
        .functions
        .iter()
        .find(|function| function.name.name == function_name)
        .unwrap_or_else(|| panic!("test function `{function_name}` is missing"));
    let span = function.name.span;
    let checked = check(&sources, &program, &mut sink);
    assert!(
        !sink.has_errors(),
        "test program should check:\n{}",
        sink.render(RenderStyle::Human)
    );
    checked
        .suspend_summaries
        .get(&span)
        .copied()
        .unwrap_or_else(|| panic!("summary for `{function_name}` is missing"))
}

#[test]
fn suspension_summary_propagates_through_forward_direct_calls() {
    let source = "fn caller(): Void { middle(); }\n\
                 fn middle(): Void { leaf(); }\n\
                 fn leaf(): Void { Timer.sleep(1s); }\n\
                 fn main(): Void { caller(); }";
    let caller = suspension_summary(source, "caller");
    assert!(caller.may_suspend);
    assert!(caller.complete);
}

#[test]
fn suspension_summary_marks_direct_channel_waits_and_concurrent_constructs() {
    let source = "fn communicate(channel: Channel<Int32>): Void {\n\
                   channel.send(1);\n\
                   _ = channel.receive();\n\
                 }\n\
                 fn main(): Void {\n\
                   concurrent { spawn (): Void => { Timer.sleep(1s); }; }\n\
                 }";
    let communicate = suspension_summary(source, "communicate");
    assert!(communicate.may_suspend);
    assert!(communicate.complete);
    let main = suspension_summary(source, "main");
    assert!(main.may_suspend);
    assert!(main.complete);
}

#[test]
fn suspension_summary_marks_native_calls_unknown_and_blocking() {
    let source = "extern \"C\" fn abs(n: Int32): Int32;\n\
                 fn main(): Void { unsafe { commit { _ = abs(-1); } } }";
    let main = suspension_summary(source, "main");
    assert!(main.may_suspend);
    assert!(main.may_block);
    assert!(!main.complete);
}

#[test]
fn suspension_summary_tracks_timer_callback_separately_from_registration() {
    let source_text = "extern \"C\" fn host_wait(): Void;\n\
                      fn pause(): Void { Timer.sleep(1s); }\n\
                      fn block(): Void { unsafe { commit { host_wait(); } } }\n\
                      fn schedule(): Void { Timer.after(1s, (): Void => { }); }\n\
                      fn transitively_effectful(): Void { pause(); block(); schedule(); }\n\
                      fn main(): Void { Timer.after(1s, (): Void => transitively_effectful()); }";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("suspension.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let main = program
        .functions
        .iter()
        .find(|function| function.name.name == "main")
        .unwrap();
    let main_span = main.name.span;
    let callback_span = main
        .body
        .statements
        .iter()
        .find_map(|statement| match statement {
            Stmt::Expr(expr) => match &expr.expr {
                Expr::Call(call) => match &call.args[1].value {
                    Expr::Lambda(lambda) => Some(lambda.span),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .expect("Timer.after callback lambda");
    let checked = check(&sources, &program, &mut sink);
    assert!(!sink.has_errors(), "{}", sink.render(RenderStyle::Human));
    let main_effect = checked.suspend_summaries[&main_span];
    assert!(!main_effect.may_suspend);
    assert!(!main_effect.may_block);
    let callback_effect = checked.suspend_summaries[&callback_span];
    assert!(callback_effect.may_suspend);
    assert!(callback_effect.may_block);
    assert!(callback_effect.may_spawn);
    assert!(!callback_effect.complete);
}

#[test]
fn suspension_summary_does_not_treat_timer_registration_as_a_wait() {
    let source = "fn main(): Void { Timer.after(1s, (): Void => { }); }";
    let main = suspension_summary(source, "main");
    assert!(!main.may_suspend);
    assert!(!main.may_block);
    assert!(main.complete);
}

#[test]
fn suspension_summary_keeps_open_method_dispatch_conservative() {
    let source = "class Worker { run(): Void { } }\n\
                 fn main(): Void { mut w: Worker = Worker(); w.run(); }";
    let main = suspension_summary(source, "main");
    assert!(main.may_suspend);
    assert!(main.may_block);
    assert!(!main.complete);
}

#[test]
fn suspension_summary_keeps_unknown_function_value_calls_conservative() {
    let source = "fn invoke(action: Fn() => Void): Void { action(); }\n\
                 fn main(): Void { }";
    let invoke = suspension_summary(source, "invoke");
    assert!(invoke.may_suspend);
    assert!(invoke.may_block);
    assert!(!invoke.complete);
}

#[test]
fn suspension_summary_marks_pure_direct_call_chains_complete_and_non_suspending() {
    let source = "fn leaf(): Int32 { return 1; }\n\
                 fn caller(): Int32 { return leaf(); }\n\
                 fn main(): Void { }";
    let caller = suspension_summary(source, "caller");
    assert!(!caller.may_suspend);
    assert!(caller.complete);
}

#[test]
fn thread_run_rejects_direct_channel_and_timer_suspension_effects() {
    let rendered = rejected_body(
        "Thread.run((channel: Channel<Int32>): Void => {
             _ = channel.receive();
             Timer.sleep(1s);
         });",
    );
    assert!(
        rendered.contains("E0484"),
        "direct channel/timer suspension must be rejected: {rendered}"
    );
}

#[test]
fn thread_run_rejects_transitive_suspension_in_a_named_function() {
    let rendered = rejected(
        "fn waits(): Void { Timer.sleep(1s); }
         fn main(): Void { Thread.run(waits); }",
    );
    assert!(
        rendered.contains("E0484"),
        "transitive suspension must be rejected: {rendered}"
    );

    let spawned = rejected(
        "fn launches(): Void { concurrent { spawn (): Void => { }; } }
         fn main(): Void { Thread.run(launches); }",
    );
    assert!(
        spawned.contains("E0484"),
        "transitive task creation must be rejected: {spawned}"
    );

    let blocked = rejected(
        "extern \"C\" fn host_wait(): Void;\n\
         fn waits_on_host(): Void { unsafe { host_wait(); } }\n\
         fn main(): Void { Thread.run(waits_on_host); }",
    );
    assert!(
        blocked.contains("E0484"),
        "transitive blocking must be rejected: {blocked}"
    );

    let unknown = rejected(
        "fn invoke(action: Fn() => Void): Void { action(); }\n\
         fn calls_unknown(): Void { invoke((): Void => { }); }\n\
         fn main(): Void { Thread.run(calls_unknown); }",
    );
    assert!(
        unknown.contains("E0484"),
        "a transitive unknown call must be rejected: {unknown}"
    );

    let channel_wait = rejected(
        "fn receives(): Void {
             inmut channel: Channel<Int32> = Channel(0);
             _ = channel.receive();
         }
         fn main(): Void { Thread.run(receives); }",
    );
    assert!(
        channel_wait.contains("E0484"),
        "a named function with a transitive channel wait must be rejected: {channel_wait}"
    );
}

#[test]
fn thread_run_rejects_nonblocking_channel_access_transitively() {
    let direct = rejected(
        "fn main(): Void {
             mut channel: Channel<Int32> = Channel();
             Thread.run((): Void => { channel.try_send(1); });
         }",
    );
    assert!(
        direct.contains("E0484"),
        "nonblocking channel access from a managed worker must be rejected: {direct}"
    );

    let transitive = rejected(
        "fn touches_channel(channel: Channel<Int32>): Void { channel.close(); }
         fn main(): Void {
             mut channel: Channel<Int32> = Channel();
             Thread.run((): Void => { touches_channel(channel); });
         }",
    );
    assert!(
        transitive.contains("E0484"),
        "channel effects must propagate through direct calls: {transitive}"
    );
}

#[test]
fn thread_run_accepts_pure_lambda_and_returns_its_typed_result() {
    accepted("fn main(): Void { inmut value: Int32 = Thread.run((): Int32 => 1); }");
    accepted_body("Thread.run((): Void => { });");
}

#[test]
fn thread_run_rejects_callable_values_without_a_known_summary() {
    let rendered = rejected_body(
        "mut action: Fn() => Void = (): Void => { };
         Thread.run(action);",
    );
    assert!(
        rendered.contains("E0484"),
        "an indirect callable without a linked summary must fail closed: {rendered}"
    );
}

#[test]
fn thread_run_checks_capture_and_result_ownership() {
    let captured = rejected_body(
        "mut rows: List<Int32> = [1];
         Thread.run((): Int32 => rows[0]);",
    );
    assert!(
        captured.contains("E0430") || captured.contains("mutable alias"),
        "a mutable list capture must fail the thread boundary: {captured}"
    );

    let returned_pointer = rejected_body(
        "mut value: Int32 = 1;
         Thread.run((): Pointer<Int32> => unsafe { Pointer.from(value) });",
    );
    assert!(
        returned_pointer.contains("E0482"),
        "a raw pointer result must fail the thread boundary: {returned_pointer}"
    );
}

#[test]
fn concurrent_detach_accepts_inline_and_named_callbacks() {
    accepted_body("Concurrent.detach((): Void => { });");
    accepted(
        "fn background_work(): Void { }
         fn main(): Void { Concurrent.detach(background_work); }",
    );
}

#[test]
fn app_background_error_handler_accepts_the_managed_signature() {
    let rendered = rejected_body(
        "fn report(error: Throwable): Void { }\n         App.on_background_error = report;",
    );
    assert!(
        !rendered.contains(codes::PENDING_FEATURE.as_str()),
        "{rendered}"
    );
    assert!(
        !rendered.contains(codes::UNKNOWN_MEMBER.as_str()),
        "the source API should have an intentional feature gate: {rendered}"
    );
    assert!(
        !rendered.contains(codes::TYPE_MISMATCH.as_str()),
        "a `(Throwable): Void` handler has the intended signature: {rendered}"
    );
}

#[test]
fn app_background_error_handler_rejects_wrong_signature() {
    let rendered = rejected_body(
        "fn report(error: Int32): Int32 { return error; }\n         App.on_background_error = report;",
    );
    assert!(
        rendered.contains(codes::TYPE_MISMATCH.as_str()),
        "{rendered}"
    );
    assert!(
        !rendered.contains(codes::PENDING_FEATURE.as_str()),
        "{rendered}"
    );
    assert!(
        !rendered.contains(codes::UNKNOWN_MEMBER.as_str()),
        "{rendered}"
    );
}

#[test]
fn concurrent_detach_rejects_invalid_arity_and_non_positional_arguments() {
    for source in [
        "Concurrent.detach();",
        "Concurrent.detach((): Void => { }, (): Void => { });",
        "Concurrent.detach(callback: (): Void => { });",
    ] {
        let rendered = rejected_body(source);
        assert!(
            rendered.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
            "expected arity/position diagnostic for {source}: {rendered}"
        );
        assert!(
            !rendered.contains(codes::PENDING_FEATURE.as_str()),
            "malformed calls should fail sema before the Phase 5 gate: {rendered}"
        );
    }
}

#[test]
fn concurrent_detach_rejects_nonzero_arity_and_non_void_callbacks() {
    let parameter = rejected_body("Concurrent.detach((value: Int32): Void => { });");
    assert!(
        parameter.contains(codes::TYPE_MISMATCH.as_str()),
        "{parameter}"
    );
    assert!(
        !parameter.contains(codes::PENDING_FEATURE.as_str()),
        "{parameter}"
    );

    let result = rejected_body("Concurrent.detach((): Int32 => 1);");
    assert!(result.contains(codes::TYPE_MISMATCH.as_str()), "{result}");
    assert!(result.contains("must return `Void`"), "{result}");
    assert!(
        !result.contains(codes::PENDING_FEATURE.as_str()),
        "{result}"
    );
}

#[test]
fn concurrent_detach_rejects_mutable_lambda_captures() {
    let rendered = rejected_body(
        "mut rows: List<Int32> = [1];
         Concurrent.detach((): Void => { _ = rows[0]; });",
    );
    assert!(
        rendered.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "mutable captures must fail the background boundary: {rendered}"
    );
    assert!(
        !rendered.contains(codes::PENDING_FEATURE.as_str()),
        "{rendered}"
    );
}

// --- Entrypoint -------------------------------------------------------------

#[test]
fn valid_minimal_program() {
    accepted("fn main(): Void { }");
}

// --- Collection sequence pipeline ----------------------------------------

#[test]
fn valid_collection_sequence_pipeline_types_and_preserves_families() {
    accepted_body(
        "inmut list: List<Int32> = [1, 2, 3];\n\
         inmut array: Array<Int32> = [1, 2, 3];\n\
         inmut mapped: List<Int32> = list.map((x: Int32): Int32 => x * 2);\n\
         inmut filtered: Array<Int32> = array.filter((x: Int32): Boolean => x > 1);\n\
         inmut reduced: String = [\"a\", \"b\"].reduce(\"\", (a: String, b: String): String => a + b);\n\
         inmut total: Int32 = list.sum();\n\
         inmut count: Int32 = list.count();\n\
         inmut materialized: List<Int32> = array.collect();\n\
         list.for_each((x: Int32): Void => { });",
    );
}

#[test]
fn invalid_collection_sum_requires_numeric_elements() {
    let output = rejected_body(
        "inmut values: List<String> = [\"a\"];\n\
         inmut total = values.sum();",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn parallel_reduce_requires_an_associative_combiner_or_ordered_escape() {
    accepted_body(
        "inmut::strict values: List<Int32> = [1, 2, 3];\n\
         inmut total: Int32 = parallel { values.reduce(0, (a: Int32, b: Int32): Int32 => a + b) };",
    );
    accepted_body(
        "inmut::strict values: List<Int32> = [1, 2, 3];\n\
         inmut total: Int32 = values.parallel.reduce(0, (a: Int32, b: Int32): Int32 => a * b);",
    );
    accepted_body(
        "inmut::strict values: List<Int32> = [1, 2, 3];\n\
         inmut total: Int32 = parallel { values.reduce_ordered(0, (a: Int32, b: Int32): Int32 => a - b) };",
    );

    let in_region = rejected_body(
        "inmut values: List<Int32> = [1, 2, 3];\n\
         inmut total: Int32 = parallel { values.reduce(0, (a: Int32, b: Int32): Int32 => a - b) };",
    );
    assert!(
        in_region.contains(codes::PARALLEL_NONASSOCIATIVE_REDUCE.as_str()),
        "{in_region}"
    );

    let adapter = rejected_body(
        "inmut values: List<Int32> = [1, 2, 3];\n\
         inmut total: Int32 = values.parallel.reduce(0, (a: Int32, b: Int32): Int32 => a - b);",
    );
    assert!(
        adapter.contains(codes::PARALLEL_NONASSOCIATIVE_REDUCE.as_str()),
        "{adapter}"
    );
}

#[test]
fn invalid_collection_sort_requires_natural_ordering() {
    let output = rejected_body(
        "inmut values: List<List<Int32>> = [[1]];\n\
         values.sort();",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("sort_by"), "{output}");
}

#[test]
fn valid_collection_companions_and_range_tuple_methods() {
    accepted_body(
        "mut values: List<Int32> = [3, 1, 2];\n\
         values.contains(2); values.reverse(); values.sort();\n\
         values.sort_by((a: Int32, b: Int32): Int32 => a - b);\n\
         inmut first: Int32? = values.first();\n\
         inmut last: Int32? = values.last();\n\
         inmut popped: Int32? = values.pop();\n\
         inmut squares: List<Int32> = (1..=3).map((x: Int32): Int32 => x * x).collect();\n\
         inmut text: String = (1, \"x\").to_string();\n\
         inmut tuple_copy = (1, \"x\").clone();\n\
         inmut arity: Int32 = (1, \"x\").length;",
    );
}

#[test]
fn invalid_tuple_does_not_have_sequence_methods() {
    let output = rejected_body("(1, \"x\").map((x: Int32): Int32 => x);");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

// --- Concurrent scopes and timers -----------------------------------------

#[test]
fn valid_job_spawn_wait_and_timer_static_members() {
    accepted(
        "fn compute(): Int32 { return 7; }\n\
         fn main(): Void {\n\
           concurrent {\n\
             inmut h: Job<Int32> = spawn compute();\n\
             inmut value: Int32 = h.wait();\n\
           }\n\
           Timer.sleep(1s);\n\
           inmut later: Job<Int32> = Timer.after(1s, (): Int32 => 9);\n\
           inmut answer: Int32 = later.wait();\n\
           Timer.every(1s, (): Void => { });\n\
         }",
    );
}

#[test]
fn concurrent_boundary_matrix_rejects_unproven_spawn_channel_and_parallel_values() {
    let spawn_capture = rejected(
        "fn main(): Void {
           mut rows: List<Int32> = [1];
           concurrent {
             mut job: Job<Int32> = spawn (): Int32 => rows[0];
             mut value: Int32 = job.wait();
           }
         }",
    );
    assert!(
        spawn_capture.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "spawn must reject a captured mutable alias: {spawn_capture}"
    );

    let spawn_result = rejected(
        "fn make_action(): Fn() => Void { return (): Void => { }; }
         fn main(): Void {
           concurrent {
             mut job: Job<Fn() => Void> = spawn make_action();
             mut value: Fn() => Void = job.wait();
           }
         }",
    );
    assert!(
        spawn_result.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "spawn must reject an untransferable function result: {spawn_result}"
    );

    let channel_send = rejected(
        "fn make_action(): Fn() => Void { return (): Void => { }; }
         fn main(): Void {
           mut channel: Channel<Fn() => Void> = Channel(1);
           channel.send(make_action());
         }",
    );
    assert!(
        channel_send.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "channel send must reject an untransferable mutable resource: {channel_send}"
    );

    let channel_receive = rejected(
        "fn main(): Void {
           mut channel: Channel<Fn() => Void> = Channel(1);
           mut value = channel.receive();
         }",
    );
    assert!(
        channel_receive.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "channel receive must reject an untransferable result type: {channel_receive}"
    );

    let parallel_result = rejected(
        "fn make_action(): Fn() => Void { return (): Void => { }; }
         fn main(): Void {
           mut value: Fn() => Void = parallel { make_action() };
         }",
    );
    assert!(
        parallel_result.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "parallel must reject an untransferable result: {parallel_result}"
    );
}

#[test]
fn timer_callbacks_enforce_capture_and_result_ownership_boundaries() {
    let capture = rejected(
        "class Counter {\n\
           mut value: Int32;\n\
           construct() { this.value = 0; }\n\
           read(): Int32 { return this.value; }\n\
         }\n\
         fn main(): Void {\n\
           mut counter = Counter();\n\
           Timer.after(1s, (): Int32 => counter.read());\n\
         }",
    );
    assert!(
        capture.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{capture}"
    );
    assert!(capture.contains("`Timer.after` callback"), "{capture}");

    let result = rejected(
        "class MutableValue implements Resource { mut value: Int32; construct() { this.value = 0; } }\n\
         fn create(): MutableValue { return MutableValue(); }\n\
         fn main(): Void { Timer.after(1s, (): MutableValue => create()); }",
    );
    assert!(
        result.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "{result}"
    );
    assert!(result.contains("`Timer.after` callback result"), "{result}");

    let every_result = rejected("fn main(): Void { Timer.every(1s, (): Int32 => 1); }");
    assert!(
        every_result.contains(codes::TYPE_MISMATCH.as_str()),
        "Timer.every must reject a callback with a value result: {every_result}"
    );
    assert!(
        every_result.contains("`Timer.every` requires a `Void` callback"),
        "the diagnostic must identify the Timer.every callback contract: {every_result}"
    );

    let named_callback = rejected(
        "fn tick(): Void { }\n\
         fn main(): Void { Timer.after(1s, tick); }",
    );
    assert!(
        named_callback.contains(codes::TYPE_MISMATCH.as_str()),
        "timer callbacks must be lifted lambdas, not named function values: {named_callback}"
    );
    assert!(
        named_callback.contains("a timer callback must be written as a lambda"),
        "the diagnostic must identify the callback representation requirement: {named_callback}"
    );

    let parameterized_callback =
        rejected("fn main(): Void { Timer.after(1s, (value: Int32): Void => { }); }");
    assert!(
        parameterized_callback.contains("a timer callback must take no arguments"),
        "timer callbacks with parameters must fail at the API boundary: {parameterized_callback}"
    );
}

#[test]
fn invalid_parallel_call_to_transitively_suspending_function() {
    let output = rejected(
        "fn pause(): Void { Timer.sleep(1s); }\n\
         fn main(): Void { parallel { pause(); } }",
    );
    assert!(
        output.contains(codes::PARALLEL_REGION_IO.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_parallel_call_to_function_that_spawns_work() {
    let output = rejected(
        "fn schedule(): Void { Timer.after(1s, (): Void => { }); }\n\
         fn main(): Void { parallel { schedule(); } }",
    );
    assert!(
        output.contains(codes::PARALLEL_REGION_IO.as_str()),
        "{output}"
    );
}

#[test]
fn valid_parallel_call_to_complete_cpu_only_function() {
    accepted(
        "fn compute(): Int32 { return 42; }\n\
         fn main(): Void { parallel { _ = compute(); } }",
    );
}

#[test]
fn invalid_channel_operation_inside_parallel_region() {
    let output = rejected_body(
        "mut channel: Channel<Int32> = Channel(1); parallel { channel.try_send(1); }",
    );
    assert!(
        output.contains(codes::PARALLEL_REGION_IO.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_spawn_outside_main_or_concurrent_scope() {
    let output = rejected(
        "fn worker(): Void { inmut handle = spawn 1; }\n\
         fn main(): Void { worker(); }",
    );
    assert!(
        output.contains(codes::SPAWN_OUTSIDE_SCOPE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_timer_every_requires_a_void_callback() {
    let output = rejected_body("Timer.every(1s, (): Int32 => 1);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_job_waited_twice() {
    let output =
        rejected_body("inmut h = spawn 1; inmut first = h.wait(); inmut second = h.wait();");
    assert!(output.contains(codes::SECOND_WAIT.as_str()), "{output}");
}

#[test]
fn valid_concurrent_bindings_hoist_after_the_block() {
    accepted(
        "fn f(): Int32 { return 1; }\n\
         fn g(): Int32 { return 2; }\n\
         fn main(): Void {\n\
           concurrent {\n\
             inmut a: Int32 = f();\n\
             inmut b: Int32 = g();\n\
           }\n\
           inmut total: Int32 = a + b;\n\
         }",
    );
}

#[test]
fn valid_concurrent_dependent_binding_names_its_input() {
    accepted(
        "fn f(): Int32 { return 1; }\n\
         fn g(x: Int32): Int32 { return x + 1; }\n\
         fn main(): Void {\n\
           concurrent {\n\
             inmut a: Int32 = f();\n\
             inmut b: Int32 = g(a);\n\
           }\n\
           inmut _seen: Int32 = b;\n\
         }",
    );
}

#[test]
fn invalid_concurrent_binding_cycle_is_rejected() {
    let output = rejected(
        "fn f(x: Int32): Int32 { return x; }\n\
         fn g(x: Int32): Int32 { return x; }\n\
         fn main(): Void {\n\
           concurrent {\n\
             inmut a: Int32 = f(b);\n\
             inmut b: Int32 = g(a);\n\
           }\n\
         }",
    );
    assert!(
        output.contains(codes::CONCURRENT_BINDING_CYCLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_concurrent_early_read_of_sibling_binding() {
    let output = rejected(
        "fn f(): Int32 { return 1; }\n\
         fn main(): Void {\n\
           concurrent {\n\
             inmut a: Int32 = f();\n\
             stdout.println(a);\n\
           }\n\
         }",
    );
    assert!(
        output.contains(codes::CONCURRENT_EARLY_READ.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_unused_job_handle_is_diagnosed() {
    let output = rejected(
        "fn work(): Int32 { return 1; }\n\
         fn main(): Void {\n\
           concurrent {\n\
             inmut h: Job<Int32> = spawn work();\n\
           }\n\
         }",
    );
    assert!(output.contains(codes::UNUSED_JOB.as_str()), "{output}");
}

#[test]
fn valid_unused_job_handle_discharged_with_wildcard() {
    accepted(
        "fn work(): Int32 { return 1; }\n\
         fn main(): Void {\n\
           concurrent {\n\
             inmut h: Job<Int32> = spawn work();\n\
             _ = h;\n\
           }\n\
         }",
    );
}

#[test]
fn a_program_does_not_need_a_source_level_main_function() {
    accepted("fn other(): Void { }");
}

#[test]
fn main_is_an_ordinary_function_name() {
    for source_text in [
        "fn main(): Int32 { return 1; }",
        "fn main(a: Int32): Void { }",
    ] {
        accepted(source_text);
    }
}

// --- Types of the subset ----------------------------------------------------

#[test]
fn valid_the_four_types_of_the_subset() {
    accepted_body("mut a: Int32 = 1;\nmut b: Boolean = true;\nmut c: String = \"x\";\nreturn;");
}

#[test]
fn invalid_type_from_a_later_phase_states_its_phase() {
    // `Int64`/`Float64`/`Char` and the rest of the scalars in this phase's
    // scope resolve now; `Channel<T>` is known and therefore reports its
    // generic arity when written without an element type.
    let output = rejected_body("mut x: Channel = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(
        output.contains("takes exactly one type argument"),
        "{output}"
    );
}

#[test]
fn valid_typed_channel_construction_members_and_outcomes() {
    accepted_body(
        "mut bounded: Channel<Int32> = Channel(2);\n\
         mut growable: Channel<Int32> = Channel.unbounded(8);\n\
         bounded.send(1);\n\
         mut sent: SendOutcome = bounded.try_send(2);\n\
         mut received: ReceiveOutcome<Int32> = bounded.try_receive();\n\
         mut value: Int32? = bounded.receive();\n\
         mut closed: Boolean = bounded.is_closed;\n\
         mut length: Int32 = bounded.length;\n\
         mut capacity: Int32 = bounded.capacity;\n\
         mut values: List<Int32> = bounded.drain();\n\
         bounded.close();\n\
         growable.close();",
    );
}

#[test]
fn valid_channel_endpoint_can_be_captured_by_a_spawn_branch() {
    accepted(
        "fn main(): Void {\n\
           mut channel: Channel<Int32> = Channel(1);\n\
           concurrent {\n\
             spawn (): Void => { channel.send(1); };\n\
             channel.close();\n\
           }\n\
         }",
    );
}

#[test]
fn valid_short_aliases_of_the_default_integer() {
    // `Int` and `Integer` name `Int32`, which has existed since Phase 1.
    accepted_body("mut a: Int = 1;\nmut b: Integer = 2;\nmut c: Int32 = a + b;");
}

#[test]
fn valid_short_aliases_of_the_default_unsigned_integer() {
    // `UInt` and `UInteger` name `UInt32` exactly the way `Int`/`Integer`
    // name `Int32` (spec: `zirk-scalars`, "Complete family of integer
    // widths").
    accepted_body("mut a: UInt = 1;\nmut b: UInteger = 2;\nmut c: UInt32 = a + b;");
}

// --- Integer widths (roadmap Phase 3b) ---------------------------------------

#[test]
fn valid_safe_widening_between_integer_widths() {
    // Same signedness, no narrower destination — implicit (`Type::accepts`).
    accepted_body("mut a: Int32 = 1;\nmut b: Int64 = a;\nmut c: Int128 = b;");
}

#[test]
fn invalid_narrowing_is_not_implicit() {
    let output = rejected_body("mut a: Int32 = 1;\nmut b: Int8 = a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("no implicit conversion"));
}

#[test]
fn invalid_widening_across_signedness_is_not_implicit() {
    let output = rejected_body("mut a: Int32 = 1;\nmut b: UInt64 = a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_explicit_narrowing_with_as() {
    accepted_body("mut a: Int32 = 300;\nmut b = a as Int8;");
}

#[test]
fn valid_explicit_sign_crossing_with_as() {
    accepted_body("mut a: Int32 = -1;\nmut b = a as UInt32;");
}

#[test]
fn valid_arithmetic_between_different_integer_widths() {
    accepted_body("mut a: Int64 = 1;\nmut b: Int32 = 2;\nmut c = a + b;");
}

#[test]
fn invalid_negation_of_an_unsigned_width() {
    let output = rejected_body("mut a: UInt32 = 1 as UInt32;\nmut b = -a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("signed"));
}

#[test]
fn valid_bitwise_not_on_an_unsigned_width() {
    accepted_body("mut a: UInt32 = 1 as UInt32;\nmut b = ~a;");
}

#[test]
fn valid_shift_amount_may_be_a_different_width() {
    accepted_body("mut a: UInt32 = 1 as UInt32;\nmut b = a << 2;");
}

#[test]
fn valid_float_family_resolves() {
    accepted_body(
        "mut a: Float64 = 1.5;\nmut b: Float32 = 1.5f32;\nmut c: Float16 = 1.5f16;\nmut d: Float128 = 1.5f128;\nreturn;",
    );
}

#[test]
fn valid_exact_float_is_the_bare_fractional_literal() {
    // The plain name and a suffix-less literal are the exact base-ten type;
    // `0.1 + 0.2 == 0.3` type-checks and the sum is exact `Decimal`.
    accepted_body("mut a: Decimal = 0.1;\nmut b = 0.2;\nmut c: Boolean = (a + b) == 0.3;");
    accepted_body("mut a: Int32 = 2;\nmut b = 0.5;\nmut c: Decimal = a + b;");
}

#[test]
fn invalid_exact_float_and_binary_float_do_not_mix() {
    let output = rejected_body("mut a: Decimal = 0.1;\nmut b: Float64 = 0.2f;\nmut c = a + b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_former_binary_spelling_redirects() {
    let output = rejected_body("mut a: BinaryFloat64 = 1.5;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(output.contains("Float64"));
    assert!(output.contains("Decimal"));
}

#[test]
fn valid_exact_float_member_surface() {
    accepted_body(
        "mut a: Decimal = 2.5;\n\
         mut b: Decimal = a.abs();\n\
         mut c: Int32 = a.sign();\n\
         mut d: Int32 = a.scale();\n\
         mut e: Boolean = a.is_integer();\n\
         mut f: Decimal = a.round(1);\n\
         mut g: Decimal = a.div(3.0, 4);",
    );
}

#[test]
fn invalid_exact_float_has_no_infinity_member() {
    let output = rejected_body("mut a: Decimal = Decimal.POSITIVE_INFINITY;");
    assert!(!output.is_empty());
}

#[test]
fn valid_exact_float_conversions() {
    // Int -> Float is implicit and exact; Float -> Int and Float <-> Float
    // are explicit.
    accepted_body("mut i: Int32 = 5;\nmut f: Float = i;");
    accepted_body("mut f: Float = 2.5;\nmut i: Int32 = f as Int32;");
    accepted_body("mut f: Float = 2.5;\nmut g: Float64 = f as Float64;");
    accepted_body("mut g: Float64 = 2.5f;\nmut f: Float = Float(g);");
}

#[test]
fn valid_exact_float_parse_returns_a_result() {
    accepted_body(
        "mut r: Result<Float, ParseError> = Float.parse(\"0.1\");\n\
         mut v: Float64 = match r { Ok(value) => value; Error(_) => 0.0f; };",
    );
}

#[test]
fn valid_exact_float_scientific_literal_is_exact() {
    accepted_body("mut a: Float = 6.25e-2;\nmut b: Boolean = a == 0.0625;");
}

#[test]
fn valid_exponentiation_operator_result_types() {
    // `exponentiation-operator`: `a ** b` is `a.pow(b)`.
    // `Int ** Int` (non-negative) stays integer.
    accepted_body("mut x: Int32 = 2 ** 3;");
    // `Int ** negativeLiteral` widens to exact `Decimal`.
    accepted_body("mut x: Decimal = 2 ** -1;");
    // exact `Decimal` base stays exact `Decimal`.
    accepted_body("mut b: Decimal = 1.5;\nmut x: Decimal = b ** 2;");
    // `Float` base stays that width.
    accepted_body("mut b: Float64 = 2.0f;\nmut x: Float64 = b ** 3;");
    // a dynamic exponent stays integer.
    accepted_body("mut k: Int32 = 4;\nmut x: Int32 = 2 ** k;");
    // compound form.
    accepted_body("mut n: Int32 = 2;\nn **= 10;");
}

#[test]
fn invalid_exponentiation_widened_result_is_not_an_integer() {
    // `2 ** -1` is exact `Float` `0.5`, not assignable to `Int32`.
    let output = rejected_body("mut x: Int32 = 2 ** -1;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_exponentiation_cannot_mix_exact_and_binary_float() {
    let output = rejected_body("mut a: Decimal = 2.0;\nmut b: Float64 = 3.0f;\nmut c = a ** b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_safe_widening_between_float_widths() {
    accepted_body("mut a: Float16 = 1.5f16;\nmut b: Float32 = a;\nmut c: Float64 = b;");
}

#[test]
fn invalid_float_narrowing_is_not_implicit() {
    let output = rejected_body("mut a: Float64 = 1.5;\nmut b: Float16 = a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_explicit_float_narrowing_with_as() {
    accepted_body("mut a: Float64 = 1.5;\nmut b: Float16 = a as Float16;");
}

#[test]
fn valid_explicit_conversion_between_int_and_float() {
    accepted_body("mut a: Int32 = 5;\nmut b: Float64 = a as Float64;\nmut c: Int32 = b as Int32;");
}

#[test]
fn valid_mixed_integer_and_float_arithmetic_produces_float() {
    accepted_body("mut a: Int32 = 2;\nmut b: Float64 = 1.5;\nmut c: Float64 = a + b;");
}

#[test]
fn valid_arithmetic_between_different_float_widths() {
    accepted_body("mut a: Float32 = 1.5f32;\nmut b: Float64 = 1.5;\nmut c = a + b;");
}

#[test]
fn valid_integer_literal_infers_its_contextual_width() {
    accepted_body(
        "mut a: Int8 = 1;\n\
         mut b: UInt8 = 1;\n\
         mut c: Int16 = 1;\n\
         mut d: Int128 = 1;\n\
         mut e: Int = 1;",
    );
}

#[test]
fn valid_float_literal_infers_its_contextual_width() {
    accepted_body(
        "mut a: Float16 = 1.0;\n\
         mut b: Float32 = 1.0;\n\
         mut c: Float64 = 1.0;\n\
         mut d: Float = 1.0;\n\
         mut e: Float128 = 1.0;",
    );
}

#[test]
fn valid_int_literal_widens_to_float_when_exact() {
    accepted_body(
        "mut a: Float16 = 1;\n\
         mut b: Float32 = 1000;\n\
         mut c: Float64 = 100000;\n\
         mut d: Float128 = 1000000000000;",
    );
}

#[test]
fn valid_mixed_signedness_arithmetic_promotes_to_a_common_width() {
    accepted_body(
        "mut a: UInt8 = 1 as UInt8;\n\
         mut b: Int32 = -1;\n\
         mut c = a + b;\n\
         mut d: UInt32 = 2 as UInt32;\n\
         mut e: Int64 = -3;\n\
         mut f = d + e;",
    );
}

#[test]
fn valid_mixed_width_float_and_int_arithmetic() {
    accepted_body(
        "mut a: Int8 = 1;\n\
         mut b: Float16 = 2.0f16;\n\
         mut c = a + b;\n\
         mut d: UInt32 = 4 as UInt32;\n\
         mut e: Float64 = 8.0;\n\
         mut f = d + e;",
    );
}

#[test]
fn valid_increment_and_decrement_keep_narrow_widths() {
    accepted_body(
        "mut a: Int8 = 1;\n\
         a++;\n\
         --a;\n\
         mut b: Float16 = 1.0f16;\n\
         b--;",
    );
}

#[test]
fn valid_string_repetition_accepts_any_integer_width() {
    accepted_body(
        "mut a: Int8 = 3;\n\
         mut b: UInt64 = 4 as UInt64;\n\
         mut s = \"x\" * a;\n\
         mut t = b * \"y\";",
    );
}

#[test]
fn invalid_int_literal_out_of_range_for_narrow_width() {
    let output = rejected_body("mut a: Int8 = 128;");
    assert!(output.contains(codes::INTEGER_OUT_OF_RANGE.as_str()));
}

#[test]
fn invalid_int_literal_to_float_not_exactly_representable() {
    let output = rejected_body("mut a: Float16 = 100000;");
    assert!(output.contains(codes::INTEGER_OUT_OF_RANGE.as_str()));
}

#[test]
fn invalid_float_literal_with_fraction_for_int() {
    let output = rejected_body("mut a: Int = 1.5;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_no_common_numeric_type() {
    let output =
        rejected_body("mut a: UInt128 = 1 as UInt128;\nmut b: Int128 = 2;\nmut c = a + b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_comparison_between_decimal_and_float() {
    // Ordered comparisons widen to the smallest common numeric type, so
    // `Int32 < Float64` is accepted. A `Decimal`/`Float` pair has no common
    // type — the exact type never joins a binary float — and stays rejected.
    let output = rejected_body("mut a: Decimal = 1.5;\nmut b: Float64 = 1.0;\nmut c = a < b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_comparison_between_integer_and_float() {
    accepted_body("mut a: Int32 = 1;\nmut b: Float64 = 1.0;\nmut c = a < b;");
}

#[test]
fn valid_comparison_between_signed_and_unsigned_integer() {
    accepted_body("mut a: Int32 = 1;\nmut b: UInt64 = 2;\nmut c = a < b;");
}

#[test]
fn valid_comparison_between_decimal_and_integer() {
    accepted_body("mut a: Decimal = 1.5;\nmut b: Int64 = 2;\nmut c = a < b;");
}

#[test]
fn valid_unary_negation_on_float() {
    accepted_body("mut a: Float64 = 1.5;\nmut b = -a;");
}

#[test]
fn invalid_bitwise_not_on_float() {
    let output = rejected_body("mut a: Float64 = 1.5;\nmut b = ~a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_char_literal_of_a_single_code_point() {
    accepted_body("mut a: Char = 'a';");
}

#[test]
fn valid_char_literal_of_an_extended_grapheme_cluster() {
    // A family emoji joined by ZWJ: several code points, one grapheme.
    accepted_body("mut a: Char = '👨‍👩‍👧‍👦';");
}

#[test]
fn invalid_char_literal_of_more_than_one_grapheme() {
    let output = rejected_body("mut a: Char = 'ab';");
    assert!(output.contains(codes::INVALID_CHAR_LITERAL.as_str()));
}

#[test]
fn invalid_char_literal_that_is_empty() {
    let output = rejected_body("mut a: Char = '';");
    assert!(output.contains(codes::INVALID_CHAR_LITERAL.as_str()));
}

#[test]
fn valid_char_equality() {
    accepted_body("mut a: Char = 'a';\nmut b: Char = 'a';\nmut c = a == b;");
}

#[test]
fn invalid_char_identity_comparison() {
    let output = rejected_body("mut a: Char = 'a';\nmut b: Char = 'a';\nmut c = a is b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_temporal_type_states_its_phase() {
    let output = rejected_body("mut x: Instant = 1;");
    assert!(output.contains("Phase 7"), "{output}");
}

#[test]
fn invalid_decimal_is_not_a_type_of_the_language() {
    // The family was removed: announcing a phase would teach a language that
    // does not exist.
    let output = rejected_body("mut x: Decimal64 = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(!output.contains("Phase"), "{output}");
}

#[test]
fn invalid_type_that_does_not_exist() {
    let output = rejected_body("mut x: Whatever = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(output.contains("Whatever"));
}

#[test]
fn invalid_variable_of_type_void() {
    let output = rejected_body("mut x: Void = 1;");
    assert!(output.contains(codes::VOID_VARIABLE.as_str()));
}

// --- No implicit conversions ------------------------------------------------

#[test]
fn valid_assignment_of_a_matching_type() {
    accepted_body("mut total: Int32 = 40;");
}

#[test]
fn invalid_assignment_of_an_incompatible_type() {
    let output = rejected_body("mut total: Int32 = \"cuarenta\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("no implicit conversion"));
}

#[test]
fn invalid_arithmetic_between_different_types() {
    let output = rejected_body("mut x = 1 + \"a\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_bitwise_and_shift_on_int32() {
    // Roadmap Phase 3b, task 4.4 — over `Int32` for now.
    accepted_body(
        "mut a = 6; mut b = 3;
         mut x = a & b;
         mut y = a | b;
         mut z = a ^ b;
         mut w = ~a;
         mut s = a << 2;
         mut t = a >> 1;",
    );
}

#[test]
fn invalid_bitwise_on_boolean() {
    let output = rejected_body("mut a = true; mut x = a & true;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("requires numbers"));
}

// --- No truthiness ----------------------------------------------------------

#[test]
fn valid_boolean_condition() {
    accepted_body("mut x: Int32 = 1;\nif x > 0 { }");
}

#[test]
fn invalid_numeric_condition() {
    let output = rejected_body("if 1 { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(
        output.contains("truthiness"),
        "the help must name the reason:\n{output}"
    );
}

#[test]
fn invalid_string_condition() {
    let output = rejected_body("if \"x\" { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- Logical operators ------------------------------------------------------

#[test]
fn valid_logical_operators_over_booleans() {
    accepted_body("mut a = true;\nmut b = false;\nmut c = a && b || !a;");
}

#[test]
fn invalid_conjunction_over_integers() {
    let output = rejected_body("mut x = 1 && 2;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_negation_of_a_number() {
    let output = rejected_body("mut x = !5;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- Mutability -------------------------------------------------------------

#[test]
fn valid_reassignment_of_a_mutable_variable() {
    accepted_body("mut count: Int32 = 0;\ncount = 1;");
}

#[test]
fn invalid_reassignment_of_an_immutable_variable() {
    let output = rejected_body("inmut NAME: String = \"Zirk\";\nNAME = \"other\";");
    assert!(output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()));
    assert!(output.contains("= help:"));
}

// --- `mut`/`inmut`/`inmut::strict` matrix on object references (D11) --------

#[test]
fn valid_strict_reference_to_a_fresh_object() {
    accepted(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void { inmut::strict s = User(\"ana\"); }",
    );
}

#[test]
fn invalid_reassignment_of_a_strict_variable() {
    let output = rejected_body(
        "class User { name: String; construct(name: String) { this.name = name; } }
         inmut::strict s = User(\"ana\");
         s = User(\"beto\");",
    );
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_mutable_alias_of_a_strict_object_reference() {
    // A strict reference cannot produce a mutable alias of its object (D11).
    let output = rejected(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void {
             inmut::strict s = User(\"ana\");
             mut alias = s;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_strict_reference_acquired_from_an_accessible_mutable_alias() {
    // A strict reference cannot be acquired while a `mut` alias of the same
    // object is still reachable (D11).
    let output = rejected(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void {
             mut m = User(\"ana\");
             inmut::strict s = m;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

// --- `inmut::strict` projection writes (fase-4e-inmut-strict-proyeccion) ----

#[test]
fn invalid_write_through_strict_projection() {
    let output = rejected(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             inmut::strict p = Point(1);
             p.x = 5;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_write_through_multi_level_strict_projection() {
    // Confirms the root-binding walk is not limited to one field access.
    let output = rejected(
        "class Inner { z: Int32; construct(z: Int32) { this.z = z; } }
         class Middle { inner: Inner; construct(inner: Inner) { this.inner = inner; } }
         class Outer { middle: Middle; construct(middle: Middle) { this.middle = middle; } }
         fn main(): Void {
             inmut::strict o = Outer(Middle(Inner(1)));
             o.middle.inner.z = 5;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn valid_write_through_mut_projection() {
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             mut p = Point(1);
             p.x = 5;
         }",
    );
}

#[test]
fn valid_write_through_non_strict_inmut_projection() {
    // `inmut` (non-strict) only prohibits rebinding `p` itself; mutating
    // what it reaches is a matter of the field's own `inmut`, not the
    // reference's (`ZIRK_SPEC_FINAL.md`/spec: only `inmut::strict`
    // prohibits referent mutation).
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             inmut p = Point(1);
             p.x = 5;
         }",
    );
}

#[test]
fn invalid_multi_assign_write_through_strict_projection() {
    let output = rejected(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             inmut::strict p = Point(1);
             mut left: Int32 = 0;
             left, p.x = 1, 5;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn valid_constructor_field_write_unaffected_by_strict_projection_check() {
    // `Expr::This` returns `None` from `root_binding_mutability` (design D1
    // of `fase-4e-inmut-strict-proyeccion`), so the in-constructor exemption
    // keeps working regardless of the new projection check — a regression
    // guard, not new behavior.
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void { Point(1); }",
    );
}

// --- Comma-grouped declarations and simultaneous assignment (roadmap Phase 4d) --

#[test]
fn valid_multi_let_type_and_permission_fan_out() {
    accepted_body(
        "mut first, second: String = \"a\", \"b\";
         inmut third, fourth: Int32 = 1, 2;
         first = \"c\";
         second = \"d\";",
    );
}

#[test]
fn invalid_multi_let_initializer_arity_mismatch() {
    let output = rejected_body("mut a, b: Int32 = 1, 2, 3;");
    assert!(
        output.contains(codes::MULTI_LET_ARITY_MISMATCH.as_str()),
        "{output}"
    );
}

#[test]
fn valid_multi_let_missing_initializer_uses_type_default() {
    // `zirk-type-system`'s "Default initialization": no initializer list
    // still leaves every binding usable, defaulted independently.
    accepted_body(
        "mut a, b: Int32;
         a = a + 1;
         b = b + 1;",
    );
}

#[test]
fn invalid_multi_assign_arity_mismatch() {
    let output = rejected_body(
        "mut left: Int32 = 1;
         mut right: Int32 = 2;
         left, right = right, left, 9;",
    );
    assert!(
        output.contains(codes::MULTI_ASSIGN_ARITY_MISMATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_multi_assign_duplicate_destination() {
    let output = rejected_body(
        "mut a: Int32 = 1;
         mut b: Int32 = 2;
         a, a = b, 3;",
    );
    assert!(
        output.contains(codes::DUPLICATE_ASSIGN_TARGET.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_multi_assign_rebinding_immutable() {
    let output = rejected_body(
        "inmut a: Int32 = 1;
         mut b: Int32 = 2;
         a, b = 5, 6;",
    );
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_multi_assign_rebinding_strict() {
    let output = rejected_body(
        "inmut::strict a: Int32 = 1;
         mut b: Int32 = 2;
         a, b = 5, 6;",
    );
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_multi_assign_swap_type_checks() {
    accepted_body(
        "mut left: Int32 = 3;
         mut right: Int32 = 4;
         left, right = right, left;",
    );
}

#[test]
fn valid_inmut_alias_of_a_mutable_object_reference() {
    // Only `mut` targets and `inmut::strict` sources trigger the matrix;
    // ordinary `inmut` sharing a mutable object's reference is unrestricted.
    accepted(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void {
             mut m = User(\"ana\");
             inmut alias = m;
         }",
    );
}

#[test]
fn valid_strict_alias_of_a_strict_object_reference() {
    accepted(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void {
             inmut::strict s = User(\"ana\");
             inmut::strict alias = s;
         }",
    );
}

// --- Inference --------------------------------------------------------------

#[test]
fn valid_inference_from_the_initializer() {
    // `mut x = 5` infers Int32: with a single integer type in the subset the
    // inference is unambiguous, which is what `LANGUAGE_SPEC` section 2 asks.
    accepted_body("mut a = 5;\nmut b: Int32 = a;");
}

#[test]
fn valid_inference_of_string_and_boolean() {
    accepted_body("mut a = \"x\";\nmut b: String = a;\nmut c = true;\nmut d: Boolean = c;");
}

#[test]
fn invalid_inferred_type_is_still_checked() {
    let output = rejected_body("mut a = 5;\nmut b: String = a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- Name resolution --------------------------------------------------------

#[test]
fn valid_use_of_a_declared_variable() {
    accepted_body("mut x: Int32 = 1;\nmut y: Int32 = x;");
}

#[test]
fn invalid_undeclared_identifier() {
    let output = rejected_body("mut y: Int32 = x;");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()));
    assert!(output.contains('x'));
}

#[test]
fn invalid_variable_outside_its_scope() {
    let output = rejected_body("{ mut inner: Int32 = 1; }\nmut y: Int32 = inner;");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()));
}

#[test]
fn invalid_inner_block_shadows_the_outer_name() {
    // There is no ordinary shadowing (D10).
    let output = rejected_body("mut x: Int32 = 1;\n{ mut x: String = \"a\"; mut y: String = x; }");
    assert!(
        output.contains(codes::ORDINARY_SHADOWING.as_str()),
        "{output}"
    );
}

#[test]
fn valid_sibling_blocks_reuse_a_name() {
    // The first `x` goes out of scope before the second one exists, so
    // neither ever hides the other — this is not shadowing.
    accepted_body("{ mut x: Int32 = 1; }\n{ mut x: String = \"a\"; }");
}

// --- Use before initialization ----------------------------------------------

#[test]
fn valid_read_after_assigning() {
    accepted_body("mut x: Int32;\nx = 1;\nmut y: Int32 = x;");
}

#[test]
fn invalid_read_before_assigning() {
    let output = rejected_body("mut x: Int32;\nmut y: Int32 = x;");
    assert!(output.contains(codes::USE_BEFORE_INITIALIZATION.as_str()));
}

// --- Calls ------------------------------------------------------------------

#[test]
fn valid_call_matching_the_signature() {
    accepted(
        "fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { mut x: Int32 = add(1, 2); }",
    );
}

#[test]
fn valid_a_function_can_call_one_declared_later() {
    accepted("fn main(): Void { mut x: Int32 = later(); }\nfn later(): Int32 { return 1; }");
}

#[test]
fn invalid_wrong_argument_count() {
    let output = rejected(
        "fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { mut x: Int32 = add(1); }",
    );
    assert!(output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()));
}

#[test]
fn invalid_argument_of_the_wrong_type() {
    let output = rejected(
        "fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { mut x: Int32 = add(1, \"two\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_call_to_an_undeclared_function() {
    let output = rejected_body("missing();");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()));
}

#[test]
fn invalid_two_functions_with_the_same_name() {
    let output = rejected("fn f(): Void { }\nfn f(): Void { }\nfn main(): Void { }");
    assert!(output.contains(codes::DUPLICATE_FUNCTION.as_str()));
    assert!(
        output.contains("overloading"),
        "the help must explain there is no overloading:\n{output}"
    );
}

// --- Return coherence -------------------------------------------------------

#[test]
fn valid_return_of_the_declared_type() {
    accepted("fn f(): Int32 { return 1; }\nfn main(): Void { }");
}

#[test]
fn valid_every_path_returns() {
    accepted(
        "fn f(a: Boolean): Int32 { if a { return 1; } else { return 2; } }\nfn main(): Void { }",
    );
}

#[test]
fn invalid_return_of_the_wrong_type() {
    let output = rejected("fn f(): Int32 { return \"x\"; }\nfn main(): Void { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_path_without_a_return() {
    // Without the `else` branch, falling through returns nothing.
    let output = rejected("fn f(a: Boolean): Int32 { if a { return 1; } }\nfn main(): Void { }");
    assert!(output.contains(codes::MISSING_RETURN.as_str()));
}

#[test]
fn invalid_value_returned_from_a_void_function() {
    let output = rejected("fn f(): Void { return 1; }\nfn main(): Void { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- Integer overflow -------------------------------------------------------

#[test]
fn valid_literal_at_the_edge_of_the_range() {
    accepted_body("mut x: Int32 = 2147483647;");
}

#[test]
fn invalid_literal_out_of_range() {
    let output = rejected_body("mut x: Int32 = 2147483648;");
    assert!(output.contains(codes::INTEGER_OUT_OF_RANGE.as_str()));
    assert!(output.contains("Int32"));
}

// --- println ----------------------------------------------------------------

#[test]
fn valid_println_accepts_any_type() {
    // The checker admits any type. Turning the value into a `String` is the
    // lowering's job, and the IR verifier enforces that `Println` only ever
    // receives one — this test used to be read as if it covered that too, and
    // it never did: a value reaching the runtime raw crashed the program.
    accepted_body("stdout.println(\"a\");\nstdout.println(1);\nstdout.println(true);");
}

#[test]
fn valid_print_and_println_accept_multiple_or_zero_arguments() {
    accepted_body(
        "stdout.print(\"sum\", 1 + 2, true);\n\
         stdout.println(\"done\", 3 * 4);\n\
         stdout.print();\n\
         stdout.println();",
    );
}

#[test]
fn valid_the_reference_program_of_the_roadmap() {
    accepted("fn main(): Void {\n    stdout.println(\"Hola desde Zirk\");\n}");
}

// --- String interpolation (roadmap Phase 3b) ---------------------------------

#[test]
fn valid_string_interpolation_of_printable_types() {
    accepted_body(
        "mut name = \"ana\"; mut age = 1;
         mut s = \"Hola, {name}, edad {age}\";",
    );
}

#[test]
fn valid_string_interpolation_yields_string() {
    accepted_body("mut s: String = \"x {1}\";");
}

#[test]
fn invalid_string_interpolation_of_a_type_without_a_text_form() {
    // `Void` and `Never` are the only types still without a text form
    // (`universal-to-string-and-multiline-strings`): every value —
    // including opaque handles like `Pointer<T>` — renders something.
    let output =
        rejected("fn nothing(): Void { }\nfn main(): Void { mut s = \"v: {nothing()}\"; }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("cannot be printed"));
}

#[test]
fn valid_printing_of_the_new_scalars() {
    accepted_body(
        "mut a: Int8 = 1 as Int8;\nmut b: UInt64 = 1 as UInt64;\nmut c: Float64 = 1.5;\nmut d: Char = 'z';\nstdout.println(a);\nstdout.println(b);\nstdout.println(c);\nstdout.println(d);",
    );
}

#[test]
fn valid_printing_of_float16() {
    // Prints by widening to Float32 first — always exact, since every f16
    // value is representable in f32 without loss.
    accepted_body("mut a: Float16 = 1.5f16;\nstdout.println(a);");
}

#[test]
fn valid_printing_of_float128() {
    // Float128 prints by truncating to Float64; this is accepted but can lose
    // precision for values not exactly representable in f64.
    accepted_body("mut a: Float128 = 1.5f128;\nstdout.println(a);");
}

#[test]
fn valid_printing_of_a_class_with_to_string() {
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; }  #override to_string(): String { return \"{this.x}\"; } }
         fn main(): Void { mut p = Point(1); stdout.println(p); }",
    );
}

#[test]
fn valid_interpolation_of_a_class_with_to_string() {
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; }  to_string(): String { return \"{this.x}\"; } }
         fn main(): Void { mut p = Point(1); mut s = \"p: {p}\"; }",
    );
}

#[test]
fn valid_explicit_to_string_on_a_native_scalar() {
    accepted_body("mut a: Int32 = 1;\nmut s: String = a.to_string();");
}

#[test]
fn valid_explicit_to_string_on_a_string_is_identity() {
    accepted_body("mut s: String = \"already\".to_string();");
}

#[test]
fn invalid_explicit_to_string_with_arguments() {
    let output = rejected_body("mut a: Int32 = 1;\nmut s: String = a.to_string(2);");
    assert!(output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()));
}

#[test]
fn valid_to_string_through_a_contract_reference() {
    accepted(
        "contract Printable {  to_string(): String; }
         class Widget implements Printable { construct() { }  #override to_string(): String { return \"w\"; } }
         fn describe(p: Printable): Void { stdout.println(p); }
         fn main(): Void { mut w = Widget(); describe(w); }",
    );
}

#[test]
fn valid_universal_to_string_members_and_nullable_receivers() {
    accepted(
        "class Marker { construct() { } }\n\
         fn render(\n\
             map: Map<Int32, String>, set: Set<Int32>, range: Range<Int32>, regex: Regex,\n\
             channel: Channel<Int32>, job: Job<Int32>, pointer: Pointer<Int32>,\n\
             native: NativeSlice<Int32>, native_mut: NativeSliceMut<Int32>,\n\
             dependent: Dependent<Int32>, pin: Pin<Marker>, maybe: Int32?\n\
         ): Void {\n\
             mut a: String = map.to_string(); mut b: String = set.to_string();\n\
             mut c: String = range.to_string(); mut d: String = regex.to_string();\n\
             mut e: String = channel.to_string(); mut f: String = job.to_string();\n\
             mut g: String = pointer.to_string(); mut h: String = native.to_string();\n\
             mut i: String = native_mut.to_string(); mut j: String = dependent.to_string();\n\
             mut k: String = pin.to_string(); mut direct: String = maybe.to_string();\n\
             mut safe: String? = maybe?.to_string();\n\
         }\n\
         fn main(): Void { }",
    );
}

// --- Final-result scalar conversion ----------------------------------------

#[test]
fn valid_integer_division_produces_decimal() {
    accepted_body("mut a: Decimal = 3 / 4;");
}

#[test]
fn invalid_integer_division_is_not_implicitly_integer() {
    let output = rejected_body("mut a: Int32 = 3 / 4;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_scalar_conversion_converts_the_completed_quotient() {
    accepted_body("mut a: Float64 = Float64(3 / 4);");
    accepted_body("mut a: Int64 = Int64(3 / 4);");
}

#[test]
fn invalid_string_conversion_does_not_retype_its_operands() {
    let output = rejected_body("mut a: String = String(\"x=\" + 42);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_scalar_conversion_of_an_ordinary_result() {
    accepted_body("mut a: Int64 = Int64(3 + 4);");
    accepted_body("mut a: String = String(42);");
}

#[test]
fn valid_scalar_conversion_after_unary_negation() {
    accepted_body("mut a: Float64 = Float64(-3 / 4);");
}

#[test]
fn valid_scalar_conversion_after_a_call() {
    accepted(
        "fn half(n: Int32): Decimal { return n / 2; }
         fn main(): Void { mut a: Float64 = Float64(half(3)); }",
    );
}

#[test]
fn invalid_scalar_conversion_of_an_incompatible_result() {
    let output = rejected_body("mut a: Float64 = Float64(true + 4);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_scalar_conversion_wrong_argument_count() {
    let output = rejected_body("mut a: Float64 = Float64(1, 2);");
    assert!(output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()));
}

// --- `Never` and `fatalError` (roadmap Phase 4a) ----------------------------

#[test]
fn valid_never_is_assignable_anywhere() {
    accepted_body("mut a: Int32 = fatalError(\"x\");");
    accepted_body("mut a: String = fatalError(\"x\");");
    accepted_body("mut a: Boolean = fatalError(\"x\");");
}

#[test]
fn valid_never_unifies_at_a_ternary_join() {
    accepted_body("mut c: Boolean = true;\nmut a: Int32 = c ? 5 : fatalError(\"x\");");
    accepted_body("mut c: Boolean = true;\nmut a: Int32 = c ? fatalError(\"x\") : 5;");
}

#[test]
fn valid_never_unifies_at_an_if_expression_join() {
    accepted_body("mut c: Boolean = true;\nmut a: Int32 = if c { 5 } else { fatalError(\"x\") };");
}

#[test]
fn valid_fatal_error_as_a_bare_statement() {
    accepted_body("fatalError(\"x\");");
}

#[test]
fn invalid_fatal_error_wrong_argument_type() {
    let output = rejected_body("fatalError(5);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_empty_enum_states_to_use_never() {
    let output = rejected("enum Impossible { }\nfn main(): Void { }");
    assert!(output.contains(codes::DUPLICATE_DECLARATION.as_str()));
    assert!(output.contains("Never"));
}

// --- `Result<T,E>` (roadmap Phase 4a) ---------------------------------------

#[test]
fn valid_result_construction_and_match() {
    accepted(
        "fn load(fail: Boolean): Result<Int32, String> {\n\
             if fail { return Error(\"boom\"); }\n\
             return Ok(5);\n\
         }\n\
         fn main(): Void {\n\
             mut r: Result<Int32, String> = load(false);\n\
             match r {\n\
                 Ok(value) => stdout.println(value);\n\
                 Error(error) => stdout.println(error);\n\
             }\n\
         }",
    );
}

#[test]
fn valid_result_contextual_projection() {
    accepted_body(
        "mut r: Result<Int32, String> = Ok(5);\n\
         mut projected: Int32? = r;\n\
         mut fallback: Int32 = r ?? 0;\n\
         mut matched: Int32 = match r { Ok(matched_value) => matched_value; Error(_) => -1; };\n\
         mut preserved: Result<Int32, String> = r;",
    );
}

#[test]
fn invalid_result_unknown_method() {
    let output = rejected_body("mut r: Result<Int32, String> = Ok(5);\nr.map_to_string();");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()));
}

#[test]
fn invalid_result_get_or_wrong_argument_type() {
    let output =
        rejected_body("mut r: Result<Int32, String> = Ok(5);\nmut a: Int32 = r.get_or(\"nope\");");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_result_get_or_wrong_argument_count() {
    let output = rejected_body("mut r: Result<Int32, String> = Ok(5);\nmut a: Int32 = r.get_or();");
    assert!(output.contains("Result.get_or` is no longer available"));
}

#[test]
fn invalid_result_cannot_be_reimplemented() {
    let output = rejected("enum Result { Ok; }\nfn main(): Void { }");
    assert!(output.contains(codes::DUPLICATE_DECLARATION.as_str()));
}

#[test]
fn valid_result_used_as_a_bare_statement() {
    accepted(
        "fn load(): Result<Int32, String> { return Ok(5); }\n\
         fn main(): Void { load(); }",
    );
}

#[test]
fn valid_result_discarded_explicitly_with_underscore() {
    accepted(
        "fn load(): Result<Int32, String> { return Ok(5); }\n\
         fn main(): Void { _ = load(); }",
    );
}

// --- Error recovery ---------------------------------------------------------

#[test]
fn invalid_one_error_does_not_cascade() {
    // `x` is undeclared: that is one error. Everything downstream involves an
    // unknown type and must stay silent instead of piling on derived errors.
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        in_main("mut y: Int32 = x + 1 + 2 + 3;"),
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    check(&sources, &program, &mut sink);

    assert_eq!(
        sink.len(),
        1,
        "only the original error was expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}

#[test]
fn invalid_several_independent_errors_are_all_reported() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        in_main("mut a: Int32 = \"x\";\nmut b: Boolean = 1;"),
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    check(&sources, &program, &mut sink);

    assert_eq!(
        sink.len(),
        2,
        "both errors were expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}

// --- Ternary, increment and `do ... while` ----------------------------------

#[test]
fn valid_ternary_with_branches_that_agree() {
    accepted_body("mut x = 1;\nmut label: String = x > 0 ? \"yes\" : \"no\";");
}

#[test]
fn invalid_ternary_with_branches_that_disagree() {
    let output = rejected_body("mut x = 1;\nmut label = x > 0 ? \"yes\" : 0;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("ternary"), "{output}");
}

#[test]
fn invalid_ternary_condition_is_not_boolean() {
    // No truthiness, here as everywhere else.
    let output = rejected_body("mut x = 1;\nmut label = x ? \"yes\" : \"no\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_increment_as_expression() {
    accepted_body("mut i = 0;\nmut previous = i++;\nmut next = ++i;");
}

#[test]
fn invalid_increment_of_an_immutable_binding() {
    let output = rejected_body("inmut I = 0;\nmut x = I++;");
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_increment_of_a_non_numeric_value() {
    let output = rejected_body("mut s = \"a\";\nmut x = s++;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_do_while_checks_a_boolean_condition() {
    accepted_body("mut i = 0;\ndo { i += 1; } while i < 10;");
}

#[test]
fn invalid_do_while_condition_is_not_boolean() {
    let output = rejected_body("mut i = 0;\ndo { i += 1; } while i;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_break_inside_a_do_while() {
    accepted_body("do { break; } while true;");
}

#[test]
fn valid_conditional_without_braces() {
    accepted_body("mut closed = true;\nif closed return;");
}

#[test]
fn valid_string_iteration_binds_a_char() {
    // `for ... in` over a `String` produces `Char` (roadmap Phase 3b, task
    // 6.3, retiring the debt tracked since Phase 2).
    accepted_body("mut text = \"hola\";\nfor c in text { mut x: Char = c; }");
}

// --- `Iterable<T>` / `Iterator<T>` (D8, task 6.9/6.10) -----------------------

#[test]
fn valid_range_iteration_is_unaffected_by_iterable() {
    // A range keeps resolving natively — it does not write
    // `implements Iterable<Int32>`, and none of this is expected to gate it.
    accepted_body("mut sum = 0;\nfor i in 0..5 { sum = sum + i; }");
}

#[test]
fn invalid_iterating_a_type_that_does_not_implement_iterable() {
    let output = rejected(
        "class Foo { construct() { } }
         fn main(): Void {
             mut f = Foo();
             for x in f { }
         }",
    );
    assert!(output.contains(codes::NOT_ITERABLE.as_str()), "{output}");
    assert!(output.contains("Iterable"), "{output}");
}

#[test]
fn invalid_reopening_the_native_iterable_contract() {
    let output = rejected("contract Iterable {  foo(): Int32; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_reopening_the_native_iterator_contract() {
    let output = rejected("trait Iterator {  foo(): Int32; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_reopening_the_native_iteration_enum() {
    let output = rejected("enum Iteration { A, B }\nfn main(): Void { }");
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn valid_a_class_implementing_iterable_lowers() {
    // `implements Iterable<Int32>`/`Iterator<Int32>` are checked for real
    // conformance (`iterator()`/`next()` required with the right signature)
    // and now lower too (roadmap task 13.5): dispatch through a contract's
    // table (10.7) plus a concrete `Iteration<Int32>` (11.3, specialized the
    // way a generic class's own fields are, 11.1).
    accepted(
        "class Counter implements Iterable<Int32> {
             construct() { }
              iterator(): Iterator<Int32> { return CounterIterator(); }
         }
         class CounterIterator implements Iterator<Int32> {
             construct() { }
              next(): Iteration<Int32> { return Iteration.Done; }
         }
         fn main(): Void { }",
    );
}

#[test]
fn generic_contract_assignability_preserves_the_implemented_type_argument() {
    let implementation = "class Counter implements Iterable<Int32> {
             construct() { }
              iterator(): Iterator<Int32> { return CounterIterator(); }
         }
         class CounterIterator implements Iterator<Int32> {
             construct() { }
              next(): Iteration<Int32> { return Iteration.Done; }
         }";
    accepted(&format!(
        "{implementation}\nfn take(values: Iterable<Int32>): Void {{ }}\nfn main(): Void {{ take(Counter()); }}"
    ));

    let wrong_argument = rejected(&format!(
        "{implementation}\nfn take(values: Iterable<String>): Void {{ }}\nfn main(): Void {{ take(Counter()); }}"
    ));
    assert!(
        wrong_argument.contains(codes::TYPE_MISMATCH.as_str()),
        "{wrong_argument}"
    );
}

#[test]
fn classes_satisfy_structural_shapes_only_through_public_compatible_fields() {
    let public = "type HasId = { inmut id: String };\n\
         class Named { id: String; }\n\
         fn take(value: HasId): Void { }\n\
         fn main(): Void { take(Named()); mut item: HasId = { id: \"literal\" }; }";
    let (_, public_diagnostics) = parse_and_check(public);
    let public_output = public_diagnostics.render(RenderStyle::Human);
    assert!(
        !public_output.contains(codes::TYPE_MISMATCH.as_str()),
        "a public read-only compatible field should satisfy the shape: {public_output}"
    );

    let private = "type HasId = { inmut id: String };\n\
         class Hidden { private id: String; }\n\
         fn take(value: HasId): Void { }\
         fn main(): Void { take(Hidden()); }";
    let (_, private_diagnostics) = parse_and_check(private);
    let private_output = private_diagnostics.render(RenderStyle::Human);
    assert!(
        private_output.contains(codes::TYPE_MISMATCH.as_str()),
        "private fields must not satisfy a public structural requirement: {private_output}"
    );

    let missing = "type HasId = { inmut id: String };\n\
         fn take(value: HasId): Void { }\n\
         fn main(): Void { mut item: HasId = { extra: 1 }; }";
    let (_, missing_diagnostics) = parse_and_check(missing);
    let missing_output = missing_diagnostics.render(RenderStyle::Human);
    assert!(
        missing_output.contains("missing required property `id`"),
        "a structural literal must provide each required property: {missing_output}"
    );
}

#[test]
fn structural_optional_reads_are_nullable_and_readonly_views_reject_writes() {
    let valid = "type Profile = { name: String; nickname?: String; mut score: Int32 };\n\
         fn inspect(profile: Profile): Void { mut nickname: String? = profile.nickname; }\n\
         fn main(): Void { mut profile: Profile = { name: \"Ada\", score: 1 }; profile.score = 2; profile.nickname = \"A\"; }";
    let (_, valid_diagnostics) = parse_and_check(valid);
    let valid_output = valid_diagnostics.render(RenderStyle::Human);
    assert!(
        !valid_output.contains(codes::TYPE_MISMATCH.as_str()),
        "optional reads and mutable shape writes should type-check: {valid_output}"
    );

    let (_, readonly_diagnostics) = parse_and_check(
        "type Profile = { inmut name: String };\n\
         fn main(): Void { mut profile: Profile = { name: \"Ada\" }; profile.name = \"Grace\"; }",
    );
    let readonly = readonly_diagnostics.render(RenderStyle::Human);
    assert!(
        readonly.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{readonly}"
    );

    let (_, non_nullable_diagnostics) = parse_and_check(
        "type Profile = { nickname?: String };\n\
         fn inspect(profile: Profile): Void { mut nickname: String = profile.nickname; }\n\
         fn main(): Void { }",
    );
    let non_nullable = non_nullable_diagnostics.render(RenderStyle::Human);
    assert!(
        non_nullable.contains(codes::TYPE_MISMATCH.as_str()),
        "optional field reads must include absence: {non_nullable}"
    );

    let explicit_null = rejected(
        "type Profile = { nickname?: String };\n\
         fn main(): Void { mut profile: Profile = { nickname: null }; }",
    );
    assert!(
        explicit_null.contains(codes::TYPE_MISMATCH.as_str()),
        "an optional property's declared value type excludes Null unless declared nullable: {explicit_null}"
    );
}

#[test]
fn structural_literals_check_excess_keys_width_and_known_spreads() {
    let valid = "type Source = { id: Int32; label?: String };\n\
         type Target = { id: Int32 };\n\
         fn take(target: Target): Void { }\n\
         fn forward(source: Source): Void { take(source); mut target: Target = { ...source }; }\n\
         fn main(): Void { }";
    let (_, valid_diagnostics) = parse_and_check(valid);
    let valid_output = valid_diagnostics.render(RenderStyle::Human);
    assert!(
        !valid_output.contains(codes::TYPE_MISMATCH.as_str()),
        "typed width compatibility and a known compatible spread should pass: {valid_output}"
    );

    let excess = rejected(
        "type Target = { id: Int32 };\n\
         fn main(): Void { mut target: Target = { id: 1, extra: true }; }",
    );
    assert!(excess.contains("unknown property `extra`"), "{excess}");

    let unknown_spread = rejected(
        "type Target = { id: Int32 };\n\
         fn main(): Void { mut source: Object = { id: 1 }; mut target: Target = { ...source }; }",
    );
    assert!(
        unknown_spread.contains("missing required property `id`"),
        "an unknown spread cannot establish a required property: {unknown_spread}"
    );

    let invalid_spread = rejected(
        "type Target = { id: Int32 };\n\
         fn main(): Void { mut target: Target = { ...1 }; }",
    );
    assert!(
        invalid_spread.contains("object spread requires an object value"),
        "{invalid_spread}"
    );
}

#[test]
fn union_object_literals_collect_candidates_and_contextualize_nested_values() {
    let (sources, sink) = parse_and_check(
        "type Enabled = { active: true; payload: { amount: Int8 } };\n\
         type Disabled = { active: false; payload: { reason: String } };\n\
         type Choice = Enabled | Disabled;\n\
         fn consume(value: Choice): Void { }\n\
         fn choose(): Choice { return { active: true, payload: { amount: 7 } }; }\n\
         fn main(): Void { consume({ active: true, payload: { amount: 7 } }); }",
    );
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    let _ = sources;

    let ambiguous = rejected(
        "type Left = { value: Int32; left?: Boolean }; type Right = { value: Int32; right?: Boolean }; type Choice = Left | Right; fn choose(): Choice { return { value: 7 }; } fn main(): Void { }",
    );
    assert!(
        ambiguous.contains("ambiguous between union alternatives"),
        "{ambiguous}"
    );
    assert!(
        ambiguous.contains("let value: Left = { ... }"),
        "ambiguity guidance should show an explicit alternative annotation: {ambiguous}"
    );
    assert!(
        ambiguous.contains("known fields remain compatible with: { left?: Boolean, value: Int32 } | { right?: Boolean, value: Int32 }"),
        "candidate names should be listed deterministically: {ambiguous}"
    );
    let reversed_ambiguity = rejected(
        "type Left = { value: Int32; left?: Boolean }; type Right = { value: Int32; right?: Boolean }; type Choice = Right | Left; fn choose(): Choice { return { value: 7 }; } fn main(): Void { }",
    );
    assert!(
        reversed_ambiguity.contains("known fields remain compatible with: { left?: Boolean, value: Int32 } | { right?: Boolean, value: Int32 }"),
        "reordering union alternatives must not reorder the ambiguity report: {reversed_ambiguity}"
    );

    let no_candidate = rejected(
        "type Left = { value: Int32 }; type Right = { name: String }; type Choice = Left | Right; fn choose(): Choice { return { extra: true }; } fn main(): Void { }",
    );
    assert!(
        no_candidate.contains("matches no alternative"),
        "{no_candidate}"
    );
    assert!(
        no_candidate.contains("missing required property `value`"),
        "{no_candidate}"
    );
    assert!(
        no_candidate.contains("missing required property `name`"),
        "{no_candidate}"
    );

    let callable_candidates = "type ReturnsInt = { map: Fn(Int32) => Int32 };\n\
         type ReturnsText = { map: Fn(Int32) => String };\n\
         type CallableChoice = ReturnsInt | ReturnsText;\n\
         fn choose_callable(): CallableChoice { return { map: (value): Int32 => value }; }\n\
         fn main(): Void { }";
    let (_, callable_sink) = parse_and_check(callable_candidates);
    let callable_output = callable_sink.render(RenderStyle::Human);
    assert!(
        !callable_output.contains("ambiguous between union alternatives"),
        "the lambda's declared return type must filter incompatible candidates: {callable_output}"
    );
    assert!(
        !callable_output.contains(codes::TYPE_MISMATCH.as_str()),
        "{callable_output}"
    );

    let singleton_candidates = "enum Shade { Red, Blue }\n\
         type FloatLeft = { choice: 1.5f64; left: Int32 };\n\
         type FloatRight = { choice: 2.5f64; right: Int32 };\n\
         type DateLeft = { choice: Date(2026, 12, 25); left: Int32 };\n\
         type DateRight = { choice: Date(2027, 1, 1); right: Int32 };\n\
         type TimeLeft = { choice: Time(9, 30); left: Int32 };\n\
         type TimeRight = { choice: Time(17, 0); right: Int32 };\n\
         type DateTimeLeft = { choice: DateTime(Date(2026, 12, 25), Time(9, 30)); left: Int32 };\n\
         type DateTimeRight = { choice: DateTime(Date(2027, 1, 1), Time(0, 0)); right: Int32 };\n\
         type EnumLeft = { choice: Shade.Red; left: Int32 };\n\
         type EnumRight = { choice: Shade.Blue; right: Int32 };\n\
         type ValueLeft = { choice: { x: 1 }; left: Int32 };\n\
         type ValueRight = { choice: { x: 2 }; right: Int32 };\n\
         type DirectLeft = { choice: 6; left: Int32 };\n\
         type DirectRight = { choice: 8; right: Int32 };\n\
         type FloatChoice = FloatLeft | FloatRight;\n\
         type DateChoice = DateLeft | DateRight;\n\
         type TimeChoice = TimeLeft | TimeRight;\n\
         type DateTimeChoice = DateTimeLeft | DateTimeRight;\n\
         type EnumChoice = EnumLeft | EnumRight;\n\
         type ValueChoice = ValueLeft | ValueRight;\n\
         type DirectChoice = DirectLeft | DirectRight;\n\
         fn float_choice(): FloatChoice { return { choice: 1.5f64, left: 1 }; }\n\
         fn date_choice(): DateChoice { return { choice: Date(2026, 12, 25), left: 1 }; }\n\
         fn time_choice(): TimeChoice { return { choice: Time(9, 30), left: 1 }; }\n\
         fn datetime_choice(): DateTimeChoice { return { choice: DateTime(Date(2026, 12, 25), Time(9, 30)), left: 1 }; }\n\
         fn enum_choice(): EnumChoice { return { choice: Shade.Red, left: 1 }; }\n\
         fn value_choice(): ValueChoice { return { choice: { x: 1 }, left: 1 }; }\n\
         fn direct_choice(): DirectChoice { return { choice: 6, left: 1 }; }\n\
         fn main(): Void { }";
    let (_, singleton_sink) = parse_and_check(singleton_candidates);
    let singleton_output = singleton_sink.render(RenderStyle::Human);
    assert!(
        !singleton_output.contains("ambiguous between union alternatives"),
        "{singleton_output}"
    );
    assert!(
        !singleton_output.contains(codes::TYPE_MISMATCH.as_str()),
        "{singleton_output}"
    );

    let wrong_discriminant = rejected(
        "type One = { tag: 1; one: Int32 }; type Two = { tag: 2; two: Int32 }; type Choice = One | Two; fn choose(): Choice { return { tag: 3, one: 1 }; } fn main(): Void { }",
    );
    assert!(
        wrong_discriminant.contains("matches no alternative"),
        "a value outside all singleton discriminants must not select an alternative: {wrong_discriminant}"
    );

    let nested_mismatch = "type Counted = { payload: { count: Int32 } };\n\
         type Named = { payload: { name: String } };\n\
         type Choice = Counted | Named;\n\
         fn choose(): Choice { return { payload: { count: \"wrong\" } }; }\n\
         fn main(): Void { }";
    let (_, nested_sink) = parse_and_check(nested_mismatch);
    let nested_output = nested_sink.render(RenderStyle::Human);
    assert!(
        nested_output.contains("property `count` has a value incompatible with `Int32`"),
        "zero-candidate diagnostics should report the closest nested field mismatch: {nested_output}"
    );
    let diagnostic = nested_sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.message.contains("matches no alternative"))
        .expect("a zero-candidate diagnostic is emitted");
    let location = diagnostic
        .location
        .as_ref()
        .expect("diagnostic is source-located");
    let source_line = nested_mismatch.lines().nth(3).unwrap();
    let expected_column = source_line.find("\"wrong\"").unwrap() as u32 + 1;
    assert_eq!(location.line, 4);
    assert_eq!(location.column, expected_column, "{nested_output}");
}

#[test]
fn type_algebra_diagnostics_keep_codes_locations_causes_and_next_steps() {
    let relation_limit = zirk_sema::type_budget::BudgetKind::NormalizationMembers.limit() as usize;
    let mut wide = String::new();
    for index in 0..=relation_limit {
        wide.push_str(&format!("enum Option{index} {{ Item }}\n"));
    }
    let alternatives = (0..=relation_limit)
        .map(|index| format!("Option{index}"))
        .collect::<Vec<_>>()
        .join(" | ");
    wide.push_str(&format!(
        "fn take(value: {alternatives}): Void {{ }}\nfn main(): Void {{ }}"
    ));

    let cases = [
        (
            "incompatible union assignment",
            "type Choice = Int32 | String; fn take(value: Choice): Void { } fn main(): Void { take(true); }",
            codes::TYPE_MISMATCH,
        ),
        (
            "conflicting intersection members",
            "contract Named { name: String; } contract Numbered { name: Int32; } contract Both implements Named, Numbered { } fn main(): Void { }",
            codes::DUPLICATE_DECLARATION,
        ),
        (
            "ambiguous structural union literal",
            "type Left = { value: Int32; left?: Boolean }; type Right = { value: Int32; right?: Boolean }; type Choice = Left | Right; fn choose(): Choice { return { value: 7 }; } fn main(): Void { }",
            codes::TYPE_MISMATCH,
        ),
        (
            "bare temporal type literal",
            "type InvalidValue = 25:00; fn take(value: InvalidValue): Void { } fn main(): Void { }",
            zirk_parser::codes::UNEXPECTED_TOKEN,
        ),
        (
            "impossible type predicate",
            "class Box { } fn test(value: String): Boolean { return value is Box; } fn main(): Void { }",
            codes::TYPE_MISMATCH,
        ),
        (
            "type complexity budget",
            wide.as_str(),
            codes::TYPE_COMPUTATION_LIMIT,
        ),
    ];

    for (label, source, expected_code) in cases {
        let (_sources, sink) = parse_and_check(source);
        let diagnostic = sink
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == expected_code)
            .unwrap_or_else(|| {
                panic!(
                    "missing {label} diagnostic: {}",
                    sink.render(RenderStyle::Human)
                )
            });
        assert!(
            diagnostic.location.is_some(),
            "{label} must retain a source location"
        );
        assert!(
            !diagnostic.message.is_empty(),
            "{label} must have a stable message"
        );
        assert!(
            diagnostic
                .cause
                .as_ref()
                .is_some_and(|cause| !cause.is_empty()),
            "{label} must explain the mismatch"
        );
        assert!(
            diagnostic
                .help
                .as_ref()
                .is_some_and(|help| !help.is_empty()),
            "{label} must provide actionable guidance"
        );
    }
}

#[test]
fn composite_type_diagnostics_bound_their_utf8_display() {
    let alternatives = (0..8)
        .map(|index| format!("\"{}-{index}\"", "🙂".repeat(80)))
        .collect::<Vec<_>>()
        .join(" | ");
    let source = format!(
        "type Choice = {alternatives};\nfn consume(value: Int32): Void {{ }}\nfn reject(value: Choice): Void {{ consume(value); }}\nfn main(): Void {{ }}"
    );
    let (_sources, sink) = parse_and_check(&source);
    let diagnostic = sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::TYPE_MISMATCH)
        .expect("the composite union argument is incompatible with Int32");
    let cause = diagnostic.cause.as_deref().expect("type mismatch cause");
    assert!(
        cause.contains("..."),
        "the union display is truncated: {cause}"
    );
    assert!(
        cause.len() <= 240,
        "composite type names must stay bounded in the diagnostic, got {} bytes",
        cause.len()
    );
    assert!(std::str::from_utf8(cause.as_bytes()).is_ok());
}

#[test]
fn structural_discriminants_and_property_presence_narrow_known_unions_conservatively() {
    let source = "type Enabled = { active: true; enabledOnly: String };\n\
         type Disabled = { active: false; disabledOnly: String };\n\
         type Choice = Enabled | Disabled;\n\
         fn by_value(value: Choice): String {\n\
             if value.active == true { return value.enabledOnly; } else { return value.disabledOnly; }\n\
         }\n\
         fn after_early_return(value: Choice): String {\n\
             if value.active == true { return value.enabledOnly; }\n\
             return value.disabledOnly;\n\
         }\n\
         fn after_never(value: Choice): String {\n\
             if value.active == true { fatalError(\"stop\"); }\n\
             return value.disabledOnly;\n\
         }\n\
         fn after_throw(value: Choice): String {\n\
             if value.active == true { throw DivisionByZeroError(\"stop\"); }\n\
             return value.disabledOnly;\n\
         }\n\
         fn short_circuit(value: Choice): String {\n\
             if value.active == true && value.enabledOnly == \"yes\" { return value.enabledOnly; }\n\
             if value.active == false || value.enabledOnly == \"no\" { return \"ok\"; }\n\
             return \"other\";\n\
         }\n\
         type HasName = { name: String }; type NoName = { id: Int32 };\n\
         type Named = HasName | NoName;\n\
         fn by_presence(value: Named): String {\n\
             if (\"name\" in value) { return value.name; }\n\
             return \"missing\";\n\
         }\n\
         type MaybeName = { name?: String };\n\
         fn optional_stays_uncertain(value: MaybeName): String {\n\
             if (\"name\" in value) { return value.name ?? \"none\"; }\n\
             return value.name ?? \"none\";\n\
         }\n\
         fn open_object(value: Object): Void {\n\
             if (\"name\" in value) { mut name = value.name; }\n\
         }\n\
         fn main(): Void { }";
    let (_, sink) = parse_and_check(source);
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn flow_facts_are_invalidated_by_reassignment_and_escaping_calls() {
    let reassigned = rejected(
        "type Enabled = { active: Boolean; enabledOnly: String }; type Disabled = { active: Boolean; disabledOnly: String }; type Choice = Enabled | Disabled; fn read(value: Choice): String { if value.active == true { value.active = false; return value.enabledOnly; } return \"off\"; } fn main(): Void { }",
    );
    assert!(
        reassigned.contains(codes::UNKNOWN_MEMBER.as_str()),
        "a field write must discard the prior discriminant refinement: {reassigned}"
    );

    let escaped = rejected(
        "type Enabled = { active: true; enabledOnly: String }; type Disabled = { active: false; disabledOnly: String }; type Choice = Enabled | Disabled; fn mutate(value: Choice): Void { } fn read(value: Choice): String { if value.active == true { mutate(value); return value.enabledOnly; } return \"off\"; } fn main(): Void { }",
    );
    assert!(
        escaped.contains(codes::UNKNOWN_MEMBER.as_str()),
        "passing a narrowed value to a call may invalidate its fact: {escaped}"
    );

    let unrelated = "type Enabled = { active: true; enabledOnly: String }; type Disabled = { active: false; disabledOnly: String }; type Choice = Enabled | Disabled; fn noop(): Void { } fn read(value: Choice): String { if value.active == true { noop(); return value.enabledOnly; } return value.disabledOnly; } fn main(): Void { }";
    let (_, sink) = parse_and_check(unrelated);
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn closures_do_not_capture_transient_union_narrowing() {
    let output = rejected(
        "type Enabled = { active: true; enabledOnly: String }; type Disabled = { active: false; disabledOnly: String }; type Choice = Enabled | Disabled; fn read(value: Choice): String { if value.active == true { let callback = (): String => value.enabledOnly; return callback(); } return value.disabledOnly; } fn main(): Void { }",
    );
    assert!(
        output.contains(codes::UNKNOWN_MEMBER.as_str()),
        "a closure may run after a mutable referent changes, so it must check the declared union type: {output}"
    );
}

#[test]
fn invoking_a_capturing_closure_invalidates_its_captured_flow_facts() {
    let output = rejected(
        "type Enabled = { active: Boolean; enabledOnly: String }; type Disabled = { active: Boolean; disabledOnly: String }; type Choice = Enabled | Disabled; fn read(mut value: Choice): String { if value.active == true { let flip = (): Void => { value.active = false; }; flip(); return value.enabledOnly; } return \"off\"; } fn main(): Void { }",
    );
    assert!(
        output.contains(codes::UNKNOWN_MEMBER.as_str()),
        "calling a closure that mutates a captured object invalidates its prior refinement: {output}"
    );
}

#[test]
fn union_flow_composes_with_result_resources_exceptions_cancellation_and_nullable_projection() {
    let source = format!(
        "{OPEN_ERROR}\n{FAKE_FILE}\n{BOOM_ERROR}\n\
         type Ready = {{ ready: true; label: String; file: FakeFile? }};\n\
         type Failed = {{ ready: false; message: String }};\n\
         type Status = Ready | Failed;\n\
         fn summarize(value: Status): String {{\n\
             if value.ready == true {{ return value.file?.name ?? value.label; }}\n\
             return value.message;\n\
         }}\n\
         fn load_label(): String {{\n\
             mut label: String = \"\";\n\
             match open(\"report.txt\") with file {{\n\
                 Ok(file) => {{ label = file.name; }}\n\
                 Error(error) => {{ label = error.message(); }}\n\
             }}\n\
             return label;\n\
         }}\n\
         fn main(): Void {{\n\
             mut output: String = load_label();\n\
         try {{ throw BoomError(); }}\n\
         catch BoomError(error) {{ output = error.message(); }}\n\
             finally {{ output = output + \"!\"; }}\n\
         inmut job: Job<Int32> = Timer.after(1s, (): Int32 => 42);\n\
         job.cancel();\n\
         }}"
    );
    let (_, sink) = parse_and_check(&source);
    let output = sink.render(RenderStyle::Human);
    assert!(
        sink.diagnostics().iter().all(|diagnostic| {
            diagnostic.severity != Severity::Error || diagnostic.code == codes::NOT_LOWERED
        }),
        "cross-feature semantic checks should pass; the anonymous union is currently only semantically checked, not code generated:\n{output}"
    );
}

#[test]
fn type_predicates_are_inferred_only_from_direct_safe_type_tests() {
    let accepted = "class Box { construct() { } }\n\
         fn isBox(value: Dynamic): Boolean { return value is Box; }\n\
         fn getBox(value: Dynamic): Box? { if isBox(value) { return value; } return null; }\n\
         fn main(): Void { }";
    let (_, sink) = parse_and_check(accepted);
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");

    let forged = rejected(
        "class Box { construct() { } } fn isBox(value: String): Boolean { return value is Box; } fn main(): Void { }",
    );
    assert!(forged.contains("can never succeed"), "{forged}");

    let external_boundary = rejected(
        "class Box { construct() { } } extern \"C\" fn nativePredicate(value: Int32): Boolean; fn checkExternal(value: Dynamic): Void { if nativePredicate(1) { mut box: Box = value; } } fn main(): Void { }",
    );
    assert!(
        external_boundary.contains("incompatible types"),
        "{external_boundary}"
    );
}

#[test]
fn match_bindings_receive_remaining_singletons_and_structural_candidates() {
    let source = "type One = 1; type Two = 2; type Choice = One | Two;\n\
         fn select(value: Choice): Int32 { match value { 1 => 10, remaining => { let two: Two = remaining; return 20; } } }\n\
         type HasLabel = { label: String }; type HasCode = { code: Int32 };\n\
         type RecordChoice = HasLabel | HasCode;\n\
         fn get_text(value: RecordChoice): String {\n\
             match value { { label } => label, { code } => \"numeric\" }\n\
         }\n\
         fn get_remaining(value: RecordChoice): String {\n\
             match value { { label } => label, other => { let code: HasCode = other; return \"numeric\"; } }\n\
         }\n\
         fn get_from_scrutinee(value: RecordChoice): String {\n\
             match value { { label } => value.label, { code } => \"numeric\" }\n\
         }\n\
         fn main(): Void { }";
    let (_, sink) = parse_and_check(source);
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn finite_match_exhaustiveness_covers_nullable_mixed_and_temporal_domains() {
    let accepted = "enum Shade { Red, Blue }\n\
         type One = 1; type Mixed = One | Shade;\n\
         type Holiday = Date(2026, 12, 25) | Date(2027, 1, 1);\n\
         fn boolean(value: Boolean?): Int32 { return match value { true => 1, false => 0, null => 2 }; }\n\
         fn mixed(value: Mixed): Int32 { return match value { 1 => 1, Shade.Red => 2, Shade.Blue => 3 }; }\n\
         fn holiday(value: Holiday): Int32 { return match value { Date(2026, 12, 25) => 1, Date(2027, 1, 1) => 2 }; }\n\
         fn main(): Void { }";
    let (_, sink) = parse_and_check(accepted);
    let output = sink.render(RenderStyle::Human);
    assert!(
        !output.contains(codes::NON_EXHAUSTIVE_MATCH.as_str()),
        "{output}"
    );

    let missing_null = rejected(
        "fn incomplete(value: Boolean?): Int32 { return match value { true => 1, false => 0 }; } fn main(): Void { }",
    );
    assert!(missing_null.contains("null"), "{missing_null}");

    let partial_payload = rejected(
        "enum Shape { Circle(radius: Int32), Point } fn incomplete(value: Shape): Int32 { return match value { Shape.Circle(1) => 1, Shape.Point => 0 }; } fn main(): Void { }",
    );
    assert!(
        partial_payload.contains("Shape.Circle"),
        "{partial_payload}"
    );
}

#[test]
fn duplicate_and_impossible_match_patterns_report_unreachable_arms() {
    let duplicate = rejected(
        "type Choice = 1 | 2; fn classify(value: Choice): Int32 { return match value { 1 => 1, 1 => 2, _ => 3 }; } fn main(): Void { }",
    );
    assert!(
        duplicate.contains(codes::UNREACHABLE_PATTERN.as_str()),
        "{duplicate}"
    );

    let impossible = rejected(
        "type One = 1; fn classify(value: One): Int32 { return match value { 2 => 2, _ => 1 }; } fn main(): Void { }",
    );
    assert!(
        impossible.contains(codes::UNREACHABLE_PATTERN.as_str()),
        "{impossible}"
    );
}

#[test]
fn loop_flow_reaches_a_bounded_stable_header() {
    let source = "class Box { construct() { } }\n\
         fn inspect(value: Box?): Box? { while value != null { return value; } return value; }\n\
         fn inspect_do(value: Box?): Box? { do { if value != null { return value; } } while true; return value; }\n\
         fn main(): Void { }";
    let (_, sink) = parse_and_check(source);
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(
        !output.contains(codes::TYPE_COMPUTATION_LIMIT.as_str()),
        "{output}"
    );
}

#[test]
fn selected_union_candidate_contextualizes_nested_generic_tuple_and_callable_values() {
    let valid = "contract Payload<T> { value: T; }\n\
         type Enabled = { active: true; payload: Payload<Int32>; pair: Tuple<Int32, String>; map: Fn(Int32) => Int32 };\n\
         type Disabled = { active: false; payload: Payload<String>; pair: Tuple<String, Int32>; map: Fn(String) => String };\n\
         type Choice = Enabled | Disabled;\n\
         fn choose(): Choice { return { active: true, payload: { value: 7 }, pair: (8, \"ok\"), map: (value): Int32 => value }; }\n\
         fn main(): Void { }";
    let (_, valid_diagnostics) = parse_and_check(valid);
    let valid_output = valid_diagnostics.render(RenderStyle::Human);
    assert!(
        !valid_output.contains(codes::TYPE_MISMATCH.as_str()),
        "the selected candidate must contextualize its nested generic contract, tuple, and callable: {valid_output}"
    );

    let invalid = "contract Payload<T> { value: T; }\n\
         type Enabled = { active: true; payload: Payload<Int32>; pair: Tuple<Int32, String>; map: Fn(Int32) => Int32 };\n\
         type Disabled = { active: false; payload: Payload<String>; pair: Tuple<String, Int32>; map: Fn(String) => String };\n\
         type Choice = Enabled | Disabled;\n\
         fn choose(): Choice { return { active: true, payload: { value: \"wrong\" }, pair: (\"wrong\", 8), map: (value): String => value }; }\n\
         fn main(): Void { }";
    let (_, invalid_diagnostics) = parse_and_check(invalid);
    let invalid_output = invalid_diagnostics.render(RenderStyle::Human);
    assert!(
        invalid_output.contains(codes::TYPE_MISMATCH.as_str()),
        "the selected alternative must reject values incompatible with its nested generic, tuple, and callable types: {invalid_output}"
    );
}

#[test]
fn union_object_literal_spreads_use_only_their_declared_discriminant_type() {
    let precise = "type Enabled = { active: true; value: Int32 };\n\
         type Disabled = { active: false; value: Int32 };\n\
         type Choice = Enabled | Disabled;\n\
         fn copy_enabled(source: Enabled): Choice { return { ...source }; }\n\
         fn main(): Void { }";
    let (_, precise_diagnostics) = parse_and_check(precise);
    let precise_output = precise_diagnostics.render(RenderStyle::Human);
    assert!(
        !precise_output.contains("matches no alternative")
            && !precise_output.contains("ambiguous between union alternatives")
            && !precise_output.contains(codes::TYPE_MISMATCH.as_str()),
        "a spread with a statically singleton discriminant should select its compatible alternative: {precise_output}"
    );

    let broad = "type Enabled = { active: true; value: Int32 };\n\
         type Disabled = { active: false; value: Int32 };\n\
         type Choice = Enabled | Disabled;\n\
         type Broad = { active: Boolean; value: Int32 };\n\
         fn copy_broad(source: Broad): Choice { return { ...source }; }\n\
         fn main(): Void { }";
    let (_, broad_diagnostics) = parse_and_check(broad);
    let broad_output = broad_diagnostics.render(RenderStyle::Human);
    assert!(
        broad_output.contains("matches no alternative"),
        "a spread declared with a broad mutable-compatible discriminant must not be treated as either singleton: {broad_output}"
    );

    let no_flow_fact = "type Enabled = { active: true; value: Int32 };\n\
         type Disabled = { active: false; value: Int32 };\n\
         type Choice = Enabled | Disabled;\n\
         fn consume_disabled(value: Disabled): Void { }\n\
         fn build(source: Choice): Choice {\n\
             mut selected: Choice = { ...source, active: true, value: 1 };\n\
             consume_disabled(source);\n\
             return selected;\n\
         }\n\
         fn main(): Void { }";
    let (_, no_flow_diagnostics) = parse_and_check(no_flow_fact);
    let no_flow_output = no_flow_diagnostics.render(RenderStyle::Human);
    assert!(
        no_flow_output.contains(codes::TYPE_MISMATCH.as_str()),
        "selecting an alternative for a new object literal must not narrow the source union binding: {no_flow_output}"
    );
}

#[test]
fn if_branches_narrow_booleans_null_and_finite_singleton_unions() {
    let source = "type Choice = 1 | 2;\n\
         fn inspect(flag: Boolean, maybe: String?, choice: Choice): Void {\n\
             if flag { mut yes: true = flag; } else { mut no: false = flag; }\n\
             if maybe != null { mut text: String = maybe; } else { }\n\
             if choice == 1 { mut one: 1 = choice; } else { mut two: 2 = choice; }\n\
         }\n\
         fn main(): Void { }";
    let (_, diagnostics) = parse_and_check(source);
    let output = diagnostics.render(RenderStyle::Human);
    assert!(
        !output.contains(codes::TYPE_MISMATCH.as_str()),
        "true/false, null, and singleton equality branches should each expose only the reachable type: {output}"
    );
}

#[test]
fn nominal_type_tests_and_checked_casts_narrow_branch_values() {
    let source = "class Animal { }\n\
         class Dog extends Animal { }\n\
         class Cat extends Animal { }\n\
         fn inspect(value: Dog | Cat): Void {\n\
             if value is Dog { mut dog: Dog = value; } else { mut cat: Cat = value; }\n\
         }\n\
         fn inspect_dynamic(value: Dynamic): Void {\n\
             if value is Dog { mut dog: Dog = value; }\n\
         }\n\
         fn inspect_cast(value: Animal): Void {\n\
             if (value as? Dog) != null { mut dog: Dog = value; }\n\
         }\n\
         fn main(): Void { }";
    let (_, diagnostics) = parse_and_check(source);
    let output = diagnostics.render(RenderStyle::Human);
    assert!(
        !output.contains(codes::TYPE_MISMATCH.as_str()),
        "nominal, Dynamic, and checked-cast success branches should use their narrowed types: {output}"
    );

    let impossible = "class Animal { } class Rock { } fn inspect(value: Animal): Void { if value is Rock { } } fn main(): Void { }";
    let (_, impossible_diagnostics) = parse_and_check(impossible);
    let impossible_output = impossible_diagnostics.render(RenderStyle::Human);
    assert!(
        impossible_output.contains("can never succeed"),
        "a disjoint nominal type test should report that its true branch is impossible: {impossible_output}"
    );
}

#[test]
fn nested_generic_class_fields_are_substituted_for_structural_assignability() {
    let source = "type HasStrings = { inmut values: List<String> };\n\
         type HasDetails = { inmut details: { inmut value: String } };\n\
         class Box<T> { values: List<T>; details: { inmut value: T }; }\n\
         fn take(value: HasStrings): Void { }\n\
         fn take_details(value: HasDetails): Void { }\n\
         fn forward(value: Box<String>): Void { take(value); take_details(value); }\
         fn main(): Void { }";
    let (_, diagnostics) = parse_and_check(source);
    let output = diagnostics.render(RenderStyle::Human);
    assert!(
        !output.contains(codes::TYPE_MISMATCH.as_str()),
        "Box<String>.values must substitute to List<String>: {output}"
    );

    let incompatible = "type HasStrings = { inmut values: List<String> };\n\
         type HasDetails = { inmut details: { inmut value: String } };\n\
         class Box<T> { values: List<T>; details: { inmut value: T }; }\n\
         fn take(value: HasStrings): Void { }\
         fn take_details(value: HasDetails): Void { }\
         fn forward(value: Box<Int32>): Void { take(value); take_details(value); }\
         fn main(): Void { }";
    let (_, incompatible_diagnostics) = parse_and_check(incompatible);
    let incompatible_output = incompatible_diagnostics.render(RenderStyle::Human);
    assert!(
        incompatible_output.contains(codes::TYPE_MISMATCH.as_str()),
        "Box<Int32>.values must not satisfy List<String>: {incompatible_output}"
    );

    let union = "type HasTextOrNumber = { inmut value: String | Int32 };\n\
         class UnionBox<T> { value: T | String; }\n\
         fn take(value: HasTextOrNumber): Void { }\n\
         fn forward(value: UnionBox<Int32>): Void { take(value); }\
         fn main(): Void { }";
    let (_, union_diagnostics) = parse_and_check(union);
    let union_output = union_diagnostics.render(RenderStyle::Human);
    assert!(
        !union_output.contains(codes::TYPE_MISMATCH.as_str()),
        "generic substitutions inside unions should preserve compatible alternatives: {union_output}"
    );
}

#[test]
fn invalid_a_class_missing_iterator_still_reports_the_missing_method() {
    // Conformance is real: leaving out `iterator()` is caught the same way
    // missing any other contract method is.
    let output = rejected(
        "class Counter implements Iterable<Int32> {
             construct() { }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_a_class_implementing_iterable_with_the_wrong_element_type() {
    let output = rejected(
        "class WrongIterator implements Iterator<String> {
             construct() { }
              next(): Iteration<String> { return Iteration.Done; }
         }
         class Counter implements Iterable<Int32> {
             construct() { }
              iterator(): Iterator<Int32> { return WrongIterator(); }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_for_in_over_a_users_iterable_lowers() {
    accepted(
        "class Counter implements Iterable<Int32> {
             construct() { }
              iterator(): Iterator<Int32> { return CounterIterator(); }
         }
         class CounterIterator implements Iterator<Int32> {
             construct() { }
              next(): Iteration<Int32> { return Iteration.Done; }
         }
         fn main(): Void {
             mut c = Counter();
             for x in c { }
         }",
    );
}

// `Iterator<T>` conformance is what makes `next()` callable directly on the
// implementing class (`has_next` is an ordinary method of the class, not a
// member of the contract — `Iteration<T>`'s `Item`/`Done` is the language's
// has-next signal).
#[test]
fn valid_iterator_next_called_directly_on_the_implementing_class() {
    accepted(
        "class CounterIterator implements Iterator<Int32> {
             limit: Int32;
             current: Int32;
             construct(limit: Int32) { this.limit = limit; this.current = 0; }
              next(): Iteration<Int32> {
                 if (this.current >= this.limit) { return Iteration.Done; }
                 mut value = this.current;
                 this.current = this.current + 1;
                 return Iteration.Item(value: value);
             }
              has_next(): Boolean { return this.current < this.limit; }
         }
         fn main(): Void {
             mut it = CounterIterator(2);
             mut more = it.has_next();
             match it.next() {
                 Iteration.Item(v) => { mut x: Int32 = v; },
                 Iteration.Done => { },
             }
         }",
    );
}

#[test]
fn valid_iterator_next_called_through_the_contract_type() {
    // Roadmap Phase 7, task 12.3: generic contract satisfaction lowers —
    // `next()` on an `Iterator<Int32>`-typed value dispatches through the
    // contract table like any other contract call.
    accepted(
        "class CounterIterator implements Iterator<Int32> {
             construct() { }
              next(): Iteration<Int32> { return Iteration.Done; }
         }
         class Counter implements Iterable<Int32> {
             construct() { }
              iterator(): Iterator<Int32> { return CounterIterator(); }
         }
         fn main(): Void {
             mut c = Counter();
             mut it = c.iterator();
             mut step = it.next();
         }",
    );
}

#[test]
fn valid_for_in_over_the_builtin_collections() {
    // `Array<T>`, `List<T>` and `String` iterate without a user-written
    // `iterator()`: the collection loops are lowered directly.
    accepted_body(
        "mut a: Array<Int32> = Array(2);\n\
         a[0] = 1;\n\
         a[1] = 2;\n\
         mut l: List<Int32> = List();\n\
         l.add(3);\n\
         mut total = 0;\n\
         for x in a { total = total + x; }\n\
         for y in l { total = total + y; }\n\
         for c in \"hi\" { mut g: Char = c; }",
    );
}

// --- Clases ------------------------------------------------------------------

/// A class plus a `main`, which every program needs.
fn with_class(class: &str, body: &str) -> String {
    format!("{class}\nfn main(): Void {{\n{body}\n}}")
}

const USER: &str = "class User {
    inmut id: Int32;
    name: String;

    construct(id: Int32, name: String) {
        this.id = id;
        this.name = name;
    }

     greeting(): String { return this.name; }
}";

#[test]
fn valid_class_with_fields_constructor_and_method() {
    accepted(&with_class(USER, ""));
}

#[test]
fn valid_this_reads_and_writes_its_own_fields() {
    accepted(&with_class(
        "class Counter {
             count: Int32;
             construct() { this.count = 0; }
              bump(): Int32 { this.count = this.count + 1; return this.count; }
         }",
        "",
    ));
}

#[test]
fn invalid_this_outside_a_class() {
    let output = rejected_body("stdout.println(this.name);");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()), "{output}");
    assert!(output.contains("inside a class"), "{output}");
}

#[test]
fn invalid_member_that_does_not_exist() {
    let output = rejected(&with_class(
        "class User {
             name: String;
             construct(name: String) { this.name = name; }
              broken(): String { return this.nombre; }
         }",
        "",
    ));
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    // The diagnostic lists what does exist, which is what turns a typo into a
    // fix rather than a search.
    assert!(output.contains("its fields are: name"), "{output}");
}

#[test]
fn invalid_private_member_from_outside() {
    let output = rejected(&with_class(
        "class User {
             private name: String;
             construct(name: String) { this.name = name; }
         }",
        "mut u = User(\"x\");\nstdout.println(u.name);",
    ));
    // A hidden member is a different mistake from one that does not exist.
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
    assert!(output.contains("private"), "{output}");
}

#[test]
fn valid_private_member_from_inside_its_class() {
    accepted(&with_class(
        "class User {
             private name: String;
             construct(name: String) { this.name = name; }
              greeting(): String { return this.name; }
         }",
        "",
    ));
}

#[test]
fn valid_omitted_attributes_take_their_type_default() {
    // `LANGUAGE_SPEC` section 7: an omitted attribute receives its type
    // default before any initializer or the constructor runs, so a constructor
    // only has to write what has none.
    accepted(&with_class(
        "class Config {
             name: String;
             count: Int32;
             on: Boolean;
             construct() { }
         }",
        "",
    ));
}

#[test]
fn invalid_constructor_leaves_a_field_that_has_no_default() {
    // A class is a reference with identity: there is no instance to default to.
    let output = rejected(
        "class Inner { construct() { } }
         class Outer { inner: Inner; construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::UNINITIALIZED_FIELD.as_str()),
        "{output}"
    );
    assert!(output.contains("inner"), "{output}");
}

#[test]
fn valid_field_assigned_in_both_branches_counts_as_initialized() {
    // A syntactic walk, not flow analysis: setting a field in both arms of an
    // `if` is ordinary code, not a mistake.
    accepted(&with_class(
        "class User {
             name: String;
             construct(anonymous: Boolean) {
                 if anonymous { this.name = \"?\"; } else { this.name = \"x\"; }
             }
         }",
        "",
    ));
}

#[test]
fn valid_inmut_field_is_set_by_the_constructor() {
    accepted(&with_class(
        "class User {
             inmut id: Int32;
             construct(id: Int32) { this.id = id; }
         }",
        "",
    ));
}

#[test]
fn invalid_writing_an_inmut_field_from_outside_the_constructor() {
    let output = rejected(&with_class(
        "class User {
             inmut id: Int32;
             construct(id: Int32) { this.id = id; }
              reset(): Void { this.id = 0; }
         }",
        "",
    ));
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_construction_with_the_wrong_number_of_arguments() {
    let output = rejected(&with_class(USER, "mut u = User(1);"));
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_construction_with_the_wrong_argument_type() {
    let output = rejected(&with_class(USER, "mut u = User(\"uno\", \"x\");"));
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_several_constructors_resolve_by_arity() {
    accepted(&with_class(
        "class Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
             construct() { this.x = 0; }
         }",
        "mut a = Point(1);\nmut b = Point();",
    ));
}

#[test]
fn valid_constructor_overloads_resolve_by_label_and_type() {
    accepted(
        "class Point {
             text: String;
             construct(x: Int32) { this.text = \"int\"; }
             construct(s: String) { this.text = s; }
         }
         fn main(): Void {
             inmut integer = Point(x: 1);
             inmut string = Point(s: \"text\");
         }",
    );
}

#[test]
fn invalid_duplicate_constructor_positional_signature_is_rejected_at_declaration() {
    let output = rejected(
        "class Point {
             text: String;
             construct(x: Int32) { this.text = \"x\"; }
             construct(y: Int32) { this.text = \"y\"; }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
    assert!(
        output.contains("duplicates a positional signature `construct(Int32)`"),
        "{output}"
    );
}

#[test]
fn invalid_constructor_overload_is_ambiguous_only_when_distinct_types_both_fit() {
    let output = rejected(
        "class Animal { construct() { } }
         class Dog extends Animal { construct() { super(); } }
         class Point {
             text: String;
             construct(animal: Animal) { this.text = \"animal\"; }
             construct(dog: Dog) { this.text = \"dog\"; }
         }
         fn main(): Void { inmut point = Point(Dog()); }",
    );
    assert!(output.contains("ambiguous"), "{output}");
}

#[test]
fn invalid_constructor_overload_with_no_matching_label_is_deterministic() {
    let output = rejected(
        "class Point {
             text: String;
             construct(x: Int32) { this.text = \"x\"; }
             construct(y: Int32) { this.text = \"y\"; }
         }
         fn main(): Void { inmut point = Point(z: 1); }",
    );
    assert!(
        output.contains("no constructor of `Point` matches"),
        "{output}"
    );
}

#[test]
fn invalid_positional_argument_after_named_argument_is_targeted() {
    let output = rejected(
        "fn join(prefix: String, suffix: String): String { return prefix + suffix; }
         fn main(): Void { inmut text = join(prefix: \"PRE\", \"SUF\"); }",
    );
    assert!(
        output.contains("positional argument after a named argument"),
        "{output}"
    );
}

#[test]
fn invalid_named_argument_shorthand_reports_missing_bindings_and_repeated_labels() {
    let missing = rejected(
        "fn join(prefix: String, suffix: String): String { return prefix + suffix; }
         fn main(): Void { inmut text = join(prefix:, suffix: \"SUF\"); }",
    );
    assert!(
        missing.contains(codes::UNDECLARED_NAME.as_str()),
        "{missing}"
    );

    let repeated = rejected(
        "fn join(prefix: String, suffix: String): String { return prefix + suffix; }
         fn main(): Void { inmut text = join(prefix: \"A\", prefix: \"B\"); }",
    );
    assert!(repeated.contains("already given a value"), "{repeated}");
}

#[test]
fn invalid_duplicate_class() {
    let output = rejected(
        "class User { name: String; construct(name: String) { this.name = name; } }
         class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_duplicate_field() {
    let output = rejected(&with_class(
        "class User { name: String; name: Int32; construct() { this.name = \"x\"; } }",
        "",
    ));
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_member_of_a_type_without_members() {
    let output = rejected_body("mut n = 1;\nstdout.println(n.field);");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn invalid_class_type_is_not_interchangeable_with_another() {
    // Nominal: two classes with the same members are still different types.
    let output = rejected(
        "class A { x: Int32; construct() { this.x = 0; } }
         class B { x: Int32; construct() { this.x = 0; } }
         fn main(): Void { mut a: A = B(); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_a_class_names_its_own_type_in_a_field() {
    // The class is registered before its fields resolve, so it can name itself.
    accepted(&with_class(
        "class Node { next: Node?; construct() { this.next = null; } }",
        "",
    ));
}

// --- Métodos -----------------------------------------------------------------

const COUNTER: &str = "class Counter {
    count: Int32;
    private step: Int32;

    construct(step: Int32) { this.count = 0; this.step = step; }

     bump(): Int32 { this.count = this.count + this.step; return this.count; }
    private  secret(): Int32 { return this.step; }
}";

#[test]
fn valid_method_call() {
    accepted(&with_class(
        COUNTER,
        "mut c = Counter(1);\nmut n = c.bump();",
    ));
}

#[test]
fn valid_method_calls_another_through_this() {
    accepted(&with_class(
        "class Counter {
             count: Int32;
             construct() { this.count = 0; }
              bump(): Int32 { this.count = this.count + 1; return this.count; }
              twice(): Int32 { this.bump(); return this.bump(); }
         }",
        "",
    ));
}

#[test]
fn invalid_method_that_does_not_exist() {
    let output = rejected(&with_class(
        COUNTER,
        "mut c = Counter(1);\nmut n = c.jump();",
    ));
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(output.contains("its methods are"), "{output}");
}

#[test]
fn invalid_calling_a_field() {
    // Naming a field where a method is called is its own mistake: saying "not
    // callable" would send the reader looking for a typo.
    let output = rejected(&with_class(
        COUNTER,
        "mut c = Counter(1);\nmut n = c.count();",
    ));
    assert!(output.contains(codes::NOT_CALLABLE.as_str()), "{output}");
    assert!(output.contains("is a field, not a method"), "{output}");
}

#[test]
fn invalid_private_method_from_outside() {
    let output = rejected(&with_class(
        COUNTER,
        "mut c = Counter(1);\nmut n = c.secret();",
    ));
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
}

#[test]
fn valid_private_method_from_inside_its_class() {
    accepted(&with_class(
        "class Counter {
             count: Int32;
             construct() { this.count = 0; }
             private  secret(): Int32 { return this.count; }
              public_view(): Int32 { return this.secret(); }
         }",
        "",
    ));
}

#[test]
fn invalid_method_call_with_the_wrong_argument_type() {
    let output = rejected(&with_class(
        "class Greeter {
             prefix: String;
             construct() { this.prefix = \">\"; }
             greet(name: String): String { return name; }
         }",
        "mut g = Greeter();\nmut s = g.greet(1);",
    ));
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_method_and_field_sharing_a_name() {
    // `u.name` would have to mean both.
    let output = rejected(&with_class(
        "class User {
             name: String;
             construct() { this.name = \"x\"; }
             name(): String { return this.name; }
         }",
        "",
    ));
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_method_without_a_return_on_every_path() {
    let output = rejected(&with_class(
        "class Broken {
             flag: Boolean;
             construct() { this.flag = true; }
              value(): Int32 { if this.flag { return 1; } }
         }",
        "",
    ));
    assert!(output.contains(codes::MISSING_RETURN.as_str()), "{output}");
}

// --- Herencia ----------------------------------------------------------------

const HIERARCHY: &str = "class User {
    name: String;
    protected role: String;

    construct(name: String) { this.name = name; this.role = \"user\"; }

    describe(): String { return this.name; }
}

class Manager extends User {
    team: Int32;

    construct(name: String, team: Int32) {
        super(name);
        this.role = \"manager\";
        this.team = team;
    }

    #override describe(): String { return this.role; }
}";

#[test]
fn valid_subclass_inherits_fields_and_methods() {
    accepted(&with_class(
        HIERARCHY,
        "mut m = Manager(\"x\", 1);\nstdout.println(m.name);\nstdout.println(m.describe());",
    ));
}

#[test]
fn valid_subclass_stands_where_its_base_is_expected() {
    accepted(&format!(
        "{HIERARCHY}\nfn greet(u: User): String {{ return u.describe(); }}\n\
         fn main(): Void {{ mut m = Manager(\"x\", 1); stdout.println(greet(m)); }}"
    ));
}

#[test]
fn invalid_base_where_a_subclass_is_expected() {
    // One direction only: accepting the reverse would promise members the
    // value may not have.
    let output = rejected(&format!(
        "{HIERARCHY}\nfn lead(m: Manager): String {{ return m.describe(); }}\n\
         fn main(): Void {{ mut u = User(\"x\"); stdout.println(lead(u)); }}"
    ));
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_protected_reaches_the_subclass() {
    accepted(&with_class(HIERARCHY, ""));
}

#[test]
fn invalid_private_does_not_reach_the_subclass() {
    let output = rejected(
        "class Base {
             private hidden: Int32;
             construct() { this.hidden = 0; }
         }
         class Derived extends Base {
             construct() { this.hidden = 1; }
              peek(): Int32 { return this.hidden; }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
    assert!(output.contains("only inside its own class"), "{output}");
}

#[test]
fn invalid_protected_from_outside_the_hierarchy() {
    let output = rejected(&with_class(
        HIERARCHY,
        "mut u = User(\"x\");\nstdout.println(u.role);",
    ));
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_override_with_a_different_signature() {
    // A call through the base would reach a body expecting something else,
    // which is the one thing the base's type promises it will not.
    let output = rejected(
        "class Base {
             x: Int32;
             construct() { this.x = 0; }
              value(): Int32 { return this.x; }
         }
         class Derived extends Base {
             construct() { super(); }
             #override

               value(): String { return \"x\"; }
         }
          main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("overrides"), "{output}");
}

#[test]
fn invalid_inheritance_cycle() {
    let output = rejected(
        "class A extends B { construct() { } }
         class B extends A { construct() { } }
         fn main(): Void { }",
    );
    assert!(output.contains("inherits from itself"), "{output}");
}

#[test]
fn invalid_extending_something_that_is_not_a_class() {
    let output = rejected("class A extends Int32 { construct() { } }\nfn main(): Void { }");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn invalid_redeclaring_an_inherited_field() {
    let output = rejected(
        "class Base { x: Int32; construct() { this.x = 0; } }
         class Derived extends Base { x: Int32; construct() { this.x = 0; } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
    assert!(output.contains("inherited field"), "{output}");
}

#[test]
fn invalid_subclass_constructor_leaving_an_inherited_field_unset() {
    // Without `super(...)` the base's constructor never runs, so a field with
    // no default would stay unset.
    let output = rejected(
        "class Inner { construct() { } }
         class Base { inner: Inner; construct() { this.inner = Inner(); } }
         class Derived extends Base { construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::UNINITIALIZED_FIELD.as_str()),
        "{output}"
    );
}

#[test]
fn valid_super_covers_what_the_base_declares() {
    accepted(
        "class Inner { construct() { } }
         class Base { inner: Inner; construct() { this.inner = Inner(); } }
         class Derived extends Base { construct() { super(); } }
         fn main(): Void { }",
    );
}

#[test]
fn valid_super_initializes_a_private_field_of_the_base() {
    // The one thing a subclass could not do before: reach what only the base's
    // own constructor may write.
    accepted(
        "class Base {
             private secret: Inner;
             construct() { this.secret = Inner(); }
         }
         class Inner { construct() { } }
         class Derived extends Base { construct() { super(); } }
         fn main(): Void { }",
    );
}

#[test]
fn valid_super_method_reaches_the_inherited_body() {
    accepted(
        "class Base {  describe(): String { return \"base\"; } construct() { } }
         class Derived extends Base {
             construct() { }
             #override

               describe(): String { return super.describe(); }
         }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_super_outside_a_class() {
    let output = rejected_body("super();");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()), "{output}");
}

#[test]
fn invalid_super_without_a_base() {
    let output = rejected("class Alone { construct() { super(); } }\nfn main(): Void { }");
    assert!(output.contains("no base class"), "{output}");
}

#[test]
fn invalid_override_without_the_keyword() {
    // Otherwise adding a method to a base silently changes what a subclass
    // means.
    let output = rejected(
        "class Base {  value(): Int32 { return 1; } construct() { } }
         class Derived extends Base {  value(): Int32 { return 2; } construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_OVERRIDE.as_str()),
        "{output}"
    );
    assert!(output.contains("#override` above `value"), "{output}");
}

#[test]
fn a_class_to_string_replaces_the_standard_trait_default() {
    accepted(
        "import { Stringifiable } from std.io;
         class Label {
             text: String;
             construct() { this.text = \"label\"; }
             #override
             to_string(): String { return this.text; }
         }
         fn render(value: Stringifiable): String { return value.to_string(); }
         fn main(): Void { stdout.println(render(Label())); }",
    );
}

#[test]
fn a_class_to_string_requires_override_for_the_standard_trait_default() {
    let output = rejected(
        "class Label {
             construct() { }
             to_string(): String { return \"label\"; }
         }
         fn main(): Void { stdout.println(Label()); }",
    );
    assert!(
        output.contains(codes::MISSING_OVERRIDE.as_str()),
        "{output}"
    );
    assert!(output.contains("trait's default method"), "{output}");
}

#[test]
fn invalid_override_that_overrides_nothing() {
    // The mirror mistake, and usually a typo in the name.
    let output = rejected(
        "class Base {  value(): Int32 { return 1; } construct() { } }
         class Derived extends Base { #override

  valeu(): Int32 { return 2; } construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_OVERRIDE.as_str()),
        "{output}"
    );
    assert!(output.contains("overrides nothing"), "{output}");
}

#[test]
fn valid_a_class_may_extend_one_declared_later() {
    accepted(
        "class Derived extends Base { construct() { this.x = 1; } }
         class Base { x: Int32; construct() { this.x = 0; } }
         fn main(): Void { }",
    );
}

// --- Contratos ---------------------------------------------------------------

const CONTRACTS: &str = "contract Describable {
     describe(): String;
}

trait Labelled {
     label(): String;
     shout(): String { return this.label(); }
}

class User implements Describable, Labelled {
    name: String;
    construct(name: String) { this.name = name; }
     describe(): String { return this.name; }
     label(): String { return \"user\"; }
}";

#[test]
fn valid_class_implements_an_interface_and_a_trait() {
    accepted(&with_class(CONTRACTS, ""));
}

#[test]
fn valid_a_class_stands_where_its_contract_is_expected() {
    accepted(&format!(
        "{CONTRACTS}\nfn announce(d: Describable): String {{ return d.describe(); }}\n\
         fn main(): Void {{ mut u = User(\"x\"); stdout.println(announce(u)); }}"
    ));
}

#[test]
fn valid_a_trait_default_body_is_adopted() {
    // The class never writes `shout`, and still has it.
    accepted(&format!(
        "{CONTRACTS}\nfn main(): Void {{ mut u = User(\"x\"); stdout.println(u.shout()); }}"
    ));
}

#[test]
fn invalid_two_traits_offer_conflicting_defaults() {
    // Neither trait's default should silently win by declaration order (D4).
    let output = rejected(
        "trait Loud {  greet(): String { return \"HELLO\"; } }
         trait Quiet {  greet(): String { return \"hello\"; } }
         class Both implements Loud, Quiet { construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
    assert!(output.contains("greet"), "{output}");
}

#[test]
fn valid_class_own_method_resolves_two_traits_offering_the_same_name() {
    // Writing the method itself is how the class picks, so it is not a
    // conflict: both traits require the same signature and get it.
    accepted(
        "trait Loud {  greet(): String { return \"HELLO\"; } }
         trait Quiet {  greet(): String { return \"hello\"; } }
         class Both implements Loud, Quiet {
             construct() { }
             #override
              greet(): String { return \"hi\"; }
         }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_class_missing_what_a_contract_requires() {
    let output = rejected(
        "contract Describable {  describe(): String; }
         class Robot implements Describable { construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
    assert!(output.contains("describe"), "{output}");
}

#[test]
fn invalid_implementation_with_a_different_signature() {
    let output = rejected(
        "contract Describable {  describe(): String; }
         class Robot implements Describable {
             construct() { }
              describe(): Int32 { return 1; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_implementation_that_is_not_public() {
    // A contract declares behaviour anyone may reach.
    let output = rejected(
        "contract Describable {  describe(): String; }
         class Robot implements Describable {
             construct() { }
             private  describe(): String { return \"x\"; }
         }
          main(): Void { }",
    );
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_reaching_a_member_the_contract_does_not_declare() {
    // Which class is behind it is what a contract exists not to say.
    let output = rejected(&format!(
        "{CONTRACTS}\nfn announce(d: Describable): String {{ return d.label(); }}\n\
         fn main(): Void {{ }}"
    ));
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn invalid_constructing_a_contract() {
    let output = rejected(
        "contract Describable {  describe(): String; }
         fn main(): Void { mut d = Describable(); }",
    );
    assert!(output.contains(codes::NOT_CALLABLE.as_str()), "{output}");
    assert!(output.contains("describes behaviour"), "{output}");
}

#[test]
fn invalid_implementing_something_that_is_not_a_contract() {
    let output = rejected(
        "class Base { construct() { } }
         class Other implements Base { construct() { } }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn valid_a_subclass_satisfies_what_its_base_satisfies() {
    accepted(&format!(
        "{CONTRACTS}
         class Admin extends User {{ construct(name: String) {{ super(name); }} }}
         fn announce(d: Describable): String {{ return d.describe(); }}
         fn main(): Void {{ mut a = Admin(\"x\"); stdout.println(announce(a)); }}"
    ));
}

#[test]
fn invalid_interface_method_with_a_body() {
    // A trait is the one that may carry implementation.
    let output = rejected(
        "contract Describable {  describe(): String { return \"x\"; } }\nfn main(): Void { }",
    );
    assert!(output.contains("has no body"), "{output}");
}

// --- Contratos de operador ---------------------------------------------------

#[test]
fn valid_string_concatenation() {
    // La deuda de la Fase 2, cerrada.
    accepted_body("mut greeting: String = \"hello\" + \" \" + \"world\";");
}

#[test]
fn valid_string_repetition_in_either_order() {
    accepted_body("mut a: String = \"ja\" * 3;\nmut b: String = 3 * \"ja\";");
}

#[test]
fn invalid_string_minus_string() {
    // Native types expose no undocumented operator.
    let output = rejected_body("mut x = \"a\" - \"b\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_string_plus_a_number_without_a_conversion() {
    // There is no implicit conversion. Convert the number itself with
    // `String(4)` or use interpolation; wrapping the invalid addition does
    // not change how its operands are checked.
    let output = rejected_body("mut x = \"count=\" + 4;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_user_type_supplies_an_operator() {
    accepted(
        "class Money {
             amount: Int32;
             construct(amount: Int32) { this.amount = amount; }
              _add(other: Money): Money { return Money(this.amount + other.amount); }
         }
         fn main(): Void { mut total = Money(30) + Money(12); }",
    );
}

#[test]
fn invalid_operator_a_user_type_does_not_supply() {
    // The diagnostic names the method that would make it work.
    let output = rejected(
        "class Money {
             amount: Int32;
             construct(amount: Int32) { this.amount = amount; }
         }
         fn main(): Void { mut total = Money(1) + Money(2); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("_add"), "{output}");
}

#[test]
fn invalid_operator_method_with_the_wrong_operand() {
    let output = rejected(
        "class Money {
             amount: Int32;
             construct(amount: Int32) { this.amount = amount; }
              _add(other: Int32): Money { return this; }
         }
         fn main(): Void { mut total = Money(1) + Money(2); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_reopening_a_native_type() {
    // Application code cannot replace what `String` means.
    for name in ["String", "Int32", "Boolean", "Float"] {
        let output = rejected(&format!(
            "class {name} {{ construct() {{ }} }}\nfn main(): Void {{ }}"
        ));
        assert!(
            output.contains("type of the language"),
            "for `{name}`:\n{output}"
        );
    }
}

// --- Identidad e igualdad ----------------------------------------------------

#[test]
fn invalid_equality_on_a_type_that_has_not_defined_it() {
    // Answering by address would be `is` wearing the wrong operator, and
    // answering `true` because the types match would be worse still.
    let output = rejected(
        "class U { name: String; construct() { this.name = \"a\"; } }
         fn main(): Void { mut a = U(); mut b = U(); stdout.println(a == b); }",
    );
    assert!(output.contains("does not define equality"), "{output}");
    assert!(output.contains("_equals"), "{output}");
}

#[test]
fn valid_equality_through_the_reserved_method() {
    accepted(
        "class Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
              _equals(other: Point): Boolean { return this.x == other.x; }
         }
         fn main(): Void { mut a = Point(1); stdout.println(a == Point(1)); }",
    );
}

#[test]
fn valid_identity_needs_no_contract() {
    // Identity *is* the address, so there is nothing to define.
    accepted(
        "class U { construct() { } }
         fn main(): Void { mut a = U(); mut b = a; stdout.println(a is b); }",
    );
}

#[test]
fn invalid_identity_on_a_value() {
    let output = rejected_body("mut x = 1 is 2;");
    assert!(output.contains("no identity to compare"), "{output}");
}

#[test]
fn valid_identity_between_two_nullable_receivers() {
    // Unlike `==` (`reject_nullable_comparison`), `Is`'s own arm of
    // `check_binary` has no such rejection: identity is the one place
    // comparing against a possibly-absent reference is allowed
    // (`ZIRK_LANGUAGE_SPEC.md` section 4).
    accepted(
        "class U { construct() { } }
         fn main(): Void { mut a: U? = U(); mut b: U? = a; stdout.println(a is b); }",
    );
}

#[test]
fn valid_identity_between_a_bare_and_a_nullable_receiver() {
    // `expect_same`'s own `unify` widens `U` to `U?` the same way an
    // ordinary assignment would — found via a lowering bug where the
    // checker accepted this and the IR did not agree with itself about
    // which of the two types the comparison ran at.
    accepted(
        "class U { construct() { } }
         fn main(): Void { mut a: U = U(); mut b: U? = a; stdout.println(a is b); }",
    );
}

#[test]
fn invalid_reaching_a_member_through_a_nullable_receiver() {
    // Calling through a value that may be absent is the same mistake as
    // reading through one.
    for body in [
        "mut u: U? = null;\nstdout.println(u.name);",
        "mut u: U? = null;\nstdout.println(u.describe());",
    ] {
        let output = rejected(&format!(
            "class U {{ name: String; construct() {{ this.name = \"a\"; }} \
              describe(): String {{ return this.name; }} }}\n\
             fn main(): Void {{ {body} }}"
        ));
        assert!(output.contains("may be absent"), "for `{body}`:\n{output}");
    }
}

#[test]
fn invalid_safe_navigation_on_a_receiver_that_is_never_null() {
    // Found in manual verification after the fase-4 nullability fixes:
    // `?.`'s whole purpose is handling absence, so — the same judgment
    // `check_coalesce` already makes for `??` on a non-nullable left side
    // — applying it to a receiver that is never absent is flagged instead
    // of silently accepted and left to reach the lowering, which assumes
    // every `?.` receiver is nullable.
    for body in [
        "mut u: U = U();\nstdout.println(u?.name);",
        "mut u: U = U();\nstdout.println(u?.describe());",
    ] {
        let output = rejected(&format!(
            "class U {{ name: String; construct() {{ this.name = \"a\"; }} \
              describe(): String {{ return this.name; }} }}\n\
             fn main(): Void {{ {body} }}"
        ));
        assert!(
            output.contains(codes::REDUNDANT_OPERATOR.as_str()),
            "for `{body}`:\n{output}"
        );
    }
}

#[test]
fn valid_safe_call_on_a_method_that_returns_void() {
    // `Void?` is rejected as a type (see the test below), so the checker
    // types `objeto?.algo()` as plain `Void` when `algo` returns `Void` —
    // there is nothing to widen to a nullable form of "no value" — rather
    // than panicking the lowering trying to build one
    // (`docs/decisions/ADR-003-investigacion-fase-4.md`, section "Extensión:
    // criterio 3...").
    accepted(
        "class C { construct() { }  bump(): Void { } }
         fn main(): Void { mut c: C? = C(); c?.bump(); }",
    );
}

#[test]
fn valid_a_nullable_contract_is_a_type() {
    // It reaches lowering as an ordinary nullable, which is what the uniform
    // representation buys.
    accepted(
        "contract D {  describe(): String; }
         class R implements D { construct() { }  describe(): String { return \"r\"; } }
         fn main(): Void { mut d: D? = R(); mut e: D? = null; }",
    );
}

// --- Genéricos ---------------------------------------------------------------

#[test]
fn valid_generic_class_body_type_checks_against_its_own_type_parameter() {
    // `T` resolves the same way everywhere the class names it, so assigning a
    // `T` to a `T` field and returning a `T` from a method that declares `T`
    // both type-check. A class this simple — every use of `T` direct, no
    // `extends`/`implements` — also fully lowers now (roadmap task 11.1), so
    // this is accepted outright rather than merely free of these two codes.
    accepted(
        "class Box<T> {
             value: T;
             construct(value: T) { this.value = value; }
              get(): T { return this.value; }
         }
         fn main(): Void { }",
    );
}

#[test]
fn unused_generic_function_template_is_accepted() {
    accepted("fn identity<T>(value: T): T { return value; }\nfn main(): Void { }");
}

#[test]
fn explicit_generic_function_arguments_seed_type_inference() {
    accepted(
        "fn identity<T>(value: T): T { return value; }
         fn main(): Void { mut value: Int32 = identity<Int32>(1); }",
    );

    let rejected_call = rejected(
        "fn identity<T>(value: T): T { return value; }
         fn main(): Void { mut value: String = identity<String>(1); }",
    );
    assert!(
        rejected_call.contains(codes::TYPE_MISMATCH.as_str()),
        "{rejected_call}"
    );
}

#[test]
fn generic_call_substitution_is_preserved_for_ir_specialization() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "generic-call.zrk",
        "fn identity<T>(value: T): T { return value; }\n\
         fn main(): Void { mut value: Int32 = identity<Int32>(1); }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let call = match &program.functions[1].body.statements[0] {
        Stmt::Let(statement) => match statement.init.as_ref().unwrap() {
            Expr::Call(call) => call,
            other => panic!("expected generic call, got {other:?}"),
        },
        other => panic!("expected let statement, got {other:?}"),
    };
    let checked = check(&sources, &program, &mut sink);
    let generic = checked.functions.get("identity").unwrap();
    assert_eq!(generic.type_params.len(), 1);
    assert_eq!(
        checked.generic_call_substitutions[&call.span].callable,
        "identity"
    );
    assert_eq!(
        checked.generic_call_substitutions[&call.span].substitutions[&generic.type_params[0]],
        Type::INT32
    );
}

#[test]
fn nested_generic_function_signature_accepts_complete_substitution() {
    accepted(
        "fn wrap<T>(value: List<T>): List<T> { return value; }
         fn exercise(value: List<Int32>): List<Int32> { return wrap(value); }
         fn main(): Void { }",
    );
}

#[test]
fn generic_contract_static_requirements_match_static_class_members() {
    accepted(
        "contract EnvironmentParsable<T> { static parse_environment(text: String): T; }
         class Port implements EnvironmentParsable<Port> {
             static parse_environment(text: String): Port { return Port(); }
         }
         fn main(): Void { }",
    );

    let output = rejected(
        "contract EnvironmentParsable<T> { static parse_environment(text: String): T; }
         class Port implements EnvironmentParsable<Port> {
             parse_environment(text: String): Port { return Port(); }
         }
         fn main(): Void { }",
    );
    assert!(output.contains("static/instance dispatch"), "{output}");
}

#[test]
fn generic_parameter_calls_static_contract_method_from_its_constraint() {
    accepted(
        "contract EnvironmentParsable<T> { static parse(text: String): T; }
         class Port implements EnvironmentParsable<Port> {
             static parse(text: String): Port { return Port(); }
         }
         fn parse_value<T from EnvironmentParsable<T>>(text: String): T {
             return T.parse(text);
         }
         fn main(): Void { mut port: Port = parse_value<Port>(\"loopback\"); }",
    );
}

#[test]
fn user_contract_named_environment_parsable_cannot_spoof_builtin_intrinsics() {
    let output = rejected(
        "contract EnvironmentParsable<T> { static parse(text: String): T; }
         fn parse_value<T from EnvironmentParsable<T>>(text: String): T {
             return T.parse(text);
         }
         fn main(): Void { mut value: Int32 = parse_value<Int32>(\"42\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("EnvironmentParsable"), "{output}");
}

#[test]
fn generic_method_substitution_is_preserved_for_ir_specialization() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "generic-method-call.zrk",
        "class Parser { static parse<T>(text: String): String { return text; } }\n\
         fn main(): Void { mut value: String = Parser.parse<Int32>(\"1\"); }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let call = match &program.functions[0].body.statements[0] {
        Stmt::Let(statement) => match statement.init.as_ref().unwrap() {
            Expr::Call(call) => call,
            other => panic!("expected generic method call, got {other:?}"),
        },
        other => panic!("expected let statement, got {other:?}"),
    };
    let checked = check(&sources, &program, &mut sink);
    assert!(
        checked.generic_call_substitutions[&call.span]
            .substitutions
            .values()
            .any(|ty| *ty == Type::INT32)
    );
    let resolved = &checked.generic_call_substitutions[&call.span];
    assert!(resolved.is_static_method);
    assert_eq!(
        resolved.owner_class,
        checked
            .classes
            .iter()
            .position(|class| class.name == "Parser")
            .map(|id| id as u32)
    );
}

#[test]
fn explicit_generic_method_arguments_seed_type_inference() {
    accepted(
        "class Parser { static parse<T>(text: String): String { return text; } }
         fn main(): Void { mut value: String = Parser.parse<Int32>(\"1\"); }",
    );
    let wrong_arity = rejected(
        "class Parser { static parse<T>(text: String): String { return text; } }
         fn main(): Void { mut value: String = Parser.parse<Int32, String>(\"1\"); }",
    );
    assert!(
        wrong_arity.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{wrong_arity}"
    );
}

#[test]
fn invalid_generic_class_with_extends_is_not_lowered_yet() {
    // Task 11.1's specialization pass does not build a specialized copy's
    // own dispatch table, so a generic class that `extends` stays gated.
    let output = rejected(
        "class Base { }
         class Box<T> extends Base { value: T; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn invalid_generic_class_with_nested_type_argument_is_not_lowered_yet() {
    // `T` nested inside another generic type (here, itself) rather than
    // named directly is the one shape task 11.1's substitution does not
    // recurse into (`Self::type_references_any_param`).
    let output = rejected(
        "class Box<T> {
             value: T;
             next: Box<T>?;
             construct(value: T) { this.value = value; this.next = null; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn invalid_returning_a_concrete_type_where_the_type_parameter_is_declared() {
    // `T` is opaque: nothing proves it is `Int32`, so a literal does not fit.
    let output = rejected(
        "class Box<T> {
              wrong(): T { return 5; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_type_parameter_is_out_of_scope_outside_its_declaration() {
    let output = rejected(
        "class Box<T> { value: T; }
         class Other { value: T; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn valid_declared_variance_in_permitted_positions() {
    // `out T` is allowed in an immutable field and a return type.
    accepted(
        "class Box<out T> { inmut value: T;  get(): T { return this.value; } }\nfn main(): Void { }",
    );
    // `in T` is allowed in parameter positions.
    accepted("class Sink<in T> {  take(value: T): Void { } }\nfn main(): Void { }");
}

#[test]
fn invalid_declared_variance_in_the_wrong_position() {
    let output = rejected("class Box<out T> {  take(value: T): Void { } }\nfn main(): Void { }");
    assert!(
        output.contains(codes::INVALID_VARIANCE.as_str()),
        "{output}"
    );
    assert!(
        output.contains("`out T` appears in an input position"),
        "{output}"
    );

    let output = rejected("class Source<in T> {  get(): T { } }\nfn main(): Void { }");
    assert!(
        output.contains(codes::INVALID_VARIANCE.as_str()),
        "{output}"
    );
    assert!(
        output.contains("`in T` appears in an output position"),
        "{output}"
    );
}

#[test]
fn invalid_declared_variance_in_a_mutable_field() {
    let output = rejected("class Cell<out T> { mut value: T; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::INVALID_VARIANCE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_method_call_through_a_type_parameters_contract_constraint() {
    // `value.greet()` resolves through `Greeter`, the one constraint `T`
    // declares — task 7.4 of the generics slice.
    accepted(
        "contract Greeter {  greet(): String; }
         fn show<T from Greeter>(value: T): String { return value.greet(); }
         fn main(): Void { }",
    );
}

#[test]
fn valid_field_access_through_a_type_parameters_class_constraint() {
    accepted(
        "class Named { name: String; construct(name: String) { this.name = name; } }
         fn show<T from Named>(value: T): String { return value.name; }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_method_the_constraint_does_not_promise() {
    let output = rejected(
        "contract Greeter {  greet(): String; }
         fn show<T from Greeter>(value: T): Void { mut r = value.somethingElse(); }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(output.contains("Greeter"), "{output}");
}

#[test]
fn invalid_member_access_on_an_unconstrained_type_parameter() {
    let output = rejected(
        "fn show<T>(value: T): Void { mut r = value.anything(); }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(output.contains("no `from` constraint"), "{output}");
}

#[test]
fn valid_call_infers_the_type_parameter_from_its_argument() {
    // `identity(5)` infers and lowers the concrete `T = Int32` instance.
    accepted(
        "fn identity<T>(value: T): T { return value; }
         fn main(): Void { mut r: Int32 = identity(5); }",
    );
}

#[test]
fn invalid_call_cannot_infer_a_type_parameter_used_only_in_the_return_type() {
    // Inference only looks at arguments today: nothing here determines `T`.
    let output = rejected(
        "fn make<T>(): T { return 5; }
         fn main(): Void { mut r = make(); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("cannot infer"), "{output}");
}

#[test]
fn invalid_call_infers_conflicting_types_for_the_same_parameter() {
    let output = rejected(
        "fn pair<T>(a: T, b: T): Void { }
         fn main(): Void { pair(5, \"x\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("cannot infer"), "{output}");
}

#[test]
fn invalid_call_infers_a_type_that_does_not_satisfy_the_constraint() {
    let output = rejected(
        "contract Serializable {  serialize(): String; }
         fn store<T from Serializable>(value: T): Void { }
         fn main(): Void { store(5); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("Serializable"), "{output}");
}

#[test]
fn valid_call_infers_a_type_that_satisfies_the_constraint() {
    accepted(
        "contract Serializable {  serialize(): String; }
         class Doc implements Serializable { construct() { }  serialize(): String { return \"\"; } }
         fn store<T from Serializable>(value: T): Void { }
         fn main(): Void { store(Doc()); }",
    );
}

#[test]
fn valid_contract_property_is_satisfied_by_a_class_field() {
    accepted(
        "contract Named { name: String; }
         class User implements Named { name: String = \"Ada\"; }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_contract_property_must_exist_with_compatible_type() {
    let output = rejected(
        "contract Named { name: String; }
         class User implements Named { age: Int32 = 42; }
         fn main(): Void { }",
    );
    assert!(
        output.contains("does not implement property `name`"),
        "{output}"
    );
}

#[test]
fn invalid_contract_mutable_property_rejects_an_immutable_field() {
    let output = rejected(
        "contract Named { mut name: String; }
         class User implements Named { inmut name: String = \"Ada\"; }
         fn main(): Void { }",
    );
    assert!(output.contains("not mutable enough"), "{output}");
}

#[test]
fn invalid_contract_cannot_inherit_conflicting_property_shapes() {
    let output = rejected(
        "contract Named { name: String; }
         contract Numbered { name: Int32; }
         contract Both implements Named, Numbered { }
         fn main(): Void { }",
    );
    assert!(output.contains("incompatible property `name`"), "{output}");
}

#[test]
fn valid_generic_type_arguments_resolve_and_check_arity() {
    // `Box<Int32>` itself resolves (roadmap task 7.3), and `Box<T>` is
    // simple enough to fully specialize (11.1), so this compiles clean.
    accepted(
        "class Box<T> { value: T; }
         class Holder { value: Box<Int32>; }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_generic_type_argument_count() {
    let output = rejected(
        "class Pair<A, B> { a: A; b: B; }
         class Holder { value: Pair<Int32>; }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_generic_type_argument_missing_its_constraint() {
    let output = rejected(
        "contract Serializable {  serialize(): String; }
         class Box<T from Serializable> { value: T; }
         class Holder { value: Box<Int32>; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("Serializable"), "{output}");
}

#[test]
fn valid_generic_type_argument_satisfying_its_class_constraint() {
    accepted(
        "class Animal { }
         class Dog extends Animal { }
         class Cage<T from Animal> { value: T; }
         class Holder { value: Cage<Dog>; }
         fn main(): Void { }",
    );
}

#[test]
fn valid_two_instantiations_of_the_same_class_are_the_same_type() {
    // Interned like a function type: `Box<Int32>` written twice is one id.
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Box<T> { value: T; }
         class Holder { a: Box<Int32>; b: Box<Int32>; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert_eq!(
        checked.generic_instances.len(),
        1,
        "`Box<Int32>` written twice should intern to one instantiation"
    );
}

// --- Enums algebraicos y mapping ----------------------------------------------

#[test]
fn valid_traditional_enum_is_unaffected() {
    accepted("enum Direction { North, South, East, West }\nfn main(): Void { }");
}

#[test]
fn valid_enum_variant_with_a_string_or_numeric_mapping() {
    accepted("enum Status { Ok -> 200, NotFound -> 404 }\nfn main(): Void { }");
    accepted("enum Role { Admin -> \"admin\", Guest -> \"guest\" }\nfn main(): Void { }");
}

#[test]
fn invalid_enum_variant_mapping_to_an_incompatible_type() {
    let output = rejected("enum Status { Ok -> true }\nfn main(): Void { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_algebraic_variant_constructs_and_destructures() {
    // Associated data fully lowers now — discriminant plus a flattened
    // payload (roadmap task 11.3/11.4) — so this compiles clean.
    accepted(
        "enum Shape { Circle(radius: Int32), Point }
         fn area(s: Shape): Int32 {
             return match s {
                 Shape.Circle(radius) => radius,
                 Shape.Point => 0,
             };
         }
         fn main(): Void { mut s = Shape.Circle(radius: 3); stdout.println(area(s)); }",
    );
}

#[test]
fn valid_algebraic_variant_matches_named_arguments() {
    accepted(
        "enum Shape { Rectangle(w: Int32, h: Int32) }
         fn main(): Void { mut s = Shape.Rectangle(h: 2, w: 4); }",
    );
}

#[test]
fn valid_generic_enum_with_data_instantiates_like_a_native_one() {
    // `fase-3-generic-enums`: a user-declared generic enum instantiates and
    // type-checks the same way `Result<T,E>` already does — the checker's
    // own `is_native` gate at `resolve_enum_reference` was a scope fence,
    // not a mechanism limit (`specialize_enum` runs generically over every
    // enum instantiation regardless of which enum it is).
    accepted(
        "enum Box<T> { Full(value: T), Empty }
         fn main(): Void { mut b: Box<Int32> = Box.Full(1); }",
    );
}

#[test]
fn valid_generic_enum_with_multiple_type_parameters() {
    // Design D2's first shape to verify: more than one type variable per
    // instantiation.
    accepted(
        "enum Either<L, R> { Left(value: L), Right(value: R) }
         fn main(): Void {
             mut a: Either<Int32, String> = Either.Left(1);
             mut b: Either<Int32, String> = Either.Right(\"x\");
         }",
    );
}

#[test]
fn valid_native_outcome_generic_type_and_fulfilled_variant() {
    accepted_body(
        "mut outcome: Outcome<Int32> = Outcome.Fulfilled(7);\n\
         mut text: Outcome<String> = Outcome.Fulfilled(\"ok\");",
    );
}

#[test]
fn native_outcome_rejected_and_cancelled_variants_have_typed_payloads() {
    accepted(
        "fn rejected(error: Throwable): Outcome<Int32> {\n\
             return Outcome.Rejected(error);\n\
         }\n\
         fn cancelled(error: CancelledError): Outcome<Int32> {\n\
             return Outcome.Cancelled(error);\n\
         }\n\
         fn main(): Void { }",
    );
}

#[test]
fn cancelled_error_is_concrete_constructible_and_a_complete_throwable() {
    accepted(
        "fn main(): Void {
             mut cancelled: CancelledError = CancelledError(\"cancelled\");
             mut message: String = cancelled.message();
             mut code: String = cancelled.code();
             mut cause: Error? = cancelled.cause();
             mut trace: String = cancelled.stack_trace();
             mut outcome: Outcome<Int32> = Outcome.Cancelled(cancelled);
             match outcome {
                 Outcome.Cancelled(error) => {
                     mut inherited: String = error.message();
                 }
                 _ => { }
             }
         }",
    );
}

#[test]
fn transfer_in_one_if_branch_invalidates_the_binding_after_the_join() {
    let output = rejected(
        "class FakeFile implements TransferableResource {\n\
             construct() { }\n\
             read_all(): String { return \"ok\"; }\n\
         }\n\
         fn main(): Void {\n\
             mut file = FakeFile();\n\
             if true { mut moved = transfer(file); } else { }\n\
             _ = file.read_all();\n\
         }",
    );
    assert!(
        output.contains(codes::USE_OF_TRANSFERRED_RESOURCE.as_str()),
        "{output}"
    );
}

#[test]
fn transfer_in_one_if_expression_branch_invalidates_the_binding_after_the_join() {
    let output = rejected(
        "class FakeFile implements TransferableResource {\n\
             construct() { }\n\
             read_all(): String { return \"ok\"; }\n\
         }\n\
         fn main(): Void {\n\
             mut file = FakeFile();\n\
             mut selected = if true { transfer(file) } else { file };\n\
             _ = file.read_all();\n\
         }",
    );
    assert!(
        output.contains(codes::USE_OF_TRANSFERRED_RESOURCE.as_str()),
        "{output}"
    );
}

#[test]
fn transfer_in_one_ternary_branch_invalidates_the_binding_after_the_join() {
    let output = rejected(
        "class FakeFile implements TransferableResource {\n\
             construct() { }\n\
             read_all(): String { return \"ok\"; }\n\
         }\n\
         fn main(): Void {\n\
             mut file = FakeFile();\n\
             mut selected = true ? transfer(file) : file;\n\
             _ = file.read_all();\n\
         }",
    );
    assert!(
        output.contains(codes::USE_OF_TRANSFERRED_RESOURCE.as_str()),
        "{output}"
    );
}

#[test]
fn transfer_in_one_match_arm_invalidates_the_binding_after_the_join() {
    let output = rejected(
        "class FakeFile implements TransferableResource {\n\
             construct() { }\n\
             read_all(): String { return \"ok\"; }\n\
         }\n\
         fn main(): Void {\n\
             mut file = FakeFile();\n\
             match true { true => transfer(file), false => file };\n\
             _ = file.read_all();\n\
         }",
    );
    assert!(
        output.contains(codes::USE_OF_TRANSFERRED_RESOURCE.as_str()),
        "{output}"
    );
}

#[test]
fn transfer_before_a_throw_is_visible_to_a_catch_join() {
    let output = rejected(&format!(
        "{BOOM_ERROR}\n\
         class FakeFile implements TransferableResource {{\n\
             construct() {{ }}\n\
             read_all(): String {{ return \"ok\"; }}\n\
         }}\n\
         fn main(): Void {{\n\
             mut file = FakeFile();\n\
             try {{ mut moved = transfer(file); throw BoomError(); }}\n\
             catch BoomError(e) {{ _ = file.read_all(); }}\n\
         }}"
    ));
    assert!(
        output.contains(codes::USE_OF_TRANSFERRED_RESOURCE.as_str()),
        "{output}"
    );
}

#[test]
fn transfer_inside_a_loop_is_rejected_until_repeated_moves_are_proven_safe() {
    let output = rejected(
        "class FakeFile implements TransferableResource {\n\
             construct() { }\n\
         }\n\
         fn main(): Void {\n\
             mut file = FakeFile();\n\
             while true { mut moved = transfer(file); break; }\n\
         }",
    );
    assert!(
        output.contains(codes::TRANSFER_IN_LOOP.as_str()),
        "{output}"
    );
}

#[test]
fn transfer_in_a_for_step_is_also_rejected_as_repeated() {
    let output = rejected(
        "class FakeFile implements TransferableResource {\n\
             construct() { }\n\
         }\n\
         fn main(): Void {\n\
             mut file = FakeFile();\n\
             for (mut i = 0; i < 1; transfer(file)) { }\n\
         }",
    );
    assert!(
        output.contains(codes::TRANSFER_IN_LOOP.as_str()),
        "{output}"
    );
}

#[test]
fn reusable_closure_cannot_transfer_a_captured_resource() {
    let output = rejected(
        "class FakeFile implements TransferableResource {\n\
             construct() { }\n\
         }\n\
         fn main(): Void {\n\
             mut file = FakeFile();\n\
             mut action: Fn() => Void = (): Void => { mut moved = transfer(file); };\n\
             action();\n\
         }",
    );
    assert!(
        output.contains(codes::TRANSFER_IN_REUSABLE_CLOSURE.as_str()),
        "{output}"
    );
}

#[test]
fn concurrent_of_stays_gated_without_a_supported_terminal() {
    let output = rejected_body("Concurrent.of((): Int32 => 1, (): Int32 => 2);");
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(
        !output.contains(codes::UNDECLARED_NAME.as_str()),
        "{output}"
    );
}

#[test]
fn within_and_collection_combinators_have_typed_pipeline_plans() {
    accepted_body(
        "mut first: Int32 = Concurrent.of((): Int32 => 1, (): Int32 => 2).within(1s).first();",
    );
    accepted_body(
        "mut outcomes: List<Outcome<Int32>> = Concurrent.of((): Int32 => 1, (): Int32 => 2).within(1s).settled();",
    );

    accepted_body(
        "inmut::strict values: List<Int32> = [1, 2];
         mut results: List<Int32> = Concurrent.each(values, (item: Int32): Int32 => item, 2);",
    );
    accepted_body(
        "inmut::strict values: List<Int32> = [1, 2];
         mut outcomes: List<Outcome<Int32>> = Concurrent.each_settled(values, (item: Int32): Int32 => item, 2);",
    );
    accepted_body(
        "inmut::strict values: List<Int32> = [1, 2];
         mut results: List<Int32> = Concurrent.each(values, (item: Int32): Int32 => item, 0);",
    );

    let invalid_collection = rejected_body("Concurrent.each(1, (item: Int32): Int32 => item);");
    assert!(
        invalid_collection.contains(codes::TYPE_MISMATCH.as_str()),
        "{invalid_collection}"
    );
    assert!(!invalid_collection.contains(codes::PENDING_FEATURE.as_str()));

    let nullable_collection = rejected_body(
        "mut values: List<Int32>? = null;
         Concurrent.each(values, (item: Int32): Int32 => item);",
    );
    assert!(
        nullable_collection.contains(codes::TYPE_MISMATCH.as_str()),
        "{nullable_collection}"
    );
    let nullable_array = rejected_body(
        "mut values: Array<Int32>? = null;
         Concurrent.each(values, (item: Int32): Int32 => item);",
    );
    assert!(
        nullable_array.contains(codes::TYPE_MISMATCH.as_str()),
        "{nullable_array}"
    );

    let aliased_collection = rejected(
        "fn values(): List<Int32> { mut result: List<Int32> = [1]; return result; }
         fn main(): Void { Concurrent.each(values(), (item: Int32): Int32 => item); }",
    );
    assert!(
        aliased_collection.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "{aliased_collection}"
    );

    let mutable_collection = rejected_body(
        "mut values: List<Int32> = [1, 2];
         Concurrent.each(values, (item: Int32): Int32 => item);",
    );
    assert!(
        mutable_collection.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "mutable collection must not cross the shared callback boundary: {mutable_collection}"
    );
    assert!(!mutable_collection.contains(codes::PENDING_FEATURE.as_str()));

    accepted_body(
        "inmut::strict values: List<Int32> = [1, 2];
         Concurrent.each(values, (item: Int32): Int32 => item);",
    );

    let inline_collection = rejected_body("Concurrent.each([1, 2], (item: Int32): Int32 => item);");
    assert!(
        inline_collection.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "inline collection has no strict deep-share proof: {inline_collection}"
    );

    let mutable_element_graph = rejected(
        "class MutableBox {\n\
             mut value: Int32;\n\
             construct() { this.value = 1; }\n\
         }\n\
         fn main(): Void {\n\
             inmut::strict values: List<MutableBox> = [MutableBox()];\n\
             Concurrent.each(values, (item: MutableBox): Int32 => item.value);\n\
         }",
    );
    assert!(
        mutable_element_graph.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "strict container is insufficient when its nested element graph is mutable: {mutable_element_graph}"
    );

    let invalid_limit =
        rejected_body("Concurrent.each([1, 2], (item: Int32): Int32 => item, true);");
    assert!(
        invalid_limit.contains(codes::TYPE_MISMATCH.as_str()),
        "{invalid_limit}"
    );

    let mutable_capture = rejected_body(
        "mut rows: List<Int32> = [1];
         Concurrent.each([1], (item: Int32): Int32 => rows[0]);",
    );
    assert!(mutable_capture.contains("E0430"), "{mutable_capture}");

    let non_transferable_result = rejected_body(
        "mut value: Int32 = 1;
         Concurrent.each([1], (item: Int32): Pointer<Int32> => unsafe { Pointer.from(value) });",
    );
    assert!(
        non_transferable_result.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "{non_transferable_result}"
    );

    let void_callback = rejected_body("Concurrent.each([1, 2], (item: Int32): Void => {});");
    assert!(
        void_callback.contains(codes::TYPE_MISMATCH.as_str()),
        "{void_callback}"
    );

    accepted_body(
        "inmut::strict values: List<Int32> = [1, 2];
         Concurrent.each(values, (item: Int32): Int32 => { Timer.sleep(1s); return item; });",
    );

    accepted(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             inmut::strict values: List<Int32> = [1];
             Concurrent.each_settled(values, (item: Int32): Int32 => {{ throw BoomError(); }});
         }}"
    ));
}

#[test]
fn concurrent_settled_returns_ordered_list_of_generic_outcomes() {
    accepted_body(
        "mut outcomes: List<Outcome<Int32>> = Concurrent.of((): Int32 => 1, (): Int32 => 2).settled();",
    );
}

#[test]
fn user_classes_named_like_compiler_namespaces_keep_static_method_resolution() {
    accepted(
        "class Concurrent {\n\
             static of(value: Int32): Int32 { return value; }\n\
             static each(value: Int32): Int32 { return value; }\n\
             static detach(value: Int32): Int32 { return value; }\n\
             static protect(value: Int32): Int32 { return value; }\n\
         }\n\
         class Timer { static sleep(value: Int32): Int32 { return value; } }\n\
         class Parallel { static each(value: Int32): Int32 { return value; } }\n\
         fn main(): Void {\n\
             Concurrent.of(1);\n\
             Concurrent.each(2);\n\
             Concurrent.detach(3);\n\
             Concurrent.protect(4);\n\
             Timer.sleep(5);\n\
             Parallel.each(6);\n\
         }",
    );

    accepted_body("mut winner: Int32 = Concurrent.of((): Int32 => 1, (): Int32 => 2).first();");

    let each = rejected_body("Concurrent.each(1);");
    assert!(
        each.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{each}"
    );
    assert!(!each.contains(codes::PENDING_FEATURE.as_str()), "{each}");
}

#[test]
fn completion_pipeline_terminals_validate_arguments() {
    let first = rejected_body("Concurrent.of((): Int32 => 1, (): Int32 => 2).first(1);");
    assert!(
        first.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{first}"
    );
    assert!(!first.contains(codes::PENDING_FEATURE.as_str()), "{first}");

    let settled = rejected_body("Concurrent.of((): Int32 => 1, (): Int32 => 2).settled(1);");
    assert!(
        settled.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{settled}"
    );

    let within = rejected_body("Concurrent.of((): Int32 => 1, (): Int32 => 2).within(1);");
    assert!(within.contains(codes::TYPE_MISMATCH.as_str()), "{within}");
    assert!(within.contains(codes::PENDING_FEATURE.as_str()), "{within}");
}

#[test]
fn concurrent_of_rejects_mutable_reference_captures_at_branch_boundary() {
    let output = rejected(
        "class Counter {\n\
           mut value: Int32;\n\
           construct() { this.value = 0; }\n\
           read(): Int32 { return this.value; }\n\
         }\n\
         fn main(): Void {\n\
           mut counter = Counter();\n\
           _ = Concurrent.of((): Int32 => counter.read(), (): Int32 => 2);\n\
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
    assert!(
        output.contains("`Concurrent.of` branch boundary"),
        "{output}"
    );
}

#[test]
fn concurrent_of_requires_branch_results_to_cross_safely() {
    let output = rejected(
        "class MutableValue implements Resource { mut value: Int32; construct() { this.value = 0; } }\n\
         fn make(): MutableValue { return MutableValue(); }\n\
         fn main(): Void {\n\
           _ = Concurrent.of((): MutableValue => make(), (): MutableValue => make());\n\
         }",
    );
    assert!(
        output.contains(codes::UNSAFE_CONCURRENT_BOUNDARY.as_str()),
        "{output}"
    );
    assert!(output.contains("branch result"), "{output}");
}

#[test]
fn concurrent_of_rejects_heterogeneous_thunk_results() {
    let output = rejected_body("Concurrent.of((): Int32 => 1, (): String => \"two\");");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(
        output.contains("homogeneous thunk result types"),
        "{output}"
    );
    assert!(!output.contains("undeclared"), "{output}");

    let settled = rejected_body("Concurrent.of((): Int32 => 1, (): String => \"two\").settled();");
    assert!(settled.contains(codes::TYPE_MISMATCH.as_str()), "{settled}");
    assert!(
        settled.contains("homogeneous thunk result types"),
        "{settled}"
    );
}

#[test]
fn concurrent_of_rejects_void_results_for_typed_outcomes() {
    let source = "Concurrent.of((): Void => { return; }, (): Void => { return; }).settled();";
    let output = rejected_body(source);
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("must return a value"), "{output}");
    assert!(output.contains("found `Void`"), "{output}");

    let first =
        rejected_body("Concurrent.of((): Void => { return; }, (): Void => { return; }).first();");
    assert!(first.contains(codes::TYPE_MISMATCH.as_str()), "{first}");
    assert!(first.contains("found `Void`"), "{first}");
}

#[test]
fn concurrent_of_rejects_invalid_arity_and_parameterized_thunks() {
    let arity = rejected_body("Concurrent.of((): Int32 => 1);");
    assert!(
        arity.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{arity}"
    );

    let parameterized = rejected_body("Concurrent.of((x: Int32): Int32 => x, (): Int32 => 2);");
    assert!(
        parameterized.contains(codes::TYPE_MISMATCH.as_str()),
        "{parameterized}"
    );
    assert!(
        parameterized.contains("must take no arguments"),
        "{parameterized}"
    );

    let void_result = rejected_body("Concurrent.of((): Void => { }, (): Void => { }).settled();");
    assert!(
        void_result.contains(codes::TYPE_MISMATCH.as_str()),
        "{void_result}"
    );
    assert!(void_result.contains("must return a value"), "{void_result}");
}

#[test]
fn concurrent_of_rejects_non_lambda_named_and_spread_arguments() {
    let non_lambda = rejected_body("Concurrent.of(compute, (): Int32 => 2);");
    assert!(
        non_lambda.contains(codes::TYPE_MISMATCH.as_str()),
        "{non_lambda}"
    );
    assert!(
        non_lambda.contains("must be a closure literal"),
        "{non_lambda}"
    );

    let named = rejected_body("Concurrent.of(first: (): Int32 => 1, (): Int32 => 2);");
    assert!(
        named.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{named}"
    );

    let spread = rejected_body(
        "mut thunks: Array<Fn() => Int32> = [(): Int32 => 1, (): Int32 => 2]; Concurrent.of(...thunks);",
    );
    assert!(
        spread.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{spread}"
    );
}

#[test]
fn concurrent_protect_gates_direct_and_transitive_suspension() {
    let direct = rejected_body("Concurrent.protect((): Void => Timer.sleep(1s));");
    assert!(
        direct.contains(codes::SUSPENDING_PROTECTED_REGION.as_str()),
        "{direct}"
    );

    let transitive = rejected(
        "fn pause(): Void { Timer.sleep(1s); }\n\
         fn main(): Void { Concurrent.protect((): Void => pause()); }",
    );
    assert!(
        transitive.contains(codes::SUSPENDING_PROTECTED_REGION.as_str()),
        "{transitive}"
    );
}

#[test]
fn concurrent_protect_accepts_a_bounded_void_callback() {
    accepted_body("Concurrent.protect((): Void => { mut n: Int32 = 0; n += 1; });");
}

#[test]
fn concurrent_protect_rejects_a_non_void_callback() {
    let output = rejected_body("Concurrent.protect((): Int32 => 1);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("must return `Void`"), "{output}");
}

#[test]
fn concurrent_protect_rejects_blocking_unknown_and_spawning_cleanup_effects() {
    let blocking = rejected(
        "extern \"C\" fn host_wait(): Void;\n\
         fn main(): Void { Concurrent.protect((): Void => unsafe { host_wait(); }); }",
    );
    assert!(
        blocking.contains(codes::SUSPENDING_PROTECTED_REGION.as_str()),
        "{blocking}"
    );

    let unknown = rejected(
        "fn invoke(action: Fn() => Void): Void { action(); }\n\
         fn main(): Void { Concurrent.protect((): Void => invoke((): Void => { })); }",
    );
    assert!(
        unknown.contains(codes::SUSPENDING_PROTECTED_REGION.as_str()),
        "{unknown}"
    );

    let spawning = rejected(
        "fn schedule(): Void { concurrent { spawn (): Void => { }; } }\n\
         fn main(): Void { Concurrent.protect((): Void => schedule()); }",
    );
    assert!(
        spawning.contains(codes::SUSPENDING_PROTECTED_REGION.as_str()),
        "{spawning}"
    );

    let channel = rejected(
        "fn main(): Void {
             mut channel: Channel<Int32> = Channel(1);
             Concurrent.protect((): Void => { channel.send(1); });
         }",
    );
    assert!(
        channel.contains(codes::SUSPENDING_PROTECTED_REGION.as_str()),
        "channel waits must be rejected in protected cleanup: {channel}"
    );

    let thread = rejected_body("Concurrent.protect((): Void => { Thread.run((): Int32 => 1); });");
    assert!(
        thread.contains(codes::SUSPENDING_PROTECTED_REGION.as_str()),
        "joining a managed thread must be rejected inside protected cleanup: {thread}"
    );

    let transitive_thread = rejected(
        "fn run_worker(): Void { _ = Thread.run((): Int32 => 1); }\n\
         fn main(): Void { Concurrent.protect((): Void => run_worker()); }",
    );
    assert!(
        transitive_thread.contains(codes::SUSPENDING_PROTECTED_REGION.as_str()),
        "the managed-thread blocking effect must propagate through calls: {transitive_thread}"
    );
}

#[test]
fn concurrent_protect_rejects_non_lambda_parameterized_and_wrong_arity() {
    let non_lambda = rejected_body("Concurrent.protect(cleanup);");
    assert!(
        non_lambda.contains(codes::TYPE_MISMATCH.as_str()),
        "{non_lambda}"
    );
    assert!(
        non_lambda.contains("requires a lambda literal"),
        "{non_lambda}"
    );

    let parameterized =
        rejected_body("Concurrent.protect((reason: String): Void => stdout.println(reason));");
    assert!(
        parameterized.contains(codes::TYPE_MISMATCH.as_str()),
        "{parameterized}"
    );
    assert!(
        parameterized.contains("cleanup lambda takes no arguments"),
        "{parameterized}"
    );

    let arity = rejected_body("Concurrent.protect();");
    assert!(
        arity.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{arity}"
    );
    let extra = rejected_body("Concurrent.protect((): Void => { }, (): Void => { });");
    assert!(
        extra.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{extra}"
    );
}

#[test]
fn invalid_generic_enum_type_argument_violating_a_constraint_is_still_rejected() {
    // Lifting the `NOT_LOWERED` gate does not relax constraint checking: a
    // type argument that does not satisfy the parameter's `from` constraint
    // is rejected the same way it already was before this change.
    let output = rejected(
        "class Animal { construct() { } }
         enum Holder<T from Animal> { Has(value: T) }
         fn main(): Void { mut h: Holder<Int32> = Holder.Has(1); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_non_generic_recursive_enum_is_not_lowered_yet() {
    // `fase-3-recursive-enums`: `register_enum` registers `IntList`'s own
    // name and arity ahead of `declare_enum_variants` resolving `tail`'s
    // type, the same two-phase split `register_class`/`declare_class_members`
    // already use for a self-referencing class field — so the *declaration*
    // itself resolves the name correctly now. But every enum still lowers to
    // an inline-flattened struct with no indirection anywhere in the
    // pipeline: a variant field that embeds the enum's own type with nothing
    // in between asks for an infinitely-sized type and crashed the compiler
    // with a stack overflow before this diagnostic existed (confirmed against
    // a real build, not assumed). Boxing a genuinely self-referential field
    // is its own future design (a new `IrType` case, a runtime allocation
    // kind, GC rules) — gated the same way any other checked-but-not-yet-
    // compilable construct is, rather than left to crash.
    let output = rejected(
        "enum IntList { Nil, Cons(head: Int32, tail: IntList) }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn invalid_generic_recursive_enum_is_not_lowered_yet() {
    // The generic case: `Tree<T>`'s own recursive field is exactly `Tree<T>`,
    // the same instantiation being declared, not a nested distinct one — so
    // `fase-3-generic-substitution-recursion`'s own gap does not apply here.
    // It still hits the same inline-embedding hazard `IntList` does.
    let output = rejected(
        "enum Tree<T> { Leaf, Node(value: T, left: Tree<T>, right: Tree<T>) }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn valid_class_and_enum_reference_each_other_enum_declared_first() {
    // Design's second flagged risk: a mutually-recursive class/enum pair.
    // Enum declared first in the file, referencing a class declared after
    // it; the class also references the enum back.
    accepted(
        "enum Wrapper { Has(value: Box), Empty }
         class Box { field: Wrapper; construct(field: Wrapper) { this.field = field; } }
         fn main(): Void { }",
    );
}

#[test]
fn valid_class_and_enum_reference_each_other_class_declared_first() {
    // Same mutual reference, opposite declaration order — order must not
    // matter either way.
    accepted(
        "class Box { field: Wrapper; construct(field: Wrapper) { this.field = field; } }
         enum Wrapper { Has(value: Box), Empty }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_literal_pattern_destructuring_a_variant_is_not_lowered_yet() {
    // A binding or a wildcard destructures a variant's field for real
    // (task 11.4); a literal sub-pattern would need combined-condition
    // compilation this pass does not build.
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn main(): Void {
             mut s = Shape.Circle(radius: 3);
             match s { Shape.Circle(3) => { } _ => { } }
         }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

// --- Enum static members (`enum-static-members`) ------------------------------

#[test]
fn valid_enum_static_members_on_a_traditional_enum() {
    accepted(
        "enum Direction { North, South, East, West }
         fn main(): Void {
             mut n: Int32 = Direction.count;
             mut names: List<String> = Direction.keys();
             mut values: List<Direction> = Direction.values();
             mut found = Direction.from_name(\"North\");
             mut by_value = Direction.from_value(0);
         }",
    );
}

#[test]
fn valid_enums_helpers_take_an_enum_type_name() {
    accepted(
        "enum Direction { North, South }
         fn main(): Void {
             mut names: List<String> = Enums.keys(Direction);
             mut values: List<Direction> = Enums.values(Direction);
             mut n: Int32 = Enums.count(Direction);
         }",
    );
}

#[test]
fn invalid_from_value_on_an_algebraic_enum() {
    // A payload case cannot be materialized out of nothing — `Shape.Circle`
    // needs its `radius`.
    for call in ["values()", "from_name(\"x\")", "from_value(0)"] {
        let output = rejected(&format!(
            "enum Shape {{ Circle(radius: Int32), Point }}
             fn main(): Void {{ mut r = Shape.{call}; }}",
        ));
        assert!(
            output.contains(codes::UNKNOWN_MEMBER.as_str()),
            "for `{call}`:\n{output}"
        );
    }
}

#[test]
fn invalid_enum_static_call_with_the_wrong_argument_count() {
    for call in [
        "Direction.keys(1)",
        "Direction.values(1)",
        "Direction.from_name()",
        "Direction.from_name(\"a\", \"b\")",
        "Direction.from_value()",
    ] {
        let output = rejected(&format!(
            "enum Direction {{ North, South }}
             fn main(): Void {{ mut r = {call}; }}",
        ));
        assert!(
            output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
            "for `{call}`:\n{output}"
        );
    }
}

#[test]
fn valid_enum_to_string_in_both_spellings_and_on_algebraic_enums() {
    // The type-name render applies to every enum — a payload changes
    // nothing about what the type is called.
    accepted(
        "enum Direction { North, South }
         enum Shape { Circle(radius: Int32), Point }
         fn main(): Void {
             mut a: String = Direction.to_string;
             mut b: String = Direction.to_string();
             mut c: String = Shape.to_string();
         }",
    );
}

#[test]
fn invalid_enum_to_string_call_with_arguments() {
    let output = rejected(
        "enum Direction { North, South }
         fn main(): Void { mut r = Direction.to_string(1); }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_from_name_with_a_non_string_argument() {
    let output = rejected(
        "enum Direction { North, South }
         fn main(): Void { mut r = Direction.from_name(3); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_from_value_argument_against_the_mapping_type() {
    // `ExitCode`'s mappings are integers, so the lookup takes an `Int32`.
    let output = rejected(
        "enum ExitCode { Success -> 0, Failure -> 1 }
         fn main(): Void { mut r = ExitCode.from_value(\"x\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_enums_helper_on_a_non_enum_name() {
    let output = rejected(
        "class Box { }
         fn main(): Void { mut r = Enums.keys(Box); }",
    );
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn invalid_enum_variants_sharing_a_lookup_key() {
    // `from_value` compares against the effective key — the `->` mapping,
    // or the name/discriminant a case without one keeps — so a repeated
    // mapping, or one colliding with a bare case's own key, is ambiguous.
    for source in [
        "enum ExitCode { Success -> 0, Failure -> 0 }",
        "enum ExitCode { Success, Failure -> 0 }",
        "enum Role { Admin -> \"Guest\", Guest }",
    ] {
        let output = rejected(&format!("{source}\nfn main(): Void {{ }}"));
        assert!(
            output.contains(codes::DUPLICATE_DECLARATION.as_str()),
            "for `{source}`:\n{output}"
        );
    }
}

#[test]
fn invalid_enum_mixing_string_and_integer_mappings_has_no_lookup_type() {
    let output = rejected(
        "enum Mixed { A -> \"x\", B -> 1 }
         fn main(): Void { mut r = Mixed.from_value(\"x\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

// --- Objects and contracts ----------------------------------------------------

#[test]
fn valid_contract_object_assignment_and_field_access() {
    accepted(
        "contract Point { x: Int32; y: Int32; }
         fn main(): Void { mut p: Point = { x: 1, y: 2 }; stdout.println(p.x); }",
    );
}

#[test]
fn valid_class_omitted_field_takes_its_type_default() {
    accepted(
        "class Settings { count: Int32; flag: Boolean; }
         fn main(): Void { mut s = Settings(); }",
    );
}

#[test]
fn legacy_record_declaration_reports_migration_error() {
    let output = rejected(
        "class Engine { }
         record Car { engine: Engine; }
         fn main(): Void { mut c = Car(); }",
    );
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn legacy_record_construction_syntax_is_rejected() {
    let output = rejected(
        "record Point { x: Int32; y: Int32; }
         fn main(): Void { mut p = Point(1, 2); }",
    );
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn legacy_record_extends_syntax_is_rejected() {
    let output = rejected(
        "class Base { }
         record Point extends Base { x: Int32; }
         fn main(): Void { }",
    );
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn legacy_record_constructor_syntax_is_rejected() {
    let output = rejected(
        "record Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn legacy_record_mutable_field_syntax_is_rejected() {
    let output = rejected(
        "record Point { mut x: Int32; }
         fn main(): Void { }",
    );
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn legacy_record_method_syntax_is_rejected() {
    let output = rejected(
        "record Point {
             x: Int32;
              reset(): Void { this.x = 0; }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn valid_object_identity_is_not_structural_equality() {
    accepted(
        "fn main(): Void { mut a: Object = { x: 1 }; mut alias: Object = a; mut b: Object = { x: 1 }; stdout.println(a is alias); stdout.println(a is b); }",
    );
}

#[test]
fn valid_class_reference_equality_uses_identity() {
    accepted(
        "class Money { amount: Int32; currency: String; construct(amount: Int32, currency: String) { this.amount = amount; this.currency = currency; } }
         fn main(): Void { mut a = Money(1, \"USD\"); mut alias = a; mut b = Money(1, \"USD\"); stdout.println(a is alias); stdout.println(a is b); }",
    );
}

#[test]
fn valid_nested_objects_keep_reference_identity() {
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         class Line { start: Point; construct(start: Point) { this.start = start; } }
         fn main(): Void {
             mut point = Point(1);
             mut line = Line(point);
             stdout.println(line.start is point);
         }",
    );
}

#[test]
fn valid_record_equality_over_a_class_field_with_its_own_equals() {
    // Design D2: a reference-typed field compares by whatever the
    // existing rule for that class's own `==` already is — here, its own
    // `_equals`.
    accepted(
        "class Id {
             value: Int32;
             construct(value: Int32) { this.value = value; }
              _equals(other: Id): Boolean { return this.value == other.value; }
         }
         class Tagged { id: Id; construct(id: Id) { this.id = id; } }
         fn main(): Void {
             mut a = Tagged(Id(1));
             mut b = Tagged(Id(1));
             stdout.println(a.id == b.id);
         }",
    );
}

#[test]
fn valid_record_equality_over_a_class_field_without_equals_uses_identity() {
    // Design D2's other branch: no `_equals` declared, so the field
    // compares by identity (`is`), never a `TYPE_MISMATCH`/`NOT_LOWERED`
    // the way a bare top-level `class == class` with no `_equals` would.
    accepted(
        "class Box { value: Int32; construct(value: Int32) { this.value = value; } }
         class Tagged { box: Box; construct(box: Box) { this.box = box; } }
         fn main(): Void {
             mut b = Box(1);
             mut a = Tagged(b);
             mut c = Tagged(b);
             stdout.println(a.box is c.box);
         }",
    );
}

#[test]
fn legacy_record_equality_syntax_is_rejected() {
    let output = rejected(
        "record Box { value: Int32?; }
         fn main(): Void { mut a = Box(value: 1); mut b = Box(value: 1); stdout.println(a == b); }",
    );
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn legacy_record_identity_syntax_is_rejected() {
    let output = rejected(
        "record Point { x: Int32; }
         fn main(): Void { mut a = Point(x: 1); mut b = Point(x: 1); stdout.println(a is b); }",
    );
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn legacy_generic_record_declaration_reports_migration_error() {
    let output = rejected("record Box<T> { value: T; }\nfn main(): Void { }");
    assert!(
        output.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{output}"
    );
}

#[test]
fn valid_class_method_called_directly() {
    accepted(
        "class Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
              double(): Int32 { return this.x * 2; }
         }
         fn main(): Void { mut p = Point(3); stdout.println(p.double()); }",
    );
}

#[test]
fn valid_class_implements_a_contract_and_dispatches_through_it() {
    accepted(
        "contract Describable {  describe(): String; }
         class Point implements Describable {
             x: Int32;
             construct(x: Int32) { this.x = x; }
              describe(): String { return \"point\"; }
         }
         fn announce(d: Describable): String { return d.describe(); }
         fn main(): Void {
             mut p = Point(3);
             mut d: Describable = p;
             stdout.println(d.describe());
             stdout.println(announce(p));
         }",
    );
}

#[test]
fn invalid_record_missing_what_a_contract_requires() {
    // A record's conformance is checked exactly the way a class's is
    // (unaffected by this change): a missing method is still caught.
    let output = rejected(
        "contract Describable {  describe(): String; }
         record Point implements Describable { x: Int32; }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
    assert!(output.contains("describe"), "{output}");
}

#[test]
fn invalid_mutating_a_field_through_a_boxed_record_s_contract_type() {
    // Design D3/task 4.4: no mutation path exists through a contract-typed
    // reference to a boxed `record`, by construction — a contract declares
    // methods only, never fields, so there is no member for an assignment
    // to even name, whether the concrete adopter behind it is a class or a
    // boxed value.
    let output = rejected(
        "contract Describable {  describe(): String; }
         record Point implements Describable {
             x: Int32;
              describe(): String { return \"point\"; }
         }
         fn main(): Void {
             mut p = Point(x: 3);
             mut d: Describable = p;
             d.x = 4;
         }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn invalid_record_not_implementing_the_named_interface() {
    // A record that never says `implements Describable` is rejected the
    // same way a class would be — this change only makes the *adopted*
    // case compile, not any and every record.
    let output = rejected(
        "contract Describable {  describe(): String; }
         record Point { x: Int32; }
         fn announce(d: Describable): String { return d.describe(); }
         fn main(): Void { mut p = Point(x: 3); stdout.println(announce(p)); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_object_contract_construction() {
    accepted(
        "contract UserId { value: Int32; }\nfn main(): Void { mut u: UserId = { value: 5 }; }",
    );
}

#[test]
fn invalid_constructing_a_variant_with_the_wrong_argument_type() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn main(): Void { mut s = Shape.Circle(radius: \"x\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_constructing_a_variant_with_the_wrong_argument_count() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn main(): Void { mut s = Shape.Circle(1, 2); }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_constructing_a_bare_variant_that_carries_data() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn main(): Void { mut s = Shape.Circle; }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_constructing_with_data_a_variant_that_carries_none() {
    let output = rejected(
        "enum Shape { Point }
         fn main(): Void { mut s = Shape.Point(1); }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_pattern_omits_the_destructuring_a_variant_needs() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn area(s: Shape): Int32 {
             match s { Shape.Circle => 0 }
         }
         fn main(): Void { mut s = Shape.Circle(radius: 1); stdout.println(area(s)); }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_over_an_algebraic_enum_is_not_exhaustive() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32), Point }
         fn area(s: Shape): Int32 {
             match s { Shape.Circle(radius) => radius }
         }
         fn main(): Void { mut s = Shape.Point; stdout.println(area(s)); }",
    );
    assert!(
        output.contains(codes::NON_EXHAUSTIVE_MATCH.as_str()),
        "{output}"
    );
}

#[test]
fn valid_match_null_arm_narrows_the_binding_arm() {
    accepted(
        "class Node { mut value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut n: Node? = Node(1);
             match n {
                 null => { }
                 binding => { stdout.println(binding.value); }
             }
         }",
    );
}

#[test]
fn valid_match_null_arm_narrows_the_variant_pattern_arm() {
    // Exhaustiveness over a nullable enum still needs a catch-all today
    // (a pre-existing rule this change does not touch) — the variant arm's
    // own destructuring is what is under test here, not exhaustiveness.
    accepted(
        "enum Status { Ok(value: Int32), Err(message: String) }
         fn main(): Void {
             mut s: Status? = Status.Ok(1);
             match s {
                 null => { }
                 Status.Ok(value) => { stdout.println(value); }
                 _ => { }
             }
         }",
    );
}

#[test]
fn invalid_match_binding_still_nullable_without_a_null_arm() {
    let output = rejected(
        "class Node { mut value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut n: Node? = Node(1);
             match n {
                 binding => { stdout.println(binding.value); }
             }
         }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_match_narrowing_does_not_apply_inside_the_null_arm_itself() {
    let output = rejected(
        "class Node { mut value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut n: Node? = Node(1);
             match n {
                 null => { stdout.println(n.value); }
                 binding => { stdout.println(binding.value); }
             }
         }",
    );
    assert!(
        output.contains(codes::TYPE_MISMATCH.as_str())
            || output.contains(codes::UNKNOWN_MEMBER.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_member_access_still_rejected_on_a_non_nullable_match_binding() {
    // Narrowing must not paper over a genuine mismatch: a `Node` scrutinee
    // (never nullable) that is matched against a member the class does not
    // have is still an error, `null`-arm narrowing or not.
    let output = rejected(
        "class Node { mut value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut n: Node = Node(1);
             match n {
                 binding => { stdout.println(binding.missing); }
             }
         }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

// --- Alias de tipo -------------------------------------------------------------

#[test]
fn valid_type_alias_is_transparent_with_its_target() {
    // Roadmap Phase 7, task 12.1: a `type` alias lowers — it expands to its
    // target everywhere, so there is nothing left to reject.
    accepted("type UserId = Int32;\nfn main(): Void { mut id: UserId = 5; }");
}

#[test]
fn valid_type_alias_to_a_declared_class() {
    accepted(
        "class User { construct() { } }
         type Account = User;
         fn main(): Void { mut a: Account = User(); }",
    );
}

#[test]
fn valid_type_alias_lowers() {
    // The `NOT_LOWERED` gate this used to assert is gone (roadmap Phase 7,
    // task 12.1): an alias resolves to its target and lowers.
    accepted(
        "type UserId = Int32;\nfn main(): Void { mut id: UserId = 5; mut other: Int32 = id; }",
    );
}

#[test]
fn invalid_type_alias_mismatched_target() {
    let output = rejected(
        "type RawId = Int32;\ntype UserId = RawId;\nfn main(): Void { mut id: UserId = \"x\"; }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("to UserId"), "{output}");
    assert!(output.contains("UserId` was expected"), "{output}");
}

#[test]
fn invalid_duplicate_type_alias() {
    let output = rejected(
        "type A = Int32;
         type A = String;
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
    assert!(
        output.contains("a previous `type` alias exists with this name"),
        "{output}"
    );
}

#[test]
fn invalid_type_alias_cycle() {
    let output = rejected(
        "type A = B;
         type B = A;
         fn main(): Void { mut x: A = 1; }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
    assert!(output.contains("itself"), "{output}");
    assert!(
        output.contains("A -> B -> A"),
        "the recursive alias diagnostic should retain its declaration chain: {output}"
    );
}

#[test]
fn guarded_recursive_object_aliases_are_finite_and_unguarded_cycles_still_fail() {
    let guarded = "type Node = { value: Int32; next: Node? };\n\
         type Left = { right: Right? };\n\
         type Right = { left: Left? };\n\
         fn value(node: Node): Int32 { return node.value; }\n\
         fn main(): Void { }";
    let (_, sink) = parse_and_check(guarded);
    let guarded_output = sink.render(RenderStyle::Human);
    assert!(
        sink.diagnostics().iter().all(|diagnostic| {
            diagnostic.severity != Severity::Error || diagnostic.code == codes::NOT_LOWERED
        }),
        "object-guarded recursion should resolve into a finite semantic graph: {guarded_output}"
    );

    let unguarded = rejected("type A = B; type B = A; fn main(): Void { mut value: A = 1; }");
    assert!(
        unguarded.contains(codes::DUPLICATE_DECLARATION.as_str())
            && unguarded.contains("A -> B -> A"),
        "an alias cycle without an object guard must keep its deterministic cycle diagnostic: {unguarded}"
    );

    let mut deep_chain = String::new();
    for index in 0..=256 {
        let target = if index == 256 {
            "Int32".to_string()
        } else {
            format!("Alias{}", index + 1)
        };
        deep_chain.push_str(&format!("type Alias{index} = {target};\n"));
    }
    deep_chain.push_str("fn main(): Void { mut value: Alias0 = 1; }");
    let deep = rejected(&deep_chain);
    assert!(
        deep.contains(codes::TYPE_COMPUTATION_LIMIT.as_str()),
        "alias unfolding must stop at its deterministic depth budget: {deep}"
    );
}

#[test]
fn valid_type_alias_chain() {
    accepted(
        "type A = Int32;
         type B = A;
         fn main(): Void { mut x: B = 5; }",
    );
}

// --- Uniones ---------------------------------------------------------------

#[test]
fn type_algebra_baseline_has_unambiguous_expected_types() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "types.zrk",
        "type Count = Int32;\n\
         type UserId = Count;\n\
         class Holder {\n\
           direct: Int32;\n\
           alias: UserId;\n\
           optional: String?;\n\
           union: Int32 | String;\n\
           mut active: Boolean;\n\
           inmut name: String;\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .expect("Holder is checked");
    let field = |name: &str| {
        holder
            .fields
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("missing field {name}"))
    };

    assert_eq!(field("direct").ty, Type::INT32);
    assert_eq!(field("alias").ty, Type::INT32);
    assert_eq!(field("direct").ty, field("alias").ty);
    assert_eq!(field("optional").ty, Type::STRING.as_nullable());
    assert_eq!(field("active").ty, Type::BOOLEAN);
    assert_eq!(field("active").mutability, Mutability::Mutable);
    assert_eq!(field("name").ty, Type::STRING);
    assert_eq!(field("name").mutability, Mutability::Immutable);
    let Base::Union(union_id) = field("union").ty.base else {
        panic!("Int32 | String should have the canonical union type");
    };
    let normalized = &checked.unions[union_id as usize];
    assert_eq!(normalized.len(), 2);
    assert!(normalized.contains(&Base::String));
    assert!(normalized.iter().any(|base| matches!(base, Base::Int(_))));
    assert!(
        !sink
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == codes::NOT_LOWERED),
        "the Int32 | String prototype union is admitted by the current lowering gate"
    );
}

#[test]
fn valid_union_type_accepts_either_alternative() {
    accepted(
        "fn f(x: String | Int32): Void { }
         fn main(): Void { f(\"a\"); f(1); }",
    );
}

#[test]
fn union_assignability_checks_every_source_member_and_target_alternatives() {
    accepted(
        "fn wider(value: String | Int32 | Boolean): Void { }\n\
         fn from_union(value: String | Int32): Void { wider(value); }\n\
         fn from_value(): Void { wider(1); }\n\
         fn main(): Void { }",
    );

    let output = rejected(
        "fn only_string(value: String): Void { }\n\
         fn from_union(value: String | Int32): Void { only_string(value); }\n\
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn intersection_assignability_uses_each_target_member_and_proven_source_member() {
    accepted(
        "contract Readable { read(): String; }\n\
         contract Closeable { close(): Void; }\n\
         fn needs_both(value: Readable & Closeable): Void { }\n\
         fn needs_readable(value: Readable): Void { }\n\
         fn from_intersection(value: Readable & Closeable): Void {\n\
             needs_both(value);\n\
             needs_readable(value);\n\
         }\n\
         class Both implements Readable, Closeable {\n\
             construct() { }\n\
             #override read(): String { return \"data\"; }\n\
             #override close(): Void { }\n\
         }\n\
         fn from_class(value: Both): Void { needs_both(value); }\n\
         fn main(): Void { }",
    );
}

#[test]
fn invalid_union_type_rejects_a_third_type() {
    let output = rejected(
        "fn f(x: String | Int32): Void { }
         fn main(): Void { f(true); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn normalization_budget_reports_source_location_without_interning_a_partial_union() {
    let alternative_count =
        zirk_sema::type_budget::BudgetKind::NormalizationMembers.limit() as usize + 1;
    let mut source = String::new();
    let names: Vec<_> = (0..alternative_count)
        .map(|index| {
            let name = format!("Kind{index}");
            source.push_str(&format!("enum {name} {{ Item }}\n"));
            name
        })
        .collect();
    source.push_str(&format!(
        "fn choose(value: {}): Void {{ }}\nfn main(): Void {{ }}",
        names.join(" | ")
    ));

    let (_sources, sink) = parse_and_check(&source);
    let diagnostic = sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::TYPE_COMPUTATION_LIMIT)
        .expect("normalization must fail at its deterministic member limit");
    assert!(diagnostic.message.contains("normalized type members"));
    assert!(diagnostic.message.contains("256"));
    let location = diagnostic
        .location
        .as_ref()
        .expect("normalization exhaustion must retain a source location");
    assert_eq!(location.file, "test.zrk");
    assert_eq!(location.line, alternative_count as u32 + 1);
}

#[test]
fn temporal_singleton_patterns_cover_canonical_union_members() {
    let source = "type Day = Date(2026, 12, 25) | Date(2027, 1, 1);\n\
         type Wait = 250ms | 1s;\n\
         fn classify(day: Day): String {\n\
             return match day { Date(2026, 12, 25) => \"holiday\"; Date(2027, 1, 1) => \"new year\"; };\n\
         }\n\
         fn classify_wait(wait: Wait): String {\n\
             return match wait { 250ms => \"short\"; 1s => \"long\"; };\n\
         }\n\
         fn main(): Void { }";
    let (_sources, sink) = parse_and_check(source);
    let diagnostics = sink.diagnostics();
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == codes::NON_EXHAUSTIVE_MATCH),
        "both canonical Date values are covered: {diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == codes::TYPE_MISMATCH),
        "each Date constructor pattern must match its singleton alternative: {diagnostics:?}"
    );

    let missing_source = "type Day = Date(2026, 12, 25) | Date(2027, 1, 1);\n\
         fn classify(day: Day): String {\n\
             return match day { Date(2026, 12, 25) => \"holiday\"; };\n\
         }\n\
         fn main(): Void { }";
    let (_sources, missing_sink) = parse_and_check(missing_source);
    let diagnostic = missing_sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::NON_EXHAUSTIVE_MATCH)
        .expect("an unhandled Date singleton must remain non-exhaustive");
    assert!(diagnostic.message.contains("singleton"));
}

#[test]
fn finite_singleton_unions_are_exhaustive_across_value_families() {
    let source = "type NumberChoice = 1 | 2;\n\
         type WordChoice = \"one\" | \"two\";\n\
         type FloatChoice = 1.5f64 | 2.5f64;\n\
         type CharChoice = 'a' | 'b';\n\
         type FlagChoice = true | false;\n\
         type TimeChoice = Time(9, 30) | Time(17, 0);\n\
         type EventChoice = DateTime(Date(2026, 12, 25), Time(9, 30)) | DateTime(Date(2027, 1, 1), Time(0, 0));\n\
         fn number(value: NumberChoice): String { return match value { 1 => \"one\"; 2 => \"two\"; }; }\n\
         fn word(value: WordChoice): String { return match value { \"one\" => \"first\"; \"two\" => \"second\"; }; }\n\
         fn float(value: FloatChoice): String { return match value { 1.5f64 => \"first\"; 2.5f64 => \"second\"; }; }\n\
         fn character(value: CharChoice): String { return match value { 'a' => \"first\"; 'b' => \"second\"; }; }\n\
         fn flag(value: FlagChoice): String { return match value { true => \"yes\"; false => \"no\"; }; }\n\
         fn time(value: TimeChoice): String { return match value { Time(9, 30) => \"morning\"; Time(17, 0) => \"evening\"; }; }\n\
         fn event(value: EventChoice): String { return match value { DateTime(Date(2026, 12, 25), Time(9, 30)) => \"holiday\"; DateTime(Date(2027, 1, 1), Time(0, 0)) => \"new year\"; }; }\n\
         fn main(): Void { }";
    let (_sources, sink) = parse_and_check(source);
    let diagnostics = sink.diagnostics();
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == codes::NON_EXHAUSTIVE_MATCH),
        "all finite members across these domains are covered: {diagnostics:?}"
    );

    let missing_source = "type WordChoice = \"one\" | \"two\";\n\
         fn word(value: WordChoice): String { return match value { \"one\" => \"first\"; }; }\n\
         fn main(): Void { }";
    let (_sources, missing_sink) = parse_and_check(missing_source);
    let diagnostic = missing_sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::NON_EXHAUSTIVE_MATCH)
        .expect("an uncovered String singleton must be reported");
    assert!(
        diagnostic
            .cause
            .as_deref()
            .unwrap_or_default()
            .contains("two")
    );
}

#[test]
fn valid_union_order_is_irrelevant() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder { a: String | Int32; b: Int32 | String; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert_eq!(
        checked.unions.len(),
        1,
        "`String | Int32` and `Int32 | String` should intern to one union"
    );
}

#[test]
fn generated_algebra_permutations_and_groupings_share_canonical_types() {
    let union_orders = [
        ["Animal", "Dog", "String"],
        ["Animal", "String", "Dog"],
        ["Dog", "Animal", "String"],
        ["Dog", "String", "Animal"],
        ["String", "Animal", "Dog"],
        ["String", "Dog", "Animal"],
    ];
    let intersection_orders = [
        ["Readable", "Closeable", "Taggable"],
        ["Readable", "Taggable", "Closeable"],
        ["Closeable", "Readable", "Taggable"],
        ["Closeable", "Taggable", "Readable"],
        ["Taggable", "Readable", "Closeable"],
        ["Taggable", "Closeable", "Readable"],
    ];
    let mut source = String::from(
        "class Animal { }\n\
         class Dog extends Animal { }\n\
         class Readable { }\n\
         class Closeable { }\n\
         class Taggable { }\n\
         type NestedUnion = (Animal | Dog) | String;\n\
         type NestedIntersection = (Readable & Closeable) & Taggable;\n\
         class Holder {\n",
    );
    for (index, order) in union_orders.iter().enumerate() {
        source.push_str(&format!("u{index}: {};\n", order.join(" | ")));
    }
    source.push_str("u_duplicate: Dog | Animal | String | Dog;\nu_nested: NestedUnion;\n");
    for (index, order) in intersection_orders.iter().enumerate() {
        source.push_str(&format!("i{index}: {};\n", order.join(" & ")));
    }
    source.push_str(
        "i_duplicate: Readable & Closeable & Taggable & Readable;\n\
         i_nested: NestedIntersection;\n\
         }\n\
         fn main(): Void { }",
    );

    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("generated-algebra.zrk", source));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .expect("generated fixture declares Holder");

    let union_type = holder.fields[0].ty;
    for field in &holder.fields[1..8] {
        assert_eq!(
            field.ty, union_type,
            "union normalization depends on syntax"
        );
    }
    let intersection_type = holder.fields[8].ty;
    for field in &holder.fields[9..16] {
        assert_eq!(
            field.ty, intersection_type,
            "intersection normalization depends on syntax"
        );
    }
}

#[test]
fn valid_union_duplicates_collapse() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder { a: String | String | Int32; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert_eq!(
        checked.unions[0].len(),
        2,
        "a repeated alternative is one member"
    );
}

#[test]
fn valid_union_flattens_nested_alias_members() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "type Inner = String | Int32;\n\
         class Holder { value: Inner | Boolean; }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    let Base::Union(id) = holder.fields[0].ty.base else {
        panic!("nested union should resolve to a union");
    };
    let members = &checked.unions[id as usize];
    assert_eq!(members.len(), 3, "nested members must be flattened");
    assert!(members.contains(&Base::String));
    assert!(members.contains(&Base::Boolean));
    assert!(members.iter().any(|base| matches!(base, Base::Int(_))));
}

#[test]
fn never_is_absorbed_by_unions_and_intersections() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "type Bottom = Never;\n\
         class Holder { union: Bottom | String; intersection: Never & String; }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, Type::STRING);
    assert_eq!(holder.fields[1].ty, Type::of(Base::Never));
    assert!(checked.unions.is_empty());
    assert!(checked.intersections.is_empty());
}

#[test]
fn singleton_type_values_are_canonical_and_reused_by_constants() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder {\n\
             yes_a: true; yes_b: true; no: false;\n\
             computed: 3; written: 3;\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, holder.fields[1].ty);
    assert_ne!(holder.fields[0].ty, holder.fields[2].ty);
    assert_eq!(holder.fields[3].ty, holder.fields[4].ty);
    assert_eq!(checked.singleton_values.len(), 3);
    assert!(matches!(holder.fields[0].ty.base, Base::Singleton(_)));
    assert!(matches!(holder.fields[3].ty.base, Base::Singleton(_)));
    assert!(
        checked
            .describe_type(holder.fields[0].ty)
            .contains("Boolean")
    );
    assert!(
        sink.diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code != codes::NOT_LOWERED)
    );
}

#[test]
fn tuple_types_are_written_as_types_without_literal_wrappers() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder {\n\
             first: Tuple<Int32, Tuple<Boolean, String>>;\n\
             same: Tuple<Int32, Tuple<Boolean, String>>;\n\
             other: Tuple<Int32, Tuple<Boolean, String>>;\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, holder.fields[1].ty);
    assert_eq!(holder.fields[0].ty, holder.fields[2].ty);
    assert!(!matches!(holder.fields[0].ty.base, Base::Singleton(_)));
}

#[test]
fn direct_object_type_shapes_keep_literal_discriminants() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder {\n\
             first: { x: 3; nested: String };\n\
             same: { nested: String; x: 3 };\n\
             other: { x: 4; nested: String };\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    assert!(
        sink.diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code != codes::TYPE_MISMATCH),
        "literal-valued object type shapes should remain valid: {}",
        sink.render(RenderStyle::Human)
    );
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, holder.fields[1].ty);
    assert_ne!(holder.fields[0].ty, holder.fields[2].ty);
    assert!(!matches!(holder.fields[0].ty.base, Base::Singleton(_)));
}

#[test]
fn enum_case_literals_are_typed_and_interned_directly() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "enum Shade { Red, Blue }\n\
         class Holder {\n\
             first: Shade.Red;\n\
             same: Shade.Red;\n\
             other_color: Shade.Blue;\n\
             other_variant: Shade.Blue;\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    assert!(
        sink.diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code == codes::NOT_LOWERED),
        "enum values should resolve semantically; only pending runtime lowering is expected: {}",
        sink.render(RenderStyle::Human)
    );
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, holder.fields[1].ty);
    assert_ne!(holder.fields[0].ty, holder.fields[2].ty);
    assert_eq!(holder.fields[2].ty, holder.fields[3].ty);
    let Base::Singleton(id) = holder.fields[0].ty.base else {
        panic!("enum constants must be represented as singleton types");
    };
    let zirk_sema::constant_value::ConstValue::Enum { fields, .. } =
        &checked.singleton_values[id as usize].value
    else {
        panic!("expected canonical algebraic enum value");
    };
    assert!(fields.is_empty());
}

#[test]
fn constructor_calls_are_rejected_as_singleton_type_syntax() {
    let (_sources, sink) = parse_and_check(
        "class Box { value: Int32; construct(value: Int32) { this.value = value; } }\n\
         type Boxed = Box(1);\n\
         fn take(value: Boxed): Void { }\n\
         fn main(): Void { }",
    );
    let diagnostic = sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == zirk_parser::codes::UNEXPECTED_TOKEN)
        .unwrap_or_else(|| {
            panic!(
                "class identity must not become a singleton: {}",
                sink.render(RenderStyle::Human)
            )
        });
    assert!(diagnostic.message.contains("type"));
}

#[test]
fn satisfies_checks_assignability_and_preserves_singleton_type() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "type Three = 3;\n\
         fn check(value: Three): Three { return value satisfies Int32; }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    assert!(
        sink.diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code == codes::NOT_LOWERED),
        "satisfies should type-check while retaining the singleton: {}",
        sink.render(RenderStyle::Human)
    );
    let expr = match &program.functions[0].body.statements[0] {
        Stmt::Return(statement) => statement.value.as_ref().unwrap(),
        other => panic!("expected a return containing satisfies, got {other:?}"),
    };
    let Expr::Satisfies(satisfies) = expr else {
        panic!("expected satisfies in the argument");
    };
    assert!(matches!(
        checked.expr_types[&satisfies.span].base,
        Base::Singleton(_)
    ));
}

#[test]
fn satisfies_rejects_an_expression_outside_the_target_type() {
    let output =
        rejected("fn main(): Void { inmut text: String = \"hello\"; _ = text satisfies Int32; }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("does not satisfy"), "{output}");
}

#[test]
fn builtin_singletons_are_assignable_to_their_owning_primitive_types() {
    let (_, sink) = parse_and_check(
        "fn take_bool(value: Boolean): Void { }\n\
         fn take_string(value: String): Void { }\n\
         fn take_date(value: Date): Void { }\n\
         fn main(): Void {\n\
             take_bool(true);\n\
             take_string(\"ready\");\n\
             take_date(Date(2026, 12, 25));\n\
         }",
    );
    assert!(
        !sink.has_errors(),
        "a singleton value must fit its owning primitive type: {}",
        sink.render(RenderStyle::Human)
    );
}

#[test]
fn mutable_numeric_literals_widen_to_the_primitive_for_reassignment() {
    accepted_body("mut count = 1; count = 2; count = 2147483647;");
    let immutable = rejected_body("inmut count = 1; count = 2;");
    assert!(
        immutable.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{immutable}"
    );
}

#[test]
fn expected_union_context_selects_and_retains_literal_alternatives() {
    accepted(
        "type Value = Int32 | String;\n\
         type Number = 1 | 2;\n\
         type Day = Date(2026, 12, 25) | Date(2027, 1, 1);\n\
         fn take(value: Value): Void { }\n\
         fn take_number(value: Number): Void { }\n\
         fn take_day(value: Day): Void { }\n\
         fn main(): Void {\n\
             mut current: Value = 1;\n\
             current = \"ready\";\n\
             take(2);\n\
             take(\"done\");\n\
             take_number(2);\n\
             take_day(Date(2026, 12, 25));\n\
         }",
    );
}

#[test]
fn explicit_singleton_annotation_rejects_a_different_value_after_widening() {
    let output = rejected("fn main(): Void { mut one: 1 = 1; one = 2; }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn primitive_union_members_absorb_owned_singletons() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder {\n\
             numeric: Int32 | 3;\n\
             text: String | \"ready\";\n\
             day: Date | Date(2026, 12, 25);\n\
             flags: true | false;\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, Type::INT32);
    assert_eq!(holder.fields[1].ty, Type::STRING);
    assert_eq!(holder.fields[2].ty, Type::of(Base::Date));
    assert_eq!(holder.fields[3].ty, Type::BOOLEAN);
}

#[test]
fn primitive_and_enum_union_coverage_absorbs_singleton_members() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "enum Color { Red, Blue }\n\
         class Holder {\n\
             integer: Int32 | 3;\n\
             float: Float64 | 1.5f64;\n\
             decimal: Decimal | 1.5;\n\
             text: String | \"ready\";\n\
             character: Char | 'x';\n\
             duration: Duration | 1s;\n\
             date: Date | Date(2026, 12, 25);\n\
             time: Time | Time(9, 30);\n\
             datetime: DateTime | DateTime(Date(2026, 12, 25), Time(9, 30));\n\
             flag: Boolean | true;\n\
             color: Color | Color.Red;\n\
             both_flags: true | false;\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, Type::INT32);
    assert_eq!(
        holder.fields[1].ty,
        Type::of(Base::Float(zirk_sema::FloatWidth::F64))
    );
    assert_eq!(holder.fields[2].ty, Type::of(Base::Decimal));
    assert_eq!(holder.fields[3].ty, Type::STRING);
    assert_eq!(holder.fields[4].ty, Type::of(Base::Char));
    assert_eq!(holder.fields[5].ty, Type::of(Base::Duration));
    assert_eq!(holder.fields[6].ty, Type::of(Base::Date));
    assert_eq!(holder.fields[7].ty, Type::of(Base::Time));
    assert_eq!(holder.fields[8].ty, Type::of(Base::DateTime));
    assert_eq!(holder.fields[9].ty, Type::BOOLEAN);
    assert!(matches!(holder.fields[10].ty.base, Base::Enum(_)));
    assert_eq!(holder.fields[11].ty, Type::BOOLEAN);
}

#[test]
fn integer_singletons_fit_only_representable_signed_and_unsigned_targets() {
    let (_sources, sink) = parse_and_check(
        "fn take_i8(value: Int8): Void { }\n\
         fn take_u8(value: UInt8): Void { }\n\
         fn signed(value: -128): Void { take_i8(value); }\n\
         fn unsigned(value: 255): Void { take_u8(value); }\n\
         fn main(): Void { }",
    );
    assert!(
        !sink
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == codes::TYPE_MISMATCH),
        "representable integer singletons should fit their destination widths"
    );

    for (source, expected) in [
        (
            "fn take_i8(value: Int8): Void { }\n\
             fn too_small(value: -129): Void { take_i8(value); }\n\
             fn main(): Void { }",
            "Int8",
        ),
        (
            "fn take_u8(value: UInt8): Void { }\n\
             fn negative(value: -1): Void { take_u8(value); }\n\
             fn main(): Void { }",
            "UInt8",
        ),
        (
            "fn take_u8(value: UInt8): Void { }\n\
             fn too_large(value: 256): Void { take_u8(value); }\n\
             fn main(): Void { }",
            "UInt8",
        ),
    ] {
        let output = rejected(source);
        assert!(
            output.contains(codes::TYPE_MISMATCH.as_str()),
            "singleton outside {expected} range must fail: {output}"
        );
    }
}

#[test]
fn union_normalization_absorbs_only_sound_numeric_supertypes() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class NumericChoices {\n\
             widened_integer: Int8 | Int32;\n\
             exactly_representable_float: Int32 | Float64;\n\
             not_exactly_representable_float: Int64 | Float64;\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let choices = checked
        .classes
        .iter()
        .find(|class| class.name == "NumericChoices")
        .unwrap();
    assert!(
        Type::of(Base::Int(zirk_sema::IntWidth::I32))
            .accepts(Type::of(Base::Int(zirk_sema::IntWidth::I8)))
    );

    let field = |name: &str| {
        choices
            .fields
            .iter()
            .find(|field| field.name == name)
            .unwrap()
            .ty
    };
    assert_eq!(
        field("widened_integer"),
        Type::of(Base::Int(zirk_sema::IntWidth::I32)),
        "unions: {:?}",
        checked.unions
    );
    assert_eq!(
        field("exactly_representable_float"),
        Type::of(Base::Float(zirk_sema::FloatWidth::F64))
    );
    assert!(matches!(
        field("not_exactly_representable_float").base,
        Base::Union(_)
    ));
    assert_eq!(checked.unions.len(), 1);
}

#[test]
fn numeric_union_normalization_matches_all_existing_lossless_assignability_pairs() {
    use zirk_sema::{FloatWidth, IntWidth};

    let numeric_types = [
        ("Int8", Type::of(Base::Int(IntWidth::I8))),
        ("Int16", Type::of(Base::Int(IntWidth::I16))),
        ("Int32", Type::of(Base::Int(IntWidth::I32))),
        ("Int64", Type::of(Base::Int(IntWidth::I64))),
        ("Int128", Type::of(Base::Int(IntWidth::I128))),
        ("UInt8", Type::of(Base::Int(IntWidth::U8))),
        ("UInt16", Type::of(Base::Int(IntWidth::U16))),
        ("UInt32", Type::of(Base::Int(IntWidth::U32))),
        ("UInt64", Type::of(Base::Int(IntWidth::U64))),
        ("UInt128", Type::of(Base::Int(IntWidth::U128))),
        ("Float16", Type::of(Base::Float(FloatWidth::F16))),
        ("Float32", Type::of(Base::Float(FloatWidth::F32))),
        ("Float64", Type::of(Base::Float(FloatWidth::F64))),
        ("Float128", Type::of(Base::Float(FloatWidth::F128))),
        ("Decimal", Type::of(Base::Decimal)),
    ];
    let mut source = String::from("class NumericMatrix {\n");
    let mut cases = Vec::new();
    for left in 0..numeric_types.len() {
        for right in (left + 1)..numeric_types.len() {
            let field = format!("pair_{left}_{right}");
            source.push_str(&format!(
                "{field}: {} | {};\n",
                numeric_types[left].0, numeric_types[right].0
            ));
            cases.push((field, left, right));
        }
    }
    source.push_str("}\nfn main(): Void { }");

    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("numeric-union-matrix.zrk", source));
    let entry = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(entry, &mut sink);
    let program = parse(entry, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let class = checked
        .classes
        .iter()
        .find(|class| class.name == "NumericMatrix")
        .unwrap();

    for (field_name, left, right) in cases {
        let actual = class
            .fields
            .iter()
            .find(|field| field.name == field_name)
            .unwrap()
            .ty;
        let (left_name, left_type) = numeric_types[left];
        let (right_name, right_type) = numeric_types[right];
        if left_type.accepts(right_type) {
            assert_eq!(actual, left_type, "{left_name} | {right_name}");
        } else if right_type.accepts(left_type) {
            assert_eq!(actual, right_type, "{left_name} | {right_name}");
        } else {
            let Base::Union(id) = actual.base else {
                panic!("{left_name} | {right_name} must remain a union, got {actual:?}");
            };
            let members = &checked.unions[id as usize];
            assert!(
                members.contains(&left_type.base),
                "{left_name} | {right_name}"
            );
            assert!(
                members.contains(&right_type.base),
                "{left_name} | {right_name}"
            );
        }
    }
}

#[test]
fn singleton_boolean_and_temporal_values_flow_into_union_nodes() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder {\n\
             flag: true | false;\n\
             one_flag: true;\n\
             covered_flag: Boolean | false;\n\
             day: Date(2026, 12, 25) | Date(2026, 12, 26);\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty.base, Base::Boolean);
    assert!(matches!(holder.fields[1].ty.base, Base::Singleton(_)));
    assert_eq!(holder.fields[2].ty.base, Base::Boolean);
    let Base::Union(days) = holder.fields[3].ty.base else {
        panic!("distinct Date constants should remain a finite union");
    };
    assert_eq!(checked.unions[days as usize].len(), 2);
    for base in &checked.unions[days as usize] {
        let Base::Singleton(id) = base else {
            panic!("each temporal union alternative must be a singleton");
        };
        assert!(matches!(
            checked.singleton_values[*id as usize].value,
            zirk_sema::constant_value::ConstValue::Date(_)
        ));
    }
}

#[test]
fn valid_union_with_null_is_a_plain_nullable_type() {
    // `String | Null` degrades to a plain nullable `String`, not a `Union`.
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder { a: String | Null; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert_eq!(checked.unions.len(), 0);
}

#[test]
fn valid_union_subsumes_a_subclass_alternative() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Animal { }
         class Dog extends Animal { }
         class Holder { a: Animal | Dog; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    // `Dog` is subsumed by `Animal`, so nothing needed a `Union` at all.
    assert_eq!(checked.unions.len(), 0);
}

#[test]
fn normalized_union_supertypes_preserve_bidirectional_assignability() {
    accepted(
        "class Animal { }\n\
         class Dog extends Animal { }\n\
         type Numeric = Int8 | Int32;\n\
         type Pet = Animal | Dog;\n\
         fn take_int(value: Int32): Void { }\n\
         fn take_numeric(value: Numeric): Void { }\n\
         fn take_animal(value: Animal): Void { }\n\
         fn take_pet(value: Pet): Void { }\n\
         fn check(number: Numeric, small: Int8, pet: Pet): Void {\n\
             take_int(number); take_numeric(small);\n\
             take_animal(pet); take_pet(Dog());\n\
         }\n\
         fn main(): Void { }",
    );
}

#[test]
fn union_reads_allow_common_structural_fields_and_join_their_types() {
    let (sources, sink) = parse_and_check(
        "class A { id: Int32; }
         class B { id: Int64; }
         fn f(x: A | B): Int32 | Int64 { return x.id; }
         fn main(): Void { }",
    );
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    // The present runtime prototype does not lower arbitrary nominal unions;
    // this test isolates the static common-field rule from that separate gate.
    let _ = sources;
}

#[test]
fn union_reads_still_reject_fields_missing_from_an_alternative() {
    let output = rejected(
        "class A { id: Int32; }
         class B { }
         fn f(x: A | B): Void { inmut value = x.id; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn union_method_calls_merge_safe_parameter_and_return_types() {
    let (sources, sink) = parse_and_check(
        "class Animal { }\n\
         class Dog extends Animal { }\n\
         class A { read(value: Animal): Int32 { return 1; } }\n\
         class B { read(value: Dog): Int64 { return 2; } }\n\
         fn read_any(value: A | B): Int32 | Int64 { return value.read(Dog()); }\n\
         fn main(): Void { }",
    );
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    let _ = sources;

    let incompatible_argument = rejected(
        "class A { read(value: Int32): Int32 { return 1; } }\n\
         class B { read(value: String): Int64 { return 2; } }\n\
         fn read_any(value: A | B): Void { value.read(1); }\n\
         fn main(): Void { }",
    );
    assert!(
        incompatible_argument.contains(codes::TYPE_MISMATCH.as_str()),
        "the common call must be valid for every receiver alternative: {incompatible_argument}"
    );

    let missing = rejected(
        "class A { read(): Int32 { return 1; } }\n\
         class B { }\n\
         fn read_any(value: A | B): Void { value.read(); }\n\
         fn main(): Void { }",
    );
    assert!(
        missing.contains(codes::UNKNOWN_MEMBER.as_str()),
        "{missing}"
    );

    let callable_properties = "type ReadsInt = { read: Fn(Int32) => Int32 };\n\
         type ReadsWide = { read: Fn(Int32) => Int64 };\n\
         fn invoke(value: ReadsInt | ReadsWide): Int32 | Int64 { return value.read(3); }\n\
         fn main(): Void { }";
    let (_, callable_sink) = parse_and_check(callable_properties);
    let callable_output = callable_sink.render(RenderStyle::Human);
    assert!(
        !callable_output.contains(codes::UNKNOWN_MEMBER.as_str()),
        "common function-valued properties should be callable: {callable_output}"
    );
    assert!(
        !callable_output.contains(codes::TYPE_MISMATCH.as_str()),
        "{callable_output}"
    );

    let generic_methods = "class A { read<T>(value: T): T { return value; } }\n\
         class B { read<U>(value: U): U { return value; } }\n\
         fn invoke(value: A | B): Int32 { return value.read(3); }\n\
         fn main(): Void { }";
    let (_, generic_sink) = parse_and_check(generic_methods);
    let generic_output = generic_sink.render(RenderStyle::Human);
    assert!(
        !generic_output.contains(codes::UNKNOWN_MEMBER.as_str()),
        "alpha-equivalent generic methods should share a callable signature: {generic_output}"
    );
    assert!(
        !generic_output.contains(codes::TYPE_MISMATCH.as_str()),
        "{generic_output}"
    );

    let incompatible_generic_bounds = rejected(
        "contract Left { } contract Right { }\n\
         class A { read<T from Left>(value: T): T { return value; } }\n\
         class B { read<U from Right>(value: U): U { return value; } }\n\
         fn invoke(value: A | B): Void { value.read(3); }\n\
         fn main(): Void { }",
    );
    assert!(
        incompatible_generic_bounds.contains("incompatible generic method"),
        "different generic bounds cannot be merged: {incompatible_generic_bounds}"
    );
}

#[test]
fn union_writes_allow_only_a_shared_required_mutable_field() {
    let (_, sink) = parse_and_check(
        "class A { id: Int32; }\n\
         class B { id: Int32; }\n\
         fn update(value: A | B): Void { value.id = 3; }\n\
         fn main(): Void { }",
    );
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn union_writes_reject_absent_or_conflicting_common_fields() {
    let output = rejected(
        "class A { id: Int32; } class B { } fn update(value: A | B): Void { value.id = 3; } fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(output.contains("safely writable common field"), "{output}");

    let conflicting = rejected(
        "class A { id: Int32; } class B { id: String; } fn update(value: A | B): Void { value.id = 3; } fn main(): Void { }",
    );
    assert!(
        conflicting.contains(codes::TYPE_MISMATCH.as_str()),
        "{conflicting}"
    );
    assert!(
        !conflicting.contains(codes::UNKNOWN_MEMBER.as_str()),
        "a shared writable field remains addressable, but no value fits both unrelated types: {conflicting}"
    );

    let readonly = rejected(
        "class A { id: Int32; } class B { inmut id: Int32; } fn update(value: A | B): Void { value.id = 3; } fn main(): Void { }",
    );
    assert!(
        readonly.contains(codes::UNKNOWN_MEMBER.as_str()),
        "{readonly}"
    );
}

#[test]
fn union_writes_accept_values_compatible_with_every_destination() {
    let (sources, sink) = parse_and_check(
        "class Animal { }\n\
         class Dog extends Animal { }\n\
         class A { value: Animal; }\n\
         class B { value: Dog; }\n\
         fn update(target: A | B): Void { target.value = Dog(); }\n\
         fn main(): Void { }",
    );
    let output = sink.render(RenderStyle::Human);
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    let _ = sources;

    let unsafe_value = rejected(
        "class Animal { } class Dog extends Animal { } class A { value: Animal; } class B { value: Dog; } fn update(target: A | B): Void { target.value = Animal(); } fn main(): Void { }",
    );
    assert!(
        unsafe_value.contains(codes::TYPE_MISMATCH.as_str()),
        "{unsafe_value}"
    );
}

#[test]
fn named_contracts_and_structural_shapes_share_property_relations() {
    let source = "type Shape = { inmut id: String };\n\
         type ShapeAlias = Shape;\n\
         contract Named { inmut id: String; }\n\
         type Combined = Shape & Named;\n\
         fn needs_shape(value: Shape): Void { }\n\
         fn needs_contract(value: Named): Void { }\
         fn exercise(shape: ShapeAlias, named: Named, combined: Combined): Void {\n\
             needs_contract(shape);\n\
             needs_shape(named);\n\
             needs_contract(combined);\n\
             needs_shape(combined);\n\
         }\n\
         fn main(): Void { }";
    let (_sources, sink) = parse_and_check(source);
    let output = sink.render(RenderStyle::Human);
    assert!(
        sink.diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code == codes::NOT_LOWERED),
        "shape/contract property rules should agree through aliases and intersections: {output}"
    );
}

#[test]
fn structural_shape_intersections_compose_property_capabilities_and_reject_conflicts() {
    accepted(
        "type OptionalId = { id?: Int32; name: String };\n\
         type ReadonlyId = { inmut id: Int32; active: Boolean };\n\
         type Combined = OptionalId & ReadonlyId;\n\
         fn read(value: Combined): Int32 { return value.id; }\n\
         fn main(): Void { }",
    );

    let readonly_write = rejected(
        "type Mutable = { id: Int32 }; type Frozen = { inmut id: Int32 }; type Combined = Mutable & Frozen; fn update(value: Combined): Void { value.id = 7; } fn main(): Void { }",
    );
    assert!(
        readonly_write.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "a read-only constituent must make the combined view read-only: {readonly_write}"
    );

    let conflict = rejected(
        "type Text = { value: String }; type Number = { value: Int32 }; type Conflict = Text & Number; fn read(value: Conflict): Void { } fn main(): Void { }",
    );
    assert!(
        conflict.contains("structural intersection property `value` has conflicting types"),
        "conflicting property requirements need a declaration-level diagnostic: {conflict}"
    );
}

#[test]
fn nominal_union_types_are_supported() {
    accepted(
        "class Left { } class Right { }\n\
         fn f(x: Left | Right): Void { }\n\
         fn main(): Void { }",
    );
}

#[test]
fn invalid_intersection_type_is_not_lowered_yet() {
    let output = rejected(
        "class Readable { }\n\
         class Closeable { }\n\
         fn f(x: Readable & Closeable): Void { }\n\
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
    assert!(output.contains("intersection type"), "{output}");
}

#[test]
fn intersections_reduce_only_proven_primitive_and_singleton_contradictions() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder {\n\
             primitive: Int32 & String;\n\
             literals: 3 & 4;\n\
             numeric: Int32 & Int64;\n\
             compatible: String & \"ready\";\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, Type::of(Base::Never));
    assert_eq!(holder.fields[1].ty, Type::of(Base::Never));
    assert!(matches!(holder.fields[2].ty.base, Base::Intersection(_)));
    assert!(matches!(holder.fields[3].ty.base, Base::Singleton(_)));
}

#[test]
fn intersection_members_are_sorted_deduplicated_and_interned() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Readable { }\n\
         class Closeable { }\n\
         class Taggable { }\n\
         type Both = Readable & Closeable;\n\
         class Holder { a: Readable & Closeable; b: Closeable & Readable; c: Readable & Readable; d: Both; e: Both & Taggable; f: Taggable & Closeable & Readable; }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, holder.fields[1].ty);
    assert!(matches!(holder.fields[0].ty.base, Base::Intersection(_)));
    assert!(checked.describe_type(holder.fields[0].ty).contains(" & "));
    assert_eq!(checked.intersections.len(), 2);
    assert_eq!(checked.intersections[0].len(), 2);
    assert_eq!(
        holder.fields[2].ty,
        Type::of(Base::Class(
            checked
                .classes
                .iter()
                .position(|class| class.name == "Readable")
                .unwrap() as u32
        ))
    );
    assert_eq!(holder.fields[0].ty, holder.fields[3].ty);
    assert_eq!(holder.fields[4].ty, holder.fields[5].ty);
    let Base::Intersection(flat_id) = holder.fields[4].ty.base else {
        panic!("nested intersections should flatten to an interned intersection");
    };
    assert_eq!(checked.intersections[flat_id as usize].len(), 3);
    assert_eq!(checked.intersections.len(), 2);
}

#[test]
fn anonymous_shapes_preserve_properties_and_intern_independent_of_source_order() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder {\n\
             first: { inmut name: String, age?: Int32 };\n\
             second: { age?: Int32, inmut name: String };\n\
         }\n\
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let holder = checked
        .classes
        .iter()
        .find(|class| class.name == "Holder")
        .unwrap();
    assert_eq!(holder.fields[0].ty, holder.fields[1].ty);
    let Base::Shape(shape_id) = holder.fields[0].ty.base else {
        panic!("expected an interned structural shape");
    };
    assert_eq!(checked.structural_types.len(), 1);
    let shape = &checked.structural_types[shape_id as usize];
    assert_eq!(shape.properties.len(), 2);
    assert_eq!(shape.properties[0].name, "age");
    assert!(shape.properties[0].optional);
    assert!(!shape.properties[0].readonly);
    assert_eq!(shape.properties[1].name, "name");
    assert!(shape.properties[1].readonly);
    assert!(
        checked
            .describe_type(holder.fields[0].ty)
            .contains("inmut name")
    );
}

#[test]
fn scalar_and_structural_type_expressions_lower() {
    accepted(
        "fn flag(value: true | false): Void { }\n\
         fn date(value: Date(2026, 12, 25)): Void { }\n\
         fn main(): Void { flag(true); date(Date(2026, 12, 25)); }",
    );
    accepted(
        "fn f(x: { inmut id: String; name?: String }): Void { }\n\
         fn main(): Void { }",
    );
}

#[test]
fn computed_expressions_are_rejected_in_singleton_type_position() {
    let (_sources, sink) = parse_and_check(
        "type Computed = 2147483647 + 1;\nfn f(value: Computed): Void { }\nfn main(): Void { }",
    );
    let diagnostic = sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == zirk_parser::codes::UNEXPECTED_TOKEN)
        .expect("computed expressions are not singleton type atoms");
    assert!(diagnostic.message.contains("expected"));

    let (_sources, sink) = parse_and_check(
        "type InvalidTime = 25:00;\nfn f(value: InvalidTime): Void { }\nfn main(): Void { }",
    );
    let diagnostic = sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == zirk_parser::codes::UNEXPECTED_TOKEN)
        .expect("bare clock literals are rejected in type position");
    let location = diagnostic
        .location
        .as_ref()
        .expect("time diagnostic is source located");
    assert_eq!(location.file, "test.zrk");
    assert_eq!(location.line, 1);
}

#[test]
fn call_expressions_are_rejected_in_singleton_type_position() {
    let (_sources, sink) = parse_and_check(
        "type FromEnvironment = Env.get(\"PATH\");\nfn f(value: FromEnvironment): Void { }\nfn main(): Void { }",
    );
    let diagnostic = sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == zirk_parser::codes::UNEXPECTED_TOKEN)
        .expect("function calls are rejected as type atoms");
    assert!(diagnostic.message.contains("call"));
    let location = diagnostic.location.as_ref().expect("source location");
    assert_eq!(location.file, "test.zrk");
    assert_eq!(location.line, 1);
}

#[test]
fn direct_literal_types_need_no_constant_function_calls() {
    let (_sources, sink) =
        parse_and_check("type Three = 3;\nfn f(value: Three): Void { }\nfn main(): Void { }");
    assert!(
        !sink
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == codes::INVALID_CONSTANT_TYPE),
        "a direct singleton literal should resolve, diagnostics: {}",
        sink.render(RenderStyle::Human)
    );

    let (_sources, sink) =
        parse_and_check("type Six = 6;\nfn f(value: Six): Void { }\nfn main(): Void { }");
    assert!(
        !sink
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == codes::INVALID_CONSTANT_TYPE),
        "direct numeric singleton aliases should resolve: {}",
        sink.render(RenderStyle::Human)
    );
}

#[test]
fn ordinary_function_calls_are_rejected_in_types_and_const_functions_still_check_effects() {
    let (_sources, sink) = parse_and_check(
        "fn ordinary(value: Int32): Int32 { return value; }\ntype A = ordinary(1);\nfn f(value: A): Void { }\nfn main(): Void { }",
    );
    let invalid: Vec<_> = sink
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == zirk_parser::codes::UNEXPECTED_TOKEN)
        .collect();
    assert_eq!(invalid.len(), 1, "{}", sink.render(RenderStyle::Human));
    assert!(invalid[0].message.contains("call"));

    let (_sources, sink) = parse_and_check(
        "const fn unused_impure(): String { return Env.get(\"PATH\"); }\nfn main(): Void { }",
    );
    assert!(
        sink.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == codes::INVALID_CONSTANT_TYPE
                && diagnostic.message.contains("environment access")
        }),
        "an unused const function must still be effect-checked: {}",
        sink.render(RenderStyle::Human)
    );
}

#[test]
fn direct_numeric_type_literals_preserve_exact_values() {
    let (_sources, sink) = parse_and_check(
        "type Quotient = 0.75;\ntype Mixed = 1.5;\ntype Repeating = 0.3333333333333333333333333333333333;\nfn f(a: Quotient, b: Mixed, c: Repeating): Void { }\nfn main(): Void { }",
    );
    assert!(
        !sink
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == codes::INVALID_CONSTANT_TYPE),
        "direct fractional literals must resolve without invalid-value diagnostics: {}",
        sink.render(RenderStyle::Human)
    );

    let (_sources, sink) = parse_and_check(
        "type DivideByZero = 3 / 0;\nfn f(a: DivideByZero): Void { }\nfn main(): Void { }",
    );
    assert!(
        sink.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == zirk_parser::codes::UNEXPECTED_TOKEN),
        "arithmetic must be rejected before type evaluation: {}",
        sink.render(RenderStyle::Human)
    );
}

#[test]
fn decimal_singletons_normalize_scale_without_binary_float_conversion() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "decimal-singletons.zrk",
        "class Amounts {\n\
             one_decimal: 1.0;\n\
             one_with_extra_scale: 1.00;\n\
             nearby: 1.01;\n\
             zero_scaled: 0.000;\n\
             zero_plain: 0.0;\n\
         }\n\
         fn main(): Void { }",
    ));
    let entry = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(entry, &mut sink);
    let program = parse(entry, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let amounts = checked
        .classes
        .iter()
        .find(|class| class.name == "Amounts")
        .unwrap();
    assert_eq!(amounts.fields[0].ty, amounts.fields[1].ty);
    assert_ne!(amounts.fields[0].ty, amounts.fields[2].ty);
    assert_eq!(amounts.fields[3].ty, amounts.fields[4].ty);
    assert_eq!(checked.singleton_values.len(), 3);
    assert!(checked.singleton_values.iter().all(|value| {
        matches!(
            value.value,
            zirk_sema::constant_value::ConstValue::Decimal { .. }
        )
    }));
}

#[test]
fn float_singletons_allow_exact_widening_but_not_narrowing() {
    let (_sources, sink) = parse_and_check(
        "fn take_f32(value: Float32): Void { }\n\
         fn take_f64(value: Float64): Void { }\n\
         fn take_f128(value: Float128): Void { }\n\
         fn f16(value: 1.5f16): Void { take_f16(value); take_f32(value); take_f64(value); take_f128(value); }\n\
         fn take_f16(value: Float16): Void { }\n\
         fn f32(value: 1.5f32): Void { take_f64(value); take_f128(value); }\n\
         fn f64(value: 1.5f64): Void { take_f128(value); }\n\
         fn main(): Void { }",
    );
    assert!(
        !sink
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == codes::TYPE_MISMATCH),
        "finite Float singletons should widen exactly to larger binary formats: {}",
        sink.render(RenderStyle::Human)
    );

    for (literal, target) in [("1.5f32", "Float16"), ("1.5f64", "Float32")] {
        let source = format!(
            "fn take(value: {target}): Void {{ }}\n\
             fn f(singleton: {literal}): Void {{ take(singleton); }}\n\
             fn main(): Void {{ }}"
        );
        let (_sources, sink) = parse_and_check(&source);
        assert!(
            sink.diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == codes::TYPE_MISMATCH),
            "{literal} must not implicitly narrow to {target}: {}",
            sink.render(RenderStyle::Human)
        );
    }
}

#[test]
fn direct_uint128_singletons_cover_the_full_unsigned_range() {
    let source = "type HighBit = 170141183460469231731687303715884105728u128;\ntype Max = 340282366920938463463374607431768211455u128;\nfn f(high: HighBit, max: Max): Void { }\nfn main(): Void { }";
    let (_sources, sink) = parse_and_check(source);
    assert!(
        !sink
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == codes::INVALID_CONSTANT_TYPE),
        "direct UInt128 literal types should resolve: {}",
        sink.render(RenderStyle::Human)
    );
}

#[test]
fn computed_type_expressions_are_rejected_at_the_source() {
    let (_sources, sink) = parse_and_check(
        "type Computed = 1 + 1;\nfn f(value: Computed): Void { }\nfn main(): Void { }",
    );
    let diagnostic = sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == zirk_parser::codes::UNEXPECTED_TOKEN)
        .expect("computed expressions are not singleton type atoms");
    let location = diagnostic
        .location
        .as_ref()
        .expect("diagnostic source location");
    assert_eq!(location.file, "test.zrk");
    assert_eq!(location.line, 1);
}

#[test]
fn invalid_constant_temporal_constructor_reports_its_location() {
    let (_sources, sink) = parse_and_check(
        "type Holiday = Date(2024, 2, 30);\nfn f(value: Holiday): Void { }\nfn main(): Void { }",
    );
    let diagnostic = sink
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::INVALID_CONSTANT_TYPE)
        .expect("invalid calendar date has a dedicated constant diagnostic");
    let location = diagnostic
        .location
        .as_ref()
        .expect("diagnostic is source located");
    assert_eq!(location.file, "test.zrk");
    assert_eq!(location.line, 1);
}

#[test]
fn temporal_constructor_type_values_share_identity_with_runtime_values() {
    let (_sources, sink) = parse_and_check(
        "type Holiday = Date(2026, 12, 25) | Date(2027, 1, 1);\n\
         type OfficeTime = Time(9, 30) | Time(17, 0);\n\
         type Meeting = DateTime(Date(2026, 12, 25), Time(9, 30)) | DateTime(Date(2027, 1, 1), Time(0, 0));\n\
         fn take_day(value: Holiday): Void { }\n\
         fn take_time(value: OfficeTime): Void { }\n\
         fn take_meeting(value: Meeting): Void { }\n\
         fn main(): Void {\n\
             take_day(Date(2026, 12, 25));\n\
             take_day(Date(2027, 1, 1));\n\
             take_time(Time(9, 30));\n\
             take_time(Time(17, 0));\n\
             take_meeting(DateTime(Date(2026, 12, 25), Time(9, 30)));\n\
             take_meeting(DateTime(Date(2027, 1, 1), Time(0, 0)));\n\
         }",
    );
    assert!(
        !sink.has_errors(),
        "temporal constructor alternatives must match runtime temporal values: {}",
        sink.render(RenderStyle::Human)
    );
}

// --- Casts -----------------------------------------------------------------

#[test]
fn valid_cast_between_a_class_and_its_base() {
    // A class-to-class cast is checked at runtime now (roadmap task 11.6),
    // so this compiles clean rather than merely avoiding `TYPE_MISMATCH`.
    accepted(
        "class Animal { }
         class Dog extends Animal { construct() { } }
         fn main(): Void { mut d = Dog(); mut a = d as Animal; mut b = <Dog>a; }",
    );
}

#[test]
fn valid_cast_between_a_class_and_a_contract() {
    let output = rejected(
        "contract Shape {  area(): Int32; }
         class Circle implements Shape { construct() { }  area(): Int32 { return 1; } }
         fn main(): Void { mut c = Circle(); mut s = c as Shape; mut back = <Circle>s; }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_cast_from_a_union_member() {
    let output = rejected(
        "fn f(x: String | Int32): Void { mut s = x as String; }
         fn main(): Void { }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_identity_cast() {
    accepted("fn main(): Void { mut x = 1 as Int32; }");
}

#[test]
fn invalid_cast_between_unrelated_types() {
    let output = rejected("fn main(): Void { mut x = \"a\" as Int32; }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_cast_between_unrelated_classes() {
    let output = rejected(
        "class A { construct() { } }
         class B { construct() { } }
         fn main(): Void { mut a = A(); mut b = a as B; }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_nullable_cast_is_not_lowered_yet() {
    // A class-to-class cast lowers now (task 11.6), but only once nullability
    // is out of the way: deciding what a null value means for the check is a
    // separate question this pass does not answer yet.
    let output = rejected(
        "class Animal { }
         class Dog extends Animal { construct() { } }
         fn main(): Void { mut a: Animal? = Dog(); mut d = a as Dog?; }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn valid_cast_from_a_type_parameter_to_its_own_constraint() {
    // `T from Serializable` may always be widened to `Serializable`: that is
    // exactly what the constraint promises, not a leap of faith.
    let output = rejected(
        "contract Serializable {  serialize(): String; }
         fn show<T from Serializable>(x: T): Void { mut y = x as Serializable; }
         fn main(): Void { }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_cast_from_a_type_parameter_to_an_unrelated_type() {
    let output = rejected(
        "contract Serializable {  serialize(): String; }
         fn show<T from Serializable>(x: T): Void { mut y = x as String; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

// --- `?.` --------------------------------------------------------------------

#[test]
fn valid_safe_field_access_types_as_nullable() {
    // The type rules are real (D7): the member's own type is `String`, but
    // reading it through `?.` makes the result `String?`. Field access through
    // `?.` is fully lowered (task 10.8), so this is accepted outright.
    accepted(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void { mut u: User? = null; mut n: String? = u?.name; }",
    );
}

#[test]
fn valid_safe_method_call_types_as_nullable() {
    accepted(
        "class User {  greet(): String { return \"hi\"; } construct() { } }
         fn main(): Void { mut u: User? = null; mut g: String? = u?.greet(); }",
    );
}

#[test]
fn invalid_plain_access_through_a_nullable_receiver_still_rejected() {
    // `?.` is what makes this legal; plain `.` still is not.
    let output = rejected(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void { mut u: User? = null; mut n = u.name; }",
    );
    assert!(output.contains("may be absent"), "{output}");
}

#[test]
fn valid_safe_method_call_lowers() {
    // Both halves of task 10.8 lower now: a field through `?.` and a method
    // call through it, dispatched under the same absent/present split.
    accepted(
        "class User {  greet(): String { return \"hi\"; } construct() { } }
         fn main(): Void { mut u: User? = null; mut g = u?.greet(); }",
    );
}

#[test]
fn invalid_safe_method_call_through_generic_param_is_not_lowered() {
    // Through a generic parameter's constraint there is no concrete method
    // body to call yet, so that combination stays gated.
    let output = rejected(
        "trait Greeter {  greet(): String; }
         class Wrap<T from Greeter> { value: T?; construct(value: T?) { this.value = value; }  hello(): String? { return this.value?.greet(); } }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

// --- Sin shadowing ordinario (D10) --------------------------------------------

#[test]
fn invalid_lambda_parameter_shadows_a_captured_variable() {
    let output = rejected(
        "fn main(): Void {
             inmut outer = 5;
             inmut g = (outer: Int32): Int32 => outer * 2;
             stdout.println(g(3));
         }",
    );
    assert!(
        output.contains(codes::ORDINARY_SHADOWING.as_str()),
        "{output}"
    );
}

#[test]
fn valid_lambda_parameter_with_a_distinct_name_still_captures() {
    accepted(
        "fn main(): Void {
             inmut outer = 5;
             inmut g = (factor: Int32): Int32 => outer * factor;
             stdout.println(g(3));
         }",
    );
}

#[test]
fn valid_capturing_lambda_passed_directly_to_a_function_parameter() {
    accepted(
        "fn invoke(action: Fn() => Int32): Int32 { return action(); }
         fn main(): Void {
             inmut captured = 42;
             inmut value = invoke((): Int32 => captured);
             stdout.println(value);
         }",
    );
}

#[test]
fn valid_lambda_parameter_matches_a_field_name() {
    // A field is never a bare name `Scopes` resolves — `this.name` is how
    // one is always read — so there is nothing for a same-named parameter
    // to shadow.
    accepted(
        "class Multiplier {
             factor: Int32;
             construct(factor: Int32) { this.factor = factor; }
              scale(factor: Int32): Int32 { return factor * this.factor; }
         }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_for_in_binding_shadows_an_outer_variable() {
    let output = rejected(
        "fn main(): Void {
             inmut i = 5;
             for i in 0..3 { stdout.println(i); }
         }",
    );
    assert!(
        output.contains(codes::ORDINARY_SHADOWING.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_binding_shadows_an_outer_variable() {
    let output = rejected(
        "fn main(): Void {
             inmut n = 5;
             mut x: Int32? = 1;
             match x { n => { stdout.println(n); } null => { } }
         }",
    );
    assert!(
        output.contains(codes::ORDINARY_SHADOWING.as_str()),
        "{output}"
    );
}

// --- Closures no anotables ni escapables (D9) ---------------------------------

#[test]
fn valid_closure_stored_in_an_inferred_local_and_called() {
    accepted(
        "fn main(): Void { mut add = (a: Int32, b: Int32): Int32 => a + b; stdout.println(add(1, 2)); }",
    );
}

#[test]
fn valid_closure_returned_from_a_function() {
    // `Fn(...) => R` is a real, writable return type (roadmap Phase 4d): a
    // capture-less closure literal — uniformly represented the same way a
    // named function is (design D12) — satisfies it like any other
    // structurally compatible value. `invalid_closure_returned_from_a_function`
    // used to stand in for this with an unrelated class purely because D9
    // made the annotation itself impossible to write at all.
    accepted(
        "fn make(): Fn() => Void { return (): Void => { }; }
         fn main(): Void { }",
    );
}

#[test]
fn callable_assignability_uses_contravariant_inputs_and_covariant_results() {
    accepted(
        "fn wide(value: Int32): Float32 { return value as Float32; }\n\
         fn main(): Void { mut callback: Fn(Int8) => Float64 = wide; }",
    );

    let narrow_input = rejected(
        "fn narrow(value: Int8): Float64 { return 1.0f64; }\n\
         fn main(): Void { mut callback: Fn(Int32) => Float64 = narrow; }",
    );
    assert!(
        narrow_input.contains(codes::TYPE_MISMATCH.as_str()),
        "a callable may not require narrower inputs: {narrow_input}"
    );

    let incompatible_result = rejected(
        "fn text(value: Int32): String { return \"x\"; }\n\
         fn main(): Void { mut callback: Fn(Int8) => Float64 = text; }",
    );
    assert!(
        incompatible_result.contains(codes::TYPE_MISMATCH.as_str()),
        "a callable result must fit the target result type: {incompatible_result}"
    );
}

#[test]
fn enum_assignability_preserves_declared_enum_identity() {
    let output = rejected(
        "enum Color { Red }\n\
         enum Mood { Red }\n\
         fn paint(color: Color): Void { }\n\
         fn main(): Void { paint(Mood.Red); }",
    );
    assert!(
        output.contains(codes::TYPE_MISMATCH.as_str()),
        "variants with the same spelling from distinct enums are incompatible: {output}"
    );
}

#[test]
fn invalid_class_returned_where_a_closure_is_expected() {
    // The class-vs-closure mismatch the old test actually exercised is still
    // rejected — replacing the class with a real class value shows the
    // mismatch is about the *types*, not about `Fn(...) => R` being
    // unwritable.
    let output = rejected(
        "class NotAClosure { }
         fn make(): Fn() => Void { return NotAClosure(); }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_generic_inference_from_a_closure_argument() {
    // A closure argument infers a generic parameter `T` the same way any
    // other value does (roadmap Phase 4d): the checker no longer refuses to
    // name `Base::Function` as `T`'s solution — that refusal
    // (`invalid_generic_inference_from_a_closure_argument`) existed only
    // because D9 made a closure's type unwritable anywhere, generic
    // arguments included. The instantiated function accepts the capture-less
    // closure ABI and is lowered as a concrete function.
    accepted(
        "fn store<T>(value: T): Void { }
         fn main(): Void { store((): Void => { }); }",
    );
}

#[test]
fn invalid_two_differently_captured_closures_in_one_return_type() {
    // Design D14: a `Fn(...) => R` return type only accepts a *single*
    // capturing closure literal — two different `return`s, each with its
    // own captures, would need the captures boxed behind a uniform
    // representation this pass does not build (design D13). Rejected with a
    // dedicated diagnostic, not a generic `TYPE_MISMATCH`.
    let output = rejected(
        "fn make(pick: Boolean): Fn() => Int32 {
             mut a = 1;
             mut b = 2;
             if pick {
                 return (): Int32 => a;
             }
             return (): Int32 => b;
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::AMBIGUOUS_CAPTURING_CALLABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_recursive_lambda_binding_used_as_a_value() {
    // A recursive lambda's own binding (`ZIRK_LANGUAGE_SPEC.md` section 6)
    // refers to itself only through a direct call inside its own body:
    // `zirk-ir` rewrites that one shape into an ordinary recursive call,
    // since no closure value for it exists yet at the point the literal is
    // still being built. Reading it into a variable — or any other use
    // besides a direct call — has no value to give and must be rejected
    // here, not reach a panic during lowering.
    let output = rejected(
        "fn main(): Void {
             mut fact: Fn(Int32) => Int32 = (n: Int32): Int32 => {
                 mut self_ref = fact;
                 return n <= 1 ? 1 : n * fact(n - 1);
             };
             stdout.println(fact(5));
         }",
    );
    assert!(
        output.contains(codes::RECURSIVE_BINDING_NOT_A_VALUE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_recursive_lambda_calling_itself_only_directly() {
    // The one shape a recursive lambda's own binding may appear in: the
    // direct callee of a call, anywhere inside its own body (including
    // nested inside a ternary/if), never through a nested lambda.
    accepted(
        "fn main(): Void {
             mut fact: Fn(Int32) => Int32 = (n: Int32): Int32 => n <= 1 ? 1 : n * fact(n - 1);
             stdout.println(fact(5));
         }",
    );
}

#[test]
fn invalid_recursive_lambda_binding_used_from_a_nested_lambda() {
    // Even in call position, a reference reached from inside a lambda
    // nested within the recursive one is an ordinary (but unimplemented)
    // capture-of-a-capture, not a self-call — `zirk-ir`'s rewrite only
    // covers the recursive lambda's own immediate body.
    let output = rejected(
        "fn main(): Void {
             mut fact: Fn(Int32) => Int32 = (n: Int32): Int32 => {
                 mut helper = (): Int32 => fact(1);
                 return n <= 1 ? 1 : n * fact(n - 1);
             };
             stdout.println(fact(5));
         }",
    );
    assert!(
        output.contains(codes::RECURSIVE_BINDING_NOT_A_VALUE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_generic_class_construction_and_member_access() {
    // A generic class's construction, annotation and member access all type
    // through a real instantiation (`Base::Instance`), not the bare class:
    // `check_construction` used to build the constructor's `Signature` with
    // no type parameters at all, so `Box(5)` never inferred `T` and this
    // failed with `TYPE_MISMATCH`. `Box<T>` is also simple enough to fully
    // specialize (roadmap task 11.1), so the whole program compiles clean.
    accepted(
        "class Box<T> {
             value: T;
             construct(value: T) { this.value = value; }
              get(): T { return this.value; }
         }
         fn main(): Void {
             mut b: Box<Int32> = Box(5);
             stdout.println(b.get());
         }",
    );
}

// --- Abstract classes ----------------------------------------------------------

const SHAPE: &str = "abstract class Shape {
    name: String;
    abstract  area(): Int32;
}";

#[test]
fn valid_class_adopts_an_abstract_class() {
    // Naming the abstract class as a type is itself accepted now
    // (fase-3-abstract-dispatch), so a fully-conforming adopter with no
    // other use of `Shape` produces no diagnostic at all.
    accepted(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             #override area(): Int32 {{ return 3; }}
         }}
         fn main(): Void {{ }}"
    ));
}

#[test]
fn invalid_abstract_class_missing_attribute() {
    let output = rejected(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             construct() {{ }}
             #override area(): Int32 {{ return 3; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_abstract_class_missing_method() {
    let output = rejected(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
}

#[test]
fn valid_abstract_class_method_needs_no_override_marker() {
    // Implementing an abstract-class requirement replaces nothing concrete,
    // so `#override` is unnecessary — only a warning when written.
    accepted(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
              area(): Int32 {{ return 3; }}
         }}
         fn main(): Void {{ }}"
    ));
}

#[test]
fn unnecessary_override_marker_on_an_abstract_requirement_warns() {
    let output = accepted_with_warnings(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             #override
              area(): Int32 {{ return 3; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        output.contains(codes::UNNECESSARY_OVERRIDE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_abstract_class_method_with_mismatched_signature() {
    let output = rejected(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             #override area(): String {{ return \"x\"; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_abstract_class_extends_a_class() {
    let output = rejected(
        "class Base { }
         abstract class Shape extends Base { }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_abstract_class_declares_a_construct() {
    let output = rejected(
        "abstract class Shape { construct() { } }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_abstract_class_alone_is_lowered() {
    // Naming and declaring an `abstract class` no longer trips
    // `NOT_LOWERED` on its own (fase-3-abstract-dispatch): only its own
    // `construct`/state/layout stayed rejected, not the declaration itself.
    accepted(&format!("{SHAPE}\nfn main(): Void {{ }}"));
}

#[test]
fn valid_abstract_class_named_as_a_parameter_type_dispatches() {
    // The specific gap fase-3-abstract-dispatch closes: a value statically
    // typed through the abstract class, not just structurally conforming to
    // it, is now accepted and lowered.
    accepted(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             #override area(): Int32 {{ return 3; }}
         }}
         fn describe(s: Shape): Int32 {{ return s.area(); }}
         fn main(): Void {{ describe(Circle(\"c\")); }}"
    ));
}

#[test]
fn valid_two_adopters_of_the_same_abstract_class_get_consistent_indices() {
    // design.md D1/D3: every concrete adopter of the same abstract class
    // must assign that class's methods the same index in its own flattened
    // method list, or `CallVirtual`'s shared index would read the wrong
    // slot depending on which adopter backs the value.
    accepted(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             #override area(): Int32 {{ return 3; }}
         }}
         class Square implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             #override area(): Int32 {{ return 4; }}
         }}
         fn describe(s: Shape): Int32 {{ return s.area(); }}
         fn main(): Void {{
             describe(Circle(\"c\"));
             describe(Square(\"s\"));
         }}"
    ));
}

#[test]
fn valid_abstract_class_implemented_before_its_own_declaration() {
    // `Self::in_hierarchy_order` has to place the abstract class before any
    // adopter that implements it, even when the adopter is declared earlier
    // in the file — otherwise the adopter would seed its method list from
    // the abstract class's still-empty, pre-declaration default.
    accepted(
        "class Circle implements Shape {
             name: String;
             construct(name: String) { this.name = name; }
             #override

               area(): Int32 { return 3; }
         }
         abstract class Shape {
             name: String;
             abstract  area(): Int32;
         }
         fn describe(s: Shape): Int32 { return s.area(); }
         fn main(): Void { describe(Circle(\"c\")); }",
    );
}

#[test]
fn valid_class_implements_an_abstract_class_and_an_interface() {
    // design.md's Risk / tasks.md 1.3: an abstract class and a plain
    // interface contributing to the same adopter's flattened method list
    // must not collide on index assignment.
    accepted(&format!(
        "{SHAPE}
         contract Describable {{
              describe(): String;
         }}
         class Circle implements Shape, Describable {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             #override area(): Int32 {{ return 3; }}
              describe(): String {{ return this.name; }}
         }}
         fn print_area(s: Shape): Int32 {{ return s.area(); }}
         fn print_description(d: Describable): String {{ return d.describe(); }}
         fn main(): Void {{
             mut c = Circle(\"c\");
             print_area(c);
             print_description(c);
         }}"
    ));
}

// --- `throw`/`try`/`catch`/`finally` (roadmap Phase 4b) ----------------------

const BOOM_ERROR: &str = "class BoomError implements RuntimeError {
    construct() { }
    #override

      message(): String { return \"boom\"; }
    #override code(): String { return \"BOOM\"; }
    #override cause(): Error? { return null; }
    #override stack_trace(): StackTrace { return StackTrace(); }
}";

#[test]
fn valid_throw_and_catch_by_concrete_type() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             try {{
                 throw BoomError();
             }} catch BoomError(e) {{
                 stdout.println(e.message());
             }}
         }}"
    ));
}

#[test]
fn valid_catch_by_throwable_supertype() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             try {{
                 throw BoomError();
             }} catch Throwable(e) {{
                 stdout.println(e.message());
             }}
         }}"
    ));
}

#[test]
fn valid_rethrow_inside_catch() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn wrapper(): Void throws BoomError {{
             try {{
                 throw BoomError();
             }} catch BoomError(e) {{
                 throw;
             }}
         }}
         fn main(): Void {{
             try {{
                 wrapper();
             }} catch Throwable(e) {{
                 stdout.println(e.message());
             }}
         }}"
    ));
}

#[test]
fn valid_throws_declaration_covers_propagation() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn explode(): Void throws BoomError {{
             throw BoomError();
         }}
         fn main(): Void throws BoomError {{
             explode();
         }}"
    ));
}

#[test]
fn valid_finally_always_runs() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             try {{
                 throw BoomError();
             }} catch BoomError(e) {{
                 stdout.println(e.message());
             }} finally {{
                 stdout.println(\"cleanup\");
             }}
         }}"
    ));
}

#[test]
fn invalid_throw_of_non_throwable_type() {
    let output = rejected_body("throw \"not throwable\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_uncaught_throw_must_be_declared() {
    let output = rejected(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             throw BoomError();
         }}"
    ));
    assert!(output.contains(codes::UNCAUGHT_THROW.as_str()), "{output}");
}

#[test]
fn invalid_uncovered_throw_from_a_call_must_be_declared() {
    let output = rejected(&format!(
        "{BOOM_ERROR}
         fn explode(): Void throws BoomError {{
             throw BoomError();
         }}
         fn main(): Void {{
             explode();
         }}"
    ));
    assert!(output.contains(codes::UNCAUGHT_THROW.as_str()), "{output}");
}

#[test]
fn invalid_unreachable_catch_after_a_broader_one() {
    let output = rejected(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             try {{
                 throw BoomError();
             }} catch Throwable(e) {{
                 stdout.println(e.message());
             }} catch BoomError(e) {{
                 stdout.println(e.message());
             }}
         }}"
    ));
    assert!(
        output.contains(codes::UNREACHABLE_CATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_bare_rethrow_outside_catch() {
    let output = rejected_body("throw;");
    assert!(
        output.contains(codes::RETHROW_OUTSIDE_CATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_return_directly_inside_finally() {
    let output = rejected(&format!(
        "{BOOM_ERROR}
         fn f(): Void {{
             try {{
                 throw BoomError();
             }} catch BoomError(e) {{
             }} finally {{
                 return;
             }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        output.contains(codes::FINALLY_REPLACES_OUTCOME.as_str()),
        "{output}"
    );
}

// --- Native `RuntimeError` subclasses (`fase-4d-runtimeerror`) --------------

#[test]
fn valid_catch_native_failure_by_its_own_concrete_type() {
    accepted_body(
        "try {
             mut a: Int32 = 1;
             mut b: Int32 = 0;
             mut c: Decimal = a / b;
         } catch DivisionByZeroError(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_catch_native_failure_by_runtime_error_supertype() {
    accepted_body(
        "try {
             mut a: Int32 = 1;
             mut b: Int32 = 0;
             mut c: Decimal = a / b;
         } catch RuntimeError(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_catch_native_failure_by_throwable_supertype() {
    accepted_body(
        "try {
             mut a: Int32 = 1;
             mut b: Int32 = 0;
             mut c: Decimal = a / b;
         } catch Throwable(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_catch_each_of_the_four_native_failure_classes() {
    // Every one of the four type-checks against its own catch, exactly like
    // a user's own `implements RuntimeError` class does — `DivisionByZeroError`
    // is covered by the tests above already, so this exercises the other
    // three: `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError`.
    accepted_body(
        "try {
             mut x: Int32 = 1;
             mut s: Int32 = 40;
             mut y: Int32 = x << s;
         } catch InvalidShiftError(e) {
             stdout.println(e.message());
         }
         try {
             mut a: String = \"x\";
             mut n: Int32 = -1;
             mut r: String = a * n;
         } catch InvalidRepeatError(e) {
             stdout.println(e.message());
         }
         try {
             mut zero: Float = 0.0;
             mut nan: Float = zero / zero;
         } catch FloatNanError(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_catch_arithmetic_overflow_by_its_own_concrete_type_and_supertypes() {
    accepted_body(
        "try {
             mut a: Int8 = 120 as Int8;
             mut b: Int8 = 10 as Int8;
             mut c: Int8 = a + b;
         } catch ArithmeticOverflowError(e) {
             stdout.println(e.message());
         }",
    );
    accepted_body(
        "try {
             mut a: Int32 = 100_000;
             mut b: Int32 = 100_000;
             mut c: Int32 = a * b;
         } catch RuntimeError(e) {
             stdout.println(e.message());
         }",
    );
    accepted_body(
        "try {
             mut a: UInt16 = 40000 as UInt16;
             mut b: UInt16 = 40000 as UInt16;
             mut c: UInt16 = a + b;
         } catch Throwable(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_catch_invalid_cast_by_its_own_concrete_type_and_supertypes() {
    accepted(
        "class Animal { construct() { } }
         class Dog extends Animal { construct() { super(); } }
         fn main(): Void {
             try {
                 mut a: Animal = Dog();
                 mut d: Dog = a as Dog;
             } catch InvalidCastError(e) {
                 stdout.println(e.message());
             }
         }",
    );
    accepted(
        "class Animal { construct() { } }
         class Cat extends Animal { construct() { super(); } }
         class Dog extends Animal { construct() { super(); } }
         fn main(): Void {
             try {
                 mut a: Animal = Cat();
                 mut d: Dog = <Dog>a;
             } catch RuntimeError(e) {
                 stdout.println(e.message());
             }
         }",
    );
}

#[test]
fn valid_native_failure_is_never_declared_in_a_throws_clause() {
    // A native failure is implicit by design
    // (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3): a function
    // that divides is not required — and has no syntax — to declare
    // `throws DivisionByZeroError`, unlike a `throw` of its own.
    accepted(
        "fn divide(a: Int32, b: Int32): Int32 {
             return Int32(a / b);
         }
         fn main(): Void {
             try {
                 mut r: Int32 = divide(1, 0);
             } catch DivisionByZeroError(e) {
                 stdout.println(e.message());
             }
         }",
    );
}

#[test]
fn valid_native_failure_class_is_constructible_like_any_other_runtime_error() {
    // `fase-4d-runtimeerror`'s design (D9) registers the four native failure
    // classes as concrete, not abstract — a program may name and construct
    // one directly, exactly as it could write its own
    // `class Foo implements RuntimeError { construct(reason: String) { ... } }`
    // and call `Foo("reason")`; nothing about being compiler-known makes
    // construction special or forbidden (see `design.md`'s `## Decisions`
    // for why no explicit rejection was added).
    accepted_body(
        "try {
             throw DivisionByZeroError(\"custom\");
         } catch DivisionByZeroError(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn native_capacity_errors_are_known_runtime_error_subclasses() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("capacity_errors.zrk", "fn main(): Void {}"));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert!(
        !sink.has_errors(),
        "no errors were expected:\n{}",
        sink.render(RenderStyle::Human)
    );
    let native = checked
        .native_exceptions
        .expect("native exception hierarchy is registered");
    for (id, expected_name) in [
        (native.task_capacity_error, "TaskCapacityError"),
        (native.timer_capacity_error, "TimerCapacityError"),
        (native.background_capacity_error, "BackgroundCapacityError"),
    ] {
        let class = &checked.classes[id as usize];
        assert_eq!(class.name, expected_name);
        assert_eq!(class.abstract_bases, [native.runtime_error]);
        assert_eq!(
            checked
                .classes
                .iter()
                .filter(|c| c.name == expected_name)
                .count(),
            1
        );
    }

    accepted_body(
        "try {
             throw TaskCapacityError(\"task quota reached\");
         } catch TaskCapacityError(e) {
             e.message();
         }
         try {
             throw TimerCapacityError(\"timer quota reached\");
         } catch RuntimeError(e) {
             e.message();
         }
         try {
             throw BackgroundCapacityError(\"background quota reached\");
         } catch RuntimeError(e) {
             e.message();
         }",
    );
}

#[test]
fn native_capacity_error_names_cannot_be_redeclared_by_user_classes() {
    for name in ["TaskCapacityError", "TimerCapacityError"] {
        let output = rejected(&format!(
            "class {name} {{ construct() {{ }} }} fn main(): Void {{}}"
        ));
        assert!(
            output.contains(&format!("class `{name}` is already defined")),
            "expected duplicate native class diagnostic for {name}:\n{output}"
        );
    }
}

// --- `Resource<E>` / `match ... with` (roadmap Phase 4c) --------------------

const OPEN_ERROR: &str = "class OpenError implements Error {
    inmut reason: String;
    construct(reason: String) { this.reason = reason; }
    #override

      message(): String { return this.reason; }
    #override

      code(): String { return \"OPEN\"; }
    #override cause(): Error? { return null; }
}";

const FAKE_FILE: &str = "class FakeFile implements Resource<OpenError> {
    inmut name: String;
    mut closed: Boolean;
    construct(name: String) { this.name = name; this.closed = false; }
     nothing(): Void { return; }
    #override

      close(): Result<Void, OpenError> {
        this.closed = true;
        return Ok(this.nothing());
    }
    #override

      is_closed(): Boolean { return this.closed; }
}

fn open(name: String): Result<FakeFile, OpenError> {
    return Ok(FakeFile(name));
}";

#[test]
fn valid_class_implements_resource() {
    accepted(&format!(
        "{OPEN_ERROR}
         {FAKE_FILE}
         fn main(): Void {{ }}"
    ));
}

#[test]
fn valid_native_contract_type_annotations_are_not_pending() {
    // `Iterable<T>`, `Iterator<T>` and `Resource<E>` are registered as native
    // contracts in `checker.rs`; they must not be rejected with a pending-phase
    // diagnostic just because `pending_type` used to name a later phase for them.
    //
    // Generic contract instantiations name a real type now (roadmap Phase 7,
    // task 12.3): `Iterator<T>` (the return type of `iterator()`),
    // `Iterable<T>` and `Resource<E>` all resolve to the same
    // `IrType::Contract` their unparameterized contract already lowered to.
    accepted("fn f(x: Iterator<Int32>): Void {}\nfn main(): Void {}");
    accepted("fn f(x: Iterable<Int32>): Void {}\nfn main(): Void {}");
    accepted(&format!(
        "{OPEN_ERROR}\nfn f(x: Resource<OpenError>): Void {{ }}\nfn main(): Void {{ }}"
    ));
}

#[test]
fn valid_match_with_closes_the_acquired_resource() {
    accepted(&format!(
        "{OPEN_ERROR}
         {FAKE_FILE}
         fn main(): Void {{
             match open(\"a.txt\") with file {{
                 Ok(file) => {{
                     stdout.println(file.name);
                 }}
                 Error(error) => {{
                     stdout.println(error.message());
                 }}
             }}
         }}"
    ));
}

#[test]
fn valid_match_with_closes_on_early_return() {
    accepted(&format!(
        "{OPEN_ERROR}
         {FAKE_FILE}
         fn read(): Int32 {{
             match open(\"a.txt\") with file {{
                 Ok(file) => {{
                     return 1;
                 }}
                 Error(error) => {{
                     return -1;
                 }}
             }}
             return 0;
         }}
         fn main(): Void {{ }}"
    ));
}

#[test]
fn invalid_class_does_not_implement_resource() {
    let output = rejected(
        "class OpenError implements Error {
             inmut reason: String;
             construct(reason: String) { this.reason = reason; }
              message(): String { return this.reason; }
              code(): String { return \"OPEN\"; }
              cause(): Error? { return null; }
         }
         class FakeFile implements Resource<OpenError> {
             construct() { }
         }
          main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_with_binding_unused_by_any_arm() {
    let output = rejected(&format!(
        "{OPEN_ERROR}
         {FAKE_FILE}
         fn main(): Void {{
             match open(\"a.txt\") with file {{
                 Ok(other) => {{ }}
                 Error(error) => {{ }}
             }}
         }}"
    ));
    assert!(
        output.contains(codes::INVALID_RESOURCE_MATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_with_scrutinee_not_a_result() {
    let output = rejected(
        "fn main(): Void {
             match 1 with file {
                 _ => { }
             }
         }",
    );
    assert!(
        output.contains(codes::INVALID_RESOURCE_MATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_with_binding_does_not_implement_resource() {
    let output = rejected(
        "fn describe(r: Result<Int32, String>): Void {
             match r with file {
                 Ok(file) => { }
                 Error(error) => { }
             }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::INVALID_RESOURCE_MATCH.as_str()),
        "{output}"
    );
}

// --- Phase 4e: unsafe/Pointer<T>/commit/extern ------------------------------

#[test]
fn valid_pointer_from_read_write_inside_unsafe() {
    accepted_body(
        "mut x: Int32 = 1;
         unsafe {
             mut p: Pointer<Int32> = Pointer.from(x);
             mut v: Int32 = p.read();
             p.write(v + 1);
             mut q: Pointer<Int32> = p.offset(0);
             mut r: Boolean = q.is_null;
         }",
    );
}

#[test]
fn invalid_pointer_read_outside_unsafe() {
    let output = rejected_body(
        "mut x: Int32 = 1;
         mut p: Pointer<Int32> = Pointer.from(x);
         mut v: Int32 = p.read();",
    );
    assert!(
        output.contains(codes::POINTER_OP_OUTSIDE_UNSAFE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_pointer_from_outside_unsafe() {
    let output = rejected_body("mut x: Int32 = 1; mut p: Pointer<Int32> = Pointer.from(x);");
    assert!(
        output.contains(codes::POINTER_OP_OUTSIDE_UNSAFE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_pointer_disallowed_element_type() {
    let output = rejected("fn f(p: Pointer<String>): Void { }\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_FFI_SAFE.as_str()), "{output}");
}

#[test]
fn valid_byte_resolves_to_uint8() {
    accepted_body(
        "mut b: Byte = 1 as Byte;
         mut u: UInt8 = b;",
    );
}

#[test]
fn invalid_commit_outside_unsafe() {
    let output = rejected_body("commit { mut x: Int32 = 1; }");
    assert!(
        output.contains(codes::COMMIT_OUTSIDE_UNSAFE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_commit_inside_unsafe() {
    accepted_body("unsafe { commit { mut x: Int32 = 1; } }");
}

#[test]
fn valid_extern_call_inside_unsafe_and_commit() {
    accepted(
        "extern \"C\" fn abs(n: Int32): Int32;
         fn main(): Void {
             unsafe {
                 commit {
                     mut r: Int32 = abs(-1);
                 }
             }
         }",
    );
}

#[test]
fn invalid_extern_call_missing_commit() {
    let output = rejected(
        "extern \"C\" fn abs(n: Int32): Int32;
         fn main(): Void {
             unsafe {
                 mut r: Int32 = abs(-1);
             }
         }",
    );
    assert!(
        output.contains(codes::EXTERN_CALL_OUTSIDE_UNSAFE_COMMIT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_extern_disallowed_parameter_type() {
    let output = rejected("extern \"C\" fn f(s: String): Void;\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_FFI_SAFE.as_str()), "{output}");
}

#[test]
fn extern_union_parameter_explains_c_abi_wrapper() {
    let output = rejected("extern \"C\" fn f(value: String | Int32): Void;\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_FFI_SAFE.as_str()), "{output}");
    assert!(output.contains("no stable C ABI"), "{output}");
    assert!(output.contains("wrapper function"), "{output}");
}

#[test]
fn invalid_pointer_escapes_as_return_value() {
    let output = rejected(
        "fn leak(x: Pointer<Int32>): Pointer<Int32> {
             return x;
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

#[test]
fn invalid_pointer_escapes_into_a_declared_field() {
    let output = rejected(
        "class Holder {
             value: Pointer<Int32>;
             construct(p: Pointer<Int32>) { this.value = p; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

#[test]
fn invalid_pointer_escapes_via_closure_capture() {
    let output = rejected_body(
        "mut x: Int32 = 1;
         unsafe {
             mut p: Pointer<Int32> = Pointer.from(x);
             mut f: Fn() => Void = (): Void => { mut q: Pointer<Int32> = p; };
         }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

#[test]
fn valid_pointer_from_record_field() {
    accepted(
        "class Point { x: Int32; y: Int32; construct(x: Int32, y: Int32) { this.x = x; this.y = y; } }
         fn main(): Void {
             mut p: Point = Point(1, 2);
             unsafe {
                 mut q: Pointer<Int32> = Pointer.from(p.x);
             }
         }",
    );
}

#[test]
fn valid_pointer_from_user_record_field() {
    accepted(
        "class UserId { value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut u: UserId = UserId(5);
             unsafe {
                 mut p: Pointer<Int32> = Pointer.from(u.value);
             }
         }",
    );
}

#[test]
fn valid_pointer_from_nested_record_field() {
    accepted(
        "class Inner { a: Int32; construct(a: Int32) { this.a = a; } }
         class Outer { inner: Inner; construct(inner: Inner) { this.inner = inner; } }
         fn main(): Void {
             mut o: Outer = Outer(Inner(1));
             unsafe {
                 mut p: Pointer<Int32> = Pointer.from(o.inner.a);
             }
         }",
    );
}

#[test]
fn invalid_pointer_from_record_temporary() {
    let output = rejected(
        "record Point { x: Int32; }
         fn make_point(): Point { return Point(x: 1); }
         fn main(): Void {
             unsafe {
                 mut p: Pointer<Int32> = Pointer.from(make_point().x);
             }
         }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

// --- Phase 4e: Weak<T> (`fase-4e-weak`) -------------------------------------

#[test]
fn valid_weak_of_a_class_referent() {
    accepted(
        "class Marker { construct() { } }
         fn main(): Void {
             mut m: Marker = Marker();
             mut w: Weak<Marker> = Weak.from(m);
         }",
    );
}

#[test]
fn valid_weak_upgrade_and_is_alive() {
    accepted(
        "class Marker { construct() { } }
         fn main(): Void {
             mut m: Marker = Marker();
             mut w: Weak<Marker> = Weak.from(m);
             mut alive: Boolean = w.is_alive;
             mut upgraded: Marker? = w.upgrade();
         }",
    );
}

#[test]
fn invalid_weak_of_a_scalar_referent() {
    let output = rejected("fn f(w: Weak<Int32>): Void { }\nfn main(): Void { }");
    assert!(
        output.contains(codes::WEAK_DISALLOWED_REFERENT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_weak_from_of_a_scalar_value() {
    let output = rejected_body("mut w: Weak<Int32> = Weak.from(1);");
    assert!(
        output.contains(codes::WEAK_DISALLOWED_REFERENT.as_str()),
        "{output}"
    );
}

// --- Phase 4e: Clone (`fase-4e-clone`) --------------------------------------

#[test]
fn valid_class_with_only_scalar_fields_derives_clone() {
    accepted(
        "class Point { mut x: Int32; mut y: Int32; construct(x: Int32, y: Int32) { this.x = x; this.y = y; } }
         fn main(): Void {
             mut p: Point = Point(1, 2);
             mut q: Point = p.clone();
         }",
    );
}

#[test]
fn valid_class_with_a_clone_class_typed_field_derives_clone() {
    accepted(
        "class Leaf { mut value: Int32; construct(value: Int32) { this.value = value; } }
         class Parent { mut leaf: Leaf; construct(leaf: Leaf) { this.leaf = leaf; } }
         fn main(): Void {
             mut leaf: Leaf = Leaf(1);
             mut parent: Parent = Parent(leaf);
             mut clone: Parent = parent.clone();
         }",
    );
}

#[test]
fn invalid_class_with_a_pointer_field_is_not_clone() {
    let output = rejected(
        "class Holder { mut p: Pointer<Int32>; construct(p: Pointer<Int32>) { this.p = p; } }
         fn main(): Void {
             mut x: Int32 = 1;
             unsafe {
                 mut h: Holder = Holder(Pointer.from(x));
                 mut clone: Holder = h.clone();
             }
         }",
    );
    assert!(output.contains(codes::NOT_CLONE.as_str()), "{output}");
    assert!(output.contains("p"), "{output}");
}

#[test]
fn invalid_class_that_implements_resource_is_not_clone() {
    let output = rejected(
        "class OpenError implements Error {
             inmut reason: String;
             construct(reason: String) { this.reason = reason; }
              message(): String { return this.reason; }
              code(): String { return \"OPEN\"; }
              cause(): Error? { return null; }
         }
         class FakeFile implements Resource<OpenError> {
             inmut name: String;
             mut closed: Boolean;
             construct(name: String) { this.name = name; this.closed = false; }
              close(): Result<Void, OpenError> { this.closed = true; return Ok(_void()); }
              is_closed(): Boolean { return this.closed; }
             _void(): Void { return; }
         }
         fn main(): Void {
             mut f: FakeFile = FakeFile(\"a.txt\");
             mut clone: FakeFile = f.clone();
         }",
    );
    assert!(output.contains(codes::NOT_CLONE.as_str()), "{output}");
}

#[test]
fn invalid_class_with_a_non_clone_nested_field_names_the_transitive_path() {
    let output = rejected(
        "class Inner { mut p: Pointer<Int32>; construct(p: Pointer<Int32>) { this.p = p; } }
         class Outer { mut inner: Inner; construct(inner: Inner) { this.inner = inner; } }
         fn main(): Void {
             mut x: Int32 = 1;
             unsafe {
                 mut inner: Inner = Inner(Pointer.from(x));
                 mut outer: Outer = Outer(inner);
                 mut clone: Outer = outer.clone();
             }
         }",
    );
    assert!(output.contains(codes::NOT_CLONE.as_str()), "{output}");
    assert!(output.contains("inner"), "{output}");
    assert!(output.contains("p"), "{output}");
}

#[test]
fn valid_generic_bound_t_from_clone_is_accepted_for_a_clone_type() {
    accepted(
        "class Marker { mut id: Int32; construct(id: Int32) { this.id = id; } }
         fn dup<T from Clone>(value: T): Void { }
         fn main(): Void {
             mut m: Marker = Marker(1);
             dup(m);
         }",
    );
}

#[test]
fn invalid_generic_bound_t_from_clone_is_rejected_for_a_non_clone_type() {
    let output = rejected(
        "class Holder { mut p: Pointer<Int32>; construct(p: Pointer<Int32>) { this.p = p; } }
         fn dup<T from Clone>(value: T): Void { }
         fn main(): Void {
             mut x: Int32 = 1;
             unsafe {
                 mut h: Holder = Holder(Pointer.from(x));
                 dup(h);
             }
         }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("Clone"), "{output}");
}

#[test]
fn valid_class_may_write_implements_clone_explicitly() {
    accepted(
        "class Point implements Clone { mut x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             mut p: Point = Point(1);
             mut q: Point = p.clone();
         }",
    );
}

#[test]
fn valid_manual_clone_implementation_is_dispatched_as_an_ordinary_method() {
    // `Greeter`-typed field: not `Clone`-derivable on its own (a
    // contract-typed member's implementors are not all known to be
    // `Clone`), but a manual `clone()` body bypasses derivation entirely —
    // `Self::check_method_call_on`'s own gate never reaches
    // `check_derived_clone_call` once the class declares the method itself.
    accepted(
        "contract Greeter {  greet(): String; }
         class Loud implements Greeter { construct() { }  greet(): String { return \"hi\"; } }
         class Holder { mut g: Greeter; construct(g: Greeter) { this.g = g; }  clone(): Holder { return this; } }
         fn main(): Void {
             mut h: Holder = Holder(Loud());
             mut clone: Holder = h.clone();
         }",
    );
}

// --- Phase 4e: NativeSlice<T>/NativeSliceMut<T> (`fase-4e-native-slice`) ----

#[test]
fn invalid_as_slice_outside_unsafe() {
    let output = rejected_body(
        "mut x: Int32 = 1;
         mut p: Pointer<Int32> = Pointer.from(x);
         mut r: Result<NativeSlice<Int32>, NativeError> = p.as_slice(1);",
    );
    assert!(
        output.contains(codes::POINTER_OP_OUTSIDE_UNSAFE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_as_slice_mut_outside_unsafe() {
    let output = rejected_body(
        "mut x: Int32 = 1;
         mut p: Pointer<Int32> = Pointer.from(x);
         mut r: Result<NativeSliceMut<Int32>, NativeError> = p.as_slice_mut(1);",
    );
    assert!(
        output.contains(codes::POINTER_OP_OUTSIDE_UNSAFE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_native_slice_disallowed_element_type() {
    let output = rejected("fn f(v: NativeSlice<String>): Void { }\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_FFI_SAFE.as_str()), "{output}");
}

#[test]
fn invalid_native_slice_mut_disallowed_element_type() {
    let output = rejected("fn f(v: NativeSliceMut<String>): Void { }\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_FFI_SAFE.as_str()), "{output}");
}

#[test]
fn valid_as_slice_construction_and_indexing_read_outside_unsafe() {
    // Construction requires `unsafe`; indexing and `.length`/`.is_empty` do
    // not — the proposal's own "no `unsafe` needed to read/write through a
    // view, only to construct one".
    accepted_body(
        "mut x: Int32 = 1;
         mut view: NativeSlice<Int32> = unsafe {
             mut p: Pointer<Int32> = Pointer.from(x);
             match p.as_slice(1) { Ok(view) => view; Error(_) => fatalError(\"slice failed\"); }
         };
         mut v: Int32 = view[0];
         mut n: UInt64 = view.length;
         mut e: Boolean = view.is_empty;",
    );
}

#[test]
fn valid_native_slice_mut_indexing_read_and_write_outside_unsafe() {
    accepted_body(
        "mut x: Int32 = 1;
         mut view: NativeSliceMut<Int32> = unsafe {
             mut p: Pointer<Int32> = Pointer.from(x);
             match p.as_slice_mut(1) { Ok(view) => view; Error(_) => fatalError(\"slice failed\"); }
         };
         view[0] = 42;
         mut v: Int32 = view[0];",
    );
}

#[test]
fn invalid_write_through_read_only_native_slice() {
    let output = rejected_body(
        "mut x: Int32 = 1;
         mut view: NativeSlice<Int32> = unsafe {
             mut p: Pointer<Int32> = Pointer.from(x);
             match p.as_slice(1) { Ok(view) => view; Error(_) => fatalError(\"slice failed\"); }
         };
         view[0] = 42;",
    );
    assert!(
        output.contains(codes::INDEX_NOT_WRITABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_indexing_an_unsupported_receiver_type() {
    let output = rejected_body("mut n: Int32 = 1;\nmut v: Int32 = n[0];");
    assert!(
        output.contains(codes::INDEXING_NOT_SUPPORTED.as_str()),
        "{output}"
    );
    assert!(output.contains("Int32"), "{output}");
}

#[test]
fn invalid_native_slice_escapes_as_return_value() {
    // The "Native view escapes its borrow" scenario, now actually enforced
    // (roadmap Phase 4e, `fase-4e-native-slice`, design D4): the owner
    // (`x`, a `Pointer<Int32>` local to the function) ends when the
    // function returns, so the view built from it cannot escape either.
    let output = rejected(
        "fn make(): NativeSlice<Int32> {
             mut x: Int32 = 1;
             unsafe {
                 mut p: Pointer<Int32> = Pointer.from(x);
                 return match p.as_slice(1) { Ok(view) => view; Error(_) => fatalError(\"slice failed\"); };
             }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

#[test]
fn invalid_native_slice_mut_escapes_as_return_value() {
    let output = rejected(
        "fn make(): NativeSliceMut<Int32> {
             mut x: Int32 = 1;
             unsafe {
                 mut p: Pointer<Int32> = Pointer.from(x);
                 return match p.as_slice_mut(1) { Ok(view) => view; Error(_) => fatalError(\"slice failed\"); };
             }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

#[test]
fn invalid_native_slice_escapes_via_closure_capture() {
    let output = rejected_body(
        "mut x: Int32 = 1;
         unsafe {
             mut p: Pointer<Int32> = Pointer.from(x);
             mut view: NativeSlice<Int32> = match p.as_slice(1) { Ok(view) => view; Error(_) => fatalError(\"slice failed\"); };
             mut f: Fn() => Void = (): Void => { mut w: NativeSlice<Int32> = view; };
         }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

#[test]
fn invalid_native_slice_escapes_into_a_declared_field() {
    let output = rejected(
        "class Holder {
             value: NativeSlice<Int32>;
             construct(v: NativeSlice<Int32>) { this.value = v; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

// --- Phase 4e: String[index] read-only grapheme access ------------------------

#[test]
fn valid_string_index_returns_a_char() {
    accepted(
        "fn main(): Void {
             mut s: String = \"hola\";
             mut c: Char = s[1];
             stdout.println(c.to_string());
         }",
    );
}

#[test]
fn valid_string_index_write() {
    // `s[i] = c` writes a grapheme in place (roadmap Phase 7, task 10.1) —
    // the `INDEX_NOT_WRITABLE` gate this used to assert is gone.
    accepted(
        "fn main(): Void {
             mut s: String = \"hola\";
             s[0] = 'x';
         }",
    );
}

#[test]
fn valid_string_index_write_through_inmut_referent() {
    accepted(
        "fn main(): Void {
             inmut s: String = \"hola\";
             s[0] = 'x';
         }",
    );
}

#[test]
fn invalid_string_index_write_through_strict_referent() {
    let output = rejected(
        "fn main(): Void {
             inmut::strict s: String = \"hola\";
             s[0] = 'x';
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

// --- `Map<K, V>` / `Set<T>` (roadmap Phase 7, `map-set-collections`) ---------

#[test]
fn valid_map_and_set_types_resolve() {
    accepted_body(
        "mut m: Map<String, Int32> = Map();\n\
         mut s: Set<String> = Set();",
    );
}

#[test]
fn valid_collection_word_layouts_cover_scalars_and_managed_handles() {
    accepted(
        "enum Direction { North, East }\n\
         class User { construct() { } }\n\
         fn main(): Void {\n\
             mut integers: Map<String, UInt64> = Map();\n\
             mut flags: Map<String, Boolean> = Map();\n\
             mut characters: Map<String, Char> = Map();\n\
             mut strings: Map<String, String> = Map();\n\
             mut durations: Map<String, Duration> = Map();\n\
             mut dates: Map<String, Date> = Map();\n\
             mut times: Map<String, Time> = Map();\n\
             mut directions: Map<String, Direction> = Map();\n\
             mut users: Map<String, User> = Map();\n\
             mut arrays: Map<String, Array<Int32>> = Map();\n\
             mut lists: Map<String, List<String>> = Map();\n\
             mut nested: Map<String, Set<Int32>> = Map();\n\
             mut keys: Set<Int64> = Set();\n\
             mut names: Set<String> = Set();\n\
             mut chars: Set<Char> = Set();\n\
             mut kinds: Set<Direction> = Set();\n\
             mut date_set: Set<Date> = Set();\n\
             mut time_set: Set<Time> = Set();\n\
         }",
    );
}

#[test]
fn valid_map_set_aggregate_descriptors_remain_semantically_admissible() {
    accepted(
        "enum Direction { North }\n\
         fn main(): Void {\n\
             mut objects: List<Object> = List();\n\
             mut nullable: List<String?> = List();\n\
             mut tuples: List<Tuple<Int32, String>> = List();\n\
             mut closures: List<Fn(Int32) => Int32> = List();\
         }",
    );
}

#[test]
fn invalid_set_of_an_unhashable_element() {
    let output = rejected_body("mut s: Set<List<Int32>> = Set();");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("unhashable"), "{output}");
}

#[test]
fn invalid_collection_keys_reject_wide_nullable_and_aggregate_types() {
    for (source, description) in [
        ("mut s: Set<Int128> = Set();", "Set<Int128>"),
        ("mut s: Set<UInt128> = Set();", "Set<UInt128>"),
        ("mut s: Set<Int32?> = Set();", "Set<Int32?>"),
        (
            "mut m: Map<Int128, Boolean> = Map();",
            "Map<Int128, Boolean>",
        ),
        (
            "mut m: Map<Tuple<Int32, String>, Boolean> = Map();",
            "tuple key",
        ),
        ("mut m: Map<List<Int32>, Boolean> = Map();", "list key"),
    ] {
        let output = rejected_body(source);
        assert!(
            output.contains(codes::TYPE_MISMATCH.as_str()),
            "{description}: {output}"
        );
        assert!(output.contains("unhashable"), "{description}: {output}");
    }
}

#[test]
fn invalid_map_values_fail_early_when_the_word_abi_cannot_round_trip_them() {
    for (source, description) in [
        ("mut m: Map<String, Int128> = Map();", "Int128"),
        ("mut m: Map<String, UInt128> = Map();", "UInt128"),
        ("mut m: Map<String, Float64> = Map();", "Float64"),
        ("mut m: Map<String, Decimal> = Map();", "Decimal"),
        ("mut m: Map<String, String?> = Map();", "String?"),
        ("mut m: Map<String, Tuple<Int32, String>> = Map();", "Tuple"),
        ("mut m: Map<String, Fn(Int32) => Int32> = Map();", "Fn("),
    ] {
        let output = rejected_body(source);
        assert!(
            output.contains(codes::TYPE_MISMATCH.as_str()),
            "{description}: {output}"
        );
        assert!(
            output.contains("one-word value representation"),
            "{description}: {output}"
        );
        assert!(output.contains(description), "{description}: {output}");
    }
}

#[test]
fn invalid_map_values_reject_payload_enums() {
    let output = rejected(
        "enum Outcome { Success(value: String), Failure }\n\
         fn main(): Void {\n\
             mut outcomes: Map<String, Outcome> = Map();\n\
         }",
    );
    assert!(output.contains("Map<..., Outcome>"), "{output}");
    assert!(output.contains("one-word value representation"), "{output}");
}

#[test]
fn invalid_map_key_payload_enum_is_rejected_at_its_type_span() {
    let output = rejected(
        "enum Outcome { Success(value: String), Failure }\n\
         fn main(): Void {\n\
             mut outcomes: Map<Outcome, Int32> = Map();\n\
         }",
    );
    assert!(output.contains("Map<Outcome, ...>"), "{output}");
    assert!(output.contains("unhashable key type"), "{output}");
}

#[test]
fn valid_map_and_set_members() {
    accepted_body(
        "mut m: Map<String, Int32> = Map();\n\
         m.set(\"k\", 1);\n\
         mut n: UInt64 = m.length;\n\
         mut e: Boolean = m.is_empty;\n\
         mut has: Boolean = m.contains_key(\"k\");\n\
         mut v: Int32? = m.get(\"k\");\n\
         mut r: Boolean = m.remove(\"k\");\n\
         mut s: Set<String> = Set();\n\
         s.add(\"x\");\n\
         mut h: Boolean = s.contains(\"x\");\n\
         mut sn: UInt64 = s.length;\n\
         mut se: Boolean = s.is_empty;\n\
         mut sr: Boolean = s.remove(\"x\");",
    );
}

#[test]
fn invalid_map_set_with_wrong_argument_types() {
    let output = rejected_body(
        "mut m: Map<String, Int32> = Map();\n\
         m.set(1, \"x\");",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_set_add_with_wrong_element_type() {
    let output = rejected_body(
        "mut s: Set<String> = Set();\n\
         s.add(1);",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

// --- Civil temporal types (date-and-time-types) ------------------------------

#[test]
fn valid_temporal_constructions_and_components() {
    accepted_body(
        "mut d: Date = Date(2026, 9, 6);\n\
         mut t: Time = Time(14, 30);\n\
         mut t2: Time = Time(14, 30, 45, 123456789);\n\
         mut dt: DateTime = DateTime(d, t);\n\
         mut y: Int32 = d.year;\n\
         mut mo: Int32 = d.month;\n\
         mut dd: Int32 = d.day;\n\
         mut h: Int32 = t.hour;\n\
         mut mi: Int32 = t.minute;\n\
         mut s: Int32 = t.second;\n\
         mut ns: Int64 = t.nanosecond;\n\
         mut dy: Int32 = dt.day;\n\
         mut th: Int32 = dt.hour;\n\
         mut dd2: Date = dt.date;\n\
         mut tt: Time = dt.time;\n\
         mut s1: String = d.to_string();\n\
         mut s2: String = t.to_string();\n\
         mut s3: String = dt.to_string();",
    );
}

#[test]
fn valid_temporal_static_constructors() {
    accepted_body(
        "mut d: Date = Date.today();\n\
         mut t: Time = Time.now_local();\n\
         mut t2: Time = Time.now_utc();\n\
         mut dt: DateTime = DateTime.now_local();\n\
         mut dt2: DateTime = DateTime.now_utc();",
    );
}

#[test]
fn valid_temporal_arithmetic_and_comparison() {
    accepted_body(
        "mut d: Date = Date(2026, 9, 6);\n\
         mut d2: Date = d + 2;\n\
         mut d3: Date = 2 + d;\n\
         mut d4: Date = d - 1;\n\
         mut days: Int64 = d - Date(2026, 9, 1);\n\
         mut t: Time = Time(23, 30);\n\
         mut t2: Time = t + 2h;\n\
         mut t3: Time = 2h + t;\n\
         mut t4: Time = t - 1h;\n\
         mut diff: Duration = t - Time(1, 0);\n\
         mut dt: DateTime = d + t;\n\
         mut dt2: DateTime = t + d;\n\
         mut b1: Boolean = d < Date(2026, 10, 1);\n\
         mut b2: Boolean = d == Date(2026, 9, 6);\n\
         mut b3: Boolean = t > Time(0, 0);\n\
         mut b4: Boolean = dt <= DateTime.now_utc();",
    );
}

#[test]
fn invalid_temporal_construction_arity() {
    let output = rejected_body("mut d: Date = Date(2026, 9);");
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
    let output = rejected_body("mut t: Time = Time(1);");
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
    let output = rejected_body("mut dt: DateTime = DateTime(Date(2026, 9, 6));");
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_temporal_construction_argument_types() {
    let output = rejected_body("mut d: Date = Date(2026, \"x\", 6);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    let output = rejected_body("mut dt: DateTime = DateTime(1, 2);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    let output = rejected_body("mut dt: DateTime = DateTime(Time(1, 0), Date(2026, 9, 6));");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_temporal_mixed_arithmetic() {
    let output = rejected_body("mut t: Time = Time(1, 0);\nmut x = t + 2;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    let output = rejected_body("mut d: Date = Date(2026, 9, 6);\nmut x = d - Time(1, 0);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_temporal_member_names() {
    let output = rejected_body("mut d: Date = Date(2026, 9, 6);\nmut x = d.hour;");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    let output = rejected_body("mut t: Time = Time(1, 0);\nmut x = t.year;");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

// --- Rich civil temporal API (temporal-rich-api) -----------------------------

#[test]
fn valid_temporal_calendrical_properties() {
    accepted_body(
        "mut d: Date = Date(2026, 9, 6);\n\
         mut dow: Int32 = d.day_of_week;\n\
         mut doy: Int32 = d.day_of_year;\n\
         mut wk: Int32 = d.week_of_year;\n\
         mut q: Int32 = d.quarter;\n\
         mut dim: Int32 = d.days_in_month;\n\
         mut diy: Int32 = d.days_in_year;\n\
         mut leap: Boolean = d.is_leap_year;\n\
         mut dt: DateTime = DateTime.now_utc();\n\
         mut dt_dow: Int32 = dt.day_of_week;\n\
         mut dt_doy: Int32 = dt.day_of_year;\n\
         mut dt_wk: Int32 = dt.week_of_year;\n\
         mut dt_q: Int32 = dt.quarter;\n\
         mut dt_dim: Int32 = dt.days_in_month;\n\
         mut dt_diy: Int32 = dt.days_in_year;\n\
         mut dt_leap: Boolean = dt.is_leap_year;",
    );
}

#[test]
fn valid_temporal_subsecond_properties() {
    accepted_body(
        "mut t: Time = Time(14, 30, 45, 123456789);\n\
         mut ms: Int32 = t.millisecond;\n\
         mut us: Int32 = t.microsecond;\n\
         mut ns: Int64 = t.nanosecond;\n\
         mut dt: DateTime = DateTime.now_utc();\n\
         mut dt_ms: Int32 = dt.millisecond;\n\
         mut dt_us: Int32 = dt.microsecond;",
    );
}

#[test]
fn valid_temporal_query_methods() {
    accepted_body(
        "mut d: Date = Date(2026, 9, 6);\n\
         mut a: Boolean = d.is_before(Date(2026, 10, 1));\n\
         mut b: Boolean = d.is_after(Date(2026, 9, 1));\n\
         mut c: Boolean = d.is_same(Date(2026, 9, 6));\n\
         mut e: Boolean = d.is_same_or_before(Date(2026, 9, 6));\n\
         mut f: Boolean = d.is_same_or_after(Date(2026, 9, 6));\n\
         mut g: Boolean = d.is_between(Date(2026, 1, 1), Date(2026, 12, 31));\n\
         mut wd: Boolean = d.is_weekday();\n\
         mut we: Boolean = d.is_weekend();\n\
         mut t: Time = Time(14, 30);\n\
         mut tb: Boolean = t.is_before(Time(15, 0));\n\
         mut dt: DateTime = DateTime.now_utc();\n\
         mut dtb: Boolean = dt.is_between(dt, dt);\n\
         mut dtw: Boolean = dt.is_weekday();",
    );
}

#[test]
fn valid_temporal_with_methods() {
    accepted_body(
        "mut d: Date = Date(2026, 9, 6);\n\
         mut d2: Date = d.with_year(2030);\n\
         mut d3: Date = d.with_month(12);\n\
         mut d4: Date = d.with_day(15);\n\
         mut t: Time = Time(14, 30);\n\
         mut t2: Time = t.with_hour(9);\n\
         mut t3: Time = t.with_minute(0);\n\
         mut t4: Time = t.with_second(0);\n\
         mut t5: Time = t.with_nanosecond(500);\n\
         mut dt: DateTime = DateTime.now_utc();\n\
         mut dt2: DateTime = dt.with_year(2030);\n\
         mut dt3: DateTime = dt.with_hour(9);\n\
         mut dt4: DateTime = dt.with_nanosecond(500);\n\
         mut dt5: DateTime = dt.with_date(d);\n\
         mut dt6: DateTime = dt.with_time(t);",
    );
}

#[test]
fn valid_temporal_boundaries_and_format() {
    accepted_body(
        "mut d: Date = Date(2026, 9, 6);\n\
         mut s1: Result<Date, ParseError> = d.start_of(\"year\");\n\
         mut e1: Result<Date, ParseError> = d.end_of(\"month\");\n\
         mut f1: String = d.format(\"YYYY-MM-DD\");\n\
         mut t: Time = Time(14, 30);\n\
         mut s2: Result<Time, ParseError> = t.start_of(\"hour\");\n\
         mut dt: DateTime = DateTime.now_utc();\n\
         mut s3: Result<DateTime, ParseError> = dt.end_of(\"day\");\n\
         mut f3: String = dt.format(\"HH:mm\");",
    );
}

#[test]
fn valid_temporal_parse_statics() {
    accepted_body(
        "mut r1: Result<Date, ParseError> = Date.parse(\"2026-09-06\");\n\
         mut d: Date? = r1;\n\
         mut r2: Result<Time, ParseError> = Time.parse(\"14:30:45\");\n\
         mut t: Time? = r2;\n\
         mut r3: Result<DateTime, ParseError> = DateTime.parse(\"2026-09-06T14:30:45\");\n\
         mut dt: DateTime? = r3;",
    );
}

#[test]
fn valid_temporal_duration_interop() {
    accepted_body(
        "mut d: Date = Date(2026, 9, 6);\n\
         mut t: Time = Time(14, 30);\n\
         mut dt: DateTime = DateTime(d, t);\n\
         mut a: DateTime = dt + 2h;\n\
         mut b: DateTime = dt - 30m;\n\
         mut c: DateTime = d + 5h;\n\
         mut e: DateTime = d - 25h;\n\
         mut f: DateTime = 3h + d;\n\
         mut g: DateTime = 2h + dt;\n\
         mut h: Duration = dt - DateTime.now_utc();",
    );
}

#[test]
fn invalid_temporal_method_argument_types() {
    // A `with_*` component must be the same width family the field is.
    let output = rejected_body("mut d: Date = Date(2026, 9, 6);\nmut x = d.with_month(\"enero\");");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    // `is_before` compares same-type operands.
    let output = rejected_body("mut d: Date = Date(2026, 9, 6);\nmut x = d.is_before(Time(1, 0));");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    // `is_weekday`/`is_weekend` are calendar members — `Time` has none.
    let output = rejected_body("mut t: Time = Time(1, 0);\nmut x = t.is_weekday();");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    // `start_of` takes a `String` unit.
    let output = rejected_body("mut d: Date = Date(2026, 9, 6);\nmut x = d.start_of(2);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_temporal_duration_interop() {
    // `Duration` does not subtract a temporal value.
    let output = rejected_body("mut d: Date = Date(2026, 9, 6);\nmut x = 2h - d;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    let output = rejected_body("mut dt: DateTime = DateTime.now_utc();\nmut x = 1h - dt;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    // `Date - Time` stays rejected; only `DateTime - DateTime` yields a
    // `Duration`.
    let output = rejected_body("mut d: Date = Date(2026, 9, 6);\nmut x = d - Time(1, 0);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    // `Time - Time` yields `Duration`; `Time + Int` stays rejected.
    let output = rejected_body("mut t: Time = Time(1, 0);\nmut x = t + 2;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

// --- `task` / `await` were removed (`remove-task-await-model`) --------------

#[test]
fn task_is_not_a_type() {
    // `Task<T>` no longer resolves: it is an unknown type name.
    let output = rejected_body("mut h: Task<Int32> = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn mutex_type_is_known_while_rwlock_remains_phase_gated() {
    accepted("fn inspect(value: Mutex<Int32>): Void { } fn main(): Void { }");
    let output = rejected("fn inspect(value: RwLock<Int32>): Void { } fn main(): Void { }");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn mutex_handles_follow_reference_alias_rules() {
    let output = rejected_body(
        "mut mutex: Mutex<Int32> = Mutex(0);\n\
         inmut::strict alias: Mutex<Int32> = mutex;",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "GC-managed mutex handles have reference identity: {output}"
    );
}

#[test]
fn mutex_with_types_a_scoped_writable_payload_view() {
    accepted(
        "class Box { mut value: Int32; construct() { this.value = 0; } }\n\
         fn main(): Void {\n\
           mut mutex: Mutex<Box> = Mutex(Box());\n\
           mutex.with((view: Box): Void => { view.value = 1; });\n\
         }",
    );
}

#[test]
fn mutex_with_marks_scalar_payload_view_mutable() {
    accepted(
        "fn main(): Void {\n\
           mut mutex: Mutex<Int32> = Mutex(0);\n\
           mutex.with((view: Int32): Void => { view = 2; });\n\
         }",
    );
}

#[test]
fn mutex_with_rejects_escaping_views_and_nested_capture() {
    let escaped = rejected_mutex_with(
        "class Box { mut value: Int32; construct() { this.value = 0; } }\n\
         fn main(): Void {\n\
           mut mutex: Mutex<Box> = Mutex(Box());\n\
           mut saved: Box? = null;\n\
           mutex.with((view: Box): Void => { saved = view; });\n\
         }",
    );
    assert!(escaped.contains("borrowed view escapes"), "{escaped}");

    let returned = rejected_mutex_with(
        "class Box { mut value: Int32; construct() { this.value = 0; } }\n\
         fn main(): Void {\n\
           mut mutex: Mutex<Box> = Mutex(Box());\n\
           mut saved: Box = mutex.with((view: Box): Box => view);\n\
         }",
    );
    assert!(
        returned.contains(codes::TYPE_MISMATCH.as_str()),
        "{returned}"
    );

    let nested = rejected_mutex_with(
        "class Box { mut value: Int32; construct() { this.value = 0; } }\n\
         fn main(): Void {\n\
           mut mutex: Mutex<Box> = Mutex(Box());\n\
           mutex.with((view: Box): Void => { _ = spawn { view.value }; });\n\
         }",
    );
    assert!(nested.contains("borrowed view escapes"), "{nested}");
}

#[test]
fn mutex_with_rejects_direct_and_transitive_suspension_and_unknown_effects() {
    let direct = rejected_mutex_with(
        "class Box { mut value: Int32; construct() { this.value = 0; } }\n\
         fn main(): Void {\n\
           mut mutex: Mutex<Box> = Mutex(Box());\n\
           mutex.with((view: Box): Void => { Timer.sleep(1s); });\n\
         }",
    );
    assert!(
        direct.contains("Mutex.with` callback may suspend"),
        "{direct}"
    );

    let transitive = rejected_mutex_with(
        "class Box { mut value: Int32; construct() { this.value = 0; } }\n\
         fn pause(): Void { Timer.sleep(1s); }\n\
         fn main(): Void {\n\
           mut mutex: Mutex<Box> = Mutex(Box());\n\
           mutex.with((view: Box): Void => { pause(); });\n\
         }",
    );
    assert!(
        transitive.contains("Mutex.with` callback may suspend"),
        "{transitive}"
    );

    let unknown = rejected_mutex_with(
        "class Box { mut value: Int32; construct() { this.value = 0; } }\n\
         fn main(): Void {\n\
           mut mutex: Mutex<Box> = Mutex(Box());\n\
           mut callback: Fn() => Void = (): Void => { };\n\
           mutex.with((view: Box): Void => { callback(); });\n\
         }",
    );
    assert!(
        unknown.contains("Mutex.with` callback may suspend"),
        "{unknown}"
    );
}

#[test]
fn nested_mutex_with_is_a_suspend_and_block_effect_under_the_outer_guard() {
    let output = rejected_mutex_with(
        "fn main(): Void {\n\
           mut outer: Mutex<Int32> = Mutex(0);\n\
           mut inner: Mutex<Int32> = Mutex(0);\n\
           outer.with((view: Int32): Void => {\n\
             inner.with((nested: Int32): Void => { });\n\
           });\n\
         }",
    );
    assert!(
        output.contains("Mutex.with` callback may suspend"),
        "nested acquisition may wait while the outer guard is live: {output}"
    );
}

#[test]
fn transitive_mutex_with_acquisition_is_rejected_under_the_outer_guard() {
    let output = rejected_mutex_with(
        "fn acquire(inner: Mutex<Int32>): Void {\n\
           inner.with((nested: Int32): Void => { });\n\
         }\n\
         fn main(): Void {\n\
           mut outer: Mutex<Int32> = Mutex(0);\n\
           mut inner: Mutex<Int32> = Mutex(0);\n\
           outer.with((view: Int32): Void => { acquire(inner); });\n\
         }",
    );
    assert!(
        output.contains("Mutex.with` callback may suspend"),
        "nested acquisition effects propagate through direct calls: {output}"
    );
}

#[test]
fn task_and_select_are_ordinary_identifiers() {
    accepted_body("mut task = 1; mut select = 2; task = select;");
}

// --- Range collection expansion (OpenSpec `range-collection-expansion`) ---

#[test]
fn valid_range_collection_context_and_constructor_expansion_type_check() {
    accepted_body(
        "mut inferred = [0..3];\n\
         mut first: Int32 = inferred[0];\n\
         mut listed: List<Int32> = [0..3, 9];\n\
         mut wide: Array<Int64> = [0..3];\n\
         mut array: Array<Int32> = Array(0..2, 9);\n\
         mut from_constructor: List<Int32> = List(0..=2);",
    );
}

#[test]
fn valid_fixed_array_is_initialized_for_semantic_reads() {
    accepted_body("mut buffer: Int32[2];\nmut first: Int32 = buffer[0];");
}

#[test]
fn invalid_range_operands_and_steps_report_migration_safe_diagnostics() {
    for body in [
        "mut end = 3;\nfor i in 0..end { }",
        "for i in 0..3:0 { }",
        "for i in 0..3:-1 { }",
        "for i in 3..0:1 { }",
    ] {
        let output = rejected_body(body);
        assert!(output.contains(codes::INVALID_RANGE.as_str()), "{output}");
    }
}

#[test]
fn invalid_collection_element_and_removed_range_builders_are_rejected() {
    let output = rejected_body("mut xs = [1, \"not an integer\"];");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");

    for body in ["mut r = 0..3; r.reverse();", "mut r = 0..3; r.step(2);"] {
        let output = rejected_body(body);
        assert!(output.contains(codes::INVALID_RANGE.as_str()), "{output}");
        assert!(output.contains("colon-step"), "{output}");
    }
}

// --- Spread and rest operators (OpenSpec `spread-rest-operators`) ------------

#[test]
fn valid_spread_and_rest_operators_type_check() {
    accepted(
        "fn sum(...values: Int32): Int32 { mut total: Int32 = 0; for value in values { total = total + value; } return total; }\n\
         fn main(): Void {\n\
             mut values: List<Int32> = [1, 2, 3];\n\
             mut total: Int32 = sum(...values);\n\
             mut array = [0, ...values, 4];\n\
             mut listed: List<Int32> = [0, ...values, 4];\n\
             mut copied: Array<Int32> = Array(...values);\n\
             mut linked: List<Int32> = List(...values);\n\
             mut [first, ...remaining] = values;\n\
         }",
    );
}

#[test]
fn valid_record_spread_and_rest_type_check() {
    accepted(
        "\n\
         fn main(): Void {\n\
             mut profile: Object = { id: 7, name: \"Ada\", active: true };\n\
             mut renamed: Object = { ...profile, name: \"Grace\" };\n\
             mut { id, ...details } = profile;\n\
             mut { id: profile_id, name: profile_name, ...flags } = profile;\n\
         }",
    );
}

// --- Parallel CPU regions (OpenSpec `parallel-cpu-regions`) -----------------

#[test]
fn valid_parallel_block_types_as_its_final_expression() {
    accepted(
        "fn work(): Int32 { return 7; }\n\
         fn main(): Void {\n\
           mut total: Int32 = parallel { work() };\n\
         }",
    );
}

#[test]
fn valid_parallel_block_with_cores_options_type_check() {
    accepted_body("parallel; cores: 4 { }");
    accepted_body("parallel; cores: -1 { }");
    accepted_body("parallel; cores: 1..=4 { }");
    accepted_body("parallel; cores: -1; chunk: 1000 { }");
}

#[test]
fn invalid_parallel_cores_operand_type_is_rejected() {
    let output = rejected_body("parallel; cores: \"four\" { }");
    assert!(
        output.contains(codes::PARALLEL_OPTION_TYPE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_parallel_cores_zero_is_rejected() {
    let output = rejected_body("parallel; cores: 0 { }");
    assert!(
        output.contains(codes::PARALLEL_OPTION_TYPE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_parallel_chunk_operand_type_is_rejected() {
    let output = rejected_body("parallel; cores: 4; chunk: \"big\" { }");
    assert!(
        output.contains(codes::PARALLEL_OPTION_TYPE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_io_inside_a_parallel_region_is_rejected() {
    for body in [
        "parallel { Timer.sleep(1s); }",
        "parallel { concurrent { } }",
        "parallel { inmut h = spawn 1; }",
        "parallel { stdout.println(1); }",
    ] {
        let output = rejected_body(body);
        assert!(
            output.contains(codes::PARALLEL_REGION_IO.as_str()),
            "for `{body}`:\n{output}"
        );
    }
}

#[test]
fn invalid_shared_mutable_alias_into_a_parallel_region_is_rejected() {
    let output = rejected_body(
        "mut rows: List<Int32> = [1, 2, 3];\n\
         parallel { rows.push(4); }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn explicitly_unsafe_parallel_boundary_allows_an_unproven_mutable_alias() {
    accepted_body(
        "mut rows: List<Int32> = [1, 2, 3];\n\
         unsafe { parallel { rows[0] = 4; } }",
    );
}

#[test]
fn valid_strict_shared_or_copied_value_into_a_parallel_region_type_checks() {
    accepted_body(
        "inmut::strict rows: List<Int32> = [1, 2, 3];\n\
         parallel { inmut first: Int32 = rows[0]; }",
    );
    accepted_body("parallel { mut total: Int32 = 0; total = total + 1; }");
}

#[test]
fn valid_parallel_adapter_types_exactly_as_the_receiver() {
    accepted_body(
        "mut rows: List<Int32> = [1, 2, 3];\n\
         mut first: Int32 = rows.parallel[0];",
    );
    accepted_body("for i in (0..10).parallel { }");
}

#[test]
fn invalid_parallel_adapter_on_a_non_iterable_is_rejected() {
    let output = rejected_body("mut n: Int32 = 4; mut x = n.parallel;");
    assert!(
        output.contains(codes::UNKNOWN_MEMBER.as_str()) || output.contains("not"),
        "{output}"
    );
}

#[test]
fn valid_parallel_each_types_a_list() {
    accepted_body(
        "inmut::strict rows: List<Int32> = [1, 2, 3];\n\
         mut doubled: List<Int32> = Parallel.each(rows, (item: Int32): Int32 => item * 2);",
    );
}

#[test]
fn invalid_parallel_each_rejects_transitively_suspending_callback() {
    let output = rejected(
        "fn delay(): Void { Timer.sleep(1s); }\n\
         fn main(): Void {\n\
           mut rows: List<Int32> = [1];\n\
           _ = Parallel.each(rows, (item: Int32): Int32 => { delay(); return item; });\n\
         }",
    );
    assert!(
        output.contains(codes::PARALLEL_REGION_IO.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_parallel_each_rejects_an_unsafe_captured_reference() {
    let output = rejected(
        "class Counter {\n\
           mut value: Int32;\n\
           construct() { this.value = 0; }\n\
           increment(): Void { this.value += 1; }\n\
         }\n\
         fn main(): Void {\n\
           inmut::strict rows: List<Int32> = [1];\n\
           mut counter = Counter();\n\
           _ = Parallel.each(rows, (item: Int32): Int32 => { counter.increment(); return item; });\n\
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

// --- Recursive Transfer/Share derivation (concurrency-completion 2.3/2.6) ---

fn ownership_program(source_text: &str) -> zirk_sema::CheckedProgram {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("ownership.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    assert!(
        !sink.has_errors(),
        "fixture must type-check:\n{}",
        sink.render(RenderStyle::Human)
    );
    checked
}

fn ownership_of_class(
    checked: &zirk_sema::CheckedProgram,
    name: &str,
) -> zirk_sema::OwnershipProperties {
    let id = checked
        .classes
        .iter()
        .position(|class| class.name == name)
        .expect("class exists") as u32;
    checked.ownership_of(zirk_sema::Type::of(zirk_sema::Base::Class(id)))
}

#[test]
fn ownership_derivation_follows_aliases_nested_types_and_recursive_shapes() {
    let checked = ownership_program(
        "type Count = Int32;
         class ScalarBox { inmut value: Count; construct(value: Count) { this.value = value; } }
         class MutableBox { mut values: List<Map<Count, String>>; construct(values: List<Map<Count, String>>) { this.values = values; } }
         class Node { mut next: Node?; construct(next: Node?) { this.next = next; } }
         enum MaybeCount { None, Some(value: Count) }
         class GenericBox<T> { inmut value: T; construct(value: T) { this.value = value; } }
         class UsesGeneric { mut value: GenericBox<Count>; construct(value: GenericBox<Count>) { this.value = value; } }
         class HasChannel { mut value: Channel<Count>; construct(value: Channel<Count>) { this.value = value; } }
         fn main(): Void { }",
    );
    assert_eq!(
        ownership_of_class(&checked, "ScalarBox"),
        zirk_sema::OwnershipProperties {
            transfer: true,
            share: true
        }
    );
    assert_eq!(
        ownership_of_class(&checked, "MutableBox"),
        zirk_sema::OwnershipProperties {
            transfer: true,
            share: false
        }
    );
    assert_eq!(
        ownership_of_class(&checked, "Node"),
        zirk_sema::OwnershipProperties {
            transfer: true,
            share: false
        }
    );
    let enum_id = checked
        .enums
        .iter()
        .position(|en| en.name == "MaybeCount")
        .unwrap() as u32;
    assert_eq!(
        checked.ownership_of(zirk_sema::Type::of(zirk_sema::Base::Enum(enum_id))),
        zirk_sema::OwnershipProperties {
            transfer: true,
            share: true
        }
    );
    let generic_id = checked
        .generic_instances
        .iter()
        .position(|instance| {
            checked.classes[instance.class as usize].name == "GenericBox"
                && instance.args == [zirk_sema::Type::INT32]
        })
        .expect("the generic class is instantiated through UsesGeneric")
        as u32;
    assert_eq!(
        checked.ownership_of(zirk_sema::Type::of(zirk_sema::Base::Instance(generic_id,))),
        zirk_sema::OwnershipProperties {
            transfer: true,
            share: true
        }
    );
    let channel = checked
        .channel_types
        .iter()
        .position(|ty| *ty == zirk_sema::Type::INT32)
        .unwrap() as u32;
    assert_eq!(
        checked.ownership_of(zirk_sema::Type::of(zirk_sema::Base::Channel(channel))),
        zirk_sema::OwnershipProperties {
            transfer: true,
            share: true
        }
    );
}

#[test]
fn ownership_derivation_denies_resources_without_compiler_audited_transfer_marker() {
    let checked = ownership_program(
        "class OpenError implements Error {
             inmut reason: String;
             construct(reason: String) { this.reason = reason; }
             message(): String { return this.reason; }
             code(): String { return \"OPEN\"; }
             cause(): Error? { return null; }
         }
         class FakeFile implements Resource<OpenError> {
             inmut name: String;
             mut closed: Boolean;
             construct(name: String) { this.name = name; this.closed = false; }
             nothing(): Void { return; }
             close(): Result<Void, OpenError> { this.closed = true; return Ok(this.nothing()); }
             is_closed(): Boolean { return this.closed; }
         }
         class Plain implements TransferableResource { mut id: Int32; construct(id: Int32) { this.id = id; } }
         fn main(): Void { }",
    );
    assert_eq!(
        ownership_of_class(&checked, "FakeFile"),
        zirk_sema::OwnershipProperties {
            transfer: false,
            share: false
        }
    );
    assert_eq!(
        ownership_of_class(&checked, "Plain"),
        zirk_sema::OwnershipProperties {
            transfer: true,
            share: false
        }
    );
}

#[test]
fn ownership_derivation_fails_closed_for_unsupported_native_and_open_contract_shapes() {
    let checked = ownership_program(
        "contract Open { read(): Int32; }
         class HoldsOpen { inmut value: Open; construct(value: Open) { this.value = value; } }
         fn main(): Void { }",
    );
    assert_eq!(
        ownership_of_class(&checked, "HoldsOpen"),
        zirk_sema::OwnershipProperties {
            transfer: false,
            share: false
        }
    );
    let checked = ownership_program(
        "fn main(): Void { mut value: Int32 = 1; unsafe { mut p = Pointer.from(value); } }",
    );
    let id = checked
        .pointer_types
        .iter()
        .position(|_| true)
        .expect("pointer type interned") as u32;
    assert_eq!(
        checked.ownership_of(zirk_sema::Type::of(zirk_sema::Base::Pointer(id))),
        zirk_sema::OwnershipProperties {
            transfer: false,
            share: false
        }
    );
    assert_eq!(
        checked.ownership_of(zirk_sema::Type::UNKNOWN),
        zirk_sema::OwnershipProperties {
            transfer: false,
            share: false
        }
    );
    assert_eq!(
        checked.ownership_of(zirk_sema::Type::of(zirk_sema::Base::Regex)),
        zirk_sema::OwnershipProperties {
            transfer: false,
            share: false
        }
    );
}

#[test]
fn ownership_derivation_uses_lambda_identity_and_fails_closed_for_captures() {
    let checked = ownership_program(
        "fn main(): Void {
             mut value: Int32 = 1;
             mut captured = (): Int32 => value;
             mut standalone = (): Int32 => 2;
         }",
    );
    let (capturing_span, _) = checked
        .lambdas
        .iter()
        .find(|(_, lambda)| !lambda.captures.is_empty())
        .expect("capturing closure is recorded");
    assert_eq!(
        checked.ownership_of_lambda(*capturing_span),
        zirk_sema::OwnershipProperties {
            transfer: false,
            share: false
        }
    );
    let (empty_span, _) = checked
        .lambdas
        .iter()
        .find(|(_, lambda)| lambda.captures.is_empty())
        .expect("captureless closure is recorded");
    assert_eq!(
        checked.ownership_of_lambda(*empty_span),
        zirk_sema::OwnershipProperties {
            transfer: true,
            share: true
        }
    );
}

#[test]
fn invalid_parallel_each_argument_count_is_rejected() {
    let output = rejected_body("mut rows: List<Int32> = [1]; Parallel.each(rows);");
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_parallel_each_non_iterable_first_argument_is_rejected() {
    let output = rejected_body("Parallel.each(4, (item: Int32): Int32 => item);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_parallel_each_callback_arity_is_rejected() {
    let output = rejected_body(
        "mut rows: List<Int32> = [1]; Parallel.each(rows, (a: Int32, b: Int32): Int32 => a);",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_io_outside_a_parallel_region_still_type_checks() {
    accepted_body(
        "parallel { }\n\
         Timer.sleep(1s);\n\
         concurrent { }\n\
         stdout.println(1);",
    );
}

#[test]
fn valid_promoted_constructor_parameter_is_a_field() {
    accepted(
        "class User { construct(public name: String) { } greet(): String { return this.name; } } fn main(): Void { mut user = User(name: \"Ana\"); stdout.println(user.greet()); }",
    );
}

#[test]
fn invalid_promoted_constructor_parameter_duplicates_a_field() {
    let output = rejected(
        "class User { name: String; construct(public name: String) { } } fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_PROMOTED_FIELD.as_str()),
        "{output}"
    );
}

#[test]
fn valid_class_without_a_construct_has_an_implicit_zero_argument_constructor() {
    accepted("class Marker { } fn main(): Void { mut marker = Marker(); }");
}

#[test]
fn invalid_arguments_to_an_implicit_constructor_are_rejected() {
    let output = rejected("class Marker { } fn main(): Void { mut marker = Marker(1); }");
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn promoted_field_use_outside_its_constructor_is_a_warning() {
    let output = accepted_with_warnings(
        "class User { construct(public name: String) { } greet(): String { return this.name; } } fn main(): Void { mut user = User(\"Ana\"); stdout.println(user.greet()); }",
    );
    assert!(
        output.contains(codes::PROMOTED_FIELD_LEGIBILITY.as_str()),
        "{output}"
    );
}

#[test]
fn promoted_field_write_outside_its_constructor_is_a_warning() {
    let output = accepted_with_warnings(
        "class Counter { construct(public mut value: Int32) { this.value = value; } bump(): Void { this.value = 1; } } fn main(): Void { mut counter = Counter(0); counter.bump(); }",
    );
    assert!(
        output.contains(codes::PROMOTED_FIELD_LEGIBILITY.as_str()),
        "{output}"
    );
}

#[test]
fn promoted_variadic_constructor_parameters_are_rejected() {
    let variadic =
        rejected("class C { construct(public ...items: List<Int32>) { } } fn main(): Void { }");
    assert!(
        variadic.contains(codes::PROMOTED_VARIADIC.as_str()),
        "{variadic}"
    );
    let legacy = rejected("record C { construct(public item: Int32) { } fn main(): Void { }");
    assert!(
        legacy.contains(parser_codes::LEGACY_RECORD.as_str()),
        "{legacy}"
    );
}

#[test]
fn promoted_parameters_support_named_optional_default_and_generic_construction() {
    accepted(
        "class User { construct(public name: String, public age?: Int32, public role: String = \"member\") { } } class Box<T> { construct(public value: T) { } } fn main(): Void { mut user = User(role: \"admin\", name: \"Ana\"); mut box: Box<Int32> = Box(value: 1); }",
    );
}

#[test]
fn promoted_inmut_field_is_initialized_but_cannot_be_mutated_later() {
    let output = rejected(
        "class User { construct(public inmut id: Int32) { } change(): Void { this.id = 2; } } fn main(): Void { mut user = User(1); user.change(); }",
    );
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn implicit_subclass_constructor_calls_an_accessible_zero_argument_base() {
    let output = accepted_with_warnings(
        "class Base { construct() { } } class Child extends Base { } fn main(): Void { mut child = Child(); }",
    );
    assert!(output.contains(codes::IMPLICIT_SUPER.as_str()), "{output}");
}

#[test]
fn implicit_subclass_constructor_reports_an_unsatisfied_base_at_instantiation() {
    let output = rejected(
        "class Base { construct(value: Int32) { } } class Child extends Base { } fn main(): Void { mut child = Child(); }",
    );
    assert!(
        output.contains(codes::UNSATISFIABLE_IMPLICIT_CONSTRUCTION.as_str()),
        "{output}"
    );
}

// --- Atomic<T> semantic surface (concurrency-completion tasks 7.3/9.1) ---

#[test]
fn atomic_supported_scalar_types_and_operations_typecheck() {
    accepted_body(
        "mut signed: Atomic<Int32> = Atomic(0);\n\
         mut loaded: Int32 = signed.load();\n\
         signed.store(value: 1);\n\
         mut old: Int32 = signed.exchange(value: 2);\n\
         mut result: Result<Int32, Int32> = signed.compare_exchange(expected: 2, desired: 3);\n\
         mut added: Int32 = signed.fetch_add(value: 1);\n\
         mut subtracted: Int32 = signed.fetch_sub(value: 1);\n\
         mut boolean: Atomic<Boolean> = Atomic(true);\n\
         mut seed: UInt64 = 1;\n\
         mut unsigned: Atomic<UInt64> = Atomic(seed);\n\
         mut i8: Atomic<Int8> = Atomic(1);\n\
         mut i16: Atomic<Int16> = Atomic(1);\n\
         mut i64: Atomic<Int64> = Atomic(1);\n\
         mut u8: Atomic<UInt8> = Atomic(1);\n\
         mut u16: Atomic<UInt16> = Atomic(1);\n\
         mut u32: Atomic<UInt32> = Atomic(1);",
    );
}

#[test]
fn atomic_weaker_orders_are_accepted_inside_unsafe() {
    accepted_body(
        "mut value: Atomic<Int32> = Atomic(0);\n\
         unsafe {\n\
             mut loaded: Int32 = value.load(order: AtomicOrder.acquire);\n\
             value.store(value: 1, order: AtomicOrder.release);\n\
             mut exchanged: Int32 = value.exchange(2, AtomicOrder.acq_rel);\n\
             mut prior: Int32 = value.fetch_add(1, AtomicOrder.relaxed);\n\
             mut result: Result<Int32, Int32> = value.compare_exchange(0, 1, AtomicOrder.acq_rel, AtomicOrder.acquire);\n\
         }",
    );
}

#[test]
fn atomic_rejects_unsupported_payload_types() {
    for source in [
        "mut value: Atomic<String> = Atomic(\"x\");",
        "mut value: Atomic<Float> = Atomic(1.0);",
        "mut value: Atomic<Int128> = Atomic(1i128);",
        "mut value: Atomic<UInt128> = Atomic(1u128);",
        "mut value: Atomic<Int32, Int32> = Atomic(1);",
    ] {
        let output = rejected_body(source);
        assert!(
            output.contains(codes::TYPE_MISMATCH.as_str())
                || output.contains(codes::UNKNOWN_TYPE.as_str()),
            "{source}\n{output}"
        );
    }
    for source in [
        "fn consume(value: Atomic<Pointer<Int32>>): Void { } fn main(): Void { }",
        "fn consume(value: Atomic<Array<Int32>>): Void { } fn main(): Void { }",
    ] {
        let output = rejected(source);
        assert!(
            output.contains(codes::TYPE_MISMATCH.as_str()),
            "{source}\n{output}"
        );
    }
}

#[test]
fn atomic_fetch_arithmetic_is_integer_only_and_values_match_payload() {
    let output = rejected_body("mut value: Atomic<Boolean> = Atomic(true); value.fetch_add(true);");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");

    let output = rejected_body("mut value: Atomic<Int32> = Atomic(0); value.store(value: true);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");

    let output = rejected_body("mut value: Atomic<Int32> = Atomic(true);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn atomic_weaker_orders_require_unsafe_context() {
    let output =
        rejected_body("mut value: Atomic<Int32> = Atomic(0); value.load(AtomicOrder.relaxed);");
    assert!(
        output.contains("non-sequentially-consistent atomic ordering"),
        "{output}"
    );
}

#[test]
fn atomic_load_and_store_reject_operation_incompatible_orders() {
    for body in [
        "mut value: Atomic<Int32> = Atomic(0); unsafe { value.load(AtomicOrder.release); }",
        "mut value: Atomic<Int32> = Atomic(0); unsafe { value.store(1, AtomicOrder.acquire); }",
        "mut value: Atomic<Int32> = Atomic(0); unsafe { value.load(AtomicOrder.acq_rel); }",
        "mut value: Atomic<Int32> = Atomic(0); unsafe { value.store(1, AtomicOrder.acq_rel); }",
    ] {
        let output = rejected_body(body);
        assert!(
            output.contains("ordering") && output.contains("invalid for Atomic"),
            "{body}\n{output}"
        );
    }
}

#[test]
fn atomic_compare_exchange_rejects_invalid_failure_order_pairs() {
    for body in [
        "mut value: Atomic<Int32> = Atomic(0); unsafe { _ = value.compare_exchange(0, 1, AtomicOrder.release, AtomicOrder.acquire); }",
        "mut value: Atomic<Int32> = Atomic(0); unsafe { _ = value.compare_exchange(0, 1, AtomicOrder.relaxed, AtomicOrder.acquire); }",
        "mut value: Atomic<Int32> = Atomic(0); unsafe { _ = value.compare_exchange(0, 1, AtomicOrder.seq_cst, AtomicOrder.release); }",
        "mut value: Atomic<Int32> = Atomic(0); unsafe { _ = value.compare_exchange(0, 1, AtomicOrder.acq_rel, AtomicOrder.acq_rel); }",
    ] {
        let output = rejected_body(body);
        assert!(
            output.contains("invalid compare_exchange failure ordering"),
            "{body}\n{output}"
        );
    }
}

#[test]
fn atomic_compare_exchange_named_orders_resolve_by_label_and_apply_defaults() {
    accepted_body(
        "mut value: Atomic<Int32> = Atomic(0);\n\
         unsafe { mut result: Result<Int32, Int32> = value.compare_exchange(desired: 1, expected: 0, failure: AtomicOrder.acquire, success: AtomicOrder.acq_rel); }",
    );
    let output = rejected_body(
        "mut value: Atomic<Int32> = Atomic(0); unsafe { _ = value.compare_exchange(expected: 0, desired: 1, success: AtomicOrder.acquire); }",
    );
    assert!(
        output.contains("invalid compare_exchange failure ordering"),
        "{output}"
    );
}

#[test]
fn atomic_seq_cst_is_safe_without_unsafe_and_dynamic_weak_order_is_not() {
    accepted_body(
        "mut value: Atomic<Int32> = Atomic(0); mut observed: Int32 = value.load(AtomicOrder.seq_cst);",
    );
    let output = rejected_body(
        "mut value: Atomic<Int32> = Atomic(0); mut order: AtomicOrder = AtomicOrder.relaxed; value.load(order);",
    );
    assert!(
        output.contains("non-sequentially-consistent atomic ordering"),
        "{output}"
    );
}

#[test]
fn atomic_rejects_bad_arity_and_unknown_operations() {
    let output = rejected_body("mut value: Atomic<Int32> = Atomic(0); value.load(1, 2);");
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
    let output = rejected_body("mut value: Atomic<Int32> = Atomic(0); value.swap(2);");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn service_callback_semantics_record_sites_for_lowering() {
    let source_text = "fn main(): Void { application.spawn_service((context: ServiceContext): Void => { context.ready(); }); }";
    let mut sources = SourceMap::new();
    let file = sources.add(SourceFile::new("service.zrk", source_text));
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(sources.file(file), &mut sink);
    let program = parse(sources.file(file), &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let output = sink.render(RenderStyle::Human);

    assert!(!output.contains(codes::NOT_LOWERED.as_str()), "{output}");
    assert!(
        !output.contains(codes::PENDING_FEATURE.as_str()),
        "{output}"
    );
    assert_eq!(checked.service_spawns.len(), 1, "{output}");
    assert_eq!(checked.service_ready_calls.len(), 1, "{output}");
    let spawn = checked.service_spawns.values().next().unwrap();
    assert_eq!(
        spawn.capacity_error,
        Type::of(Base::Class(
            checked.native_exceptions.unwrap().service_capacity_error
        ))
    );
    assert_eq!(
        checked.service_ready_calls.values().next(),
        Some(&spawn.context_parameter)
    );
}

#[test]
fn service_context_is_bound_to_callback_parameter_and_cannot_escape() {
    let outside = rejected(
        "fn ordinary(context: ServiceContext): Void { context.ready(); } fn main(): Void { }",
    );
    assert!(outside.contains(codes::TYPE_MISMATCH.as_str()), "{outside}");

    for body in [
        "mut saved: ServiceContext = context;",
        "return context;",
        "throw context;",
        "mut aggregate = (context,);",
        "mut nested = (): Void => { context.ready(); };",
    ] {
        let source = format!(
            "fn serve(context: ServiceContext): Void {{ {body} }} fn main(): Void {{ application.spawn_service(serve); }}"
        );
        let output = rejected(&source);
        assert!(
            output.contains("ServiceContext cannot escape")
                || output.contains(codes::TYPE_MISMATCH.as_str()),
            "{body}: {output}"
        );
    }
}

#[test]
fn open_object_literal_infers_object_and_checks_interpolated_keys() {
    accepted(
        "fn build(dynamic_key: String): Void { mut data = { status: \"ok\", \"{dynamic_key}\": true }; } fn main(): Void { }",
    );
}

#[test]
fn object_view_preserves_class_widening_and_dynamic_member_reads() {
    accepted(
        "class Box { } fn widen(value: Box): Object { return value; } fn read(value: Object): Dynamic { return value.status; } fn read_computed(value: Object): Dynamic { return value[\"status\"]; } fn narrow(value: Object): Box? { return value as? Box; } fn narrow_dynamic(value: Object): String? { return value.status as? String; } fn require_dynamic(value: Object): String { return value.status as String; } fn main(): Void { }",
    );
}

#[test]
fn open_object_property_mutation_respects_reference_mutability() {
    accepted(
        "fn main(): Void { inmut value = { status: \"initial\" }; value.status = \"ready\"; value[\"count\"] = 2; }",
    );
    let output = rejected(
        "fn main(): Void { inmut::strict value = { status: \"initial\" }; value.status = \"ready\"; }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn object_literals_check_known_contract_properties_statically() {
    accepted(
        "contract Named { name: String; } fn main(): Void { inmut value: Named = { name: \"Ada\" }; }",
    );

    let missing = rejected(
        "contract Named { name: String; } fn main(): Void { inmut value: Named = { age: 37 }; }",
    );
    assert!(
        missing.contains("missing contract property `name`"),
        "{missing}"
    );

    let wrong_type = rejected(
        "contract Named { name: String; } fn main(): Void { inmut value: Named = { name: 37 }; }",
    );
    assert!(
        wrong_type.contains(codes::TYPE_MISMATCH.as_str()),
        "{wrong_type}"
    );
}

#[test]
fn object_literal_callable_properties_satisfy_contract_operations() {
    accepted(
        "contract Runnable { run(): Int32; }
         fn run_once(): Int32 { return 1; }
         fn main(): Void { inmut runner: Runnable = { run: run_once }; }",
    );

    let mismatch = rejected(
        "contract Runnable { run(value: Int32): Int32; }
         fn run_without_arguments(): Int32 { return 1; }
         fn main(): Void { inmut runner: Runnable = { run: run_without_arguments }; }",
    );
    assert!(
        mismatch.contains("operation `run`") || mismatch.contains("contract"),
        "{mismatch}"
    );
}

#[test]
fn generic_type_aliases_substitute_arguments_defaults_and_constraints() {
    accepted(
        "type Boxed<T> = T;\n\
         type Pair<A, B = A> = A;\n\
         fn take(value: Boxed<Int32>): Void { }\n\
         fn main(): Void { mut value: Pair<Int32> = 1; take(value); }",
    );

    let missing = rejected("type Boxed<T> = T; fn main(): Void { mut value: Boxed; }");
    assert!(
        missing.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{missing}"
    );

    let extra = rejected("type Boxed<T> = T; fn main(): Void { mut value: Boxed<Int32, String>; }");
    assert!(
        extra.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{extra}"
    );

    let constrained = rejected(
        "type Numeric<T from Int32> = T;\n\
         fn main(): Void { mut value: Numeric<String> = \"no\"; }",
    );
    assert!(
        constrained.contains(codes::TYPE_MISMATCH.as_str()),
        "{constrained}"
    );
}

#[test]
fn checked_union_records_transitive_source_aliases_for_diagnostics() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "aliases.zrk",
        "type InternalValue = Int32 | String;\n\
         type PublicValue = InternalValue;\n\
         fn take(value: PublicValue): Void { }\n\
         fn main(): Void { take(1); }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert!(!sink.has_errors(), "{}", sink.render(RenderStyle::Human));
    let aliases = checked
        .union_aliases
        .values()
        .find(|aliases| aliases.contains(&"PublicValue".to_string()))
        .expect("the resolved union carries the public alias provenance");
    assert_eq!(aliases, &["InternalValue", "PublicValue"]);
}

#[test]
fn union_type_mismatch_diagnostic_preserves_alias_and_expansion() {
    let output = rejected(
        "type InternalValue = Int32 | String;\n\
         type PublicValue = InternalValue;\n\
         fn take(value: PublicValue): Void { }\n\
         fn main(): Void { take(true); }",
    );
    assert!(output.contains("PublicValue"), "{output}");
    assert!(output.contains("Int32 | String"), "{output}");
}

#[test]
fn generic_union_alias_substitution_checks_each_argument_alternative() {
    let valid = "type Choice<T> = T | Int32;\n\
         fn take(value: Choice<String>): Void { }\n\
         fn main(): Void { take(\"text\"); take(1); }";
    let (_, valid_diagnostics) = parse_and_check(valid);
    let valid_output = valid_diagnostics.render(RenderStyle::Human);
    assert!(
        !valid_output.contains(codes::TYPE_MISMATCH.as_str()),
        "valid generic union alternatives should type-check: {valid_output}"
    );

    let invalid = "type Choice<T> = T | Int32;\n\
         fn take(value: Choice<String>): Void { }\n\
         fn main(): Void { take(true); }";
    let (_, invalid_diagnostics) = parse_and_check(invalid);
    let invalid_output = invalid_diagnostics.render(RenderStyle::Human);
    assert!(
        invalid_output.contains(codes::TYPE_MISMATCH.as_str()),
        "an incompatible argument must not fit a substituted generic union: {invalid_output}"
    );

    let returns = "type Choice<T> = T | Int32;\n\
         fn valid(): Choice<String> { return \"text\"; }\n\
         fn invalid(): Choice<String> { return true; }\n\
         fn main(): Void { }";
    let (_, return_diagnostics) = parse_and_check(returns);
    let return_output = return_diagnostics.render(RenderStyle::Human);
    assert!(
        return_output.contains(codes::TYPE_MISMATCH.as_str()),
        "return expressions must use the same substituted union relation: {return_output}"
    );
    assert!(return_output.contains("the returned type does not match the signature"));
}

#[test]
fn generic_alias_substitution_preserves_canonical_union_and_intersection_types() {
    let source = "type Choice<T> = T | Int32;\n\
         type SameChoice = String | Int32;\n\
         type TextView<T> = T & String;\n\
         type SameText = String & String;\n\
         fn take_choice(value: SameChoice): Void { }\n\
         fn take_text(value: SameText): Void { }\n\
         fn pass_aliases(choice: Choice<String>, text: TextView<String>): Void {\n\
             take_choice(choice);\n\
             take_text(text);\n\
         }\n\
         fn main(): Void { }";
    let (_, sink) = parse_and_check(source);
    let output = sink.render(RenderStyle::Human);
    assert!(
        !output.contains(codes::TYPE_MISMATCH.as_str()),
        "substituting generic parameters must canonicalize to the same union/intersection types as direct expressions: {output}"
    );
}

#[test]
fn keyof_returns_finite_visible_structural_and_tuple_keys() {
    fn checked_program(source_text: &str) -> (zirk_sema::CheckedProgram, DiagnosticSink) {
        let mut sources = SourceMap::new();
        sources.add(SourceFile::new("keyof.zrk", source_text));
        let source = sources.entry();
        let mut sink = DiagnosticSink::new();
        let tokens = tokenize(source, &mut sink);
        let program = parse(source, &tokens, &mut sink);
        let checked = check(&sources, &program, &mut sink);
        (checked, sink)
    }

    let source = "type Profile = { name: String; enabled: Boolean };\n\
         type ProfileKey = keyof Profile;\n\
         type Pair = Tuple<Int32, String>;\n\
         type PairKey = keyof Pair;\n\
         fn profile_key(key: ProfileKey): Void { }\n\
         fn pair_key(key: PairKey): Void { }\
         fn main(): Void { }";
    let (checked, sink) = checked_program(source);
    let output = sink.render(RenderStyle::Human);
    assert!(
        !output.contains(codes::TYPE_MISMATCH.as_str()),
        "keyof must produce canonical string singleton keys: {output}"
    );
    let profile_keys = checked.describe_type(checked.functions["profile_key"].params[0].ty);
    assert!(profile_keys.contains("enabled") && profile_keys.contains("name"));
    let pair_keys = checked.describe_type(checked.functions["pair_key"].params[0].ty);
    assert!(
        pair_keys.contains("\"0\"") && pair_keys.contains("\"1\"") && pair_keys.contains("length")
    );

    let private = "class Profile { public name: String; private token: String; describe(): String { return name; } }\n\
         type ProfileKey = keyof Profile;\n\
         fn take(key: ProfileKey): Void { }\n\
         fn main(): Void { }";
    let (private_checked, private_sink) = checked_program(private);
    let private_output = private_sink.render(RenderStyle::Human);
    let private_keys =
        private_checked.describe_type(private_checked.functions["take"].params[0].ty);
    assert!(
        private_keys.contains("name")
            && private_keys.contains("describe")
            && !private_keys.contains("token"),
        "private fields must not be exposed as keys; resolved {private_keys}: {private_output}"
    );

    let dynamic = rejected(
        "type RuntimeKey = keyof Dynamic; fn dynamic(key: RuntimeKey): Void { } fn main(): Void { }",
    );
    assert!(
        dynamic.contains("runtime-only properties do not form a finite static key set"),
        "Dynamic must not turn runtime properties into static keys: {dynamic}"
    );
}

#[test]
fn indexed_access_resolves_finite_keys_optional_properties_readonly_fields_and_union_receivers() {
    fn checked_program(source_text: &str) -> (zirk_sema::CheckedProgram, DiagnosticSink) {
        let mut sources = SourceMap::new();
        sources.add(SourceFile::new("indexed-access.zrk", source_text));
        let source = sources.entry();
        let mut sink = DiagnosticSink::new();
        let tokens = tokenize(source, &mut sink);
        let program = parse(source, &tokens, &mut sink);
        let checked = check(&sources, &program, &mut sink);
        (checked, sink)
    }

    let source = "type Shape = { name: String; maybe?: Int32; inmut title: String; active: Boolean };\n\
         type AllValues = Shape[keyof Shape];\n\
         type MaybeValue = Shape[\"maybe\"];\n\
         type ReadonlyValue = Shape[\"title\"];\n\
         type Left = { shared: Boolean; left: String };\n\
         type Right = { shared: Boolean; right: Int32 };\n\
         type Choice = Left | Right;\n\
         type Shared = Choice[\"shared\"];\n\
         fn values(all: AllValues, maybe: MaybeValue, title: ReadonlyValue, shared: Shared): Void { }\n\
         fn main(): Void { }";
    let (checked, sink) = checked_program(source);
    let output = sink.render(RenderStyle::Human);
    assert!(
        !output.contains(codes::UNKNOWN_MEMBER.as_str())
            && !output.contains(codes::TYPE_MISMATCH.as_str()),
        "valid indexed reads must resolve across finite keys and compatible union members: {output}"
    );
    let signature = &checked.functions["values"];
    let all_values = checked.describe_type(signature.params[0].ty);
    assert!(
        all_values.contains("String")
            && all_values.contains("Int32")
            && all_values.contains("Boolean"),
        "unexpected indexed union: {all_values}"
    );
    assert!(checked.describe_type(signature.params[1].ty).ends_with('?'));
    assert_eq!(checked.describe_type(signature.params[2].ty), "String");
    assert_eq!(checked.describe_type(signature.params[3].ty), "Boolean");

    let invalid_key = rejected(
        "type Shape = { name: String }; type Value = Shape[\"missing\"]; fn read(value: Value): Void { }",
    );
    assert!(
        invalid_key.contains(codes::UNKNOWN_MEMBER.as_str()),
        "keys absent from the receiver must be rejected: {invalid_key}"
    );

    let invalid_union_key = rejected(
        "type Left = { left: String }; type Right = { right: Int32 }; type Choice = Left | Right; type Value = Choice[\"left\"]; fn read(value: Value): Void { }",
    );
    assert!(
        invalid_union_key.contains(codes::UNKNOWN_MEMBER.as_str()),
        "an indexed key must exist on every possible union receiver: {invalid_union_key}"
    );
}

#[test]
fn conditional_types_distribute_only_for_naked_generic_parameters() {
    let source = "type Distributed<T> = T extends String ? String : Boolean;\n\
         type Grouped<T> = (T) extends String ? String : Boolean;\n\
         type Input = String | Int32;\n\
         type Each = Distributed<Input>;\n\
         type Whole = Grouped<Input>;\n\
         fn results(each: Each, whole: Whole): Void { }\n\
         fn main(): Void { }";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("conditional.zrk", source));
    let source_file = sources.entry();
    let mut diagnostics = DiagnosticSink::new();
    let tokens = tokenize(source_file, &mut diagnostics);
    let program = parse(source_file, &tokens, &mut diagnostics);
    let output = check(&sources, &program, &mut diagnostics);
    assert!(
        diagnostics.is_empty(),
        "conditional aliases should type-check: {}",
        diagnostics.render(RenderStyle::Human)
    );
    let signature = &output.functions["results"];
    let distributed = output.describe_type(signature.params[0].ty);
    let grouped = output.describe_type(signature.params[1].ty);
    assert!(
        distributed.contains("String") && distributed.contains("Boolean"),
        "expected distribution, got {distributed}"
    );
    assert_eq!(
        grouped, "Boolean",
        "a grouped type parameter must be checked as a whole"
    );
}

#[test]
fn conditional_infer_binds_generic_components_only_in_true_branch() {
    let source = "type Element<T> = T extends Array<infer Item> ? Item : Boolean;\n\
         type TextElement<T> = T extends Array<infer Item from String> ? Item : Boolean;\n\
         type Found = Element<Array<String>>;\n\
         type Missing = Element<Int32>;\n\
         type Constrained = TextElement<Array<Int32>>;\n\
         fn results(found: Found, missing: Missing, constrained: Constrained): Void { }\n\
         fn main(): Void { }";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("conditional-infer.zrk", source));
    let source_file = sources.entry();
    let mut diagnostics = DiagnosticSink::new();
    let tokens = tokenize(source_file, &mut diagnostics);
    let program = parse(source_file, &tokens, &mut diagnostics);
    let checked = check(&sources, &program, &mut diagnostics);
    assert!(
        diagnostics.is_empty(),
        "conditional infer should type-check: {}",
        diagnostics.render(RenderStyle::Human)
    );
    let signature = &checked.functions["results"];
    assert_eq!(checked.describe_type(signature.params[0].ty), "String");
    assert_eq!(checked.describe_type(signature.params[1].ty), "Boolean");
    assert_eq!(checked.describe_type(signature.params[2].ty), "Boolean");

    let escaped = rejected(
        "type Leaks<T> = T extends Array<infer Item> ? String : Item; type Result = Leaks<Int32>; fn read_result(value: Result): Void { }",
    );
    assert!(
        escaped.contains(codes::UNKNOWN_TYPE.as_str()),
        "infer bindings must not escape the true branch: {escaped}"
    );
}

#[test]
fn mapped_types_transform_finite_keys_and_keep_source_modifiers() {
    let source = "type Source = { name: String; maybe?: Int32; inmut active: Boolean };\n\
         type Optional<T> = { [K in keyof T]?: T[K] };\n\
         type Frozen<T> = { inmut [K in keyof T]: T[K] };\n\
         type MutableRequired<T> = { mut [K in keyof T]!: T[K] };\n\
         type Renamed<T> = { [K in keyof T as K]: T[K] };\n\
         type PartialSource = Optional<Source>;\n\
         type FrozenSource = Frozen<Source>;\n\
         type MutableSource = MutableRequired<Source>;\n\
         type RenamedSource = Renamed<Source>;\n\
         fn values(partial: PartialSource, frozen: FrozenSource, mutable: MutableSource, renamed: RenamedSource): Void { }\n\
         fn main(): Void { }";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("mapped.zrk", source));
    let source_file = sources.entry();
    let mut diagnostics = DiagnosticSink::new();
    let tokens = tokenize(source_file, &mut diagnostics);
    let program = parse(source_file, &tokens, &mut diagnostics);
    let checked = check(&sources, &program, &mut diagnostics);
    let rendered = diagnostics.render(RenderStyle::Human);
    assert!(
        !rendered.contains("mapped type produced a duplicate key"),
        "identity remapping should preserve unique keys: {rendered}"
    );
    let signature = &checked.functions["values"];
    let partial = checked.describe_type(signature.params[0].ty);
    let frozen = checked.describe_type(signature.params[1].ty);
    let mutable = checked.describe_type(signature.params[2].ty);
    let renamed = checked.describe_type(signature.params[3].ty);
    assert!(
        partial.contains("name?: String") && partial.contains("maybe?: Int32?"),
        "optional mapping should preserve optionality: {partial}"
    );
    assert!(
        frozen.contains("inmut name: String") && frozen.contains("inmut maybe?: Int32?"),
        "readonly mapping should preserve and add readonly: {frozen}"
    );
    assert!(
        mutable.contains("active: Boolean")
            && !mutable.contains("inmut")
            && mutable.contains("maybe: Int32"),
        "mutable required mapping should remove source modifiers: {mutable}"
    );
    assert!(
        renamed.contains("name: String"),
        "identity key remapping should retain its key: {renamed}"
    );
    let duplicate = rejected(
        "type Source = { first: String; second: Int32 }; type Collapsed = { [K in keyof Source as \"field\"]: Source[K] }; fn read(value: Collapsed): Void { }",
    );
    assert!(
        duplicate.contains("mapped type produced a duplicate key"),
        "duplicate remapped keys need a deterministic error: {duplicate}"
    );
}

#[test]
fn template_literal_types_expand_finite_string_numeric_and_boolean_unions() {
    let source = "type Color = \"red\" | \"blue\";\n\
         type Number = 2 | 7;\n\
         type Flag = true | false;\n\
         type Names = \"color-{Color}\";\n\
         type Numbers = \"n{Number}\";\n\
         type Flags = \"flag-{Flag}\";\n\
         fn values(names: Names, numbers: Numbers, flags: Flags): Void { }\n\
         fn main(): Void { }";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("template-types.zrk", source));
    let source_file = sources.entry();
    let mut diagnostics = DiagnosticSink::new();
    let tokens = tokenize(source_file, &mut diagnostics);
    let program = parse(source_file, &tokens, &mut diagnostics);
    let checked = check(&sources, &program, &mut diagnostics);
    let rendered = diagnostics.render(RenderStyle::Human);
    assert!(
        !rendered.contains("template interpolation requires")
            && !rendered.contains("template literal expansion limit"),
        "finite values should expand: {rendered}"
    );
    let signature = &checked.functions["values"];
    let names = checked.describe_type(signature.params[0].ty);
    let numbers = checked.describe_type(signature.params[1].ty);
    let flags = checked.describe_type(signature.params[2].ty);
    assert!(
        names.contains("color-red") && names.contains("color-blue"),
        "unexpected template keys: {names}"
    );
    assert!(
        numbers.contains("n2") && numbers.contains("n7"),
        "unexpected numeric expansion: {numbers}"
    );
    assert!(
        flags.contains("flag-true") && flags.contains("flag-false"),
        "unexpected Boolean expansion: {flags}"
    );
    let rejected_dynamic =
        rejected("type DynamicName = \"x{String}\"; fn read(value: DynamicName): Void { }");
    assert!(
        rejected_dynamic.contains("template interpolation requires a finite singleton"),
        "unbounded interpolation must be rejected: {rejected_dynamic}"
    );
    let placeholders = "{Flag}".repeat(13);
    let stress = format!(
        "type Flag = Boolean; type Product = \"{placeholders}\"; fn use_product(value: Product): Void {{ }}"
    );
    let limited = rejected(&stress);
    assert!(
        limited.contains("template literal expansion limit exceeded"),
        "cross products must stop at the deterministic budget: {limited}"
    );
}

#[test]
fn open_template_inference_resolves_keyof_and_contextual_indexed_callbacks() {
    fn checked(source_text: &str) -> (zirk_sema::CheckedProgram, DiagnosticSink) {
        let mut sources = SourceMap::new();
        sources.add(SourceFile::new("open-template.zrk", source_text));
        let source = sources.entry();
        let mut sink = DiagnosticSink::new();
        let tokens = tokenize(source, &mut sink);
        let program = parse(source, &tokens, &mut sink);
        let program = check(&sources, &program, &mut sink);
        (program, sink)
    }

    let valid = "fn on<T, K from String & keyof T>(value: T, event: \"{K}Changed\", callback: Fn(T[K]) => Void): Void { }\n\
        fn main(): Void {\n\
            inmut person: { age: Int32 } = { age: 1 };\n\
            on(person, \"ageChanged\", (age): Void => { inmut number: Int32 = age; });\n\
        }";
    let (_, diagnostics) = checked(valid);
    let output = diagnostics.render(RenderStyle::Human);
    assert!(
        diagnostics.is_empty(),
        "a fixed-suffix event should infer K and contextualize T[K] as Int32: {output}"
    );

    let wrong_callback = valid.replace("inmut number: Int32 = age", "inmut text: String = age");
    let (_, diagnostics) = checked(&wrong_callback);
    let output = diagnostics.render(RenderStyle::Human);
    assert!(
        output.contains(codes::TYPE_MISMATCH.as_str()),
        "callback parameter must use the indexed property type: {output}"
    );

    let missing_key = valid.replace("ageChanged", "heightChanged");
    let (program, diagnostics) = checked(&missing_key);
    let output = diagnostics.render(RenderStyle::Human);
    assert!(
        output.contains("has no property `height`")
            || output.contains("inferred key does not exist"),
        "a key absent from T must be rejected at the event argument: {output}"
    );
    assert!(
        program.generic_call_substitutions.is_empty(),
        "a failed open-template relation must not publish a partial generic substitution"
    );

    let broad_string = "fn on<T, K from String & keyof T>(value: T, event: \"{K}Changed\", callback: Fn(T[K]) => Void): Void { }\n\
        fn main(): Void {\n\
            inmut person: { age: Int32 } = { age: 1 };\n\
            inmut event: String = \"ageChanged\";\n\
            on(person, event, (age): Void => { });\n\
        }";
    let (_, diagnostics) = checked(broad_string);
    let output = diagnostics.render(RenderStyle::Human);
    assert!(
        output.contains("broad `String` cannot determine the template key"),
        "open interpolation inference must not guess from broad String: {output}"
    );

    let wrong_suffix = valid.replace("ageChanged", "ageUpdated");
    let (_, diagnostics) = checked(&wrong_suffix);
    let output = diagnostics.render(RenderStyle::Human);
    assert!(
        output.contains("does not match `on`'s open template"),
        "a nonmatching suffix needs a focused diagnostic: {output}"
    );

    let multiple_holes =
        rejected("fn take<K>(event: \"{K}Changed{K}\"): Void { } fn main(): Void { }");
    assert!(
        multiple_holes.contains("open template inference supports one interpolation hole"),
        "multiple open holes must be rejected deterministically: {multiple_holes}"
    );

    let ambiguous_prefix = rejected(
        "type Prefix = \"a\" | \"b\"; fn take<K>(event: \"{Prefix}{K}Changed\"): Void { } fn main(): Void { }",
    );
    assert!(
        ambiguous_prefix.contains("open template pattern has an ambiguous prefix"),
        "finite alternatives before an open hole must not pick a prefix arbitrarily: {ambiguous_prefix}"
    );

    let conflict = rejected(
        "fn take<T, K from String & keyof T>(value: T, key: K, event: \"{K}Changed\"): Void { }\n\
         fn main(): Void { inmut person: { age: Int32; name: String } = { age: 1, name: \"Ada\" }; inmut key: \"name\" = \"name\"; take(person, key, \"ageChanged\"); }",
    );
    assert!(
        conflict.contains("cannot infer one template key"),
        "ordinary and template-derived constraints must agree: {conflict}"
    );
}

#[test]
fn generic_contract_method_infers_open_template_event_and_checks_implementation() {
    let source = "type Person = { age: Int32; name: String; };\n\
        contract Watched<T> { on<K from String & keyof T>(event: \"{K}Changed\", callback: Fn(T[K]) => Void): Void; }\n\
        class PersonWatcher implements Watched<Person> { on<K from String & keyof Person>(event: \"{K}Changed\", callback: Fn(Person[K]) => Void): Void { } }\n\
        fn attach(person: Watched<Person>): Void { person.on(\"ageChanged\", (age): Void => { inmut number: Int32 = age; }); }\n\
        fn main(): Void { }";
    let (_, sink) = parse_and_check(source);
    let output = sink.render(RenderStyle::Human);
    assert!(
        !output.contains(codes::TYPE_MISMATCH.as_str()),
        "generic contract implementation and contextual callback should type-check: {output}"
    );
    assert!(
        !output.contains(codes::UNKNOWN_MEMBER.as_str()),
        "the instantiated contract method should be visible: {output}"
    );

    let invalid_key = source.replace("ageChanged", "heightChanged");
    let (_, sink) = parse_and_check(&invalid_key);
    let output = sink.render(RenderStyle::Human);
    assert!(
        output.contains(codes::TYPE_MISMATCH.as_str())
            || output.contains(codes::UNKNOWN_MEMBER.as_str()),
        "an event that does not name a key of T must be rejected: {output}"
    );

    let wrong_bound = source.replace(
        "on<K from String & keyof Person>",
        "on<K from String & keyof Person & \"age\">",
    );
    let (_, sink) = parse_and_check(&wrong_bound);
    let output = sink.render(RenderStyle::Human);
    assert!(
        output.contains(codes::TYPE_MISMATCH.as_str()),
        "an implementation with a weaker generic bound must not satisfy Watched<Person>: {output}"
    );
}

#[test]
fn open_template_matching_stops_at_the_deterministic_source_budget() {
    let key = "a".repeat(zirk_sema::type_budget::BudgetKind::TemplateSourceBytes.limit() as usize);
    let source_text = format!(
        "fn on<T, K from String & keyof T>(value: T, event: \"{{K}}Changed\"): Void {{ }}\n\
         fn main(): Void {{ inmut person: {{ age: Int32 }} = {{ age: 1 }}; on(person, \"{key}Changed\"); }}"
    );
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("open-template-budget.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let output = sink.render(RenderStyle::Human);
    assert!(
        output.contains("E0482") && output.contains("open template matching limit exceeded"),
        "an over-budget event must fail with the stable computation-limit diagnostic: {output}"
    );
    assert!(
        checked.generic_call_substitutions.is_empty(),
        "budget failure must not publish a partial template substitution"
    );
}

#[test]
fn standard_type_utilities_resolve_structures_unions_and_callables() {
    let source = "type Source = { name: String; count?: Int32; inmut enabled: Boolean };\n\
         type MaybeSource = Partial<Source>;\n\
         type RequiredSource = Required<Source>;\n\
         type FrozenSource = Readonly<Source>;\n\
         type NameOnly = Pick<Source, \"name\">;\n\
         type WithoutCount = Omit<Source, \"count\">;\n\
         type Table = Record<\"first\" | \"second\", Int32>;\n\
         type TextOnly = Extract<String | Int32, String>;\n\
         type NonNull = NonNullable<String?>;\n\
         type Args = Parameters<Fn(String, Int32) => Boolean>;\n\
         type Result = ReturnType<Fn(String) => Boolean>;\n\
         fn values(maybe: MaybeSource, required: RequiredSource, frozen: FrozenSource, name: NameOnly, without: WithoutCount, table: Table, text: TextOnly, non_null: NonNull, args: Args, result: Result): Void { }\n\
         fn main(): Void { }";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("utilities.zrk", source));
    let source_file = sources.entry();
    let mut diagnostics = DiagnosticSink::new();
    let tokens = tokenize(source_file, &mut diagnostics);
    let program = parse(source_file, &tokens, &mut diagnostics);
    let checked = check(&sources, &program, &mut diagnostics);
    let rendered = diagnostics.render(RenderStyle::Human);
    assert!(
        !rendered.contains("utility has the wrong number")
            && !rendered.contains("requires a structural type"),
        "standard utilities should resolve: {rendered}"
    );
    let params = &checked.functions["values"].params;
    assert!(
        checked
            .describe_type(params[0].ty)
            .contains("name?: String")
    );
    assert!(
        checked.describe_type(params[1].ty).contains("count: Int32"),
        "required utility result: {}",
        checked.describe_type(params[1].ty)
    );
    assert!(
        checked
            .describe_type(params[2].ty)
            .contains("inmut enabled: Boolean")
    );
    assert!(checked.describe_type(params[3].ty).contains("name: String"));
    assert!(!checked.describe_type(params[4].ty).contains("count"));
    assert!(
        checked.describe_type(params[5].ty).contains("first: Int32")
            && checked
                .describe_type(params[5].ty)
                .contains("second: Int32")
    );
    assert_eq!(checked.describe_type(params[6].ty), "String");
    assert_eq!(checked.describe_type(params[7].ty), "String");
    assert!(checked.describe_type(params[8].ty).contains("String"));
    assert_eq!(checked.describe_type(params[9].ty), "Boolean");
}

#[test]
fn generic_inference_combines_arguments_and_expected_result_context() {
    accepted(
        "fn optional<T>(): T? { return null; }\n\
         fn identity<T>(value: T): T { return value; }\n\
         fn main(): Void {\n\
             mut inferred: String? = optional();\n\
             mut combined: String = identity(\"ready\");\n\
         }",
    );
    let conflict = rejected(
        "fn pair<T>(left: T, right: T): T { return left; } fn main(): Void { pair(1, \"text\"); }",
    );
    assert!(
        conflict.contains("cannot infer `T`") && conflict.contains("both"),
        "conflicting constraints must be diagnosed rather than selecting the last argument: {conflict}"
    );
}

#[test]
fn declared_generic_variance_controls_nested_assignability() {
    accepted(
        "class Animal { }\n\
         class Dog extends Animal { }\n\
         class ReadBox<out T> { inmut value: T; }\n\
         class Consumer<in T> { accept(value: T): Void { } }\n\
         fn take_read(box: ReadBox<Animal>): Void { }\n\
         fn pass_read(box: ReadBox<Dog>): Void { take_read(box); }\n\
         fn take_consumer(box: Consumer<Dog>): Void { }\n\
         fn pass_consumer(box: Consumer<Animal>): Void { take_consumer(box); }",
    );
    let invariant = rejected(
        "class Animal { } class Dog extends Animal { } class MutableBox<T> { mut value: T; } fn take(box: MutableBox<Dog>): Void { } fn pass(box: MutableBox<Animal>): Void { take(box); }",
    );
    assert!(
        invariant.contains(codes::TYPE_MISMATCH.as_str()),
        "mutable generic arguments remain invariant: {invariant}"
    );
    accepted("type Producer<out T> = Fn() => T; type Consumer<in T> = Fn(T) => Void;");
    let alias_variance = rejected("type Bad<out T> = Fn(T) => Void;");
    assert!(
        alias_variance.contains(codes::INVALID_VARIANCE.as_str()),
        "aliases must validate variance through callable positions: {alias_variance}"
    );
    let mutable_alias = rejected("type BadContainer<out T> = Array<T>;");
    assert!(
        mutable_alias.contains(codes::INVALID_VARIANCE.as_str()),
        "mutable generic containers are invariant through aliases: {mutable_alias}"
    );
}

#[test]
fn callable_unions_require_all_alternatives_and_intersections_select_one_signature() {
    let source = "type TextFn = Fn(String) => String;\n\
         type BoolFn = Fn(String) => Boolean;\n\
         type NumberFn = Fn(Int32) => Int32;\n\
         type Either = TextFn | BoolFn;\n\
         type TextOverload = TextFn & NumberFn;\n\
         fn union_call(callable: Either): Void { mut result: String | Boolean = callable(\"x\"); }\n\
         fn overloaded(callable: TextOverload): Void { mut text: String = callable(\"x\"); mut number: Int32 = callable(1); }";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("callable-algebra.zrk", source));
    let source_file = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source_file, &mut sink);
    let program = parse(source_file, &tokens, &mut sink);
    let _checked = check(&sources, &program, &mut sink);
    let valid_diagnostics = sink.render(RenderStyle::Human);
    assert!(
        !valid_diagnostics.contains(codes::TYPE_MISMATCH.as_str())
            && !valid_diagnostics.contains(codes::NOT_CALLABLE.as_str()),
        "callable algebra should select safe signatures: {valid_diagnostics}"
    );
    let invalid_union = rejected(
        "type TextFn = Fn(String) => String; type NumberFn = Fn(Int32) => Int32; type Either = TextFn | NumberFn; fn invoke(callable: Either): Void { callable(\"x\"); }",
    );
    assert!(
        invalid_union.contains("valid for every callable alternative"),
        "union calls must satisfy every possible signature: {invalid_union}"
    );
    let ambiguous = rejected(
        "type A = Fn(String) => String; type B = Fn(String) => Boolean; type Both = A & B; fn invoke(callable: Both): Void { callable(\"x\"); }",
    );
    assert!(
        ambiguous.contains("intersection callable selection is ambiguous"),
        "intersection calls need one unique match: {ambiguous}"
    );
}
