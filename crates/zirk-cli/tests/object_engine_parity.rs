//! Native/interpreter parity checks for the unified dynamic-object model.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_CASE: AtomicUsize = AtomicUsize::new(0);

fn interpreter_runner() -> PathBuf {
    if let Some(path) = std::env::var_os("ZIRK_REPRO") {
        return PathBuf::from(path);
    }
    let test_executable = std::env::current_exe().expect("test executable path");
    let profile = test_executable
        .parent()
        .and_then(Path::parent)
        .expect("Cargo test executable is under <profile>/deps");
    let runner = profile.join("examples").join(if cfg!(windows) {
        "interp_repro.exe"
    } else {
        "interp_repro"
    });
    assert!(
        runner.is_file(),
        "missing interpreter harness at {}; build it with `cargo build -p zirk-browser --example interp_repro` or set ZIRK_REPRO",
        runner.display()
    );
    runner
}

fn run_same_program(name: &str, program: &str) -> (Output, Output) {
    let case = NEXT_CASE.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "object_engine_parity_{}_{}_{}",
        std::process::id(),
        case,
        name
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated case directory");
    let source = dir.join(format!("{name}.zrk"));
    std::fs::write(&source, program).expect("write source fixture");
    let native = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg(&source)
        .current_dir(&dir)
        .output()
        .expect("run native compiler");
    let interpreted = Command::new(interpreter_runner())
        .arg(&source)
        .output()
        .expect("run interpreter harness");
    (native, interpreted)
}

