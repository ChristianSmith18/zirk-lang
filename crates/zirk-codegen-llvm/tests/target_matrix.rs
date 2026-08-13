//! Verifies object emission for the nine targets of `ZIRK_COMPILER_SPEC.md`
//! section 6, from whichever host runs the test.
//!
//! This is *portability B* from `docs/decisions/ADR-004-portabilidad.md`: what
//! the compiler produces, independently of where it runs.
//!
//! **Linking** for other targets is not verified: that requires destination
//! sysroots and is out of scope in this phase.

mod common;

use common::{DetectedContainer, detect_container, detect_machine};
use inkwell::context::Context;
use zirk_codegen_llvm::{Container, TARGETS, emit_object};

/// Expected COFF machine type per architecture.
fn expected_coff_machine(name: &str) -> u16 {
    match name {
        "x86-windows" => 0x014C,
        "x86_64-windows" => 0x8664,
        "aarch64-windows" => 0xAA64,
        other => panic!("unhandled COFF target: {other}"),
    }
}

fn expected_container(container: Container) -> DetectedContainer {
    match container {
        Container::MachO => DetectedContainer::MachO,
        Container::Elf => DetectedContainer::Elf,
        Container::Coff => DetectedContainer::Coff,
    }
}

#[test]
fn a_valid_object_is_emitted_for_every_target_of_the_spec() {
    let _llvm = common::llvm_lock();

    let dir = common::temp_dir();

    for target in TARGETS {
        let context = Context::create();
        let module = context.create_module("matriz");
        let builder = context.create_builder();

        // A trivial function is enough: what is verified is emission and the
        // container format, not the quality of the generated code.
        let i32_type = context.i32_type();
        let f = module.add_function("zirk_probe", i32_type.fn_type(&[], false), None);
        let entry = context.append_basic_block(f, "entry");
        builder.position_at_end(entry);
        builder
            .build_return(Some(&i32_type.const_int(42, false)))
            .expect("could not build the return");

        let object = dir.join(format!("matriz-{}.o", target.name));
        emit_object(&module, target, &object)
            .unwrap_or_else(|d| panic!("failed to emit for `{}`:\n{}", target.name, d.render()));

        let bytes = std::fs::read(&object)
            .unwrap_or_else(|e| panic!("could not read the object for `{}`: {e}", target.name));

        assert!(
            !bytes.is_empty(),
            "the object for `{}` is empty",
            target.name
        );

        assert_eq!(
            detect_container(&bytes),
            expected_container(target.container),
            "wrong container for `{}`",
            target.name
        );

        if target.container == Container::Coff {
            assert_eq!(
                detect_machine(&bytes),
                Some(expected_coff_machine(target.name)),
                "wrong COFF architecture for `{}`",
                target.name
            );
        }
    }
}

#[test]
fn the_matrix_covers_the_three_platforms() {
    let coff = TARGETS
        .iter()
        .filter(|t| t.container == Container::Coff)
        .count();
    let elf = TARGETS
        .iter()
        .filter(|t| t.container == Container::Elf)
        .count();
    let macho = TARGETS
        .iter()
        .filter(|t| t.container == Container::MachO)
        .count();

    assert!(coff > 0, "Windows targets are missing");
    assert!(elf > 0, "Linux targets are missing");
    assert!(macho > 0, "macOS targets are missing");
    assert_eq!(coff + elf + macho, TARGETS.len());
}
