> **Las secciones son una lista de cobertura, no un orden de ejecución.**
>
> Clases, contratos y genéricos son un núcleo mutuamente dependiente: las clases
> necesitan genéricos para ser útiles, los genéricos necesitan contratos para
> restringir, y los contratos necesitan clases para implementarse. No existe un
> orden en que una parte se termine antes que las otras, así que leer esto como
> "terminar todas las clases antes de empezar contratos" llevaría a un callejón.
>
> La implementación avanza por **cortes verticales**:
>
> 1. Crear las representaciones mínimas compartidas: símbolos nominales,
>    parámetros de tipo, contratos, miembros y relaciones entre tipos.
> 2. Parsear el subconjunto mínimo de clases, interfaces y genéricos.
> 3. Registrar las declaraciones y resolver las referencias entre ellas, aunque
>    sus cuerpos todavía no estén verificados por completo.
> 4. Completar en conjunto la implementación de contratos, las restricciones
>    `from` y los miembros genéricos.
> 5. Añadir lowering y codegen **solo** después de cerrar cada construcción en
>    parser, resolución y chequeo.
> 6. Expandir después hacia traits, herencia, tipos algebraicos, casts y
>    optimizaciones.
>
> Cada caja marcada significa que su regla está cubierta de punta a punta, no
> que su capa esté terminada.

## 1. Léxico

> El cambio `alinear-implementacion-con-norma-refinada` adelantó parte de este
> grupo: `interface` y `trait` ya son palabras clave y ya declaran esta fase.
> Y corrigió el supuesto de 1.1: **`value` no debe ser palabra reservada**.
> Reservarla rompería `mut value = 1;` y `match r { Ok(value) => ... }`, que es
> código Zirk corriente —el propio spec lo usa en sus ejemplos—. `value class`
> se reconoce por posición, igual que `strict` en `inmut::strict`.

- [x] 1.1 Añadir las palabras clave que faltan: `interface` y `trait`, ya hechas por el cambio de alineación. `value` queda contextual, no reservada
- [ ] 1.2 Retirar de `phase()` lo que esta fase implementa: `class`, `construct`, `this`, `record`, `type`, `public`, `private`, `protected`, `abstract`, `implements`, `extends`, `from`, `as`, `is`, `interface`, `trait`
- [ ] 1.3 Tests: `value class` se parsea sin que `value` deje de ser un identificador válido, y las palabras retiradas dejan de declarar fase
- [ ] 1.4 Al implementar genéricos, partir el token `>>` en dos `>` dentro del parser de tipos: `Box<Box<Int32>>` termina en el token de desplazamiento

## 2. Gramática — clases y contratos

- [x] 2.1 Parsear `class` con campos, métodos, campos `public mut` por defecto y múltiples `construct`
- [x] 2.2 Parsear los modificadores de visibilidad y `abstract` sobre clases y miembros
- [x] 2.3 Parsear `extends` con una sola clase. `implements` llega con los contratos
- [x] 2.4 Parsear `this` como expresión, rechazándolo fuera de una clase
- [x] 2.5 Parsear `interface`, rechazando cuerpos en sus métodos
- [x] 2.6 Parsear `trait`, admitiendo cuerpos
- [x] 2.7 Tests: un caso válido y uno inválido por cada regla nueva

## 3. Gramática — genéricos

- [ ] 3.1 Parsear parámetros de tipo `<T>` en funciones, clases y tipos de datos
- [ ] 3.2 Parsear restricciones con `from`
- [ ] 3.3 Parsear argumentos de tipo en los usos (`Box<Int32>`)
- [ ] 3.4 Distinguir `<` de apertura de genéricos y de comparación
- [ ] 3.5 Tests: un caso válido y uno inválido por cada regla nueva

## 4. Gramática — tipos de datos y casts

- [ ] 4.1 Parsear variantes de enum con datos asociados
- [ ] 4.2 Parsear patrones con destructuring de variantes
- [ ] 4.3 Parsear `record` y value classes
- [ ] 4.4 Parsear uniones `A | B` y alias con `type`
- [ ] 4.5 Parsear casts postfijos (`as`) y prefijos (`<T>`)
- [ ] 4.6 Rechazar los casts que exigen `unsafe`, indicando la fase que los trae
- [ ] 4.7 Parsear `?.`, retirando el diagnóstico de fase de la fase anterior
- [ ] 4.8 Rechazar temporalmente `Function(...) => R` y `Fn(...) => R` en posición de tipo, indicando la fase que los implementa (D9)
- [ ] 4.9 Tests: un caso válido y uno inválido por cada regla nueva

