# Libraries

A library publishes typed API and portable IR. It cannot declare application globals or grant itself final permissions. Instead it declares `requires` for build effects used by decorators; the consuming application grants them. Public decorator contracts include targets, configuration and ordering dependencies, while applications are erased after expansion.

Distributed `.zpkg` files also carry manifest, documentation, and license material.

---

**Previous:** [← Applications](03-applications.md) · **Next:** [ Entry Point](05-entry-point.md)
