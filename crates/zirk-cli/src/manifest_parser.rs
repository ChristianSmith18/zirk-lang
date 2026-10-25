//! Parser and typed project model construction for the canonical `.zkinit`
//! grammar. The small token stream preserves byte ranges for diagnostics and
//! editor tooling while keeping manifest syntax separate from `.zrk` source.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};

use crate::project::{
    Capability, Dependency, DependencySource, GlobalVariable, ManifestSpan, ManifestSpans,
    ProjectError, ProjectKind, ProjectManifest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Atom(String),
    String(String),
    Symbol(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    Atom(String),
    String(String),
    Array(Vec<Value>),
    Object(Vec<Member>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Member {
    key: String,
    key_span: ManifestSpan,
    value: Value,
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    cursor: usize,
}

pub fn parse_manifest(source: &str) -> Result<ProjectManifest, ProjectError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser {
        source,
        tokens,
        cursor: 0,
    };
    let mut project = None;
    let mut source_roots = Vec::new();
    let mut source_spans = Vec::new();
    let mut build_targets = Vec::new();
    let mut build_target_spans = Vec::new();
    let mut build_targets_section_span = None;
    let mut dependencies = BTreeMap::new();
    let mut dependency_spans = BTreeMap::new();
    let mut dev_dependencies = BTreeMap::new();
    let mut dev_dependency_spans = BTreeMap::new();
    let mut globals = BTreeMap::new();
    let mut global_spans = BTreeMap::new();
    let mut globals_section_span = None;
    let mut requirements = BTreeSet::new();
    let mut requirement_spans = BTreeMap::new();
    let mut manifest_field_spans = BTreeMap::new();
    let mut seen = HashMap::<String, ManifestSpan>::new();

    while !parser.is_eof() {
        let (section, section_span) = parser.take_atom()?;
        match section.as_str() {
            "source" => {
                let (path, span) = parser.take_string()?;
                validate_relative_path(&path, "source root", span)?;
                if source_roots.contains(&PathBuf::from(&path)) {
                    return Err(parser.error_at(span, format!("duplicate source root `{path}`")));
                }
                source_roots.push(PathBuf::from(path));
                source_spans.push(span);
                parser.optional_symbol(';');
            }
            "project" => {
                parser.unique_section(&mut seen, &section, section_span)?;
                project = Some(parser.object()?);
            }
            "build_targets" => {
                parser.unique_section(&mut seen, &section, section_span)?;
                build_targets_section_span = Some(section_span);
                for (value, span) in parser.value_block()? {
                    let Value::String(value) = value else {
                        return Err(parser.error_at(span, "build targets must be quoted strings"));
                    };
                    if !is_supported_target(&value) {
                        return Err(
                            parser.error_at(span, format!("unsupported build target `{value}`"))
                        );
                    }
                    if build_targets.contains(&value) {
                        return Err(
                            parser.error_at(span, format!("duplicate build target `{value}`"))
                        );
                    }
                    build_targets.push(value);
                    build_target_spans.push(span);
                }
                if build_targets.is_empty() {
                    return Err(parser.error_at(section_span, "`build_targets` must not be empty"));
                }
            }
            "dependencies" | "dev_dependencies" => {
                parser.unique_section(&mut seen, &section, section_span)?;
                let members = parser.object()?;
                let (target, spans) = if section == "dependencies" {
                    (&mut dependencies, &mut dependency_spans)
                } else {
                    (&mut dev_dependencies, &mut dev_dependency_spans)
                };
                for member in members {
                    if let Value::Object(fields) = &member.value {
                        let prefix = if section == "dependencies" {
                            "dependencies"
                        } else {
                            "dev_dependencies"
                        };
                        for field in fields {
                            manifest_field_spans.insert(
                                format!("{prefix}.{}.{}", member.key, field.key),
                                field.key_span,
                            );
                        }
                    }
                    let dependency = parse_dependency(&parser, &member)?;
                    spans.insert(member.key.clone(), member.key_span);
                    target.insert(member.key, dependency);
                }
            }
            "globals" => {
                parser.unique_section(&mut seen, &section, section_span)?;
                globals_section_span = Some(section_span);
                for member in parser.object()? {
                    if let Value::Object(fields) = &member.value {
                        for field in fields {
                            manifest_field_spans.insert(
                                format!("globals.{}.{}", member.key, field.key),
                                field.key_span,
                            );
                        }
                    }
                    let global = parse_global(&parser, &member)?;
                    global_spans.insert(member.key.clone(), member.key_span);
                    globals.insert(member.key, global);
                }
            }
            "requires" => {
                parser.unique_section(&mut seen, &section, section_span)?;
                parser.expect_symbol('{')?;
                while !parser.next_is_symbol('}') {
                    if parser.is_eof() {
                        return Err(parser.error_at(section_span, "unclosed `requires` block"));
                    }
                    let token = parser.peek().cloned().expect("not at EOF");
                    let TokenKind::Atom(ref name) = token.kind else {
                        return Err(parser
                            .error_at(parser.span(&token), "expected a capability identifier"));
                    };
                    parser.cursor += 1;
                    let span = parser.span(&token);
                    let capability = Capability::parse(&name).ok_or_else(|| {
                        parser.error_at(span, format!("unknown capability `{name}`"))
                    })?;
                    if !requirements.insert(capability) {
                        return Err(parser.error_at(span, format!("duplicate capability `{name}`")));
                    }
                    requirement_spans.insert(capability, span);
                    parser.optional_symbol(';');
                    parser.optional_symbol(',');
                }
                parser.expect_symbol('}')?;
            }
            other => {
                let hint = match other {
                    "permissions" | "compile_permissions" | "during" => {
                        "legacy permission syntax was removed; declare capability names in `requires { ... }`"
                    }
                    _ => "",
                };
                return Err(parser.error_at(
                    section_span,
                    if hint.is_empty() {
                        format!("unknown `.zkinit` section `{other}`")
                    } else {
                        format!("unknown `.zkinit` section `{other}`; {hint}")
                    },
                ));
            }
        }
    }

    let project_members =
        project.ok_or_else(|| ProjectError("manifest is missing `project { ... }`".into()))?;
    let mut fields = BTreeMap::new();
    let mut field_spans = manifest_field_spans;
    for member in project_members {
        let key = format!("project.{}", member.key);
        if field_spans.insert(key.clone(), member.key_span).is_some() {
            return Err(parser.error_at(member.key_span, format!("duplicate field `{key}`")));
        }
        fields.insert(member.key, member.value);
    }

    let name = take_string_field(&parser, &mut fields, &field_spans, "name")?
        .ok_or_else(|| ProjectError("`project.name` is required".into()))?;
    let version = take_string_field(&parser, &mut fields, &field_spans, "version")?
        .ok_or_else(|| ProjectError("`project.version` is required".into()))?;
    if name.trim().is_empty() {
        return Err(parser.error_at(
            field_spans["project.name"],
            "`project.name` must not be empty",
        ));
    }
    if version.trim().is_empty() {
        return Err(parser.error_at(
            field_spans["project.version"],
            "`project.version` must not be empty",
        ));
    }
    if semver::Version::parse(&version).is_err() {
        return Err(parser.error_at(
            field_spans["project.version"],
            "`project.version` must be a semantic version",
        ));
    }
    let kind_value = take_atom_field(&parser, &mut fields, &field_spans, "type")?
        .ok_or_else(|| ProjectError("`project.type` is required".into()))?;
    let kind = parse_kind(
        &parser,
        &kind_value,
        field_spans.get("project.type").copied(),
    )?;
    let authors =
        take_string_array_field(&parser, &mut fields, &field_spans, "authors")?.unwrap_or_default();
    let description = take_string_field(&parser, &mut fields, &field_spans, "description")?;
    let license = take_string_field(&parser, &mut fields, &field_spans, "license")?;
    if let Some(expression) = &license {
        if !valid_license_expression(expression) {
            return Err(parser.error_at(
                field_spans["project.license"],
                "`project.license` must be a valid SPDX license expression",
            ));
        }
    }
    let entry_value = take_string_field(&parser, &mut fields, &field_spans, "entry")?;
    let root_value = take_string_field(&parser, &mut fields, &field_spans, "root")?;

    match kind {
        ProjectKind::Application if root_value.is_some() => {
            return Err(parser.error_at(
                field_spans["project.root"],
                "`project.root` is only valid for a package",
            ));
        }
        ProjectKind::Package if entry_value.is_some() => {
            return Err(parser.error_at(
                field_spans["project.entry"],
                "`project.entry` is only valid for an application",
            ));
        }
        _ => {}
    }
    if !fields.is_empty() {
        let unknown = fields.keys().next().expect("not empty");
        let span = field_spans[&format!("project.{unknown}")];
        let message = if matches!(
            unknown.as_str(),
            "permissions" | "compile_permissions" | "during"
        ) {
            format!("`project.{unknown}` is removed; declare capabilities in `requires {{ ... }}`")
        } else {
            format!("unknown project field `project.{unknown}`")
        };
        return Err(parser.error_at(span, message));
    }

    let entry = match kind {
        ProjectKind::Application => {
            PathBuf::from(entry_value.unwrap_or_else(|| "src/main.zrk".into()))
        }
        ProjectKind::Package => {
            PathBuf::from(root_value.unwrap_or_else(|| "src/package.zrk".into()))
        }
    };
    validate_relative_path(
        &entry.to_string_lossy(),
        if kind == ProjectKind::Application {
            "project.entry"
        } else {
            "project.root"
        },
        field_spans
            .get(if kind == ProjectKind::Application {
                "project.entry"
            } else {
                "project.root"
            })
            .copied()
            .unwrap_or(ManifestSpan {
                start: 0,
                end: 0,
                line: 1,
                column: 1,
            }),
    )?;

    if source_roots.is_empty() {
        source_roots.push(PathBuf::from("src"));
    }
    if kind == ProjectKind::Package && build_targets_section_span.is_some() {
        return Err(parser.error_at(
            build_targets_section_span.expect("target list has a section"),
            "`build_targets` is only valid for applications",
        ));
    }
    if kind == ProjectKind::Package && globals_section_span.is_some() {
        return Err(parser.error_at(
            globals_section_span.expect("globals have a section"),
            "`globals` is only valid for applications",
        ));
    }

    let spans = ManifestSpans {
        fields: field_spans,
        dependencies: dependency_spans,
        dev_dependencies: dev_dependency_spans,
        globals: global_spans,
        requirements: requirement_spans,
        source_roots: source_spans,
        build_targets: build_target_spans,
    };
    Ok(ProjectManifest {
        name,
        version,
        kind,
        authors,
        description,
        license,
        entry: entry.clone(),
        package_root: (kind == ProjectKind::Package).then_some(entry),
        source_roots,
        build_targets,
        dependencies,
        dev_dependencies,
        globals,
        requires: requirements,
        spans,
        environment_permissions: Default::default(),
        network_permissions: Default::default(),
    })
}

pub fn format_manifest(manifest: &ProjectManifest) -> String {
    let mut output = String::from("project {\n");
    output.push_str(&format!("    name: {};\n", quote(&manifest.name)));
    output.push_str(&format!("    version: {};\n", quote(&manifest.version)));
    output.push_str(&format!(
        "    type: {};\n",
        match manifest.kind {
            ProjectKind::Application => "application",
            ProjectKind::Package => "package",
        }
    ));
    if !manifest.authors.is_empty() {
        output.push_str(&format!(
            "    authors: [{}];\n",
            manifest
                .authors
                .iter()
                .map(|author| quote(author))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(description) = &manifest.description {
        output.push_str(&format!("    description: {};\n", quote(description)));
    }
    if let Some(license) = &manifest.license {
        output.push_str(&format!("    license: {};\n", quote(license)));
    }
    match manifest.kind {
        ProjectKind::Application => output.push_str(&format!(
            "    entry: {};\n",
            quote(&manifest.entry.to_string_lossy())
        )),
        ProjectKind::Package => output.push_str(&format!(
            "    root: {};\n",
            quote(&manifest.entry.to_string_lossy())
        )),
    }
    output.push_str("}\n");

    for source in &manifest.source_roots {
        output.push_str(&format!("\nsource {};\n", quote(&source.to_string_lossy())));
    }
    if !manifest.build_targets.is_empty() {
        output.push_str("\nbuild_targets {\n");
        for target in &manifest.build_targets {
            output.push_str(&format!("    {};\n", quote(target)));
        }
        output.push_str("}\n");
    }
    format_dependencies(&mut output, "dependencies", &manifest.dependencies);
    format_dependencies(&mut output, "dev_dependencies", &manifest.dev_dependencies);
    if !manifest.globals.is_empty() {
        output.push_str("\nglobals {\n");
        for (name, global) in &manifest.globals {
            if global.default.is_none() && global.from.is_none() && !global.readonly {
                output.push_str(&format!("    {name}: {};\n", global.type_name));
                continue;
            }
            output.push_str(&format!(
                "    {name}: {{\n        type: {};\n",
                global.type_name
            ));
            if let Some(default) = &global.default {
                output.push_str(&format!("        default: {default};\n"));
            }
            if let Some(from) = &global.from {
                output.push_str(&format!(
                    "        from: {};\n",
                    quote(&from.to_string_lossy())
                ));
            }
            if global.readonly {
                output.push_str("        readonly: true;\n");
            }
            output.push_str("    }\n");
        }
        output.push_str("}\n");
    }
    if !manifest.requires.is_empty() {
        output.push_str("\nrequires {\n");
        for capability in &manifest.requires {
            output.push_str(&format!("    {};\n", capability.as_str()));
        }
        output.push_str("}\n");
    }
    output
}

fn format_dependencies(
    output: &mut String,
    section: &str,
    dependencies: &BTreeMap<String, Dependency>,
) {
    if dependencies.is_empty() {
        return;
    }
    output.push_str(&format!("\n{section} {{\n"));
    for (name, dependency) in dependencies {
        if let (DependencySource::Registry(registry), None, None) = (
            &dependency.source,
            &dependency.revision,
            &dependency.integrity,
        ) && registry == "default"
        {
            output.push_str(&format!("    {name}: {};\n", quote(&dependency.constraint)));
            continue;
        }
        let locator = match &dependency.source {
            DependencySource::Local(path) => format!("path:{}", path.to_string_lossy()),
            DependencySource::Registry(registry) => format!("registry:{registry}"),
            DependencySource::Git(repository) => format!("git+{repository}"),
            DependencySource::Vendored(path) => format!("vendored:{}", path.to_string_lossy()),
        };
        output.push_str(&format!("    {name}: {{ from: {};", quote(&locator)));
        if !dependency.constraint.is_empty() {
            match dependency.source {
                DependencySource::Git(_) => {}
                _ => output.push_str(&format!(" version: {};", quote(&dependency.constraint))),
            }
        }
        if let Some(revision) = &dependency.revision {
            output.push_str(&format!(" rev: {};", quote(revision)));
        }
        if let Some(integrity) = &dependency.integrity {
            output.push_str(&format!(" integrity: {};", quote(integrity)));
        }
        output.push_str(" }\n");
    }
    output.push_str("}\n");
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn parse_dependency(parser: &Parser<'_>, member: &Member) -> Result<Dependency, ProjectError> {
    let (source, constraint, revision, integrity) = match &member.value {
        Value::String(version) => (
            DependencySource::Registry("default".into()),
            version.clone(),
            None,
            None,
        ),
        Value::Object(fields) => {
            let mut values = BTreeMap::new();
            for field in fields {
                if values.insert(field.key.as_str(), field).is_some() {
                    return Err(parser.error_at(
                        field.key_span,
                        format!("duplicate dependency field `{}`", field.key),
                    ));
                }
            }
            let from_field = values.get("from").copied().ok_or_else(|| {
                parser.error_at(member.key_span, "expanded dependency needs `from`")
            })?;
            let from = string_member(parser, from_field, "dependency.from")?;
            let version = optional_string_member(parser, &values, "version", "dependency.version")?;
            let revision = optional_string_member(parser, &values, "rev", "dependency.rev")?;
            let integrity =
                optional_string_member(parser, &values, "integrity", "dependency.integrity")?;
            for (key, field) in &values {
                if !["from", "version", "rev", "integrity"].contains(key) {
                    return Err(parser
                        .error_at(field.key_span, format!("unknown dependency field `{key}`")));
                }
            }
            let source = parse_source_locator(parser, &from, from_field.key_span)?;
            if let Some(version) = &version {
                if semver::VersionReq::parse(version).is_err() {
                    let field = values.get("version").expect("version came from field");
                    return Err(parser.error_at(
                        field.key_span,
                        format!("invalid semantic version requirement `{version}`"),
                    ));
                }
            }
            if let Some(revision) = &revision {
                if revision.trim().is_empty() || revision.chars().any(char::is_whitespace) {
                    let field = values.get("rev").expect("revision came from field");
                    return Err(parser.error_at(
                        field.key_span,
                        "`dependency.rev` must be a non-empty revision without whitespace",
                    ));
                }
            }
            if let Some(integrity) = &integrity
                && !valid_sha256_integrity(integrity)
            {
                let field = values.get("integrity").expect("integrity came from field");
                return Err(parser.error_at(
                    field.key_span,
                    "`dependency.integrity` must use `sha256:` followed by 64 hexadecimal digits",
                ));
            }
            match source {
                DependencySource::Registry(_) => {
                    if version.is_none() {
                        return Err(parser.error_at(
                            from_field.key_span,
                            "`dependency.from` registry locator requires `dependency.version`",
                        ));
                    }
                    if let Some(field) = values.get("rev") {
                        return Err(parser
                            .error_at(field.key_span, "registry dependency cannot declare `rev`"));
                    }
                }
                DependencySource::Git(_) => {
                    if revision.is_none() {
                        return Err(parser.error_at(
                            from_field.key_span,
                            "`dependency.from` Git locator requires an immutable `dependency.rev`",
                        ));
                    }
                    if let Some(field) = values.get("version") {
                        return Err(parser
                            .error_at(field.key_span, "git dependency uses `rev`, not `version`"));
                    }
                }
                DependencySource::Local(_) | DependencySource::Vendored(_) => {
                    if let Some(field) = values.get("rev") {
                        return Err(parser
                            .error_at(field.key_span, "local dependency cannot declare `rev`"));
                    }
                }
            }
            let constraint = version.unwrap_or_default();
            (source, constraint, revision, integrity)
        }
        _ => {
            return Err(parser.error_at(
                member.key_span,
                "dependency must be a version string or source object",
            ));
        }
    };
    Ok(Dependency {
        name: member.key.clone(),
        constraint,
        source,
        revision,
        integrity,
    })
}

fn parse_source_locator(
    parser: &Parser<'_>,
    locator: &str,
    span: ManifestSpan,
) -> Result<DependencySource, ProjectError> {
    if let Some(path) = locator.strip_prefix("path:") {
        if path.is_empty() || Path::new(path).is_absolute() {
            return Err(parser.error_at(span, "dependency path must be a non-empty relative path"));
        }
        return Ok(DependencySource::Local(PathBuf::from(path)));
    }
    if let Some(repository) = locator.strip_prefix("git+") {
        if repository.starts_with("https://")
            || repository.starts_with("ssh://")
            || repository.starts_with("file://")
        {
            return Ok(DependencySource::Git(repository.into()));
        }
        return Err(parser.error_at(span, "git source must use https://, ssh://, or file://"));
    }
    if let Some(registry) = locator.strip_prefix("registry:") {
        if !registry.is_empty() {
            return Ok(DependencySource::Registry(registry.into()));
        }
    }
    if let Some(path) = locator.strip_prefix("vendored:") {
        validate_relative_path(path, "vendored dependency path", span)?;
        return Ok(DependencySource::Vendored(PathBuf::from(path)));
    }
    Err(parser.error_at(span, format!("unsupported dependency source `{locator}`")))
}

fn parse_global(parser: &Parser<'_>, member: &Member) -> Result<GlobalVariable, ProjectError> {
    match &member.value {
        Value::Atom(type_name) => Ok(GlobalVariable {
            type_name: type_name.clone(),
            default: None,
            from: None,
            readonly: false,
        }),
        Value::Object(fields) => {
            let mut values = BTreeMap::new();
            for field in fields {
                if values.insert(field.key.as_str(), field).is_some() {
                    return Err(parser.error_at(
                        field.key_span,
                        format!("duplicate global field `{}`", field.key),
                    ));
                }
            }
            let type_field = values
                .get("type")
                .copied()
                .ok_or_else(|| parser.error_at(member.key_span, "expanded global needs `type`"))?;
            let type_name = atom_member(parser, type_field, "global.type")?;
            let default = values.get("default").map(|field| field.value.to_source());
            let from =
                optional_string_member(parser, &values, "from", "global.from")?.map(PathBuf::from);
            let readonly = match values.get("readonly").map(|field| &field.value) {
                None => false,
                Some(Value::Atom(value)) if value == "true" => true,
                Some(Value::Atom(value)) if value == "false" => false,
                Some(_) => {
                    let field = values.get("readonly").expect("matched field");
                    return Err(
                        parser.error_at(field.key_span, "global `readonly` must be true or false")
                    );
                }
            };
            for (key, field) in &values {
                if !["type", "default", "from", "readonly"].contains(key) {
                    return Err(
                        parser.error_at(field.key_span, format!("unknown global field `{key}`"))
                    );
                }
            }
            if let Some(path) = &from {
                let from_span = values
                    .get("from")
                    .expect("from value came from field")
                    .key_span;
                validate_relative_path(&path.to_string_lossy(), "global type source", from_span)?;
            }
            Ok(GlobalVariable {
                type_name,
                default,
                from,
                readonly,
            })
        }
        _ => Err(parser.error_at(
            member.key_span,
            "global must be a type name or expanded object",
        )),
    }
}

fn optional_string_member(
    parser: &Parser<'_>,
    fields: &BTreeMap<&str, &Member>,
    name: &str,
    display_name: &str,
) -> Result<Option<String>, ProjectError> {
    fields
        .get(name)
        .map(|field| string_member(parser, field, display_name))
        .transpose()
}

fn string_member(
    parser: &Parser<'_>,
    field: &Member,
    display_name: &str,
) -> Result<String, ProjectError> {
    match &field.value {
        Value::String(value) => Ok(value.clone()),
        _ => Err(parser.error_at(field.key_span, format!("`{display_name}` must be a string"))),
    }
}

fn atom_member(
    parser: &Parser<'_>,
    field: &Member,
    display_name: &str,
) -> Result<String, ProjectError> {
    match &field.value {
        Value::Atom(value) => Ok(value.clone()),
        _ => Err(parser.error_at(
            field.key_span,
            format!("`{display_name}` must be an unquoted type name"),
        )),
    }
}

fn take_string_field(
    parser: &Parser<'_>,
    fields: &mut BTreeMap<String, Value>,
    spans: &BTreeMap<String, ManifestSpan>,
    name: &str,
) -> Result<Option<String>, ProjectError> {
    match fields.remove(name) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(parser.error_at(
            spans[&format!("project.{name}")],
            format!("`project.{name}` must be a string"),
        )),
    }
}

fn take_atom_field(
    parser: &Parser<'_>,
    fields: &mut BTreeMap<String, Value>,
    spans: &BTreeMap<String, ManifestSpan>,
    name: &str,
) -> Result<Option<String>, ProjectError> {
    match fields.remove(name) {
        None => Ok(None),
        Some(Value::Atom(value)) => Ok(Some(value)),
        Some(_) => Err(parser.error_at(
            spans[&format!("project.{name}")],
            format!("`project.{name}` must be an unquoted value"),
        )),
    }
}

fn take_string_array_field(
    parser: &Parser<'_>,
    fields: &mut BTreeMap<String, Value>,
    spans: &BTreeMap<String, ManifestSpan>,
    name: &str,
) -> Result<Option<Vec<String>>, ProjectError> {
    match fields.remove(name) {
        None => Ok(None),
        Some(Value::Array(items)) => items
            .into_iter()
            .map(|item| match item {
                Value::String(value) => Ok(value),
                _ => Err(parser.error_at(
                    spans[&format!("project.{name}")],
                    format!("`project.{name}` must contain only strings"),
                )),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(parser.error_at(
            spans[&format!("project.{name}")],
            format!("`project.{name}` must be an array"),
        )),
    }
}

/// Accepts the SPDX expression grammar shape without embedding a license
/// catalog. Identifier spelling is preserved for downstream policy tooling.
fn valid_license_expression(expression: &str) -> bool {
    let mut expect_identifier = true;
    let mut depth = 0usize;
    for token in expression.split_whitespace() {
        match token {
            "AND" | "OR" if !expect_identifier => expect_identifier = true,
            "WITH" if !expect_identifier => expect_identifier = true,
            "(" if expect_identifier => depth += 1,
            ")" if !expect_identifier && depth > 0 => depth -= 1,
            identifier if expect_identifier => {
                if identifier.is_empty()
                    || !identifier
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
                {
                    return false;
                }
                expect_identifier = false;
            }
            _ => return false,
        }
    }
    !expression.trim().is_empty() && !expect_identifier && depth == 0
}

fn valid_sha256_integrity(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn parse_kind(
    parser: &Parser<'_>,
    value: &str,
    span: Option<ManifestSpan>,
) -> Result<ProjectKind, ProjectError> {
    match value {
        "application" => Ok(ProjectKind::Application),
        "package" => Ok(ProjectKind::Package),
        _ => Err(span.map_or_else(
            || ProjectError("`project.type` must be `application` or `package`".into()),
            |span| parser.error_at(span, "`project.type` must be `application` or `package`"),
        )),
    }
}

fn validate_relative_path(path: &str, field: &str, span: ManifestSpan) -> Result<(), ProjectError> {
    let path_ref = Path::new(path);
    if path.is_empty()
        || path_ref.is_absolute()
        || path_ref.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ProjectError(format!(
            "{field} must be a non-empty project-relative path at {}:{}",
            span.line, span.column
        )));
    }
    Ok(())
}

#[cfg(feature = "backend")]
fn is_supported_target(target: &str) -> bool {
    zirk_codegen_llvm::TARGETS
        .iter()
        .any(|supported| supported.name == target)
}

#[cfg(not(feature = "backend"))]
fn is_supported_target(target: &str) -> bool {
    matches!(
        target,
        "x86_64-windows"
            | "aarch64-windows"
            | "x86_64-linux"
            | "aarch64-linux"
            | "x86_64-macos"
            | "aarch64-macos"
    )
}

impl Value {
    fn to_source(&self) -> String {
        match self {
            Self::Atom(value) => value.clone(),
            Self::String(value) => serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into()),
            Self::Array(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(Self::to_source)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Object(fields) => format!(
                "{{{}}}",
                fields
                    .iter()
                    .map(|field| format!("{}: {}", field.key, field.value.to_source()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl Parser<'_> {
    fn is_eof(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    fn next_is_symbol(&self, symbol: char) -> bool {
        self.peek()
            .is_some_and(|token| token.kind == TokenKind::Symbol(symbol))
    }

    fn span(&self, token: &Token) -> ManifestSpan {
        let prefix = &self.source[..token.start.min(self.source.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .count()
            + 1;
        ManifestSpan {
            start: token.start,
            end: token.end,
            line,
            column,
        }
    }

    fn error_at(&self, span: ManifestSpan, message: impl AsRef<str>) -> ProjectError {
        ProjectError(format!(
            "{} at {}:{}",
            message.as_ref(),
            span.line,
            span.column
        ))
    }

    fn take_atom(&mut self) -> Result<(String, ManifestSpan), ProjectError> {
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .ok_or_else(|| ProjectError("unexpected end of `.zkinit`".into()))?;
        let span = self.span(&token);
        self.cursor += 1;
        match token.kind {
            TokenKind::Atom(value) => Ok((value, span)),
            _ => Err(self.error_at(span, "expected an identifier")),
        }
    }

    fn take_string(&mut self) -> Result<(String, ManifestSpan), ProjectError> {
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .ok_or_else(|| ProjectError("unexpected end of `.zkinit`".into()))?;
        let span = self.span(&token);
        self.cursor += 1;
        match token.kind {
            TokenKind::String(value) => Ok((value, span)),
            _ => Err(self.error_at(span, "expected a quoted string")),
        }
    }

    fn expect_symbol(&mut self, symbol: char) -> Result<(), ProjectError> {
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .ok_or_else(|| ProjectError(format!("expected `{symbol}`")))?;
        let span = self.span(&token);
        if token.kind == TokenKind::Symbol(symbol) {
            self.cursor += 1;
            Ok(())
        } else {
            Err(self.error_at(span, format!("expected `{symbol}`")))
        }
    }

    fn optional_symbol(&mut self, symbol: char) -> bool {
        if self.next_is_symbol(symbol) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn unique_section(
        &self,
        seen: &mut HashMap<String, ManifestSpan>,
        name: &str,
        span: ManifestSpan,
    ) -> Result<(), ProjectError> {
        if let Some(previous) = seen.get(name) {
            return Err(ProjectError(format!(
                "duplicate `{name}` section at {}:{} (first declared at {}:{})",
                span.line, span.column, previous.line, previous.column
            )));
        }
        seen.insert(name.into(), span);
        Ok(())
    }

    fn object(&mut self) -> Result<Vec<Member>, ProjectError> {
        self.expect_symbol('{')?;
        let mut fields = Vec::new();
        let mut seen = HashMap::<String, ManifestSpan>::new();
        while !self.next_is_symbol('}') {
            if self.is_eof() {
                return Err(ProjectError("unclosed `.zkinit` object".into()));
            }
            let token = self.tokens[self.cursor].clone();
            let key_span = self.span(&token);
            let key = match token.kind {
                TokenKind::Atom(value) | TokenKind::String(value) => value,
                _ => return Err(self.error_at(key_span, "expected an object field name")),
            };
            self.cursor += 1;
            if let Some(previous) = seen.insert(key.clone(), key_span) {
                return Err(ProjectError(format!(
                    "duplicate field `{key}` at {}:{} (first declared at {}:{})",
                    key_span.line, key_span.column, previous.line, previous.column
                )));
            }
            self.expect_symbol(':')?;
            let value = self.value()?;
            fields.push(Member {
                key,
                key_span,
                value,
            });
            self.optional_symbol(';');
            self.optional_symbol(',');
        }
        self.expect_symbol('}')?;
        Ok(fields)
    }

    fn value_block(&mut self) -> Result<Vec<(Value, ManifestSpan)>, ProjectError> {
        self.expect_symbol('{')?;
        let mut values = Vec::new();
        while !self.next_is_symbol('}') {
            if self.is_eof() {
                return Err(ProjectError("unclosed `.zkinit` list".into()));
            }
            let token = self.tokens[self.cursor].clone();
            let span = self.span(&token);
            values.push((self.value()?, span));
            self.optional_symbol(';');
            self.optional_symbol(',');
        }
        self.expect_symbol('}')?;
        Ok(values)
    }

    fn value(&mut self) -> Result<Value, ProjectError> {
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .ok_or_else(|| ProjectError("expected a manifest value".into()))?;
        match token.kind {
            TokenKind::Atom(value) => {
                self.cursor += 1;
                Ok(Value::Atom(value))
            }
            TokenKind::String(value) => {
                self.cursor += 1;
                Ok(Value::String(value))
            }
            TokenKind::Symbol('[') => {
                self.cursor += 1;
                let mut values = Vec::new();
                while !self.next_is_symbol(']') {
                    if self.is_eof() {
                        return Err(ProjectError("unclosed manifest array".into()));
                    }
                    values.push(self.value()?);
                    self.optional_symbol(',');
                    self.optional_symbol(';');
                }
                self.expect_symbol(']')?;
                Ok(Value::Array(values))
            }
            TokenKind::Symbol('{') => Ok(Value::Object(self.object()?)),
            _ => Err(self.error_at(self.span(&token), "expected a manifest value")),
        }
    }
}

fn tokenize(source: &str) -> Result<Vec<Token>, ProjectError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') || bytes[index] == b'#' {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        let start = index;
        if bytes[index] == b'"' {
            index += 1;
            let mut escaped = false;
            while index < bytes.len() {
                let byte = bytes[index];
                index += 1;
                if escaped {
                    escaped = false;
                    continue;
                }
                if byte == b'\\' {
                    escaped = true;
                    continue;
                }
                if byte == b'"' {
                    break;
                }
            }
            if index > bytes.len() || bytes.get(index.saturating_sub(1)) != Some(&b'"') {
                return Err(ProjectError("unterminated string in `.zkinit`".into()));
            }
            let raw = &source[start..index];
            let value = serde_json::from_str::<String>(raw)
                .map_err(|_| ProjectError(format!("invalid quoted string at byte {start}")))?;
            tokens.push(Token {
                kind: TokenKind::String(value),
                start,
                end: index,
            });
            continue;
        }
        let symbol = match bytes[index] {
            b'{' | b'}' | b'[' | b']' | b':' | b';' | b',' => Some(bytes[index] as char),
            _ => None,
        };
        if let Some(symbol) = symbol {
            index += 1;
            tokens.push(Token {
                kind: TokenKind::Symbol(symbol),
                start,
                end: index,
            });
            continue;
        }
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && !matches!(
                bytes[index],
                b'{' | b'}' | b'[' | b']' | b':' | b';' | b',' | b'"'
            )
            && !(bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/'))
        {
            index += 1;
        }
        if index == start {
            return Err(ProjectError(format!(
                "invalid character in `.zkinit` at byte {index}"
            )));
        }
        tokens.push(Token {
            kind: TokenKind::Atom(source[start..index].into()),
            start,
            end: index,
        });
    }
    Ok(tokens)
}