fn normalize(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn interpreted_payload(output: &Output) -> String {
    normalize(&output.stdout)
        .lines()
        .skip(1)
        .filter(|line| !line.starts_with("[completion: "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_parity(name: &str, program: &str, expected: &str) {
    let (native, interpreted) = run_same_program(name, program);
    assert!(
        native.status.success(),
        "native `{name}` failed with status {:?}:\n{}\n{}",
        native.status,
        normalize(&native.stdout),
        normalize(&native.stderr)
    );
    assert!(
        interpreted.status.success(),
        "interpreter `{name}` failed with status {:?}:\n{}\n{}",
        interpreted.status,
        normalize(&interpreted.stdout),
        normalize(&interpreted.stderr)
    );
    assert_eq!(
        normalize(&native.stdout),
        expected,
        "native `{name}` output"
    );
    let mut payload = interpreted_payload(&interpreted);
    if !payload.is_empty() {
        payload.push('\n');
    }
    assert_eq!(payload, expected, "interpreter `{name}` output");
}

#[test]
fn generic_contract_method_specialization_matches_native_and_interpreter() {
    assert_parity(
        "generic_contract_method_specialization",
        r#"import { stdout } from std.io;
type Person = { age: Int32; name: String; };
contract Watched<T> {
    on<K from String & keyof T>(event: "{K}Changed", callback: Fn(T[K]) => Void): Void;
}

class PersonWatcher implements Watched<Person> {
    on<K from String & keyof Person>(event: "{K}Changed", callback: Fn(Person[K]) => Void): Void {
        stdout.println("called");
    }
}

fn attach(person: Watched<Person>): Void {
    person.on("ageChanged", (age): Void => { });
    person.on("nameChanged", (name): Void => { });
}
fn main(): Void {
    mut watcher: Watched<Person> = PersonWatcher();
    attach(watcher);
}
main();"#,
        "called\ncalled\n",
    );
}

#[test]
fn generic_trait_defaults_substitute_contract_types_for_each_adopter() {
    assert_parity(
        "generic_trait_default_substitution",
        r#"import { stdout } from std.io;
trait DefaultContainer<T from Clone> {
    value: T;
    echo(input: T): T { return input; }
    echo_many(input: List<T>): List<T> { return input; }
    get(): T { return this.value; }
    duplicate(): T { return this.value.clone(); }
}
class IntBox implements DefaultContainer<Int32> {
    value: Int32;
    construct(value: Int32) { this.value = value; }
}
class StringBox implements DefaultContainer<String> {
    value: String;
    construct(value: String) { this.value = value; }
}
fn main(): Void {
    mut i: IntBox = IntBox(7);
    mut s: StringBox = StringBox("ok");
    mut ci: DefaultContainer<Int32> = i;
    mut cs: DefaultContainer<String> = s;
    inmut numbers: List<Int32> = [9, 10];
    inmut words: List<String> = ["uno", "dos"];
    stdout.println(i.echo(42));
    stdout.println(i.get());
    stdout.println(s.echo("text"));
    stdout.println(s.get());
    stdout.println(i.duplicate());
    stdout.println(s.duplicate());
    stdout.println(i.echo_many(numbers)[0]);
    stdout.println(s.echo_many(words)[1]);
    stdout.println(ci.echo(84));
    stdout.println(cs.get());
}
main();"#,
        "42\n7\ntext\nok\n7\nok\n9\ndos\n84\nok\n",
    );
}

#[test]
fn nested_generic_contract_arguments_substitute_and_dispatch_recursively() {
    assert_parity(
        "nested_generic_contract_arguments",
        r#"import { stdout } from std.io;
enum Box<T> { Full(value: T) }
contract Container<T> {
    item: T;
    get(): T;
}
contract NestedContainer<T> implements Container<Box<T>> {}
class IntContainer implements NestedContainer<Int32> {
    item: Box<Int32>;
    construct(item: Box<Int32>) { this.item = item; }
    get(): Box<Int32> { return this.item; }
}
fn main(): Void {
    mut value: IntContainer = IntContainer(Box.Full(42));
    mut container: Container<Box<Int32>> = value;
    stdout.println(match container.get() {
        Box.Full(number) => number;
    });
}
main();"#,
        "42\n",
    );
}

#[test]
fn generic_enum_match_dispatches_after_recursive_substitution() {
    assert_parity(
        "generic_enum_match_recursive_substitution",
        r#"import { stdout } from std.io;
enum Box<T> { Full(value: T), Empty }
fn rebox<T>(box: Box<T>): Box<T> {
    return match box {
        Box.Full(value) => Box.Full(value);
        Box.Empty => Box.Empty;
    };
}
fn describe_int(box: Box<Int32>): String {
    return match box {
        Box.Full(value) => "full:" + value.to_string();
        Box.Empty => "empty";
    };
}
fn describe_string(box: Box<String>): String {
    return match box {
        Box.Full(value) => "full:" + value;
        Box.Empty => "empty";
    };
}
fn main(): Void {
    inmut number: Box<Int32> = Box.Full(42);
    inmut text: Box<String> = Box.Full("hola");
    inmut nested: Box<Box<String>> = Box.Full(Box.Full("nested"));
    inmut empty: Box<String> = Box.Empty;
    stdout.println(describe_int(rebox(number)));
    stdout.println(describe_string(rebox(text)));
    stdout.println(match rebox(nested) {
        Box.Full(inner) => describe_string(inner);
        Box.Empty => "empty";
    });
    stdout.println(describe_string(rebox(empty)));
}
main();"#,
        "full:42\nfull:hola\nfull:nested\nempty\n",
    );
}

#[test]
fn datetime_collection_keys_match_native_and_interpreter() {
    assert_parity(
        "datetime_collection_keys",
        r#"import { stdout } from std.io;
fn main(): Void {
    inmut first = DateTime(Date(2026, 10, 27), Time(9, 30));
    inmut equivalent = DateTime(Date(2026, 10, 27), Time(9, 30));
    mut map: Map<DateTime, Int32> = Map();
    map.set(first, 42);
    stdout.println(map.contains_key(equivalent).to_string());
    stdout.println((map.get(equivalent) ?? 0).to_string());
    mut set: Set<DateTime> = Set();
    stdout.println(set.add(first).to_string());
    stdout.println(set.add(equivalent).to_string());
    stdout.println(set.length.to_string());
}
main();"#,
        "true\n42\ntrue\nfalse\n1\n",
    );
}

#[test]
fn decimal_collection_keys_match_native_and_interpreter() {
    assert_parity(
        "decimal_collection_keys",
        r#"import { stdout } from std.io;
fn main(): Void {
    mut map: Map<Decimal, Int32> = Map();
    map.set(1.0, 7);
    stdout.println(map.contains_key(1.00).to_string());
    stdout.println((map.get(1.00) ?? 0).to_string());
    mut set: Set<Decimal> = Set();
    stdout.println(set.add(1.0).to_string());
    stdout.println(set.add(1.00).to_string());
    stdout.println(set.length.to_string());
}
main();"#,
        "true\n7\ntrue\nfalse\n1\n",
    );
}

#[test]
fn decimal_collection_keys_match_at_coefficient_and_scale_boundaries() {
    assert_parity(
        "decimal_collection_boundaries",
        r#"import { stdout } from std.io;
fn main(): Void {
    inmut upper = match Decimal.parse("99999999999999999999999999999999999999") {
        Ok(value) => value;
        Error(_) => Decimal(0);
    };
    inmut upper_scaled = match Decimal.parse("99999999999999999999999999999999999999") {
        Ok(value) => value;
        Error(_) => Decimal(0);
    };
    inmut tiny = match Decimal.parse("0.0000000000000000000000000000000000001") {
        Ok(value) => value;
        Error(_) => Decimal(0);
    };
    inmut tiny_scaled = match Decimal.parse("0.00000000000000000000000000000000000010") {
        Ok(value) => value;
        Error(_) => Decimal(0);
    };
    mut values: Set<Decimal> = Set();
    stdout.println(values.add(upper).to_string());
    stdout.println(values.add(upper_scaled).to_string());
    stdout.println(values.add(tiny).to_string());
    stdout.println(values.add(tiny_scaled).to_string());
    stdout.println(values.length.to_string());
    mut map: Map<Decimal, String> = Map();
    map.set(tiny, "smallest-scale-boundary");
    stdout.println((map.get(tiny_scaled) ?? "missing"));
    mut decimal_values: Map<Decimal, Decimal> = Map();
    decimal_values.set(upper, tiny);
    stdout.println((decimal_values.get(upper_scaled) ?? Decimal(0)).to_string());
}
main();"#,
        "true\nfalse\ntrue\nfalse\n2\nsmallest-scale-boundary\n0.0000000000000000000000000000000000001\n",
    );
}

#[test]
fn nullable_decimal_collection_keys_match_native_and_interpreter() {
    assert_parity(
        "nullable_decimal_collection_keys",
        r#"import { stdout } from std.io;
fn main(): Void {
    inmut absent: Decimal? = null;
    inmut first: Decimal? = 1.0;
    inmut equivalent: Decimal? = 1.00;
    mut values: Set<Decimal?> = Set();
    stdout.println(values.add(absent).to_string());
    stdout.println(values.add(absent).to_string());
    stdout.println(values.add(first).to_string());
    stdout.println(values.add(equivalent).to_string());
    stdout.println(values.length.to_string());
    mut map: Map<Decimal?, String> = Map();
    map.set(absent, "none");
    map.set(first, "some");
    stdout.println((map.get(absent) ?? "missing"));
    stdout.println((map.get(equivalent) ?? "missing"));
    stdout.println(map.length.to_string());
    mut tuple_values: Set<Tuple<Decimal?, Boolean>> = Set();
    stdout.println(tuple_values.add((absent, false)).to_string());
    stdout.println(tuple_values.add((absent, false)).to_string());
    stdout.println(tuple_values.add((first, false)).to_string());
    stdout.println(tuple_values.add((equivalent, false)).to_string());
    stdout.println(tuple_values.length.to_string());
    mut tuple_map: Map<Tuple<Decimal?, Boolean>, Int32> = Map();
    tuple_map.set((first, true), 42);
    stdout.println((tuple_map.get((equivalent, true)) ?? 0).to_string());
}
main();"#,
        "true\nfalse\ntrue\nfalse\n2\nnone\nsome\n2\ntrue\nfalse\ntrue\nfalse\n2\n42\n",
    );
}

#[test]
fn generated_decimal_arithmetic_values_preserve_collection_hash_coherence() {
    let mut program = String::from(
        "import { stdout } from std.io;\nfn main(): Void {\n\
         mut values: Set<Decimal> = Set();\n\
         mut entries: Map<Decimal, Int32> = Map();\n\
         inmut quarter: Decimal = Decimal.parse(\"0.25\") ?? Decimal(0);\n",
    );
    let mut expected = String::new();
    for value in 0..8_i32 {
        let whole = value * 7919 + 1;
        program.push_str(&format!(
            "inmut first_{value}: Decimal = Decimal.parse(\"{whole}.00\") ?? Decimal(0);\n\
             inmut equal_{value}: Decimal = Decimal.parse(\"{whole}.0\") ?? Decimal(0);\n\
             inmut arithmetic_{value}: Decimal = first_{value} + quarter;\n\
             inmut arithmetic_equal_{value}: Decimal = Decimal.parse(\"{whole}.25\") ?? Decimal(0);\n\
             stdout.println(values.add(first_{value}).to_string());\n\
             stdout.println(values.add(equal_{value}).to_string());\n\
             stdout.println(values.add(arithmetic_{value}).to_string());\n\
             stdout.println(values.add(arithmetic_equal_{value}).to_string());\n\
             entries.set(arithmetic_{value}, {value});\n\
             stdout.println((entries.get(arithmetic_equal_{value}) ?? -1).to_string());\n"
        ));
        expected.push_str(&format!("true\nfalse\ntrue\nfalse\n{value}\n"));
    }
    program.push_str("}\nmain();\n");
    assert_parity(
        "generated_decimal_collection_hash_coherence",
        &program,
        &expected,
    );
}

#[test]
fn canonically_equivalent_text_collection_keys_match_native_and_interpreter() {
    assert_parity(
        "canonical_text_collection_keys",
        r#"import { stdout } from std.io;
fn main(): Void {
    mut map: Map<String, Int32> = Map();
    map.set("é", 17);
    stdout.println(map.contains_key("é").to_string());
    stdout.println((map.get("é") ?? 0).to_string());
    mut set: Set<String> = Set();
    stdout.println(set.add("é").to_string());
    stdout.println(set.add("é").to_string());
    stdout.println(set.length.to_string());
    stdout.println(set.remove("é").to_string());
    stdout.println(set.add("é").to_string());
    stdout.println(set.length.to_string());
    mut chars: Set<Char> = Set();
    stdout.println(chars.add('é').to_string());
    stdout.println(chars.add('é').to_string());
    stdout.println(chars.remove('é').to_string());
    stdout.println(chars.add('é').to_string());
    stdout.println(chars.length.to_string());
}
main();"#,
        "true\n17\ntrue\nfalse\n1\ntrue\ntrue\n1\ntrue\nfalse\ntrue\ntrue\n1\n",
    );
}

#[test]
fn nullable_and_heterogeneous_tuple_collection_keys_match_native_and_interpreter() {
    assert_parity(
        "nullable_and_heterogeneous_tuple_collection_keys",
        r#"import { stdout } from std.io;
enum ScalarKey { Count(value: Int128), Ratio(value: Float64?), Empty }
fn main(): Void {
    mut nullable: Set<Int32?> = Set();
    stdout.println(nullable.add(null).to_string());
    stdout.println(nullable.add(null).to_string());
    stdout.println(nullable.add(7).to_string());
    stdout.println(nullable.length.to_string());
    mut tuples: Map<Tuple<Int32, Boolean>, Int32> = Map();
    tuples.set((1, true), 42);
    stdout.println(tuples.contains_key((1, true)).to_string());
    stdout.println((tuples.get((1, true)) ?? 0).to_string());
    mut tuple_set: Set<Tuple<Int32, Boolean>> = Set();
    stdout.println(tuple_set.add((1, true)).to_string());
    stdout.println(tuple_set.add((1, true)).to_string());
    stdout.println(tuple_set.length.to_string());
    mut float_tuples: Set<Tuple<Float64, Boolean>> = Set();
    stdout.println(float_tuples.add((0.0 as Float64, false)).to_string());
    stdout.println(float_tuples.add((-0.0 as Float64, false)).to_string());
    stdout.println(float_tuples.length.to_string());
    mut nullable_tuples: Set<Tuple<Int32?, Boolean>> = Set();
    inmut no_integer: Int32? = null;
    inmut one_integer: Int32? = 1;
    stdout.println(nullable_tuples.add((no_integer, false)).to_string());
    stdout.println(nullable_tuples.add((no_integer, false)).to_string());
    stdout.println(nullable_tuples.add((one_integer, false)).to_string());
    stdout.println(nullable_tuples.length.to_string());
    mut nullable_text: Set<String?> = Set();
    stdout.println(nullable_text.add(null).to_string());
    stdout.println(nullable_text.add(null).to_string());
    stdout.println(nullable_text.add("key").to_string());
    stdout.println(nullable_text.add("key").to_string());
    for value in nullable_text { stdout.println((value ?? "nil")); }
    stdout.println(nullable_text.remove(null).to_string());
    stdout.println(nullable_text.remove(null).to_string());
    stdout.println(nullable_text.remove("key").to_string());
    stdout.println(nullable_text.length.to_string());
    mut nullable_text_map: Map<String?, Int32> = Map();
    nullable_text_map.set(null, 5);
    nullable_text_map.set("key", 6);
    stdout.println(nullable_text_map.contains_key(null).to_string());
    stdout.println((nullable_text_map.get("key") ?? 0).to_string());
    for key in nullable_text_map.keys() { stdout.println((key ?? "nil")); }
    for entry in nullable_text_map.entries() { stdout.println(entry.to_string()); }
    mut enum_keys: Set<ScalarKey> = Set();
    stdout.println(enum_keys.add(ScalarKey.Count(1)).to_string());
    stdout.println(enum_keys.add(ScalarKey.Count(1)).to_string());
    stdout.println(enum_keys.add(ScalarKey.Ratio(0.0 as Float64)).to_string());
    stdout.println(enum_keys.add(ScalarKey.Ratio(-0.0 as Float64)).to_string());
    stdout.println(enum_keys.add(ScalarKey.Empty).to_string());
    stdout.println(enum_keys.add(ScalarKey.Empty).to_string());
    stdout.println(enum_keys.length.to_string());
    mut enum_map: Map<ScalarKey, Int32> = Map();
    enum_map.set(ScalarKey.Ratio(0.0 as Float64), 23);
    stdout.println((enum_map.get(ScalarKey.Ratio(-0.0 as Float64)) ?? 0).to_string());
}

main();"#,
        "true\nfalse\ntrue\n2\ntrue\n42\ntrue\nfalse\n1\ntrue\nfalse\n1\ntrue\nfalse\ntrue\n2\ntrue\nfalse\ntrue\nfalse\nnil\nkey\ntrue\nfalse\ntrue\n0\ntrue\n6\nnil\nkey\n(<Null>, 5)\n(key, 6)\ntrue\nfalse\ntrue\nfalse\ntrue\nfalse\n3\n23\n",
    );
}

#[test]
fn enum_text_collection_keys_use_variant_aware_content_snapshots() {
    assert_parity(
        "enum_text_collection_keys",
        r#"import { stdout } from std.io;
enum TextKey { Name(value: String), Character(value: Char), Empty }
fn main(): Void {
    mut source: String = "é";
    mut names: Set<TextKey> = Set();
    stdout.println(names.add(TextKey.Name(source)).to_string());
    source[0] = "x";
    stdout.println(names.contains(TextKey.Name("é")).to_string());
    stdout.println(names.contains(TextKey.Name("x")).to_string());
    stdout.println(names.add(TextKey.Name("é")).to_string());
    stdout.println(names.add(TextKey.Character('é')).to_string());
    stdout.println(names.add(TextKey.Character('é')).to_string());
    stdout.println(names.add(TextKey.Empty).to_string());
    stdout.println(names.add(TextKey.Empty).to_string());
    mut values: Map<TextKey, Int32> = Map();
    values.set(TextKey.Name("é"), 17);
    values.set(TextKey.Character('é'), 23);
    stdout.println((values.get(TextKey.Name("é")) ?? 0).to_string());
    stdout.println((values.get(TextKey.Character('é')) ?? 0).to_string());
    stdout.println(values.length.to_string());
}
main();"#,
        "true\ntrue\nfalse\nfalse\ntrue\nfalse\ntrue\nfalse\n17\n23\n2\n",
    );
}

#[test]
fn aggregate_set_removal_uses_declared_equality_and_preserves_order() {
    assert_parity(
        "aggregate_set_removal_order",
        r#"import { stdout } from std.io;
enum Key { Count(value: Int128), Empty }
enum NestedKey { Value(value: Tuple<Decimal?, Boolean>), Empty }
fn main(): Void {
    mut tuples: Set<Tuple<Int32, Boolean>> = Set();
    tuples.add((1, true));
    tuples.add((2, false));
    stdout.println(tuples.remove((1, true)).to_string());
    stdout.println(tuples.remove((1, true)).to_string());
    tuples.add((1, true));
    for key in tuples { stdout.println(key[0].to_string()); }
    mut nullable: Set<Tuple<Int32?, Boolean>> = Set();
    inmut absent: Int32? = null;
    inmut present: Int32? = 7;
    nullable.add((absent, false));
    nullable.add((present, false));
    stdout.println(nullable.remove((absent, false)).to_string());
    for key in nullable { stdout.println((key[0] ?? 0).to_string()); }
    mut enums: Set<Key> = Set();
    enums.add(Key.Count(10));
    enums.add(Key.Empty);
    stdout.println(enums.remove(Key.Count(10)).to_string());
    enums.add(Key.Count(10));
    for key in enums {
        match key {
            Key.Count(value) => stdout.println(value.to_string());
            Key.Empty => stdout.println("empty");
        }
    }
    inmut decimal_absent: Decimal? = null;
    inmut other_absent: Decimal? = null;
    inmut decimal_present: Decimal? = 1.00;
    inmut decimal_equivalent: Decimal? = 1.0;
    mut nested: Set<NestedKey> = Set();
    nested.add(NestedKey.Value((decimal_absent, false)));
    nested.add(NestedKey.Empty);
    stdout.println(nested.remove(NestedKey.Value((other_absent, false))).to_string());
    stdout.println(nested.remove(NestedKey.Value((decimal_absent, false))).to_string());
    nested.add(NestedKey.Value((decimal_present, true)));
    nested.add(NestedKey.Value((decimal_equivalent, true)));
    stdout.println(nested.length.to_string());
    for key in nested {
        match key {
            NestedKey.Value(_) => stdout.println("value");
            NestedKey.Empty => stdout.println("empty");
        }
    }
}
main();"#,
        "true\nfalse\n2\n1\ntrue\n7\ntrue\nempty\n10\ntrue\nfalse\n2\nempty\nvalue\n",
    );
}

#[test]
fn scalar_collection_hash_matrix_matches_native_and_interpreter() {
    let cases = [
        ("Int8", "1 as Int8"),
        ("Int16", "1 as Int16"),
        ("Int32", "1 as Int32"),
        ("Int64", "1 as Int64"),
        ("Int128", "1 as Int128"),
        ("UInt8", "1 as UInt8"),
        ("UInt16", "1 as UInt16"),
        ("UInt32", "1 as UInt32"),
        ("UInt64", "1 as UInt64"),
        ("UInt128", "1 as UInt128"),
        ("Float16", "1.5 as Float16"),
        ("Float32", "1.5 as Float32"),
        ("Float64", "1.5 as Float64"),
        ("Decimal", "1.5"),
        ("Boolean", "true"),
        ("Char", "'x'"),
        ("String", "\"key\""),
        ("Duration", "1s"),
        ("Date", "Date(2026, 10, 27)"),
        ("Time", "Time(9, 30)"),
        ("DateTime", "DateTime(Date(2026, 10, 27), Time(9, 30))"),
    ];
    let mut program = String::from("import { stdout } from std.io;\nfn main(): Void {\n");
    let mut expected = String::new();
    for (index, (ty, key)) in cases.iter().enumerate() {
        program.push_str(&format!(
            "    mut map_{index}: Map<{ty}, Int32> = Map();\n    map_{index}.set({key}, 17);\
             \n    stdout.println(map_{index}.contains_key({key}).to_string());\
             \n    stdout.println((map_{index}.get({key}) ?? 0).to_string());\
             \n    mut set_{index}: Set<{ty}> = Set();\
             \n    stdout.println(set_{index}.add({key}).to_string());\
             \n    stdout.println(set_{index}.add({key}).to_string());\
             \n    stdout.println(set_{index}.length.to_string());\n"
        ));
        expected.push_str("true\n17\ntrue\nfalse\n1\n");
        if *ty != "Decimal" {
            program.push_str(&format!(
                "    mut value_map_{index}: Map<Int32, {ty}> = Map();\n\
                 value_map_{index}.set({index}, {key});\n\
                 stdout.println(((value_map_{index}.get({index}) ?? {key}) == {key}).to_string());\n"
            ));
            expected.push_str("true\n");
        }
    }
    program.push_str("}\nmain();\n");
    assert_parity("scalar_collection_hash_matrix", &program, &expected);
}

#[test]
fn float128_collection_keys_match_native_and_interpreter() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    mut key: Float128 = 1.5 as Float128;
    mut map: Map<Float128, Int32> = Map();
    map.set(key, 17);
    stdout.println(map.contains_key(key).to_string());
    stdout.println((map.get(key) ?? 0).to_string());
    mut set: Set<Float128> = Set();
    stdout.println(set.add(key).to_string());
    stdout.println(set.add(key).to_string());
    stdout.println(set.length.to_string());
}
main();"#;
    assert_parity(
        "float128_collection_keys",
        program,
        "true\n17\ntrue\nfalse\n1\n",
    );
}

#[test]
fn ordered_set_operations_and_add_results_match_native_and_interpreter() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    mut left: Set<Int32> = Set();
    mut right: Set<Int32> = Set();
    stdout.println(left.add(3));
    stdout.println(left.add(1));
    stdout.println(left.add(3));
    left.add(4);
    right.add(1);
    right.add(2);
    right.add(4);
    stdout.println(left.union(right).to_string());
    stdout.println(left.intersection(right).to_string());
    stdout.println(left.difference(right).to_string());
    stdout.println(left.symmetric_difference(right).to_string());
    stdout.println(left.is_subset_of(right));
    stdout.println(left.is_superset_of(right));
    stdout.println(left.is_disjoint(right));
    stdout.println(left.to_string());
    stdout.println(right.to_string());
    for value in left { stdout.println(value.to_string()); }
}
main();"#;
    assert_parity(
        "ordered_set_operations",
        program,
        "true\ntrue\nfalse\n{1, 2, 3, 4}\n{1, 4}\n{3}\n{2, 3}\nfalse\nfalse\nfalse\n{1, 3, 4}\n{1, 2, 4}\n3\n1\n4\n",
    );
}

