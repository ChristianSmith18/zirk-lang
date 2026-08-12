//! Verifica la emisión de objetos para los nueve targets de
//! `ZIRK_COMPILER_SPEC.md` sección 6, desde el host que ejecute el test.
//!
//! Corresponde a la *portabilidad B* de `docs/decisions/ADR-004-portabilidad.md`:
//! qué produce el compilador, con independencia de dónde corre.
//!
//! No se verifica el **enlace** para otros targets: eso requiere sysroots de
//! destino y queda fuera de alcance en esta fase.

mod common;

use common::{DetectedContainer, detect_container, detect_machine};
use inkwell::context::Context;
use zirk_codegen_llvm::{Container, TARGETS, emit_object};

/// Tipo de máquina COFF esperado por arquitectura.
fn coff_machine_esperada(nombre: &str) -> u16 {
    match nombre {
        "x86-windows" => 0x014C,
        "x86_64-windows" => 0x8664,
        "aarch64-windows" => 0xAA64,
        otro => panic!("target COFF no contemplado: {otro}"),
    }
}

fn contenedor_esperado(container: Container) -> DetectedContainer {
    match container {
        Container::MachO => DetectedContainer::MachO,
        Container::Elf => DetectedContainer::Elf,
        Container::Coff => DetectedContainer::Coff,
    }
}

#[test]
fn se_emite_un_objeto_valido_para_cada_target_del_spec() {
    let dir = common::temp_dir();

    for target in TARGETS {
        let context = Context::create();
        let module = context.create_module("matriz");
        let builder = context.create_builder();

        // Una función trivial basta: lo que se verifica es la emisión y el
        // formato del contenedor, no la calidad del código generado.
        let i32_type = context.i32_type();
        let f = module.add_function("zirk_probe", i32_type.fn_type(&[], false), None);
        let entry = context.append_basic_block(f, "entry");
        builder.position_at_end(entry);
        builder
            .build_return(Some(&i32_type.const_int(42, false)))
            .expect("no se pudo construir el return");

        let objeto = dir.join(format!("matriz-{}.o", target.name));
        emit_object(&module, target, &objeto)
            .unwrap_or_else(|d| panic!("fallo al emitir para `{}`:\n{}", target.name, d.render()));

        let bytes = std::fs::read(&objeto)
            .unwrap_or_else(|e| panic!("no se pudo leer el objeto de `{}`: {e}", target.name));

        assert!(
            !bytes.is_empty(),
            "el objeto de `{}` está vacío",
            target.name
        );

        assert_eq!(
            detect_container(&bytes),
            contenedor_esperado(target.container),
            "contenedor incorrecto para `{}`",
            target.name
        );

        if target.container == Container::Coff {
            assert_eq!(
                detect_machine(&bytes),
                Some(coff_machine_esperada(target.name)),
                "arquitectura COFF incorrecta para `{}`",
                target.name
            );
        }
    }
}

#[test]
fn la_matriz_cubre_las_tres_plataformas() {
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

    assert!(coff > 0, "faltan targets de Windows");
    assert!(elf > 0, "faltan targets de Linux");
    assert!(macho > 0, "faltan targets de macOS");
    assert_eq!(coff + elf + macho, TARGETS.len());
}
