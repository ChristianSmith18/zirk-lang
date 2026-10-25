# `.zkinit`

`.zkinit` is Zirk's typed, declarative project manifest. It describes project
identity and source selection, package dependencies, application globals, and
the capabilities the code may need. It does not execute as startup code and it
does not store user consent.

```text
project {
    name: "acme.api";
    version: "1.2.0";
    type: application;
    authors: ["Acme Engineering"];
    description: "HTTP API for Acme services.";
    license: "MIT";
    entry: "src/main.zrk";
}

source "src";

dependencies {
    acme.http: "^2.1";
    acme.shared: { from: "path:../shared"; }
}

dev_dependencies {
    acme.testkit: "^0.8";
}

globals {
    request_count: UInt64;
    settings: {
        type: Settings;
        from: "src/config.zrk";
        readonly: true;
    }
}

requires {
    network.connect;
}
```

`name`, `version`, and `type` are required. Optional project metadata includes
`authors` (a list), `description`, and `license`. Applications select an
`entry` file; packages select an importable `root`. Their defaults are
`src/main.zrk` and `src/package.zrk`. Repeated `source "path";` declarations
set project-relative source roots and default to `src`. Only applications may
declare `build_targets`.

Dependencies are keyed by their logical import name. A version string uses the
configured default registry; an expanded `{ from: ... }` selects a path, named
registry, Git source, or vendored source. `dev_dependencies` are available only
to the current project's development/test graph. The lockfile pins each
resolved source and package root. Source imports use the logical name, such as
`import { Client } from acme.http;`, regardless of where the package came
from.

Only an application may declare `globals`. A key may use a type shorthand or
an object with `type`, optional `default`, optional custom-type `from`, and
optional `readonly`. Readonly defaults to `false`; a readonly value remains
readable and only blocks `Global.set`.

`requires` is the one finite capability declaration block shared by
applications and packages. It contains capability names, not environment or
build/runtime phases. A package request propagates through the locked
dependency tree with its requester and path. A declaration never grants
access: exact user consent is stored separately in the current user's Zirk
security directory and checked when the protected operation is reached.

`zirk init` asks for project type, identity, entry/root, optional metadata,
source roots, and application targets, showing defaults as it goes. Enter
accepts a default and blank optional answers are omitted. `zirk init -y` skips
prompts and writes the canonical application defaults (`0.1.0`,
`src/main.zrk`, and `src`). Neither form creates source files or overwrites an
existing manifest.

The CLI and language server validate the manifest before executing user code.
See [Applications](03-applications.md), [Libraries](04-libraries.md),
[Globals](06-globals.md), [Dependencies](11-dependencies.md), and
[Permission approval and audit](14-permission-approval-and-audit.md).

---

**Previous:** [← Project Layout](01-project-layout.md) · **Next:** [Applications](03-applications.md)
