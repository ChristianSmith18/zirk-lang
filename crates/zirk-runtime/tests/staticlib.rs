//! Verifica que `zirk-runtime` produce una biblioteca estática enlazable y que
//! sus símbolos cruzan la frontera ABI C sin mangling.
//!
//! Los símbolos se buscan como texto dentro del archivo: la tabla de símbolos de
//! un `.a` (Unix) y de un `.lib` (Windows) contiene el nombre literal. Evita
//! depender de `nm`, `objdump` o `dumpbin`, que difieren entre plataformas.

use std::path::PathBuf;

/// Localiza el directorio de artefactos a partir del ejecutable de test,
/// que Cargo ubica en `<target>/<profile>/deps/`.
fn profile_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("no se pudo ubicar el ejecutable de test");
    exe.parent()
        .and_then(|deps| deps.parent())
        .expect("estructura de directorios de Cargo inesperada")
        .to_path_buf()
}

fn staticlib_path() -> PathBuf {
    let dir = profile_dir();
    let candidatos = if cfg!(windows) {
        vec![dir.join("zirk_runtime.lib")]
    } else {
        vec![dir.join("libzirk_runtime.a")]
    };

    candidatos
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| {
            panic!(
                "no se encontró la biblioteca estática en {}; \
                 ¿el crate-type `staticlib` sigue declarado en Cargo.toml?",
                dir.display()
            )
        })
}

#[test]
fn se_produce_una_biblioteca_estatica() {
    let lib = staticlib_path();
    let metadata = std::fs::metadata(&lib).expect("no se pudo leer la biblioteca");

    assert!(
        metadata.len() > 0,
        "la biblioteca estática está vacía: {}",
        lib.display()
    );
}

#[test]
fn los_simbolos_del_ciclo_de_vida_no_llevan_mangling() {
    let lib = staticlib_path();
    let bytes = std::fs::read(&lib).expect("no se pudo leer la biblioteca");

    for simbolo in ["zirk_rt_init", "zirk_rt_shutdown"] {
        assert!(
            contiene(&bytes, simbolo.as_bytes()),
            "el símbolo `{simbolo}` no aparece sin mangling en {}",
            lib.display()
        );
    }
}

fn contiene(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|ventana| ventana == needle)
}