#[test]
fn map_key_and_value_snapshots_preserve_insertion_order_in_both_engines() {
    assert_parity(
        "map_typed_snapshots",
        include_str!("corpus/valid/map_typed_snapshots.zrk"),
        include_str!("corpus/valid/map_typed_snapshots.out"),
    );
}

#[test]
fn mutable_text_collection_keys_and_elements_use_insertion_snapshots() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    mut key: String = "one";
    mut map: Map<String, Int32> = Map();
    mut set: Set<String> = Set();
    map.set(key, 7);
    set.add(key);

    key[0] = 'd';
    stdout.println(map.contains_key("one").to_string());
    stdout.println((map.get("one") ?? 0).to_string());
    stdout.println(map.contains_key(key).to_string());
    stdout.println(set.contains("one").to_string());
    stdout.println(set.contains(key).to_string());

    mut map_keys: Array<String> = map.keys();
    mut exposed_map_key: String = map_keys[0];
    exposed_map_key[0] = 'x';
    stdout.println(map.contains_key("one").to_string());
    stdout.println(set.contains("one").to_string());
}
main();"#;
    assert_parity(
        "mutable_text_collection_snapshots",
        program,
        "true\n7\nfalse\ntrue\nfalse\ntrue\ntrue\n",
    );
}

#[test]
fn typed_collection_rendering_preserves_narrow_and_character_layouts() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    mut values: Map<Int32, String> = Map();
    values.set(10, "ten");
    values.set(2, "two");
    values.set(1, "one");
    stdout.println(values.to_string());

    mut letters: Set<Char> = Set();
    letters.add('z');
    letters.add('a');
    stdout.println(letters.to_string());
}
main();"#;
    assert_parity(
        "typed_collection_rendering",
        program,
        "{1: one, 2: two, 10: ten}\n{a, z}\n",
    );
}

#[test]
fn aggregate_nullable_and_float128_map_values_match_both_engines() {
    assert_parity(
        "map_aggregate_values",
        include_str!("corpus/valid/map_aggregate_values.zrk"),
        include_str!("corpus/valid/map_aggregate_values.out"),
    );
}

#[test]
fn collection_failure_types_are_catchable_in_both_engines() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    try {
        throw AllocationError("collection growth failed");
    } catch AllocationError(error) {
        stdout.println(error.message());
    }
    try {
        throw EmptyCollectionError("collection is empty");
    } catch EmptyCollectionError(error) {
        stdout.println(error.message());
    }
    try {
        throw EmptyCollectionError("collection is empty as a RuntimeError");
    } catch RuntimeError(error) {
        stdout.println(error.message());
    }
    inmut outcome: Result<Int32, EmptyCollectionError> = Result.Error(EmptyCollectionError("typed empty outcome"));
    match outcome {
        Result.Ok(_) => stdout.println("unexpected value");
        Result.Error(error) => stdout.println(error.message());
    }
}

