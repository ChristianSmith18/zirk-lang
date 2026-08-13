//! Sanity check of the LLVM toolchain.
//!
//! Verifies the full chain required by `ZIRK_ROADMAP.md` Phase 0:
//!
//! ```text
//! inkwell -> LLVM IR -> native object -> link -> a binary that runs
//! ```
//!
//! No Zirk syntax is involved: it validates the toolchain, not the language.
//!
//! It lives in the repo as a permanent test rather than a throwaway spike
//! (design D2): its value is not spent by passing once. It is what detects that
//! someone has the wrong LLVM version, that a CI runner lost
//! `LLVM_SYS_201_PREFIX`, or that an inkwell update broke emission.

mod common;

use inkwell::AddressSpace;
use inkwell::context::Context;
use std::process::Command;
use zirk_codegen_llvm::emit_object_for_host;

const MESSAGE: &str = "zirk sanity check";

#[test]
fn the_full_chain_produces_a_native_binary_that_runs() {
    let _llvm = common::llvm_lock();

    eprintln!("[sanity] creating LLVM context");
    let context = Context::create();
    let module = context.create_module("sanity");
    let builder = context.create_builder();

    // declare i32 @puts(ptr)
    let i32_type = context.i32_type();
    let ptr_type = context.ptr_type(AddressSpace::default());
    let puts = module.add_function("puts", i32_type.fn_type(&[ptr_type.into()], false), None);

    // define i32 @main() { puts(MESSAGE); ret 0 }
    let main_fn = module.add_function("main", i32_type.fn_type(&[], false), None);
    let entry = context.append_basic_block(main_fn, "entry");
    builder.position_at_end(entry);

    let mensaje = builder
        .build_global_string_ptr(MESSAGE, "mensaje")
        .expect("could not build the string constant");
    builder
        .build_call(puts, &[mensaje.as_pointer_value().into()], "")
        .expect("could not build the call to puts");
    builder
        .build_return(Some(&i32_type.const_int(0, false)))
        .expect("could not build the return");

    // inkwell's `Module::verify()` triggers STATUS_ACCESS_VIOLATION on Windows
    // even with a valid module. See issue #2.
    //
    // Disabling it there leaves nothing uncovered: the test still emits the
    // object, links it and runs the binary, so an invalid module fails all the
    // same — only later and with a worse message.
    #[cfg(not(windows))]
    {
        eprintln!("[sanity] verifying module");
        module
            .verify()
            .expect("the generated LLVM module does not verify");
    }

    // Emit the object for the host.
    let dir = common::temp_dir();
    let object = dir.join(format!(
        "sanity{}",
        if cfg!(windows) { ".obj" } else { ".o" }
    ));
    // The triple is resolved separately so the log distinguishes a failure to
    // query it from a failure to emit. See #2.
    eprintln!("[sanity] resolving host triple");
    let triple = zirk_codegen_llvm::host_triple();
    eprintln!("[sanity] host triple: {triple}");

    eprintln!("[sanity] emitting object for the host");
    emit_object_for_host(&module, &object)
        .unwrap_or_else(|d| panic!("failed to emit the object:\n{}", d.render()));

    assert!(object.is_file(), "the object was not written to disk");

    // Link.
    let binary = dir.join(common::exe("sanity"));
    eprintln!("[sanity] enlazando");
    let linker = common::linker_driver();
    let link_output = Command::new(&linker)
        .arg(&object)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap_or_else(|e| panic!("could not invoke linker `{}`: {e}", linker.display()));

    assert!(
        link_output.status.success(),
        "linking failed:\n{}",
        String::from_utf8_lossy(&link_output.stderr)
    );

    // Run.
    eprintln!("[sanity] running binary");
    let output = Command::new(&binary)
        .output()
        .expect("could not run the produced binary");

    assert!(
        output.status.success(),
        "the binary exited with code {:?}",
        output.status.code()
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(MESSAGE),
        "the binary did not print the expected message"
    );
}

#[test]
fn an_unknown_triple_fails_with_a_diagnostic() {
    let _llvm = common::llvm_lock();

    let context = Context::create();
    let module = context.create_module("invalido");
    let dir = common::temp_dir();

    let error = zirk_codegen_llvm::emit_object_for_triple(
        &module,
        "arquitectura-que-no-existe",
        &dir.join("nunca.o"),
    )
    .expect_err("an invalid triple must not emit an object");

    let rendered = error.render();
    assert!(
        rendered.contains("arquitectura-que-no-existe"),
        "the diagnostic must name the requested target:\n{rendered}"
    );
    assert!(rendered.contains("= cause:"), "missing cause:\n{rendered}");
    assert!(rendered.contains("= help:"), "missing help:\n{rendered}");
    assert!(
        !dir.join("nunca.o").exists(),
        "no invalid object must be produced"
    );
}
