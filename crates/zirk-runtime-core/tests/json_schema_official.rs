use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use url::Url;
use zirk_runtime_core::json::{
    JsonLimits, validate_json_schema, validate_json_schema_with_resolver,
};

const DRAFT_2020_12_SCHEMA: &str = "https://json-schema.org/draft/2020-12/schema";
const TEST_REMOTE_PREFIX: &str = "http://localhost:1234/draft2020-12/";

fn json_files(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read JSON Schema suite directory") {
        let path = entry.expect("suite entry").path();
        if path.is_dir() {
            json_files(&path, out);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            out.push(path);
        }
    }
}

fn requires_external_resource(value: &Value) -> bool {
    match value {
        Value::Object(properties) => properties.iter().any(|(key, value)| {
            (matches!(key.as_str(), "$ref" | "$dynamicRef")
                && value
                    .as_str()
                    .is_some_and(|reference| !reference.starts_with('#')))
                || (key == "$schema"
                    && value.as_str().is_some_and(|uri| {
                        uri != DRAFT_2020_12_SCHEMA
                            && !uri.starts_with("https://json-schema.org/draft/2020-12/")
                    }))
                || requires_external_resource(value)
        }),
        Value::Array(items) => items.iter().any(requires_external_resource),
        _ => false,
    }
}

fn resolve_uri(reference: &str, base: Option<&str>) -> Option<String> {
    if let Ok(absolute) = Url::parse(reference) {
        return Some(resource_uri(absolute));
    }
    Url::parse(base?)
        .ok()?
        .join(reference)
        .ok()
        .map(resource_uri)
}

fn resource_uri(mut uri: Url) -> String {
    uri.set_fragment(None);
    uri.to_string()
}

fn collect_internal_ids(value: &Value, inherited_base: Option<&str>, ids: &mut BTreeSet<String>) {
    match value {
        Value::Object(properties) => {
            let base = properties
                .get("$id")
                .and_then(Value::as_str)
                .and_then(|id| resolve_uri(id, inherited_base))
                .or_else(|| inherited_base.map(str::to_owned));
            if properties.contains_key("$id")
                && let Some(base) = &base
            {
                ids.insert(base.clone());
            }
            for child in properties.values() {
                collect_internal_ids(child, base.as_deref(), ids);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_internal_ids(item, inherited_base, ids);
            }
        }
        _ => {}
    }
}

fn referenced_resources(
    value: &Value,
    inherited_base: Option<&str>,
    internal_ids: &BTreeSet<String>,
    refs: &mut BTreeSet<String>,
    unsupported: &mut bool,
) {
    match value {
        Value::Object(properties) => {
            let base = properties
                .get("$id")
                .and_then(Value::as_str)
                .and_then(|id| resolve_uri(id, inherited_base))
                .or_else(|| inherited_base.map(str::to_owned));
            for (key, value) in properties {
                if matches!(key.as_str(), "$ref" | "$dynamicRef" | "$schema") {
                    let Some(reference) = value.as_str() else {
                        continue;
                    };
                    if key == "$schema" && reference == DRAFT_2020_12_SCHEMA {
                        continue;
                    }
                    if reference.starts_with('#') {
                        continue;
                    }
                    let Some(uri) = resolve_uri(reference, base.as_deref()) else {
                        *unsupported = true;
                        continue;
                    };
                    if internal_ids.contains(&uri)
                        || uri.starts_with("https://json-schema.org/draft/2020-12/")
                    {
                        continue;
                    }
                    if uri.starts_with(TEST_REMOTE_PREFIX) {
                        refs.insert(uri);
                    } else {
                        *unsupported = true;
                    }
                }
                referenced_resources(value, base.as_deref(), internal_ids, refs, unsupported);
            }
        }
        Value::Array(items) => {
            for item in items {
                referenced_resources(item, inherited_base, internal_ids, refs, unsupported);
            }
        }
        _ => {}
    }
}

