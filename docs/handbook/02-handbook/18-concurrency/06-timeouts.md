# Timeouts

> **Estado:** disponible para `Concurrent.of(...).first()` y
> `Concurrent.of(...).settled()`. `Concurrent.each` y
> `Concurrent.each_settled` están disponibles en el backend nativo, pero no
> tienen una variante `.within(duration)`.

`.within(duration)` limita una pipeline de `Concurrent.of` antes de su terminal.
Al vencer el plazo, el runtime cancela los branches, espera su cleanup y propaga
`TimeoutError`. La cancelación del caller se mantiene como cancelación, no se
convierte en timeout. Los deadlines negativos se rechazan antes de iniciar los
branches. El cleanup no puede revertir un efecto externo que un servicio remoto
ya haya observado; eso depende del protocolo subyacente.

---

**Previous:** [← Cancellation](05-cancellation.md) · **Next:** [ Channels](07-channels.md)