main();"#;
    assert_parity(
        "collection_failure_types",
        program,
        "collection growth failed\ncollection is empty\ncollection is empty as a RuntimeError\ntyped empty outcome\n",
    );
}

#[test]
fn deque_constructor_has_interpreter_and_native_parity() {
    assert_parity(
        "deque_constructor",
        r#"import { stdout } from std.io;
fn main(): Void {
    mut deque: Deque<Int32> = Deque();
    deque.push_back(1);
    deque.push_front(0);
    deque.push_back(2);
    stdout.println(deque.length.to_string());
    stdout.println(deque.to_string());
    for value in deque { stdout.println(value.to_string()); }
    match deque.pop_front() {
        Result.Ok(value) => stdout.println(value.to_string());
        Result.Error(_) => stdout.println("empty");
    }
    match deque.pop_back() {
        Result.Ok(value) => stdout.println(value.to_string());
        Result.Error(_) => stdout.println("empty");
    }
    deque.clear();
    stdout.println(deque.is_empty.to_string());
    match deque.front() {
        Result.Ok(_) => stdout.println("unexpected");
        Result.Error(_) => stdout.println("empty");
    }
    mut changing: Deque<Int32> = Deque();
    changing.push_back(1);
    try {
        for value in changing { changing.push_back(value); }
    } catch IteratorInvalidatedError(_) {
        stdout.println("invalidated");
    }
}
main();"#,
        "3\n[0, 1, 2]\n0\n1\n2\n0\n2\ntrue\nempty\ninvalidated\n",
    );
}

#[test]
fn queue_and_stack_preserve_fifo_and_lifo_in_both_engines() {
    assert_parity(
        "queue_stack_order",
        r#"import { stdout } from std.io;
fn main(): Void {
    mut queue: Queue<Int32> = Queue();
    queue.enqueue(1);
    queue.enqueue(2);
    match queue.dequeue() {
        Result.Ok(value) => stdout.println(value.to_string());
        Result.Error(_) => stdout.println("empty");
    }
    match queue.peek() {
        Result.Ok(value) => stdout.println(value.to_string());
        Result.Error(_) => stdout.println("empty");
    }
    mut stack: Stack<Int32> = Stack();
    stack.push(1);
    stack.push(2);
    match stack.peek() {
        Result.Ok(value) => stdout.println(value.to_string());
        Result.Error(_) => stdout.println("empty");
    }
    stack.clear();
}

main();"#,
        "1\n2\n2\n",
    );
}

#[test]
fn priority_queue_is_minimum_first_and_iterable_in_both_engines() {
    assert_parity(
        "priority_queue_order",
        r#"import { stdout } from std.io;
fn main(): Void {
    mut values: PriorityQueue<Int32> = PriorityQueue();
    values.enqueue(3);
    values.enqueue(1);
    values.enqueue(2);
    stdout.println(values.length.to_string());
    for value in values { stdout.println(value.to_string()); }
    match values.dequeue() {
        Result.Ok(value) => stdout.println(value.to_string());
        Result.Error(_) => stdout.println("empty");
    }
    stdout.println(values.is_empty.to_string());
}

main();"#,
        "3\n1\n2\n3\n1\nfalse\n",
    );
}

#[test]
fn sequence_families_preserve_reference_and_composite_elements() {
    assert_parity(
        "sequence_family_layouts",
        r#"import { stdout } from std.io;
fn main(): Void {
    mut deque: Deque<String> = Deque();
    deque.push_back("deque-value");
    match deque.pop_front() {
        Result.Ok(value) => stdout.println(value);
        Result.Error(_) => stdout.println("empty");
    }
    mut composite_deque: Deque<Tuple<Int32, String>> = Deque();
    composite_deque.push_back((6, "deque-composite"));
    match composite_deque.pop_front() {
        Result.Ok(value) => stdout.println((value == (6, "deque-composite")).to_string());
        Result.Error(_) => stdout.println("empty");
    }
    mut queue: Queue<Tuple<Int32, String>> = Queue();
    queue.enqueue((7, "queue-value"));
    match queue.dequeue() {
        Result.Ok(value) => stdout.println((value == (7, "queue-value")).to_string());
        Result.Error(_) => stdout.println("empty");
    }
    mut stack: Stack<Tuple<Int32, String>> = Stack();
    stack.push((8, "stack-value"));
    match stack.pop() {
        Result.Ok(value) => stdout.println((value == (8, "stack-value")).to_string());
        Result.Error(_) => stdout.println("empty");
    }
    mut priorities: PriorityQueue<String> = PriorityQueue();
    priorities.enqueue("zulu");
    priorities.enqueue("alpha");
    match priorities.dequeue() {
        Result.Ok(value) => stdout.println(value);
        Result.Error(_) => stdout.println("empty");
    }
}
main();"#,
        "deque-value\ntrue\ntrue\ntrue\nalpha\n",
    );
}

#[test]
fn map_iteration_is_invalidated_by_structural_mutation_only() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    mut values: Map<Int32, Int32> = Map();
    values.set(1, 10);
    values.set(2, 20);
    for entry in values {
        values.set(1, 11);
    }
    stdout.println("replacement completed");
    try {
        for entry in values {
            values.set(3, 30);
        }
    } catch IteratorInvalidatedError(error) {
        stdout.println("insertion invalidated");
    }
    stdout.println(values.length.to_string());
    try {
        for entry in values {
            values.remove(2);
        }
    } catch IteratorInvalidatedError(error) {
        stdout.println("removal invalidated");
    }
    stdout.println(values.length.to_string());
}
main();"#;
    assert_parity(
        "map_iteration_invalidation",
        program,
        "replacement completed\ninsertion invalidated\n3\nremoval invalidated\n2\n",
    );
}

#[test]
fn mutable_reference_collection_keys_use_identity_in_both_engines() {
    assert_parity(
        "reference_identity_collection_keys",
        include_str!("corpus/valid/reference_identity_collection_keys.zrk"),
        include_str!("corpus/valid/reference_identity_collection_keys.out"),
    );
}

#[test]
fn contract_reference_set_keys_keep_underlying_object_identity() {
    assert_parity(
        "contract_reference_set_identity",
        r#"import { stdout } from std.io;
contract HasValue { value: Int32; }
class MutableValue implements HasValue {
    mut value: Int32;
    construct(value: Int32) { this.value = value; }
}
fn main(): Void {
    mut concrete: MutableValue = MutableValue(7);
    mut first_view: HasValue = concrete;
    mut second_view: HasValue = concrete;
    mut distinct: HasValue = MutableValue(7);
    mut values: Set<HasValue> = Set();
    stdout.println(values.add(first_view).to_string());
    stdout.println(values.add(second_view).to_string());
    stdout.println(values.add(distinct).to_string());
    concrete.value = 9;
    stdout.println(values.contains(second_view).to_string());
    stdout.println(values.remove(second_view).to_string());
    stdout.println(values.remove(first_view).to_string());
    stdout.println(values.length.to_string());
    for value in values { stdout.println(value.value.to_string()); }
    inmut absent: HasValue? = null;
    inmut present: HasValue? = first_view;
    mut nullable: Set<HasValue?> = Set();
    stdout.println(nullable.add(absent).to_string());
    stdout.println(nullable.add(absent).to_string());
    stdout.println(nullable.add(present).to_string());
    stdout.println(nullable.contains(present).to_string());
    stdout.println(nullable.remove(absent).to_string());
    stdout.println(nullable.remove(absent).to_string());
    stdout.println(nullable.remove(present).to_string());
    stdout.println(nullable.length.to_string());
}
main();"#,
        "true\nfalse\ntrue\ntrue\ntrue\nfalse\n1\n7\ntrue\nfalse\ntrue\ntrue\ntrue\nfalse\ntrue\n0\n",
    );
}

#[test]
fn float128_arithmetic_comparison_and_format_match_native_and_interpreter() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    mut precise: Float128 = 123456789012345678901234567890;
    mut negative_integer: Int128 = -123456789012345678901234567890;
    mut negative: Float128 = negative_integer as Float128;
    mut unsigned_integer: UInt128 = 123456789012345678901234567890;
    mut unsigned: Float128 = unsigned_integer as Float128;
    mut fraction: Float128 = 1.23456;
    stdout.println(precise);
    stdout.println(negative);
    stdout.println(unsigned);
    stdout.println(fraction.format(".3"));
    stdout.println(fraction.format(".3e"));
    stdout.println((1.5 as Float128 + 0.25 as Float128).format(".2"));
    stdout.println(((1.5 as Float128) > (1.0 as Float128)).to_string());
    mut from64: Float128 = (0.1 as Float64) as Float128;
    stdout.println(from64);
    stdout.println((from64 as Float64).to_string());
    stdout.println(((42.75 as Float128) as Int32).to_string());
    stdout.println(((1.0 as Float128) / (0.0 as Float128)).to_string());
    try {
        inmut nan: Float128 = (0.0 as Float128) / (0.0 as Float128);
        stdout.println(nan);
    } catch FloatNanError(error) {
        stdout.println("FloatNanError:" + error.message());
    }
}

main();"#;
    assert_parity(
        "float128_arithmetic_format",
        program,
        "1.2345678901234567890123456789e29\n-1.2345678901234567890123456789e29\n1.2345678901234567890123456789e29\n1.235\n1.235e0\n1.75\ntrue\n0.100000000000000005551115123125782702\n0.1\n42\ninf\nFloatNanError:Float operation produced an indeterminate result (NaN)\n",
    );
}

