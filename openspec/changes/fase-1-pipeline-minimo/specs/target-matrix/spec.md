## ADDED Requirements

### Requirement: El ejecutable producido corre en el host

La verificación SHALL comprobar que el ejecutable generado a partir de código Zirk se ejecuta correctamente en la plataforma del host, no solo que el archivo objeto se emite.

Hasta la Fase 0 se verificaba la emisión de objetos y un binario construido directamente desde Rust. Con un lenguaje real por delante, la evidencia que importa es que un `.zrk` termina siendo un proceso que corre.

#### Scenario: Programa de referencia
- **WHEN** se compila y ejecuta un programa `.zrk` que imprime una cadena
- **THEN** el proceso termina con código de salida 0
- **AND** la salida estándar contiene exactamente la cadena esperada

#### Scenario: Verificación en las plataformas soportadas
- **WHEN** se ejecuta el pipeline de integración continua
- **THEN** la compilación y ejecución del programa de referencia se verifica en cada plataforma de la matriz
