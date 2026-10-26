//! The frontend driver: loading, parsing, and checking without lowering.
//!
//! This module is independent of `zirk-ir`, `zirk-codegen-llvm`, the linker,
//! and the runtime. It is the shared half of both `zirk` and `zirk-check`.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use zirk_diagnostics::{Color, DiagnosticSink, RenderStyle, SourceFile, SourceMap};

/// Result of a successful frontend run.
pub struct FrontendResult {
    pub program: zirk_ast::Program,
    pub checked: zirk_sema::CheckedProgram,
    pub sources: SourceMap,
    pub module_roots: crate::modules::ModuleRoots,
    pub project: Option<crate::modules::ProjectContext>,
}

/// Loads, parses, resolves, and checks the program rooted at `path`.
///
/// On failure it returns `None` and `sink` contains the diagnostics that
/// explain why. On success it returns the merged program and the checked tree.
pub fn run_frontend(path: &Path, sink: &mut DiagnosticSink) -> Option<FrontendResult> {
    let mut loaded = load_target(path, sink)?;
    if sink.has_errors() {
        // Loading recovered enough to continue, but the errors already make the
        // program invalid. Returning None keeps the contract simple: a Some
        // result is one that a backend pass may attempt to lower.
        return None;
    }

    let program = merge(&mut loaded, sink);
    let globals = project_global_specs(loaded.project.as_ref());
    let checked = zirk_sema::check_with_globals(&loaded.sources, &program, &globals, sink);
    if sink.has_errors() {
        return None;
    }

    Some(FrontendResult {
        program,
        checked,
        sources: loaded.sources,
        module_roots: loaded.module_roots,
        project: loaded.project,
    })
}

/// Like [`run_frontend`], but keeps the checked program when the checker
/// reported errors: `zirk check --types` still renders the entries it
/// managed to resolve (spec `zirk-check`, "Type map on a file with
/// errors").
pub fn run_frontend_lenient(path: &Path, sink: &mut DiagnosticSink) -> Option<FrontendResult> {
    run_frontend_lenient_with_overlay(path, &std::collections::HashMap::new(), sink)
}

/// [`run_frontend_lenient`] reading entry text from `overlay` before the
/// filesystem — the language server's entry point, where the open document
/// is the editor's in-memory text rather than the bytes on disk.
pub fn run_frontend_lenient_with_overlay(
    path: &Path,
    overlay: &std::collections::HashMap<PathBuf, String>,
    sink: &mut DiagnosticSink,
) -> Option<FrontendResult> {
    let mut loaded = if path.is_dir() || path.file_name().is_some_and(|name| name == ".zkinit") {
        crate::modules::load_project_with_overlay(path, overlay, sink)?
    } else if crate::project::discover(path).is_some() {
        crate::modules::load_project_entry_with_overlay(path, path, overlay, sink)?
    } else {
        crate::modules::load_with_overlay(path, overlay, sink)?
    };
    if sink.has_errors() {
        return None;
    }

    let program = merge(&mut loaded, sink);
    let globals = project_global_specs(loaded.project.as_ref());
    let checked = zirk_sema::check_with_globals(&loaded.sources, &program, &globals, sink);
    Some(FrontendResult {
        program,
        checked,
        sources: loaded.sources,
        module_roots: loaded.module_roots,
        project: loaded.project,
    })
}

/// Best-effort semantic frontend for editor requests. Unlike the CLI's
/// lenient check mode, it retains a partial snapshot when an imported file or
/// declared package is not resolved yet, so import completion can help the
/// user repair the current line. Project validation, root authorization, and
/// compiler diagnostics are unchanged; non-import loading failures still
/// produce no frontend result.
pub fn run_frontend_ide_with_overlay(
    path: &Path,
    overlay: &std::collections::HashMap<PathBuf, String>,
    sink: &mut DiagnosticSink,
) -> Option<FrontendResult> {
    run_frontend_ide_with_overlay_cancellable(path, overlay, None, sink)
}

