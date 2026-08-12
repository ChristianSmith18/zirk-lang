//! # zirk-sema
//!
//! **Responsabilidad:** resolución de módulos y nombres, chequeo de tipos y
//! análisis de flujo sobre el árbol de `zirk-ast`.
//!
//! Cubre las etapas 3 y 4 del pipeline de `ZIRK_COMPILER_SPEC.md` sección 2: es
//! donde se aplican las reglas de `ZIRK_LANGUAGE_SPEC.md` sobre mutabilidad
//! (`mut` / `inmut` / `inmut::strict`), nullability, e inferencia.
//!
//! **Límite:** produce un árbol tipado y verificado; no genera código ni decide
//! representación de datos en memoria. Cómo se materializa un valor es problema
//! de `zirk-ir` y del runtime.
//!
//! # Estado
//!
//! Vacío por diseño. La Fase 0 monta el esqueleto del workspace sin implementar
//! sintaxis de Zirk; el chequeo de tipos llega en Fase 1 con un subset mínimo.
