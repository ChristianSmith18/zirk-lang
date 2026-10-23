//! Emission tests: IR to LLVM.
//!
//! The essential check is that the produced module **verifies**: LLVM rejects
//! malformed IR, and a module that verifies is one the backend can compile.
//!
//! Shape is asserted over the textual IR rather than over inkwell handles. It
//! reads far better in a failure, and it is what a person would inspect when
//! debugging codegen.

mod common;

use inkwell::context::Context;
use zirk_ast::{Ident, ModuleDecl};
use zirk_codegen_llvm::{emit, emit_with_sources, symbols};
use zirk_diagnostics::{DiagnosticSink, RenderStyle, SourceFile, SourceMap};
use zirk_ir::lower;
use zirk_lexer::tokenize;
use zirk_parser::parse;
use zirk_sema::check;

/// Compiles source down to LLVM IR text, asserting the module verifies.
///
/// Standard stream calls (`stdout.println`, `stderr.print`, ...) require an
/// explicit `std.io` import (see `add-stdin-stdout-standard-library`), so any
/// source using one is automatically prefixed with the import and checked
/// alongside the compiler-bundled `std.io` modules, mirroring how
/// `zirk-browser::check_source` merges them for a program with no
/// filesystem-backed package loader.
fn llvm_ir(source_text: &str) -> String {
    let ir = checked_ir(source_text);
    llvm_ir_for_module(&ir)
}

fn checked_ir(source_text: &str) -> zirk_ir::Module {
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
    let checked = check(&sources, &program, &mut sink);

    assert!(
        !sink.has_errors(),
        "the program must be valid:\n{}",
        sink.render(RenderStyle::Human)
    );

    let ir = lower(&program, &checked);
    zirk_ir::verify(&ir).expect("the IR must be well formed before emitting");
    ir
}

fn llvm_ir_for_module(ir: &zirk_ir::Module) -> String {
    let _llvm = common::llvm_lock();
    let context = Context::create();
    let module = emit(&context, ir, "test");

    if let Err(error) = module.verify() {
        panic!(
            "the generated LLVM module does not verify:\n{}\n--- module ---\n{}",
            error.to_string(),
            module.print_to_string().to_string()
        );
    }

    module.print_to_string().to_string()
}

