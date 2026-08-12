# Zirk — Especificación del runtime

## 1. Principios

El runtime proporciona memoria automática, concurrencia estructurada, paralelismo, I/O no bloqueante, timers, cancelación, recursos y diagnósticos de fallo. Debe ser pequeño, portable y enlazarse en binarios standalone. Los subsistemas se inicializan de manera diferida cuando sea posible.

Zirk no expone un event loop global. El runtime puede usar internamente uno o más reactors de eventos.

## 2. Ciclo de vida de una aplicación

Orden normativo:

```text
validar init.zrk y permisos
    ↓
cargar runtime mínimo
    ↓
inicializar globals en orden determinista
    ↓
invocar main()
    ↓
ejecutar scopes de concurrencia estructurada
    ↓
cierre ordenado de recursos y threads gestionados
    ↓
flush de streams y terminación con exit code
```

`main` es el único símbolo ejecutable del archivo `project.entry`:

```text
fn main(): Void { ... }
```

También podrá retornar un código o un `Result` definido por el contrato de entrypoint. Un fallo durante globals impide ejecutar `main` y produce un diagnóstico determinista.

La finalización normal ocurre cuando `main` termina y todos sus scopes raíz han concluido. No se espera silenciosamente trabajo huérfano.

## 3. Scheduler y reactor de I/O

El scheduler ejecuta tasks sobre un pool multinúcleo dimensionado según hardware y configuración segura. Puede usar colas locales, work stealing, prioridades internas y afinidad, pero no garantiza que una task permanezca en un thread.

El reactor integra I/O, timers y señales usando mecanismos nativos como IOCP, `epoll` o `kqueue`. Una task que espera I/O se suspende sin reservar innecesariamente un thread; al completarse la operación vuelve a la cola ejecutable.

El scheduler debe evitar starvation, limitar crecimiento de colas y aplicar backpressure donde el contrato lo permita.

## 4. Tasks y await

`task` crea trabajo concurrente administrado:

```text
mut operation = task {
    load_data();
};

mut result = await operation;
```

`await` suspende la task actual, no un thread del sistema operativo. No existe `async fn`: el tipo de una función describe su valor lógico y `task` hace explícita la ejecución concurrente.

Las tasks están tipadas, propagan resultado/error y pertenecen a un scope. Al salir del scope, sus hijas deben haber terminado o recibir cancelación cooperativa y ser esperadas. Ninguna task puede quedar huérfana implícitamente.

Una futura operación detached, si se incorpora, deberá ser explícita y transferir propiedad a un supervisor raíz; no forma parte del contrato inicial.

## 5. Cancelación y timeout

La cancelación es cooperativa, idempotente y observable en puntos seguros: `await`, I/O, channels, timers y comprobaciones de loops prolongados. `task.cancel()` solicita cancelación; no destruye ejecución en una instrucción arbitraria.

La cancelación se propaga de padre a hijos. La task ejecuta limpieza de recursos y termina con una exception recuperable tipada de cancelación, salvo que su API la convierta explícitamente en `Result`.

```text
await load_data() timeout 5s;
```

Un timeout solicita cancelación y produce una exception recuperable de timeout. Las duraciones admiten `ms`, `s`, `m` y `h` y se representan internamente con precisión suficiente, aunque puedan normalizarse.

## 6. Parallel

`parallel` expresa trabajo CPU potencialmente simultáneo:

```text
parallel {
    process_a();
    process_b();
}
```

`parallel for` distribuye iteraciones independientes en el pool:

```text
mut squares = parallel for value in 0..1000 {
    value * value
};
```

El runtime decide partición y cantidad de workers internos. Los resultados conservan un orden definido por el contrato de la operación, no por el orden físico de finalización. Si una iteración retorna `Result`, se conserva el primer error relevante, se cancela cooperativamente el trabajo restante y se espera su cierre.

Capturas mutables inseguras son error de compilación. Reducciones deben usar primitivas explícitas o acumuladores seguros.

## 7. Threads

`thread` crea un thread real del OS:

```text
mut native_thread = thread "worker" {
    process();
};

native_thread.join();
```

Un thread tiene nombre opcional, resultado, `join` y solicitud cooperativa de cancelación cuando aplique. No puede abandonarse implícitamente al finalizar su scope. Compartir memoria mutable requiere `sync`, mutex o atomics.

No existe `worker` como entidad del lenguaje. Un worker aislado o dedicado se construye con un thread/task supervisado y uno o más channels.

## 8. Channels, sincronización y atomics

`Channel<T>` es una cola tipada y thread-safe entre tasks, parallel y threads:

```text
mut messages: Channel<String> = Channel();
messages.send("ok");
mut message = await messages.receive();
```

`receive` suspende eficientemente en una task. `try_receive` devuelve `Option<T>` o el tipo opcional definido por stdlib. Los channels deben soportar cierre explícito, distinguir cierre de ausencia temporal y aplicar backpressure en canales acotados.

`sync` delimita acceso protegido. Los mutexes no deben mantenerse a través de `await` salvo un tipo diseñado expresamente para ello; el compilador/linter lo diagnostica.

`Atomic<T>` existe solo para tipos y operaciones soportadas. Expone load/store, exchange, compare-exchange y operaciones numéricas como increment. El orden de memoria predeterminado debe ser seguro; órdenes más débiles son explícitos y avanzados.

## 9. Memoria

La administración es automática. Stack/heap, escape analysis, regiones, movimientos, RC o GC son detalles internos combinables. La representación nunca altera igualdad, identidad ni vida observable.

Requisitos:

- no use-after-free en código seguro;
- no double-free;
- ciclos y concurrencia deben liberarse correctamente;
- pausas y consumo deben medirse;
- los tipos de valor pueden almacenarse inline;
- objetos con identidad mantienen identidad estable aunque se muevan físicamente.

No hay destructores de propósito general cuyo momento sea observable. La liberación de memoria no se usa para administrar archivos, sockets, locks o procesos.

## 10. Recursos

Los recursos externos implementan `Resource<E>` y se gestionan con `match with`. El cierre ocurre exactamente una vez al abandonar el bloque por éxito, error, exception, retorno o cancelación. Los errores de apertura se manejan antes de adquirir el recurso; los de cierre siguen el contrato tipado del recurso y no deben ocultar silenciosamente un error principal.

No existe `defer` en 1.x. El compilador impide que un recurso administrado o una referencia dependiente escape del scope.

## 11. Señales y cierre ordenado

La stdlib traduce señales soportadas (`SIGINT`, `SIGTERM` o equivalentes) a eventos de shutdown. El supervisor raíz:

1. marca estado de cierre;
2. solicita cancelación de scopes raíz;
3. despierta esperas cancelables;
4. cierra recursos en orden inverso de adquisición;
5. une threads gestionados;
6. vacía `stdout`/`stderr` dentro de un límite;
7. termina con exit code.

Una segunda señal o un límite agotado puede forzar salida controlada. `fatalError` intenta emitir diagnóstico y realizar solo la limpieza que sea segura; no promete continuar ejecución normal.

## 12. Seguridad y permisos

El runtime aplica permisos declarados para filesystem, red, procesos, entorno y otras capacidades. Una librería declara requisitos; la aplicación concede el conjunto final. La ausencia de permiso produce un error claro, no una concesión automática.

`unsafe` no omite permisos ni validaciones del sistema operativo.

## 13. Observabilidad

Stack traces incluyen funciones, tasks, threads, awaits y expansiones de decoradores cuando hay metadata. El debugger puede enumerar tasks pendientes, timers, channels y threads. Las métricas internas no forman parte de la semántica salvo APIs explícitas.
