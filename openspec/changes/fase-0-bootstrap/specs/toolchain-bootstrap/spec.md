## ADDED Requirements

### Requirement: Pin de versión mayor de LLVM

El proyecto SHALL construirse exclusivamente contra LLVM 20.1.x mediante la feature `llvm20-1` de `inkwell` 0.10. Una versión mayor distinta de LLVM NO SHALL considerarse soportada.

#### Scenario: LLVM correcto disponible
- **WHEN** `LLVM_SYS_201_PREFIX` apunta a una instalación de LLVM 20.1.x con bibliotecas estáticas
- **THEN** `cargo build` del workspace completa sin errores de enlace

#### Scenario: Versión mayor de LLVM incorrecta
- **WHEN** el entorno provee una versión mayor de LLVM distinta de 20
- **THEN** el build SHALL fallar con un diagnóstico que indique la versión encontrada, la esperada, y una referencia a `docs/TOOLCHAIN.md`
- **AND** el diagnóstico NO SHALL ser un error de enlace crudo del linker

#### Scenario: LLVM ausente
- **WHEN** no existe `LLVM_SYS_201_PREFIX` ni un `llvm-config` compatible en el `PATH`
- **THEN** el build SHALL fallar indicando qué variable de entorno definir

### Requirement: Cadena completa a binario nativo

El toolchain SHALL demostrar que produce un binario nativo ejecutable partiendo de LLVM IR construido en proceso, sin depender de ninguna sintaxis de Zirk.

#### Scenario: Generación, enlace y ejecución
- **WHEN** se ejecuta el test de sanity de `zirk-codegen-llvm`
- **THEN** se construye un módulo LLVM que verifica correctamente
- **AND** se emite un archivo objeto para el target del host
- **AND** el objeto se enlaza en un ejecutable nativo
- **AND** el ejecutable corre y termina con exit code 0

### Requirement: Configuración de toolchain no versionada por máquina

La ubicación de LLVM SHALL resolverse por variable de entorno. NO SHALL versionarse ninguna ruta absoluta específica de una máquina o plataforma en el repositorio.

#### Scenario: Ruta absoluta en configuración versionada
- **WHEN** `.cargo/config.toml` u otro archivo versionado define `LLVM_SYS_201_PREFIX` con una ruta absoluta
- **THEN** se considera una violación de esta especificación

### Requirement: Instalación documentada por plataforma

El repositorio SHALL documentar la obtención de LLVM 20.1 con bibliotecas estáticas para macOS, Linux y Windows.

#### Scenario: Advertencia sobre el instalador de Windows
- **WHEN** un desarrollador consulta la documentación de instalación para Windows
- **THEN** la documentación SHALL indicar explícitamente que el instalador `.exe` oficial de LLVM no incluye las bibliotecas estáticas requeridas
- **AND** SHALL dirigir al tarball oficial de desarrollo `clang+llvm-20.1.8-*-pc-windows-msvc.tar.xz`
