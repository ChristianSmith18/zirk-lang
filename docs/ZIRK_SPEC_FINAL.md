# Zirk — Especificación maestra

Estado: diseño normativo inicial  
Fecha: 12 de agosto de 2026  
Extensión fuente: `.zrk`  
CLI: `zirk`  
Manifiesto: `init.zrk`  
Lockfile: `zirk.lock`  
Paquete distribuible: `.zpkg`

## 1. Identidad

Zirk es un lenguaje compilado de propósito general, orientado a objetos, estáticamente tipado con inferencia, de alto nivel por defecto y con acceso opcional a bajo nivel. Compila a binarios nativos e incorpora concurrencia explícita y paralelismo multinúcleo como características de primera clase.

Su filosofía es:

> Fácil por defecto, explícito cuando necesitas control.

El nombre nace de transformar el nombre *Crist*: al invertir su sonido se obtiene *Sirc* y después se modifican sus letras hasta obtener **Zirk**, conservando esa sonoridad con una identidad propia.

## 2. Alcance inicial

Zirk 1.x estará orientado a aplicaciones backend, CLI, desktop, sistemas y librerías nativas. Los binarios serán standalone y no necesitarán Node.js, Python, Java ni otra instalación de lenguaje.

Targets iniciales, cuando la combinación sea soportada por LLVM, el linker y las dependencias:

- Windows, Linux y macOS.
- `x86`, `x86_64`, `armv7` y `aarch64`.
- Mach-O, ELF y PE.
- compilación cruzada mediante `zirk build --target <target>`.

`build_targets` en `init.zrk` permite producir varios targets. Un `--target` explícito tiene prioridad. Sin ambos, se detecta el host.

## 3. Fuera del alcance inicial

No forman parte de Zirk 1.x:

- target WebAssembly o integración nativa con navegador/DOM;
- directivas `@runtime`, `@target`, `@host` o `@platform`;
- un `worker` como primitiva independiente: se compone con `task`, `thread` y `Channel<T>`;
- `async fn`: `task` y `await` expresan la asincronía;
- un event loop público o administrado manualmente;
- assembly textual inline;
- `comptime {}` general;
- `defer` general;
- herencia múltiple de clases;
- sobrecarga tradicional de funciones;
- operador de propagación `?` para `Result`;
- ownership o reference counting como semántica pública;
- backend propio o varios backends iniciales.

Estas exclusiones no deben reinterpretarse como huecos que una implementación pueda completar libremente.

## 4. Documentos normativos

Esta especificación se divide en:

- [ZIRK_LANGUAGE_SPEC.md](./ZIRK_LANGUAGE_SPEC.md): sintaxis, tipos, objetos, control de flujo, errores, módulos, metaprogramación y seguridad.
- [ZIRK_COMPILER_SPEC.md](./ZIRK_COMPILER_SPEC.md): frontend, Syntax API, IR, LLVM, targets, diagnósticos y tooling.
- [ZIRK_RUNTIME_SPEC.md](./ZIRK_RUNTIME_SPEC.md): memoria, ejecución, tasks, scheduler, I/O, threads, recursos, cancelación y cierre.
- [ZIRK_STDLIB_SPEC.md](./ZIRK_STDLIB_SPEC.md): módulos y contratos mínimos de la biblioteca estándar.

Si dos documentos se contradicen, esta especificación maestra define el alcance y las exclusiones; el documento especializado define la semántica de su área. Toda ambigüedad restante debe producir un diagnóstico o quedar documentada antes de implementarse, nunca resolverse silenciosamente.

## 5. Proyecto mínimo

```text
my_app/
├── init.zrk
├── zirk.lock
├── src/
│   └── main.zrk
└── test/
```

```text
project {
    name: "my-app";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}
```

```text
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Hola desde Zirk");
}
```

## 6. Contratos fundacionales

- Todo valor pertenece semánticamente a una clase; los valores simples pueden representarse inline.
- `mut` permite reasignación; `inmut` impide reasignar la referencia; `inmut::strict` exige inmutabilidad profunda.
- `null` solo habita tipos `T?`; no existe `undefined`.
- `==` compara estructuralmente y `is` comprueba identidad en tipos de referencia.
- `Result<T, E>` representa fallos esperables; exceptions representan situaciones excepcionales recuperables; `fatalError` termina ante estados irreparables.
- La memoria es automática. Stack, heap, escape analysis, movimientos y RC son decisiones internas.
- El código seguro no admite use-after-free, null dereference, data races ni comportamiento indefinido.
- Los punteros y casts inseguros requieren `unsafe {}`.
- Las tasks usan concurrencia estructurada; `parallel` solicita trabajo CPU multinúcleo; `thread` representa un thread real del sistema operativo.
- El runtime puede usar un reactor de eventos internamente, pero Zirk no expone un event loop global.
- `share` publica declaraciones, `import` las incorpora y `use` habilita globals de `init.zrk`.

## 7. Distribución y seguridad

Una `application` puede declarar globals y concede los permisos finales. Una `library` no puede declarar globals; declara `requires` y `compile_permissions`. Los paquetes incluyen API pública tipada, representación intermedia portable, manifiesto, documentación y licencia. En el build final, todas las piezas se compilan para el mismo target.

Los permisos son capacidades finitas declaradas en `init.zrk`. `zirk prepare` audita y puede proponer cambios; `zirk build` es estricto y no concede permisos de forma interactiva. Tokens y secretos nunca se almacenan en `init.zrk` ni en `zirk.lock`.

## 8. Criterio de completitud

Una implementación compatible debe acompañar cada característica con:

1. gramática;
2. reglas de tipos;
3. semántica observable;
4. diagnósticos de compilación;
5. errores de runtime;
6. ejemplos válidos e inválidos;
7. interacción con mutabilidad, concurrencia y targets;
8. pruebas de conformidad.

El checkpoint histórico de diseño no es normativo cuando contiene preguntas, alternativas o texto marcado como pendiente.
