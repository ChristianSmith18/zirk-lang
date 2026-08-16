# File Modules

Declarations belong to their source file until explicitly published. Local module paths are quoted and omit `.zrk`:

```zirk
import { User } from "./domain/user";
```

Path resolution is project-aware and must be deterministic across supported filesystems. Do not rely on case-insensitive filenames or implicit directory globals.

---

**Previous:** [← Modules](README.md) · **Next:** [ share](02-share.md)
