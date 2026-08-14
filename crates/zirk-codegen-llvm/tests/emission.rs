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
use zirk_codegen_llvm::{emit, symbols};
use zirk_diagnostics::{DiagnosticSink, RenderStyle, SourceFile, SourceMap};
use zirk_ir::lower;
use zirk_lexer::tokenize;
use zirk_parser::parse;
use zirk_sema::check;

/// Compiles source down to LLVM IR text, asserting the module verifies.
fn llvm_ir(source_text: &str) -> String {
    let _llvm = common::llvm_lock();

    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("test.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert!(
        !sink.has_errors(),
        "the program must be valid:\n{}",
        sink.render(RenderStyle::Human)
    );

    let ir = lower(&program, &checked);
    zirk_ir::verify(&ir).expect("the IR must be well formed before emitting");

    let context = Context::create();
    let module = emit(&context, &ir, "test");

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

// --- Module shape -----------------------------------------------------------

#[test]
fn the_minimal_program_verifies() {
    let ir = llvm_ir("fn main(): Void { }");
    assert!(ir.contains("define void @zk_main()"));
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
    let shutdown = entrypoint
        .find(symbols::SHUTDOWN)
        .expect("calls the shutdown");

    assert!(
        init < zirk && zirk < shutdown,
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
    assert!(ir.contains(symbols::OVERFLOW));
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
    let ir = llvm_ir(&in_main(
        "mut a: Int32 = 10;\nmut b: Int32 = 2;\nmut c: Int32 = a / b;",
    ));
    assert!(
        ir.contains(symbols::DIVISION_BY_ZERO),
        "division by zero must not be undefined behaviour:\n{ir}"
    );
    assert!(ir.contains("sdiv i32"));
}

#[test]
fn the_remainder_also_checks_the_divisor() {
    let ir = llvm_ir(&in_main(
        "mut a: Int32 = 10;\nmut b: Int32 = 3;\nmut c: Int32 = a % b;",
    ));
    assert!(ir.contains(symbols::DIVISION_BY_ZERO));
    assert!(ir.contains("srem i32"));
}

#[test]
fn the_failure_handlers_do_not_return() {
    let ir = llvm_ir(&in_main("mut a: Int32 = 1;\nmut x: Int32 = a + 2;"));
    assert!(
        ir.contains("unreachable"),
        "after calling the handler nothing executes:\n{ir}"
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
    assert!(ir.contains(" and "), "{ir}");
    assert!(!ir.contains("llvm.sadd"));
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
