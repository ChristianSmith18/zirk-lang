//! # zirk-ir
//!
//! **Responsabilidad:** la representación intermedia tipada, independiente del
//! target y versionada que define `ZIRK_COMPILER_SPEC.md` sección 4.
//!
//! Es la frontera entre el frontend y el backend: todo lo que llega a codegen
//! pasa por acá. También es lo que se distribuye dentro de un `.zpkg` como
//! `portable.ir`, así que su formato es un contrato, no un detalle interno.
//!
//! **Límite:** la IR no conoce LLVM. Traducirla a LLVM IR es responsabilidad de
//! `zirk-codegen-llvm`, y esa separación es lo que permite añadir otro backend
//! sin cambiar la semántica pública.
//!
//! # Regla de memoria
//!
//! La IR **no asume un modelo de memoria concreto**. Toda alocación se expresa
//! como una operación abstracta que resuelve el runtime. La estrategia de
//! memoria se decide en Fase 4 (ver `docs/decisions/ADR-003-memoria.md`) y la IR
//! no debe adelantarla con asunciones tácitas.
//!
//! # Estado
//!
//! Vacío por diseño. La Fase 0 monta el esqueleto del workspace sin implementar
//! sintaxis de Zirk; la IR mínima llega en Fase 1.
