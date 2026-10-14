# Threads

> **Estado: diseño futuro; no está disponible en el lenguaje actualmente.** La
> tabla [Phase 5 — Concurrency and parallelism](../../../init/ZIRK_FEATURE_STATUS.md#phase-5-concurrency-and-parallelism)
> marca `thread` como pendiente junto con `Sync`. El runtime sí cuenta con un
> ejecutor cooperativo y un pool para `parallel`, pero ninguno implementa esta
> API de threads del sistema operativo.

El siguiente fragmento ilustra el diseño previsto; **no se garantiza que compile
con la implementación actual**:

```zirk
mut native_thread = thread "worker" { process(); };
native_thread.join();
```

En el diseño, un thread administrado no puede abandonarse al salir del ámbito y
la mutación compartida requiere sincronización. Esas garantías todavía no están
entregadas para una API `thread` de Zirk.

El diseño reserva los threads para afinidad nativa, aislamiento de llamadas
bloqueantes inevitables o requisitos especializados de stack/runtime. No
interpretes esto como soporte actual para `thread` ni para el puente explícito
de bloqueo, que también sigue pendiente. Para cómputo CPU-bound sí existe
`parallel`; su estado y límites están resumidos en la tabla enlazada arriba.

---

**Previous:** [← parallel for](09-parallel-for.md) · **Next:** [ Workers by Composition](11-workers-by-composition.md)