#[test]
fn floating_collection_keys_normalize_signed_zero_in_both_engines() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    mut positive_half: Float16 = 0.0;
    mut negative_half: Float16 = -0.0;
    mut half_values: Set<Float16> = Set();
    stdout.println(half_values.add(positive_half).to_string());
    stdout.println(half_values.add(negative_half).to_string());
    stdout.println(half_values.length.to_string());
    mut half_map: Map<Float16, Float16> = Map();
    half_map.set(positive_half, negative_half);
    stdout.println((half_map.get(positive_half) ?? positive_half).to_string());

    mut positive_single: Float32 = 0.0;
    mut negative_single: Float32 = -0.0;
    mut single_values: Set<Float32> = Set();
    stdout.println(single_values.add(positive_single).to_string());
    stdout.println(single_values.add(negative_single).to_string());
    stdout.println(single_values.length.to_string());
    mut single_map: Map<Float32, Float32> = Map();
    single_map.set(positive_single, negative_single);
    stdout.println((single_map.get(positive_single) ?? positive_single).to_string());

    mut positive_zero: Float64 = 0.0;
    mut negative_zero: Float64 = -0.0;
    mut values: Set<Float64> = Set();
    stdout.println(values.add(positive_zero).to_string());
    stdout.println(values.add(negative_zero).to_string());
    stdout.println(values.length.to_string());
    stdout.println(values.contains(negative_zero).to_string());

    mut map: Map<Float64, Int32> = Map();
    map.set(positive_zero, 17);
    stdout.println((map.get(negative_zero) ?? 0).to_string());
    stdout.println(map.contains_key(negative_zero).to_string());

    mut float_values: Map<Float64, Float64> = Map();
    float_values.set(positive_zero, negative_zero);
    stdout.println((float_values.get(positive_zero) ?? 1.0).to_string());


    mut positive_quad: Float128 = 0.0 as Float128;
    mut negative_quad: Float128 = -0.0 as Float128;
    mut quad_values: Set<Float128> = Set();
    stdout.println(quad_values.add(positive_quad).to_string());
    stdout.println(quad_values.add(negative_quad).to_string());
    stdout.println(quad_values.length.to_string());
    mut quad_map: Map<Float128, Int128> = Map();
    inmut wide_value: Int128 = 123456789012345678901234567890;
    quad_map.set(positive_quad, wide_value);
    stdout.println((quad_map.get(negative_quad) ?? 0 as Int128).to_string());

    mut wide_keys: Set<Int128> = Set();
    inmut first_wide: Int128 = 123456789012345678901234567890;
    inmut second_wide: Int128 = 123456789012345678901234567891;
    stdout.println(wide_keys.add(first_wide).to_string());
    stdout.println(wide_keys.add(second_wide).to_string());
    stdout.println(wide_keys.add(first_wide).to_string());
    stdout.println(wide_keys.length.to_string());
    mut wide_map: Map<Int128, String> = Map();
    wide_map.set(second_wide, "wide-key");
    stdout.println((wide_map.get(second_wide) ?? "missing"));
}
main();"#;
    assert_parity(
        "floating_collection_keys_signed_zero",
        program,
        "true\nfalse\n1\n-0\ntrue\nfalse\n1\n-0\ntrue\nfalse\n1\ntrue\n17\ntrue\n-0\ntrue\nfalse\n1\n123456789012345678901234567890\ntrue\ntrue\nfalse\n2\nwide-key\n",
    );
}

#[test]
fn floating_collection_keys_distinguish_infinities_and_preserve_equal_lookups() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    inmut positive16: Float16 = Float16.parse("inf") ?? 0.0;
    inmut negative16: Float16 = Float16.parse("-inf") ?? 0.0;
    inmut positive64: Float64 = Float64.parse("inf") ?? 0.0;
    inmut negative64: Float64 = Float64.parse("-inf") ?? 0.0;
    inmut positive32: Float32 = Float32.parse("inf") ?? 0.0;
    inmut negative32: Float32 = Float32.parse("-inf") ?? 0.0;
    inmut positive128: Float128 = Float128.parse("inf") ?? 0.0;
    inmut negative128: Float128 = Float128.parse("-inf") ?? 0.0;

    mut values16: Set<Float16> = Set();
    stdout.println(values16.add(positive16).to_string());
    stdout.println(values16.add(negative16).to_string());
    stdout.println(values16.length.to_string());
    mut entries16: Map<Float16, Int32> = Map();
    entries16.set(positive16, 161);
    entries16.set(negative16, 162);
    stdout.println((entries16.get(positive16) ?? 0).to_string());
    stdout.println((entries16.get(negative16) ?? 0).to_string());

    mut values64: Set<Float64> = Set();
    stdout.println(values64.add(positive64).to_string());
    stdout.println(values64.add(positive64).to_string());
    stdout.println(values64.add(negative64).to_string());
    stdout.println(values64.length.to_string());
    stdout.println(values64.contains(positive64).to_string());
    stdout.println(values64.contains(negative64).to_string());
    mut entries64: Map<Float64, Int32> = Map();
    entries64.set(positive64, 41);
    entries64.set(negative64, 42);
    stdout.println((entries64.get(positive64) ?? 0).to_string());
    stdout.println((entries64.get(negative64) ?? 0).to_string());

    mut values32: Set<Float32> = Set();
    stdout.println(values32.add(positive32).to_string());
    stdout.println(values32.add(negative32).to_string());
    stdout.println(values32.length.to_string());
    mut entries32: Map<Float32, Int32> = Map();
    entries32.set(positive32, 31);
    entries32.set(negative32, 32);
    stdout.println((entries32.get(positive32) ?? 0).to_string());
    stdout.println((entries32.get(negative32) ?? 0).to_string());

    mut values128: Set<Float128> = Set();
    stdout.println(values128.add(positive128).to_string());
    stdout.println(values128.add(negative128).to_string());
    stdout.println(values128.length.to_string());
    mut entries128: Map<Float128, Int32> = Map();
    entries128.set(positive128, 1281);
    entries128.set(negative128, 1282);
    stdout.println((entries128.get(positive128) ?? 0).to_string());
    stdout.println((entries128.get(negative128) ?? 0).to_string());
}
main();"#;
    assert_parity(
        "floating_collection_keys_infinities",
        program,
        "true\ntrue\n2\n161\n162\ntrue\nfalse\ntrue\n2\ntrue\ntrue\n41\n42\ntrue\ntrue\n2\n31\n32\ntrue\ntrue\n2\n1281\n1282\n",
    );
}

#[test]
fn repeated_scalar_tuple_keys_use_recursive_value_equality() {
    let program = r#"import { stdout } from std.io;
enum Token {
    Number(value: Int32),
    Error(code: Int32),
    Missing
}
fn main(): Void {
    mut points: Set<Tuple<Int32, Int32>> = Set();
    stdout.println(points.add((1, 2)).to_string());
    stdout.println(points.add((1, 2)).to_string());
    stdout.println(points.contains((1, 2)).to_string());
    stdout.println(points.contains((2, 1)).to_string());
    mut labels: Map<Tuple<Int64, Int64>, String> = Map();
    labels.set((4 as Int64, 9 as Int64), "point");
    stdout.println((labels.get((4 as Int64, 9 as Int64)) ?? "missing"));
    stdout.println(labels.contains_key((9 as Int64, 4 as Int64)).to_string());
    mut nested: Set<Tuple<Tuple<Int32, Int32>, Tuple<Int32, Int32>>> = Set();
    stdout.println(nested.add(((1, 2), (3, 4))).to_string());
    stdout.println(nested.add(((1, 2), (3, 4))).to_string());
    mut tokens: Set<Token> = Set();
    stdout.println(tokens.add(Token.Number(7)).to_string());
    stdout.println(tokens.add(Token.Number(7)).to_string());
    stdout.println(tokens.add(Token.Error(7)).to_string());
    stdout.println(tokens.contains(Token.Error(7)).to_string());
}
main();"#;
    assert_parity(
        "repeated_scalar_tuple_keys",
        program,
        "true\nfalse\ntrue\nfalse\npoint\nfalse\ntrue\nfalse\ntrue\nfalse\ntrue\ntrue\n",
    );
}

#[test]
fn dynamic_object_patterns_and_rest_match_across_engines() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    inmut profile: Object = { id: 7, name: "Ada", active: true };
    inmut { id, ... details } = profile;
    stdout.println(id as Int32);
    stdout.println(details.name as String);
    stdout.println(details.active as Boolean);
    inmut { id: profile_id, name: profile_name, ... flags } = profile;
    stdout.println(profile_id as Int32);
    stdout.println(profile_name as String);
    stdout.println(flags.active as Boolean);
}
main();

"#;
    assert_parity("object_rest", program, "7\nAda\ntrue\n7\nAda\ntrue\n");
}

#[test]
fn scalar_union_matching_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
type Value = Int32 | String;
fn identity(value: Value): Value { return value; }
fn apply(transform: Fn(Value) => Value, value: Value): Value { return transform(value); }
fn choose(flag: Boolean): Value {
    if flag { return "ready"; }
    return 42;
}
fn checked(flag: Boolean): Value throws InvalidArgumentError {
    if flag { return "ready"; }
    throw InvalidArgumentError("missing value");
}
fn classify(value: Value): String {
    return match value { 42 => "number"; "ready" => "text"; _ => "other"; };
}
fn main(): Void {
    stdout.println(classify(identity(42)));
    stdout.println(classify(identity("ready")));
    stdout.println(classify(apply(identity, 42)));
    stdout.println(classify(apply(identity, "ready")));
    stdout.println(classify(choose(true)));
    stdout.println(classify(choose(false)));
    try {
        stdout.println(classify(checked(false)));
    } catch InvalidArgumentError(_) {
        stdout.println("caught");
    } finally {
        stdout.println("cleanup");
    }
}
main();
"#;
    assert_parity(
        "scalar_union",
        program,
        "number\ntext\nnumber\ntext\ntext\nnumber\ncaught\ncleanup\n",
    );
}

