## 1. Léxico

- [x] 1.1 Definir el tipo de token del subset, incluyendo las palabras clave del lenguaje completo (D6)
- [x] 1.2 Implementar el escaneo de identificadores, palabras clave y delimitadores, con span por token
- [x] 1.3 Implementar literales enteros con separador `_`, rechazando posiciones inválidas
- [x] 1.4 Implementar literales de cadena con escapes `\n`, `\t`, `\"` y `\\`
- [x] 1.5 Implementar literales booleanos y comentarios de línea y de bloque
- [x] 1.6 Emitir diagnósticos léxicos: cadena sin cerrar, comentario sin cerrar, escape desconocido, carácter no reconocido
- [x] 1.7 Tests: un caso válido y uno inválido por cada regla léxica

## 2. Árbol de sintaxis

- [x] 2.1 Definir los nodos del subset en `zirk-ast`, cada uno con su span (D1)
- [x] 2.2 Definir la representación de tipos sintácticos (`Void`, `Int32`, `Boolean`, `String`)
- [x] 2.3 Documentar en `lib.rs` que la AST es privada y que la Syntax API pública es de Fase 10

## 3. Gramática

- [x] 3.1 Implementar el parseo de declaraciones de función con parámetros y tipo de retorno
- [x] 3.2 Implementar declaraciones `mut` e `inmut`, con tipo explícito o inferido
- [x] 3.3 Implementar expresiones con la precedencia y asociatividad del spec
- [x] 3.4 Implementar `if`/`else` como sentencia, con cuerpos siempre entre llaves
- [x] 3.5 Implementar llamadas a función, asignación y `return`
- [x] 3.6 Admitir la omisión del punto y coma cuando no hay ambigüedad
- [x] 3.7 Emitir diagnósticos específicos para construcciones de fases posteriores (D6)
- [x] 3.8 Emitir un diagnóstico propio para `import`, indicando que los módulos llegan después
- [x] 3.9 Tests: un caso válido y uno inválido por cada regla gramatical

## 4. Nombres, tipos y flujo

- [x] 4.1 Construir la tabla de scopes con anidamiento de bloques y sombras
- [x] 4.2 Resolver identificadores a su declaración, con diagnóstico si no existe
- [x] 4.3 Implementar el chequeo de tipos del subset, sin conversiones implícitas
- [x] 4.4 Rechazar truthiness: exigir `Boolean` en toda condición
- [x] 4.5 Restringir `&&`, `||` y `!` a operandos booleanos
- [x] 4.6 Implementar la inferencia desde el inicializador cuando es inequívoca
- [x] 4.7 Verificar mutabilidad: rechazar reasignación de `inmut`
- [x] 4.8 Verificar aridad y tipos de los argumentos contra la firma
- [x] 4.9 Verificar coherencia del retorno y que toda ruta de una función no `Void` retorne
- [x] 4.10 Análisis de flujo para uso antes de disponibilidad
- [x] 4.11 Rechazar literales enteros fuera del rango de su tipo
- [x] 4.12 Verificar la existencia y firma de `main`
- [x] 4.13 Tests: un caso válido y uno inválido por cada regla de tipos

## 5. Representación intermedia

- [x] 5.1 Definir los tipos de la IR y su representación de valores tipados
- [x] 5.2 Definir bloques básicos con terminador único (D2)
- [x] 5.3 Definir el conjunto de instrucciones: aritméticas, comparación, lógicas, llamada, carga, almacenamiento, salto, salto condicional, retorno
- [x] 5.4 Definir la operación abstracta de alocación, sin nombrar estrategia de memoria (D3, ADR-003)
- [x] 5.5 Conservar la ubicación del source en cada instrucción
- [x] 5.6 Implementar el lowering desde el árbol verificado, con locales como slots (D2)
- [x] 5.7 Lowering de `if`/`else` a bloques con salto condicional
- [x] 5.8 Verificador de IR bien formada, usado en tests
- [x] 5.9 Tests: IR esperada para cada construcción del subset

## 6. Backend

- [ ] 6.1 Traducir tipos de la IR a tipos de LLVM
- [ ] 6.2 Traducir bloques básicos e instrucciones a LLVM IR
- [ ] 6.3 Implementar aritmética con detección de overflow mediante intrínsecos de LLVM
- [ ] 6.4 Implementar división con comprobación de divisor cero
- [ ] 6.5 Generar el entrypoint que invoca `zirk_rt_init` y `zirk_rt_shutdown` alrededor de `main`
- [ ] 6.6 Materializar literales de cadena como constantes globales más llamada al runtime (D5)
- [ ] 6.7 Tests: el módulo LLVM generado verifica para cada construcción del subset

## 7. Runtime

- [ ] 7.1 Definir la representación interna de `String`, privada del runtime (ADR-005)
- [ ] 7.2 Implementar `zirk_str_from_utf8` como `extern "C"`
- [ ] 7.3 Implementar `zirk_io_println` como `extern "C"`, con vaciado antes de terminar
- [ ] 7.4 Tests: contenido no ASCII se escribe correctamente en UTF-8
- [ ] 7.5 Tests: los símbolos nuevos aparecen sin mangling en la biblioteca estática

## 8. CLI

- [ ] 8.1 Implementar el subcomando de compilación sobre un archivo único
- [ ] 8.2 Implementar el subcomando de compilar y ejecutar, propagando el código de salida
- [ ] 8.3 Invocar el linker desde la CLI, usando el toolchain de LLVM pineado (D7)
- [ ] 8.4 Presentar diagnósticos en la salida de error, en formato legible y estructurado
- [ ] 8.5 Diagnóstico ante múltiples archivos, indicando que los proyectos llegan después
- [ ] 8.6 Resolver dónde queda el ejecutable al ejecutar (pregunta abierta del design)

## 9. Verificación de punta a punta

- [ ] 9.1 Crear el corpus de programas `.zrk` válidos, uno por construcción del subset
- [ ] 9.2 Crear el corpus de programas inválidos, con snapshots de sus diagnósticos
- [ ] 9.3 Test de punta a punta: compilar, enlazar, ejecutar y comparar salida y código de salida
- [ ] 9.4 Verificar el programa de referencia del roadmap: `fn main(): Void { stdout.println("Hola desde Zirk"); }`
- [ ] 9.5 Confirmar que CI pasa en las cuatro plataformas de la matriz

## 10. Cierre

- [ ] 10.1 Actualizar `docs/init/ZIRK_AGENT_PROMPT.md` con el estado de la fase
- [ ] 10.2 Registrar en ADRs cualquier decisión de arquitectura tomada durante la implementación
- [ ] 10.3 Resolver o registrar como pendientes las preguntas abiertas del design
