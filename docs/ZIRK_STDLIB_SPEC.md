# Zirk — Especificación de la biblioteca estándar

## 1. Principios

La stdlib debe ser pequeña, coherente, tipada, multiplataforma y explícita en I/O, permisos y errores. No hay funciones globales de impresión o lectura. Los módulos estándar se importan sin comillas:

```text
import { stdout } from std.io;
import { File } from std.fs;
```

La primera versión incluye el núcleo necesario para aplicaciones nativas. Funcionalidad especializada puede distribuirse como paquetes oficiales sin ampliar el lenguaje.

## 2. Módulos iniciales

```text
std.io
std.fs
std.path
std.process
std.net
std.http
std.json
std.crypto
std.time
std.task
std.thread
std.sync
std.collections
std.testing
std.reflect
std.system
```

Las implementaciones pueden subdividirlos sin cambiar sus imports públicos.

## 3. `std.io`

Expone tres streams:

```text
import { stdin, stdout, stderr } from std.io;

stdout.print("Hola");
stdout.println("Mundo");
stdout.println();
stderr.println("Diagnóstico");
mut line = stdin.read_line();
```

- `print(value)` escribe sin salto.
- `println(value)` escribe con el salto de la plataforma.
- `println()` escribe solo un salto.
- `stdout` y `stderr` tienen las mismas operaciones de formato, pero destinos diferentes.
- todo valor imprimible usa `to_string(): String` o el trait de formato correspondiente.
- se favorece interpolación: `stdout.println("Usuario: {user.name}");`.

`stdin` ofrece lectura de línea, char y bytes. EOF no es una exception: se representa mediante `ReadResult<T>` o un resultado algebraico equivalente que distingue `Data`, `Eof` y `Error`. Estados inválidos del stream son errores tipados; fallos excepcionales del sistema pueden convertirse en exceptions según contrato.

Las variantes async suspenden una task sin bloquear un thread.

## 4. `std.fs`

```text
import { File } from std.fs;

mut content: String = match with File.open("data.txt") {
    Ok(file) { file.read_text() }
    Error(error) { return Error(error); }
};
```

`File` implementa `Resource<FileError>`. Operaciones mínimas:

- `open`, `create` y apertura con opciones tipadas;
- `read_text`, `read_bytes`, `read_line`;
- `write_text`, `write_bytes`, `append`;
- `flush`, `metadata` y cierre idempotente administrado;
- variantes async para operaciones que puedan esperar.

Los modos no usan strings mágicos: se expresan mediante opciones/enums. Los errores distinguen no encontrado, permiso denegado, ya existe, ruta inválida, tipo incorrecto, EOF cuando corresponda y fallo del sistema.

Filesystem requiere permisos de lectura/escritura con alcance declarable. El runtime valida rutas reales cuando sea necesario para evitar escapes mediante `..` o symlinks.

## 5. `std.path`

```text
import { Path } from std.path;

inmut FILE_PATH = Path("./data/file.txt");
FILE_PATH.parent();
FILE_PATH.name();
FILE_PATH.extension();
```

`Path` es una representación semántica de rutas, no un `String`. Permite `join`, normalización léxica, componentes, nombre, extensión, parent, absolute/canonical mediante operaciones que acceden al sistema y conversión explícita a string.

Debe preservar reglas de la plataforma y evitar concatenación textual insegura. Construir o manipular una ruta no accede al filesystem; canonicalizar sí puede hacerlo y requiere permiso.

## 6. `std.process`

```text
import { Process } from std.process;

mut result = await Process.run("git", ["status"]);
```

La API separa ejecutable y argumentos; no invoca un shell por defecto. Ofrece:

- `run` para esperar un resultado;
- `spawn` para obtener un recurso `ChildProcess`;
- stdin/stdout/stderr configurables;
- environment y working directory explícitos;
- exit code, signal y bytes/text capturados;
- timeout y cancelación cooperativa.

La ejecución por shell es una API diferente y visiblemente peligrosa. Requiere permiso de procesos; environment adicional requiere su capacidad correspondiente.

## 7. `std.collections`

Tipos mínimos:

- `Array<T>` dinámico de uso general;
- arrays fijos cuando el tamaño forma parte del tipo;
- `List<T>` cuando se necesite un contrato de lista explícito;
- `Map<K,V>` con keys hashable/equatable;
- `Set<T>`;
- `Range<T>`;
- iteradores y vistas.

Las colecciones ofrecen `map`, `filter`, `reduce`, búsqueda, ordenamiento y conversión explícita. Las operaciones funcionales no mutan el origen. El acceso por índice comprueba límites; se ofrecen accesos seguros que retornan un tipo opcional.

## 8. `std.time`

Incluye `Duration`, instantes monotónicos, fecha/hora civil, zonas horarias mediante datos versionados, timers y sleep cancelable.

