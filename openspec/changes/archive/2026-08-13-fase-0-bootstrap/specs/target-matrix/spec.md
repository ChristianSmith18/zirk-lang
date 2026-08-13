## ADDED Requirements

### Requirement: Emisión de objetos para los targets del spec

El backend SHALL emitir archivos objeto válidos para los targets de `ZIRK_COMPILER_SPEC.md` sección 6, desde cualquier host soportado. Esta capacidad corresponde a la *portabilidad B* de [ADR-004](../../../../docs/decisions/ADR-004-portabilidad.md): qué produce el compilador.

Targets cubiertos: `x86-windows`, `x86_64-windows`, `aarch64-windows`, `x86-linux`, `x86_64-linux`, `armv7-linux`, `aarch64-linux`, `x86_64-macos`, `aarch64-macos`.

#### Scenario: Emisión cross-target desde un único host
- **WHEN** se ejecuta el test de matriz de targets en cualquier plataforma soportada
- **THEN** se emite un archivo objeto para cada uno de los nueve targets
- **AND** cada objeto tiene el formato de contenedor correcto: Mach-O para macOS, ELF para Linux, COFF para Windows
- **AND** cada objeto declara la arquitectura correspondiente al target

#### Scenario: Target no soportado por el backend
- **WHEN** se solicita emisión para un triple que LLVM no reconoce
- **THEN** el compilador SHALL fallar con un diagnóstico que nombre el target solicitado
- **AND** NO SHALL producir un objeto inválido

### Requirement: Construcción del compilador en las tres plataformas

El compilador SHALL poder construirse desde fuente en Windows, Linux y macOS. Esta capacidad corresponde a la *portabilidad A* de ADR-004: dónde se construye el compilador.

#### Scenario: Verificación continua en integración
- **WHEN** se ejecuta el pipeline de integración continua
- **THEN** el workspace se construye y sus tests pasan en Linux (x86_64 y aarch64), macOS (x86_64 y aarch64) y Windows (x86_64)

#### Scenario: Fallo en una plataforma
- **WHEN** el workspace no construye o sus tests fallan en cualquier plataforma de la matriz
- **THEN** la portabilidad se considera no verificada
- **AND** el hallazgo SHALL tratarse como bloqueante, no como pendiente

### Requirement: Linker único y multiplataforma

El proyecto SHALL usar `lld` como linker de referencia, cubriendo ELF, Mach-O y COFF desde una misma versión, en lugar de depender del linker por defecto de cada host.

#### Scenario: Disponibilidad de drivers
- **WHEN** se verifica la instalación del toolchain
- **THEN** `lld` provee los drivers para los tres formatos de contenedor requeridos

### Requirement: Cross-linking fuera de alcance en esta fase

La producción de un ejecutable completo para un target distinto del host SHALL quedar fuera de alcance en esta fase, por requerir sysroots de destino.

#### Scenario: Alcance de la verificación de targets
- **WHEN** se verifica la matriz de targets
- **THEN** la verificación cubre emisión de objetos
- **AND** NO cubre enlace de ejecutables para targets distintos del host
