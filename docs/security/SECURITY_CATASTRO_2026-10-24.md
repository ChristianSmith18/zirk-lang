# Catastro de seguridad de Zirk — 2026-10-24

## Alcance y método

Auditoría estática del árbol de trabajo en `develop` (`2b651778`), incluida la diferencia local de `crates/zirk-cli/src/approvals.rs` y `crates/zirk-cli/src/project.rs`. El árbol ya tenía esos dos archivos modificados y `demo/34-express-like/build/` sin seguimiento; no se alteraron.

Se trazaron las rutas de ejecución de fuentes, permisos, HTTP/compresión, DNS/TLS, parsers y dependencias frente a las alertas de PHP, Python, Node.js, Go y Rust enumeradas en la conversación. También se ejecutó `cargo audit --json`: base RustSec actualizada el 24-10-2026, 235 dependencias, cero advisories aplicables.

No se ejecutaron tests, fuzzing, pruebas dinámicas de protocolo ni una reproducción del hallazgo. Las conclusiones de código son revisión estática; el resultado de `cargo audit` solo cubre advisories conocidos para las dependencias que reconoce.

## Resultado ejecutivo

- **1 hallazgo alto** en el cambio local de búsqueda de permisos para `zirk run`: una fuente externa puede recibir el token de permisos de un proyecto aprobado mediante una ruta con `..`.
- **No encontré evidencia de las RCE críticas de PHP ni de una CVE equivalente de Brotli en las dependencias de Zirk.** Zirk sí decodifica Brotli en rutas que reciben datos de red, pero usa los crates Rust `brotli` 9.0.0 / `brotli-decompressor` 6.0.1, una implementación distinta del decodificador C de Google afectado por CVE-2026-111497. El lockfile no presenta advisory RustSec aplicable.
- No se confirmó una RCE crítica de Node.js, Go o Rust aplicable al código o lockfile auditado. La extensión de VS Code hereda el runtime Node del host; ese runtime instalado no forma parte del lockfile de Zirk y no se evaluó.

## Hallazgo ZIRK-SEC-001 — fuente fuera del proyecto ejecutada con sus permisos aprobados

**Severidad: Alta, condicional.** No se asigna CVSS. El impacto depende de que el proyecto tenga scopes sensibles aprobados y de que se ejecute una fuente externa con una ruta que atraviese el proyecto.

**Código:** `crates/zirk-cli/src/approvals.rs:417-431`, flujo de ejecución en `crates/zirk-cli/src/main.rs:239-273`, carga independiente en `crates/zirk-cli/src/modules.rs:117-142`.

**Evidencia estática:**

1. `project_root_for_execution_path` asciende `parent.ancestors()` y busca `.zkinit` usando la ruta recibida, sin normalizar ni canonicalizar la fuente ni exigir que su destino permanezca bajo la raíz del proyecto.
2. Para un argumento de archivo, `zirk run` compila como fuente independiente. `load_with_overlay` normaliza la ruta y establece como raíz de carga el directorio padre de la fuente ya normalizada.
3. Después de compilar, `main` llama `runtime_token_for_path` con la ruta original y entrega el token obtenido al ejecutable hijo.
4. El runtime acepta los permisos únicamente si el token firmado es válido; el token no queda vinculado a la ruta canónica del código compilado.

**Ejemplo de ruta que debe rechazarse:** desde la raíz de un proyecto aprobado que contiene `src/`, ejecutar `zirk run src/../../outside.zrk`, si `outside.zrk` existe fuera del proyecto. La carga normaliza la ruta y trata el archivo como script independiente; la búsqueda de permisos aún puede encontrar el `.zkinit` del proyecto al recorrer el ancestro léxico `src/..`. El programa externo recibe los scopes que el usuario aprobó para el proyecto.

**Impacto:** una fuente ajena al proyecto puede usar los permisos de entorno y red incluidos en la aprobación —por ejemplo, leer secretos autorizados y comunicarse con un origen permitido— cuando el usuario ejecuta esa ruta. Esto es una fuga del límite de autorización del proyecto; no equivale por sí solo a ejecución remota de código ni otorga permisos del sistema operativo fuera de las capacidades Zirk concedidas.

**Corrección recomendada:** normalizar y canonicalizar la fuente antes de buscar el proyecto; resolver la raíz aprobada canónicamente y conceder el token solo si la fuente ejecutada está contenida en esa raíz. Aplicar la misma regla a enlaces simbólicos y rutas con `..`. Añadir casos de regresión para `..`, enlace simbólico al archivo y directorio simbólico, incluyendo proyectos con scopes de red y secretos.

## Comparación con las alertas de la semana

