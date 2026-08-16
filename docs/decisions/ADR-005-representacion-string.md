# ADR-005 — `String` opaco tras la frontera del runtime

- **Estado:** aceptada
- **Fecha:** 12 de agosto de 2026
- **Fase:** 0

## Contexto

`ZIRK_LANGUAGE_SPEC.md` sección 3 define `String` como una secuencia Unicode *indexada semánticamente por graphemes y con índice/caché interno adaptativo*. Eso es trabajo considerable y corresponde a Fase 7.

Pero Fase 1 ya necesita un `String`: el literal de `stdout.println("Hola desde Zirk")`.

La trampa: si Fase 1 representa `String` como "puntero a bytes UTF-8" **dentro de la IR y del codegen**, esa asunción se filtra a cada sitio que toque strings y desandarla en Fase 7 es un refactor transversal.

## Decisión

`String` es un **tipo opaco** para el compilador. Su layout es un detalle privado de `zirk-runtime` ([ADR-002](./ADR-002-runtime-staticlib.md)).

```
IR de Zirk        ──▶  ZirkStr  (handle opaco, layout desconocido para la IR)
codegen LLVM      ──▶  { ptr, len }   ← representación de HOY, no contrato
zirk-runtime      ──▶  hoy:    UTF-8 plano
                       Fase 7: + índice de graphemes adaptativo
```

Toda operación sobre strings pasa por símbolos `extern "C"` del runtime. Ni la IR ni el codegen inspeccionan el contenido.

## Motivo

Añadir la indexación por graphemes en Fase 7 no debe requerir tocar el lexer, el parser, la IR ni el codegen. Con esta frontera, el cambio queda contenido en `zirk-runtime`.

El costo es una llamada indirecta donde podría haber acceso directo. Es aceptable: `ZIRK_COMPILER_SPEC.md` sección 5 asigna la optimización a LLVM, e inlinear llamadas triviales a través de una staticlib es exactamente lo que LTO resuelve en release.

## Consecuencias

- Los literales de string se materializan como constantes globales de LLVM más una llamada de construcción del runtime, no como punteros crudos entregados al usuario.
- La misma disciplina aplica a las futuras colecciones (`List<T>`, `Map<K,V>`, `Set<T>`): layout privado del runtime.
- Si en Fase 11 se mide que la indirección es un costo real en un caso concreto, se optimiza ahí con evidencia — no se rompe la frontera preventivamente.

## Enmienda — el handle es la identidad observable

- **Fecha:** 15 de agosto de 2026
- **Motivo:** el refinamiento normativo definió `String` como referencia mutable compartida con identidad observable mediante `is`.

La norma posterior no contradice esta decisión: la refuerza.

El handle opaco **es** la identidad que `is` compara. Dos bindings que aliasan la misma `String` comparten handle y son idénticos; dos handles distintos no lo son aunque su contenido coincida. Esto no añade nada al compilador —comparar dos handles es comparar dos punteros— y no rompe la opacidad, porque comparar identidades no es inspeccionar contenido.

De ahí se sigue el reparto de responsabilidades:

| Operación | Quién la resuelve | Por qué |
|---|---|---|
| `is` | comparación de handles | la identidad **es** el handle |
| `==` | runtime | depende del contenido y de la equivalencia canónica Unicode |
| hash | runtime | debe derivarse de la misma forma canónica que `==` |
| indexación por grafemas | runtime | exige el índice adaptativo que esta frontera protege |
| normalización | runtime, con literales precanonizados por el compilador | ver [ADR-011](./ADR-011-identidad-e-igualdad-de-string.md) |

El compilador conserva una sola responsabilidad nueva: emitir los literales ya en forma canónica. Es una transformación sobre el texto del literal, no sobre la representación del `String`, así que la frontera sigue intacta.

**La opacidad, lejos de estorbar, es lo que hace barato todo esto:** el runtime puede guardar junto a los bytes las banderas y los campos cacheados que necesite —`is_ascii`, `normalization`, `grapheme_count`, `hash`— sin que ninguna otra capa se entere ni tenga que cambiar cuando aparezcan.
