# Permission Catalog

Capability families include scoped filesystem read/write, network addresses, process execution, environment access, native libraries, signals/system information, and separate compile-time filesystem/network/process access.

Applications grant runtime permissions; libraries declare requirements; decorators use `compile_permissions`. `unsafe` grants none. Secrets and tokens never belong in permission declarations, manifests, or lockfiles.

---

**Previous:** [← Target Matrix](09-target-matrix.md) · **Next:** [ Standard Library Index](11-standard-library-index.md)