> **El spec de objetos se actualizó el 17 de agosto de 2026 y varias reglas de
> este grupo cambiaron de contenido.** Lo ya implementado se realineó:
>
> - Todo atributo omitido recibe el default de su tipo antes del constructor, así
>   que este solo debe escribir lo que no tiene default (una clase no lo tiene).
> - `super(...)` y `super.method()` existen, lo que retira la regla provisional
>   de que una subclase inicializara los campos heredados.
> - Reemplazar un método heredado exige `override fn`.
> - `abstract class` dejó de ser algo que se extiende: es un conjunto de
>   requisitos que se adopta con `implements`, sin constructor ni layout.

## 5. Tipos — nominalidad y miembros

- [x] 5.1 Representar clases, records, value classes y enums como tipos nominales
- [x] 5.2 Construir la jerarquía de herencia y detectar ciclos
- [x] 5.3 Resolver miembros contra el tipo y su cadena de herencia
- [x] 5.4 Verificar visibilidad, distinguiendo miembro oculto de miembro inexistente
- [x] 5.5 Admitir subclase donde se espera la base, y rechazar la dirección contraria
- [x] 5.6 Verificar `construct`: que exista, resolver por aridad e inicializar todo campo. Falta desempatar firmas de igual aridad por tipo y por etiqueta, que llega con los argumentos nombrados reordenados
- [x] 5.7 Verificar la redefinición de métodos: misma firma, y rechazo si difiere — llega con `extends`, que es lo que hace posible redefinir
- [ ] 5.8 Verificar `abstract class` como conjunto de requisitos adoptado con `implements`: sin constructor, sin estado y sin contribución al layout
- [x] 5.9 Tipar `?.` como el tipo del miembro en forma nulable (D7)
- [ ] 5.10 Rechazar anotar una closure como parámetro, retorno o campo (D9)
- [ ] 5.11 Rechazar que una closure escape de la función que la crea (D9)
- [ ] 5.12 Tests: una closure sigue funcionando en variable local e invocación (D9)
- [ ] 5.13 Tests: un caso válido y uno inválido por cada regla nueva
- [ ] 5.14 Rechazar shadowing ordinario y resolver una captura homónima únicamente mediante `this.nombre`
- [ ] 5.15 Aplicar la matriz `mut`/`inmut`/`inmut::strict` a referencias de objeto y rechazar aliases que rompan strictness

## 6. Tipos — contratos

- [x] 6.1 Registrar interfaces y traits como contratos, con sus métodos
- [x] 6.2 Verificar que una declaración implemente por completo lo que dice implementar
- [x] 6.3 Copiar en la clase los métodos de trait con cuerpo que no redefine (D4)
- [ ] 6.4 Rechazar el conflicto entre dos traits que aportan el mismo método (D4)
- [x] 6.5 Admitir una implementación donde se espera su contrato
- [x] 6.6 Definir los contratos de operador y sus métodos reservados (`_add`, `_subtract`, etc.), impidiendo reabrir tipos nativos (D6)
- [x] 6.7 Resolver los operadores por contrato en vez de por lista fija de tipos
- [x] 6.8 Hacer que `String` implemente el contrato de concatenación, cerrando la deuda de `+`
- [x] 6.8a Hacer que `String` implemente repetición checked en ambos órdenes (`String * Integer`, `Integer * String`)
- [ ] 6.9 Definir `Iterable<T>` e `Iterator<T>` como contratos del lenguaje
- [ ] 6.10 Hacer que `for ... in` exija `Iterable<T>`, y que rangos y `String` lo implementen (D8)
- [x] 6.11 Tests: un caso válido y uno inválido por cada regla nueva

## 7. Tipos — genéricos

- [ ] 7.1 Representar parámetros de tipo y su alcance en la declaración
- [ ] 7.2 Verificar el cuerpo genérico una sola vez contra sus restricciones (D5)
- [ ] 7.3 Verificar en el sitio de uso que el argumento cumple la restricción `from`
- [ ] 7.4 Rechazar en el cuerpo lo que la restricción no garantiza
- [ ] 7.5 Rechazar varianza, tipos asociados y orden superior con diagnóstico propio
- [ ] 7.6 Unificar tipos genéricos en llamadas, inferiendo el argumento cuando es inequívoco
- [ ] 7.7 Tests: un caso válido y uno inválido por cada regla nueva

## 8. Tipos — tipos de datos y casts

