//! Sanity check del toolchain de LLVM.
//!
//! Verifica la cadena completa exigida por `ZIRK_ROADMAP.md` Fase 0:
//!
//! ```text
//! inkwell → LLVM IR → objeto nativo → enlace → binario que ejecuta
//! ```
//!
//! No involucra sintaxis de Zirk: valida el toolchain, no el lenguaje.
//!
//! Vive en el repo como test permanente y no como spike desechable (design D2):
//! su valor no se agota al pasar una vez. Es lo que detecta que alguien tiene la
//! versión equivocada de LLVM, que un runner de CI perdió `LLVM_SYS_201_PREFIX`,
//! o que una actualización de inkwell rompió la emisión.

mod common;

use inkwell::AddressSpace;
use inkwell::context::Context;
use std::process::Command;
use zirk_codegen_llvm::emit_object_for_host;

const MENSAJE: &str = "sanity check de zirk";

#[test]
fn la_cadena_completa_produce_un_binario_nativo_que_ejecuta() {
    let _llvm = common::llvm_lock();

    eprintln!("[sanity] creando contexto LLVM");
    let context = Context::create();
    let module = context.create_module("sanity");
    let builder = context.create_builder();

    // declare i32 @puts(ptr)
    let i32_type = context.i32_type();
    let ptr_type = context.ptr_type(AddressSpace::default());
    let puts = module.add_function("puts", i32_type.fn_type(&[ptr_type.into()], false), None);

    // define i32 @main() { puts(MENSAJE); ret 0 }
    let main_fn = module.add_function("main", i32_type.fn_type(&[], false), None);
    let entry = context.append_basic_block(main_fn, "entry");
    builder.position_at_end(entry);

    let mensaje = builder
        .build_global_string_ptr(MENSAJE, "mensaje")
        .expect("no se pudo construir la constante de string");
    builder
        .build_call(puts, &[mensaje.as_pointer_value().into()], "")
        .expect("no se pudo construir la llamada a puts");
    builder
        .build_return(Some(&i32_type.const_int(0, false)))
        .expect("no se pudo construir el return");

    eprintln!("[sanity] verificando modulo");
    module
        .verify()
        .expect("el módulo LLVM generado no verifica");

    // Emisión del objeto para el host.
    let dir = common::temp_dir();
    let objeto = dir.join(format!(
        "sanity{}",
        if cfg!(windows) { ".obj" } else { ".o" }
    ));
    eprintln!("[sanity] emitiendo objeto para el host");
    emit_object_for_host(&module, &objeto)
        .unwrap_or_else(|d| panic!("fallo al emitir el objeto:\n{}", d.render()));

    assert!(objeto.is_file(), "el objeto no se escribió en disco");

    // Enlace.
    let binario = dir.join(common::exe("sanity"));
    eprintln!("[sanity] enlazando");
    let linker = common::linker_driver();
    let salida_enlace = Command::new(&linker)
        .arg(&objeto)
        .arg("-o")
        .arg(&binario)
        .output()
        .unwrap_or_else(|e| panic!("no se pudo invocar el linker `{}`: {e}", linker.display()));

    assert!(
        salida_enlace.status.success(),
        "el enlace falló:\n{}",
        String::from_utf8_lossy(&salida_enlace.stderr)
    );

    // Ejecución.
    eprintln!("[sanity] ejecutando binario");
    let salida = Command::new(&binario)
        .output()
        .expect("no se pudo ejecutar el binario producido");

    assert!(
        salida.status.success(),
        "el binario terminó con código {:?}",
        salida.status.code()
    );
    assert!(
        String::from_utf8_lossy(&salida.stdout).contains(MENSAJE),
        "el binario no imprimió el mensaje esperado"
    );
}

#[test]
fn un_triple_desconocido_falla_con_diagnostico() {
    let _llvm = common::llvm_lock();

    let context = Context::create();
    let module = context.create_module("invalido");
    let dir = common::temp_dir();

    let error = zirk_codegen_llvm::emit_object_for_triple(
        &module,
        "arquitectura-que-no-existe",
        &dir.join("nunca.o"),
    )
    .expect_err("un triple inválido no debe emitir un objeto");

    let rendered = error.render();
    assert!(
        rendered.contains("arquitectura-que-no-existe"),
        "el diagnóstico debe nombrar el target solicitado:\n{rendered}"
    );
    assert!(rendered.contains("= causa:"), "falta la causa:\n{rendered}");
    assert!(rendered.contains("= ayuda:"), "falta la ayuda:\n{rendered}");
    assert!(
        !dir.join("nunca.o").exists(),
        "no debe producirse un objeto inválido"
    );
}
