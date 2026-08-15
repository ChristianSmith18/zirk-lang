# ADR-010 — Ubicaciones a través de varios archivos

- **Estado:** aceptada
- **Fecha:** 14 de agosto de 2026
- **Fase:** 2

## Contexto

Hasta la Fase 2 el compilador procesaba **un solo archivo**, y por eso `Span` es un par de offsets de bytes sin más: `{ start, end }`. Quien lo interpreta es el único `SourceFile` que existe, y `location(span)` y `snippet(span)` son métodos suyos.

Los módulos rompen esa suposición. Un `import` trae declaraciones de otro archivo, y un diagnóstico sobre ese otro archivo, renderizado con el `SourceFile` de entrada, señalaría un fragmento equivocado del texto equivocado — sin fallar, porque los offsets son válidos en cualquier cadena lo bastante larga.

`Span` no es una estructura interna cualquiera: viaja en cada nodo de la AST, en cada instrucción de la IR ([ADR-007](./ADR-007-forma-de-la-ir.md)) y, por la Fase 8, dentro del `.zpkg`. `ZIRK_COMPILER_SPEC.md` sección 11 exige del debugger un mapeo fiel al `.zrk`. Cambiar su forma después cuesta más que decidirla ahora.

## Opciones consideradas

### Un artefacto de source map aparte, al estilo de JavaScript

Un archivo separado que mapea salida a entrada, generado al final. Se descarta sin discusión larga: resuelve el problema contrario. Sirve para reconstruir ubicaciones **después** de compilar, y lo que hace falta es consultarlas **durante**. Como estructura interna sería una indirección que nadie quiere pagar en cada diagnóstico.

### Espacio de offsets global, al estilo de rustc

Todos los archivos ocupan un espacio de direcciones virtual único: el archivo 2 empieza donde termina el 1. `Span` no cambia, y para saber a qué archivo pertenece se hace búsqueda binaria sobre los offsets de inicio.

Es la opción **más barata hoy**: no toca el lexer, ni el parser, ni la AST, ni la IR. Cero cambios en cinco crates.

Se descarta por dos razones, ambas sobre hacia dónde va este proyecto y no sobre lo que necesita esta fase:

1. **Un archivo que cambia de tamaño corre la base de todos los siguientes**, y con ella invalida todos sus spans. Un LSP relexa un archivo en cada pulsación de tecla: editar el primero invalidaría los spans de todo el resto. El LSP es Fase 9 y la compilación incremental viene después; adoptar ahora una representación que las estorba es elegir el costo más caro de los dos.

2. **Un error de cálculo en la base no falla: miente.** Un span del archivo A interpretado contra el archivo B produce un diagnóstico plausible que señala el lugar equivocado. Un diagnóstico que apunta mal es peor que uno que no sale.

rustc adoptó este diseño antes de tener ambiciones de LSP, y lo compensa relativizando spans al serializar para el caché incremental. Es un parche a una decisión temprana, no un modelo a copiar.

## Decisión

**Un `Span` nombra su archivo. Los offsets siguen siendo locales a él.**

```rust
pub struct FileId(pub u32);

pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}
```

Un `SourceMap` posee los `SourceFile` del crate y responde `location(span)` y `snippet(span)` despachando por `span.file`. Las etapas reciben `&SourceMap` donde antes recibían `&SourceFile`; la forma de las llamadas no cambia.

`Span::to()` combina dos spans y **exige que sean del mismo archivo**: combinar ubicaciones de archivos distintos no significa nada, y que sea imposible por construcción es la mitad del valor de esta decisión.

### Sobre el tamaño

`Span` pasa de 8 a 12 bytes. En un crate grande —del orden de 100 000 nodos de AST— son unos 400 KB adicionales, y en la IR una cifra del mismo orden. Para un compilador eso no es una cantidad que justifique nada.

Se consideró empaquetar los tres campos en 64 bits (archivo de 16 bits, offsets de 24). Se descarta: el alineamiento se come buena parte del ahorro, los accesores dejan de ser campos, y la depuración empeora. Si alguna vez un perfil demuestra que importa, empaquetar es un cambio local a este tipo. Adivinarlo ahora no.

## Consecuencias

- **Los diagnósticos son correctos por construcción a través de archivos.** No hay forma de renderizar un span contra el archivo equivocado sin que el tipo lo delate.
- **Editar un archivo no invalida los spans de los demás**, que es la propiedad que el LSP de la Fase 9 y la compilación incremental necesitan.
- **La migración es mecánica pero transversal**: el lexer construye los spans y todo lo demás los propaga, así que el punto de creación es uno solo por archivo.
- `SourceFile` sigue existiendo con la misma responsabilidad —texto, líneas, fragmentos— y deja de ser el punto de entrada; lo es `SourceMap`.
- La forma serializada del `.zpkg` de la Fase 8 tendrá que decidir si el `FileId` se guarda tal cual o se reindexa por paquete. Queda anotado, no resuelto: depende del formato, que no existe.