#[test]
fn class_reference_union_narrowing_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
class Dog {
    name: String;
    construct(name: String) { this.name = name; }
}
class Cat {
    name: String;
    construct(name: String) { this.name = name; }
}
type Animal = Dog | Cat;
fn describe(animal: Animal): String {
    if animal is Dog { return "dog " + animal.name; }
    if animal is Cat { return "cat " + animal.name; }
    return "unknown";
}
fn main(): Void {
    stdout.println(describe(Dog("Rex")));
    stdout.println(describe(Cat("Miso")));
}
main();
"#;
    assert_parity("class_reference_union", program, "dog Rex\ncat Miso\n");
}

#[test]
fn payload_enum_union_narrowing_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
enum Message { Success(value: String), Failure(code: Int32), Empty }
type Value = Message | Int32;
fn describe(value: Value): String {
    return match value {
        7 => "number";
        Message.Success(text) => "success " + text;
        Message.Failure(code) => "failure " + code.to_string();
        Message.Empty => "empty";
        _ => "other";
    };
}
fn category(value: Value): String {
    if value is Message { return "message"; }
    return "integer";
}
fn main(): Void {
    stdout.println(describe(7));
    stdout.println(describe(Message.Success("ready")));
    stdout.println(describe(Message.Failure(23)));
    stdout.println(describe(Message.Empty));
    stdout.println(category(Message.Success("tagged")));
    stdout.println(category(7));
}
main();
"#;
    assert_parity(
        "payload_enum_union",
        program,
        "number\nsuccess ready\nfailure 23\nempty\nmessage\ninteger\n",
    );
}

#[test]
fn generic_list_union_payload_dispatch_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
enum Message { Text(value: String), Count(value: Int32) }
type Value = Message | Boolean;
fn describe(value: Value): String {
    return match value {
        Message.Text(text) => "text " + text;
        Message.Count(count) => "count " + count.to_string();
        true => "true";
        false => "false";
        _ => "other";
    };
}
fn render_all(values: List<Value>): Void {
    for value in values { stdout.println(describe(value)); }
}
fn main(): Void {
    mut values: List<Value> = List();
    values.add(Message.Text("nested"));
    values.add(true);
    values.add(Message.Count(42));
    render_all(values);
    mut current: Value = Message.Count(1);
    current = true;
    current = Message.Text("assigned");
    stdout.println(describe(current));
}
main();
"#;
    assert_parity(
        "generic_list_union_payload",
        program,
        "text nested\ntrue\ncount 42\ntext assigned\n",
    );
}

#[test]
fn map_union_values_round_trip_and_match_across_engines() {
    let program = r#"import { stdout } from std.io;
enum StoredMessage { Text(value: String), Empty }
type Value = StoredMessage | Int32;
fn describe(value: Value?): String {
    return match value {
        null => "missing";
        7 => "number";
        StoredMessage.Text(text) => "text " + text;
        StoredMessage.Empty => "empty";
        _ => "other";
    };
}
fn main(): Void {
    mut values: Map<String, Value> = Map();
    values.set("text", StoredMessage.Text("stored"));
    values.set("number", 7);
    values.set("empty", StoredMessage.Empty);
    stdout.println(describe(values.get("text")));
    stdout.println(describe(values.get("number")));
    stdout.println(describe(values.get("empty")));
    stdout.println(describe(values.get("missing")));
}
main();
"#;
    assert_parity(
        "map_union_values",
        program,
        "text stored\nnumber\nempty\nmissing\n",
    );
}

#[test]
fn union_narrowing_survives_return_error_nested_match_and_finally() {
    let program = r#"import { stdout } from std.io;
type Payload = { kind: "text"; text: String };
type Value = Payload | Int32;
fn describe(value: Value): String {
    if value is Payload {
        return match value.kind {
            "text" => value.text;
        };
    }
    return "number";
}
fn fail(value: Value): String throws InvalidArgumentError {
    if value is Payload { throw InvalidArgumentError(value.text); }
    return "number";
}
fn main(): Void {
    inmut value: Value = { kind: "text", text: "payload stays live" };
    stdout.println(describe(value));
    try {
        stdout.println(fail(value));
    } catch InvalidArgumentError(error) {
        stdout.println(error.message());
    } finally {
        stdout.println(describe(value));
        stdout.println("cleanup");
    }
}
main();
"#;
    assert_parity(
        "union_narrowing_control_flow",
        program,
        "payload stays live\npayload stays live\npayload stays live\ncleanup\n",
    );
}

#[test]
fn collection_reference_union_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
type Value = Int32 | Array<Int32> | List<Int32> | Map<String, Int32> | Set<Int32> | Deque<Int32> | Queue<Int32> | Stack<Int32>;
type IntArray = Array<Int32>;
type IntList = List<Int32>;
type IntMap = Map<String, Int32>;
type IntSet = Set<Int32>;
type IntDeque = Deque<Int32>;
type IntQueue = Queue<Int32>;
type IntStack = Stack<Int32>;
fn describe(value: Value): String {
    if value is IntArray { return "array " + value.length.to_string(); }
    if value is IntList { return "list " + value.length.to_string(); }
    if value is IntMap { return "map " + (value.get("answer") ?? 0).to_string(); }
    if value is IntSet { return "set " + value.contains(10).to_string(); }
    if value is IntDeque { return "deque " + value.length.to_string(); }
    if value is IntQueue { return "queue " + value.length.to_string(); }
    if value is IntStack { return "stack " + value.length.to_string(); }
    return "number " + value.to_string();
}

fn make_value(kind: Int32): Value {
    if kind == 1 {
        mut values: List<Int32> = List();
        values.add(10);
        values.add(20);
        return values;
    }
    if kind == 2 {
        mut values: Map<String, Int32> = Map();
        values.set("answer", 42);
        return values;
    }
    if kind == 3 {
        mut values: Set<Int32> = Set();
        values.add(10);
        return values;
    }
    if kind == 4 {
        mut values: Array<Int32> = Array(1);
        values[0] = 30;
        return values;
    }
    if kind == 5 {
        mut values: Deque<Int32> = Deque();
        values.push_back(50);
        return values;
    }
    if kind == 6 {
        mut values: Queue<Int32> = Queue();
        values.enqueue(60);
        return values;
    }
    if kind == 7 {
        mut values: Stack<Int32> = Stack();
        values.push(70);
        return values;
    }
    return 7;
}
fn main(): Void {
    stdout.println(describe(make_value(0)));
    stdout.println(describe(make_value(1)));
    stdout.println(describe(make_value(2)));
    stdout.println(describe(make_value(3)));
    stdout.println(describe(make_value(4)));
    stdout.println(describe(make_value(5)));
    stdout.println(describe(make_value(6)));
    stdout.println(describe(make_value(7)));
}
main();
"#;
    assert_parity(
        "collection_reference_union",
        program,
        "number 7\nlist 2\nmap 42\nset true\narray 1\ndeque 1\nqueue 1\nstack 1\n",
    );
}

#[test]
fn union_elements_round_trip_through_ordered_sequences_in_both_engines() {
    assert_parity(
        "union_ordered_sequence_elements",
        r#"import { stdout } from std.io;
type Item = Int32 | String;
fn main(): Void {
    mut deque: Deque<Item> = Deque();
    deque.push_back(3);
    deque.push_back("d");
    for item in deque {
        match item {
            3 => stdout.println("3");
            "d" => stdout.println("d");
            _ => stdout.println("other");
        }
    }
    mut queue: Queue<Item> = Queue();
    queue.enqueue(4);
    queue.enqueue("q");
    match queue.dequeue() {
        Result.Ok(item) => {
            match item {
                4 => stdout.println("4");
                "q" => stdout.println("q");
                _ => stdout.println("other");
            }
        }
        Result.Error(_) => stdout.println("empty");
    }
    mut stack: Stack<Item> = Stack();
    stack.push(5);
    stack.push("s");
    match stack.pop() {
        Result.Ok(item) => {
            match item {
                5 => stdout.println("5");
                "s" => stdout.println("s");
                _ => stdout.println("other");
            }
        }
        Result.Error(_) => stdout.println("empty");
    }
}
main();"#,
        "3\nd\n4\ns\n",
    );
}

#[test]
fn managed_union_elements_in_all_sequence_families_match_across_engines() {
    assert_parity(
        "managed_union_sequence_families",
        r#"import { stdout } from std.io;
class Payload {
    text: String;
    construct(text: String) { this.text = text; }
}
type Item = Payload | Int32;
fn show(item: Item): Void {
    if item is Payload { stdout.println(item.text); }
    else { stdout.println(item.to_string()); }
}
fn main(): Void {
    mut array: Array<Item> = Array(1);
    array[0] = Payload("array");
    mut list: List<Item> = List();
    list.add(Payload("list"));
    mut deque: Deque<Item> = Deque();
    deque.push_back(Payload("deque"));
    mut queue: Queue<Item> = Queue();
    queue.enqueue(Payload("queue"));
    mut stack: Stack<Item> = Stack();
    stack.push(Payload("stack"));
    show(array[0]);
    for value in list { show(value); }
    for value in deque { show(value); }
    match queue.dequeue() {
        Result.Ok(value) => show(value);
        Result.Error(_) => stdout.println("queue-empty");
    }
    match stack.pop() {
        Result.Ok(value) => show(value);
        Result.Error(_) => stdout.println("stack-empty");
    }
}
main();"#,
        "array\nlist\ndeque\nqueue\nstack\n",
    );
}

#[test]
fn mixed_literal_union_matching_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
type State = "ready" | "failed" | 0 | 1 | true | false;
fn describe(value: State): String {
    return match value {
        "ready" => "ready";
        "failed" => "failed";
        0 => "zero";
        1 => "one";
        true => "true";
        false => "false";
    };
}
fn main(): Void {
    stdout.println(describe("ready"));
    stdout.println(describe(1));
    stdout.println(describe(false));
}
main();
"#;
    assert_parity("literal_union", program, "ready\none\nfalse\n");
}