fn in_main(body: &str) -> String {
    format!("fn main(): Void {{\n{body}\n}}")
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

// --- Module shape -----------------------------------------------------------

#[test]
fn the_minimal_program_verifies() {
    let ir = llvm_ir("fn main(): Void { }");
    assert!(ir.contains("define void @zk_main()"));
}

#[test]
fn native_debug_emission_maps_source_lines_parameters_and_locals() {
    let _llvm = common::llvm_lock();
    let source_text = "fn add(left: Int32, right: Int32): Int32 {\n    return left + right;\n}\n\nfn main(): Void {\n    inmut base: Int32 = 10;\n    inmut result = add(base, 5);\n}\n";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("/workspace/debug_main.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    assert!(
        !sink.has_errors(),
        "debug fixture must be valid:\n{}",
        sink.render(RenderStyle::Human)
    );
    let ir = lower(&program, &checked);

    let context = Context::create();
    let module = emit_with_sources(&context, &ir, "debug_test", &sources);
    module
        .verify()
        .expect("source debug metadata must produce a valid LLVM module");
    let text = module.print_to_string().to_string();
    assert!(text.contains("!DICompileUnit"));
    assert!(text.contains("!DILocation(line: 2"));
    assert!(text.contains(r#"name: "left""#));
    assert!(text.contains(r#"name: "base""#));
    assert!(text.contains("debug_main.zrk"));
    assert!(text.contains("/workspace"));
}

#[test]
fn a_c_entrypoint_is_generated() {
    let ir = llvm_ir("fn main(): Void { }");
    assert!(
        ir.contains("define i32 @main()"),
        "the operating system invokes `main`, not Zirk's:\n{ir}"
    );
}

#[test]
fn zirk_functions_are_prefixed() {
    // A Zirk function named `printf` must not become the C one.
    let ir = llvm_ir("fn printf(): Void { }\nfn main(): Void { }");
    assert!(ir.contains("@zk_printf"));
}

// --- Application lifecycle --------------------------------------------------

#[test]
fn the_entrypoint_wraps_main_with_the_runtime_lifecycle() {
    let ir = llvm_ir("fn main(): Void { }");

    let entrypoint = ir
        .split("define i32 @main()")
        .nth(1)
        .expect("the C entrypoint exists");

    let init = entrypoint
        .find(symbols::INIT)
        .expect("calls the initializer");
    let zirk = entrypoint.find("@zk_main").expect("calls Zirk main");
    let deadline_policy = entrypoint
        .find("zirk_rt_run_main_cli")
        .expect("CLI entrypoint selects the bounded deadline policy");
    let shutdown = entrypoint
        .find(symbols::SHUTDOWN)
        .expect("calls the shutdown");

    assert!(
        init < deadline_policy && deadline_policy < zirk && zirk < shutdown,
        "the order must be init, main, shutdown:\n{entrypoint}"
    );
}

#[test]
fn the_entrypoint_returns_zero() {
    let ir = llvm_ir("fn main(): Void { }");
    let entrypoint = ir.split("define i32 @main()").nth(1).expect("entrypoint");
    assert!(entrypoint.contains("ret i32 0"));
}

// --- Types ------------------------------------------------------------------

#[test]
fn the_subset_types_map_to_llvm() {
    let ir = llvm_ir(
        "fn i(): Int32 { return 1; }\nfn b(): Boolean { return true; }\nfn main(): Void { }",
    );

    assert!(ir.contains("define i32 @zk_i()"));
    assert!(ir.contains("define i1 @zk_b()"));
    assert!(ir.contains("define void @zk_main()"));
}

#[test]
fn a_string_is_an_opaque_pointer() {
    let ir = llvm_ir(&in_main("mut s: String = \"hola\";"));
    assert!(
        ir.contains(&format!("call ptr @{}", symbols::STR_FROM_UTF8)),
        "the string is built by the runtime:\n{ir}"
    );
}

// --- Slots ------------------------------------------------------------------

#[test]
fn locals_become_allocas() {
    let ir = llvm_ir(&in_main("mut x: Int32 = 1;"));
    assert!(ir.contains("alloca i32"));
    assert!(ir.contains("store i32"));
}

#[test]
fn parameters_are_stored_into_their_slots() {
    let ir = llvm_ir("fn f(a: Int32): Int32 { return a; }\nfn main(): Void { }");
    assert!(
        ir.contains("alloca i32"),
        "the parameter has its slot:\n{ir}"
    );
    assert!(ir.contains("load i32"));
}

// --- Checked arithmetic -----------------------------------------------------

#[test]
fn addition_checks_for_overflow() {
    let ir = llvm_ir(&in_main("mut a: Int32 = 1;\nmut x: Int32 = a + 2;"));
    assert!(
        ir.contains("llvm.sadd.with.overflow.i32"),
        "ordinary overflow must be a controlled error:\n{ir}"
    );
}

#[test]
fn subtraction_and_multiplication_also_check() {
    let ir = llvm_ir(&in_main(
        "mut n: Int32 = 3;\nmut a: Int32 = n - 1;\nmut b: Int32 = n * 4;",
    ));
    assert!(ir.contains("llvm.ssub.with.overflow.i32"));
    assert!(ir.contains("llvm.smul.with.overflow.i32"));
}

#[test]
fn negation_goes_through_the_overflow_check() {
    // `-Int32.MIN` does not fit in Int32, so it cannot be a bare `neg`.
    let ir = llvm_ir(&in_main("mut x: Int32 = 1;\nmut y: Int32 = -x;"));
    assert!(ir.contains("llvm.ssub.with.overflow.i32"));
}

#[test]
fn division_checks_the_divisor() {
    // The check itself moved to `zirk-ir` (`fase-4d-runtimeerror`, design
    // D10): `checked_division` no longer calls `zirk_rt_division_by_zero`
    // directly (that handler stays declared in the module only for the
    // `noreturn` attribute it shares with the rest of that family, never
    // called from here again) — instead, the divisor is compared to zero
    // and, on a hit, a `DivisionByZeroError` is built and handed to
    // `zirk_rt_throw`, the same mechanism an explicit `throw` uses.
    let ir = llvm_ir(&in_main(
        "mut a: Int32 = 10;\nmut b: Int32 = 2;\nmut c: Decimal = a / b;",
    ));
    assert!(
        ir.contains(symbols::THROW),
        "division by zero must not be undefined behaviour:\n{ir}"
    );
    assert!(
        ir.contains("@zirk_rt_decimal_is_zero"),
        "the decimal divisor is checked for zero:\n{ir}"
    );
    assert!(ir.contains("@zirk_rt_decimal_div"));
}

#[test]
fn the_remainder_also_checks_the_divisor() {
    let ir = llvm_ir(&in_main(
        "mut a: Int32 = 10;\nmut b: Int32 = 3;\nmut c: Int32 = a % b;",
    ));
    assert!(ir.contains(symbols::THROW));
    assert!(ir.contains("srem i32"));
}

// --- Mixed-width numeric promotion ------------------------------------------

#[test]
fn mixed_int8_and_int32_sign_extends_before_adding() {
    let ir = llvm_ir(&in_main(
        "mut a: Int8 = 1;\nmut b: Int32 = 2;\nmut c: Int32 = a + b;",
    ));
    assert!(
        ir.contains("sext i8"),
        "the Int8 operand must be sign-extended:\n{ir}"
    );
    assert!(
        ir.contains("llvm.sadd.with.overflow.i32"),
        "overflow must be checked at the common Int32 width:\n{ir}"
    );
}

#[test]
fn mixed_uint8_and_int32_converts_both_to_float64() {
    let ir = llvm_ir(&in_main(
        "mut a: UInt8 = 1 as UInt8;\nmut b: Int32 = -2;\nmut c = a + b;",
    ));
    assert!(
        ir.contains("uitofp i8"),
        "UInt8 must be converted unsigned to Float64:\n{ir}"
    );
    assert!(
        ir.contains("sitofp i32"),
        "Int32 must be converted signed to Float64:\n{ir}"
    );
    assert!(
        ir.contains("fadd double"),
        "the addition must run at Float64:\n{ir}"
    );
}

#[test]
fn mixed_int32_and_float64_converts_int_to_float() {
    let ir = llvm_ir(&in_main(
        "mut a: Int32 = 1;\nmut b: Float64 = 2.5;\nmut c = a + b;",
    ));
    assert!(
        ir.contains("sitofp i32"),
        "Int32 must be converted to Float64:\n{ir}"
    );
    assert!(
        ir.contains("fadd double"),
        "the addition must run at Float64:\n{ir}"
    );
}

#[test]
fn int8_postfix_increment_uses_i8_overflow_check() {
    let ir = llvm_ir(&in_main("mut a: Int8 = 1;\na++;"));
    assert!(
        ir.contains("llvm.sadd.with.overflow.i8"),
        "Int8++ must check overflow at the i8 width:\n{ir}"
    );
}

// --- Comparison and logic ---------------------------------------------------

#[test]
fn comparisons_emit_icmp() {
    // Variables rather than literals: LLVM folds constants while building, so
    // `1 < 2` would become `true` and emit no comparison at all.
    let ir = llvm_ir(&in_main(
        "mut x: Int32 = 1;\nmut y: Int32 = 2;\n\
         mut a: Boolean = x < y;\nmut b: Boolean = x == y;\nmut c: Boolean = x >= y;",
    ));
    assert!(ir.contains("icmp slt"), "{ir}");
    assert!(ir.contains("icmp eq"), "{ir}");
    assert!(ir.contains("icmp sge"), "{ir}");
}

#[test]
fn logical_operators_do_not_check_overflow() {
    // Variables for the same reason: `true && false` gets folded away.
    let ir = llvm_ir(&in_main(
        "mut p: Boolean = true;\nmut q: Boolean = false;\nmut x: Boolean = p && q;",
    ));
    assert!(!ir.contains("llvm.sadd"), "{ir}");
}

#[test]
fn logical_operators_short_circuit() {
    // `&&` becomes a branch rather than an `and`: the right operand must not
    // run when the left already decided the answer.
    let ir = llvm_ir(&in_main(
        "mut p: Boolean = true;\nmut q: Boolean = false;\nmut x: Boolean = p && q;",
    ));
    assert!(ir.contains("br i1"), "{ir}");
}

// --- Control flow -----------------------------------------------------------

#[test]
fn a_conditional_emits_a_branch() {
    let ir = llvm_ir(&in_main("if true { } else { }"));
    assert!(ir.contains("br i1"));
}

#[test]
fn a_chained_conditional_verifies() {
    // What matters is that it verifies: LLVM rejects a block without a
    // terminator or a branch to a nonexistent block.
    llvm_ir(&in_main("if true { } else if false { } else { }\nreturn;"));
}

#[test]
fn a_function_where_both_branches_return_verifies() {
    llvm_ir(
        "fn f(a: Boolean): Int32 { if a { return 1; } else { return 2; } }\nfn main(): Void { }",
    );
}

// --- Calls ------------------------------------------------------------------

#[test]
fn a_call_emits_with_its_arguments() {
    let ir = llvm_ir(
        "fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { mut x: Int32 = add(1, 2); }",
    );
    assert!(ir.contains("call i32 @zk_add"));
}

#[test]
fn println_calls_the_runtime() {
    let ir = llvm_ir(&in_main("stdout.println(\"hola\");"));
    assert!(ir.contains(&format!("@{}", symbols::IO_PRINTLN)));
}

#[test]
fn printing_an_integer_converts_through_the_runtime() {
    // Without the conversion the value would reach the runtime as a pointer.
    let ir = llvm_ir(&in_main("mut n: Int32 = 42;\nstdout.println(n);"));
    assert!(
        ir.contains(&format!("@{}", symbols::STR_FROM_I32)),
        "the conversion must go through the runtime:\n{ir}"
    );
}

#[test]
fn printing_a_boolean_converts_through_the_runtime() {
    let ir = llvm_ir(&in_main("mut b: Boolean = true;\nstdout.println(b);"));
    assert!(ir.contains(&format!("@{}", symbols::STR_FROM_BOOL)), "{ir}");
}

#[test]
fn println_always_receives_a_pointer() {
    // The invariant the crash violated: whatever gets printed arrives as a
    // string handle, never as a raw value.
    for body in [
        "mut s: String = \"a\";\nstdout.println(s);",
        "mut n: Int32 = 1;\nstdout.println(n);",
        "mut b: Boolean = true;\nstdout.println(b);",
    ] {
        let ir = llvm_ir(&in_main(body));
        let call = format!("call void @{}(ptr", symbols::IO_PRINTLN);
        assert!(ir.contains(&call), "for `{body}`:\n{ir}");
    }
}

// --- String equality --------------------------------------------------------

#[test]
fn string_equality_is_structural() {
    // `LANGUAGE_SPEC` section 4: `==` compares structurally. Comparing handles
    // would compare identity, which is what `is` means.
    let ir = llvm_ir(&in_main(
        "mut a: String = \"x\";\nmut b: String = \"y\";\nmut eq: Boolean = a == b;",
    ));
    assert!(
        ir.contains(&format!("@{}", symbols::STR_EQ)),
        "equality must go through the runtime:\n{ir}"
    );
}

#[test]
fn string_inequality_negates_the_comparison() {
    let ir = llvm_ir(&in_main(
        "mut a: String = \"x\";\nmut b: String = \"y\";\nmut ne: Boolean = a != b;",
    ));
    assert!(ir.contains(&format!("@{}", symbols::STR_EQ)));
    assert!(
        ir.contains(" xor "),
        "the negation of an i1 is a xor:\n{ir}"
    );
}

// --- Reference program ------------------------------------------------------

#[test]
fn the_reference_program_of_the_roadmap_emits() {
    let ir = llvm_ir("fn main(): Void {\n    stdout.println(\"Hola desde Zirk\");\n}");

    assert!(ir.contains("Hola desde Zirk"));
    assert!(ir.contains(&format!("@{}", symbols::STR_FROM_UTF8)));
    assert!(ir.contains(&format!("@{}", symbols::IO_PRINTLN)));
    assert!(ir.contains("define i32 @main()"));
}

#[test]
fn generated_symbols_only_use_characters_every_assembler_accepts() {
    // A symbol reaches the object file through the target's assembler, and the
    // safe set is not the same everywhere: `#` starts a comment in x86_64's
    // AT&T syntax, so a lifted lambda named `<lambda>#0` broke there and
    // nowhere else — aarch64 comments with `//`.
    //
    // `$` is accepted here even though it is outside that same AT&T set,
    // because `zirk-ir/src/lower.rs`'s own `method_symbol`/`constructor_symbol`
    // already build every class method's and constructor's own symbol with it
    // (`Class$method`, `Class$construct$0`) — the four native failure classes
    // `fase-4d-runtimeerror` adds (`DivisionByZeroError`, …) are the first
    // thing to put one of those symbols in a program that declares no class
    // of its own: their `message`/`code`/`cause`/`stack_trace` bodies are
    // synthesized into every module unconditionally, the same "no
    // `program.classes` entry, build it by hand" shape
    // `zk.unreachable_abstract_method` already uses.
    let ir = llvm_ir(
        "fn main(): Void {\n\
         inmut A = 1;\n\
         inmut F = (x: Int32): Int32 => x + A;\n\
         stdout.println(F(1));\n\
         }",
    );

    for line in ir.lines().filter(|l| l.starts_with("define")) {
        let name: String = line
            .chars()
            .skip_while(|c| *c != '@')
            .skip(1)
            .take_while(|c| *c != '(')
            .collect();
        // LLVM's own textual printer wraps a global in `"..."` whenever its
        // name contains a character outside its bare-identifier grammar
        // (`$` is one, which is exactly why `Class$method` needs it) — that
        // quoting is IR syntax, not part of the symbol the assembler
        // ultimately sees, so it is stripped before checking what is inside.
        let name = name.strip_prefix('"').unwrap_or(&name);
        let name = name.strip_suffix('"').unwrap_or(name);

        assert!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$'),
            "`{name}` uses a character an assembler may not accept"
        );
    }
}

// --- Collector shadow-stack frames (`fase-4e-colector-mark-sweep`, design D2) ---

/// The text of one Zirk-defined function, from its `define` line up to (and
/// including) its closing brace — LLVM's textual printer never nests braces
/// inside a single function body, so the first `"\n}\n"` after the marker is
/// always that function's own end.
fn function_body<'a>(ir: &'a str, symbol: &str) -> &'a str {
    let marker = format!("@{symbol}(");
    let start = ir
        .find(&marker)
        .unwrap_or_else(|| panic!("`{symbol}` is defined:\n{ir}"));
    let after = &ir[start..];
    let end = after.find("\n}\n").map(|i| i + 3).unwrap_or(after.len());
    &after[..end]
}

#[test]
fn function_entry_pushes_exactly_one_gc_frame() {
    let ir = llvm_ir("fn main(): Void { }");
    let body = function_body(&ir, "zk_main");
    assert_eq!(
        body.matches("call void @zirk_rt_push_frame").count(),
        1,
        "exactly one push per function entry:\n{body}"
    );
}

#[test]
fn every_return_path_pops_its_own_gc_frame() {
    let ir = llvm_ir(
        "fn classify(n: Int32): Int32 {
             if (n < 0) { return -1; }
             if (n == 0) { return 0; }
             return 1;
         }
         fn main(): Void { }",
    );
    let body = function_body(&ir, "zk_classify");

    assert_eq!(
        body.matches("call void @zirk_rt_push_frame").count(),
        1,
        "exactly one push per function entry, however many return statements follow:\n{body}"
    );
    assert_eq!(
        body.matches("call void @zirk_rt_pop_frame").count(),
        4,
        "one pop per return path plus the entry cancellation exit:\n{body}"
    );
}

#[test]
fn a_reference_typed_local_is_pushed_as_a_gc_root() {
    let ir = llvm_ir(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void { mut p = Point(1); }",
    );
    let body = function_body(&ir, "zk_main");

    // Two roots, not one: `p` itself, plus design D4's own synthetic slot
    // spilling `Point(1)`'s `Alloc` result the moment it is produced —
    // before the constructor call that follows it, so a collection
    // triggered by anything the constructor itself might allocate can never
    // collect the object being constructed.
    assert!(
        body.contains("call void @zirk_rt_push_frame(ptr %gc_roots, i64 2)"),
        "a function with a named reference local plus its own D4 synthetic \
         spill must push a frame of exactly two roots:\n{body}"
    );
    assert!(
        body.contains("<gc_root>"),
        "the Alloc result must be spilled to a synthetic root slot before the constructor runs:\n{body}"
    );
}

// --- Exact base-ten `Decimal` ---------------------------------------------

#[test]
fn exact_float_literal_is_a_struct_constant_not_a_binary_float() {
    let ir = llvm_ir(&in_main("mut a: Decimal = 0.1;"));
    assert!(
        ir.contains("i128 1, i8 1"),
        "the `0.1` literal is the struct constant {{ 1, 1 }}:\n{ir}"
    );
}

#[test]
fn exact_float_addition_calls_the_decimal_runtime_by_pointer() {
    let ir = llvm_ir(&in_main(
        "mut a: Decimal = 0.1;\nmut b = 0.2;\nmut c = a + b;",
    ));
    assert!(
        ir.contains("call void @zirk_rt_decimal_add(ptr"),
        "`a + b` is a by-pointer call to the decimal-add helper:\n{ir}"
    );
}

#[test]
fn exact_float_to_string_calls_the_decimal_formatter() {
    let ir = llvm_ir(&in_main("mut a: Decimal = 0.1;\nstdout.println(a);"));
    assert!(
        ir.contains("@zirk_str_from_decimal(ptr"),
        "printing an exact Decimal goes through the decimal formatter:\n{ir}"
    );
}

#[test]
fn binary_float_arithmetic_is_still_a_native_fadd() {
    let ir = llvm_ir(&in_main(
        "mut a: Float64 = 1.5;\nmut b: Float64 = 2.5;\nmut c = a + b;",
    ));
    assert!(
        ir.contains("fadd double"),
        "Float64 arithmetic stays a native LLVM fadd:\n{ir}"
    );
}

#[test]
fn enum_construction_zero_initializes_inactive_reference_payloads() {
    let ir = llvm_ir(
        "enum MaybeText { Some(value: String), None }\n\
         fn main(): Void { mut value: MaybeText = MaybeText.None; }",
    );
    let main = ir
        .split("define void @zk_main()")
        .nth(1)
        .expect("main body exists");
    assert!(
        main.contains("{ i32 1, ptr null }"),
        "the constructed enum must contain the active tag and a null inactive payload:\n{main}"
    );
    assert!(
        !main.contains("undef"),
        "an inactive managed payload must never be LLVM undef:\n{main}"
    );
}

#[test]
fn float_to_integer_emission_checks_truncated_value_before_fptosi() {
    let ir = llvm_ir(
        "fn main(): Void {\n\
             mut value: Float64 = 7 as Float64;\n\
             mut result: Int32 = value as Int32;\n\
         }",
    );
    assert!(ir.contains("@llvm.trunc.f64"), "{ir}");
    assert!(ir.contains("fcmp oge double"), "{ir}");
    assert!(ir.contains("fcmp olt double"), "{ir}");
    assert!(ir.contains("@zirk_rt_invalid_cast()"), "{ir}");
    let cast = ir
        .split("fptosi")
        .next()
        .expect("checked conversion is emitted");
    assert!(
        cast.contains("br i1"),
        "the range checks must branch before the conversion:\n{cast}"
    );
}

#[test]
fn checked_float_to_integer_emission_covers_wide_signed_and_unsigned_bounds() {
    let ir = llvm_ir(
        "fn main(): Void {\n\
             mut wide: Float128 = 7 as Float128;\n\
             mut signed: Int128 = wide as Int128;\n\
             mut unsigned: UInt128 = wide as UInt128;\n\
         }",
    );
    assert!(
        ir.contains("0xL0000000000000000407F000000000000"),
        "UInt128's exclusive upper bound is exact 2^128, not a rounded UInt128::MAX:\n{ir}"
    );
    assert!(ir.contains("fcmp oge fp128"), "{ir}");
    assert!(ir.contains("fcmp olt fp128"), "{ir}");
    assert!(ir.contains("fptosi fp128"), "{ir}");
    assert!(ir.contains("fptoui fp128"), "{ir}");
}

#[test]
fn concurrent_block_emits_scope_calls_a_branch_thunk_and_a_spawn() {
    let ir = llvm_ir(
        "fn load(): Int32 { return 1; }\n         fn main(): Void {\n           concurrent {\n             inmut a: Int32 = load();\n           }\n           stdout.println(a);\n         }",
    );
    assert!(ir.contains(&format!("call i64 @{}", symbols::SCOPE_ENTER)));
    assert!(ir.contains(&format!("call i1 @{}", symbols::SCOPE_EXIT)));
    assert!(ir.contains(&format!("@{}", symbols::SPAWN)));
    assert!(ir.contains(&format!("@{}", symbols::BRANCH_REGISTER)));
    assert!(ir.contains("zk.branch_thunk."));
    assert!(
        ir.contains("zk.task.result.descriptor."),
        "task results use a GC-described box:\n{ir}"
    );
    assert!(
        ir.contains("@zirk_rt_push_frame"),
        "the result box is rooted before calling the branch:\n{ir}"
    );
    assert!(
        ir.contains("@zirk_rt_pop_frame"),
        "the temporary result root is popped on completion:\n{ir}"
    );
    assert!(
        ir.contains("uwtable"),
        "generated functions carry an unwind table"
    );
}

#[test]
fn collect_all_scope_ir_emits_its_distinct_runtime_abi() {
    let mut module = checked_ir(
        "fn load(): Int32 { return 1; } fn main(): Void { concurrent { inmut a: Int32 = load(); } }",
    );
    for function in &mut module.functions {
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                match instruction.kind {
                    zirk_ir::InstKind::ScopeEnter => {
                        instruction.kind = zirk_ir::InstKind::ScopeEnterCollectAll;
                    }
                    zirk_ir::InstKind::ScopeExit { scope } => {
                        instruction.kind = zirk_ir::InstKind::ScopeExitCollectAll { scope };
                    }
                    _ => {}
                }
            }
        }
    }
    zirk_ir::verify(&module).expect("collect-all scope IR verifies");
    let ir = llvm_ir_for_module(&module);
    assert!(
        ir.contains("declare i64 @zirk_rt_scope_enter_collect_all()"),
        "{ir}"
    );
    assert!(
        ir.contains("declare i1 @zirk_rt_scope_exit_collect_all(i64)"),
        "{ir}"
    );
    assert!(
        ir.contains("call i64 @zirk_rt_scope_enter_collect_all()"),
        "{ir}"
    );
    assert!(
        ir.contains("call i1 @zirk_rt_scope_exit_collect_all(i64 "),
        "{ir}"
    );
}

