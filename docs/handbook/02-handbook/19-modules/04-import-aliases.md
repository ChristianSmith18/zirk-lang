# Import Aliases

Use `Original -> Alias` when two imports conflict or a local domain name is clearer.

```zirk
import { User, Role -> DomainRole } from "./domain/user";
```

The alias changes only the local binding, not the exported API. Avoid aliases that conceal which module owns a security- or resource-sensitive operation.

---

**Previous:** [← `import`](./03-import.md) · **Next:** [`use` and Globals →](./05-use-globals.md)
