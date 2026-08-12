# Zirk — Especificación del lenguaje

Este documento define la semántica pública del código fuente `.zrk`.

## 1. Léxico y estructura

Zirk distingue mayúsculas de minúsculas. Los bloques usan `{}`. El punto y coma es opcional para el parser cuando no hay ambigüedad, pero el formatter oficial lo agrega. Se admiten `//` y `/* ... */`; la documentación usa comentarios multilínea de documentación.

Convenciones:

- variables, funciones, métodos, parámetros y archivos: `snake_case`;
- clases, interfaces, traits, records y enums: `UpperCamelCase`;
- constantes y globals: `UPPER_SNAKE_CASE`;
- parámetros nombrados: `name: value`;
- aliases en imports y destructuración: `Original -> Alias`.

## 2. Variables, scopes y globals

```text
mut count: Int32 = 0;
inmut NAME: String = "Zirk";
inmut::strict CONFIG: Config = Config();
```

`mut` permite reasignar. `inmut` inmoviliza la referencia y respeta la mutabilidad del tipo referido. `inmut::strict` impide modificaciones transitivas. Las mayúsculas son una convención, no semántica codificada en el nombre.

La inferencia se permite cuando es inequívoca. Cada tipo posee un valor predeterminado; el análisis de flujo impide leer una variable que aún no esté disponible.

Existen scopes de bloque, función, archivo y módulo. Un símbolo de archivo no sale de él salvo que se publique con `share`.

Solo una `application` puede declarar globals, exclusivamente en `globals` de `init.zrk`:

```text
globals {
    inmut APP_NAME: String = "App";
    mut REQUEST_COUNT: Atomic<UInt64> = Atomic(0);
}
```

Cada consumidor usa `use APP_NAME;`. Una global mutable accedida desde `parallel` o `thread` debe protegerse con `sync` o `Atomic<T>`; en caso contrario hay error de compilación.

## 3. Sistema de tipos

El tipado es estático con inferencia. Todo valor pertenece a una clase semántica, aunque el compilador puede representarlo sin allocation.

Familias fundamentales:

- `Int8`, `Int16`, `Int32`, `Int64`, `Int128` y aliases `Int`/`Integer` para el entero firmado predeterminado.
- `UInt8`, `UInt16`, `UInt32`, `UInt64`, `UInt128`.
- `Decimal16`, `Decimal32`, `Decimal64`, `Decimal128` y aliases `Dec`/`Decimal` para el decimal predeterminado.
- `Boolean`, exclusivamente `true` o `false`.
- `Char`, un code point Unicode.
- `String`, secuencia Unicode indexada semánticamente por graphemes y con índice/cache interno adaptativo.
- `Void`, `Never`, `Null`, `Object` y tipos de colección.

No hay truthiness numérico. `Boolean?` admite `null`; `Boolean` no. No existe `undefined`.

Los literales admiten notación científica (`1e2`) y `_` como separador (`1_000_000`). Las duraciones son literales tipados como `5000ms`, `5s`, `5m` y `5h`; el runtime puede normalizarlas internamente.

El overflow ordinario produce un error controlado; las variantes wrapping, saturating o checked deben ser operaciones explícitas. Las conversiones que puedan perder información no son implícitas.

## 4. Nullability, igualdad y operadores

`T?` equivale a `T | Null`. El acceso seguro usa `?.` y el valor alternativo usa `??`.

- `==` y `!=`: igualdad estructural.
- `is`: misma instancia, solo para tipos con identidad observable.
- Comparadores: `<`, `<=`, `>`, `>=` según contratos del tipo.
- Lógicos: `&&`, `||`, `!`, solo con booleanos.
- Aritméticos y compuestos: `+`, `-`, `*`, `/`, `%`, `+=`, `-=`, `*=`, `/=`, `%=`.
- Incremento: `count++`, `count--`, `++count`, `--count`, conservando semántica postfix/prefix convencional.
- Ternario: `condition ? when_true : when_false`.

Los operadores solo pueden sobrecargarse mediante contratos definidos por el lenguaje; una sobrecarga no puede alterar precedencia ni aridad.

## 5. Control de flujo y pattern matching

Se incluyen `if`/`else`, `for`, `for ... in`, `while`, `loop`, `break` y `continue`. `if` puede ser expresión cuando todas las ramas producen tipos compatibles.

`match` es exhaustivo cuando se usa como expresión:

```text
mut message: String = match result {
    Ok(value) { "Valor: {value}" }
    Error(error) { "Error: {error}" }
};
```

Como sentencia, controla flujo y no produce valor. Admite valores, tipos, enums asociados, unions y desestructuración. No existen `capture` ni `yield` especiales para recuperar su resultado.

`match with` adquiere un `Resource<E>` y garantiza su cierre al terminar cualquier rama, incluido error, exception, `return` o cancelación:

```text
mut first_line: String = match with File.open("data.txt") {
    Ok(file) { file.read_line() }
    Error(error) { "" }
};
```

El recurso se cierra antes de entregar el valor y no puede escapar directa ni indirectamente.

## 6. Funciones y closures

```text
fn add(a: Int32, b: Int32): Int32 {
    return a + b;
}
```

Se admiten inferencia local, parámetros opcionales (`name?`), tipos nullable (`String?`), valores predeterminados, parámetros nombrados y variádicos (`...values`). No existe sobrecarga tradicional; se usan unions, genéricos o nombres diferentes.

