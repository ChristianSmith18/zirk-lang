# ADR-007 — Forma de la representación intermedia

- **Estado:** aceptada
- **Fecha:** 13 de agosto de 2026
- **Fase:** 1

## Contexto

`ZIRK_COMPILER_SPEC.md` sección 4 exige que la IR sea tipada, independiente del target y versionada, y que conserve información suficiente para especialización de genéricos, devirtualización, escape analysis, comprobaciones de seguridad, vectorización y debug info.

Además es lo que se distribuye dentro de un `.zpkg` como `portable.ir` (sección 4), así que su forma es un contrato con el futuro, no un detalle interno del compilador.

Esta decisión se toma en la Fase 1, cuando el subset del lenguaje es trivial. Ese es exactamente el riesgo: lo que alcanza para `if`/`else` y aritmética puede no alcanzar para clases, genéricos y concurrencia.

## Decisión

**Código de tres direcciones, tipado, sobre bloques básicos, con las variables locales como slots y sin SSA propia.**

```
   Función
     ├── slots        (parámetros y locales; se leen y escriben con Load/Store)
     └── bloques      (cada uno termina en exactamente un terminador)
           ├── instrucciones
           └── Return | Jump | Branch
```

Invariantes que el verificador hace cumplir:

- todo bloque tiene exactamente un terminador;
- los valores **no cruzan bloques**: lo que necesita sobrevivir a un salto viaja por un slot;
- toda instrucción conserva la ubicación del source que la originó;
- las operaciones de alocación no nombran estrategia de memoria.

## Motivo

**Bloques básicos y no un árbol.** Para el subset de la Fase 1 un árbol habría alcanzado. Se descarta porque:

- los análisis que el spec exige conservar —escape analysis, vectorización— son análisis de flujo, incómodos sobre un árbol;
- la Fase 2 introduce bucles, `break` y `continue`, que sobre un árbol obligan a rehacer la representación;
- el mapeo a LLVM es directo, porque LLVM ya es exactamente eso.

**Sin SSA propia.** Las locales son slots con carga y almacenamiento, y la promoción a registros se delega al backend. SSA propia —con funciones phi y su mantenimiento— es trabajo real que no paga hasta que existan optimizaciones propias. Cuando existan, se introduce como una pasada sobre esta forma, no en su lugar.

Que los valores no crucen bloques es la contrapartida de esa decisión, y por eso el verificador lo comprueba: es el invariante que hace innecesarias las funciones phi.

**Tipada de forma independiente del frontend.** `zirk-ir` define sus propios tipos en vez de reutilizar los de `zirk-sema`. La IR es la frontera que se distribuye en un `.zpkg`; no debe moverse cada vez que cambia la representación interna de tipos del frontend.

## Relación con la estrategia de memoria

La IR expresa **que** un valor necesita almacenamiento, nunca **cómo** se obtiene ni se libera. Ninguna instrucción nombra malloc, recuento de referencias ni recolección de basura.

Es requisito directo de [ADR-003](./ADR-003-memoria.md): la estrategia se decide en la Fase 4, y una IR que la adelante haría esa decisión mucho más cara. Hay un test que fija la restricción, porque es del tipo que se erosiona sin querer.

## Consecuencias

- **Añadir una construcción del lenguaje es añadir instrucciones, no cambiar la forma.** Bucles, `match` y closures encajan en bloques básicos sin rediseño.
- **El verificador es parte del contrato**, no una herramienta de depuración. Un bug de lowering aparece como mensaje preciso en vez de como un fallo ilegible de LLVM o, peor, un binario que miscompila en silencio.
- **La IR todavía no está versionada.** El spec lo exige para `.zpkg`; corresponde a la Fase 8, cuando exista algo que distribuir. Anotado para que no se descubra tarde.
- Si en la Fase 3 los genéricos exigen información que esta forma no conserva, se revisa este ADR antes de deformar la IR con parches.
