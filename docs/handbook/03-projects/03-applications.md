# Applications

An application owns the executable entry point, may declare globals, and grants the final finite runtime permission set.

```zirk
project { name: "app"; version: "0.1.0"; type: application; entry: "src/main.zrk"; }
```

Application policy must satisfy library requirements without granting capabilities silently.

---

**Previous:** [← .zkinit](02-zkinit.md) · **Next:** [ Libraries](04-libraries.md)