#[test]
fn concurrent_task_boxes_nested_record_results_with_managed_fields() {
    let ir = llvm_ir(
        "record Inner { text: String; }\n\
         record Outer { prefix: Int64; inner: Inner; }\n\
         fn make(): Outer { return Outer(prefix: 5, inner: Inner(text: \"kept alive\")); }\n\
         fn main(): Void {\n\
           concurrent { inmut value: Outer = make(); }\n\
           stdout.println(value.inner.text);\n\
         }",
    );
    assert!(
        ir.contains("zk.task.result.descriptor."),
        "aggregate result descriptor is emitted:\n{ir}"
    );
    assert!(
        ir.contains("task_result_payload"),
        "the exact aggregate is stored in its result box:\n{ir}"
    );
    assert!(
        ir.contains("job_payload"),
        "JobWait reloads the aggregate from its result box:\n{ir}"
    );
    assert!(
        ir.contains("i32 3, i32 1, i32 0"),
        "the descriptor offset walks the box payload into the nested String field:\n{ir}"
    );
}

#[test]
fn timer_sleep_emits_a_runtime_suspend_call() {
    let ir = llvm_ir("fn main(): Void { Timer.sleep(1ms); }");
    assert!(ir.contains(&format!("@{}", symbols::SLEEP)));
}

