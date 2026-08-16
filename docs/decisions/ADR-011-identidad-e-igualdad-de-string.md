# ADR-011 — Identidad, igualdad y normalización de `String`

- **Estado:** aceptada
- **Fecha:** 15 de agosto de 2026
- **Fase:** 2 (cierre)

## Contexto

`ZIRK_LANGUAGE_SPEC.md` sección 4 dice que `==` compara estructuralmente y que `is` compara la misma instancia. Para `String` eso no alcanza para decidir una implementación, porque Unicode permite escribir el mismo texto percibido con bytes distintos:

```text
"hó"  →  U+0068 U+00F3          (NFC, forma compuesta)
"hó"  →  U+0068 U+006F U+0301   (NFD, forma descompuesta)
```

Son cuatro bytes contra cinco. Un usuario que escribe ambas en su editor ve exactamente lo mismo, y muy probablemente no sabe cuál produjo cada una: depende del sistema operativo, del método de entrada y de por dónde pasó el texto antes de llegar al archivo.

La implementación actual compara bytes, así que hoy respondería `false`. Y la definición de `String` como secuencia **de grafemas** hace que esa respuesta sea incoherente con el resto del tipo: si `length` cuenta grafemas y la indexación devuelve grafemas, la igualdad no puede razonar en bytes.

## Decisión

**`is` compara referentes. `==` compara contenido, y es indiferente a la forma de normalización.**

```text
mut a = "hó";              // NFC
mut b = "hó";              // NFD, mismo texto percibido
mut c = a;

a == b   // true   — mismo contenido
a is b   // false  — referentes distintos
a is c   // true   — mismo referente
```

El hash se deriva de la forma canónica. Si se derivara de los bytes, dos claves iguales por `==` caerían en cubetas distintas y `Map<String, _>` contradiría al operador — un mapa en el que `m[a]` y `m[b]` son entradas separadas siendo `a == b` verdadero.

### Cómo se paga

La regla es cara si se implementa ingenuamente —normalizar ambos lados en cada comparación— y barata si se ordenan los caminos por frecuencia:

1. **Mismo handle** → iguales. Es también la respuesta de `is`. Un puntero.
2. **Longitud y bytes idénticos** → iguales. Un `memcmp`, sin asignar memoria. Este es el caso abrumadoramente mayoritario.
3. **Ambos marcados como canónicos, bytes distintos** → distintos. Dos formas canónicas distintas son textos distintos, por definición.
4. **Resto** → comparación canónica incremental, sin materializar copias normalizadas cuando sea evitable.

El handle guarda junto a los bytes lo que hace falta para llegar temprano a los primeros tres pasos: `is_ascii`, `normalization`, `grapheme_count` y `hash`. Una cadena ASCII no admite formas equivalentes distintas, así que `is_ascii` garantiza que el paso 2 decide.

**Los literales se normalizan en tiempo de compilación** y llegan al runtime marcados como canónicos. Es trabajo que se hace una vez, en una máquina que no tiene prisa, y convierte la comparación entre literales —lo más común que hay— en comparación de bytes.

## Alternativas consideradas

**Comparar bytes y ya.** Es lo que hay hoy y es lo más rápido posible. Se descarta porque hace que `"hó" == "hó"` dependa de con qué teclado se escribió cada literal, que es exactamente la clase de sorpresa que el lenguaje se propone no tener. Un usuario no puede depurar eso: los dos literales se ven idénticos en su pantalla.

**Normalizar al construir, siempre.** Toda `String` entra en forma canónica y la igualdad vuelve a ser un `memcmp`. Tentador, pero paga normalización en cada cadena construida en ejecución —incluidas las que nadie compara— y además destruye información: un programa que lee un archivo y lo vuelve a escribir alteraría bytes que no le pidieron alterar. Se descarta por eso segundo más que por el costo.

**Exponer la normalización al usuario** (`a.normalized() == b.normalized()`). Traslada el problema a quien escribe el programa y garantiza que se olvide, porque el caso que falla es justamente el que no se ve. Además obliga a que `Map<String, _>` documente cuál de las dos igualdades usa.

## Consecuencias

- El runtime gana una dependencia de datos de equivalencia canónica Unicode. Se acota a lo que la equivalencia canónica necesita, que es bastante menos que Unicode completo, y solo se toca en el paso 4.
- El compilador gana una responsabilidad menor: normalizar el texto de los literales al emitirlos. No toca la frontera de [ADR-005](./ADR-005-representacion-string.md), porque opera sobre el texto del literal y no sobre la representación del `String`.
- Los campos cacheados del handle —`normalization`, `grapheme_count`, `hash`— pasan a ser estado invalidable. Mientras `String` no sea mutable no hay ruta que los desincronice; la fase que introduzca la mutación se hace cargo de invalidarlos, y eso es parte de su contrato y de sus tests.
- La misma disciplina aplicará a `Char`: un grafema comparado por contenido canónico, por las mismas razones y con los mismos caminos rápidos.
