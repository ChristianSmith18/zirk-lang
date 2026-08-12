//! # zirk-runtime
//!
//! **Responsabilidad:** el runtime que se enlaza en cada binario que Zirk
//! produce. Provee el ciclo de vida de la aplicación, y más adelante memoria,
//! scheduler, channels y recursos.
//!
//! **Límite:** este crate **no** es parte del compilador y ningún crate del
//! compilador depende de él. Se compila a `staticlib` para el target del
//! programa compilado, no para el host donde corre `zirkc`.
//!
//! # Frontera ABI C
//!
//! Todo símbolo destinado al código generado se declara `extern "C"` sin
//! mangling. Es la misma frontera que `ZIRK_LANGUAGE_SPEC.md` sección 13 exige
//! para interoperabilidad nativa: no es andamiaje temporal, es la frontera
//! definitiva estrenada temprano (ver `docs/decisions/ADR-002-runtime-staticlib.md`).
//!
//! Estos símbolos son superficie de compatibilidad: cambiarlos rompe binarios ya
//! compilados.
//!
//! # Estado
//!
//! El ciclo de vida de `ZIRK_RUNTIME_SPEC.md` sección 2 es:
//!
//! ```text
//! validar init.zrk y permisos → cargar runtime mínimo → inicializar globals
//!   → main() → scopes de concurrencia → cierre de recursos → flush → exit
//! ```
//!
//! En Fase 0 solo existen los extremos de esa secuencia, con cuerpo vacío. Están
//! definidos ahora a propósito: fijan la forma donde Fase 4 (memoria) y Fase 5
//! (concurrencia) se cuelgan sin refactorizar el codegen.

/// Inicializa el runtime antes de ejecutar `main`.
///
/// Corresponde a los pasos "cargar runtime mínimo" e "inicializar globals" de
/// `ZIRK_RUNTIME_SPEC.md` sección 2.
///
/// # Safety
///
/// La invoca el código generado por Zirk, una sola vez, antes de cualquier otra
/// función del runtime. Llamarla más de una vez, o después de
/// [`zirk_rt_shutdown`], no está soportado.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_init() {
    // Fase 0: sin subsistemas que inicializar.
    //
    // `ZIRK_RUNTIME_SPEC.md` sección 1 pide inicialización diferida de los
    // subsistemas, así que este punto probablemente nunca haga trabajo pesado:
    // marca el inicio del ciclo de vida, no construye todo el runtime.
}

/// Cierra el runtime después de que `main` retorna.
///
/// Corresponde a "cierre ordenado de recursos y threads gestionados" y al
/// "flush de streams" de `ZIRK_RUNTIME_SPEC.md` sección 2.
///
/// # Safety
///
/// La invoca el código generado por Zirk, una sola vez, después de `main` y
/// antes de terminar el proceso. Usar cualquier función del runtime luego de
/// esta llamada no está soportado.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_shutdown() {
    // Fase 0: no hay recursos, threads ni streams propios que cerrar.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_ciclo_de_vida_minimo_es_invocable() {
        unsafe {
            zirk_rt_init();
            zirk_rt_shutdown();
        }
    }
}