#[test]
fn concurrent_protect_emits_shield_calls_and_post_cleanup_cancellation_check() {
    let ir = llvm_ir("fn main(): Void { Concurrent.protect((): Void => { }); }");
    let enter = ir
        .find(&format!("call void @{}", symbols::CANCEL_SHIELD_ENTER))
        .expect("cancellation shield enter call");
    let exit = ir
        .find(&format!("call void @{}", symbols::CANCEL_SHIELD_EXIT))
        .expect("cancellation shield exit call");
    let cancelled = ir[exit..]
        .find(&format!("call i1 @{}", symbols::IS_CANCELLED))
        .map(|offset| exit + offset)
        .expect("cancellation check after cleanup");
    assert!(
        enter < exit && exit < cancelled,
        "generated LLVM must restore the shield before checking cancellation:\n{ir}"
    );
}

#[test]
fn background_start_emits_runtime_call_with_branch_thunk_capture_and_u32_status() {
    let mut module = checked_ir("fn main(): Void {}");
    module.closures.push(zirk_ir::ClosureLayout {
        captures: vec![zirk_ir::IrType::Int(zirk_ir::IntWidth::I32)],
        params: Vec::new(),
        returns: zirk_ir::IrType::Void,
    });
    module.functions.push(zirk_ir::Function {
        name: "background_branch".into(),
        params: vec![zirk_ir::SlotId(0)],
        return_type: zirk_ir::IrType::Void,
        slots: vec![zirk_ir::Slot {
            name: "capture".into(),
            ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::I32),
            span: zirk_diagnostics::Span::empty(0),
        }],
        blocks: vec![zirk_ir::Block {
            id: zirk_ir::BlockId(0),
            instructions: Vec::new(),
            terminator: Some(zirk_ir::Terminator::Return(None)),
        }],
        entry: zirk_ir::BlockId(0),
        span: zirk_diagnostics::Span::empty(0),
        gc_roots: Vec::new(),
    });

    let main = module
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .expect("main function");
    let status_slot = zirk_ir::SlotId(main.slots.len() as u32);
    main.slots.push(zirk_ir::Slot {
        name: "background_status".into(),
        ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::U32),
        span: zirk_diagnostics::Span::empty(0),
    });
    let next = main
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.result)
        .map(|value| value.0)
        .max()
        .unwrap_or(0)
        + 1;
    let block = main
        .blocks
        .iter_mut()
        .find(|block| block.terminator.is_some())
        .expect("terminated main block");
    block.instructions.extend([
        zirk_ir::Instruction {
            result: Some(zirk_ir::ValueId(next)),
            kind: zirk_ir::InstKind::ConstInt(7),
            ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::I32),
            span: zirk_diagnostics::Span::empty(0),
        },
        zirk_ir::Instruction {
            result: Some(zirk_ir::ValueId(next + 1)),
            kind: zirk_ir::InstKind::MakeCallable {
                target: "background_branch".into(),
                captures: vec![zirk_ir::Operand(zirk_ir::ValueId(next))],
            },
            ty: zirk_ir::IrType::Callable(0),
            span: zirk_diagnostics::Span::empty(0),
        },
        zirk_ir::Instruction {
            result: None,
            kind: zirk_ir::InstKind::BackgroundStart {
                target: "background_branch".into(),
                body: zirk_ir::Operand(zirk_ir::ValueId(next + 1)),
                status_slot,
            },
            ty: zirk_ir::IrType::Void,
            span: zirk_diagnostics::Span::empty(0),
        },
    ]);
    zirk_ir::verify(&module).expect("BackgroundStart module verifies");

    let llvm = llvm_ir_for_module(&module);
    assert!(
        llvm.contains("declare i32 @zirk_rt_background_spawn(ptr, ptr)"),
        "background runtime declaration uses the status-returning ABI:\n{llvm}"
    );
    assert!(
        llvm.contains("zk.background_thunk.background_branch"),
        "the generated branch thunk is present and named for the target:\n{llvm}"
    );
    let call = llvm
        .lines()
        .find(|line| line.contains("call i32 @zirk_rt_background_spawn"))
        .expect("background spawn call returns U32");
    assert!(
        call.contains("@zk.background_thunk.background_branch")
            && call.contains("ptr %background_capture"),
        "spawn receives the generated thunk and allocated capture block: {call}"
    );
    assert!(
        llvm.contains("store i32 %background_admission_status, ptr %background_status"),
        "U32 admission status is stored in its IR status slot:\n{llvm}"
    );
}

