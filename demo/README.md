# Demo del lenguaje Zirk

Este directorio reúne ejemplos numerados del lenguaje y de las funciones que están implementadas en este checkout. Los `.zrk` se derivan de casos válidos del corpus del compilador para mantener la sintaxis alineada con la implementación. Los ejemplos avanzados deben leerse junto con el estado vigente en [`docs/init/ZIRK_FEATURE_STATUS.md`](../docs/init/ZIRK_FEATURE_STATUS.md).

## Ejecutar

Desde la raíz del repositorio, con el compilador ya construido:

```sh
./target/debug/zirk check demo/01-tipos-y-conversiones.zrk
./target/debug/zirk run demo/01-tipos-y-conversiones.zrk
./target/debug/zirk run demo/29-proyecto-y-modulos/src/main.zrk
```

`check` valida el frontend; `run` compila y ejecuta en el host actual. Algunas muestras usan concurrencia nativa y dependen de las capacidades de la plataforma descritas en el catálogo.

## Índice de capacidades

| Archivo | Capacidad |
|---|---|
| [`01-tipos-y-conversiones.zrk`](01-tipos-y-conversiones.zrk) | Familias enteras con signo y sin signo, conversiones explícitas y operaciones bit a bit. |
| [`02-nullabilidad-y-narrowing.zrk`](02-nullabilidad-y-narrowing.zrk) | Tipos opcionales, null y refinamiento de tipo dentro de match. |
| [`03-control-de-flujo.zrk`](03-control-de-flujo.zrk) | Condiciones, ciclos y salida o continuación de iteraciones. |
| [`04-rangos-y-slices.zrk`](04-rangos-y-slices.zrk) | Rangos abiertos/cerrados, pasos y recorrido de rangos. |
| [`05-funciones-y-closures.zrk`](05-funciones-y-closures.zrk) | Funciones como valores, tipos Fn, lambdas, capturas y recursión. |
| [`06-arrays-y-listas.zrk`](06-arrays-y-listas.zrk) | Secuencias de longitud fija, listas mutables, indexación e iteración. |
| [`07-mapas.zrk`](07-mapas.zrk) | Colecciones clave-valor con claves admitidas por el runtime. |
| [`08-conjuntos.zrk`](08-conjuntos.zrk) | Colecciones de elementos únicos y operaciones de pertenencia. |
| [`09-clases-y-objetos.zrk`](09-clases-y-objetos.zrk) | Constructores, campos, visibilidad, métodos e identidad de objetos. |
| [`10-enums-algebraicos.zrk`](10-enums-algebraicos.zrk) | Casos con y sin payload, patrones exhaustivos y destructuración. |
| [`11-tuplas.zrk`](11-tuplas.zrk) | Valores producto heterogéneos y acceso/destructuración. |
| [`12-genericos-y-contratos.zrk`](12-genericos-y-contratos.zrk) | Interfaces genéricas, implementaciones, despacho y protocolos de iteración. |
| [`13-resultados.zrk`](13-resultados.zrk) | Result<T,E>, construcción, consumo y match exhaustivo. |
| [`14-excepciones.zrk`](14-excepciones.zrk) | throws, throw, try/catch/finally y jerarquía Throwable. |
| [`15-recursos.zrk`](15-recursos.zrk) | Resource<E> y match with con cierre automático en las salidas. |
| [`16-clonado-profundo.zrk`](16-clonado-profundo.zrk) | clone de records y objetos administrados con preservación de valores. |
| [`17-punteros-unsafe.zrk`](17-punteros-unsafe.zrk) | Acceso explícito a Pointer dentro de un bloque unsafe. |
| [`18-regex.zrk`](18-regex.zrk) | Literales de expresiones regulares, búsqueda, grupos y reemplazo. |
| [`19-tiempo-civil.zrk`](19-tiempo-civil.zrk) | Date, Time, DateTime y validación de componentes temporales. |
| [`20-duraciones.zrk`](20-duraciones.zrk) | Literales con unidades y aritmética de Duration. |
| [`21-canales-tipados.zrk`](21-canales-tipados.zrk) | Channel<T>, envío/recepción tipados, capacidad y cierre. |
| [`22-concurrencia-estructurada.zrk`](22-concurrencia-estructurada.zrk) | Bloques concurrent, spawn y espera estructurada al salir del ámbito. |
| [`23-concurrent-each.zrk`](23-concurrent-each.zrk) | Mapeo concurrente acotado, límite explícito y orden de resultados. |
| [`24-concurrent-first.zrk`](24-concurrent-first.zrk) | Selección por orden de finalización de Concurrent.of(...).first(). |
| [`25-thread-run.zrk`](25-thread-run.zrk) | Trabajo nativo administrado con resultado tipado y raíces GC. |
| [`26-mutex-con-vista.zrk`](26-mutex-con-vista.zrk) | Mutex<T>.with y vista prestada protegida durante el callback. |
| [`27-atomics.zrk`](27-atomics.zrk) | Atomic<T>, operaciones RMW y órdenes de memoria. |
| [`28-parallel.zrk`](28-parallel.zrk) | Región CPU paralela con división de trabajo sobre una colección. |
| [`29-proyecto-y-modulos/`](29-proyecto-y-modulos/) | Proyecto `.zkinit`, entrypoint, `import` local y `share`. |

## Cobertura y límites actuales

Los ejemplos cubren bindings y tipos escalares, nulabilidad, control de flujo, rangos, funciones y closures, colecciones, clases/records/enums/tuplas, genéricos y contratos, `Result`, excepciones, recursos, clone, punteros `unsafe`, regex, tiempo civil, canales y concurrencia/parallel nativos. El catálogo de estado es la fuente para conocer las restricciones exactas por plataforma y backend.

No presento como implementadas las construcciones que el catálogo marca como pendientes o excluidas: entre ellas `async fn` y `await`, `task`/`select`, generadores `fn gen`, `|>`, `map`/`filter`/`reduce` funcionales, decoradores `fn dec`, `Thread<T>`, `RwLock`, `Semaphore`, `Barrier`, `Once<T>`, atomics de punteros y adquisición/publicación de paquetes externos.