Las lambdas son equivalentes a valores función:

```text
inmut ADD = (a: Int32, b: Int32): Int32 => a + b;
inmut ACTION = (): Void => {
    stdout.println("ok");
};
```

Una closure captura valores inmutables con seguridad. La captura mutable compartida requiere que el análisis de concurrencia demuestre seguridad o que se use sincronización explícita.

## 7. Objetos y tipos de datos

```text
class User implements Serializable {
    public inmut id: UInt64;
    public mut name: String;

    construct(id: UInt64, name: String) {
        this.id = id;
        this.name = name;
    }
}

mut user = User(1, "Cristian");
```

El constructor se llama `construct`; no existe `new`; la instancia actual es `this`. La visibilidad es `public`, `private` o `protected`, con `public` por defecto.

Una clase puede extender una clase y combinar múltiples interfaces y traits. Las clases son heredables por defecto; existen clases y métodos `abstract`, pero no `final`. Los traits pueden incluir implementación reutilizable.

Los genéricos usan `<T>` y restricciones con `from`:

```text
fn serialize<T from Serializable>(value: T): String { ... }
```

Se especializan por tipos concretos cuando sea apropiado.

Tipos de datos adicionales:

- enums tradicionales y enums algebraicos con valores asociados;
- aliases mediante `type`;
- unions `A | B`;
- records inmutables con semántica estructural;
- value classes sin identidad observable, almacenables inline;
- arrays dinámicos, arrays fijos, `List<T>`, `Map<K,V>` y `Set<T>`.

Una clase normal tiene identidad y estado; un record representa datos; una value class representa un valor compacto. `clone()` solo existe mediante un trait explícito y puede derivarse cuando todos los campos son clonables.

## 8. Iteración y estilo funcional

`Iterable<T>` y `Iterator<T>` definen la iteración. Los generators usan `fn gen` y producen valores de forma suspendible. Las colecciones ofrecen `map`, `filter` y `reduce` sin mutar el origen. El pipe `|>` pasa el resultado izquierdo a la siguiente operación.

`Range<T>` es iterable e independiente del slicing. Se admiten rangos inclusivos/exclusivos definidos por su constructor y slicing `[inicio:fin:paso]`.

## 9. Errores

`Result<T,E>` es el mecanismo principal para fallos esperables. Se maneja explícitamente con `match`; no existe `?`.

Las exceptions son excepcionales pero recuperables:

```text
try {
    execute();
} catch<HttpError> error {
    stderr.println(error);
} default error {
    stderr.println(error);
} finally {
}
```

`fatalError(message)` representa un estado irreparable y termina el proceso después del diagnóstico y cierre seguro posible. Un error de índice, división por cero, null o estado inválido nunca se convierte en comportamiento indefinido.

## 10. Módulos, proyecto y paquetes

```text
share class User {}
import { User, Role -> DomainRole } from "./domain/user";
import { stdin, stdout, stderr } from std.io;
```

Las rutas locales usan comillas y omiten `.zrk`. Los módulos estándar usan nombres sin comillas. `share` publica código; `import` incorpora código; `use` solo habilita globals.

`init.zrk` es una DSL declarativa, no código ejecutable. Contiene `project`, `build_targets`, `globals`, `permissions`, `compile_permissions`, `requires` y dependencias según el tipo de proyecto. No contiene imports globales ni configuración arbitraria del compilador/runtime.

## 11. Conversión y casts

Las conversiones seguras usan constructores o métodos tipados que pueden retornar `Result`. Los casts explícitos admiten forma postfix y prefix:

```text
mut value = source as String;
mut value = <String>source;
mut field = <CustomObject>(obj.field).field;
```

Los casts comprobables fallan de forma controlada. Los casts que reinterpretan memoria o eliminan garantías requieren `unsafe {}`.

## 12. Decoradores y reflection

Un decorador nativo se declara con `fn dec`. Sus parámetros externos configuran el decorador y los bloques internos determinan los targets admitidos:

```text
fn dec route(path: String) {
    class(target) {
        // lógica para clases
    }

    method(target) {
        // lógica para métodos
    }
}
```

Un mismo decorador puede implementar varios targets. Cada bloque recibe una API contextual fija y tipada. Los decoradores se ejecutan en compilación, pueden leer metadata y realizar transformaciones mediante una Syntax API controlada; no reciben acceso arbitrario a la AST interna ni al sistema.

La identidad básica de tipo existe siempre. Reflection estructural avanzada solo se conserva cuando un tipo o decorador la solicita. La compile-time reflection general se realiza dentro de decoradores; no existe `comptime {}` general en 1.x.

## 13. Seguridad y bajo nivel

El código seguro garantiza ausencia de use-after-free, null dereference no controlado, data races y undefined behavior. Los índices se verifican salvo optimización demostrablemente segura.

```text
unsafe {
    mut pointer: Pointer<Int32> = &value;
    *pointer = 20;
}
```

`unsafe` habilita operaciones concretas, no desactiva el type checker, scopes, mutabilidad ni permisos. Las referencias seguras no son null y respetan vidas útiles verificadas. La interoperabilidad nativa usa ABI C como frontera estable; C++ y Rust exponen wrappers `extern "C"`.

## 14. Sintaxis reservada a concurrencia

`task`, `await`, `parallel`, `parallel for`, `thread`, `Channel<T>`, `sync` y `Atomic<T>` se definen normativamente en la especificación del runtime. No existe `async fn` ni `worker` independiente.