fn explicit_suite_resources(schema: &Value, remote_root: &Path) -> Option<Vec<(String, String)>> {
    let mut queued = BTreeSet::new();
    let mut unsupported = false;
    let mut internal_ids = BTreeSet::new();
    collect_internal_ids(schema, None, &mut internal_ids);
    referenced_resources(schema, None, &internal_ids, &mut queued, &mut unsupported);
    if unsupported {
        return None;
    }

    let mut loaded = BTreeMap::new();
    while let Some(uri) = queued.pop_first() {
        if loaded.contains_key(&uri) {
            continue;
        }
        let relative = uri.strip_prefix(TEST_REMOTE_PREFIX)?;
        let path = remote_root.join(relative);
        let document = fs::read_to_string(path).ok()?;
        let resource: Value = serde_json::from_str(&document).ok()?;
        let mut nested = BTreeSet::new();
        let mut resource_ids = BTreeSet::new();
        collect_internal_ids(&resource, Some(&uri), &mut resource_ids);
        referenced_resources(
            &resource,
            Some(&uri),
            &resource_ids,
            &mut nested,
            &mut unsupported,
        );
        if unsupported {
            return None;
        }
        queued.extend(nested);
        loaded.insert(uri, document);
    }
    Some(loaded.into_iter().collect())
}

#[test]
fn json_schema_matches_official_draft_2020_12_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/json/schema/official/draft2020-12");
    let remote_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/json/schema/official/remotes/draft2020-12");
    let mut files = Vec::new();
    json_files(&root, &mut files);
    files.sort();

    let mut cases = 0usize;
    let mut skipped_optional_external = 0usize;
    let mut resolver_cases = 0usize;
    let mut optional_cases = 0usize;
    let mut skipped_optional_formats = 0usize;
    for path in files {
        let is_optional = path
            .components()
            .any(|component| component.as_os_str() == "optional");
        let is_optional_format = is_optional
            && path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "format");
        if is_optional_format {
            let contents = fs::read_to_string(&path).expect("read optional format cases");
            let groups: Value =
                serde_json::from_str(&contents).expect("valid optional format JSON");
            skipped_optional_formats += groups
                .as_array()
                .expect("optional format groups are arrays")
                .iter()
                .map(|group| {
                    group["tests"]
                        .as_array()
                        .expect("group tests are an array")
                        .len()
                })
                .sum::<usize>();
            continue;
        }
        let contents = fs::read_to_string(&path).expect("read official JSON Schema cases");
        let groups: Value = serde_json::from_str(&contents).expect("valid official suite JSON");
        for group in groups.as_array().expect("test groups are arrays") {
            let schema = &group["schema"];
            let resolver_documents = if requires_external_resource(schema) {
                let Some(documents) = explicit_suite_resources(schema, &remote_root) else {
                    let test_count = group["tests"]
                        .as_array()
                        .expect("group tests are an array")
                        .len();
                    if is_optional {
                        skipped_optional_external += test_count;
                    } else {
                        panic!(
                            "required external-resource group is not mapped: {} / {}",
                            path.display(),
                            group["description"]
                        );
                    }
                    continue;
                };
                resolver_cases += group["tests"]
                    .as_array()
                    .expect("group tests are an array")
                    .len();
                Some(documents)
            } else {
                None
            };
            let schema = serde_json::to_string(schema).expect("serialize suite schema");
            for test in group["tests"].as_array().expect("group tests are an array") {
                let instance = serde_json::to_string(&test["data"]).expect("serialize instance");
                let result = if let Some(documents) = &resolver_documents {
                    validate_json_schema_with_resolver(
                        &schema,
                        &instance,
                        JsonLimits::default(),
                        documents,
                    )
                } else {
                    validate_json_schema(&schema, &instance, JsonLimits::default())
                };
                let expected = test["valid"].as_bool().expect("expected validity");
                let errors = result.unwrap_or_else(|error| {
                    panic!(
                        "{} / {}: validator rejected suite schema: {error:?}",
                        path.display(),
                        test["description"]
                    )
                });
                let actual = errors.is_empty();
                assert_eq!(
                    actual,
                    expected,
                    "{} / {} / {} (errors: {errors:?})",
                    path.display(),
                    group["description"],
                    test["description"]
                );
                cases += 1;
                optional_cases += usize::from(is_optional);
            }
        }
    }

    assert_eq!(cases, 1463, "official Draft 2020-12 case count changed");
    assert_eq!(resolver_cases, 128);
    assert_eq!(optional_cases, 161);
    assert_eq!(skipped_optional_external, 1);
    assert_eq!(skipped_optional_formats, 896);
    eprintln!(
        "ran {cases} official cases ({resolver_cases} with explicit suite resources, {optional_cases} optional); skipped zero required external-resource cases, {skipped_optional_external} optional cross-draft cases, and {skipped_optional_formats} optional format assertions"
    );
}
