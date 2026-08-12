# Zirk — Toolchain de desarrollo

Requisitos para **construir el compilador de Zirk** desde fuente, en las tres plataformas soportadas.

> Esto es distinto de los targets que Zirk **produce**. Ver [ADR-004](./decisions/ADR-004-portabilidad.md).

## Componentes

| Componente | Versión | Motivo |
|---|---|---|
| Rust | 1.94+ (edición estable) | lenguaje del compilador |
| LLVM | **20.1.x** (pin exacto de versión mayor) | backend de codegen |
| lld | 20.1.x | linker cross-platform (ELF / Mach-O / COFF) |
| inkwell | 0.10 con feature `llvm20-1` | bindings seguros a LLVM |

El pin de LLVM **no es negociable por conveniencia local**: `llvm-sys` enlaza contra la ABI de C++ de una versión mayor concreta. Una versión distinta no compila. Ver [ADR-001](./decisions/ADR-001-pin-llvm.md).

## Variable de entorno obligatoria

`llvm-sys` localiza LLVM mediante `llvm-config` en el `PATH` o, preferentemente, mediante:

```
LLVM_SYS_201_PREFIX=<prefijo de la instalación de LLVM 20.1>
```

Se usa la variable y no el `PATH` porque en la mayoría de las instalaciones LLVM 20 convive con otras versiones y no queda enlazado globalmente.

---

## macOS

```sh
brew install llvm@20 lld@20
```

`llvm@20` es *keg-only* (no se enlaza en `/opt/homebrew/bin`), así que la variable es obligatoria:

```sh
# Apple Silicon
export LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20

# Intel
export LLVM_SYS_201_PREFIX=/usr/local/opt/llvm@20
```

Requiere las Command Line Tools de Xcode para los SDK del sistema.

## Linux (Debian / Ubuntu)

El repositorio oficial `apt.llvm.org` publica las bibliotecas estáticas en `llvm-20-dev`:

```sh
wget https://apt.llvm.org/llvm.sh
chmod +x llvm.sh
sudo ./llvm.sh 20
sudo apt-get install -y llvm-20-dev libpolly-20-dev lld-20

export LLVM_SYS_201_PREFIX=/usr/lib/llvm-20
```

En otras distros, el paquete necesario es el que contiene `llvm-config` **y** los `.a` (habitualmente `llvm-devel` o `llvm-static`), no solo los binarios de clang.

## Windows

> **No uses `LLVM-20.1.8-win64.exe`.** El instalador oficial (367 MB) trae los binarios de clang pero **no** las bibliotecas estáticas ni las cabeceras que `llvm-sys` necesita para enlazar. Es la causa más común de fallos de build en Windows.

Descargar el tarball de desarrollo desde las releases oficiales de LLVM:

```
https://github.com/llvm/llvm-project/releases/tag/llvmorg-20.1.8

  clang+llvm-20.1.8-x86_64-pc-windows-msvc.tar.xz    (x86_64)
  clang+llvm-20.1.8-aarch64-pc-windows-msvc.tar.xz   (aarch64)
```

Ambos vienen con firma `.sig` — verificarla antes de extraer.

Extraer, por ejemplo, en `C:\LLVM` y registrar la variable:

```powershell
setx LLVM_SYS_201_PREFIX C:\LLVM
```

Requiere **Visual Studio 2022** con las herramientas de C++ (o Build Tools equivalentes), porque `llvm-sys` enlaza contra la runtime de MSVC.

---

## Verificación

```sh
rustc --version                 # 1.94+
$LLVM_SYS_201_PREFIX/bin/llvm-config --version   # 20.1.x
```

La comprobación real es la suite del workspace, que incluye el sanity check de LLVM (genera IR, emite objeto, enlaza y ejecuta un binario nativo):

```sh
cargo test --workspace
```

Si el pin de LLVM está mal, el build falla antes de compilar, con un diagnóstico que nombra la versión encontrada y la esperada. No hace falta interpretar errores de enlace.

## Notas de plataforma

- **Windows:** el prefijo se declara con barras normales (`C:/LLVM`, no `C:\LLVM`). Evita problemas de escape al pasar por shells tipo bash, y las APIs de Windows aceptan ambos separadores.
- **macOS:** `llvm@20` y `lld@20` son formulae separadas desde LLVM 20; instalar solo `llvm@20` deja el sistema sin `lld`.
- **Linux:** el paquete necesario es `llvm-20-dev`, no `llvm-20`. El primero incluye las bibliotecas estáticas; el segundo solo los binarios.

La instalación por plataforma está automatizada en `.github/workflows/ci.yml`, que es la referencia ejecutable de este documento.