#[test]
fn nullable_union_matching_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
type Value = Int32 | String?;
fn describe(value: Value): String {
    return match value {
        null => "empty";
        9 => "number";
        "zirk" => "text";
        _ => "other";
    };
}
fn main(): Void {
    stdout.println(describe(null));
    stdout.println(describe(9));
    stdout.println(describe("zirk"));
}
main();
"#;
    assert_parity("nullable_union", program, "empty\nnumber\ntext\n");
}

#[test]
fn enum_singleton_union_matching_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
enum Shade { Red, Blue }
type Color = Shade.Red | Shade.Blue;
fn describe(value: Color): String {
    return match value {
        Shade.Red => "warm";
        Shade.Blue => "cool";
    };
}
fn main(): Void {
    stdout.println(describe(Shade.Red));
    stdout.println(describe(Shade.Blue));
}
main();
"#;
    assert_parity("enum_singleton_union", program, "warm\ncool\n");
}

#[test]
fn numeric_and_temporal_singleton_unions_match_across_engines() {
    let program = r#"import { stdout } from std.io;
type Wait = 250ms | 1s;
type Day = Date(2026, 12, 25) | Date(2027, 1, 1);
type Ratio = 1.5f64 | 2.5f64;
type Money = 1.25 | 2.5;
type Letter = 'a' | 'b';
fn wait(value: Wait): String { return match value { 250ms => "short"; 1s => "long"; }; }
fn day(value: Day): String { return match value { Date(2026, 12, 25) => "holiday"; Date(2027, 1, 1) => "new year"; }; }
fn ratio(value: Ratio): String { return match value { 1.5f64 => "first"; 2.5f64 => "second"; }; }
fn money(value: Money): String { return match value { 1.25 => "first"; 2.5 => "second"; }; }
fn letter(value: Letter): String { return match value { 'a' => "first"; 'b' => "second"; }; }
fn main(): Void {
    stdout.println(wait(1s));
    stdout.println(day(Date(2027, 1, 1)));
    stdout.println(ratio(2.5f64));
    stdout.println(money(2.5));
    stdout.println(letter('b'));
}

main();
"#;
    assert_parity(
        "numeric_temporal_singleton_unions",
        program,
        "long\nnew year\nsecond\nsecond\nsecond\n",
    );
}

#[test]
fn structural_union_discriminants_and_narrowing_match_across_engines() {
    let program = r#"import { stdout } from std.io;
type NumberEntry = { kind: "number"; value: Int32 };
type TextEntry = { kind: "text"; value: String };
type Entry = NumberEntry | TextEntry;
fn show(entry: Entry): Void {
    if entry.kind == "number" { stdout.println("number {entry.value}"); }
    else if entry.kind == "text" { stdout.println("text {entry.value}"); }
}
fn main(): Void {
    inmut first: Entry = { kind: "number", value: 42 };
    inmut second: Entry = { kind: "text", value: "ready" };
    show(first);
    show(second);
}
main();
"#;
    assert_parity(
        "structural_union_discriminants",
        program,
        "number 42\ntext ready\n",
    );
}

#[test]
fn structural_union_three_alternatives_match_across_engines() {
    let program = r#"import { stdout } from std.io;
type NumberEntry = { kind: "number"; value: Int32 };
type TextEntry = { kind: "text"; value: String };
type FlagEntry = { kind: "flag"; value: Boolean };
type Entry = NumberEntry | TextEntry | FlagEntry;
fn show(entry: Entry): Void {
    if entry.kind == "number" { stdout.println("number {entry.value}"); }
    else if entry.kind == "text" { stdout.println("text {entry.value}"); }
    else if entry.kind == "flag" { stdout.println("flag {entry.value}"); }
}
fn main(): Void {
    inmut number: Entry = { kind: "number", value: 42 };
    inmut text: Entry = { kind: "text", value: "ready" };
    inmut flag: Entry = { kind: "flag", value: true };
    show(number);
    show(text);
    show(flag);
}
main();
"#;
    assert_parity(
        "structural_union_three_alternatives",
        program,
        "number 42\ntext ready\nflag true\n",
    );
}

#[test]
fn structural_union_with_scalar_alternative_matches_across_engines() {
    let program = r#"import { stdout } from std.io;
type TextEntry = { kind: "text"; value: String };
type Entry = TextEntry | 42;
fn identity(entry: Entry): Entry { return entry; }
fn show(entry: Entry): String {
    inmut result: Entry = identity(entry);
    return match result { 42 => "number"; _ => result.value; };
}
fn main(): Void {
    inmut text: Entry = { kind: "text", value: "ready" };
    inmut number: Entry = 42;
    stdout.println(show(text));
    stdout.println(show(number));
}
main();
"#;
    assert_parity(
        "structural_union_scalar_alternative",
        program,
        "ready\nnumber\n",
    );
}

#[test]
fn captured_structural_union_payload_matches_across_engines() {
    let program = r#"import { stdout } from std.io;
type Payload = { text: String };
type Value = Payload | 42;
fn main(): Void {
    inmut captured: Value = { text: "captured structural payload" };
    inmut read: Fn() => Value = (): Value => captured;
    inmut value: Value = read();
    stdout.println(match value { 42 => "number"; _ => value.text; });
}
main();
"#;
    assert_parity(
        "captured_structural_union_payload",
        program,
        "captured structural payload\n",
    );
}

#[test]
fn structural_intersection_view_keeps_identity_across_engines() {
    let program = r#"import { stdout } from std.io;
type Named = { name: String };
type Numbered = { id: Int32 };
type IdentifiedPerson = Named & Numbered;
fn describe(value: IdentifiedPerson): String { return "{value.name}:{value.id}"; }
fn main(): Void {
    mut person: IdentifiedPerson = { name: "Ada", id: 7 };
    mut same: IdentifiedPerson = person;
    same.id = 8;
    stdout.println(describe(person));
}
main();
"#;
    assert_parity("structural_intersection_view", program, "Ada:8\n");
}

#[test]
fn class_contract_intersection_view_keeps_identity_across_engines() {
    let program = r#"import { stdout } from std.io;
contract Named { name: String; }
contract Identified { id: Int32; }
class Entity {
    id: Int32;
    construct(id: Int32) { this.id = id; }
}
class Person extends Entity implements Named, Identified {
    name: String;
    construct(name: String, id: Int32) { super(id); this.name = name; }
}
type NamedPerson = Person & Entity & Named & Identified;
fn describe(value: NamedPerson): String { return value.name + ":" + value.id.to_string(); }
fn main(): Void {
    mut person: NamedPerson = Person("Ada", 7);
    mut same: NamedPerson = person;
    same.name = "Grace";
    same.id = 8;
    stdout.println(describe(person));
}
main();
"#;
    assert_parity("class_contract_intersection_view", program, "Grace:8\n");
}

#[test]
fn open_template_indexed_callback_matches_native_and_interpreter() {
    let program = r#"import { stdout } from std.io;
fn on<T, K from String & keyof T>(value: T, event: "{K}Changed", new_value: T[K], callback: Fn(T[K]) => Void): Void {
    callback(new_value);
}
fn main(): Void {
    inmut person: { age: Int32; name: String } = { age: 42, name: "Ada" };
    on(person, "ageChanged", person.age, (age): Void => { stdout.println(age); });
}
main();
"#;
    assert_parity("open_template_indexed_callback", program, "42\n");
}

#[test]
fn contract_sealing_missing_reads_and_class_properties_match_across_engines() {
    for (name, source, expected) in [
        (
            "dynamic_object_named_read",
            include_str!("corpus/valid/dynamic_object_named_read.zrk"),
            include_str!("corpus/valid/dynamic_object_named_read.out"),
        ),
        (
            "json_stringify_dynamic",
            include_str!("corpus/valid/json_stringify_dynamic.zrk"),
            include_str!("corpus/valid/json_stringify_dynamic.out"),
        ),
        (
            "json_generated_codec",
            include_str!("corpus/valid/json_generated_codec.zrk"),
            include_str!("corpus/valid/json_generated_codec.out"),
        ),
        (
            "json_generated_codec_implicit_json",
            include_str!("corpus/valid/json_generated_codec_implicit_json.zrk"),
            include_str!("corpus/valid/json_generated_codec_implicit_json.out"),
        ),
        (
            "json_generated_codec_collections",
            include_str!("corpus/valid/json_generated_codec_collections.zrk"),
            include_str!("corpus/valid/json_generated_codec_collections.out"),
        ),
        (
            "json_generated_codec_field_defaults",
            include_str!("corpus/valid/json_generated_codec_field_defaults.zrk"),
            include_str!("corpus/valid/json_generated_codec_field_defaults.out"),
        ),
        (
            "json_generated_codec_payload_enum",
            include_str!("corpus/valid/json_generated_codec_payload_enum.zrk"),
            include_str!("corpus/valid/json_generated_codec_payload_enum.out"),
        ),
        (
            "json_stringify_pretty_dynamic",
            include_str!("corpus/valid/json_stringify_pretty_dynamic.zrk"),
            include_str!("corpus/valid/json_stringify_pretty_dynamic.out"),
        ),
        (
            "json_canonicalize_dynamic",
            include_str!("corpus/valid/json_canonicalize_dynamic.zrk"),
            include_str!("corpus/valid/json_canonicalize_dynamic.out"),
        ),
        (
            "json_streaming_public_api",
            include_str!("corpus/valid/json_streaming_public_api.zrk"),
            include_str!("corpus/valid/json_streaming_public_api.out"),
        ),
        (
            "jsonpath_dynamic",
            include_str!("corpus/valid/jsonpath_dynamic.zrk"),
            include_str!("corpus/valid/jsonpath_dynamic.out"),
        ),
        (
            "jsonschema_dynamic",
            include_str!("corpus/valid/jsonschema_dynamic.zrk"),
            include_str!("corpus/valid/jsonschema_dynamic.out"),
        ),
        (
            "contract_conformance",
            include_str!("corpus/valid/dynamic_object_contract_conformance.zrk"),
            include_str!("corpus/valid/dynamic_object_contract_conformance.out"),
        ),
        (
            "callable_contract_operations",
            include_str!("corpus/valid/object_callable_contract_operations.zrk"),
            include_str!("corpus/valid/object_callable_contract_operations.out"),
        ),
        (
            "missing_property",
            include_str!("corpus/valid/dynamic_object_missing_read_catch.zrk"),
            include_str!("corpus/valid/dynamic_object_missing_read_catch.out"),
        ),
        (
            "class_dynamic_properties",
            include_str!("corpus/valid/class_dynamic_object_properties.zrk"),
            include_str!("corpus/valid/class_dynamic_object_properties.out"),
        ),
        (
            "dynamic_cast_mismatch",
            include_str!("corpus/valid/dynamic_cast_mismatch_catch.zrk"),
            include_str!("corpus/valid/dynamic_cast_mismatch_catch.out"),
        ),
        (
            "object_identity",
            include_str!("corpus/valid/structural_equality.zrk"),
            include_str!("corpus/valid/structural_equality.out"),
        ),
        (
            "spread_and_rest",
            include_str!("corpus/valid/spread_rest_operators.zrk"),
            include_str!("corpus/valid/spread_rest_operators.out"),
        ),
    ] {
        assert_parity(name, source, expected);
    }
}

