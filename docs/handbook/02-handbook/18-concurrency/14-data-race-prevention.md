# Data-Race Prevention

> **Estado: soporte parcial; no existe una garantía general de prevención de
> data races en código Zirk.** `Atomic<T>` está disponible para escalares
> compatibles y protege sus operaciones individuales; no reemplaza locks ni
> demuestra por sí solo que un programa completo esté libre de carreras. La tabla
> [Phase 5 — Concurrency and parallelism](../../../init/ZIRK_FEATURE_STATUS.md#phase-5-concurrency-and-parallelism)
> confirma `parallel` y `concurrent` como entregados, pero mantiene `thread`/
> `Thread.run`, Mutex, RwLock, Semaphore, Barrier y Once siguen pendientes.

En particular, `parallel` rechaza capturas de referencias mutables en su
frontera; esa regla no equivale a un análisis global de carreras. El compilador
también deriva capacidades internas `Transfer` y `Share` en los límites que
implementa, pero eso no habilita acceso compartido sincronizado mediante APIs
`thread` o `sync`, que aún no están disponibles.

El modelo previsto combina `inmut::strict`, transferencia exclusiva inferida,
`clone()` independiente, canales y locks explícitos como estrategias distintas.
Los locks pertenecen al diseño futuro. Las garantías efectivas dependen de cada
frontera ya implementada; no infieras que compilar hoy prueba la ausencia de
cualquier carrera en todos los escenarios concurrentes. El orden de finalización
de trabajo concurrente puede seguir siendo no determinista.

---

**Previous:** [← Atomics](13-atomics.md) · **Next:** [ Transfer, Sharing, and Captures](17-transfer-sharing-and-captures.md)
