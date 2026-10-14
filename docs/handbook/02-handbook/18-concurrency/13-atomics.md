# Atomics

> **Estado: API escalar disponible en el compilador y el runtime nativo.** La tabla
> [Phase 5 — Concurrency and parallelism](../../../init/ZIRK_FEATURE_STATUS.md#phase-5-concurrency-and-parallelism)
> registra su implementación. `Atomic<T>` admite `Boolean` y enteros con signo
> o sin signo de 8, 16, 32 y 64 bits, únicamente cuando el target ofrece
> operaciones atómicas para ese ancho. La ejecución E2E se verificó en el host
> nativo. El runtime también compila para nueve triples LLVM configurados y
> `wasm32-unknown-unknown`; esas comprobaciones son de compilación y no prueban
> ejecución ni soporte funcional en esos targets. Los atomics de punteros,
> incluidos los punteros gestionados, no están soportados.

`Atomic(initial)` construye el valor gestionado. Sus operaciones incluyen
`load`, `store`, `exchange`, `compare_exchange` y, para enteros,
`fetch_add`/`fetch_sub` con aritmética modular. Compare-exchange devuelve
`Result<T,T>` con el valor observado tanto en éxito como en fallo. Las
operaciones usan `SeqCst` por defecto. Los ordenamientos más débiles deben
indicarse dentro de `unsafe`; las combinaciones incompatibles conocidas
estáticamente producen un diagnóstico de compilación. Si la combinación solo
se conoce en ejecución, la operación lanza `InvalidArgumentError` antes de
acceder al valor.

El escalar se almacena inline en el objeto `Atomic<T>` administrado por el GC;
no se crea un handle nativo separado. Los atomics protegen operaciones
individuales, no invariantes de varios pasos, y no se promete comportamiento
lock-free para todos los targets.

---

**Previous:** [← Synchronization and Mutexes](12-sync-and-mutex.md) · **Next:** [ Data-Race Prevention](14-data-race-prevention.md)