| Área | Resultado en Zirk |
|---|---|
| PHP — CVE-2026-109618, CVE-2026-109619 y CVE-2026-115560 | PHP no forma parte del runtime ni de las dependencias revisadas. No se encontró un equivalente de la gestión de memoria/estado por solicitud de PHP. |
| Python — CVE-2026-111497 en paquetes Brotli | No hay runtime Python de la aplicación; los `.py` del repositorio son scripts de mantenimiento/prueba. El paquete Brotli del lockfile de Rust es otro artefacto. |
| Node.js — CVE-2026-111497 y avisos de DNS, TLS, HTTP y permisos | La extensión contiene JavaScript para el host de VS Code, pero no se encontró uso de `zlib`/Brotli en ella. El runtime Node del editor no está fijado por este repositorio. No se halló en código Zirk una copia del decodificador Node ni del agente HTTP de Node. |
| Go — parsers ASN.1/XML, TLS, HTTP/2 y checksums | No hay runtime Go en el proyecto. Las rutas HTTP y TLS de Zirk usan Hyper/H2 y Rustls; el mecanismo de verificación de checksum DB de Go no existe en el resolver revisado. La adquisición de registry/Git está separada del cargador y se declara fuera de ese flujo. |
| Rust — Wasmtime/Cranelift, Tokio, raíces webpki, zip y criptografía | Wasmtime/Cranelift, `webpki-roots`, `webpki-root-certs`, `zip`, `crypto-bigint` y `ctutils` no aparecen en `Cargo.lock`. Tokio está fijado en 1.53.3: la advisory de carrera afecta 1.54.0; la de `AsyncFd` declara corregidas las versiones desde 1.53.3. `cargo audit` no reportó advisories. |

## Brotli y límites de decodificación

El cliente HTTP anuncia `br` por defecto (`crates/zirk-runtime/src/http_runtime.rs:1630-1632`) y `decode_response_body` usa `BrotliDecoder` para respuestas con `Content-Encoding: br` (`:600-656`). El servidor también decodifica cuerpos de solicitud con `Content-Encoding` (`:6385-6432`). Por tanto, el decodificador procesa bytes que pueden venir de un servidor remoto o de un cliente remoto, según el rol del programa.

El código limita el cuerpo comprimido y expandido a 16 MiB, la razón de expansión a 1000, la cadena de codificaciones a cuatro y el tiempo contabilizado dentro del decodificador a dos segundos; la salida viaja por una cola acotada de dos elementos (`http_runtime.rs:450-454, 642-703`; el límite de 16 MiB está en `crates/zirk-runtime-core/src/http_values.rs:7-9`). Son defensas frente a consumo de recursos, pero no demuestran por sí solas ausencia de corrupción de memoria en una implementación de compresión.

El advisory CVE-2026-111497 describe el decodificador C de Google Brotli. El lockfile de Zirk resuelve el crate Rust `brotli` 9.0.0 y `brotli-decompressor` 6.0.1; sus metadatos describen una implementación Rust y no la biblioteca C de Google/PyPI. No hay un advisory RustSec para esas dependencias en la base consultada. **Conclusión:** superficie de entrada Brotli expuesta, pero no encontré evidencia de que Zirk tenga la misma vulnerabilidad. Mantener esta conclusión limitada a las versiones y fuentes inspeccionadas.

## Controles relacionados observados

- Las respuestas comprimidas se procesan en un worker, con límites de tamaño, razón de expansión, profundidad, CPU y cola acotada.
- Las solicitudes HTTP del servidor limitan el cuerpo a 16 MiB; HTTP/1 limita encabezados a 256 y usa timeout de lectura de encabezados.
- La resolución DNS acota resultados y vuelve a validar cada dirección contra el permiso de origen; los nombres deben resolver a direcciones públicas y las IP literales deben coincidir exactamente con la aprobada (`network_transport.rs:95-132`; `zirk-runtime-core/src/network_policy.rs:307-335`).
- La identidad TLS se forma desde el host parseado por `Url` y se pasa a `rustls::ServerName`, distinguiendo IP de nombre DNS (`network_transport.rs:900-920`). Esto reduce la similitud con los casos de normalización de hostname específicos de Node.js, aunque no sustituye pruebas de interoperabilidad.
- La carga de imports del modo proyecto verifica contención canónica para archivos importados (`modules.rs:446-489`). El hallazgo ZIRK-SEC-001 aparece en el flujo de script independiente y en la concesión del token, antes de que esa protección de imports sea suficiente para fijar la autoridad al proyecto.

## Referencias de las alertas comparadas

- [PHP: avisos y versiones corregidas](https://www.php.net/archive/2026.php)
- [Node.js: boletín de seguridad del 21 de octubre](https://nodejs.org/en/blog/vulnerability/october-2026-security-releases)
- [GitHub Advisory: Brotli en Node.js/Python](https://github.com/advisories/GHSA-9v8h-qqgm-4m35)
- [Ubuntu USN-8935-1: Go](https://ubuntu.com/security/notices/USN-8935-1)
- [RustSec: avisos](https://rustsec.org/advisories/)
