# Synchronization and Mutexes

> **Estado:** `Mutex<T>` y `mutex.with` están disponibles para programas
> nativos. `RwLock<T>`, `Semaphore`, `Barrier` y `Once<T>` siguen siendo
> diseños de runtime sin API de fuente. Consulta
> [Phase 5 — Concurrency and parallelism](../../../init/ZIRK_FEATURE_STATUS.md#phase-5-concurrency-and-parallelism)
> para la matriz completa.

`Mutex<T>` protege un payload administrado por el GC y expone acceso escribible
solo dentro de `mutex.with((value: T): T => { ... })`. La vista no puede escapar,
capturarse ni sobrevivir a la llamada. El callback no puede suspender, bloquear,
crear tareas ni cruzar puntos seguros; las lecturas de metadatos no asignantes
de tipos nativos están permitidas. La espera del mutex usa la cola cooperativa,
es FIFO y no reentrante; la cancelación quita al waiter de la cola. El runtime
desbloquea antes de propagar una excepción de Zirk y también libera durante un
unwind nativo. No existe estado de poisoning. Los otros sincronizadores de esta
página no están disponibles como API de fuente.

Prefiere canales o valores inmutables cuando la transferencia de propiedad represente mejor el diseño.

El diseño también contempla `RwLock<T>`, `Semaphore`, `Barrier` y `Once<T>` como
tipos de biblioteca estándar, no como sintaxis adicional. Esos tipos tampoco
deben considerarse APIs entregadas.

---

**Previous:** [← Workers by Composition](11-workers-by-composition.md) · **Next:** [ Atomics](13-atomics.md)
