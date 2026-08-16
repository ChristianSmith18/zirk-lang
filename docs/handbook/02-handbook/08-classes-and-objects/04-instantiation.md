# Instantiation

Call the class name to construct an instance:

```zirk
mut user = User(1, "Cristian");
mut named_user = User(name: "Cristian", id: 1);
```

Named arguments may appear in a different order because their labels select the
matching constructor parameters. They are especially useful when a class has
multiple constructors; the selected signature must still be unambiguous.

Arguments are checked against the chosen `construct`. This is invalid:

```zirk
mut user = new User(1, "Cristian");
```

The compiler should explain that Zirk construction has no `new`. Allocation strategy remains internal; class-call syntax does not promise heap allocation.

---

**Previous:** [← Constructors](03-constructors.md) · **Next:** [ this](05-this.md)
