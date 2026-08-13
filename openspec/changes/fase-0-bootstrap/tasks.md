## 1. Esqueleto del workspace

- [x] 1.1 Crear `Cargo.toml` raíz como workspace con resolver 2 y dependencias compartidas en `[workspace.dependencies]`
- [x] 1.2 Crear `rust-toolchain.toml` fijando la versión de Rust y los componentes `rustfmt` y `clippy`
- [x] 1.3 Crear los nueve crates en `crates/`: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli`, `zirk-runtime`
- [x] 1.4 Documentar en el `lib.rs` de cada crate su responsabilidad y su límite, según D1 del design
- [x] 1.5 Declarar las dependencias entre crates respetando la dirección única del pipeline
- [x] 1.6 Verificar que `cargo build` y `cargo clippy` pasan limpios sobre el workspace completo

## 2. Diagnósticos

- [x] 2.1 Definir en `zirk-diagnostics` los tipos de severidad, código estable y ubicación en source
- [x] 2.2 Definir la estructura de diagnóstico con causa y ayuda opcional
- [x] 2.3 Implementar el renderizado humano del formato de `COMPILER_SPEC` §8, con fragmento de source y marcador de columna
- [x] 2.4 Implementar el renderizado cuando el source no está disponible
- [x] 2.5 Implementar la salida estructurada para herramientas
- [x] 2.6 Implementar la elevación de warnings a errores
- [x] 2.7 Tests de snapshot del formato renderizado, con y sin fragmento de source

## 3. Integración con LLVM

- [x] 3.1 Añadir `inkwell` 0.10 con feature `llvm20-1` y sin features por defecto a `zirk-codegen-llvm`
- [x] 3.2 Escribir el build script que verifica la versión mayor de LLVM y falla con un mensaje que referencia `docs/TOOLCHAIN.md` (D3)
- [x] 3.3 Verificar que el build script falla con mensaje claro cuando falta `LLVM_SYS_201_PREFIX`
- [x] 3.4 Portar el sanity check desde el spike: construir módulo, verificar, emitir objeto, enlazar y ejecutar
- [x] 3.5 Convertir el sanity check en test permanente del crate, no en binario de ejemplo (D2)

## 4. Matriz de targets

- [x] 4.1 Definir la tabla de los nueve targets del spec con su triple LLVM correspondiente
- [x] 4.2 Implementar la emisión de objetos por target
- [x] 4.3 Test que emite un objeto para cada uno de los nueve targets y valida formato de contenedor y arquitectura
- [x] 4.4 Test que verifica el fallo con diagnóstico ante un triple no reconocido

## 5. Runtime

- [x] 5.1 Configurar `zirk-runtime` como `staticlib` en su `Cargo.toml`
- [x] 5.2 Definir `zirk_rt_init` y `zirk_rt_shutdown` como `extern "C"` sin mangling, con cuerpo vacío (D-Open Questions)
- [x] 5.3 Test que verifica que el artefacto producido es una biblioteca estática enlazable
- [x] 5.4 Test que verifica que los símbolos exportados aparecen sin mangling

## 6. Integración continua

- [x] 6.1 Crear `.github/workflows/ci.yml` con la matriz `{linux, macos, windows}`
- [x] 6.2 Instalar LLVM 20.1 por plataforma: `apt.llvm.org` en Linux, `brew install llvm@20` en macOS, tarball oficial de desarrollo en Windows
- [x] 6.3 Definir `LLVM_SYS_201_PREFIX` por plataforma en el workflow, nunca versionado en el repo (D4)
- [x] 6.4 Cachear la instalación de LLVM y las dependencias de Cargo (D5)
- [x] 6.5 Ejecutar `cargo build`, `cargo test`, `cargo clippy` y `cargo fmt --check` en cada job
- [x] 6.6 Extender la matriz a aarch64 en Linux y macOS
- [x] 6.7 Resolver si `aarch64-windows` entra en la matriz según disponibilidad de runners
- [x] 6.8 Confirmar que el job de Windows construye `llvm-sys` correctamente — riesgo principal de esta change

## 7. Cierre

- [x] 7.1 Verificar que CI pasa en verde en las tres plataformas; hasta entonces la portabilidad se considera no verificada
- [x] 7.2 Actualizar `docs/decisions/README.md` marcando resueltos los pendientes de layout de workspace y formato de diagnósticos
- [x] 7.3 Registrar en ADR-004 el resultado real de la verificación de portabilidad
- [x] 7.4 Actualizar `docs/TOOLCHAIN.md` con cualquier corrección que surja de montar CI