```text
await task.sleep(500ms);
await operation timeout 5s;
```

Las mediciones de elapsed usan reloj monotónico. Fecha civil y duration son tipos distintos; no se mezclan implícitamente.

## 9. `std.task`, `std.thread` y `std.sync`

Estos módulos exponen tipos de soporte para las construcciones del lenguaje:

- handles y scopes de `Task<T>`;
- cancelación y razones de cancelación;
- `Channel<T>` acotado/no acotado y cierre;
- `Thread<T>` y `join`;
- `Mutex<T>`, locks de lectura/escritura, semáforos y barreras cuando se justifiquen;
- `Atomic<T>` y órdenes de memoria;
- primitivas de reducción paralela.

La sintaxis `task`, `await`, `parallel` y `thread` pertenece al lenguaje; la stdlib no crea un modelo alternativo.

## 10. `std.net` y `std.http`

`std.net` proporciona direcciones, DNS, TCP y UDP mediante APIs tipadas, cancelables y compatibles con el reactor. Toda conexión respeta `permissions.network`.

`std.http` incluye inicialmente cliente y servidor nativos básicos, con:

- request/response tipados;
- headers con validación;
- streaming y backpressure;
- timeouts y límites configurables;
- TLS mediante implementación auditada;
- cancelación asociada a la desconexión;
- handlers ejecutados como tasks estructuradas.

No se crea un thread por request. El reactor maneja I/O y el scheduler ejecuta handlers; trabajo CPU pesado debe pasar a `parallel`.

Frameworks, routing avanzado, ORM y plantillas quedan en paquetes, no en el núcleo.

## 11. `std.json`

Ofrece un árbol JSON tipado y encode/decode genérico. La derivación de serializers puede hacerse mediante decoradores o reflection solicitada. Los errores incluyen ubicación, path y expectativa de tipo. Límites de profundidad/tamaño deben prevenir consumo hostil.

## 12. `std.crypto`

Solo algoritmos modernos y auditados, con defaults seguros, comparación constante donde corresponda, CSPRNG del sistema y tipos que dificulten mezclar claves, nonces y hashes. Algoritmos obsoletos no se habilitan por comodidad. Las APIs pueden estar respaldadas por librerías nativas verificadas.

## 13. `std.reflect`

Expone identidad básica de tipos siempre disponible y metadata estructural únicamente cuando fue preservada. No permite romper visibilidad ni mutabilidad. Reflection dinámica que requiera metadata ausente produce un resultado/diagnóstico explícito.

La Syntax API del compilador no pertenece a `std.reflect`; es una API de tooling separada.

## 14. `std.testing`

Los tests unitarios usan `@test` exclusivamente en `.spec.zrk`:

```text
// user_service.spec.zrk
@test
fn creates_user(): Void {
    assert.equal(actual, expected);
}
```

Usar `@test` fuera de `.spec.zrk` es error de compilación. Los tests no entran en binarios release ni reciben acceso privado automático.

Los E2E viven en `test/*.e2e.zrk` y usan `@e2e`. Comandos:

```text
zirk test
zirk test --unit
zirk test --e2e
zirk test --all
zirk test --file <path>
zirk test --tag <tag>
zirk test --seed <seed>
zirk test --jobs <n>
zirk test --report json
```

Assertions mínimas: igualdad, identidad, truth, nullability, resultado, exception, colección y aproximación decimal. Cada failure muestra valores, diff y source location.

`@bench` define benchmarks ejecutados por `zirk bench`, con warmup, múltiples muestras, estadísticas, prevención de optimización muerta y salida machine-readable.

## 15. `std.system`

Expone información portable del proceso, target y señales sin convertir detalles internos del runtime en API estable. `exit(code)` es inmediato y debe reservarse para fronteras; el retorno normal desde `main` permite cierre ordenado.

Variables de entorno, señales y datos sensibles requieren permisos según su capacidad.

## 16. Contratos comunes

Las APIs públicas de stdlib deben:

- preferir enums/options a strings mágicos;
- usar `Result` para fallos operacionales esperables;
- reservar exceptions para fallos excepcionales recuperables;
- ser cancelables cuando puedan esperar;
- documentar thread-safety, blocking, allocations y permisos;
- aceptar `Path` en APIs de filesystem;
- evitar copias mediante buffers/vistas seguras cuando sea posible;
- ofrecer límites contra inputs hostiles;
- mantener comportamiento equivalente entre targets soportados o declarar diferencias explícitas.

## 17. Exclusiones

No forman parte inicial de la stdlib: DOM/browser, UI framework, ORM, framework web completo, package registry client como API pública, shell implícito, algoritmos criptográficos obsoletos y un event loop controlado por el usuario.