/// Editor frontend that can abandon obsolete type checking when the document
/// changes. Module loading is checked at phase boundaries; the semantic
/// checker also polls the shared token during expression and statement walks.
pub fn run_frontend_ide_with_overlay_cancellable(
    path: &Path,
    overlay: &std::collections::HashMap<PathBuf, String>,
    cancellation: Option<Arc<AtomicBool>>,
    sink: &mut DiagnosticSink,
) -> Option<FrontendResult> {
    let mut loaded = if path.is_dir() || path.file_name().is_some_and(|name| name == ".zkinit") {
        crate::modules::load_project_with_overlay(path, overlay, sink)?
    } else if crate::project::discover(path).is_some() {
        crate::modules::load_project_entry_with_overlay(path, path, overlay, sink)?
    } else {
        crate::modules::load_with_overlay(path, overlay, sink)?
    };
    if cancellation
        .as_ref()
        .is_some_and(|token| token.load(Ordering::Relaxed))
    {
        return None;
    }
    let only_unresolved_imports = sink
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == zirk_diagnostics::Severity::Error)
        .all(|diagnostic| {
            diagnostic.code == crate::codes::UNREADABLE_FILE
                || diagnostic.code == crate::codes::PACKAGE_RESOLUTION
        });
    if sink.has_errors() && !only_unresolved_imports {
        return None;
    }

    let program = merge(&mut loaded, sink);
    let globals = project_global_specs(loaded.project.as_ref());
    let checked = match &cancellation {
        Some(token) => zirk_sema::check_with_globals_cancellable(
            &loaded.sources,
            &program,
            &globals,
            token.clone(),
            sink,
        )?,
        None => zirk_sema::check_with_globals(&loaded.sources, &program, &globals, sink),
    };
    if cancellation
        .as_ref()
        .is_some_and(|token| token.load(Ordering::Relaxed))
    {
        return None;
    }
    Some(FrontendResult {
        program,
        checked,
        sources: loaded.sources,
        module_roots: loaded.module_roots,
        project: loaded.project,
    })
}

fn load_target(path: &Path, sink: &mut DiagnosticSink) -> Option<crate::modules::Crate> {
    if path.is_dir() || path.file_name().is_some_and(|name| name == ".zkinit") {
        crate::modules::load_project(path, sink)
    } else if crate::project::discover(path).is_some() {
        crate::modules::load_project_entry(path, path, sink)
    } else {
        crate::modules::load(path, sink)
    }
}

fn project_global_specs(
    project: Option<&crate::modules::ProjectContext>,
) -> Vec<zirk_sema::ProjectGlobalSpec> {
    let Some(project) =
        project.filter(|project| project.manifest.kind == crate::project::ProjectKind::Application)
    else {
        return Vec::new();
    };
    project
        .manifest
        .globals
        .iter()
        .enumerate()
        .map(|(index, (name, global))| zirk_sema::ProjectGlobalSpec {
            name: name.clone(),
            type_name: global.type_name.clone(),
            type_source: global
                .from
                .as_ref()
                .map(|source| project.root.join(source).to_string_lossy().into_owned()),
            readonly: global.readonly,
            has_explicit_default: global.default.is_some(),
            default_function: Some(format!("$zirk.global.default.{index}")),
            // `.zkinit` spans belong to a separate source file and are kept
            // by the manifest parser. The frontend does not yet add that file
            // to the Zirk SourceMap, so the checker uses the entry span for
            // diagnostics until manifest diagnostics move to a shared map.
            span: None,
        })
        .collect()
}

/// Renders the checker's type map as structured JSON — sibling to the
/// diagnostics, on stdout (spec `zirk-check`, "Type-map output mode").
pub fn render_type_map(checked: &zirk_sema::CheckedProgram, sources: &SourceMap) -> String {
    let mut out = String::from("{\"types\":[");
    for (index, entry) in checked.type_map.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let file = sources.file(entry.span.file).name();
        out.push_str(&format!(
            "{{\"file\":\"{}\",\"start\":{},\"end\":{},\"kind\":\"{}\"",
            json_escape(file),
            entry.span.start,
            entry.span.end,
            entry.kind.as_str(),
        ));
        if let Some(name) = &entry.name {
            out.push_str(&format!(",\"name\":\"{}\"", json_escape(name)));
        }
        out.push_str(&format!(
            ",\"type\":\"{}\",\"inferred\":{}}}",
            json_escape(&entry.text),
            entry.inferred,
        ));
    }
    out.push_str("]}");
    out
}

