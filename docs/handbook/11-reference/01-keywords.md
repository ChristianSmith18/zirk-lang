# Keywords

Core declarations and control words include `fn`, `gen`, `yield`, `class`,
`interface`, `trait`, `record`, `enum`, `type`, `construct`, `share`, `import`,
`use`, `mut`, `inmut`, `if`, `else`, `match`, `with`, `for`, `in`,
`while`, `do`, `loop`, `break`, `continue`, `return`, `try`, `catch`, `finally`,
`throw`, `throws`, `unsafe`, `commit`, `task`, `await`, `select`, `parallel`, `thread`, `abstract`,
`implements`, `extends`, `override`, `from`, `as`, `is`, `public`, `private`,
and `protected`. Manifest control includes
`requires`, `permissions`, and `during`. `scope`, `shield`, `after`,
`cancelled`, and `default` are contextual words in the applicable task/select
forms rather than general reserved values.

`strict` and `value` are contextual identifiers, not reserved keywords. They
acquire special meaning only in `inmut::strict` and `value class`; ordinary
bindings such as `mut strict = true;` and `mut value = 1;` remain valid.

Reserved literals/types include `true`, `false`, `null`, `Void`, `Never`, `Null`, and `Object`. The formal lexer remains authoritative; historical words excluded from 1.x—such as `async fn`, `worker`, `comptime`, and general `defer`—are not usable features.

---

**Previous:** [← Reference](README.md) · **Next:** [ Operators and Precedence](02-operators-and-precedence.md)