#[test]
fn job_wait_any_emits_a_bounded_id_array_and_runtime_call() {
    let mut ir = checked_ir(
        "fn main(): Void {
            concurrent {
                inmut first: Int32 = 11;
                inmut second: Int32 = 22;
            }
        }",
    );
    let main = ir
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .expect("main function");
    let jobs: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| matches!(instruction.kind, zirk_ir::InstKind::BranchStart { .. }))
        .filter_map(|instruction| instruction.result)
        .map(zirk_ir::Operand)
        .collect();
    assert_eq!(jobs.len(), 2, "expected two generated branch handles");
    let next_value = main
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.result)
        .map(|value| value.0)
        .max()
        .unwrap_or(0)
        + 1;
    let job_slots: Vec<_> = jobs
        .iter()
        .map(|job| {
            main.blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find_map(|instruction| match instruction.kind {
                    zirk_ir::InstKind::Store(slot, value) if value == *job => Some(slot),
                    _ => None,
                })
                .expect("each branch handle is spilled across admission checks")
        })
        .collect();
    let wait_block =
        main.blocks
            .iter_mut()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(instruction.kind, zirk_ir::InstKind::ScopeExit { .. })
                })
            })
            .expect("scope join block dominated by both branch handles");
    let scope_exit_index = wait_block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction.kind, zirk_ir::InstKind::ScopeExit { .. }))
        .expect("scope exit follows wait-any");
    let loads: Vec<_> = job_slots
        .iter()
        .enumerate()
        .map(|(index, slot)| zirk_ir::Instruction {
            result: Some(zirk_ir::ValueId(next_value + index as u32)),
            kind: zirk_ir::InstKind::Load(*slot),
            ty: zirk_ir::IrType::Job,
            span: zirk_diagnostics::Span::empty(0),
        })
        .collect();
    let wait_jobs = (0..job_slots.len())
        .map(|index| zirk_ir::Operand(zirk_ir::ValueId(next_value + index as u32)))
        .collect();
    for (index, load) in loads.into_iter().enumerate() {
        wait_block
            .instructions
            .insert(scope_exit_index + index, load);
    }
    wait_block.instructions.insert(
        scope_exit_index + job_slots.len(),
        zirk_ir::Instruction {
            result: Some(zirk_ir::ValueId(next_value + job_slots.len() as u32)),
            kind: zirk_ir::InstKind::JobWaitAny { jobs: wait_jobs },
            ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::U64),
            span: zirk_diagnostics::Span::empty(0),
        },
    );
    zirk_ir::verify(&ir).expect("the bounded wait-any IR must verify");

    let llvm = llvm_ir_for_module(&ir);
    assert!(
        llvm.contains("call i64 @zirk_rt_job_wait_any(ptr "),
        "wait-any ABI call is emitted:\n{llvm}"
    );
    assert!(
        llvm.contains("wait_any_jobs"),
        "the task IDs are copied to the ABI array:\n{llvm}"
    );
}

#[test]
fn concurrent_within_emits_the_timer_wait_any_abi() {
    let module = checked_ir(
        "fn main(): Void {
            mut winner: Int32 = Concurrent.of(
                (): Int32 => 1,
                (): Int32 => 2,
            ).within(5s).first();
        }",
    );
    let llvm = llvm_ir_for_module(&module);
    assert!(
        llvm.contains("declare i64 @zirk_rt_job_wait_any_with_timer(ptr, i64)"),
        "timer-specific wait-any ABI must be declared:\n{llvm}"
    );
    assert!(
        llvm.contains("call i64 @zirk_rt_job_wait_any_with_timer(ptr "),
        "within lowering must call the timer-specific ABI:\n{llvm}"
    );
}

#[test]
fn concurrent_first_uses_cleanup_failure_cancellation_abi() {
    let module = checked_ir(
        "fn main(): Void {
            mut winner: Int32 = Concurrent.of(
                (): Int32 => 1,
                (): Int32 => 2,
            ).first();
        }",
    );
    let llvm = llvm_ir_for_module(&module);
    assert!(
        llvm.contains("declare void @zirk_rt_cancel_with_cleanup_failure(i64)"),
        "cleanup-aware cancellation ABI declaration must be explicit:\n{llvm}"
    );
    assert!(
        llvm.contains("call void @zirk_rt_cancel_with_cleanup_failure(i64 "),
        "first() must cancel losers through the cleanup-aware ABI:\n{llvm}"
    );
}

#[test]
fn task_and_timer_admission_statuses_are_checked_before_handle_use() {
    let llvm = llvm_ir(
        "fn main(): Void {
            concurrent {
                inmut value: Int32 = 1;
                Timer.after(1ms, (): Void => { });
                Timer.every(1ms, (): Void => { });
                Timer.sleep(1ms);
            }
        }",
    );
    assert!(
        llvm.contains("@zirk_rt_spawn_checked"),
        "generated task creation uses the status-bearing ABI:\n{llvm}"
    );
    assert!(
        llvm.contains("@zirk_rt_sleep_checked"),
        "generated timer sleep receives an admission status:\n{llvm}"
    );
    assert!(
        llvm.contains("@zirk_rt_timer_after_checked")
            && llvm.contains("@zirk_rt_timer_every_checked"),
        "ambient timers use the task-and-timer reservation ABI:\n{llvm}"
    );
    assert!(
        llvm.contains("E_TASK_CAPACITY")
            && llvm.contains("E_TIMER_CAPACITY")
            && llvm.contains("E_BACKGROUND_CAPACITY"),
        "all capacity errors are present in the generated module:\n{llvm}"
    );
}

#[test]
fn job_outcome_wait_take_emits_exact_runtime_abi_and_rooted_slot_addresses() {
    let mut module = checked_ir(
        "fn main(): Void {
            concurrent {
                inmut first: Int32 = 11;
                inmut second: Int32 = 22;
            }
        }",
    );
    let main = module
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .expect("main function");
    let branch_job = main
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction.kind {
            zirk_ir::InstKind::BranchStart { .. } => instruction.result,
            _ => None,
        })
        .expect("spawned job handle");
    let job_slot = main
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction.kind {
            zirk_ir::InstKind::Store(slot, value) if value == zirk_ir::Operand(branch_job) => {
                Some(slot)
            }
            _ => None,
        })
        .expect("branch handle is spilled across admission checks");
    let value_slot = zirk_ir::SlotId(main.slots.len() as u32);
    main.slots.push(zirk_ir::Slot {
        name: "outcome_value".into(),
        ty: zirk_ir::IrType::String,
        span: zirk_diagnostics::Span::empty(0),
    });
    let throwable_slot = zirk_ir::SlotId(main.slots.len() as u32);
    main.slots.push(zirk_ir::Slot {
        name: "outcome_throwable".into(),
        ty: zirk_ir::IrType::String,
        span: zirk_diagnostics::Span::empty(0),
    });
    main.gc_roots.extend([value_slot, throwable_slot]);
    let next_value = main
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.result)
        .map(|value| value.0)
        .max()
        .unwrap_or(0)
        + 1;
    let wait_block =
        main.blocks
            .iter_mut()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(instruction.kind, zirk_ir::InstKind::ScopeExit { .. })
                })
            })
            .expect("scope close block");
    let scope_exit_index = wait_block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction.kind, zirk_ir::InstKind::ScopeExit { .. }))
        .expect("scope exit instruction");
    wait_block.instructions.insert(
        scope_exit_index,
        zirk_ir::Instruction {
            result: Some(zirk_ir::ValueId(next_value)),
            kind: zirk_ir::InstKind::Load(job_slot),
            ty: zirk_ir::IrType::Job,
            span: zirk_diagnostics::Span::empty(0),
        },
    );
    wait_block.instructions.insert(
        scope_exit_index + 1,
        zirk_ir::Instruction {
            result: Some(zirk_ir::ValueId(next_value + 1)),
            kind: zirk_ir::InstKind::JobOutcomeWaitTake {
                job: zirk_ir::Operand(zirk_ir::ValueId(next_value)),
                value_slot,
                throwable_slot,
            },
            ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::U32),
            span: zirk_diagnostics::Span::empty(0),
        },
    );
    zirk_ir::verify(&module).expect("outcome wait/take IR verifies");

    let llvm = llvm_ir_for_module(&module);
    assert!(
        llvm.contains("declare i32 @zirk_rt_job_outcome_wait_take(i64, ptr, ptr)"),
        "runtime declaration has the exact ABI signature:\n{llvm}"
    );
    assert!(
        llvm.contains("call i32 @zirk_rt_job_outcome_wait_take(i64 "),
        "runtime call returns i32 and passes the job id:\n{llvm}"
    );
    let call = llvm
        .lines()
        .find(|line| line.contains("call i32 @zirk_rt_job_outcome_wait_take"))
        .expect("outcome wait/take call");
    assert!(
        call.contains("ptr %outcome_value") && call.contains("ptr %outcome_throwable"),
        "both rooted output-slot addresses are passed in their declared order: {call}"
    );
}

