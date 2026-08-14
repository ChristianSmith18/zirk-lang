# ADR-008 — Color en los diagnósticos

- **Estado:** aceptada
- **Fecha:** 14 de agosto de 2026
- **Fase:** 1

## Contexto

`ZIRK_COMPILER_SPEC.md` sección 8 define el **formato** de un diagnóstico —severidad, código, ubicación, causa, ayuda— pero no dice nada sobre su presentación. El color no cambia el formato: cambia cuánto cuesta leerlo.

La sección 9 del mismo documento sí impone una restricción que condiciona cómo se implementa:

> The CLI must start fast, produce **deterministic output** and offer a structured mode (`--json`) for tooling.

Un diagnóstico coloreado que llega a una tubería no es determinista: la herramienta que lo consume recibe secuencias de escape que no pidió.

## Decisión

**Los diagnósticos se colorean solo cuando el destino es una terminal que una persona está leyendo.**

La biblioteca nunca decide por su cuenta: `render()` no lleva color y `render_colored(Color)` sí, de modo que quien renderiza declara el destino. La CLI resuelve el color con esta precedencia, de más fuerte a más débil:

1. `--color=always` o `--color=never` en la línea de comandos;
2. la variable `NO_COLOR`, que por convención desactiva el color sea cual sea su valor ([no-color.org](https://no-color.org));
3. si la salida de error es una terminal.

Que una bandera explícita gane sobre `NO_COLOR` es lo que hacen `rustc`, `cargo` y `ripgrep`: quien la escribe la está pidiendo para esa invocación puntual.

**La forma estructurada nunca lleva color**, aunque se pida: una secuencia de escape dentro de una cadena JSON rompe a quien la parsea.

### Paleta

Los roles se nombran por lo que significan, no por su color, para que un tema futuro cambie la paleta sin tocar el renderizado.

| Rol | Color | Por qué |
|---|---|---|
| Error | rojo apagado, negrita | El rojo básico de ANSI es agresivo en la mayoría de los temas, y un diagnóstico que grita se lee peor |
| Warning | ámbar | Claramente distinto del rojo del error |
| Ubicación y canal | gris | Importan, pero no son lo primero que se lee |
| Fragmento señalado | **solo negrita** | El source conserva su propia lectura; el énfasis marca el punto exacto |
| Marcador `^^^` | mismo rojo del error | Une visualmente el marcador con el encabezado |
| `= cause:` | azul suave | |
| `= help:` | violeta suave | Distinguirlo de la causa importa: responden preguntas distintas |

## Motivo

El color se implementa con secuencias ANSI escritas a mano, sin dependencia. Todo el compilador tiene una sola dependencia externa —`inkwell`— y está ahí porque escribir bindings de LLVM a mano no es razonable. El color son unas pocas secuencias de escape; agregar un crate cambiaría un costo real —una cosa más que auditar, versionar y mantener funcionando en tres plataformas— por muy poco.

La detección de terminal usa `std::io::IsTerminal`, que es de la biblioteca estándar.

## Consecuencias

- **La salida redirigida es byte a byte idéntica a la de antes de este cambio.** Hay un test que compara ambas quitando las secuencias de escape, porque la propiedad que importa es que el color agregue énfasis y no cambie lo que se dice.
- Los tests existentes no se tocaron: usan `render()`, que sigue sin color. Eso es evidencia de que el cambio es puramente aditivo.
- El énfasis del fragmento señalado parte por caracteres Unicode y no por bytes: cortar una `ñ` a la mitad corrompería la salida.
- En Windows el color depende de que la terminal procese secuencias ANSI. Las terminales modernas lo hacen; en una que no, la detección de TTY seguiría dando verdadero y aparecerían los códigos crudos. No se vio en la práctica y se revisará si aparece.