/// Escapes a string for embedding in the JSON `render_type_map` writes —
/// quotes, backslashes and control characters only; the type map's own
/// field names are fixed ASCII.
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Merges the files of a crate into the program the checker sees.
fn merge(loaded: &mut crate::modules::Crate, sink: &mut DiagnosticSink) -> zirk_ast::Program {
    let mut program = zirk_ast::Program {
        package: None,
        imports: Vec::new(),
        modules: Vec::new(),
        module_reexports: Vec::new(),
        uses: Vec::new(),
        enums: Vec::new(),
        classes: Vec::new(),
        contracts: Vec::new(),
        functions: Vec::new(),
        type_aliases: Vec::new(),
        externs: Vec::new(),
        body: Vec::new(),
        span: loaded.sources.entry().span(0, 0),
    };

    let mut initializer_calls = Vec::new();
    for unit in &loaded.units {
        if unit.program.span.file == zirk_diagnostics::FileId::ENTRY {
            program.package = unit.program.package.clone();
        }
        program.imports.extend(unit.program.imports.iter().cloned());
        program.modules.extend(unit.program.modules.iter().cloned());
        program
            .module_reexports
            .extend(unit.program.module_reexports.iter().cloned());
        program.uses.extend(unit.program.uses.iter().cloned());
        program.enums.extend(unit.program.enums.iter().cloned());
        program.classes.extend(unit.program.classes.iter().cloned());
        program
            .contracts
            .extend(unit.program.contracts.iter().cloned());
        program
            .functions
            .extend(unit.program.functions.iter().cloned());
        program
            .type_aliases
            .extend(unit.program.type_aliases.iter().cloned());
        program.externs.extend(unit.program.externs.iter().cloned());
        if !unit.program.body.is_empty()
            && !loaded.type_only_files.contains(&unit.program.span.file)
        {
            let name = format!("$zirk.module.{}", unit.program.span.file.0);
            let span = unit.program.span;
            program.functions.push(zirk_ast::FnDecl {
                name: zirk_ast::Ident::new(name.clone(), span),
                type_params: Vec::new(),
                params: Vec::new(),
                return_type: Some(zirk_ast::TypeRef::new("Void", span)),
                throws: None,
                body: zirk_ast::Block {
                    statements: unit.program.body.clone(),
                    span,
                },
                shared: true,
                is_unsafe: false,
                is_const: false,
                span,
            });
            initializer_calls.push(zirk_ast::Stmt::Expr(zirk_ast::ExprStmt {
                expr: zirk_ast::Expr::Call(zirk_ast::CallExpr {
                    callee: Box::new(zirk_ast::Expr::Path(zirk_ast::Ident::new(name, span))),
                    type_args: Vec::new(),
                    args: Vec::new(),
                    span,
                }),
                span,
            }));
        }
    }

    let entry_span = loaded.sources.entry().span(0, 0);
    program.functions.push(zirk_ast::FnDecl {
        name: zirk_ast::Ident::new(zirk_ast::GENERATED_ENTRYPOINT, entry_span),
        type_params: Vec::new(),
        params: Vec::new(),
        return_type: Some(zirk_ast::TypeRef::new("Void", entry_span)),
        throws: None,
        body: zirk_ast::Block {
            statements: initializer_calls,
            span: entry_span,
        },
        shared: false,
        is_unsafe: false,
        is_const: false,
        span: entry_span,
    });

    append_project_global_defaults(loaded, &mut program, sink);

    program
}