#[test]
fn scope_outcome_wait_take_emits_status_call_and_rooted_outputs() {
    let mut module = checked_ir("fn main(): Void {}");
    let main = module
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .expect("main function");
    let ordinal_slot = zirk_ir::SlotId(main.slots.len() as u32);
    let value_slot = zirk_ir::SlotId(ordinal_slot.0 + 1);
    let throwable_slot = zirk_ir::SlotId(ordinal_slot.0 + 2);
    for (name, ty) in [
        (
            "scope_ordinal",
            zirk_ir::IrType::Int(zirk_ir::IntWidth::U64),
        ),
        ("scope_value", zirk_ir::IrType::String),
        ("scope_throwable", zirk_ir::IrType::String),
    ] {
        main.slots.push(zirk_ir::Slot {
            name: name.into(),
            ty,
            span: zirk_diagnostics::Span::empty(0),
        });
    }
    main.gc_roots.extend([value_slot, throwable_slot]);
    let next = main
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.result)
        .map(|value| value.0)
        .max()
        .unwrap_or(0)
        + 1;
    let block = main
        .blocks
        .iter_mut()
        .find(|block| block.terminator.is_some())
        .expect("terminated main block");
    block.instructions.extend([
        zirk_ir::Instruction {
            result: Some(zirk_ir::ValueId(next)),
            kind: zirk_ir::InstKind::ScopeEnterCollectAll,
            ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::I64),
            span: zirk_diagnostics::Span::empty(0),
        },
        zirk_ir::Instruction {
            result: Some(zirk_ir::ValueId(next + 1)),
            kind: zirk_ir::InstKind::ScopeOutcomeWaitTake {
                scope: zirk_ir::Operand(zirk_ir::ValueId(next)),
                ordinal_slot,
                value_slot,
                throwable_slot,
            },
            ty: zirk_ir::IrType::Int(zirk_ir::IntWidth::U32),
            span: zirk_diagnostics::Span::empty(0),
        },
        zirk_ir::Instruction {
            result: None,
            kind: zirk_ir::InstKind::ScopeCancel {
                scope: zirk_ir::Operand(zirk_ir::ValueId(next)),
            },
            ty: zirk_ir::IrType::Void,
            span: zirk_diagnostics::Span::empty(0),
        },
    ]);
    zirk_ir::verify(&module).expect("scope outcome IR verifies");

    let llvm = llvm_ir_for_module(&module);
    assert!(
        llvm.contains("declare i32 @zirk_rt_scope_outcome_wait_take(i64, ptr, ptr, ptr)"),
        "scope ABI must declare status plus three output pointers:\n{llvm}"
    );
    let call = llvm
        .lines()
        .find(|line| line.contains("call i32 @zirk_rt_scope_outcome_wait_take"))
        .expect("scope wait/take call");
    assert!(
        call.contains("ptr %scope_ordinal")
            && call.contains("ptr %scope_value")
            && call.contains("ptr %scope_throwable"),
        "all output destinations are passed in ABI order: {call}"
    );
    assert!(
        llvm.contains("declare void @zirk_rt_scope_cancel(i64)")
            && llvm.contains("call void @zirk_rt_scope_cancel(i64 "),
        "scope cancellation symbol is declared and emitted:\n{llvm}"
    );
}

#[test]
fn settled_pipeline_emits_wait_take_result_box_load_and_cancellation_drain() {
    let llvm = llvm_ir(
        "fn main(): Void {\n\
           mut outcomes: List<Outcome<Int32>> = Concurrent.of(\n\
             (): Int32 => 1,\n\
             (): Int32 => 2\n\
           ).settled();\n\
         }",
    );
    assert!(
        llvm.contains("declare i32 @zirk_rt_job_outcome_wait_take(i64, ptr, ptr)"),
        "{llvm}"
    );
    assert!(
        llvm.contains("call i32 @zirk_rt_job_outcome_wait_take(i64 "),
        "{llvm}"
    );
    assert!(
        llvm.contains("declare i32 @zirk_rt_job_outcome_take(i64, ptr, ptr)"),
        "{llvm}"
    );
    assert!(
        llvm.contains("call i32 @zirk_rt_job_outcome_take(i64 "),
        "cancellation drains joined outcomes:\n{llvm}"
    );
    assert!(
        llvm.contains("job_payload"),
        "fulfilled payload is loaded from its typed result box:\n{llvm}"
    );
    assert!(llvm.contains("zirk_rt_scope_enter_collect_all"), "{llvm}");
    assert!(llvm.contains("zirk_rt_scope_exit_collect_all"), "{llvm}");
}

#[test]
fn typed_channel_operations_emit_the_channel_abi_and_typed_outcomes() {
    let ir = llvm_ir(
        "fn main(): Void {\n\
           mut channel: Channel<Int32> = Channel(2);\n\
           channel.send(1);\n\
           mut sent: SendOutcome = channel.try_send(2);\n\
           mut received: ReceiveOutcome<Int32> = channel.try_receive();\n\
           mut value: Int32? = channel.receive();\n\
           mut values: List<Int32> = channel.drain();\n\
           channel.close();\n\
         }",
    );
    for symbol in [
        symbols::CHAN_NEW,
        symbols::CHAN_SEND,
        symbols::CHAN_TRY_SEND,
        symbols::CHAN_TRY_RECV,
        symbols::CHAN_RECV,
        symbols::CHAN_DRAIN,
        symbols::CHAN_CLOSE,
    ] {
        assert!(
            ir.contains(&format!("@{symbol}")),
            "missing channel ABI call {symbol}:\n{ir}"
        );
    }
    assert!(
        ir.contains("channel_try_tag")
            && ir.contains("channel_recv")
            && ir.contains("channel_result"),
        "channel calls must reconstruct typed status/nullable values:\n{ir}"
    );
    assert!(
        ir.contains("channel is closed") || ir.contains("E_NATIVE_ERROR"),
        "send must route the closed status through the native failure path:\n{ir}"
    );
}

#[test]
fn map_and_set_constructors_receive_content_key_semantics_for_string_and_char() {
    let ir = llvm_ir(
        "fn main(): Void {\n\
           mut strings: Map<String, String> = Map();\n\
           mut chars: Set<Char> = Set();\n\
           mut words: Map<Int32, Int32> = Map();\n\
           mut numbers: Set<Int32> = Set();\n\
         }",
    );
    assert!(ir.contains(&format!("@{}", symbols::MAP_NEW_WITH_LAYOUT)));
    assert!(ir.contains(&format!("@{}", symbols::SET_NEW_WITH_LAYOUT)));
    let content_map_calls = ir
        .matches(&format!("call ptr @{}", symbols::MAP_NEW_WITH_LAYOUT))
        .count();
    let content_set_calls = ir
        .matches(&format!("call ptr @{}", symbols::SET_NEW_WITH_LAYOUT))
        .count();
    assert_eq!(
        content_map_calls, 2,
        "all Map constructors use the typed ABI:\n{ir}"
    );
    assert_eq!(
        content_set_calls, 2,
        "all Set constructors use the typed ABI:\n{ir}"
    );
    assert!(
        ir.contains("i1 true"),
        "String/Char constructors pass content mode:\n{ir}"
    );
    assert!(
        ir.contains("i1 false"),
        "scalar constructors pass word mode:\n{ir}"
    );
    assert!(
        !ir.contains("zk.map.key_offsets."),
        "snapshotted String keys must not retain the mutable key handle:\n{ir}"
    );
    assert!(
        !ir.contains("zk.set.element_offsets."),
        "snapshotted Char elements must not retain the mutable handle:\n{ir}"
    );
    assert!(
        ir.contains("zk.map.value_offsets."),
        "managed Map values must still be traced:\n{ir}"
    );
}

