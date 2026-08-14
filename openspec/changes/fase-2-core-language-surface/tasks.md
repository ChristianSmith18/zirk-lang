## 1. Léxico y gramática — bucles y condicional como expresión

- [x] 1.1 Reconocer las palabras clave de esta fase, añadir `in` y los tokens `..`, `..=`, `...`, y retirarlos todos de `phase()`
- [x] 1.2 Parsear `for (init; cond; incr) { }`, `for x in expr { }`, `while cond { }`, `loop { }`
- [x] 1.3 Parsear `break` y `continue`, sin exigir aún que estén dentro de un bucle (lo valida sema)
- [x] 1.4 Extender `if`/`else` para admitirse en posición de expresión (D7)
- [x] 1.5 Retirar del diagnóstico "construcción de fase posterior" las palabras `for`, `while`, `loop`, `match`
- [x] 1.6 Tests: un caso válido y uno inválido por cada regla nueva

## 2. Gramática — funciones completas y closures

- [x] 2.1 Parsear parámetros opcionales (`nombre?: Tipo`)
- [x] 2.2 Parsear valores por defecto en parámetros
- [x] 2.3 Parsear el parámetro variadic (`...nombre: Tipo`), rechazando que no sea el último
- [x] 2.4 Parsear argumentos nombrados en llamadas
- [x] 2.5 Parsear lambdas de expresión y de bloque (D4)
- [x] 2.6 Tests: un caso válido y uno inválido por cada regla nueva

## 3. Gramática — `match` y enum mínimo

- [x] 3.1 Parsear `enum Nombre { Constructor, ... }` sin datos asociados (D1)
- [x] 3.2 Parsear `match` como sentencia y como expresión, con brazos `patrón => cuerpo`
- [x] 3.3 Parsear patrones: literal, constructor de enum, binding, `_`
- [x] 3.4 Emitir diagnóstico específico para `match with` (fuera de alcance, depende de `Resource<E>`)
- [x] 3.5 Tests: un caso válido y uno inválido por cada regla nueva

## 4. Gramática — nulabilidad y operadores compuestos

- [x] 4.1 Parsear `T?` en posición de tipo
- [x] 4.2 Parsear `null` como literal
- [x] 4.3 Parsear `??` con la precedencia correcta respecto al resto de operadores
- [x] 4.4 Rechazar `?.` con el diagnóstico de fase, indicando Fase 3 (D8)
- [x] 4.5 Expandir `+=`, `-=`, `*=`, `/=`, `%=` a la asignación equivalente, en posición de sentencia (D9)
- [x] 4.6 Expandir `++` y `--` prefijo y postfijo a la asignación equivalente, en posición de sentencia (D9)
- [x] 4.7 Rechazar incremento y decremento en posición de expresión, con diagnóstico explícito (D9)
- [x] 4.8 Tests: un caso válido y uno inválido por cada regla nueva

## 5. Gramática — módulos

- [x] 5.1 Parsear el modificador `share` sobre declaraciones de nivel superior
- [x] 5.2 Parsear `import { nombres } from "ruta"` con rutas locales entre comillas
- [x] 5.3 Parsear alias de importación (`nombre -> alias`)
- [x] 5.4 Parsear `import { nombres } from modulo.estandar;` sin comillas
- [x] 5.5 Parsear `use nombre;`
- [x] 5.6 Retirar el diagnóstico de Fase 1 que rechazaba todo `import`
- [x] 5.7 Tests: un caso válido y uno inválido por cada regla nueva

## 6. Resolución de módulos (`zirk-modules`)

- [ ] 6.1 Construir el grafo de `import` entre los archivos de un crate (D6)
- [ ] 6.2 Detectar y reportar ciclos de `import`, directos e indirectos
- [ ] 6.3 Resolver rutas locales relativas al archivo que importa
- [ ] 6.4 Aplicar la regla de visibilidad binaria (`share` vs. privado al archivo)
- [ ] 6.5 Detectar y reportar colisiones de nombres `share` entre archivos
- [ ] 6.6 Resolver alias de importación en la tabla de scopes
- [ ] 6.7 Resolver `use` sobre nombres ya importados, rechazando los que no lo están
- [ ] 6.8 Reconocer `std.io` como único módulo estándar de esta fase
- [ ] 6.9 Ejecutar esta pasada antes del chequeo de tipos existente, sin cambiar su forma
- [ ] 6.10 Tests: un caso válido y uno inválido por cada regla, incluyendo un crate de varios archivos

## 7. Tipos y flujo — bucles y `if` como expresión

- [ ] 7.1 Exigir `Boolean` sin truthiness en la condición de `for` y `while`
- [ ] 7.2 Rechazar `break`/`continue` fuera de un bucle
- [ ] 7.3 Resolver a qué bucle apunta `break`/`continue` en bucles anidados
- [ ] 7.4 Implementar el protocolo mínimo de `for ... in`: rangos y `String` (D3)
- [ ] 7.5 Rechazar `for ... in` sobre tipos no soportados, con diagnóstico de fase
- [ ] 7.6 Tipar `if` como expresión: exigir ambas ramas y tipos compatibles (D7)
- [ ] 7.7 Extender el análisis de retorno-en-toda-ruta para considerar `if` exhaustivo y bucles infinitos sin `break` que salga
- [ ] 7.8 Tests: un caso válido y uno inválido por cada regla nueva