fn append_project_global_defaults(
    loaded: &mut crate::modules::Crate,
    program: &mut zirk_ast::Program,
    sink: &mut DiagnosticSink,
) {
    let Some(project) = loaded.project.as_ref() else {
        return;
    };
    if project.manifest.kind != crate::project::ProjectKind::Application {
        return;
    }
    if !project.manifest.globals.is_empty() {
        let source = loaded.sources.add(SourceFile::new(
            "std.global/types.zrk",
            "share class GlobalSnapshot<T> {\n\
                 public inmut value: T;\n\
                 public inmut revision: UInt64;\n\
             }\n\
             share class GlobalChange<T> {\n\
                 public inmut previous: T;\n\
                 public inmut current: T;\n\
                 public inmut revision: UInt64;\n\
             }\n\
             share enum GlobalWatchError { Lagged, Closed, RuntimeFailure }\n\
             share abstract class GlobalSubscription<T> {\n\
                 private inmut token: UInt64;\n\
                 private inmut snapshot_value: T;\n\
                 private inmut snapshot_revision: UInt64;\n\
             }\n",
        ));
        let file = loaded.sources.file(source);
        let tokens = zirk_lexer::tokenize(file, sink);
        let mut parsed = zirk_parser::parse(file, &tokens, sink);
        program.classes.append(&mut parsed.classes);
        program.enums.append(&mut parsed.enums);
    }
    let globals = project
        .manifest
        .globals
        .iter()
        .enumerate()
        .map(|(index, (name, global))| {
            (
                index,
                name.clone(),
                global.type_name.clone(),
                global.default.clone(),
            )
        })
        .collect::<Vec<_>>();
    let class_sources = class_source_files(&loaded.units);
    let entry_span = loaded.sources.entry().span(0, 0);
    let mut initializer_calls = Vec::new();

    for (index, name, type_name, explicit_default) in globals {
        let expression = explicit_default
            .or_else(|| default_expression_for_type(&type_name, &program.classes, &mut Vec::new()));
        let Some(expression) = expression else {
            // Semantic analysis reports the specific unsupported or ambiguous
            // type default. Do not manufacture a fallback value here.
            continue;
        };
        let source =
            format!("fn __zkinit_global_default(): {type_name} {{ return {expression}; }}\n");
        let file_id = loaded.sources.add(SourceFile::new(
            format!(".zkinit/globals/{index}-{name}.zrk"),
            source,
        ));
        let file = loaded.sources.file(file_id);
        let tokens = zirk_lexer::tokenize(file, sink);
        let mut parsed = zirk_parser::parse(file, &tokens, sink);
        let Some(mut function) = parsed.functions.pop() else {
            continue;
        };
        let initializer_name = format!("$zirk.global.default.{index}");
        function.name = zirk_ast::Ident::new(initializer_name.clone(), entry_span);
        function.span = entry_span;
        if let Some(return_type) = &mut function.return_type {
            let class_name = type_name.split(['<', '?']).next().unwrap_or(&type_name);
            if let Some(source_file) = class_sources.get(class_name) {
                return_type.span.file = *source_file;
            }
        }
        if let Some(zirk_ast::Stmt::Return(statement)) = function.body.statements.first_mut() {
            if let Some(value) = &mut statement.value {
                patch_constructor_source_context(value, &class_sources);
            }
        }
        program.functions.push(function);
        initializer_calls.push(zirk_ast::Stmt::Expr(zirk_ast::ExprStmt {
            expr: zirk_ast::Expr::Call(zirk_ast::CallExpr {
                callee: Box::new(zirk_ast::Expr::Path(zirk_ast::Ident::new(
                    initializer_name,
                    entry_span,
                ))),
                type_args: Vec::new(),
                args: Vec::new(),
                span: entry_span,
            }),
            span: entry_span,
        }));
    }

    if let Some(entry) = program
        .functions
        .iter_mut()
        .find(|function| function.name.name == zirk_ast::GENERATED_ENTRYPOINT)
    {
        initializer_calls.append(&mut entry.body.statements);
        entry.body.statements = initializer_calls;
    }
}

fn class_source_files(
    units: &[crate::modules::Unit],
) -> std::collections::HashMap<String, zirk_diagnostics::FileId> {
    fn add_class(
        class: &zirk_ast::ClassDecl,
        sources: &mut std::collections::HashMap<String, zirk_diagnostics::FileId>,
    ) {
        sources.insert(class.name.name.clone(), class.name.span.file);
        for nested in &class.nested {
            add_class(nested, sources);
        }
    }

    let mut sources = std::collections::HashMap::new();
    for unit in units {
        for class in &unit.program.classes {
            add_class(class, &mut sources);
        }
    }
    sources
}

