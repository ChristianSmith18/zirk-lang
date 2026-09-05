# Decorator Targets

Zirk 1.x supports exactly `class`, `attribute`, `function`, `method`, and `parameter` target blocks. Parameter includes initialization parameters. Method permits inspection of bodyless abstract, interface, and trait signatures, but wrapping requires a body.

There are no module, interface, trait, record, enum, enum-case, property, accessor, or constructor targets. `@Module`, `@Controller`, `@Entity`, and `@Component` are framework roles implemented as class decorators. Records expose stored data through attribute targets. Construction injection uses parameter targets or a generated typed factory.

One decorator may declare several supported target blocks. Each receives only operations valid for its category; applying it elsewhere is a focused compile-time error.

---

**Previous:** [← fn dec](02-fn-dec.md) · **Next:** [ Syntax API](04-syntax-api.md)