#[test]
fn array_and_list_constructors_emit_nested_aggregate_reference_offsets() {
    let ir = llvm_ir(
        "record Inner { text: String; }\n\
         record Outer { prefix: Int64; inner: Inner; }\n\
         fn main(): Void {\n\
           mut arrays: Array<Outer> = Array(2);\n\
           mut lists: List<Outer> = List();\n\
         }",
    );
    assert!(ir.contains(&format!("@{}", symbols::ARRAY_NEW_WITH_LAYOUT)));
    assert!(ir.contains(&format!("@{}", symbols::LIST_NEW_WITH_LAYOUT)));
    assert!(ir.contains("zk.array.offsets."));
    assert!(ir.contains("zk.list.offsets."));
    assert!(
        ir.contains("i64 8"),
        "nested field path is expressed as an element-relative offset:\n{ir}"
    );
}

#[test]
fn array_clone_and_slice_preserve_nested_aggregate_reference_offsets() {
    let ir = llvm_ir(
        "record Inner { text: String; }\n\
         record Outer { prefix: Int64; inner: Inner; }\n\
         fn main(): Void {\n\
           mut arrays: Array<Outer> = Array(2);\n\
           mut cloned = arrays.clone();\n\
           mut sliced = arrays[0: 1];\n\
         }",
    );
    assert!(ir.contains(&format!("@{}", symbols::ARRAY_CLONE_WITH_LAYOUT)));
    assert!(ir.contains(&format!("@{}", symbols::ARRAY_SLICE_WITH_LAYOUT)));
    assert!(ir.contains("zk.array.clone.offsets."));
    assert!(ir.contains("zk.array.slice.offsets."));
}

#[test]
fn thread_run_emits_managed_callback_adapter_and_rooted_output_abi() {
    use zirk_ir::{
        Block, BlockId, ClosureLayout, Function, Instruction, IrType, Module, ObjectLayout, Slot,
        SlotId, ValueId,
    };
    let span = zirk_diagnostics::Span::empty(0);
    let ir = Module {
        functions: vec![Function {
            name: "thread_run_probe".into(),
            params: vec![],
            return_type: IrType::Void,
            slots: vec![
                Slot {
                    name: "callable".into(),
                    ty: IrType::Callable(0),
                    span,
                },
                Slot {
                    name: "result".into(),
                    ty: IrType::String,
                    span,
                },
                Slot {
                    name: "exception".into(),
                    ty: IrType::Object(0),
                    span,
                },
            ],
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: zirk_ir::InstKind::Load(SlotId(0)),
                        ty: IrType::Callable(0),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: zirk_ir::InstKind::ThreadRun {
                            callable_slot: SlotId(0),
                            result_slot: Some(SlotId(1)),
                            exception_slot: SlotId(2),
                        },
                        ty: IrType::Int(zirk_ir::IntWidth::U32),
                        span,
                    },
                ],
                terminator: Some(zirk_ir::Terminator::Return(None)),
            }],
            entry: BlockId(0),
            span,
            gc_roots: vec![SlotId(0), SlotId(1), SlotId(2)],
        }],
        closures: vec![ClosureLayout {
            captures: vec![IrType::String],
            params: vec![],
            returns: IrType::String,
        }],
        objects: vec![ObjectLayout {
            name: "Throwable".into(),
            fields: vec![],
            contracts: vec![],
            methods: vec![],
            ancestors: vec![0],
        }],
        ..Module::default()
    };
    zirk_ir::verify(&ir)
        .expect("ThreadRun IR has a zero-argument rooted callable and output slots");
    let text = llvm_ir_for_module(&ir);
    assert!(
        text.contains("declare i32 @zirk_rt_thread_run(ptr, ptr, ptr, ptr, ptr)"),
        "{text}"
    );
    assert!(
        text.contains("define internal i32 @zirk.thread.run.thunk(ptr"),
        "{text}"
    );
    assert!(
        text.contains("call i32 @zirk_rt_thread_run(ptr @zirk.thread.run.thunk"),
        "{text}"
    );
    assert!(
        text.contains("store ptr %thread.callback"),
        "managed callback result is published into its rooted slot:\n{text}"
    );
    assert!(
        text.contains("store ptr %callback.exception"),
        "callback exceptions are published into the caller's rooted slot:\n{text}"
    );
}

#[test]
fn borrowed_view_raw_llvm_emission_fails_closed_until_payload_lowering_exists() {
    use zirk_ir::{
        Block, BlockId, Function, InstKind, Instruction, IrType, Module, Slot, SlotId, ValueId,
    };
    let span = zirk_diagnostics::Span::empty(0);
    let ir = Module {
        functions: vec![Function {
            name: "borrow_view_probe".into(),
            params: vec![],
            return_type: IrType::Void,
            slots: vec![Slot {
                name: "mutex".into(),
                ty: IrType::Mutex(0),
                span,
            }],
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::Undefined,
                        ty: IrType::Mutex(0),
                        span,
                    },
                    Instruction {
                        result: None,
                        kind: InstKind::Store(SlotId(0), zirk_ir::Operand(ValueId(0))),
                        ty: IrType::Void,
                        span,
                    },
                    Instruction {
                        result: None,
                        kind: InstKind::MutexAcquire {
                            id: zirk_ir::MutexId(1),
                            mutex: SlotId(0),
                        },
                        ty: IrType::Void,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::Load(SlotId(0)),
                        ty: IrType::Mutex(0),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: InstKind::BorrowViewStart {
                            region: 1,
                            mutex: zirk_ir::Operand(ValueId(1)),
                        },
                        ty: IrType::BorrowedView {
                            region: 1,
                            mutex_type: 0,
                        },
                        span,
                    },
                    Instruction {
                        result: None,
                        kind: InstKind::BorrowViewEnd {
                            view: zirk_ir::Operand(ValueId(2)),
                        },
                        ty: IrType::Void,
                        span,
                    },
                    Instruction {
                        result: None,
                        kind: InstKind::MutexRelease {
                            id: zirk_ir::MutexId(1),
                            mutex: SlotId(0),
                        },
                        ty: IrType::Void,
                        span,
                    },
                ],
                terminator: Some(zirk_ir::Terminator::Return(None)),
            }],
            entry: BlockId(0),
            span,
            gc_roots: vec![SlotId(0)],
        }],
        mutex_types: vec![IrType::Int(zirk_ir::IntWidth::I32)],
        ..Module::default()
    };
    zirk_ir::verify(&ir).expect("borrowed view lifetime token verifies");
    let text = llvm_ir_for_module(&ir);
    assert!(text.contains("define void @zk_borrow_view_probe"), "{text}");
    assert_eq!(text.matches("call void @llvm.trap()").count(), 4, "{text}");
}