fn default_expression_for_type(
    type_name: &str,
    classes: &[zirk_ast::ClassDecl],
    active: &mut Vec<String>,
) -> Option<String> {
    let type_name = type_name.trim();
    if type_name.ends_with('?') {
        return Some("null".into());
    }
    if let Some(ty) = zirk_sema::Type::from_name(type_name)
        && ty.has_default()
    {
        return Some(match ty.base {
            zirk_sema::Base::Boolean => "false".into(),
            zirk_sema::Base::String => "\"\"".into(),
            zirk_sema::Base::Int(_) | zirk_sema::Base::Float(_) | zirk_sema::Base::Decimal => {
                "0".into()
            }
            _ => return None,
        });
    }

    let class_name = type_name.split('<').next().unwrap_or(type_name).trim();
    if active.iter().any(|entry| entry == class_name) {
        return None;
    }
    let class = classes
        .iter()
        .find_map(|class| find_class(class, class_name))?;
    active.push(class_name.to_owned());
    let result = if class.constructors.is_empty() {
        Some(format!("{type_name}()"))
    } else {
        let mut eligible = Vec::new();
        for constructor in &class.constructors {
            let mut arguments = Vec::new();
            let mut derivable = true;
            for parameter in &constructor.params {
                if parameter.default.is_some() || parameter.optional {
                    continue;
                }
                let Some(value) = default_expression_for_type(&parameter.ty.name, classes, active)
                else {
                    derivable = false;
                    break;
                };
                arguments.push(format!("{}: {value}", parameter.name.name));
            }
            if derivable {
                eligible.push(format!("{type_name}({})", arguments.join(", ")));
            }
        }
        (eligible.len() == 1).then(|| eligible.remove(0))
    };
    active.pop();
    result
}

fn find_class<'a>(class: &'a zirk_ast::ClassDecl, name: &str) -> Option<&'a zirk_ast::ClassDecl> {
    if class.name.name == name {
        return Some(class);
    }
    class
        .nested
        .iter()
        .find_map(|nested| find_class(nested, name))
}

fn patch_constructor_source_context(
    expression: &mut zirk_ast::Expr,
    class_sources: &std::collections::HashMap<String, zirk_diagnostics::FileId>,
) {
    match expression {
        zirk_ast::Expr::Call(call) => {
            if let zirk_ast::Expr::Path(callee) = &mut *call.callee
                && let Some(source_file) = class_sources.get(&callee.name)
            {
                callee.span.file = *source_file;
            }
            for argument in &mut call.args {
                patch_constructor_source_context(&mut argument.value, class_sources);
            }
        }
        zirk_ast::Expr::Field(field) => {
            patch_constructor_source_context(&mut field.object, class_sources);
        }
        zirk_ast::Expr::Unary(unary) => {
            patch_constructor_source_context(&mut unary.operand, class_sources);
        }
        zirk_ast::Expr::Binary(binary) => {
            patch_constructor_source_context(&mut binary.left, class_sources);
            patch_constructor_source_context(&mut binary.right, class_sources);
        }
        zirk_ast::Expr::Ternary(ternary) => {
            patch_constructor_source_context(&mut ternary.condition, class_sources);
            patch_constructor_source_context(&mut ternary.when_true, class_sources);
            patch_constructor_source_context(&mut ternary.when_false, class_sources);
        }
        _ => {}
    }
}

/// Renders the accumulated diagnostics.
pub fn render(sink: &DiagnosticSink, json: bool, color: Color) -> String {
    let style = if json {
        RenderStyle::Json
    } else {
        RenderStyle::Human
    };
    sink.render_colored(style, color)
}

/// Resolves a source argument to the path the compiler reads.
///
/// If the argument already ends with `.zrk`, it is used verbatim. Otherwise,
/// `.zrk` is appended. This lets `zirk run main` resolve to `main.zrk` without
/// any ambiguity.
pub fn resolve_source_path(arg: &str) -> PathBuf {
    if arg.ends_with(".zrk") {
        PathBuf::from(arg)
    } else {
        PathBuf::from(format!("{arg}.zrk"))
    }
}

/// Decides whether the diagnostics carry colour.
///
/// `ZIRK_COMPILER_SPEC.md` section 9 requires deterministic output, so colour
/// only appears when the destination is a terminal a person is reading. Piping
/// the output must yield exactly the same bytes as an uncoloured run.
pub fn resolve_color(requested: Option<&str>) -> Color {
    match requested {
        Some("always") => return Color::Ansi,
        Some("never") => return Color::Never,
        _ => {}
    }

    if std::env::var_os("NO_COLOR").is_some() {
        return Color::Never;
    }

    // Standard error and not output: that is where diagnostics go.
    if std::io::stderr().is_terminal() {
        Color::Ansi
    } else {
        Color::Never
    }
}