## 8. Tipos y flujo — funciones completas y closures

- [ ] 8.1 Resolver argumentos nombrados a posición contra la firma (D4)
- [ ] 8.2 Sustituir parámetros ausentes por su valor por defecto, evaluado en el sitio de llamada
- [ ] 8.3 Empaquetar argumentos sobrantes en el parámetro variadic
- [ ] 8.4 Tipar valores nulos para parámetros opcionales sin argumento
- [ ] 8.5 Inferir el tipo función de una lambda a partir de parámetros y cuerpo
- [ ] 8.6 Registrar qué variables externas captura una closure
- [ ] 8.7 Rechazar la mutación de una variable capturada (D2)
- [ ] 8.8 Tests: un caso válido y uno inválido por cada regla nueva

## 9. Tipos y flujo — `match`

- [ ] 9.1 Registrar el conjunto de constructores de cada `enum` declarado
- [ ] 9.2 Verificar exhaustividad de `match` sobre `enum`: todos los constructores o `_`
- [ ] 9.3 Exigir `_` como brazo final en `match` sobre tipos sin conjunto cerrado
- [ ] 9.4 Verificar que todos los brazos de un `match`-expresión produzcan un tipo común
- [ ] 9.5 Tests: un caso válido y uno inválido por cada regla nueva

## 10. Tipos y flujo — nulabilidad

- [ ] 10.1 Representar `T?` como tipo propio, distinto de `T`, en el sistema de tipos (D5)
- [ ] 10.2 Admitir `T` donde se espera `T?`, y rechazar la dirección contraria sugiriendo `??`
- [ ] 10.3 Tipar `null`, rechazándolo donde el tipo no admite ausencia de valor
- [ ] 10.4 Tipar `??` exigiendo un tipo común, produciendo el tipo no nulable cuando el fallback no lo es
- [ ] 10.5 Rechazar `??` sobre un operando izquierdo no nulable
- [ ] 10.6 Tests: un caso válido y uno inválido por cada regla nueva

## 11. IR — bucles, `if`-expresión y `break`/`continue`

- [ ] 11.1 Extender el lowering para producir grafos de bloques con ciclos
- [ ] 11.2 Bajar `while`, `loop`, `for` y `for ... in` a bloques de condición/cuerpo/continuación
- [ ] 11.3 Bajar `break`/`continue` a saltos directos al bloque correspondiente del bucle que los contiene
- [ ] 11.4 Bajar `if`-expresión produciendo el valor de la rama tomada en el bloque de continuación
- [ ] 11.5 Verificador de IR: aceptar ciclos bien formados, seguir rechazando terminadores fuera de lugar
- [ ] 11.6 Tests: IR esperada para cada construcción nueva

## 12. IR — closures, `match`, nulabilidad

- [ ] 12.1 Definir la operación de alocación de entorno de closure, reutilizando la alocación abstracta existente (D2)
- [ ] 12.2 Bajar la creación de una closure a función independiente más entorno con capturas copiadas
- [ ] 12.3 Bajar la llamada a un valor de closure como llamada indirecta con entorno implícito
- [ ] 12.4 Bajar `match` a comparaciones sobre el discriminante con salto a cada bloque de brazo
- [ ] 12.5 Bajar `match`-expresión con bloque de continuación común que recibe el valor del brazo
- [ ] 12.6 Bajar `??` a comprobación explícita de nulidad con evaluación perezosa del fallback (D5)
- [ ] 12.7 Tests: IR esperada para cada construcción nueva

## 13. Backend LLVM

- [ ] 13.1 Traducir bloques con ciclos, verificando el módulo LLVM resultante
- [ ] 13.2 Traducir closures: función LLVM con entorno como primer argumento, valor agregado (función, entorno)
- [ ] 13.3 Traducir llamadas indirectas a closures
- [ ] 13.4 Traducir `match` exhaustivo sobre `enum` a `switch` de LLVM
- [ ] 13.5 Traducir `match` no exhaustivo o sobre otros tipos a comparaciones encadenadas
- [ ] 13.6 Traducir la comprobación de nulidad de `??`, sin costo para tipos no nulables
- [ ] 13.7 Tests: el módulo LLVM generado verifica para cada construcción nueva

## 14. Verificación de punta a punta

- [ ] 14.1 Ampliar el corpus con programas válidos: bucles, closures, `match` exhaustivo, nulabilidad, módulos de varios archivos
- [ ] 14.2 Ampliar el corpus con programas inválidos, con snapshots de sus diagnósticos
- [ ] 14.3 Test de un crate de varios archivos con `share`/`import`/`use`, compilado y ejecutado de punta a punta
- [ ] 14.4 Test de ciclo de importación, verificando el diagnóstico y la cadena reportada
- [ ] 14.5 Confirmar que CI pasa en las cuatro plataformas de la matriz

## 15. Cierre

- [ ] 15.1 Actualizar `docs/init/ZIRK_AGENT_PROMPT.md` con el estado de la fase
- [ ] 15.2 Registrar en ADRs cualquier decisión de arquitectura tomada durante la implementación
- [ ] 15.3 Resolver o registrar como pendientes las preguntas abiertas del design
