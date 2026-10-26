#!/usr/bin/env python3
"""Generate deterministic type-algebra parser/checker/LSP stress fixtures."""
from pathlib import Path
import argparse

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=Path)
out = parser.parse_args().directory
out.mkdir(parents=True, exist_ok=True)

def write(name: str, lines: list[str]) -> None:
    (out / name).write_text("\n".join(lines + ["fn main(): Void {}", ""]))


write("flat-128.zrk", [f"type T{i} = Int32 | String | Boolean | Null;" for i in range(128)])
write(
    "nested-48.zrk",
    ["type Nested0 = Int32;", *[f"type Nested{i} = Array<Nested{i-1}>;" for i in range(1, 49)]],
)

intersection_members = [f"{{ field_{i}: Int32 }}" for i in range(64)]
write(
    "flat-intersection-64.zrk",
    [*([f"type Shape{i} = {member};" for i, member in enumerate(intersection_members)]),
     "type Combined = " + " & ".join(f"Shape{i}" for i in range(64)),
     "fn read(value: Combined): Int32 { return value.field_0; }"],
)

nested_algebra = ["type Algebra0 = Int32 | String;"]
for i in range(1, 49):
    nested_algebra.append(
        f"type Algebra{i} = (Algebra{i-1} | Boolean) & {{ depth_{i}: Int32 }};"
    )
write("nested-algebra-48.zrk", nested_algebra)

conditional = [
    "type Select<T> = T extends String ? String : Boolean;",
    "type Input = String | Int32 | Boolean;",
]
conditional.extend(f"type Result{i} = Select<Input>;" for i in range(128))
conditional.append("fn consume_conditional(value: Result0): Void {}")
write("conditional-128.zrk", conditional)

template = [
    "type Left = " + " | ".join(f'\"left-{i}\"' for i in range(8)) + ";",
    "type Right = " + " | ".join(f'\"right-{i}\"' for i in range(8)) + ";",
    'type Product = "{Left}:{Right}";',
    "fn consume_template(value: Product): Void {}",
]
write("template-product-64.zrk", template)

write(
    "recursive-object-alias.zrk",
    [
        "type Node = { value: Int32; next: Node? };",
        "type Left = { right: Right? };",
        "type Right = { left: Left? };",
        "fn read(node: Node): Int32 { return node.value; }",
    ],
)

shapes = [
    f"type Candidate{i} = {{ tag: const({i}); field_{i}: Int32 }};"
    for i in range(32)
]
shapes.extend(
    [
        "type CandidateSet = " + " | ".join(f"Candidate{i}" for i in range(32)) + ";",
        "fn select(): CandidateSet { return { tag: 17, field_17: 17 }; }",
    ]
)
write("contextual-shapes-32.zrk", shapes)