- [ ] 8.1 Extender los enums con datos asociados, conservando los tradicionales, su nombre como valor por defecto y mappings explícitos con `->`
- [ ] 8.2 Verificar aridad y tipos de un constructor de variante
- [ ] 8.3 Tipar los patrones con destructuring, ligando los nombres a su tipo
- [ ] 8.4 Extender la exhaustividad a enums con datos asociados
- [ ] 8.5 Implementar records: inmutables, con igualdad estructural derivada
- [ ] 8.6 Implementar value classes: sin identidad observable
- [ ] 8.7 Implementar uniones y exigir discriminarlas antes de usarlas
- [ ] 8.8 Implementar alias con `type`
- [ ] 8.9 Verificar casts: admitir los relacionados, rechazar los que no lo están
- [ ] 8.10 Tests: un caso válido y uno inválido por cada regla nueva

## 9. Runtime — alocación de objetos

- [x] 9.1 Añadir la función de alocación como `extern "C"` tras la frontera ABI (D1, ADR-002)
- [x] 9.2 Documentar en el runtime que no libera y que la estrategia llega en Fase 4
- [x] 9.3 Tests: el símbolo aparece sin mangling en la biblioteca estática
- [x] 9.4 Tests: un programa que construye objetos termina con código de salida cero

## 10. IR — objetos y despacho

- [x] 10.1 Representar el layout de un objeto: cabecera y campos (D2)
- [x] 10.2 Colocar los campos heredados antes que los propios (D2)
- [x] 10.3 Bajar la construcción a alocación abstracta más inicialización
- [x] 10.4 Bajar el acceso a campo a lectura por desplazamiento
- [x] 10.5 Bajar la llamada directa cuando el método no es redefinible (D3). Sin herencia ninguno lo es, así que hoy toda llamada es directa
- [x] 10.6 Bajar la llamada indirecta por tabla cuando lo es (D3)
- [ ] 10.7 Bajar el despacho a través de un contrato por la tabla de la interfaz (D3)
- [ ] 10.8 Bajar `?.` a comprobación de nulidad con dos bloques (D7)
- [x] 10.9 Extender el verificador a las instrucciones nuevas
- [x] 10.10 Tests: IR esperada para cada construcción nueva

## 11. IR — genéricos y tipos de datos

- [ ] 11.1 Especializar cada combinación de argumentos de tipo usada (D5)
- [ ] 11.2 Reutilizar la especialización cuando la combinación se repite
- [ ] 11.3 Bajar los enums con datos asociados: discriminante más carga útil
- [ ] 11.4 Bajar el destructuring de patrones a lecturas de la carga útil
- [ ] 11.5 Bajar records y value classes sin indirección
- [ ] 11.6 Bajar el cast comprobado a comparación de descriptor
- [ ] 11.7 Tests: IR esperada para cada construcción nueva

## 12. Backend LLVM

- [x] 12.1 Traducir el layout de objetos, con el prefijo compartido entre base y subclase
- [x] 12.2 Emitir el descriptor de tipo de cada tipo con identidad
- [x] 12.3 Emitir la tabla de métodos por tipo, con índices estables al heredar
- [ ] 12.4 Emitir una tabla por interfaz implementada y su búsqueda en el descriptor
- [x] 12.5 Traducir la llamada indirecta del despacho dinámico
- [ ] 12.6 Traducir el cast comprobado, transfiriendo al runtime cuando falla
- [ ] 12.7 Traducir value classes inline, sin puntero intermedio
- [ ] 12.8 Tests: el módulo LLVM generado verifica para cada construcción nueva

## 13. Verificación de punta a punta

- [ ] 13.1 Ampliar el corpus con programas válidos: clases, herencia, contratos, genéricos, tipos de datos, casts
- [ ] 13.2 Ampliar el corpus con programas inválidos, con snapshots de sus diagnósticos
- [ ] 13.3 Verificar que el corpus de las fases 1 y 2 sigue verde sin modificarlo
- [ ] 13.4 Test de que `"a" + "b"` concatena, cerrando la deuda de la fase anterior
- [ ] 13.5 Test de `for ... in` sobre un tipo propio que implementa `Iterable<T>`
- [ ] 13.6 Sondear los cruces entre construcciones, no solo cada una por separado
- [ ] 13.7 Confirmar que CI pasa en las cuatro plataformas de la matriz

## 14. Cierre

- [ ] 14.1 Actualizar `docs/init/ZIRK_AGENT_PROMPT.md` con el estado de la fase
- [ ] 14.2 Registrar en ADRs las decisiones durables: layout de objetos y forma del despacho
- [ ] 14.3 Resolver o registrar como pendientes las preguntas abiertas del design
- [ ] 14.4 Revisar qué deudas de fases anteriores quedan vivas y con qué fecha
- [ ] 14.5 Confirmar el límite de Fase 3 de D9 sin presentarlo como semántica final: ninguna closure escapa ni se anota todavía