#[test]
fn mutex_with_emits_runtime_adapter_result_and_rooted_nested_captures() {
    use zirk_ir::{
        Block, BlockId, ClosureLayout, Function, InstKind, Instruction, IntWidth, IrType, Module,
        ObjectField, Slot, SlotId, ValueId, ValueLayout,
    };
    let span = zirk_diagnostics::Span::empty(0);
    let payload = IrType::Value(1);
    let view_ty = IrType::BorrowedView {
        region: 17,
        mutex_type: 0,
    };
    let ir = Module {
        functions: vec![
            Function {
                name: "mutex_caller".into(),
                params: vec![],
                return_type: IrType::Void,
                slots: vec![Slot {
                    name: "result".into(),
                    ty: payload,
                    span,
                }],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::Undefined,
                            ty: IrType::Mutex(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::Undefined,
                            ty: IrType::Value(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::MakeCallable {
                                target: "mutex_callback".into(),
                                captures: vec![zirk_ir::Operand(ValueId(1))],
                            },
                            ty: IrType::Callable(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(3)),
                            kind: InstKind::MutexWithCall {
                                mutex: zirk_ir::Operand(ValueId(0)),
                                callback: zirk_ir::Operand(ValueId(2)),
                                layout: 0,
                                result_slot: Some(SlotId(0)),
                            },
                            ty: IrType::Int(IntWidth::I32),
                            span,
                        },
                    ],
                    terminator: Some(zirk_ir::Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span,
                gc_roots: vec![SlotId(0)],
            },
            Function {
                name: "mutex_callback".into(),
                params: vec![SlotId(0), SlotId(1)],
                return_type: payload,
                slots: vec![
                    Slot {
                        name: "capture".into(),
                        ty: IrType::Value(0),
                        span,
                    },
                    Slot {
                        name: "view".into(),
                        ty: view_ty,
                        span,
                    },
                ],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::Load(SlotId(1)),
                            ty: view_ty,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::BorrowViewLoad {
                                view: zirk_ir::Operand(ValueId(0)),
                            },
                            ty: payload,
                            span,
                        },
                    ],
                    terminator: Some(zirk_ir::Terminator::Return(Some(zirk_ir::Operand(
                        ValueId(1),
                    )))),
                }],
                entry: BlockId(0),
                span,
                gc_roots: vec![SlotId(0)],
            },
        ],
        closures: vec![ClosureLayout {
            captures: vec![IrType::Value(0)],
            params: vec![view_ty],
            returns: payload,
        }],
        values: vec![
            ValueLayout {
                name: "NestedCapture".into(),
                fields: vec![ObjectField {
                    name: "text".into(),
                    ty: IrType::String,
                }],
            },
            ValueLayout {
                name: "Payload".into(),
                fields: vec![ObjectField {
                    name: "text".into(),
                    ty: IrType::String,
                }],
            },
        ],
        mutex_types: vec![payload],
        ..Module::default()
    };
    zirk_ir::verify(&ir).expect("typed mutex callback IR verifies");
    let text = llvm_ir_for_module(&ir);
    assert!(
        text.contains("declare i32 @zirk_rt_gc_mutex_with(ptr, ptr, ptr)"),
        "{text}"
    );
    assert!(
        text.contains("define internal void @zirk.mutex.with.thunk.0(ptr"),
        "{text}"
    );
    assert!(
        text.contains("call i32 @zirk_rt_gc_mutex_with(ptr "),
        "runtime call uses the declared i32(ptr, ptr, ptr) ABI:\n{text}"
    );
    assert!(
        text.contains("ptr @zirk.mutex.with.thunk.0, ptr %mutex.callback.context)"),
        "runtime receives the callback adapter and synchronous context:\n{text}"
    );
    assert!(
        text.contains("call { ptr } %mutex.callback.fn({ ptr } %mutex.capture, ptr %0)"),
        "{text}"
    );
    assert!(
        text.contains("store { ptr } %mutex.callback"),
        "callback result is stored only after return:\n{text}"
    );
    assert!(
        text.contains("call void @zirk_rt_push_frame"),
        "capture block is temporarily rooted across lock acquisition:\n{text}"
    );
    assert!(
        text.contains("call void @zirk_rt_pop_frame"),
        "temporary callback root is released after runtime returns:\n{text}"
    );
    assert!(
        text.contains("zk.closure.type.0"),
        "nested managed capture descriptor is emitted:\n{text}"
    );
}

#[test]
fn mutex_new_emits_typed_allocation_and_roots_nested_payload_references() {
    use zirk_ir::{
        Block, BlockId, Function, InstKind, Instruction, IntWidth, IrType, Module, ObjectField,
        Slot, SlotId, ValueId, ValueLayout,
    };
    let span = zirk_diagnostics::Span::empty(0);
    let payload = IrType::Value(0);
    let ir = Module {
        functions: vec![Function {
            name: "mutex_new_probe".into(),
            params: vec![],
            return_type: IrType::Void,
            slots: vec![Slot {
                name: "mutex".into(),
                ty: IrType::Mutex(0),
                span,
            }],
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: InstKind::Undefined,
                        ty: payload,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: InstKind::MutexNew {
                            payload: zirk_ir::Operand(ValueId(0)),
                            mutex_type: 0,
                            object_slot: SlotId(0),
                        },
                        ty: IrType::Int(IntWidth::I32),
                        span,
                    },
                ],
                terminator: Some(zirk_ir::Terminator::Return(None)),
            }],
            entry: BlockId(0),
            span,
            gc_roots: vec![SlotId(0)],
        }],
        values: vec![ValueLayout {
            name: "Payload".into(),
            fields: vec![ObjectField {
                name: "text".into(),
                ty: IrType::String,
            }],
        }],
        mutex_types: vec![payload],
        ..Module::default()
    };
    zirk_ir::verify(&ir).expect("MutexNew IR is well formed");
    let text = llvm_ir_for_module(&ir);
    assert!(
        text.contains("declare ptr @zirk_rt_gc_mutex_alloc(ptr, i64, i64, ptr, i64, ptr)"),
        "{text}"
    );
    assert!(
        text.contains("call void @zirk_rt_push_frame"),
        "staging payload reference is rooted during allocation:\n{text}"
    );
    assert!(text.contains("call ptr @zirk_rt_gc_mutex_alloc"), "{text}");
    assert!(
        text.contains("call void @zirk_rt_pop_frame"),
        "temporary payload root is released after allocation:\n{text}"
    );
    assert!(
        text.contains("store ptr %mutex.new.handle"),
        "allocated handle is stored in destination slot:\n{text}"
    );
}

#[test]
fn borrowed_value_field_projection_emits_typed_gep_access() {
    use zirk_ir::{
        Block, BlockId, ClosureLayout, Function, InstKind, Instruction, IntWidth, IrType, Module,
        ObjectField, Slot, SlotId, ValueId, ValueLayout,
    };
    let span = zirk_diagnostics::Span::empty(0);
    let payload = IrType::Value(0);
    let view = IrType::BorrowedView {
        region: 9,
        mutex_type: 0,
    };
    let ir = Module {
        functions: vec![
            Function {
                name: "caller".into(),
                params: vec![],
                return_type: IrType::Void,
                slots: vec![],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::Undefined,
                            ty: IrType::Mutex(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::MakeCallable {
                                target: "callback".into(),
                                captures: vec![],
                            },
                            ty: IrType::Callable(0),
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(2)),
                            kind: InstKind::MutexWithCall {
                                mutex: zirk_ir::Operand(ValueId(0)),
                                callback: zirk_ir::Operand(ValueId(1)),
                                layout: 0,
                                result_slot: None,
                            },
                            ty: IrType::Int(IntWidth::I32),
                            span,
                        },
                    ],
                    terminator: Some(zirk_ir::Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span,
                gc_roots: vec![],
            },
            Function {
                name: "callback".into(),
                params: vec![SlotId(0)],
                return_type: IrType::Void,
                slots: vec![Slot {
                    name: "view".into(),
                    ty: view,
                    span,
                }],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        Instruction {
                            result: Some(ValueId(0)),
                            kind: InstKind::Load(SlotId(0)),
                            ty: view,
                            span,
                        },
                        Instruction {
                            result: None,
                            kind: InstKind::BorrowViewFieldStore {
                                view: zirk_ir::Operand(ValueId(0)),
                                path: vec![0],
                                value: zirk_ir::Operand(ValueId(1)),
                            },
                            ty: IrType::Void,
                            span,
                        },
                        Instruction {
                            result: Some(ValueId(1)),
                            kind: InstKind::ConstInt(4),
                            ty: IrType::Int(IntWidth::I32),
                            span,
                        },
                    ],
                    terminator: Some(zirk_ir::Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span,
                gc_roots: vec![],
            },
        ],
        closures: vec![ClosureLayout {
            captures: vec![],
            params: vec![view],
            returns: IrType::Void,
        }],
        values: vec![ValueLayout {
            name: "Payload".into(),
            fields: vec![ObjectField {
                name: "count".into(),
                ty: IrType::Int(IntWidth::I32),
            }],
        }],
        mutex_types: vec![payload],
        ..Module::default()
    };
    // The value operand is defined before use in instruction order.
    let mut ir = ir;
    ir.functions[1].blocks[0].instructions.swap(1, 2);
    zirk_ir::verify(&ir).expect("borrowed field projection IR is valid");
    let text = llvm_ir_for_module(&ir);
    assert!(
        text.contains("mutex.field.load") || text.contains("mutex.payload.path"),
        "{text}"
    );
    assert!(
        text.contains("store i32"),
        "typed field store is emitted:\n{text}"
    );
}