#[test]
fn json5_relaxed_values_and_errors_match_native_and_interpreter() {
    assert_parity(
        "json5_relaxed_values",
        r#"import { JSON } from std.json;
import { stdout } from std.io;
fn main(): Void {
    try {
        inmut value: Dynamic = JSON.parse5("\{ // relaxed syntax\n unquoted: 'text', hex: +0x10, fraction: .5, list: [1, 2,], duplicate: 1, duplicate: 2 \}");
        stdout.println(JSON.stringify(value));
    } catch JsonConversionError(_) { stdout.println("parse failed"); }
    try {
        inmut invalid: Dynamic = JSON.parse5("\{ value: '\\u12' \}");
        stdout.println(JSON.stringify(invalid));
    } catch JsonConversionError(_) { stdout.println("syntax caught"); }
}

main();"#,
        "{\"duplicate\":2.0,\"fraction\":0.5,\"hex\":16.0,\"list\":[1.0,2.0],\"unquoted\":\"text\"}\nsyntax caught\n",
    );
}

#[test]
fn instant_parse_arithmetic_and_format_match_across_engines() {
    assert_parity(
        "instant_temporal",
        include_str!("corpus/valid/instant_temporal.zrk"),
        "2024-03-10T07:30:00+00:00\n1710055800\n1710055800000\n1710055800000000000\n2024-03-10T07:30:00+00:00\ntrue\n2s\n-1\n-1\n-1\ntrue\n",
    );
}

#[test]
fn instant_unix_factories_and_range_errors_match_across_engines() {
    let source = r#"import { stdout } from std.io;
fn main(): Void {
    match Instant.from_unix_seconds(0) {
        Ok(value) => stdout.println(value);
        Error(_) => stdout.println("seconds error");
    };
    match Instant.from_unix_milliseconds(1000) {
        Ok(value) => stdout.println(value);
        Error(_) => stdout.println("milliseconds error");
    };
    match Instant.from_unix_nanoseconds(1500000000) {
        Ok(value) => stdout.println(value);
        Error(_) => stdout.println("nanoseconds error");
    };
    match Instant.from_unix_nanoseconds(-1) {
        Ok(value) => stdout.println(value);
        Error(_) => stdout.println("negative nanoseconds error");
    };
    stdout.println(match Instant.from_unix_seconds(1) {
        Ok(seconds) => match Instant.from_unix_milliseconds(1000) {
            Ok(milliseconds) => seconds == milliseconds;
            Error(_) => false;
        };
        Error(_) => false;
    });
    stdout.println(match Instant.from_unix_seconds(Int64.MAX) {
        Error(_) => true;
        Ok(_) => false;
    });
    stdout.println(match Instant.from_unix_nanoseconds(Int128.MAX) {
        Error(_) => true;
        Ok(_) => false;
    });
}
main();"#;
    assert_parity(
        "instant_unix_factories",
        source,
        "1970-01-01T00:00:00+00:00\n1970-01-01T00:00:01+00:00\n1970-01-01T00:00:01.500+00:00\n1969-12-31T23:59:59.999999999+00:00\ntrue\ntrue\ntrue\n",
    );
}

#[test]
fn time_zone_parsing_and_queries_match_native_and_interpreter() {
    assert_parity(
        "time_zone_parse",
        include_str!("corpus/valid/time_zone_parse.zrk"),
        include_str!("corpus/valid/time_zone_parse.out"),
    );
}

#[test]
fn zoned_datetime_conversion_preserves_the_instant_across_engines() {
    assert_parity(
        "zoned_datetime_facade",
        include_str!("corpus/valid/zoned_datetime_facade.zrk"),
        include_str!("corpus/valid/zoned_datetime_facade.out"),
    );
}

#[test]
fn local_zone_resolution_preserves_typed_dst_error_subclasses() {
    assert_parity(
        "typed_zone_resolution_errors",
        r#"import { stdout } from std.io;
import { Date, DateTime, Time, TimeZone } from std.time;
fn label(error: LocaleError): String {
    if error is AmbiguousLocalTimeError { return "ambiguous"; }
    if error is NonexistentLocalTimeError { return "nonexistent"; }
    if error is InvalidDstPolicyError { return "invalid-policy"; }
    if error is UnknownTimeZoneError { return "unknown-zone"; }
    if error is InvalidTimeZoneError { return "invalid-zone"; }
    if error is TemporalRangeError { return "out-of-range"; }
    return "generic";
}
fn main(): Void {
    match TimeZone.parse("Mars/Base") {
        Ok(_) => stdout.println("unexpected");
        Error(error) => stdout.println(label(error));
    };
    match TimeZone.parse("+25:00") {
        Ok(_) => stdout.println("unexpected");
        Error(error) => stdout.println(label(error));
    };
    match TimeZone.parse("America/New_York") {
        Ok(zone) => {
            match DateTime(Date(2024, 11, 3), Time(1, 30)).in_zone(zone, "reject", "reject") {
                Ok(_) => stdout.println("unexpected");
                Error(error) => stdout.println(label(error));
            };
            match DateTime(Date(2024, 3, 10), Time(2, 30)).in_zone(zone, "reject", "reject") {
                Ok(_) => stdout.println("unexpected");
                Error(error) => stdout.println(label(error));
            };
            match DateTime(Date(2024, 5, 1), Time(12, 0)).in_zone(zone, "bad", "reject") {
                Ok(_) => stdout.println("unexpected");
                Error(error) => stdout.println(label(error));
            };
        }
        Error(_) => stdout.println("zone-unavailable");
    };
}
main();"#,
        "unknown-zone\ninvalid-zone\nambiguous\nnonexistent\ninvalid-policy\n",
    );
}

#[test]
fn localized_temporal_formatting_matches_native_and_interpreter() {
    assert_parity(
        "temporal_locale",
        include_str!("corpus/valid/temporal_locale.zrk"),
        include_str!("corpus/valid/temporal_locale.out"),
    );
}

#[test]
fn localized_temporal_parsing_matches_native_and_interpreter() {
    assert_parity(
        "temporal_locale_parse",
        include_str!("corpus/valid/temporal_locale_parse.zrk"),
        include_str!("corpus/valid/temporal_locale_parse.out"),
    );
}

#[test]
fn localized_zoned_temporal_formatting_matches_native_and_interpreter() {
    assert_parity(
        "temporal_locale_zones",
        include_str!("corpus/valid/temporal_locale_zones.zrk"),
        "November 3, 2024 at 1:30:00\u{202f}AM Eastern Daylight Time\nNovember 3, 2024 at 1:30:00\u{202f}AM Eastern Standard Time\nambiguous time rejected\nMarch 10, 2024 at 3:30:00\u{202f}AM Eastern Daylight Time\n2024年3月10日 2:30:00 GMT+09:00\ninvalid DST policy rejected\n",
    );
}

#[test]
fn tagged_time_shifts_preserve_clock_carry_and_calendar_dst_order() {
    assert_parity(
        "timeshift_tagged_forms",
        include_str!("corpus/valid/timeshift_tagged_forms.zrk"),
        include_str!("corpus/valid/timeshift_tagged_forms.out"),
    );
}

#[test]
fn period_construction_parsing_and_formatting_match_across_engines() {
    assert_parity(
        "period_temporal",
        include_str!("corpus/valid/period_temporal.zrk"),
        include_str!("corpus/valid/period_temporal.out"),
    );
}

#[test]
fn composite_text_collection_keys_match_native_and_interpreter() {
    let program = include_str!("corpus/valid/composite_text_collection_keys.zrk");
    assert_parity(
        "composite_text_collection_keys",
        program,
        "41\nfalse\ntrue\ntrue\nfalse\n1\n2\ntrue\ntrue\ntrue\ntrue\nfalse\n2\n",
    );
}

#[test]
fn union_collection_keys_match_native_and_interpreter() {
    assert_parity(
        "union_collection_keys",
        include_str!("corpus/valid/union_collection_keys.zrk"),
        "70\n80\nfalse\n2\n81\n2\ntrue\ntrue\n93\n73\n2\n111\n2\n112\n100\nboolean\n5\n1\n31\n12\n5\n6\n2\n2\n",
    );
}

#[test]
fn union_scalar_collection_keys_match_native_and_interpreter() {
    assert_parity(
        "union_scalar_collection_keys",
        include_str!("corpus/valid/union_scalar_collection_keys.zrk"),
        "6\n1\n2\n7\n4\n5\n6\n6\ntrue\ntrue\n",
    );
}

#[test]
fn nested_aggregate_union_collections_match_native_and_interpreter() {
    assert_parity(
        "union_nested_aggregate_collection",
        include_str!("corpus/valid/union_nested_aggregate_collection.zrk"),
        "nested union payload\ntrue\n",
    );
}
