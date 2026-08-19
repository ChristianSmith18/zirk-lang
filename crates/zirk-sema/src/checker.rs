//! Name resolution, type checking and flow analysis.
//!
//! The checker does not stop at the first error: it assigns [`Type::UNKNOWN`]
//! to whatever it cannot determine and keeps going, so one compilation reports
//! every problem instead of one per run. `Unknown` is compatible with
//! everything, which is what prevents one real error from producing a dozen
//! derived ones.

use crate::codes;
use crate::scope::{Binding, ParamInfo, Scopes, Signature};
use crate::types::{
    AssociatedFieldInfo, Base, ClassType, ContractMethod, ContractType, EnumType, EnumVariantInfo,
    FieldInfo, FloatWidth, FnType, GenericContractInstance, GenericEnumInstance, GenericInstance,
    IntWidth, MethodInfo, Type, TypeNames, TypeParamInfo, describe, pending_type,
};
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;
use zirk_ast::*;
use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceMap, Span};

/// The result of checking, for later stages.
#[derive(Debug, Default, Clone)]
pub struct CheckedProgram {
    pub functions: HashMap<String, Signature>,
    /// Declared enums, indexed by the id their [`Base::Enum`] carries.
    pub enums: Vec<EnumType>,
    /// Declared classes, indexed by the id their [`Base::Class`] carries.
    pub classes: Vec<ClassType>,
    /// Declared contracts, indexed by the id their [`Base::Contract`] carries.
    pub contracts: Vec<ContractType>,
    /// Function types, indexed by the id their [`Base::Function`] carries.
    pub fn_types: Vec<FnType>,
    /// What each lambda captures, keyed by the lambda's span.
    ///
    /// Lowering needs this to build the environment, and a span identifies a
    /// lambda uniquely without inventing a numbering the parser would have to
    /// maintain.
    pub lambdas: HashMap<Span, LambdaInfo>,
    /// Names written under an import alias, keyed by the use site.
    ///
    /// `import { Role -> DomainRole }` lets a file write `DomainRole` for a
    /// declaration called `Role`. Recording the resolution here means lowering
    /// does not have to repeat it — or know that aliases exist at all.
    pub aliases: HashMap<Span, String>,
    /// The resolved type of each `match` scrutinee, keyed by the match's span.
    ///
    /// Lowering compares against a discriminant for enums and against the value
    /// itself otherwise, and that choice is made here.
    pub matches: HashMap<Span, Type>,
    /// Member accesses that turned out to be enum variants, by their span.
    ///
    /// `Direction.North` and `user.name` are the same shape to the parser, and
    /// which one it is depends on whether the base names a type or a value.
    /// Recording the answer here means lowering resolves it once, here, rather
    /// than repeating the decision with the same tables.
    pub variant_accesses: std::collections::HashSet<Span>,
    /// Declared generic type parameters, indexed by the id their
    /// [`Base::Param`] carries.
    pub type_params: Vec<TypeParamInfo>,
    /// Interned generic instantiations, indexed by the id their
    /// [`Base::Instance`] carries.
    pub generic_instances: Vec<GenericInstance>,
    /// Interned generic contract instantiations, indexed by the id their
    /// [`Base::ContractInstance`] carries.
    pub contract_instances: Vec<GenericContractInstance>,
    /// Interned generic enum instantiations, indexed by the id their
    /// [`Base::EnumInstance`] carries.
    pub enum_instances: Vec<GenericEnumInstance>,
    /// Interned unions, indexed by the id their [`Base::Union`] carries.
    pub unions: Vec<Vec<Base>>,
    /// Which `Base::Instance` a generic class's construction call resolved
    /// to, keyed by the call's span — an id into `generic_instances`.
    ///
    /// Lowering derives every other type from the shape of the AST plus
    /// these tables, but a construction call's own instantiation is inferred
    /// from its arguments (task 7.6's mechanism) and lives nowhere else: the
    /// callee's bare name alone (`Box`) cannot tell `Box<Int32>` from
    /// `Box<String>` apart the way an annotation or a field's resolved type
    /// can (roadmap task 11.1).
    pub generic_constructions: std::collections::HashMap<Span, u32>,
    /// Which `Iteration<T>` instantiation a `for ... in` loop over a type's
    /// own `Iterable<T>` resolves to, keyed by the iterable expression's
    /// span — an id into `enum_instances` (roadmap task 13.5).
    ///
    /// The element `T` is a class's own `contract_instances` entry, not
    /// anything a written annotation names, so nothing else records which
    /// concrete `Iteration<T>` a given loop needs — the same reason a
    /// generic construction's instantiation needs `generic_constructions`.
    /// Absent for a loop whose iterable is a range: those need none of the
    /// `iterator()`/`next()` protocol.
    pub for_in_iteration: std::collections::HashMap<Span, u32>,
    /// Which `Base::EnumInstance` a generic enum's variant construction
    /// resolved to (`Iteration.Item(value: v)`), keyed by the call's span —
    /// an id into `enum_instances`. The `implements`/construction
    /// equivalent for a generic enum's own `T`, the same reason
    /// `generic_constructions` exists for a generic class's.
    pub variant_constructions: std::collections::HashMap<Span, u32>,
}

/// What the checker learned about one lambda.
#[derive(Debug, Clone)]
pub struct LambdaInfo {
    /// Names captured from the enclosing scope, in a stable order.
    pub captures: Vec<Capture>,
    pub fn_type: u32,
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub name: String,
    pub ty: Type,
}

/// Checks a program, accumulating diagnostics in the sink.
pub fn check(sources: &SourceMap, program: &Program, sink: &mut DiagnosticSink) -> CheckedProgram {
    Checker::new(sources, sink).run(program)
}

/// Names types for diagnostics, given the checker's tables.
struct Names<'t> {
    enums: &'t [EnumType],
    fn_types: &'t [FnType],
    classes: &'t [ClassType],
    contracts: &'t [ContractType],
    type_params: &'t [TypeParamInfo],
    generic_instances: &'t [GenericInstance],
    contract_instances: &'t [GenericContractInstance],
    enum_instances: &'t [GenericEnumInstance],
    unions: &'t [Vec<Base>],
}

impl TypeNames for Names<'_> {
    fn enum_name(&self, id: u32) -> String {
        self.enums
            .get(id as usize)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "<enum>".into())
    }

    fn class_name(&self, id: u32) -> String {
        self.classes
            .get(id as usize)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "<class>".into())
    }

    fn contract_name(&self, id: u32) -> String {
        self.contracts
            .get(id as usize)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "<contract>".into())
    }

    fn function_type(&self, id: u32) -> String {
        let Some(f) = self.fn_types.get(id as usize) else {
            return "<function>".into();
        };
        let params: Vec<String> = f.params.iter().map(|t| describe(*t, self)).collect();
        format!("({}) => {}", params.join(", "), describe(f.returns, self))
    }

    fn type_param_name(&self, id: u32) -> String {
        self.type_params
            .get(id as usize)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "<type parameter>".into())
    }

    fn instance_name(&self, id: u32) -> String {
        let Some(instance) = self.generic_instances.get(id as usize) else {
            return "<generic instance>".into();
        };
        let class = self.class_name(instance.class);
        let args: Vec<String> = instance.args.iter().map(|t| describe(*t, self)).collect();
        format!("{class}<{}>", args.join(", "))
    }

    fn contract_instance_name(&self, id: u32) -> String {
        let Some(instance) = self.contract_instances.get(id as usize) else {
            return "<generic contract instance>".into();
        };
        let contract = self.contract_name(instance.contract);
        let args: Vec<String> = instance.args.iter().map(|t| describe(*t, self)).collect();
        format!("{contract}<{}>", args.join(", "))
    }

    fn enum_instance_name(&self, id: u32) -> String {
        let Some(instance) = self.enum_instances.get(id as usize) else {
            return "<generic enum instance>".into();
        };
        let enum_name = self.enum_name(instance.enum_id);
        let args: Vec<String> = instance.args.iter().map(|t| describe(*t, self)).collect();
        format!("{enum_name}<{}>", args.join(", "))
    }

    fn union_name(&self, id: u32) -> String {
        let Some(members) = self.unions.get(id as usize) else {
            return "<union>".into();
        };
        let names: Vec<String> = members
            .iter()
            .map(|&base| describe(Type::of(base), self))
            .collect();
        names.join(" | ")
    }
}

struct Checker<'a> {
    sources: &'a SourceMap,
    sink: &'a mut DiagnosticSink,
    scopes: Scopes,
    functions: HashMap<String, Signature>,
    enums: Vec<EnumType>,
    classes: Vec<ClassType>,
    contracts: Vec<ContractType>,
    fn_types: Vec<FnType>,
    lambdas: HashMap<Span, LambdaInfo>,
    matches: HashMap<Span, Type>,
    variant_accesses: std::collections::HashSet<Span>,
    /// Per file, the names it imported: bound name to original name.
    imported: HashMap<zirk_diagnostics::FileId, HashMap<String, String>>,
    /// Use sites whose written name differs from the declaration's.
    aliases: HashMap<Span, String>,
    /// Return type of the function or lambda being checked.
    current_return: Type,
    /// The class whose body is being checked, if any. It is what `this` names.
    this_type: Option<Type>,
    /// Whether the body being checked is a constructor, which is the one place
    /// an `inmut` field may be written.
    in_constructor: bool,
    /// How many loops enclose the statement being checked.
    ///
    /// Zero means `break` and `continue` have nothing to jump out of.
    loop_depth: u32,
    /// Captures collected for the lambda being checked, innermost last.
    capture_stack: Vec<Vec<Capture>>,
    /// Declared generic type parameters, indexed by the id their
    /// [`Base::Param`] carries.
    type_params: Vec<TypeParamInfo>,
    /// Type-parameter names in scope, innermost last: a class's own frame,
    /// then a generic method's or function's on top of it.
    type_param_scope: Vec<HashMap<String, u32>>,
    /// The id already minted for a `TypeParam`, keyed by its span.
    ///
    /// A class's or function's signature is resolved twice — once to declare
    /// it, once to check its body — and each pass calls
    /// [`Self::enter_type_params`] with the same AST list. Its span is stable
    /// across both calls, so this is what makes the second pass reuse `T`'s id
    /// instead of minting a second, unrelated `T`.
    type_param_ids: HashMap<Span, u32>,
    /// Interned generic instantiations, indexed by the id their
    /// [`Base::Instance`] carries.
    generic_instances: Vec<GenericInstance>,
    /// Interned generic contract instantiations, indexed by the id their
    /// [`Base::ContractInstance`] carries.
    contract_instances: Vec<GenericContractInstance>,
    /// Interned generic enum instantiations, indexed by the id their
    /// [`Base::EnumInstance`] carries.
    enum_instances: Vec<GenericEnumInstance>,
    /// `type` aliases, by name, holding the target written rather than a
    /// resolved `Type`: an alias is transparent (`Base::Class` and friends
    /// already say what it means), so there is nothing of its own to store —
    /// resolving a reference to it just resolves `target` instead, lazily,
    /// the same way a forward-referenced class does.
    type_aliases: HashMap<String, TypeRef>,
    /// Aliases currently being resolved, to catch `type A = B; type B = A;`
    /// before it recurses forever.
    resolving_aliases: Vec<String>,
    /// Interned unions, indexed by the id their [`Base::Union`] carries —
    /// already normalized: sorted, deduplicated, subsumed alternatives
    /// removed.
    unions: Vec<Vec<Base>>,
    /// See [`CheckedProgram::generic_constructions`].
    generic_constructions: HashMap<Span, u32>,
    /// Ids of the language's own `Iterable<T>` and `Iterator<T>` contracts
    /// and `Iteration<T>` enum, minted by
    /// [`Self::register_native_iteration_contracts`] before anything in the
    /// program is declared (task 6.9, D8).
    native_iteration: Option<NativeIteration>,
    /// See [`CheckedProgram::for_in_iteration`].
    for_in_iteration: HashMap<Span, u32>,
    /// See [`CheckedProgram::variant_constructions`].
    variant_constructions: HashMap<Span, u32>,
}

/// Ids of the contracts and enum `for ... in` checks a type against, once
/// [`Checker::register_native_iteration_contracts`] has run.
///
/// `iterable` finds `T` for `for ... in` (task 6.9); `iteration` finds which
/// `Iteration<T>` a given loop needs (roadmap task 13.5). `iterator` is not
/// read outside registration: lowering finds the `Iterator` contract by name
/// instead, the same way it finds `Iterable`/`Iteration`.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct NativeIteration {
    iterable: u32,
    iterator: u32,
    iteration: u32,
}

impl<'a> Checker<'a> {
    fn new(sources: &'a SourceMap, sink: &'a mut DiagnosticSink) -> Self {
        Self {
            sources,
            sink,
            scopes: Scopes::new(),
            functions: HashMap::new(),
            enums: Vec::new(),
            classes: Vec::new(),
            contracts: Vec::new(),
            fn_types: Vec::new(),
            lambdas: HashMap::new(),
            matches: HashMap::new(),
            variant_accesses: std::collections::HashSet::new(),
            imported: HashMap::new(),
            aliases: HashMap::new(),
            current_return: Type::VOID,
            this_type: None,
            in_constructor: false,
            loop_depth: 0,
            capture_stack: Vec::new(),
            type_params: Vec::new(),
            type_param_scope: Vec::new(),
            type_param_ids: HashMap::new(),
            generic_instances: Vec::new(),
            contract_instances: Vec::new(),
            enum_instances: Vec::new(),
            type_aliases: HashMap::new(),
            resolving_aliases: Vec::new(),
            unions: Vec::new(),
            generic_constructions: HashMap::new(),
            native_iteration: None,
            for_in_iteration: HashMap::new(),
            variant_constructions: HashMap::new(),
        }
    }

    // --- Diagnostics ------------------------------------------------------

    fn error(
        &mut self,
        code: Code,
        span: Span,
        message: impl Into<String>,
        cause: impl Into<String>,
        help: Option<String>,
    ) {
        let mut d = Diagnostic::error(code, message)
            .at(self.sources.location(span))
            .with_snippet(self.sources.snippet(span))
            .with_cause(cause);
        if let Some(help) = help {
            d = d.with_help(help);
        }
        self.sink.emit(d);
    }

    /// Renders a type for a diagnostic.
    fn name(&self, ty: Type) -> String {
        describe(
            ty,
            &Names {
                enums: &self.enums,
                fn_types: &self.fn_types,
                classes: &self.classes,
                contracts: &self.contracts,
                type_params: &self.type_params,
                generic_instances: &self.generic_instances,
                contract_instances: &self.contract_instances,
                enum_instances: &self.enum_instances,
                unions: &self.unions,
            },
        )
    }

    // --- Program ----------------------------------------------------------

    fn run(mut self, program: &Program) -> CheckedProgram {
        self.register_native_iteration_contracts();
        self.record_imports(program);
        self.declare_type_aliases(program);

        // Contracts before classes: a class says which it satisfies, and a
        // field or signature may name one.
        for c in &program.contracts {
            self.declare_contract(c);
        }

        // A class's own name (and how many type parameters it takes) is
        // registered before enums resolve their associated field types
        // below: an algebraic variant may carry a class as its payload
        // (`Event.Login(user: User)`), the same way a class's own field may
        // name an enum. `register_class` only mints the name and arity here
        // — nothing that resolves a type, so nothing about it needs enums,
        // contracts or another class's own members to exist yet.
        for c in &program.classes {
            self.register_class(c);
        }

        // Enums next: a class's own signature may name one below, and now a
        // class is nameable from an enum's associated data too.
        for e in &program.enums {
            self.declare_enum(e);
        }

        // Classes' members last, in three passes, because a class may
        // extend one declared later in the file and its members depend on
        // its base's.
        self.resolve_bases(program);
        self.resolve_class_type_param_constraints(program);
        self.declare_members(program);
        self.check_generic_class_lowering(program);
        self.check_conformance(program);

        // Signatures next, so a function can call another declared later.
        for f in &program.functions {
            self.declare_function(f);
        }

        self.check_entrypoint(program);

        self.scopes.push();
        for c in &program.classes {
            self.check_class(c);
        }
        for f in &program.functions {
            self.check_function(f);
        }
        self.scopes.pop();

        CheckedProgram {
            functions: self.functions,
            enums: self.enums,
            classes: self.classes,
            contracts: self.contracts,
            fn_types: self.fn_types,
            lambdas: self.lambdas,
            matches: self.matches,
            variant_accesses: self.variant_accesses,
            aliases: self.aliases,
            type_params: self.type_params,
            generic_instances: self.generic_instances,
            contract_instances: self.contract_instances,
            enum_instances: self.enum_instances,
            unions: self.unions,
            generic_constructions: self.generic_constructions,
            for_in_iteration: self.for_in_iteration,
            variant_constructions: self.variant_constructions,
        }
    }

    /// Notes which names each file brought in with `import`.
    ///
    /// A name is visible in a file when it was declared there, or when it is
    /// `share`d and that file imported it. Both halves are needed: importing
    /// something private must fail, and so must naming something shared that
    /// was never imported.
    fn record_imports(&mut self, program: &Program) {
        for import in &program.imports {
            for name in &import.names {
                // Standard modules are recorded too. They name no declaration
                // of the crate, so they never reach the visibility check, but
                // `use` does ask whether the file imported the name.
                self.imported
                    .entry(import.span.file)
                    .or_default()
                    .insert(name.bound_name().name.clone(), name.name.name.clone());
            }
        }

        // `use` only enables globals already brought in; naming something it
        // never imported is what the requirement rejects.
        for use_decl in &program.uses {
            let known = self
                .imported
                .get(&use_decl.span.file)
                .is_some_and(|names| names.contains_key(&use_decl.name.name));

            if !known && Type::from_name(&use_decl.name.name).is_none() {
                self.error(
                    codes::UNDECLARED_NAME,
                    use_decl.name.span,
                    format!("`{}` is not available in this file", use_decl.name.name),
                    "`use` enables a name the file already imported",
                    Some(format!(
                        "add `import {{ {} }} from ...` first",
                        use_decl.name.name
                    )),
                );
            }
        }
    }

    /// Where a previous declaration lives, naming its file when it is another.
    ///
    /// A line number alone is confusing across files: "line 1" says nothing
    /// when the two declarations are in different ones.
    fn declared_at(&self, previous: Span, current: Span) -> String {
        let line = self.sources.location(previous).line;
        if previous.file == current.file {
            return format!("on line {line}");
        }

        let file = self.sources.file(previous.file).name().to_string();
        format!("on line {line} of `{file}`")
    }

    /// Declares a local, rejecting it first if it would hide one still
    /// visible (D10): there is no ordinary shadowing, in a nested block, a
    /// redeclaration in the same one, a parameter, a `for ... in` binding or
    /// a `match` pattern's own — every local goes through here. A field
    /// never triggers this: it is always read as `this.name`, never a bare
    /// name `Scopes` holds, so there is nothing for a local to shadow.
    fn declare_local(&mut self, binding: Binding) {
        if let Some(resolution) = self.scopes.resolve(&binding.name) {
            let where_ = self.declared_at(resolution.binding.span, binding.span);
            self.error(
                codes::ORDINARY_SHADOWING,
                binding.span,
                format!(
                    "`{}` shadows a variable from an enclosing scope",
                    binding.name
                ),
                format!(
                    "a variable named `{}` is already declared {where_}",
                    binding.name
                ),
                Some("rename this declaration, or read the outer value before it is hidden".into()),
            );
        }
        self.scopes.declare(binding);
    }

    /// The declaration a name refers to, following the file's import aliases.
    ///
    /// `import { Role -> DomainRole }` binds `DomainRole` in the importing
    /// file, but the declaration is still called `Role`: every lookup goes
    /// through here so the alias is applied in exactly one place.
    fn resolved_name(&mut self, name: &str, at: Span) -> String {
        let Some(original) = self
            .imported
            .get(&at.file)
            .and_then(|names| names.get(name))
            .cloned()
        else {
            return name.to_string();
        };

        if original != name {
            self.aliases.insert(at, original.clone());
        }
        original
    }

    /// Whether a value of this type can be turned into text through
    /// `to_string()` (`ZIRK_STDLIB_SPEC.md` section 3, roadmap Phase 3b task
    /// 8).
    ///
    /// Every native scalar except `Float16`/`Float128` implicitly satisfies
    /// it — those two have no stable Rust primitive the runtime can format
    /// through yet (`zirk-runtime/src/string.rs`'s own doc comment on the
    /// gap), consistent with `Float128` arithmetic's own tracked portability
    /// gap on Windows. A class, record or value class satisfies it the same
    /// way it supplies any other operator (decision D6): by declaring the
    /// method itself, `fn to_string(): String`, checked structurally rather
    /// than through a registered contract — the same shape `native_arithmetic`'s
    /// reserved-method dispatch (`_add`, `_subtract`, …) already uses.
    fn is_printable(&self, ty: Type) -> bool {
        match ty.base {
            Base::Int(_) | Base::Boolean | Base::String | Base::Char => true,
            Base::Float(w) => matches!(w, FloatWidth::F32 | FloatWidth::F64),
            Base::Class(id) => self.classes[id as usize]
                .method("to_string")
                .is_some_and(|m| m.params.is_empty() && m.returns == Type::STRING),
            _ => false,
        }
    }

    /// Reports printing a value the runtime cannot turn into text.
    fn require_printable(&mut self, ty: Type, span: Span) {
        if ty.is_unknown() {
            return;
        }
        if !ty.nullable && self.is_printable(ty) {
            return;
        }

        let reason = match ty.base {
            _ if ty.nullable => "a value that may be absent has no text form",
            Base::Enum(_) => "an enum has no text for its variants yet",
            Base::Function(_) => "a closure is code, not data",
            Base::Void => "`Void` is the absence of a value",
            Base::Float(_) => {
                "Float16/Float128 have no stable conversion to text yet (roadmap Phase 3b, task 8.3)"
            }
            _ => "the runtime has no text form for it",
        };

        let name = self.name(ty);
        let help = if ty.nullable {
            "use `?? <fallback>` to provide a value to print"
        } else {
            "implement `fn to_string(): String` for a class, record or value class"
        };

        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("`{name}` cannot be printed"),
            reason,
            Some(help.to_string()),
        );
    }

    /// Reports naming a declaration that another file keeps to itself.
    ///
    /// Visibility is binary in this phase: a declaration is private to its file
    /// unless marked `share`. The three levels of
    /// `ZIRK_LANGUAGE_SPEC.md` section 7 depend on classes and are Phase 3.
    fn require_visible(&mut self, owner: Span, shared: bool, used: &Ident, what: &str) {
        if owner.file == used.span.file {
            return;
        }

        let keyword = if what == "enum" { "enum" } else { "fn" };
        let file = self.sources.file(owner.file).name().to_string();
        let line = self.sources.location(owner).line;

        if !shared {
            self.error(
                codes::UNDECLARED_NAME,
                used.span,
                format!("{what} `{}` is not accessible from this file", used.name),
                format!("it is declared on line {line} of `{file}` without `share`"),
                Some(format!(
                    "mark it `share {keyword} {}` to publish it",
                    used.name
                )),
            );
            return;
        }

        // Being shared is not enough: the file has to have asked for it.
        // Otherwise every `share` in the crate would be in scope everywhere,
        // and `import` would be decoration.
        let imported = self
            .imported
            .get(&used.span.file)
            .is_some_and(|names| names.contains_key(&used.name));

        if !imported {
            self.error(
                codes::UNDECLARED_NAME,
                used.span,
                format!("`{}` is not imported in this file", used.name),
                format!("it is declared on line {line} of `{file}`, but nothing brings it here"),
                Some(format!(
                    "add `import {{ {} }} from \"...\";` naming that file",
                    used.name
                )),
            );
        }
    }

    /// Registers a class as a nominal type, with its fields and constructors.
    ///
    /// Declaration comes before checking so a field, a parameter or another
    /// class may name it regardless of the order the file declares them in.
    /// Whether a name is one of the language's own types.
    ///
    /// Declaring a class with it is how application code would try to reopen a
    /// native type, and `ZIRK_LANGUAGE_SPEC.md` section 4 forbids exactly that:
    /// a program cannot replace what `String` or `Int32` mean.
    fn is_native_type_name(name: &str) -> bool {
        Type::from_name(name).is_some() || pending_type(name).is_some()
    }

    /// Registers `Iterable<T>`, `Iterator<T>` and `Iteration<T>` as if they
    /// were declared by the language itself (D8, task 6.9): `for ... in`
    /// requires `Iterable<T>`, and this is what lets a user's own type
    /// implement it exactly like a range or `String` would, rather than the
    /// closed set the Phase 2 subset had.
    ///
    /// Injected directly into the tables instead of parsed from source, so
    /// registering them never reports anything on its own — [`Self::declare_contract`]
    /// and [`Self::declare_enum`]'s `NOT_LOWERED` gate for a *user's* generic
    /// declaration would otherwise fire for every program, whether it ever
    /// names these or not. What is genuinely not lowered — dispatching
    /// through a contract's table (roadmap task 10.7) and a generic enum with
    /// associated data (11.3) — is gated where a program actually names a
    /// concrete instantiation, in [`Self::resolve_contract_reference`] and
    /// [`Self::resolve_enum_reference`].
    fn register_native_iteration_contracts(&mut self) {
        let at = Span::empty(0);

        let iterable_t = self.type_params.len() as u32;
        self.type_params.push(TypeParamInfo {
            name: "T".into(),
            constraints: Vec::new(),
            span: at,
        });
        let iterable = self.contracts.len() as u32;
        self.contracts.push(ContractType {
            name: "Iterable".into(),
            kind: ContractKind::Interface,
            methods: Vec::new(),
            type_params: vec![iterable_t],
            shared: true,
            span: at,
        });

        let iterator_t = self.type_params.len() as u32;
        self.type_params.push(TypeParamInfo {
            name: "T".into(),
            constraints: Vec::new(),
            span: at,
        });
        let iterator = self.contracts.len() as u32;
        self.contracts.push(ContractType {
            name: "Iterator".into(),
            kind: ContractKind::Interface,
            methods: Vec::new(),
            type_params: vec![iterator_t],
            shared: true,
            span: at,
        });

        let iteration_t = self.type_params.len() as u32;
        self.type_params.push(TypeParamInfo {
            name: "T".into(),
            constraints: Vec::new(),
            span: at,
        });
        let iteration = self.enums.len() as u32;
        self.enums.push(EnumType {
            name: "Iteration".into(),
            variants: vec![
                EnumVariantInfo {
                    name: "Item".into(),
                    associated: vec![AssociatedFieldInfo {
                        name: "value".into(),
                        ty: Type::of(Base::Param(iteration_t)),
                    }],
                    span: at,
                },
                EnumVariantInfo {
                    name: "Done".into(),
                    associated: Vec::new(),
                    span: at,
                },
            ],
            type_params: vec![iteration_t],
            shared: true,
            span: at,
        });

        // `iterator(): Iterator<T>`
        let returns_iterator = self.intern_contract_instance(GenericContractInstance {
            contract: iterator,
            args: vec![Type::of(Base::Param(iterable_t))],
        });
        self.contracts[iterable as usize]
            .methods
            .push(ContractMethod {
                name: "iterator".into(),
                params: Vec::new(),
                returns: Type::of(Base::ContractInstance(returns_iterator)),
                span: at,
                has_default: false,
                index: 0,
            });

        // `next(): Iteration<T>`
        let returns_iteration = self.intern_enum_instance(GenericEnumInstance {
            enum_id: iteration,
            args: vec![Type::of(Base::Param(iterator_t))],
        });
        self.contracts[iterator as usize]
            .methods
            .push(ContractMethod {
                name: "next".into(),
                params: Vec::new(),
                returns: Type::of(Base::EnumInstance(returns_iteration)),
                span: at,
                has_default: false,
                index: 0,
            });

        self.native_iteration = Some(NativeIteration {
            iterable,
            iterator,
            iteration,
        });
    }

    /// Registers a contract with its method signatures.
    fn declare_contract(&mut self, decl: &ContractDecl) {
        if Self::is_native_contract_name(&decl.name.name) {
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("`{}` is a contract of the language", decl.name.name),
                "application code cannot reopen a native contract or replace what it means",
                Some("pick a different name".into()),
            );
            return;
        }

        if let Some(previous) = self.contracts.iter().find(|c| c.name == decl.name.name) {
            let where_ = self.declared_at(previous.span, decl.name.span);
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("`{}` is already defined", decl.name.name),
                format!("a previous definition exists {where_}"),
                Some("rename one of the two".into()),
            );
            return;
        }

        let type_params = self.enter_type_params(&decl.type_params);
        let mut methods: Vec<ContractMethod> = Vec::new();
        for (index, method) in decl.methods.iter().enumerate() {
            if let Some(previous) = methods.iter().find(|m| m.name == method.name.name) {
                let where_ = self.declared_at(previous.span, method.name.span);
                self.error(
                    codes::DUPLICATE_DECLARATION,
                    method.name.span,
                    format!("method `{}` is already defined", method.name.name),
                    format!("a previous definition exists {where_}"),
                    Some("rename one of the two: there is no overloading".into()),
                );
                continue;
            }

            methods.push(ContractMethod {
                name: method.name.name.clone(),
                params: method
                    .params
                    .iter()
                    .map(|p| self.resolve_param(p))
                    .collect(),
                returns: self.resolve_type(&method.return_type),
                span: method.name.span,
                has_default: method.body.is_some(),
                index,
            });
        }

        self.leave_type_params();

        self.contracts.push(ContractType {
            name: decl.name.name.clone(),
            kind: decl.kind,
            methods,
            type_params,
            shared: decl.shared,
            span: decl.name.span,
        });
    }

    /// Whether `name` is one of the contracts the language itself registers
    /// (task 6.9): `Iterable` and `Iterator`, injected directly into the
    /// tables rather than parsed, so application code cannot reopen them.
    fn is_native_contract_name(name: &str) -> bool {
        matches!(name, "Iterable" | "Iterator")
    }

    fn contract_id(&self, name: &str) -> Option<u32> {
        self.contracts
            .iter()
            .position(|c| c.name == name)
            .map(|i| i as u32)
    }

    /// Verifies that every class satisfies what it says it does.
    fn check_conformance(&mut self, program: &Program) {
        for decl in &program.classes {
            let Some(id) = self.class_id(&decl.name.name) else {
                continue;
            };

            // A record or value class has no descriptor to carry a contract's
            // table (roadmap task 11.5, design.md's open question on virtual
            // methods): its own methods dispatch statically and lower today
            // (`Self::method_of` accepts `IrType::Value` the same way it does
            // `IrType::Object`), but reaching one through the contract it
            // implements — the only reason dynamic dispatch would matter for
            // a type with no identity — needs a vtable no value carries.
            // Conformance is still checked below, the same way it is for an
            // abstract class's requirements: what is missing is only the
            // path from a contract-typed reference back to the value.
            if matches!(decl.kind, ClassKind::Record | ClassKind::ValueClass)
                && !decl.implements.is_empty()
            {
                self.not_lowered(
                    decl.name.span,
                    &format!("a {} that implements a contract", decl.kind.as_str()),
                    "call its methods directly on the concrete type for now, without naming the contract as its type",
                );
            }

            // A class satisfies what its base satisfies: that is what makes a
            // subclass usable wherever the base was.
            let mut satisfied: Vec<u32> = self.classes[id as usize]
                .base
                .map(|b| self.classes[b as usize].contracts.clone())
                .unwrap_or_default();
            let mut abstract_bases: Vec<u32> = self.classes[id as usize]
                .base
                .map(|b| self.classes[b as usize].abstract_bases.clone())
                .unwrap_or_default();
            let mut contract_instances: Vec<u32> = self.classes[id as usize]
                .base
                .map(|b| self.classes[b as usize].contract_instances.clone())
                .unwrap_or_default();

            for named in &decl.implements {
                let resolved = self.resolved_name(&named.name, named.span);

                if let Some(contract) = self.contract_id(&resolved) {
                    let declared = self.contracts[contract as usize].span;
                    let shared = self.contracts[contract as usize].shared;
                    let ident = Ident::new(named.name.clone(), named.span);
                    self.require_visible(declared, shared, &ident, "contract");

                    if satisfied.contains(&contract) {
                        let name = self.contracts[contract as usize].name.clone();
                        self.error(
                            codes::DUPLICATE_DECLARATION,
                            named.span,
                            format!("`{name}` is listed more than once"),
                            "a class satisfies a contract once, its base's included",
                            None,
                        );
                        continue;
                    }

                    // A generic contract (task 6.9, `Iterable<T>` chief among
                    // them): the type arguments replace its own `<T>` before
                    // comparing what the class wrote against what is
                    // required, the same way `Box<Int32>` would for a class.
                    let (subst, instance) = self.resolve_implements_args(contract, named);
                    self.require_conformance(id, contract, named.span, &subst);
                    satisfied.push(contract);
                    if let Some(instance) = instance {
                        contract_instances.push(instance);
                    }
                    continue;
                }

                if let Some(abstract_id) = self
                    .classes
                    .iter()
                    .position(|c| c.name == resolved && c.kind == ClassKind::Abstract)
                    .map(|i| i as u32)
                {
                    let declared = self.classes[abstract_id as usize].span;
                    let shared = self.classes[abstract_id as usize].shared;
                    let ident = Ident::new(named.name.clone(), named.span);
                    self.require_visible(declared, shared, &ident, "abstract class");
                    self.reject_type_arguments(named);

                    if abstract_bases.contains(&abstract_id) {
                        let name = self.classes[abstract_id as usize].name.clone();
                        self.error(
                            codes::DUPLICATE_DECLARATION,
                            named.span,
                            format!("`{name}` is listed more than once"),
                            "a class adopts an abstract class once, its base's included",
                            None,
                        );
                        continue;
                    }

                    self.require_abstract_conformance(id, abstract_id, named.span, decl);
                    abstract_bases.push(abstract_id);
                    continue;
                }

                self.error(
                    codes::UNKNOWN_TYPE,
                    named.span,
                    format!(
                        "`{}` is not a declared contract or abstract class",
                        named.name
                    ),
                    "`implements` names an interface, a trait or an abstract class",
                    None,
                );
            }

            self.classes[id as usize].contracts = satisfied;
            self.classes[id as usize].abstract_bases = abstract_bases;
            self.classes[id as usize].contract_instances = contract_instances;
        }
    }

    /// Resolves the type arguments an `implements` clause writes for a
    /// generic contract, returning the substitution its type parameter(s)
    /// need before comparing what the class supplies against what is
    /// required (task 6.9), plus the interned instantiation id — the
    /// `implements` equivalent of [`Self::resolve_contract_reference`].
    fn resolve_implements_args(
        &mut self,
        contract: u32,
        reference: &TypeRef,
    ) -> (Vec<(u32, Type)>, Option<u32>) {
        let expected = self.contracts[contract as usize].type_params.clone();

        if expected.is_empty() {
            self.reject_type_arguments(reference);
            return (Vec::new(), None);
        }

        if reference.arguments.len() != expected.len() {
            let name = self.contracts[contract as usize].name.clone();
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                reference.span,
                format!(
                    "`{name}` takes {} type argument{}, not {}",
                    expected.len(),
                    if expected.len() == 1 { "" } else { "s" },
                    reference.arguments.len()
                ),
                format!("`{name}` is declared with {} of its own", expected.len()),
                None,
            );
            for arg in &reference.arguments {
                self.resolve_type(arg);
            }
            return (Vec::new(), None);
        }

        let args: Vec<Type> = reference
            .arguments
            .iter()
            .map(|a| self.resolve_type(a))
            .collect();

        for (&param_id, &arg) in expected.iter().zip(&args) {
            let constraints = self.type_params[param_id as usize].constraints.clone();
            for constraint in &constraints {
                if self.satisfies_constraint(arg, *constraint) {
                    continue;
                }
                let param_name = self.type_params[param_id as usize].name.clone();
                let arg_name = self.name(arg);
                let constraint_name = self.name(*constraint);
                self.error(
                    codes::TYPE_MISMATCH,
                    reference.span,
                    format!("`{arg_name}` does not satisfy `{param_name}`"),
                    format!(
                        "`{param_name}` requires `{constraint_name}`, which `{arg_name}` does not provide"
                    ),
                    None,
                );
            }
        }

        // `implements Iterable<T>` and `implements Iterator<T>` are the
        // generic contract instantiations that lower (roadmap task 13.5,
        // `for ... in` over a type's own iterator, which needs both halves
        // of the protocol implemented): any other stays gated, since
        // nothing else specializes a class's own generic contract table yet.
        if self
            .native_iteration
            .is_none_or(|n| contract != n.iterable && contract != n.iterator)
        {
            self.not_lowered(
                reference.span,
                "an implementation of a generic contract",
                "implement it manually per concrete type for now, without naming the contract's own type parameter",
            );
        }

        let instance = self.intern_contract_instance(GenericContractInstance {
            contract,
            args: args.clone(),
        });
        (expected.into_iter().zip(args).collect(), Some(instance))
    }

    /// Checks that a class supplies every required attribute and
    /// `abstract fn` an `abstract class` declares.
    ///
    /// Unlike a contract's method, an abstract class's is only satisfied by
    /// an explicit `override fn` — `ZIRK_LANGUAGE_SPEC.md`'s scenario for
    /// this spells it out for both a field and a method at once.
    fn require_abstract_conformance(
        &mut self,
        class: u32,
        abstract_id: u32,
        at: Span,
        decl: &ClassDecl,
    ) {
        let required_fields = self.classes[abstract_id as usize].fields.clone();
        let required_methods = self.classes[abstract_id as usize].methods.clone();
        let abstract_name = self.classes[abstract_id as usize].name.clone();
        let class_name = self.classes[class as usize].name.clone();

        for field in &required_fields {
            let Some(supplied) = self.classes[class as usize].field(&field.name).cloned() else {
                self.error(
                    codes::MISSING_IMPLEMENTATION,
                    at,
                    format!("`{class_name}` does not implement `{}`", field.name),
                    format!("`{abstract_name}` requires this attribute"),
                    Some(format!("add `{}: ...;` to `{class_name}`", field.name)),
                );
                continue;
            };
            if supplied.ty != field.ty {
                let expected = self.name(field.ty);
                let actual = self.name(supplied.ty);
                self.error(
                    codes::TYPE_MISMATCH,
                    supplied.span,
                    format!(
                        "`{}` does not match what `{abstract_name}` requires",
                        field.name
                    ),
                    format!("`{abstract_name}` declares it `{expected}`, not `{actual}`"),
                    None,
                );
            }
        }

        for method in &required_methods {
            let Some(supplied) = self.classes[class as usize].method(&method.name).cloned() else {
                self.error(
                    codes::MISSING_IMPLEMENTATION,
                    at,
                    format!("`{class_name}` does not implement `{}`", method.name),
                    format!("`{abstract_name}` requires it"),
                    Some(format!(
                        "add `override fn {}(...): ...` to `{class_name}`",
                        method.name
                    )),
                );
                continue;
            };

            let same_params = supplied.params.len() == method.params.len()
                && supplied
                    .params
                    .iter()
                    .zip(&method.params)
                    .all(|(a, b)| a.ty == b.ty);
            if !same_params || supplied.returns != method.returns {
                let line = self.sources.location(method.span).line;
                self.error(
                    codes::TYPE_MISMATCH,
                    supplied.span,
                    format!(
                        "`{}` does not match what `{abstract_name}` requires",
                        method.name
                    ),
                    format!("the one declared on line {line} has a different signature"),
                    Some(
                        "keep the parameters and the return type the abstract class declared"
                            .into(),
                    ),
                );
                continue;
            }

            // Unlike a contract method, an abstract class's requirement is
            // only satisfied by an explicit `override fn`
            // (`ZIRK_LANGUAGE_SPEC.md`'s conformance scenario).
            let written_override = decl
                .methods
                .iter()
                .find(|m| m.name.name == method.name)
                .is_some_and(|m| m.is_override);
            if !written_override {
                self.error(
                    codes::MISSING_OVERRIDE,
                    supplied.span,
                    format!(
                        "`{}` implements an abstract class's requirement",
                        method.name
                    ),
                    format!("`{abstract_name}` requires it, the same as an inherited method"),
                    Some(format!("write `override fn {}(...): ...`", method.name)),
                );
            }
        }
    }

    /// Replaces a contract method's own type parameters with the concrete
    /// arguments of one `implements` instantiation.
    fn substitute_contract_method(
        &mut self,
        method: &ContractMethod,
        subst: &[(u32, Type)],
    ) -> ContractMethod {
        ContractMethod {
            name: method.name.clone(),
            params: method
                .params
                .iter()
                .map(|p| ParamInfo {
                    ty: self.substitute_type(p.ty, subst),
                    ..p.clone()
                })
                .collect(),
            returns: self.substitute_type(method.returns, subst),
            span: method.span,
            has_default: method.has_default,
            index: method.index,
        }
    }

    /// Checks that a class supplies everything a contract requires.
    fn require_conformance(&mut self, class: u32, contract: u32, at: Span, subst: &[(u32, Type)]) {
        let required = self.contracts[contract as usize].methods.clone();
        let contract_name = self.contracts[contract as usize].name.clone();
        let class_name = self.classes[class as usize].name.clone();

        // A generic contract's own `<T>` is substituted before anything below
        // compares against it, so `Iterable<Int32>` is checked against
        // `iterator(): Iterator<Int32>`, not the unsubstituted `Iterator<T>`
        // every instantiation shares in the table (task 6.9).
        let required: Vec<ContractMethod> = if subst.is_empty() {
            required
        } else {
            required
                .iter()
                .map(|m| self.substitute_contract_method(m, subst))
                .collect()
        };

        for method in &required {
            let Some(supplied) = self.classes[class as usize].method(&method.name).cloned() else {
                // A trait's own body stands in for the one the class did not
                // write: that is the whole of what a trait adds.
                if method.has_default {
                    self.adopt_default(class, contract, method);
                    continue;
                }

                self.error(
                    codes::MISSING_IMPLEMENTATION,
                    at,
                    format!("`{class_name}` does not implement `{}`", method.name),
                    format!("`{contract_name}` requires it"),
                    Some(format!(
                        "add `fn {}(...): ...` to `{class_name}`",
                        method.name
                    )),
                );
                continue;
            };

            // Two traits offering their own default for the same name is the
            // diamond problem (D4): the class never wrote a choice, so
            // whichever trait happened to be adopted first would otherwise
            // win silently, by declaration order alone.
            if method.has_default
                && let Some(previous_contract) = supplied.from_contract
                && previous_contract != contract
            {
                let name = method.name.clone();
                let previous_name = self.contracts[previous_contract as usize].name.clone();
                self.error(
                    codes::DUPLICATE_DECLARATION,
                    at,
                    format!("`{class_name}` inherits two defaults for `{name}`"),
                    format!(
                        "`{contract_name}` and `{previous_name}` each supply their own `{name}`"
                    ),
                    Some(format!(
                        "add `fn {name}(...): ...` to `{class_name}` to choose one"
                    )),
                );
                continue;
            }

            self.require_matching_signature(&supplied, method, &contract_name, at);
        }
    }

    /// Copies a trait's default body into the class that did not write it.
    ///
    /// The class ends up with an ordinary method pointing at the trait's body,
    /// so nothing downstream has to know the difference — which is what makes
    /// the dispatch table need no new shape.
    fn adopt_default(&mut self, class: u32, contract: u32, method: &ContractMethod) {
        let index = self.classes[class as usize].methods.len();
        self.classes[class as usize].methods.push(MethodInfo {
            name: method.name.clone(),
            params: method.params.clone(),
            returns: method.returns,
            visibility: Visibility::Public,
            span: method.span,
            index,
            owner: class,
            overridden: false,
            from_contract: Some(contract),
        });
    }

    /// A supplied method has to match what the contract asked for.
    fn require_matching_signature(
        &mut self,
        supplied: &MethodInfo,
        required: &ContractMethod,
        contract: &str,
        at: Span,
    ) {
        let same_params = supplied.params.len() == required.params.len()
            && supplied
                .params
                .iter()
                .zip(&required.params)
                .all(|(a, b)| a.ty == b.ty);

        if same_params && supplied.returns == required.returns {
            if supplied.visibility != Visibility::Public {
                let name = supplied.name.clone();
                self.error(
                    codes::INACCESSIBLE_MEMBER,
                    supplied.span,
                    format!("`{name}` implements a contract, so it must be public"),
                    format!("`{contract}` declares it as behaviour anyone may reach"),
                    Some("remove the visibility modifier".into()),
                );
            }
            return;
        }

        let name = supplied.name.clone();
        let line = self.sources.location(required.span).line;
        self.error(
            codes::TYPE_MISMATCH,
            supplied.span,
            format!("`{name}` does not match what `{contract}` requires"),
            format!("the signature it declares is on line {line}"),
            Some("keep the parameters and the return type the contract asked for".into()),
        );
        let _ = at;
    }

    /// Registers a class by name, before anything about it is resolved.
    fn register_class(&mut self, decl: &ClassDecl) {
        if Self::is_native_type_name(&decl.name.name) {
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("`{}` is a type of the language", decl.name.name),
                "application code cannot reopen a native type or replace what it means",
                Some("pick a different name".into()),
            );
            return;
        }

        if let Some(previous) = self.classes.iter().find(|c| c.name == decl.name.name) {
            let where_ = self.declared_at(previous.span, decl.name.span);
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("class `{}` is already defined", decl.name.name),
                format!("a previous definition exists {where_}"),
                Some("rename one of the two".into()),
            );
            return;
        }

        // Minted here, ahead of every other pass: a field elsewhere in the
        // file may write `Box<Int32>` before `Box` itself is declared, and
        // resolving that needs to know how many parameters `Box` takes.
        // Constraints are filled in later, by `resolve_class_type_param_constraints`,
        // once every class and contract they might name is registered too.
        let type_params = self.mint_type_param_ids(&decl.type_params);

        self.classes.push(ClassType {
            name: decl.name.name.clone(),
            kind: decl.kind,
            base: None,
            fields: Vec::new(),
            constructors: Vec::new(),
            methods: Vec::new(),
            contracts: Vec::new(),
            contract_instances: Vec::new(),
            abstract_bases: Vec::new(),
            type_params,
            shared: decl.shared,
            span: decl.name.span,
        });
    }

    /// Mints an id for each `TypeParam`, without resolving its constraints.
    ///
    /// Shared by [`Self::register_class`], which needs a class's arity known
    /// before any field in the file is resolved, and by
    /// [`Self::enter_type_params`], which mints on first use for a function
    /// or method — nothing outside its own declaration ever names its
    /// parameter, so there is no equivalent ordering hazard there.
    fn mint_type_param_ids(&mut self, params: &[TypeParam]) -> Vec<u32> {
        params
            .iter()
            .map(|p| {
                if let Some(&id) = self.type_param_ids.get(&p.span) {
                    return id;
                }
                let id = self.type_params.len() as u32;
                self.type_params.push(TypeParamInfo {
                    name: p.name.name.clone(),
                    constraints: Vec::new(),
                    span: p.span,
                });
                self.type_param_ids.insert(p.span, id);
                id
            })
            .collect()
    }

    /// Resolves the `from` constraints of every class's own type parameters,
    /// and reports its variance and its `NOT_LOWERED` status, once each.
    ///
    /// Separate from `register_class` because a constraint may name a class
    /// or contract declared later in the file: this runs only after every
    /// class is registered and `resolve_bases` has run.
    fn resolve_class_type_param_constraints(&mut self, program: &Program) {
        for decl in &program.classes {
            if decl.type_params.is_empty() {
                continue;
            }
            // Whether this class's own `T` lowers per instantiation is
            // decided once its members exist (`Self::check_generic_class_lowering`,
            // which needs field and method types, not yet available here).
            self.report_declared_variance(&decl.type_params);

            self.type_param_scope.push(
                decl.type_params
                    .iter()
                    .map(|p| (p.name.name.clone(), self.type_param_ids[&p.span]))
                    .collect(),
            );
            for p in &decl.type_params {
                let constraints: Vec<Type> =
                    p.constraints.iter().map(|c| self.resolve_type(c)).collect();
                let id = self.type_param_ids[&p.span];
                self.type_params[id as usize].constraints = constraints;
            }
            self.type_param_scope.pop();
        }
    }

    /// Gates a generic class with `NOT_LOWERED` unless its own `T` is used
    /// simply enough to specialize per instantiation (roadmap task 11.1,
    /// D5): lowering builds one specialized copy per combination of type
    /// arguments used, substituting `T` directly in fields, constructor
    /// parameters and method signatures. What that pass does not build yet —
    /// `extends` on a generic class, a type parameter nested inside another
    /// generic type (`Box<T>`, `T | Int32`, `Iterable<T>`) rather than named
    /// directly, or a method with type parameters of its own — stays gated.
    fn check_generic_class_lowering(&mut self, program: &Program) {
        for decl in &program.classes {
            if decl.type_params.is_empty() {
                continue;
            }
            let Some(id) = self.class_id(&decl.name.name) else {
                continue;
            };
            if self.generic_class_is_directly_specializable(id) {
                continue;
            }
            self.not_lowered(
                decl.type_params[0].span,
                "a generic type parameter",
                "declare it without `<...>` for now, or instantiate it manually per concrete type",
            );
        }
    }

    /// See [`Self::check_generic_class_lowering`].
    fn generic_class_is_directly_specializable(&self, id: u32) -> bool {
        let class = &self.classes[id as usize];
        // `extends` and `implements` both stay out of scope for this pass:
        // a specialized copy's own dispatch tables (its method table for
        // `extends`, a contract table for `implements`) are a separate
        // concern this pass does not build.
        if class.base.is_some() || !class.contracts.is_empty() || !class.abstract_bases.is_empty() {
            return false;
        }

        let ids = &class.type_params;
        let direct_or_absent = |ty: Type| {
            matches!(ty.base, Base::Param(pid) if ids.contains(&pid))
                || !self.type_references_any_param(ty, ids)
        };

        class.fields.iter().all(|f| direct_or_absent(f.ty))
            && class
                .constructors
                .iter()
                .all(|params| params.iter().all(|p| direct_or_absent(p.ty)))
            && class.methods.iter().all(|m| {
                m.params.iter().all(|p| direct_or_absent(p.ty)) && direct_or_absent(m.returns)
                // A method with type parameters of its own is already gated
                // independently: `enter_type_params` reports `NOT_LOWERED`
                // for it the same way a generic function's would, regardless
                // of what this check decides for the class's own `T`.
            })
    }

    /// Whether `ty` names any of `ids` anywhere within it, including nested
    /// inside another generic instantiation or a union.
    fn type_references_any_param(&self, ty: Type, ids: &[u32]) -> bool {
        match ty.base {
            Base::Param(pid) => ids.contains(&pid),
            Base::Instance(inst) => self.generic_instances[inst as usize]
                .args
                .iter()
                .any(|&a| self.type_references_any_param(a, ids)),
            Base::ContractInstance(inst) => self.contract_instances[inst as usize]
                .args
                .iter()
                .any(|&a| self.type_references_any_param(a, ids)),
            Base::EnumInstance(inst) => self.enum_instances[inst as usize]
                .args
                .iter()
                .any(|&a| self.type_references_any_param(a, ids)),
            Base::Union(union_id) => self.unions[union_id as usize]
                .iter()
                .any(|&b| self.type_references_any_param(Type::of(b), ids)),
            _ => false,
        }
    }

    /// Reports each `in`/`out` on a list of type parameters, once.
    ///
    /// Its own diagnostic rather than folding it into `not_lowered`: task 7.5
    /// of the generics slice asks for variance to say so distinctly, since
    /// subtyping between instantiations — what variance would mean — needs
    /// more than "generics do not lower yet" to explain.
    fn report_declared_variance(&mut self, params: &[TypeParam]) {
        for p in params {
            let keyword = match p.variance {
                Variance::Invariant => continue,
                Variance::In => "in",
                Variance::Out => "out",
            };
            self.error(
                codes::PENDING_FEATURE,
                p.span,
                "declared variance is not verified yet",
                format!(
                    "`{keyword} {}` parses, but the checker treats every parameter as invariant",
                    p.name.name
                ),
                Some("remove the `in`/`out`, or accept that it is not enforced yet".into()),
            );
        }
    }

    /// Resolves every `extends`, rejecting a base that is not a class and any
    /// cycle in the result.
    fn resolve_bases(&mut self, program: &Program) {
        for decl in &program.classes {
            let Some(base) = &decl.extends else { continue };
            let Some(id) = self.class_id(&decl.name.name) else {
                continue;
            };

            if decl.kind != ClassKind::Class {
                let article = if decl.kind == ClassKind::Abstract {
                    "an"
                } else {
                    "a"
                };
                let kind = decl.kind.as_str();
                self.error(
                    codes::TYPE_MISMATCH,
                    base.span,
                    format!("{article} {kind} cannot extend anything"),
                    format!("{kind} has no identity for inheritance to build on"),
                    Some("remove `extends`".into()),
                );
                continue;
            }

            let resolved = self.resolved_name(&base.name, base.span);
            let Some(base_id) = self.class_id(&resolved) else {
                self.error(
                    codes::UNKNOWN_TYPE,
                    base.span,
                    format!("`{}` is not a declared class", base.name),
                    "a class extends a class",
                    None,
                );
                continue;
            };

            let declared = self.classes[base_id as usize].span;
            let shared = self.classes[base_id as usize].shared;
            self.require_visible(declared, shared, base, "class");
            self.classes[id as usize].base = Some(base_id);
        }

        // A cycle would make the flattening below run forever, so it is cut
        // before anything walks the chain.
        for id in 0..self.classes.len() as u32 {
            if let Some(cycle) = self.inheritance_cycle(id) {
                let span = self.classes[id as usize].span;
                let name = self.classes[id as usize].name.clone();
                self.error(
                    codes::DUPLICATE_DECLARATION,
                    span,
                    format!("`{name}` inherits from itself"),
                    format!("the chain closes on itself: {cycle}"),
                    Some("a class cannot be, directly or indirectly, its own base".into()),
                );
                self.classes[id as usize].base = None;
            }
        }
    }

    /// The chain from a class back to itself, if it has one.
    fn inheritance_cycle(&self, id: u32) -> Option<String> {
        let mut seen = vec![id];
        let mut current = self.classes[id as usize].base;

        while let Some(next) = current {
            if seen.contains(&next) {
                let names: Vec<&str> = seen
                    .iter()
                    .map(|i| self.classes[*i as usize].name.as_str())
                    .collect();
                return Some(format!(
                    "{} -> {}",
                    names.join(" -> "),
                    self.classes[next as usize].name
                ));
            }
            seen.push(next);
            current = self.classes[next as usize].base;
        }

        None
    }

    fn class_id(&self, name: &str) -> Option<u32> {
        self.classes
            .iter()
            .position(|c| c.name == name)
            .map(|i| i as u32)
    }

    /// Fills in fields, constructors and methods, bases before subclasses.
    fn declare_members(&mut self, program: &Program) {
        for decl in self.in_hierarchy_order(program) {
            self.declare_class_members(decl);
        }
    }

    /// The declarations ordered so a class always follows its base.
    fn in_hierarchy_order<'p>(&self, program: &'p Program) -> Vec<&'p ClassDecl> {
        let mut ordered: Vec<&ClassDecl> = Vec::new();
        let mut pending: Vec<&ClassDecl> = program.classes.iter().collect();

        // The chain is acyclic by now, so every round places at least one
        // class and the loop terminates.
        while !pending.is_empty() {
            let mut placed = Vec::new();
            pending.retain(|decl| {
                let ready = match self
                    .class_id(&decl.name.name)
                    .map(|id| self.classes[id as usize].base)
                {
                    Some(Some(base)) => ordered
                        .iter()
                        .any(|d| self.class_id(&d.name.name) == Some(base)),
                    _ => true,
                };
                if ready {
                    placed.push(*decl);
                }
                !ready
            });

            if placed.is_empty() {
                // Only reachable if a base was rejected: the rest are placed
                // as they are so their own members still get checked.
                ordered.append(&mut pending);
                break;
            }
            ordered.extend(placed);
        }

        ordered
    }

    fn declare_class_members(&mut self, decl: &ClassDecl) {
        let Some(id) = self.class_id(&decl.name.name) else {
            return;
        };
        self.enter_type_params(&decl.type_params);

        // The grammar and the type rules for a record, value class or
        // abstract class exist from here on (construction or conformance,
        // equality, immutability).
        match decl.kind {
            ClassKind::Class => {}
            ClassKind::Abstract => {
                // Nothing to lower on its own — no `construct`, no state, no
                // layout — but a value typed through it would need dynamic
                // dispatch through its adopter the way a contract's does,
                // and that path does not exist for it yet.
                self.not_lowered(
                    decl.name.span,
                    "an abstract class",
                    "implement its requirements directly on the concrete class for now, without naming the abstract class as a type",
                );
            }
            // A record or value class lowers to an inline value (roadmap
            // task 11.5) — but only a non-generic one: combining that with
            // per-instantiation specialization (11.1) is a separate concern
            // this pass does not build.
            ClassKind::Record | ClassKind::ValueClass if !decl.type_params.is_empty() => {
                self.not_lowered(
                    decl.name.span,
                    &format!("a generic {}", decl.kind.as_str()),
                    "model it as an ordinary `class` for now, with a `construct` that sets every field",
                );
            }
            ClassKind::Record | ClassKind::ValueClass => {}
        }

        // Inherited members come first, which is what makes a subclass's
        // layout start with its base's (D2).
        let base = self.classes[id as usize].base;
        let mut fields: Vec<FieldInfo> = base
            .map(|b| self.classes[b as usize].fields.clone())
            .unwrap_or_default();

        for field in &decl.fields {
            if let Some(previous) = fields.iter().find(|f| f.name == field.name.name) {
                let owner = self.classes[previous.owner as usize].name.clone();
                let inherited = previous.owner != id;
                let where_ = self.declared_at(previous.span, field.name.span);
                self.error(
                    codes::DUPLICATE_DECLARATION,
                    field.name.span,
                    format!("field `{}` is already defined", field.name.name),
                    if inherited {
                        format!("`{owner}` already declares it, {where_}")
                    } else {
                        format!("a previous definition exists {where_}")
                    },
                    Some(if inherited {
                        "a subclass cannot redeclare an inherited field".into()
                    } else {
                        "rename one of the two".to_string()
                    }),
                );
                continue;
            }

            let ty = self.resolve_type(&field.ty);
            if matches!(ty.base, Base::Void) {
                self.error(
                    codes::VOID_VARIABLE,
                    field.ty.span,
                    "a field cannot have type `Void`",
                    "`Void` is the absence of a value, so there is nothing to store",
                    None,
                );
            }

            // A record or value class is a value: every field is immutable,
            // the same way `class`'s own `public mut` default does not apply
            // to it. Writing `mut` explicitly says so, which is worth its own
            // diagnostic rather than a silently ignored modifier.
            let mutability = if matches!(decl.kind, ClassKind::Class | ClassKind::Abstract) {
                // A required attribute has whatever mutability its declared
                // implementer gives it — an abstract class states that a
                // field must exist, not that it is a value.
                field.mutability
            } else {
                if field.mutability == Mutability::Mutable && field.explicit_modifiers {
                    self.error(
                        codes::TYPE_MISMATCH,
                        field.name.span,
                        format!("a {} field is always immutable", decl.kind.as_str()),
                        "it has no identity to protect by forbidding reassignment instead",
                        Some("remove `mut`".into()),
                    );
                }
                Mutability::Immutable
            };

            fields.push(FieldInfo {
                name: field.name.name.clone(),
                ty,
                visibility: field.visibility,
                mutability,
                span: field.name.span,
                owner: id,
            });
        }

        if decl.kind != ClassKind::Class && !decl.constructors.is_empty() {
            let article = if decl.kind == ClassKind::Abstract {
                "an"
            } else {
                "a"
            };
            let cause = if decl.kind == ClassKind::Abstract {
                "it is never instantiated: a concrete class adopts its requirements with `implements`"
            } else {
                "its construction is always the implicit named constructor over its fields"
            };
            for constructor in &decl.constructors {
                self.error(
                    codes::TYPE_MISMATCH,
                    constructor.span,
                    format!("{article} {} has no `construct`", decl.kind.as_str()),
                    cause,
                    Some("remove `construct`".into()),
                );
            }
        }

        let constructors = decl
            .constructors
            .iter()
            .map(|c| c.params.iter().map(|p| self.resolve_param(p)).collect())
            .collect();

        let mut methods: Vec<MethodInfo> = base
            .map(|b| self.classes[b as usize].methods.clone())
            .unwrap_or_default();

        for method in &decl.methods {
            if let Some(field) = fields.iter().find(|f| f.name == method.name.name) {
                let where_ = self.declared_at(field.span, method.name.span);
                self.error(
                    codes::DUPLICATE_DECLARATION,
                    method.name.span,
                    format!("`{}` is already a field of this class", method.name.name),
                    format!("the field is declared {where_}"),
                    Some("a member is a field or a method, not both".into()),
                );
                continue;
            }

            // The grammar only rejects a body on a method written `abstract`;
            // one written without it inside `abstract class` still has to be
            // caught, since every member there is a signature.
            if decl.kind == ClassKind::Abstract && method.body.is_some() {
                self.error(
                    codes::TYPE_MISMATCH,
                    method.name.span,
                    format!("`{}` has a body inside an abstract class", method.name.name),
                    "an abstract class declares signatures only, with no body of its own",
                    Some(format!(
                        "write `abstract fn {}(...): ...;`",
                        method.name.name
                    )),
                );
                continue;
            }

            self.enter_type_params(&method.type_params);
            let resolved = MethodInfo {
                name: method.name.name.clone(),
                params: method
                    .params
                    .iter()
                    .map(|p| self.resolve_param(p))
                    .collect(),
                returns: self.resolve_type(&method.return_type),
                visibility: method.visibility,
                span: method.name.span,
                index: 0,
                owner: id,
                overridden: false,
                from_contract: None,
            };
            self.leave_type_params();

            match methods.iter().position(|m| m.name == method.name.name) {
                Some(position) if methods[position].owner != id => {
                    // Replacing an inherited method has to say so
                    // (`ZIRK_LANGUAGE_SPEC.md` section 7): otherwise adding a
                    // method to a base silently changes what a subclass means.
                    if !method.is_override {
                        let owner = self.classes[methods[position].owner as usize].name.clone();
                        let line = self.sources.location(methods[position].span).line;
                        self.error(
                            codes::MISSING_OVERRIDE,
                            method.name.span,
                            format!("`{}` replaces an inherited method", method.name.name),
                            format!("`{owner}` declares it on line {line}"),
                            Some(format!("write `override fn {}`", method.name.name)),
                        );
                    }

                    // An override replaces the entry it overrides, keeping its
                    // index: that is what lets a subclass's table start with
                    // its base's, so a method's slot does not move.
                    self.require_same_signature(&methods[position], &resolved, method.name.span);
                    let index = methods[position].index;
                    methods[position] = MethodInfo { index, ..resolved };
                }
                Some(position) => {
                    let previous = methods[position].span;
                    let where_ = self.declared_at(previous, method.name.span);
                    self.error(
                        codes::DUPLICATE_DECLARATION,
                        method.name.span,
                        format!("method `{}` is already defined", method.name.name),
                        format!("a previous definition exists {where_}"),
                        Some("rename one of the two: there is no overloading".into()),
                    );
                }
                None => {
                    // `override` on something that overrides nothing is the
                    // mirror mistake, and just as worth catching: it usually
                    // means a typo in the name. Not when the class `implements`
                    // something, though: it may be satisfying an abstract
                    // class's requirement, which is checked for real once
                    // every class has registered its own methods
                    // (`Self::require_abstract_conformance`) — this pass runs
                    // too early to know, in either declaration order.
                    if method.is_override && decl.implements.is_empty() {
                        self.error(
                            codes::MISSING_OVERRIDE,
                            method.name.span,
                            format!("`{}` overrides nothing", method.name.name),
                            "no base class declares a method with that name and signature",
                            Some("remove `override`, or check the spelling".into()),
                        );
                    }
                    let index = methods.len();
                    methods.push(MethodInfo { index, ..resolved });
                }
            }
        }

        self.classes[id as usize].fields = fields;
        self.classes[id as usize].constructors = constructors;
        self.classes[id as usize].methods = methods;

        // The base now knows one of its methods is redefined, which is what
        // decides whether a call to it can be direct.
        if let Some(base_id) = base {
            for method in &decl.methods {
                self.mark_overridden(base_id, &method.name.name);
            }
        }
        self.leave_type_params();
    }

    /// Whether a value of one type may stand where another is expected
    /// because it is a subclass of it.
    ///
    /// One direction only: a `Manager` is a `User`, and a `User` is not a
    /// `Manager`. Accepting the reverse would mean promising members the value
    /// may not have.
    ///
    /// Nullability follows the same rule as everywhere: `T` fits `T?`, and the
    /// other way needs `??`.
    fn is_subclass_of(&self, actual: Type, expected: Type) -> bool {
        if actual.nullable && !expected.nullable {
            return false;
        }

        let Base::Class(current) = actual.base else {
            return false;
        };

        match expected.base {
            // A class satisfies a contract by saying so and supplying it.
            Base::Contract(target) => self.classes[current as usize].contracts.contains(&target),
            // A class satisfies a generic contract instantiation
            // (`Iterable<Int32>`, task 6.9) the same way, but has to match
            // the specific arguments too: implementing `Iterable<Int32>`
            // does not make a class assignable to `Iterable<String>`.
            Base::ContractInstance(target) => self.classes[current as usize]
                .contract_instances
                .iter()
                .any(|&id| {
                    self.contract_instances[id as usize] == self.contract_instances[target as usize]
                }),
            // A class satisfies an `abstract class` the same way it
            // satisfies a contract: by naming it in `implements`. Its own
            // `extends` chain never does, since an abstract class has no
            // state or layout to extend.
            Base::Class(target) if self.classes[target as usize].kind == ClassKind::Abstract => {
                self.classes[current as usize]
                    .abstract_bases
                    .contains(&target)
            }
            Base::Class(target) => {
                let mut current = current;
                while let Some(base) = self.classes[current as usize].base {
                    if base == target {
                        return true;
                    }
                    current = base;
                }
                false
            }
            _ => false,
        }
    }

    /// Whether a bare variant of a generic enum fits where one of its
    /// instantiations is expected, such as `Iteration.Done` where
    /// `Iteration<Int32>` is declared.
    ///
    /// An approximation: there is no generic-enum-construction syntax that
    /// infers `T` (task 6.9 does not build one), so a variant of `Iteration`
    /// always types as the bare `Base::Enum`, never as the instantiation its
    /// context implies. Accepting it here — rather than making every such
    /// return a spurious `TYPE_MISMATCH` — is sound exactly because nothing
    /// downstream reads the argument yet: constructing one is `NOT_LOWERED`
    /// at its own declaration (`Self::resolve_enum_reference`) before this
    /// check would ever matter to a running program.
    fn bare_enum_matches_instance(&self, actual: Type, expected: Type) -> bool {
        if actual.nullable && !expected.nullable {
            return false;
        }
        let Base::Enum(actual_id) = actual.base else {
            return false;
        };
        let Base::EnumInstance(target) = expected.base else {
            return false;
        };
        self.enum_instances[target as usize].enum_id == actual_id
    }

    /// Whether a value fits where a union is expected: it matches, or is a
    /// subclass of, at least one alternative.
    ///
    /// `Type::accepts` alone cannot answer this: a union's member list lives
    /// in the checker's table, not in the two bytes a bare `Type` carries.
    fn accepts_into_union(&self, expected: Type, actual: Type) -> bool {
        let Base::Union(id) = expected.base else {
            return false;
        };
        if actual.is_unknown() {
            return true;
        }
        if actual.nullable && !expected.nullable {
            return false;
        }
        let actual = actual.without_null();
        self.unions[id as usize].iter().any(|&member| {
            let member = Type::of(member);
            member.accepts(actual) || self.is_subclass_of(actual, member)
        })
    }

    /// Marks a method as redefined, all the way up the chain.
    fn mark_overridden(&mut self, id: u32, name: &str) {
        let mut current = Some(id);
        while let Some(class) = current {
            if let Some(method) = self.classes[class as usize]
                .methods
                .iter_mut()
                .find(|m| m.name == name)
            {
                method.overridden = true;
            }
            current = self.classes[class as usize].base;
        }
    }

    /// An override keeps the signature it overrides.
    ///
    /// Anything else would let a call through the base reach a body that
    /// expects something different, which is the one thing the base's type is
    /// supposed to promise.
    fn require_same_signature(&mut self, base: &MethodInfo, override_: &MethodInfo, at: Span) {
        let same_params = base.params.len() == override_.params.len()
            && base
                .params
                .iter()
                .zip(&override_.params)
                .all(|(a, b)| a.ty == b.ty);

        if same_params && base.returns == override_.returns {
            return;
        }

        let name = base.name.clone();
        let line = self.sources.location(base.span).line;
        self.error(
            codes::TYPE_MISMATCH,
            at,
            format!("`{name}` does not match the method it overrides"),
            format!("the one declared on line {line} has a different signature"),
            Some(
                "an override keeps the parameters and the return type of the method it replaces"
                    .into(),
            ),
        );
    }

    /// Checks the bodies of a class: its constructors and its methods.
    ///
    /// `this` is bound as an ordinary immutable binding of the class type. It
    /// cannot be reassigned — there is no instance to swap for another — but
    /// what it points at is mutable, which is what makes `this.name = value`
    /// work.
    fn check_class(&mut self, decl: &ClassDecl) {
        let Some(id) = self.classes.iter().position(|c| c.name == decl.name.name) else {
            // Its declaration was rejected; its bodies would report the same
            // problem again from every member.
            return;
        };
        let class_type = Type::of(Base::Class(id as u32));
        self.enter_type_params(&decl.type_params);

        for constructor in &decl.constructors {
            self.in_constructor = true;
            self.check_member_body(class_type, &constructor.params, Type::VOID, |checker| {
                checker.check_block(&constructor.body);
            });
            self.in_constructor = false;
            self.require_fields_initialized(decl, constructor);
        }

        for method in &decl.methods {
            let Some(body) = &method.body else { continue };
            self.enter_type_params(&method.type_params);
            let returns = self.resolve_type(&method.return_type);
            let name = method.name.name.clone();
            let span = body.span;
            self.check_member_body(class_type, &method.params, returns, |checker| {
                let always_returns = checker.check_block(body);
                if returns != Type::VOID && !returns.is_unknown() && !always_returns {
                    let declared = checker.name(returns);
                    checker.error(
                        codes::MISSING_RETURN,
                        span,
                        format!("not every path of `{name}` returns a value"),
                        format!("the method declares `{declared}` as its return type"),
                        Some("add a `return` at the end of the method".into()),
                    );
                }
            });
            self.leave_type_params();
        }
        self.leave_type_params();
    }

    /// Runs a member body with `this` and the parameters in scope.
    fn check_member_body(
        &mut self,
        class_type: Type,
        params: &[Param],
        returns: Type,
        check: impl FnOnce(&mut Self),
    ) {
        let previous_return = self.current_return;
        let previous_this = self.this_type;
        self.current_return = returns;
        self.this_type = Some(class_type);

        self.scopes.push_function();
        self.declare_local(Binding {
            name: "this".to_string(),
            ty: class_type,
            mutability: Mutability::Immutable,
            span: Span::new(0, 0),
            initialized: true,
        });
        for param in params {
            let info = self.resolve_param(param);
            self.declare_local(Binding {
                name: info.name,
                ty: info.ty,
                mutability: Mutability::Immutable,
                span: param.name.span,
                initialized: true,
            });
        }

        check(self);

        self.scopes.pop();
        self.current_return = previous_return;
        self.this_type = previous_this;
    }

    /// Every field without a default must be written by the constructor.
    ///
    /// An object whose fields were never set would hand out whatever the
    /// allocation happened to contain, and `LANGUAGE_SPEC` section 2 forbids
    /// reading a variable before it holds a value. A field is no different.
    fn require_fields_initialized(&mut self, decl: &ClassDecl, constructor: &ConstructDecl) {
        let assigned = assigned_fields(&constructor.body);
        let Some(id) = self.class_id(&decl.name.name) else {
            return;
        };

        // Only a field with no type default has to be written: an omitted
        // attribute receives its default before any initializer or constructor
        // runs (`ZIRK_LANGUAGE_SPEC.md` section 7), so leaving one out is not
        // leaving it undefined.
        // A `super(...)` runs the base's constructor, so everything the base
        // declares is its responsibility rather than this one's.
        let delegates = calls_super(&constructor.body);
        let base = self.classes[id as usize].base;

        let unset: Vec<FieldInfo> = self.classes[id as usize]
            .fields
            .iter()
            .filter(|f| !f.ty.has_default() && !assigned.contains(&f.name))
            .filter(|f| !(delegates && base.is_some() && f.owner != id))
            .cloned()
            .collect();

        if unset.is_empty() {
            return;
        }

        // A field the subclass cannot even name is a different problem: it is
        // not that the author forgot, it is that the language gives them no
        // way to do it. Saying "assign it" would be advice they cannot follow.
        let (reachable, hidden): (Vec<_>, Vec<_>) = unset
            .into_iter()
            .partition(|f| self.can_access(f.visibility, f.owner));

        if let Some(field) = hidden.first() {
            let owner = self.classes[field.owner as usize].name.clone();
            self.error(
                codes::UNINITIALIZED_FIELD,
                constructor.span,
                format!("`{}` cannot be initialized from here", field.name),
                format!(
                    "it is `{}` in `{owner}`, and this class has no way to run that constructor",
                    field.visibility.as_str()
                ),
                Some(
                    "the language has no base-constructor call yet; make the field `protected`, or do not extend this class"
                        .into(),
                ),
            );
        }

        if let Some(first) = reachable.first() {
            let names = reachable
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>()
                .join("`, `");
            self.error(
                codes::UNINITIALIZED_FIELD,
                constructor.span,
                format!("`{names}` is not initialized by this constructor"),
                "every field must hold a value once the constructor returns",
                Some(format!("assign it with `this.{} = ...`", first.name)),
            );
        }
    }

    /// Registers every `type` alias's target, unresolved.
    ///
    /// Resolution happens lazily, inside [`Self::resolve_type`]: an alias may
    /// name a class declared later in the file, the same forward-reference
    /// every other type name already tolerates.
    fn declare_type_aliases(&mut self, program: &Program) {
        for decl in &program.type_aliases {
            if self.type_aliases.contains_key(&decl.name.name) {
                self.error(
                    codes::DUPLICATE_DECLARATION,
                    decl.name.span,
                    format!("`{}` is already defined", decl.name.name),
                    "a previous `type` alias exists with this name",
                    Some("rename one of the two".into()),
                );
                continue;
            }
            self.type_aliases
                .insert(decl.name.name.clone(), decl.target.clone());

            // Resolution is real (`Self::resolve_type_alias`), but lowering
            // independently re-resolves a written type name from the AST in
            // some paths and does not know aliases exist yet, so a lowered
            // program could still crash on one.
            self.not_lowered(
                decl.span,
                "a `type` alias",
                "write the aliased type directly for now",
            );
        }
    }

    fn declare_enum(&mut self, decl: &EnumDecl) {
        if decl.name.name == "Iteration" {
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                "`Iteration` is an enum of the language",
                "application code cannot reopen a native enum or replace what it means",
                Some("pick a different name".into()),
            );
            return;
        }

        if let Some(previous) = self.enums.iter().find(|e| e.name == decl.name.name) {
            let where_ = self.declared_at(previous.span, decl.name.span);
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("enum `{}` is already defined", decl.name.name),
                format!("a previous definition exists {where_}"),
                Some("rename one of the two: a crate has one namespace in this phase".into()),
            );
            return;
        }

        let type_params = self.enter_type_params(&decl.type_params);
        let mut variants: Vec<EnumVariantInfo> = Vec::new();
        for variant in &decl.variants {
            if variants.iter().any(|v| v.name == variant.name.name) {
                self.error(
                    codes::DUPLICATE_DECLARATION,
                    variant.span,
                    format!("variant `{}` is repeated", variant.name.name),
                    format!("`{}` already declares it", decl.name.name),
                    None,
                );
                continue;
            }

            let mut seen_fields: Vec<String> = Vec::new();
            let mut associated = Vec::new();
            for field in &variant.associated {
                if seen_fields.contains(&field.name.name) {
                    self.error(
                        codes::DUPLICATE_DECLARATION,
                        field.span,
                        format!("associated field `{}` is repeated", field.name.name),
                        format!("`{}` already declares it", variant.name.name),
                        None,
                    );
                    continue;
                }
                seen_fields.push(field.name.name.clone());
                associated.push(AssociatedFieldInfo {
                    name: field.name.name.clone(),
                    ty: self.resolve_type(&field.ty),
                });
            }

            if let Some(mapping) = &variant.mapping {
                let ty = self.check_expr(mapping);
                if !ty.is_unknown() && ty != Type::STRING && ty != Type::INT32 {
                    let name = self.name(ty);
                    self.error(
                        codes::TYPE_MISMATCH,
                        mapping.span(),
                        format!("`{name}` cannot map a variant"),
                        "a mapping is a string or a numeric value, per `ZIRK_LANGUAGE_SPEC.md` section 7",
                        Some("write a string or an integer literal".into()),
                    );
                }
            }

            variants.push(EnumVariantInfo {
                name: variant.name.name.clone(),
                associated,
                span: variant.span,
            });
        }

        // An enum with no variants names a type nothing can ever be. The
        // language spells that `Never`, which arrives in Phase 3; here it is a
        // typo, and accepting it would give the concept a second spelling.
        if variants.is_empty() {
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("enum `{}` has no variants", decl.name.name),
                "a type with no values can never be constructed",
                Some("add at least one variant".into()),
            );
        }

        self.leave_type_params();

        self.enums.push(EnumType {
            name: decl.name.name.clone(),
            variants,
            type_params,
            shared: decl.shared,
            span: decl.name.span,
        });
    }

    fn declare_function(&mut self, f: &FnDecl) {
        let type_params = self.enter_type_params(&f.type_params);
        let params = f
            .params
            .iter()
            .map(|p| self.resolve_param(p))
            .collect::<Vec<_>>();

        let signature = Signature {
            name: f.name.name.clone(),
            params,
            returns: self.resolve_type(&f.return_type),
            type_params,
            shared: f.shared,
            span: f.name.span,
        };
        self.leave_type_params();

        if let Some(previous) = self.functions.get(&signature.name).cloned() {
            let where_ = self.declared_at(previous.span, f.name.span);
            self.error(
                codes::DUPLICATE_FUNCTION,
                f.name.span,
                format!("function `{}` is already defined", f.name.name),
                format!("a previous definition exists {where_}"),
                Some("rename one of the two: there is no overloading".into()),
            );
        } else {
            self.functions.insert(signature.name.clone(), signature);
        }
    }

    /// Resolves one parameter, applying the rule that `name?: T` is nullable.
    fn resolve_param(&mut self, p: &Param) -> ParamInfo {
        let mut ty = self.resolve_type(&p.ty);

        // An optional parameter may be absent, and absence is `null`.
        if p.optional {
            ty = ty.as_nullable();
        }

        // A variadic collects its values into a sequence, and no collection
        // type exists until Phase 3 brings `List`.
        if p.variadic {
            self.not_lowered(
                p.span,
                "a variadic parameter",
                "declare the parameters explicitly until collections arrive",
            );
        }

        ParamInfo {
            name: p.name.name.clone(),
            ty,
            optional: p.optional,
            has_default: p.default.is_some(),
            variadic: p.variadic,
        }
    }

    /// `ZIRK_RUNTIME_SPEC.md` section 2 requires a `main` entrypoint.
    fn check_entrypoint(&mut self, program: &Program) {
        let Some(main) = self.functions.get("main").cloned() else {
            self.error(
                codes::MISSING_ENTRYPOINT,
                self.sources.entry().span(0, 0),
                "the program has no entrypoint",
                "no `main` function was found in the file",
                Some("add `fn main(): Void { }`".into()),
            );
            return;
        };

        if !main.params.is_empty() || main.returns != Type::VOID {
            let span = program
                .functions
                .iter()
                .find(|f| f.name.name == "main")
                .map(|f| f.name.span)
                .unwrap_or(main.span);

            self.error(
                codes::INVALID_ENTRYPOINT,
                span,
                "`main` has the wrong signature",
                "the entrypoint takes no parameters and returns `Void` in this phase",
                Some("write `fn main(): Void { ... }`".into()),
            );
        }
    }

    /// Brings a class's, function's or method's own `<T from A & B>` into
    /// scope, interning each as an opaque [`Base::Param`] so the declaration
    /// can use it as a type immediately.
    ///
    /// Idempotent per `TypeParam`, keyed by its span: the same declaration is
    /// resolved once to register it and again to check its body, and both
    /// calls must agree on `T`'s id or a field's declared type would never
    /// equal the type of the value assigned to it in its own constructor.
    ///
    /// Verifying `from` at the use site and inside the body (roadmap tasks
    /// 7.2 and 7.4) is not implemented yet: `constraints` is only recorded.
    fn enter_type_params(&mut self, params: &[TypeParam]) -> Vec<u32> {
        let mut frame = HashMap::new();
        let mut newly_created = Vec::new();
        let mut ids = Vec::with_capacity(params.len());
        for (i, p) in params.iter().enumerate() {
            let id = match self.type_param_ids.get(&p.span) {
                Some(&id) => id,
                None => {
                    let id = self.type_params.len() as u32;
                    self.type_params.push(TypeParamInfo {
                        name: p.name.name.clone(),
                        constraints: Vec::new(),
                        span: p.span,
                    });
                    self.type_param_ids.insert(p.span, id);
                    newly_created.push((i, id));
                    id
                }
            };
            frame.insert(p.name.name.clone(), id);
            ids.push(id);
        }
        self.type_param_scope.push(frame);

        // A non-empty `newly_created` means this is the first time this
        // exact declaration is entered (the declaring pass, ahead of the
        // later checking pass that reuses these same ids), which is the one
        // place to report it once: the grammar and the type rules for a
        // generic body exist from here on, but lowering does not yet.
        if !newly_created.is_empty() {
            self.not_lowered(
                params[0].span,
                "a generic type parameter",
                "declare it without `<...>` for now, or instantiate it manually per concrete type",
            );
        }

        if !newly_created.is_empty() {
            let declared: Vec<TypeParam> = newly_created
                .iter()
                .map(|&(i, _)| params[i].clone())
                .collect();
            self.report_declared_variance(&declared);
        }

        // Constraints are resolved only the first time a `TypeParam` is seen;
        // a later re-entry reuses what was already resolved for its id.
        for (i, id) in newly_created {
            let constraints: Vec<Type> = params[i]
                .constraints
                .iter()
                .map(|c| self.resolve_type(c))
                .collect();
            self.type_params[id as usize].constraints = constraints;
        }

        ids
    }

    fn leave_type_params(&mut self) {
        self.type_param_scope.pop();
    }

    fn lookup_type_param(&self, name: &str) -> Option<u32> {
        self.type_param_scope
            .iter()
            .rev()
            .find_map(|frame| frame.get(name).copied())
    }

    /// A type reference, admitting `A | B | ...` — see [`Self::resolve_union`]
    /// — before resolving one alternative on its own.
    fn resolve_type(&mut self, reference: &TypeRef) -> Type {
        if !reference.union_with.is_empty() {
            return self.resolve_union(reference);
        }
        self.resolve_type_atom(reference)
    }

    /// `String | Int32`: resolves each alternative, folds a bare `Null` one
    /// into the ordinary nullable bit instead of storing it (`T | Null` is
    /// `T?`), normalizes what remains — order-independent, deduplicated,
    /// subsumed alternatives collapsed into their supertype
    /// (`ZIRK_LANGUAGE_SPEC.md` section 4) — and interns it.
    ///
    /// Member access before narrowing (task 8.7's other half) is not
    /// implemented: a union type exists and type-checks, but nothing can be
    /// read from a value of one yet.
    fn resolve_union(&mut self, reference: &TypeRef) -> Type {
        let atoms: Vec<&TypeRef> = std::iter::once(reference)
            .chain(reference.union_with.iter())
            .collect();

        let mut nullable = false;
        let mut bases: Vec<Base> = Vec::new();
        for atom in atoms {
            if atom.name == "Null" && atom.arguments.is_empty() && !atom.nullable {
                nullable = true;
                continue;
            }
            let resolved = self.resolve_type_atom(atom);
            if resolved.nullable {
                nullable = true;
            }
            if resolved.is_unknown() {
                continue;
            }
            if !bases.contains(&resolved.base) {
                bases.push(resolved.base);
            }
        }

        // A class alternative that is a subclass of another in the same
        // union contributes nothing a value of the supertype does not
        // already cover.
        let snapshot = bases.clone();
        bases.retain(|&candidate| {
            !snapshot.iter().any(|&other| {
                other != candidate && self.is_subclass_of(Type::of(candidate), Type::of(other))
            })
        });

        Self::sort_bases(&mut bases);

        let base = match bases.len() {
            0 => Base::Null,
            1 => bases[0],
            _ => {
                let before = self.unions.len();
                let id = self.intern_union(bases);
                // Reported once, the first time this exact normalized union
                // is interned: the grammar and this normalization are real,
                // but nothing lowers a union yet (roadmap task 11.x), and
                // nothing reads a member of one before narrowing either
                // (`Self::member_type`'s union branch says so directly).
                if id as usize == before {
                    self.not_lowered(
                        reference.span,
                        "a union type",
                        "narrow it into a single type before using it, or avoid `|` for now",
                    );
                }
                Base::Union(id)
            }
        };
        Type { base, nullable }
    }

    /// A stable, arbitrary order for [`Base`] values, so two unions with the
    /// same members intern to the same id regardless of how each was
    /// written — `A | B` and `B | A` are one type.
    fn sort_bases(bases: &mut [Base]) {
        fn key(base: &Base) -> (u8, u32) {
            match *base {
                Base::Void => (0, 0),
                // Ten widths share one key group, ordered among themselves by
                // `IntWidth`'s own declaration order (its `Ord` derive).
                Base::Int(width) => (1, width as u32),
                // Same idea, one group over for the float family.
                Base::Float(width) => (2, width as u32),
                Base::Char => (3, 0),
                Base::Boolean => (4, 0),
                Base::String => (5, 0),
                Base::Null => (6, 0),
                Base::Range => (7, 0),
                Base::Unknown => (8, 0),
                Base::Enum(id) => (9, id),
                Base::Function(id) => (10, id),
                Base::Contract(id) => (11, id),
                Base::Class(id) => (12, id),
                Base::Param(id) => (13, id),
                Base::Instance(id) => (14, id),
                Base::ContractInstance(id) => (15, id),
                Base::EnumInstance(id) => (16, id),
                Base::Union(id) => (17, id),
            }
        }
        bases.sort_by_key(key);
    }

    /// Interns a normalized union, returning the id its [`Base::Union`]
    /// carries.
    fn intern_union(&mut self, bases: Vec<Base>) -> u32 {
        if let Some(index) = self.unions.iter().position(|u| *u == bases) {
            return index as u32;
        }
        self.unions.push(bases);
        (self.unions.len() - 1) as u32
    }

    /// One alternative of a type reference on its own — never a union; see
    /// [`Self::resolve_type`] for that.
    fn resolve_type_atom(&mut self, reference: &TypeRef) -> Type {
        let base = if let Some(id) = self.lookup_type_param(&reference.name) {
            Some(Type::of(Base::Param(id)))
        } else if reference.arguments.is_empty()
            && let Some(ty) = Type::from_name(&reference.name)
        {
            Some(ty)
        } else {
            let resolved = self.resolved_name(&reference.name, reference.span);
            let named = Ident::new(reference.name.clone(), reference.span);

            if self.type_aliases.contains_key(&resolved) {
                self.reject_type_arguments(reference);
                Some(self.resolve_type_alias(&resolved, reference.span))
            } else if let Some(index) = self.enums.iter().position(|e| e.name == resolved) {
                let declared = self.enums[index].span;
                let shared = self.enums[index].shared;
                self.require_visible(declared, shared, &named, "enum");
                Some(self.resolve_enum_reference(index as u32, reference))
            } else if let Some(index) = self.classes.iter().position(|c| c.name == resolved) {
                let declared = self.classes[index].span;
                let shared = self.classes[index].shared;
                self.require_visible(declared, shared, &named, "class");
                Some(self.resolve_class_reference(index as u32, reference))
            } else if let Some(index) = self.contracts.iter().position(|c| c.name == resolved) {
                let declared = self.contracts[index].span;
                let shared = self.contracts[index].shared;
                self.require_visible(declared, shared, &named, "contract");
                Some(self.resolve_contract_reference(index as u32, reference))
            } else {
                None
            }
        };

        if let Some(ty) = base {
            // `Void?` has no value to be absent, so it is not a type.
            if reference.nullable && ty == Type::VOID {
                self.error(
                    codes::UNKNOWN_TYPE,
                    reference.span,
                    "`Void?` is not a type",
                    "`Void` is the absence of a value, so it cannot also be absent",
                    Some("write `Void`".into()),
                );
                return Type::VOID;
            }

            return if reference.nullable {
                ty.as_nullable()
            } else {
                ty
            };
        }

        match pending_type(&reference.name) {
            Some(pending) => self.error(
                codes::UNKNOWN_TYPE,
                reference.span,
                format!("type `{}` is not implemented yet", reference.name),
                format!(
                    "the type exists in the language but arrives in Phase {}",
                    pending.phase
                ),
                Some(
                    "the available types are Void, Int32, Boolean, String, and declared enums and classes"
                        .into(),
                ),
            ),
            None => self.error(
                codes::UNKNOWN_TYPE,
                reference.span,
                format!("unknown type: `{}`", reference.name),
                "no type with that name exists in the language",
                Some(
                    "the available types are Void, Int32, Boolean, String, and declared enums and classes"
                        .into(),
                ),
            ),
        }

        Type::UNKNOWN
    }

    /// Resolves a `type` alias by resolving what it names, catching a cycle
    /// (`type A = B; type B = A;`) rather than recursing forever.
    fn resolve_type_alias(&mut self, name: &str, at: Span) -> Type {
        if self.resolving_aliases.contains(&name.to_string()) {
            let chain = self.resolving_aliases.join(" -> ");
            self.error(
                codes::DUPLICATE_DECLARATION,
                at,
                format!("`{name}` is defined in terms of itself"),
                format!("the chain closes on itself: {chain} -> {name}"),
                Some("a `type` alias cannot name itself, directly or indirectly".into()),
            );
            return Type::UNKNOWN;
        }

        let target = self.type_aliases[name].clone();
        self.resolving_aliases.push(name.to_string());
        let ty = self.resolve_type(&target);
        self.resolving_aliases.pop();
        ty
    }

    /// Reports `<...>` on a type reference that names a declaration with no
    /// type parameters of its own — a `type` alias, or an enum or contract
    /// declared without `<T>`.
    fn reject_type_arguments(&mut self, reference: &TypeRef) {
        if reference.arguments.is_empty() {
            return;
        }
        self.error(
            codes::PENDING_FEATURE,
            reference.span,
            "`<...>` is only valid on a generic declaration",
            format!(
                "`{}` was not declared with type parameters of its own",
                reference.name
            ),
            Some("remove the `<...>`, or add `<T>` to its declaration".into()),
        );
        for arg in &reference.arguments {
            self.resolve_type(arg);
        }
    }

    /// `Box` alone, or `Box<Int32>` with its arguments checked against
    /// `Box`'s own type parameters (roadmap task 7.3): the right arity, and
    /// each argument satisfying what its parameter's `from` constraints
    /// promise.
    fn resolve_class_reference(&mut self, class: u32, reference: &TypeRef) -> Type {
        if reference.arguments.is_empty() {
            return Type::of(Base::Class(class));
        }

        let expected = self.classes[class as usize].type_params.clone();
        if reference.arguments.len() != expected.len() {
            let name = self.classes[class as usize].name.clone();
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                reference.span,
                format!(
                    "`{name}` takes {} type argument{}, not {}",
                    expected.len(),
                    if expected.len() == 1 { "" } else { "s" },
                    reference.arguments.len()
                ),
                format!("`{name}` is declared with {} of its own", expected.len()),
                None,
            );
            for arg in &reference.arguments {
                self.resolve_type(arg);
            }
            return Type::of(Base::Class(class));
        }

        let args: Vec<Type> = reference
            .arguments
            .iter()
            .map(|a| self.resolve_type(a))
            .collect();

        for (&param_id, &arg) in expected.iter().zip(&args) {
            let constraints = self.type_params[param_id as usize].constraints.clone();
            for constraint in &constraints {
                if self.satisfies_constraint(arg, *constraint) {
                    continue;
                }
                let param_name = self.type_params[param_id as usize].name.clone();
                let arg_name = self.name(arg);
                let constraint_name = self.name(*constraint);
                self.error(
                    codes::TYPE_MISMATCH,
                    reference.span,
                    format!("`{arg_name}` does not satisfy `{param_name}`"),
                    format!(
                        "`{param_name}` requires `{constraint_name}`, which `{arg_name}` does not provide"
                    ),
                    None,
                );
            }
        }

        Type::of(Base::Instance(
            self.intern_instance(GenericInstance { class, args }),
        ))
    }

    /// Whether a type argument satisfies one of its parameter's `from`
    /// constraints: it either is the constraint, or (for a class) extends or
    /// implements it.
    fn satisfies_constraint(&self, arg: Type, constraint: Type) -> bool {
        arg == constraint || self.is_subclass_of(arg, constraint)
    }

    /// Interns a generic instantiation, returning the id its
    /// `Base::Instance` carries. `Box<Int32>` written twice, even in
    /// different files, resolves to the same id.
    fn intern_instance(&mut self, instance: GenericInstance) -> u32 {
        if let Some(index) = self.generic_instances.iter().position(|i| *i == instance) {
            return index as u32;
        }
        self.generic_instances.push(instance);
        (self.generic_instances.len() - 1) as u32
    }

    /// Interns a generic contract instantiation, returning the id its
    /// `Base::ContractInstance` carries. `Iterator<Int32>` written twice
    /// resolves to the same id, the same as [`Self::intern_instance`] for a
    /// class.
    fn intern_contract_instance(&mut self, instance: GenericContractInstance) -> u32 {
        if let Some(index) = self.contract_instances.iter().position(|i| *i == instance) {
            return index as u32;
        }
        self.contract_instances.push(instance);
        (self.contract_instances.len() - 1) as u32
    }

    /// Interns a generic enum instantiation, returning the id its
    /// `Base::EnumInstance` carries.
    fn intern_enum_instance(&mut self, instance: GenericEnumInstance) -> u32 {
        if let Some(index) = self.enum_instances.iter().position(|i| *i == instance) {
            return index as u32;
        }
        self.enum_instances.push(instance);
        (self.enum_instances.len() - 1) as u32
    }

    /// `Iterator` alone, or `Iterator<Int32>` with its arguments checked
    /// against `Iterator`'s own type parameters — the contract equivalent of
    /// [`Self::resolve_class_reference`] (task 6.9). A concrete instantiation
    /// is `NOT_LOWERED`: nothing downstream dispatches through it yet
    /// (roadmap task 10.7), and no generic enum it may return lowers either
    /// (11.3).
    fn resolve_contract_reference(&mut self, contract: u32, reference: &TypeRef) -> Type {
        let expected = self.contracts[contract as usize].type_params.clone();
        if expected.is_empty() {
            self.reject_type_arguments(reference);
            return Type::of(Base::Contract(contract));
        }

        if reference.arguments.len() != expected.len() {
            let name = self.contracts[contract as usize].name.clone();
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                reference.span,
                format!(
                    "`{name}` takes {} type argument{}, not {}",
                    expected.len(),
                    if expected.len() == 1 { "" } else { "s" },
                    reference.arguments.len()
                ),
                format!("`{name}` is declared with {} of its own", expected.len()),
                None,
            );
            for arg in &reference.arguments {
                self.resolve_type(arg);
            }
            return Type::of(Base::Contract(contract));
        }

        let args: Vec<Type> = reference
            .arguments
            .iter()
            .map(|a| self.resolve_type(a))
            .collect();

        for (&param_id, &arg) in expected.iter().zip(&args) {
            let constraints = self.type_params[param_id as usize].constraints.clone();
            for constraint in &constraints {
                if self.satisfies_constraint(arg, *constraint) {
                    continue;
                }
                let param_name = self.type_params[param_id as usize].name.clone();
                let arg_name = self.name(arg);
                let constraint_name = self.name(*constraint);
                self.error(
                    codes::TYPE_MISMATCH,
                    reference.span,
                    format!("`{arg_name}` does not satisfy `{param_name}`"),
                    format!(
                        "`{param_name}` requires `{constraint_name}`, which `{arg_name}` does not provide"
                    ),
                    None,
                );
            }
        }

        // `Iterator<T>`, written as `iterator()`'s return type, is the one
        // generic contract instantiation that lowers (roadmap task 13.5):
        // dispatch through a contract's table never depended on its type
        // arguments to begin with, so nothing more is needed to name it.
        if self.native_iteration.is_none_or(|n| contract != n.iterator) {
            self.not_lowered(
                reference.span,
                "a generic contract instantiation",
                "name the contract without `<...>` for now, or model the concrete case as its own type",
            );
        }

        Type::of(Base::ContractInstance(self.intern_contract_instance(
            GenericContractInstance { contract, args },
        )))
    }

    /// `Iteration` alone, or `Iteration<Int32>` with its arguments checked —
    /// the enum equivalent of [`Self::resolve_class_reference`]. Blocked by
    /// `NOT_LOWERED` the same way a class instantiation is (task 7.1): there
    /// is no monomorphization, and an algebraic variant with associated data
    /// does not lower yet regardless (8.1).
    fn resolve_enum_reference(&mut self, enum_id: u32, reference: &TypeRef) -> Type {
        let expected = self.enums[enum_id as usize].type_params.clone();
        if expected.is_empty() {
            self.reject_type_arguments(reference);
            return Type::of(Base::Enum(enum_id));
        }

        if reference.arguments.len() != expected.len() {
            let name = self.enums[enum_id as usize].name.clone();
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                reference.span,
                format!(
                    "`{name}` takes {} type argument{}, not {}",
                    expected.len(),
                    if expected.len() == 1 { "" } else { "s" },
                    reference.arguments.len()
                ),
                format!("`{name}` is declared with {} of its own", expected.len()),
                None,
            );
            for arg in &reference.arguments {
                self.resolve_type(arg);
            }
            return Type::of(Base::Enum(enum_id));
        }

        let args: Vec<Type> = reference
            .arguments
            .iter()
            .map(|a| self.resolve_type(a))
            .collect();

        for (&param_id, &arg) in expected.iter().zip(&args) {
            let constraints = self.type_params[param_id as usize].constraints.clone();
            for constraint in &constraints {
                if self.satisfies_constraint(arg, *constraint) {
                    continue;
                }
                let param_name = self.type_params[param_id as usize].name.clone();
                let arg_name = self.name(arg);
                let constraint_name = self.name(*constraint);
                self.error(
                    codes::TYPE_MISMATCH,
                    reference.span,
                    format!("`{arg_name}` does not satisfy `{param_name}`"),
                    format!(
                        "`{param_name}` requires `{constraint_name}`, which `{arg_name}` does not provide"
                    ),
                    None,
                );
            }
        }

        // `Iteration<T>`, written as `next()`'s return type, is the one
        // generic enum instantiation that lowers (roadmap task 13.5): a
        // dedicated specialization pass builds one concrete `EnumLayout` per
        // instantiation the program actually names, the same way a generic
        // class's does (11.1).
        if self.native_iteration.is_none_or(|n| enum_id != n.iteration) {
            self.not_lowered(
                reference.span,
                "a generic enum instantiation",
                "name the enum without `<...>` for now, or model the concrete case as its own type",
            );
        }

        Type::of(Base::EnumInstance(
            self.intern_enum_instance(GenericEnumInstance { enum_id, args }),
        ))
    }

    /// Replaces a contract's own type parameters with the concrete arguments
    /// of one instantiation (task 6.9) — what lets a class's signature for
    /// `Iterator<Int32>` compare equal to `Iterator<T>` with `T = Int32`
    /// substituted, instead of two structurally different types.
    ///
    /// Recurses into a nested generic instantiation's own arguments, since a
    /// contract method may return another parameterized type built from the
    /// same `T` (`Iterable<T>`'s `iterator(): Iterator<T>`).
    fn substitute_type(&mut self, ty: Type, subst: &[(u32, Type)]) -> Type {
        if let Base::Param(id) = ty.base
            && let Some(&(_, replacement)) = subst.iter().find(|(pid, _)| *pid == id)
        {
            return if ty.nullable {
                replacement.as_nullable()
            } else {
                replacement.without_null()
            };
        }

        let base = match ty.base {
            Base::ContractInstance(inst_id) => {
                let instance = self.contract_instances[inst_id as usize].clone();
                let args: Vec<Type> = instance
                    .args
                    .iter()
                    .map(|&a| self.substitute_type(a, subst))
                    .collect();
                Base::ContractInstance(self.intern_contract_instance(GenericContractInstance {
                    contract: instance.contract,
                    args,
                }))
            }
            Base::EnumInstance(inst_id) => {
                let instance = self.enum_instances[inst_id as usize].clone();
                let args: Vec<Type> = instance
                    .args
                    .iter()
                    .map(|&a| self.substitute_type(a, subst))
                    .collect();
                Base::EnumInstance(self.intern_enum_instance(GenericEnumInstance {
                    enum_id: instance.enum_id,
                    args,
                }))
            }
            Base::Instance(inst_id) => {
                let instance = self.generic_instances[inst_id as usize].clone();
                let args: Vec<Type> = instance
                    .args
                    .iter()
                    .map(|&a| self.substitute_type(a, subst))
                    .collect();
                Base::Instance(self.intern_instance(GenericInstance {
                    class: instance.class,
                    args,
                }))
            }
            other => other,
        };

        Type {
            base,
            nullable: ty.nullable,
        }
    }

    /// Interns a function type, returning the id its `Base::Function` carries.
    fn intern_fn_type(&mut self, fn_type: FnType) -> u32 {
        if let Some(index) = self.fn_types.iter().position(|f| *f == fn_type) {
            return index as u32;
        }
        self.fn_types.push(fn_type);
        (self.fn_types.len() - 1) as u32
    }

    // --- Functions --------------------------------------------------------

    fn check_function(&mut self, f: &FnDecl) {
        self.enter_type_params(&f.type_params);
        let signature = self.functions.get(&f.name.name).cloned();
        self.current_return = signature
            .as_ref()
            .map(|s| s.returns)
            .unwrap_or(Type::UNKNOWN);

        // A function body cannot see the locals of another: the barrier is what
        // makes a name from outside a capture rather than a plain read.
        self.scopes.push_function();

        let params = signature.map(|s| s.params).unwrap_or_default();
        for (param, info) in f.params.iter().zip(&params) {
            // The default is evaluated at the call site, but it is checked here
            // where the declared type is known.
            if let Some(default) = &param.default {
                let actual = self.check_expr(default);
                self.expect_assignable(info.ty, actual, default.span(), "the default value");
            }

            self.declare_local(Binding {
                name: info.name.clone(),
                ty: info.ty,
                mutability: Mutability::Immutable,
                span: param.name.span,
                initialized: true,
            });
        }

        let always_returns = self.check_block(&f.body);
        self.scopes.pop();

        // Every path of a non-`Void` function must return a value.
        if self.current_return != Type::VOID && !self.current_return.is_unknown() && !always_returns
        {
            let returns = self.name(self.current_return);
            self.error(
                codes::MISSING_RETURN,
                f.body.span,
                format!("not every path of `{}` returns a value", f.name.name),
                format!("the function declares `{returns}` as its return type"),
                Some("add a `return` at the end of the function".into()),
            );
        }
        self.leave_type_params();
    }

    /// Checks a block and reports whether every path through it returns.
    fn check_block(&mut self, block: &Block) -> bool {
        self.scopes.push();
        let returns = self.check_statements(&block.statements);
        self.scopes.pop();
        returns
    }

    fn check_statements(&mut self, statements: &[Stmt]) -> bool {
        let mut always_returns = false;
        for stmt in statements {
            if self.check_stmt(stmt) {
                always_returns = true;
            }
        }
        always_returns
    }

    // --- Statements -------------------------------------------------------

    /// Returns `true` when the statement guarantees the function has returned.
    fn check_stmt(&mut self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Let(s) => {
                self.check_let(s);
                false
            }
            Stmt::Assign(s) => {
                self.check_assign(s);
                false
            }
            Stmt::If(s) => self.check_if(s),
            Stmt::Loop(s) => self.check_loop(s),
            Stmt::ForIn(s) => {
                self.check_for_in(s);
                false
            }
            Stmt::Break(s) => {
                self.check_jump(s, "break");
                // `break` leaves the loop, and the loop's own analysis accounts
                // for it: from the enclosing block's point of view nothing is
                // guaranteed to have returned.
                false
            }
            Stmt::Continue(s) => {
                self.check_jump(s, "continue");
                false
            }
            Stmt::Return(s) => {
                self.check_return(s);
                true
            }
            // A `match` whose arms all return, and which covers every case,
            // guarantees the function returned — the same way an `if` with
            // both branches does.
            Stmt::Expr(ExprStmt {
                expr: Expr::Match(m),
                ..
            }) => self.check_match_statement(m),
            Stmt::Expr(s) => {
                self.check_expr(&s.expr);
                false
            }
            Stmt::Block(b) => self.check_block(b),
        }
    }

    fn check_let(&mut self, stmt: &LetStmt) {
        let annotated = stmt.ty.as_ref().map(|t| self.resolve_type(t));
        let initializer = stmt.init.as_ref().map(|e| self.check_expr(e));

        let ty = match (annotated, initializer) {
            (Some(declared), Some(actual)) => {
                let span = stmt.init.as_ref().map(|e| e.span()).unwrap_or(stmt.span);
                self.expect_assignable(declared, actual, span, "the initial value");
                declared
            }
            (Some(declared), None) => declared,
            // Inference is allowed where it is unambiguous
            // (`LANGUAGE_SPEC` section 2). With one integer type in the subset,
            // a literal always is.
            (None, Some(inferred)) => {
                // `mut x = null;` gives no base to infer: nullability alone is
                // not a type.
                if matches!(inferred.base, Base::Null) {
                    self.error(
                        codes::UNKNOWN_TYPE,
                        stmt.span,
                        format!("cannot infer the type of `{}`", stmt.name.name),
                        "`null` alone does not say which type is absent",
                        Some(format!("annotate it, as in `{}: String?`", stmt.name.name)),
                    );
                    Type::UNKNOWN
                } else {
                    inferred
                }
            }
            (None, None) => Type::UNKNOWN,
        };

        // `Void` has no representable value, so no variable can hold it.
        if ty == Type::VOID {
            self.error(
                codes::VOID_VARIABLE,
                stmt.span,
                format!("variable `{}` cannot be of type Void", stmt.name.name),
                "`Void` represents the absence of a value, so it cannot be stored",
                None,
            );
        }

        if let Some(init) = &stmt.init {
            self.check_strict_alias(init, ty, stmt.mutability, &stmt.name.name, stmt.name.span);
        }

        self.declare_local(Binding {
            name: stmt.name.name.clone(),
            ty,
            mutability: stmt.mutability,
            span: stmt.name.span,
            initialized: stmt.init.is_some(),
        });
    }

    /// The `mut`/`inmut`/`inmut::strict` matrix (D11), applied to objects and
    /// contracts: only naming another variable outright shares its reference,
    /// so that is the only shape this looks at. A strict reference must not
    /// gain a mutable alias, and it must not be acquired from one that is
    /// still reachable through its own `mut` binding.
    fn check_strict_alias(
        &mut self,
        init: &Expr,
        ty: Type,
        target_mutability: Mutability,
        target_name: &str,
        span: Span,
    ) {
        let Expr::Path(source) = init else { return };
        if !self.is_reference_type(ty) {
            return;
        }
        let Some(resolved) = self.scopes.resolve(&source.name) else {
            return;
        };

        match (resolved.binding.mutability, target_mutability) {
            (Mutability::Strict, Mutability::Mutable) => {
                self.error(
                    codes::STRICT_ALIAS_VIOLATION,
                    span,
                    format!(
                        "`{target_name}` would be a mutable alias of `{}`",
                        source.name
                    ),
                    format!(
                        "`{}` is `inmut::strict`, and a strict reference cannot produce a mutable alias",
                        source.name
                    ),
                    Some(format!("declare `{target_name}` with `inmut` or `inmut::strict`")),
                );
            }
            (Mutability::Mutable, Mutability::Strict) => {
                self.error(
                    codes::STRICT_ALIAS_VIOLATION,
                    span,
                    format!(
                        "`{target_name}` cannot be `inmut::strict`: `{}` is still a mutable alias",
                        source.name
                    ),
                    format!(
                        "`{}` was declared `mut`, so it can still mutate the same reachable graph",
                        source.name
                    ),
                    Some(format!(
                        "clone `{}` first, once `Clone` is available, or drop the `mut` binding before this point",
                        source.name
                    )),
                );
            }
            _ => {}
        }
    }

    /// Whether `ty` has reference semantics, so the strict-alias matrix
    /// applies to it. Records and value classes are inline (task 11.5), so
    /// they carry no aliasing to police.
    fn is_reference_type(&self, ty: Type) -> bool {
        match ty.base {
            Base::Class(id) => self.classes[id as usize].kind == ClassKind::Class,
            // A generic class's specialized instance is reached through its
            // address exactly the way an ordinary class's is (roadmap task
            // 11.1) — the strict-aliasing matrix (D11, task 5.15) applies
            // the same way; a record or value class's own `Base::Instance`
            // never reaches this arm to begin with, since one is inline and
            // never `is_reference_type` regardless of `T` (roadmap 11.5).
            Base::Instance(id) => {
                self.classes[self.generic_instances[id as usize].class as usize].kind
                    == ClassKind::Class
            }
            Base::Contract(_) | Base::ContractInstance(_) => true,
            _ => false,
        }
    }

    fn check_assign(&mut self, stmt: &AssignStmt) {
        let value = self.check_expr(&stmt.value);

        let AssignTarget::Name(name) = &stmt.target else {
            let AssignTarget::Field(field) = &stmt.target else {
                unreachable!("an assignment target is a name or a field")
            };
            let target = self.check_writable_field(field);
            self.expect_assignable(target, value, stmt.value.span(), "the assigned value");
            return;
        };

        let Some(target) = self.require_writable(name) else {
            return;
        };

        self.expect_assignable(target, value, stmt.value.span(), "the assigned value");
        self.scopes.mark_initialized(&name.name);
    }

    /// The type of a field being written to, reporting why it cannot be.
    ///
    /// A field is writable when its own `inmut` allows it. Where the object
    /// came from does not enter into it here: whether the *reference* permits
    /// mutation is the `mut`/`inmut`/`inmut::strict` matrix, which lands with
    /// the rest of reference mutability.
    fn check_writable_field(&mut self, field: &FieldExpr) -> Type {
        let object = self.check_expr(&field.object);
        let ty = self.member_type(object, &field.name, field.object.span(), false);

        // An `inmut` field is set exactly once, by the constructor: that is
        // where its value comes from. Rejecting it there would leave no way to
        // give it one.
        let in_own_constructor = self.in_constructor && matches!(&*field.object, Expr::This(_));

        if !in_own_constructor
            && let Base::Class(id) = object.base
            && let Some(info) = self.classes[id as usize].field(&field.name.name)
            && info.mutability == Mutability::Immutable
        {
            let line = self.sources.location(info.span).line;
            self.error(
                codes::ASSIGN_TO_IMMUTABLE,
                field.name.span,
                format!("cannot assign to `{}`", field.name.name),
                format!("the field is declared `inmut` on line {line}"),
                Some("an `inmut` field is set by the constructor and not again".into()),
            );
        }

        ty
    }

    /// Resolves a name that is about to be written to, reporting why it cannot
    /// be if that is the case.
    ///
    /// Shared by assignment and by the increment operators, which write to
    /// their operand and therefore demand the same thing of it.
    ///
    /// Returns the declared type, or `None` when the name does not resolve at
    /// all — an immutable binding still returns its type, so the rest of the
    /// expression is checked and one error does not cascade.
    fn require_writable(&mut self, target: &Ident) -> Option<Type> {
        let Some(resolved) = self.scopes.resolve(&target.name) else {
            self.undeclared(target);
            return None;
        };

        let ty = resolved.binding.ty;
        let declared_line = self.sources.location(resolved.binding.span).line;

        // A closure captures by value, so writing to a captured name would
        // silently update a copy. Decision D2.
        if resolved.captured {
            self.error(
                codes::CAPTURED_MUTATION,
                target.span,
                format!("cannot reassign `{}` inside a closure", target.name),
                format!("it is captured from line {declared_line}, and capture is by value"),
                Some(
                    "a closure captures immutable values; return the new value instead of writing to it"
                        .into(),
                ),
            );
            return None;
        }

        if resolved.binding.mutability != Mutability::Mutable {
            let qualifier = if resolved.binding.mutability == Mutability::Strict {
                "inmut::strict"
            } else {
                "inmut"
            };
            self.error(
                codes::ASSIGN_TO_IMMUTABLE,
                target.span,
                format!("cannot reassign `{}`", target.name),
                format!("it was declared with `{qualifier}` on line {declared_line}"),
                Some(format!(
                    "declare it with `mut {}` if it has to change",
                    target.name
                )),
            );
        }

        Some(ty)
    }

    fn check_if(&mut self, stmt: &IfStmt) -> bool {
        let condition = self.check_expr(&stmt.condition);
        self.expect_boolean(condition, stmt.condition.span(), "the condition of an `if`");

        let then_returns = self.check_block(&stmt.then_branch);

        // A conditional only guarantees a return when both branches do.
        match &stmt.else_branch {
            Some(ElseBranch::Block(b)) => then_returns & self.check_block(b),
            Some(ElseBranch::If(nested)) => then_returns & self.check_if(nested),
            None => false,
        }
    }

    /// `while`, `loop` and the three-clause `for`, which share a node.
    fn check_loop(&mut self, stmt: &LoopStmt) -> bool {
        // The initializer belongs to a scope enclosing the loop: `for (mut i =
        // 0; ...)` declares `i` for the condition and step too, but not after.
        self.scopes.push();

        if let Some(init) = &stmt.init {
            self.check_stmt(init);
        }

        if let Some(condition) = &stmt.condition {
            let ty = self.check_expr(condition);
            let context = format!("the condition of a `{}`", stmt.kind.as_str());
            self.expect_boolean(ty, condition.span(), &context);
        }

        self.loop_depth += 1;
        let body_returns = self.check_block(&stmt.body);
        self.loop_depth -= 1;

        if let Some(step) = &stmt.step {
            self.check_stmt(step);
        }

        self.scopes.pop();

        // Only a `loop` with no exit guarantees the code after it is
        // unreachable, which is what lets a function end on one without a
        // `return`. A conditional loop may run zero times.
        stmt.condition.is_none() && !self.contains_break(&stmt.body) && body_returns
    }

    /// Whether a block can leave its loop through `break`.
    ///
    /// Nested loops are skipped: their `break` belongs to them.
    fn contains_break(&self, block: &Block) -> bool {
        fn in_stmt(stmt: &Stmt) -> bool {
            match stmt {
                Stmt::Break(_) => true,
                Stmt::Block(b) => b.statements.iter().any(in_stmt),
                Stmt::If(i) => {
                    i.then_branch.statements.iter().any(in_stmt)
                        || match &i.else_branch {
                            Some(ElseBranch::Block(b)) => b.statements.iter().any(in_stmt),
                            Some(ElseBranch::If(nested)) => in_stmt(&Stmt::If((**nested).clone())),
                            None => false,
                        }
                }
                // A `break` inside a nested loop exits that one, not this.
                Stmt::Loop(_) | Stmt::ForIn(_) => false,
                _ => false,
            }
        }

        block.statements.iter().any(in_stmt)
    }

    fn check_for_in(&mut self, stmt: &ForInStmt) {
        // The range is checked directly here rather than through `check_expr`,
        // which rejects it: this is the one position where it is meaningful.
        let iterable = match &stmt.iterable {
            Expr::Range(range) => self.check_range(range),
            other => self.check_expr(other),
        };
        let element = self.element_type(iterable, stmt.iterable.span());

        self.scopes.push();
        self.declare_local(Binding {
            name: stmt.binding.name.clone(),
            ty: element,
            // The loop variable is rebound each iteration, not assigned.
            mutability: Mutability::Immutable,
            span: stmt.binding.span,
            initialized: true,
        });

        self.loop_depth += 1;
        self.check_block(&stmt.body);
        self.loop_depth -= 1;

        self.scopes.pop();
    }

    /// What `for ... in` binds for each element.
    ///
    /// `for ... in` requires `Iterable<T>` (D8, task 6.9): the closed set
    /// Phase 2 iterated is gone, replaced by a real conformance check, which
    /// is what makes a user's own type indistinguishable from a range or
    /// `String` here. A range still resolves natively rather than through a
    /// written `implements Iterable<Int32>` it does not have — nothing
    /// changes for it, and Phase 2's corpus keeps compiling unmodified.
    fn element_type(&mut self, iterable: Type, span: Span) -> Type {
        match iterable.base {
            Base::Range => Type::INT32,
            // A `String` iterates by grapheme and binds a `Char` — `Char`
            // itself exists now (roadmap Phase 3b, task 6.1), but the
            // lowering that walks a `String`'s graphemes at runtime does not
            // yet (task 6.3, still open debt from Phase 2): unlike the wait
            // for `Char` to exist at all, this is the grammar/type-rule-
            // exists-but-does-not-compile shape `not_lowered` names, not a
            // missing type.
            Base::String => {
                self.not_lowered(
                    span,
                    "iterating a `String`",
                    "iterate a range, as in `for i in 0..n`",
                );
                Type::UNKNOWN
            }
            Base::Unknown => Type::UNKNOWN,
            Base::Class(id) => {
                let Some(element) = self.iterable_element_type(id) else {
                    let name = self.name(iterable);
                    self.error(
                        codes::NOT_ITERABLE,
                        span,
                        format!("`{name}` cannot be iterated"),
                        "`for ... in` requires `Iterable<T>`, which this type does not implement",
                        Some(format!(
                            "add `implements Iterable<T>` to `{name}` and supply `iterator()`"
                        )),
                    );
                    return Type::UNKNOWN;
                };

                // Reaching an element calls through the class's contract
                // table (task 10.7) and matches against `Iteration<T>`'s
                // associated data (11.3/11.4) — both lower on their own by
                // now (roadmap task 13.5), but the specific `Iteration<T>`
                // this loop's element type needs is not derivable from the
                // AST the way most other types are (nothing writes `T`
                // here), so it is interned and recorded the same way a
                // generic construction's instantiation is
                // (`CheckedProgram::generic_constructions`).
                let iteration = self
                    .native_iteration
                    .expect("registered unconditionally before any program declaration")
                    .iteration;
                let instance = self.intern_enum_instance(GenericEnumInstance {
                    enum_id: iteration,
                    args: vec![element],
                });
                self.for_in_iteration.insert(span, instance);
                element
            }
            _ => {
                let name = self.name(iterable);
                self.error(
                    codes::NOT_ITERABLE,
                    span,
                    format!("`{name}` cannot be iterated"),
                    "`for ... in` requires `Iterable<T>`, which this type does not implement",
                    None,
                );
                Type::UNKNOWN
            }
        }
    }

    /// The `T` a class's `Iterable<T>` implementation binds, if it has one.
    fn iterable_element_type(&self, class: u32) -> Option<Type> {
        let iterable = self.native_iteration?.iterable;
        self.classes[class as usize]
            .contract_instances
            .iter()
            .find_map(|&id| {
                let instance = &self.contract_instances[id as usize];
                (instance.contract == iterable).then(|| instance.args[0])
            })
    }

    fn check_jump(&mut self, stmt: &JumpStmt, word: &str) {
        if self.loop_depth == 0 {
            self.error(
                codes::JUMP_OUTSIDE_LOOP,
                stmt.span,
                format!("`{word}` is not inside a loop"),
                format!("`{word}` only makes sense within `for`, `while` or `loop`"),
                None,
            );
        }
    }

    fn check_return(&mut self, stmt: &ReturnStmt) {
        let actual = match &stmt.value {
            Some(expr) => self.check_expr(expr),
            None => Type::VOID,
        };

        if self.current_return == Type::VOID && stmt.value.is_some() {
            self.error(
                codes::TYPE_MISMATCH,
                stmt.value.as_ref().map(|e| e.span()).unwrap_or(stmt.span),
                "a Void function cannot return a value",
                "the function declares `Void` as its return type",
                Some("write `return;` with no value".into()),
            );
            return;
        }

        if !self.current_return.accepts(actual)
            && !self.is_subclass_of(actual, self.current_return)
            && !self.bare_enum_matches_instance(actual, self.current_return)
        {
            let expected = self.name(self.current_return);
            let found = self.name(actual);
            self.error(
                codes::TYPE_MISMATCH,
                stmt.value.as_ref().map(|e| e.span()).unwrap_or(stmt.span),
                "the returned type does not match the signature",
                format!("the function declares `{expected}` and this returns `{found}`"),
                None,
            );
        }
    }

    // --- Expressions ------------------------------------------------------

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Int(lit) => self.check_int_literal(lit),
            Expr::Float(lit) => self.check_float_literal(lit),
            Expr::Char(lit) => self.check_char_literal(lit),
            Expr::Str(_) => Type::STRING,
            Expr::Bool(_) => Type::BOOLEAN,
            Expr::Null(_) => Type::NULL,
            Expr::Path(ident) => self.check_path(ident),
            Expr::Unary(e) => self.check_unary(e),
            Expr::Binary(e) => self.check_binary(e),
            Expr::Call(e) => self.check_call(e),
            // A range is not a value: there is no `Range` type to hold one
            // until Phase 3 brings collections. It only means something as the
            // iterable of a `for ... in`, which checks it directly.
            Expr::Range(e) => {
                self.check_range(e);
                self.error(
                    codes::TYPE_MISMATCH,
                    e.span,
                    "a range is not a value",
                    "it can only be iterated, not stored or passed around",
                    Some("write it directly in a `for ... in`".into()),
                );
                Type::UNKNOWN
            }
            Expr::If(e) => self.check_if_expr(e),
            Expr::This(e) => match self.this_type {
                Some(ty) => {
                    // `this` used inside a lambda is a capture exactly like
                    // any other name from the enclosing scope — it just does
                    // not go through `check_path`/`Scopes::resolve`, since it
                    // is not an ordinary binding.
                    if !self.capture_stack.is_empty() {
                        self.record_this_capture(ty);
                    }
                    ty
                }
                None => {
                    self.error(
                        codes::UNDECLARED_NAME,
                        e.span,
                        "`this` is only available inside a class",
                        "it names the instance a constructor or method runs on",
                        None,
                    );
                    Type::UNKNOWN
                }
            },
            Expr::Super(e) => {
                self.error(
                    codes::TYPE_MISMATCH,
                    e.span,
                    "`super` is not a value",
                    "it selects where to look, so it only means something in `super(...)` or `super.method()`",
                    Some("write `this` to refer to the instance".into()),
                );
                Type::UNKNOWN
            }
            Expr::Field(e) => self.check_field(e),
            Expr::Ternary(e) => self.check_ternary(e),
            Expr::Increment(e) => self.check_increment(e),
            Expr::Match(e) => self.check_match(e, true),
            Expr::Lambda(e) => self.check_lambda(e),
            Expr::Variant(e) => self.check_variant(e),
            Expr::Println(e) => {
                let ty = self.check_expr(&e.arg);
                self.require_printable(ty, e.arg.span());
                Type::VOID
            }
            Expr::Cast(e) => self.check_cast(e),
            Expr::Interpolated(e) => self.check_interpolated(e),
        }
    }

    /// `"text {expr} text"` (roadmap Phase 3b).
    ///
    /// Each `{expr}` needs exactly what `println`'s own argument does — a
    /// text form to convert to — so it is checked through the same gate
    /// (`require_printable`) rather than a rule of its own. Once `to_string()`
    /// exists as a real contract (task 8 of the phase), both gain a proper
    /// user-extensible text form together; today both mean "one of the
    /// runtime's own three printable types".
    fn check_interpolated(&mut self, expr: &InterpolatedStrExpr) -> Type {
        for part in &expr.parts {
            if let InterpolatedPart::Expr(inner) = part {
                let ty = self.check_expr(inner);
                self.require_printable(ty, inner.span());
            }
        }
        Type::STRING
    }

    /// `expr as Type` or `<Type>expr` (`ZIRK_LANGUAGE_SPEC.md` section 11).
    ///
    /// Admits a cast between types that could plausibly share a value at
    /// runtime — the same type, a class and its base or a contract it may
    /// implement, or a union and one of its members — and rejects one
    /// between types that never could, such as `String` and `Int32`, at
    /// compile time rather than waiting for a check that could never pass.
    ///
    /// Not lowered yet: nothing runs the checked failure a cast that turns
    /// out wrong needs at runtime (roadmap task 11.6).
    fn check_cast(&mut self, expr: &CastExpr) -> Type {
        let actual = self.check_expr(&expr.expr);
        let target = self.resolve_type(&expr.target);

        if actual.is_unknown() || target.is_unknown() {
            return target;
        }

        if !self.casts_are_related(actual, target) {
            let a = self.name(actual);
            let t = self.name(target);
            self.error(
                codes::TYPE_MISMATCH,
                expr.span,
                format!("`{a}` cannot be cast to `{t}`"),
                "neither is a class related to the other, a contract either may implement, or a union containing the other",
                None,
            );
            return Type::UNKNOWN;
        }

        if !self.cast_is_directly_lowerable(actual, target) {
            self.not_lowered(
                expr.span,
                "this cast",
                "convert the value some other way for now, e.g. through a constructor",
            );
        }
        target
    }

    /// Whether a related cast (`Self::casts_are_related` already confirmed
    /// it is one) has a runtime check this pass builds (roadmap task 11.6).
    ///
    /// Three shapes: the trivial identity cast (same base, any kind — no
    /// check needed, the value already is the target), a class or contract
    /// value checked against a class's descriptor, and any integer width to
    /// any other (roadmap Phase 3b, task 4.3 — truncate/sign-extend/
    /// zero-extend, unchecked, exactly like Rust's own `as`; there is no
    /// `Result`-returning safe conversion yet to make it checked, since
    /// `Result` itself is Phase 4). Left out: a union member (unions do not
    /// lower at all yet, task 8.7/11.x) and a generic parameter's
    /// constraint (the concrete type behind `T` is not known here the way
    /// an instantiation's substitution is). Nullable either side is left
    /// out too — the check would also have to decide what a null value
    /// means for it, which is a second question this does not answer yet.
    fn cast_is_directly_lowerable(&self, actual: Type, target: Type) -> bool {
        if actual.nullable || target.nullable {
            return false;
        }
        if actual.base == target.base {
            return true;
        }
        // Any integer width to any other lowers unchecked — truncating or
        // sign/zero-extending, exactly like Rust's own `as` between
        // integers (roadmap Phase 3b, task 4.3). Same-signedness widening
        // is additionally *implicit* (`Type::accepts`'s own safe case);
        // `as` here is what makes narrowing and sign-crossing possible at
        // all, since there is no `Result`-returning safe conversion yet
        // (`ZIRK_LANGUAGE_SPEC.md` section 11 names one, but it needs
        // `Result`, which is Phase 4) — `as` is the only path there is.
        if let (Base::Int(_), Base::Int(_)) = (target.base, actual.base) {
            return true;
        }
        // `Float` to `Float`, any width, unchecked (LLVM's `fptrunc`/`fpext`,
        // roadmap Phase 3b): narrowing may lose precision or overflow to an
        // infinity, exactly the kind of explicit-only conversion `as` exists
        // for. Int-to-Float and Float-to-Int are the same idea across
        // families — `fptosi`/`fptoui`/`sitofp`/`uitofp` — and equally
        // unchecked: a `Float` that does not fit the target integer's range,
        // or an integer whose magnitude a narrow `Float` cannot represent
        // exactly, is on the same footing as any other narrowing `as`.
        if let (Base::Float(_), Base::Float(_)) = (target.base, actual.base) {
            return true;
        }
        if matches!(
            (target.base, actual.base),
            (Base::Int(_), Base::Float(_)) | (Base::Float(_), Base::Int(_))
        ) {
            return true;
        }
        matches!(actual.base, Base::Class(_) | Base::Contract(_))
            && matches!(target.base, Base::Class(_))
    }

    /// Whether a cast between two types could ever succeed at runtime.
    fn casts_are_related(&self, actual: Type, target: Type) -> bool {
        let actual_bare = actual.without_null();
        let target_bare = target.without_null();

        if actual_bare.base == target_bare.base {
            return true;
        }
        // Any integer width may be cast to any other: narrowing and
        // sign-crossing conversions (roadmap Phase 3b, task 4.3) are exactly
        // the "explicit" half of the widening/narrowing rule, still gated
        // with `NOT_LOWERED` below until the checked runtime conversion
        // itself is built — `casts_are_related` only answers "could this
        // ever succeed", not "does it lower yet".
        if matches!(actual_bare.base, Base::Int(_)) && matches!(target_bare.base, Base::Int(_)) {
            return true;
        }
        // Same idea across `Float` widths and between `Int` and `Float`
        // (roadmap Phase 3b, task 4.3/task 5) — see `cast_is_directly_lowerable`.
        if matches!(actual_bare.base, Base::Float(_)) && matches!(target_bare.base, Base::Float(_))
        {
            return true;
        }
        if matches!(
            (actual_bare.base, target_bare.base),
            (Base::Int(_), Base::Float(_)) | (Base::Float(_), Base::Int(_))
        ) {
            return true;
        }
        if self.is_subclass_of(actual_bare, target_bare)
            || self.is_subclass_of(target_bare, actual_bare)
        {
            return true;
        }
        // A contract may be implemented by the other side's class, checked
        // either way: up through it, or down from it to a concrete type.
        if matches!(actual_bare.base, Base::Contract(_))
            && matches!(target_bare.base, Base::Class(_))
        {
            return true;
        }
        if let Base::Union(id) = actual_bare.base {
            return self.unions[id as usize]
                .iter()
                .any(|&member| Type::of(member).base == target_bare.base);
        }
        if let Base::Union(id) = target_bare.base {
            return self.unions[id as usize]
                .iter()
                .any(|&member| Type::of(member).base == actual_bare.base);
        }
        // A generic parameter is related to whatever its own `from`
        // constraints promise: casting to one of them is a widening, not a
        // leap of faith, and the reverse (something related to a
        // constraint, cast down to `T`) is checked the same way any other
        // up- or downcast between classes already is above.
        if let Base::Param(id) = actual_bare.base {
            return self.type_params[id as usize].constraints.iter().any(|&c| {
                c.base == target_bare.base
                    || self.is_subclass_of(target_bare, c)
                    || self.is_subclass_of(c, target_bare)
            });
        }
        if let Base::Param(id) = target_bare.base {
            return self.type_params[id as usize].constraints.iter().any(|&c| {
                c.base == actual_bare.base
                    || self.is_subclass_of(actual_bare, c)
                    || self.is_subclass_of(c, actual_bare)
            });
        }
        false
    }

    /// Integer literals are `Int32`, the only integer type in the subset.
    ///
    /// The value is kept in `i128` by the lexer precisely so overflow of the
    /// destination type is detected here rather than lost while parsing
    /// (`LANGUAGE_SPEC` section 3: ordinary overflow is a controlled error).
    fn check_int_literal(&mut self, lit: &IntLit) -> Type {
        if let Some((min, max)) = Type::INT32.integer_range()
            && (lit.value < min || lit.value > max)
        {
            self.error(
                codes::INTEGER_OUT_OF_RANGE,
                lit.span,
                format!("the literal {} does not fit in Int32", lit.value),
                format!("Int32 admits values from {min} to {max}"),
                None,
            );
            return Type::UNKNOWN;
        }
        Type::INT32
    }

    /// A fractional or scientific literal is `Float64` unless it names an
    /// explicit width suffix (roadmap Phase 3b) — there is no context-directed
    /// inference yet (task 7, deep contextual conversion covers the operator
    /// case; a bare literal's own destination is separate and not built).
    ///
    /// The magnitude check parses the text as `f64` purely to catch a literal
    /// that overflows to infinity in its destination width — it does not
    /// decide the literal's runtime value, which `zirk-codegen-llvm` builds
    /// straight from this same text through LLVM's own parser, at the actual
    /// destination width. `Float128` is exempted: its true range vastly
    /// exceeds what an `f64` parse can even represent, so there is no `f64`
    /// bound to check it against without a false result — see
    /// `FloatWidth::literal_bound`.
    fn check_float_literal(&mut self, lit: &FloatLit) -> Type {
        use FloatWidth::*;
        let width = match lit.width.as_deref() {
            None => F64,
            Some("f16") => F16,
            Some("f32") => F32,
            Some("f64") => F64,
            Some("f128") => F128,
            Some(_) => F64,
        };

        let value: f64 = lit.text.parse().unwrap_or(f64::NAN);
        if let Some(bound) = width.literal_bound()
            && value.abs() > bound
        {
            self.error(
                codes::INTEGER_OUT_OF_RANGE,
                lit.span,
                format!("the literal {} does not fit in {}", lit.text, width.name()),
                format!("{} admits magnitudes up to {bound}", width.name()),
                None,
            );
            return Type::UNKNOWN;
        }

        Type::of(Base::Float(width))
    }

    /// A `Char` literal must be exactly one Unicode grapheme
    /// (`ZIRK_LANGUAGE_SPEC.md` section 3) — deciding that needs Unicode
    /// segmentation (UAX #29), which the lexer deliberately does not have
    /// (`zirk_lexer::character()`'s own doc comment); this is the one place
    /// that segments, since a `Char` value can only be built from a literal
    /// today (there is no `for ... in` over `String` yet, task 6.3).
    fn check_char_literal(&mut self, lit: &CharLit) -> Type {
        let graphemes = lit.value.graphemes(true).count();
        if graphemes != 1 {
            let found = if graphemes == 0 {
                "it is empty".to_string()
            } else {
                format!("it holds {graphemes} graphemes")
            };
            self.error(
                codes::INVALID_CHAR_LITERAL,
                lit.span,
                "a character literal must be exactly one Unicode grapheme",
                found,
                None,
            );
            return Type::UNKNOWN;
        }
        Type::of(Base::Char)
    }

    fn check_path(&mut self, ident: &Ident) -> Type {
        // A bare function name is a value: that is what lets a function be
        // passed where a closure is expected.
        let declared = self.resolved_name(&ident.name, ident.span);
        if let Some(signature) = self.functions.get(&declared).cloned()
            && self.scopes.lookup(&ident.name).is_none()
        {
            let id = self.intern_fn_type(FnType {
                params: signature.param_types(),
                returns: signature.returns,
            });
            return Type::of(Base::Function(id));
        }

        let Some(resolved) = self.scopes.resolve(&ident.name) else {
            self.undeclared(ident);
            return Type::UNKNOWN;
        };

        if !resolved.binding.initialized {
            let line = self.sources.location(resolved.binding.span).line;
            self.error(
                codes::USE_BEFORE_INITIALIZATION,
                ident.span,
                format!("`{}` is read before it holds a value", ident.name),
                format!("it was declared without an initializer on line {line}"),
                Some("assign a value to it before reading it".into()),
            );
        }

        // Reading a name from outside this lambda is what makes it a capture.
        if resolved.captured {
            self.record_capture(&resolved.binding);
        }

        resolved.binding.ty
    }

    fn record_capture(&mut self, binding: &Binding) {
        let Some(captures) = self.capture_stack.last_mut() else {
            return;
        };
        if captures.iter().any(|c| c.name == binding.name) {
            return;
        }
        captures.push(Capture {
            name: binding.name.clone(),
            ty: binding.ty,
        });
    }

    /// Records `this` as a capture, the same way [`Self::record_capture`]
    /// does for an ordinary name.
    fn record_this_capture(&mut self, ty: Type) {
        let Some(captures) = self.capture_stack.last_mut() else {
            return;
        };
        if captures.iter().any(|c| c.name == "this") {
            return;
        }
        captures.push(Capture {
            name: "this".to_string(),
            ty,
        });
    }

    fn check_unary(&mut self, expr: &UnaryExpr) -> Type {
        let operand = self.check_expr(&expr.operand);

        match expr.op {
            UnaryOp::Neg => {
                // Only a signed integer width: negating an unsigned value has
                // no representable result in its own type (roadmap Phase 3b).
                // Every `Float` width is signed by construction (IEEE 754's
                // sign bit), so it needs no equivalent guard.
                let ok = !operand.nullable
                    && (matches!(operand.base, Base::Int(w) if w.signed())
                        || matches!(operand.base, Base::Float(_)));
                if !ok && !operand.is_unknown() {
                    let found = self.name(operand);
                    self.error(
                        codes::TYPE_MISMATCH,
                        expr.span,
                        "the `-` operator requires a signed number",
                        format!("it was applied to a value of type {found}"),
                        None,
                    );
                    return Type::UNKNOWN;
                }
                if operand.is_unknown() {
                    Type::UNKNOWN
                } else {
                    operand
                }
            }
            UnaryOp::Not => {
                self.expect_boolean(operand, expr.span, "the operand of `!`");
                Type::BOOLEAN
            }
            UnaryOp::BitNot => {
                let ok = !operand.nullable && matches!(operand.base, Base::Int(_));
                if !ok && !operand.is_unknown() {
                    let found = self.name(operand);
                    self.error(
                        codes::TYPE_MISMATCH,
                        expr.span,
                        "the `~` operator requires a number",
                        format!("it was applied to a value of type {found}"),
                        None,
                    );
                    return Type::UNKNOWN;
                }
                if operand.is_unknown() {
                    Type::UNKNOWN
                } else {
                    operand
                }
            }
        }
    }

    fn check_binary(&mut self, expr: &BinaryExpr) -> Type {
        let left = self.check_expr(&expr.left);
        let right = self.check_expr(&expr.right);

        use BinaryOp::*;
        match expr.op {
            // `??` is the one operator whose whole purpose is nullability, so
            // it is checked before the rules that reject nullable operands.
            Coalesce => self.check_coalesce(left, right, expr),

            // Logical operators accept booleans only: there is no numeric
            // truthiness (`LANGUAGE_SPEC` section 3).
            And | Or => {
                self.expect_boolean(left, expr.left.span(), "the left operand");
                self.expect_boolean(right, expr.right.span(), "the right operand");
                Type::BOOLEAN
            }

            // Equality is structural and requires both sides to share a type.
            // `is` asks about identity, so it needs types with identity and
            // nothing else: no contract, no structure, no content.
            Is => {
                self.expect_same(left, right, expr);
                let has_identity = match left.base {
                    Base::Class(id) => self.classes[id as usize].kind == ClassKind::Class,
                    Base::Contract(_) | Base::String => true,
                    _ => false,
                };
                if !has_identity && !left.is_unknown() {
                    let name = self.name(left);
                    self.error(
                        codes::TYPE_MISMATCH,
                        expr.op_span,
                        format!("`{name}` has no identity to compare"),
                        "`is` asks whether two references name the same instance, and a value is not a reference",
                        Some("compare content with `==`".into()),
                    );
                }
                Type::BOOLEAN
            }

            Eq | NotEq => {
                self.expect_same(left, right, expr);
                self.reject_nullable_comparison(left, right, expr);
                self.reject_closure_comparison(left, right, expr);
                self.reject_unstructured_comparison(left, expr);
                Type::BOOLEAN
            }

            // Comparison only makes sense on numbers in this subset —
            // integer or `Float` (roadmap Phase 3b), unlike bitwise/shift
            // which stay integer-only, so this does not reuse
            // `expect_numeric`.
            Lt | LtEq | Gt | GtEq => {
                self.expect_comparable(left, expr.left.span(), expr.op);
                self.expect_comparable(right, expr.right.span(), expr.op);
                // Each side is some width on its own; comparing two different
                // ones — including an integer against a `Float` — needs an
                // explicit conversion first, the same rule arithmetic has for
                // integers (roadmap Phase 3b, task 4.3). Arithmetic's mixed
                // int/Float rule does not extend here: which side's width the
                // comparison would run at is not the unambiguous "produces
                // Float" the spec states for arithmetic, so it is left
                // explicit rather than guessed.
                if !left.is_unknown() && !right.is_unknown() && left.base != right.base {
                    let l = self.name(left);
                    let r = self.name(right);
                    self.error(
                        codes::TYPE_MISMATCH,
                        expr.op_span,
                        format!("cannot compare {l} with {r}"),
                        "comparison requires both sides to be the same numeric type",
                        None,
                    );
                }
                Type::BOOLEAN
            }

            Add | Sub | Mul | Div | Rem => self.check_arithmetic(left, right, expr),

            // `&`/`|`/`^` need the same width on both sides, the same rule
            // arithmetic has; a shift's amount does not — `1i8 << 2i32` asks
            // "shift this Int8 by 2 places", and the amount's own width
            // carries no meaning beyond that. Roadmap Phase 3b, task 4.4.
            BitAnd | BitOr | BitXor => {
                self.expect_numeric(left, expr.left.span(), expr.op);
                self.expect_numeric(right, expr.right.span(), expr.op);
                if left.is_unknown() || right.is_unknown() {
                    return Type::UNKNOWN;
                }
                if left.base != right.base {
                    let l = self.name(left);
                    let r = self.name(right);
                    self.error(
                        codes::TYPE_MISMATCH,
                        expr.op_span,
                        format!(
                            "`{}` requires both sides to be the same integer type",
                            expr.op.as_str()
                        ),
                        format!("found {l} and {r}"),
                        None,
                    );
                    return Type::UNKNOWN;
                }
                left
            }
            Shl | Shr => {
                self.expect_numeric(left, expr.left.span(), expr.op);
                self.expect_numeric(right, expr.right.span(), expr.op);
                if left.is_unknown() {
                    Type::UNKNOWN
                } else {
                    left
                }
            }
        }
    }

    /// Rejects `==` on a type that has not said what equality means for it.
    ///
    /// `ZIRK_LANGUAGE_SPEC.md` section 4 makes `==` structural, and section 7
    /// says equality exists only through an explicit contract. Without one
    /// there is nothing to compare structurally: answering by address would be
    /// `is` wearing the wrong operator, and answering `true` because the types
    /// match would be worse still.
    fn reject_unstructured_comparison(&mut self, left: Type, expr: &BinaryExpr) {
        let (Base::Class(_) | Base::Contract(_)) = left.base else {
            return;
        };

        // A user type says what equality means through the reserved method,
        // the same way it supplies any other operator.
        if let Base::Class(id) = left.base
            && self.classes[id as usize].method("_equals").is_some()
        {
            return;
        }

        // A record or value class always has one: equality is derived from
        // every field, per `ZIRK_LANGUAGE_SPEC.md` section 7, not opted into
        // with a reserved method the way a plain class's is. Lowering it —
        // a field-by-field comparison, recursing into a nested record —
        // does not exist yet for anything beyond a scalar or nested-value
        // field (roadmap task 11.5 built the value representation itself,
        // not the comparison over it), so it is gated the same way any
        // other checked-but-not-compilable construct is.
        if let Base::Class(id) = left.base
            && matches!(
                self.classes[id as usize].kind,
                ClassKind::Record | ClassKind::ValueClass
            )
        {
            self.not_lowered(
                expr.span,
                "structural equality on a record or value class",
                "compare its fields individually for now",
            );
            return;
        }
        if let Base::Class(id) = left.base
            && self.classes[id as usize].kind != ClassKind::Class
        {
            return;
        }

        let name = self.name(left);
        let help = match left.base {
            Base::Class(_) => format!(
                "implement `fn _equals(other: {name}): Boolean`, or compare identity with `is`"
            ),
            _ => "compare identity with `is`, or require an equality contract".to_string(),
        };
        self.error(
            codes::TYPE_MISMATCH,
            expr.op_span,
            format!("`{name}` does not define equality"),
            "`==` compares content, and this type has not said what its content comparison is",
            Some(help),
        );
    }

    /// An arithmetic operator, resolved by what its operands support.
    ///
    /// The checker no longer holds a fixed list of types per operator: it asks
    /// what the operands offer. Integers still resolve directly — they belong
    /// to the language, not to a library — but through the same path a user
    /// type takes, which is what makes the two indistinguishable at the use
    /// site (decision D6).
    fn check_arithmetic(&mut self, left: Type, right: Type, expr: &BinaryExpr) -> Type {
        if left.is_unknown() || right.is_unknown() {
            return Type::UNKNOWN;
        }

        if let Some(ty) = native_arithmetic(left, right, expr.op) {
            return ty;
        }

        // A user type supplies the operator through its reserved method.
        if let Base::Class(id) = left.base
            && !left.nullable
        {
            let reserved = operator_method(expr.op);
            if let Some(method) = self.classes[id as usize].method(reserved).cloned() {
                let expected = method.params.first().map(|p| p.ty);
                if method.params.len() == 1
                    && expected
                        .is_some_and(|ty| ty.accepts(right) || self.is_subclass_of(right, ty))
                {
                    return method.returns;
                }

                let class = self.name(left);
                let r = self.name(right);
                self.error(
                    codes::TYPE_MISMATCH,
                    expr.op_span,
                    format!("`{}` on `{class}` does not accept {r}", expr.op.as_str()),
                    format!("`{reserved}` declares a different operand"),
                    None,
                );
                return Type::UNKNOWN;
            }
        }

        self.reject_operator(left, right, expr);
        Type::UNKNOWN
    }

    /// Reports an operator neither operand supplies.
    fn reject_operator(&mut self, left: Type, right: Type, expr: &BinaryExpr) {
        let l = self.name(left);
        let r = self.name(right);
        let op = expr.op.as_str();

        // Naming the reserved method turns "this does not work" into
        // "here is what would make it work".
        let help = match left.base {
            Base::Class(_) => Some(format!(
                "implement `fn {}(other: {r}): ...` on `{l}`",
                operator_method(expr.op)
            )),
            _ => None,
        };

        self.error(
            codes::TYPE_MISMATCH,
            expr.op_span,
            format!("`{op}` is not available on {l} and {r}"),
            "an operator comes from what its operands support",
            help,
        );
    }

    fn check_coalesce(&mut self, left: Type, right: Type, expr: &BinaryExpr) -> Type {
        if !left.admits_null() && !left.is_unknown() {
            let found = self.name(left);
            self.error(
                codes::REDUNDANT_OPERATOR,
                expr.op_span,
                "`??` on a value that is never null",
                format!("the left operand has type `{found}`, which always holds a value"),
                Some("remove the `??` and its fallback".into()),
            );
            return left;
        }

        let Some(unified) = left.without_null().unify(right) else {
            let l = self.name(left);
            let r = self.name(right);
            self.error(
                codes::TYPE_MISMATCH,
                expr.op_span,
                format!("`??` cannot combine {l} with {r}"),
                "both sides must share a type: the fallback replaces the value",
                None,
            );
            return Type::UNKNOWN;
        };

        unified
    }

    fn check_range(&mut self, expr: &RangeExpr) -> Type {
        let start = self.check_expr(&expr.start);
        let end = self.check_expr(&expr.end);

        self.expect_numeric_value(start, expr.start.span(), "the start of a range");
        self.expect_numeric_value(end, expr.end.span(), "the end of a range");

        Type::RANGE
    }

    /// `if` used where a value is expected. Decision D7.
    fn check_if_expr(&mut self, stmt: &IfStmt) -> Type {
        let condition = self.check_expr(&stmt.condition);
        self.expect_boolean(condition, stmt.condition.span(), "the condition of an `if`");

        let then_type = self.check_block_value(&stmt.then_branch);

        let else_type = match &stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.check_block_value(b),
            Some(ElseBranch::If(nested)) => self.check_if_expr(nested),
            None => {
                self.error(
                    codes::IF_WITHOUT_ELSE,
                    stmt.span,
                    "an `if` used as a value needs an `else`",
                    "without the alternative branch there is no value when the condition is false",
                    Some("add `else { ... }`, or use the `if` as a statement".into()),
                );
                return Type::UNKNOWN;
            }
        };

        match then_type.unify(else_type) {
            Some(ty) => ty,
            None => {
                let t = self.name(then_type);
                let e = self.name(else_type);
                self.error(
                    codes::TYPE_MISMATCH,
                    stmt.span,
                    "the branches of the `if` produce different types",
                    format!("one branch produces {t} and the other {e}"),
                    Some("both branches must agree for the `if` to be a value".into()),
                );
                Type::UNKNOWN
            }
        }
    }

    /// `cond ? a : b`.
    ///
    /// The same rules as the `if` expression — Boolean condition, branches that
    /// agree on a type — with wording that names the ternary, because that is
    /// what the author wrote.
    fn check_ternary(&mut self, expr: &TernaryExpr) -> Type {
        let condition = self.check_expr(&expr.condition);
        self.expect_boolean(
            condition,
            expr.condition.span(),
            "the condition of a ternary",
        );

        let when_true = self.check_expr(&expr.when_true);
        let when_false = self.check_expr(&expr.when_false);

        match when_true.unify(when_false) {
            Some(ty) => ty,
            None => {
                let t = self.name(when_true);
                let f = self.name(when_false);
                self.error(
                    codes::TYPE_MISMATCH,
                    expr.op_span,
                    "the branches of the ternary produce different types",
                    format!("one branch produces {t} and the other {f}"),
                    Some("both branches must agree, since either can be the result".into()),
                );
                Type::UNKNOWN
            }
        }
    }

    /// `i++`, `++i`, `i--` and `--i` where a value is expected.
    ///
    /// Both forms write to their operand, so they demand exactly what an
    /// assignment does. Which value they produce is a lowering concern
    /// (`LANGUAGE_SPEC` section 4); the type is the same either way.
    fn check_increment(&mut self, expr: &IncrementExpr) -> Type {
        let AssignTarget::Name(name) = &expr.target else {
            self.not_checked(
                expr.target.span(),
                "incrementing a field",
                "writing a member needs a type with members",
            );
            return Type::UNKNOWN;
        };

        let Some(ty) = self.require_writable(name) else {
            return Type::UNKNOWN;
        };

        if !ty.is_unknown() && !Type::INT32.accepts(ty) {
            let found = self.name(ty);
            self.error(
                codes::TYPE_MISMATCH,
                expr.op_span,
                format!("`{}` requires a number", expr.op.as_str()),
                format!("`{}` has type {found}", expr.target.name()),
                None,
            );
            return Type::UNKNOWN;
        }

        ty
    }

    /// The value a block produces, which is that of its last statement.
    ///
    /// Semicolons are optional in Zirk (`LANGUAGE_SPEC` section 1), so unlike
    /// Rust their presence cannot mark the tail: the last statement is the
    /// value when it is an expression, and there is no value otherwise.
    fn check_block_value(&mut self, block: &Block) -> Type {
        self.scopes.push();

        let (last, rest) = match block.statements.split_last() {
            Some(split) => split,
            None => {
                self.scopes.pop();
                self.error(
                    codes::TYPE_MISMATCH,
                    block.span,
                    "an empty block produces no value",
                    "a block used as a value ends in an expression",
                    None,
                );
                return Type::UNKNOWN;
            }
        };

        self.check_statements(rest);

        let ty = match last {
            Stmt::Expr(e) => self.check_expr(&e.expr),
            // The parser builds `if` as a statement wherever it appears, so a
            // block ending in one is a block ending in an expression: which it
            // is depends on the position, not on the shape.
            Stmt::If(nested) => self.check_if_expr(nested),
            other => {
                self.check_stmt(other);
                self.error(
                    codes::TYPE_MISMATCH,
                    other.span(),
                    "this block produces no value",
                    "a block used as a value ends in an expression",
                    Some("make the last statement the value the block produces".into()),
                );
                Type::UNKNOWN
            }
        };

        self.scopes.pop();
        ty
    }

    /// Checks a `match` used as a statement, reporting whether it returns.
    fn check_match_statement(&mut self, expr: &MatchExpr) -> bool {
        self.check_match(expr, false);

        // Only an exhaustive match can guarantee anything: with a case left
        // uncovered, execution can fall past it.
        let covers_everything = expr.arms.iter().any(|a| a.pattern.is_irrefutable())
            || self.matches.get(&expr.span).is_some_and(|scrutinee| {
                matches!(scrutinee.base, Base::Enum(_)) && !scrutinee.nullable
            });

        covers_everything && self.arms_all_return(expr)
    }

    /// Whether every arm of a `match` ends in a return.
    ///
    /// The arms were already checked; this walks them again only to ask about
    /// control flow, which is cheap and keeps `check_match` about types.
    fn arms_all_return(&mut self, expr: &MatchExpr) -> bool {
        expr.arms.iter().all(|arm| match &arm.body {
            ArmBody::Block(b) => b.statements.iter().any(|s| self.returns_always(s)),
            ArmBody::Expr(_) => false,
        })
    }

    /// Whether a statement guarantees a return, without re-reporting errors.
    fn returns_always(&self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Return(_) => true,
            Stmt::Block(b) => b.statements.iter().any(|s| self.returns_always(s)),
            Stmt::If(i) => {
                let then_returns = i
                    .then_branch
                    .statements
                    .iter()
                    .any(|s| self.returns_always(s));
                then_returns
                    && match &i.else_branch {
                        Some(ElseBranch::Block(b)) => {
                            b.statements.iter().any(|s| self.returns_always(s))
                        }
                        Some(ElseBranch::If(nested)) => {
                            self.returns_always(&Stmt::If((**nested).clone()))
                        }
                        None => false,
                    }
            }
            _ => false,
        }
    }

    fn check_match(&mut self, expr: &MatchExpr, as_value: bool) -> Type {
        let scrutinee = self.check_expr(&expr.scrutinee);
        self.matches.insert(expr.span, scrutinee);

        let mut arm_types: Vec<(Type, Span)> = Vec::new();
        let mut covered: Vec<String> = Vec::new();
        let mut has_wildcard = false;

        for arm in &expr.arms {
            let mut bindings = Vec::new();
            self.check_pattern(
                &arm.pattern,
                scrutinee,
                &mut covered,
                &mut has_wildcard,
                &mut bindings,
            );

            self.scopes.push();
            // A binding pattern, bare or inside a variant's `(...)`, names
            // its part of the scrutinee inside the arm.
            for binding in bindings {
                self.declare_local(binding);
            }

            let ty = match &arm.body {
                ArmBody::Expr(e) => self.check_expr(e),
                ArmBody::Block(b) if as_value => self.check_block_value(b),
                ArmBody::Block(b) => {
                    self.check_block(b);
                    Type::VOID
                }
            };
            self.scopes.pop();

            arm_types.push((ty, arm.body.span()));
        }

        self.check_exhaustive(expr, scrutinee, &covered, has_wildcard);

        if !as_value {
            return Type::VOID;
        }

        // Every arm must agree, the same way both branches of an `if` must.
        let mut result = arm_types.first().map(|(t, _)| *t).unwrap_or(Type::UNKNOWN);
        for (ty, span) in arm_types.iter().skip(1) {
            match result.unify(*ty) {
                Some(unified) => result = unified,
                None => {
                    let a = self.name(result);
                    let b = self.name(*ty);
                    self.error(
                        codes::TYPE_MISMATCH,
                        *span,
                        "the arms of the `match` produce different types",
                        format!("an earlier arm produces {a} and this one {b}"),
                        Some("every arm must agree for the `match` to be a value".into()),
                    );
                    return Type::UNKNOWN;
                }
            }
        }

        result
    }

    /// Checks one pattern against the type it is matched against, collecting
    /// the names it binds.
    ///
    /// `bindings` accumulates rather than being returned so a variant
    /// pattern's own bindings (`Loading(progress)`) land in the same list as
    /// a bare one (`other`) — `check_match` declares all of them together,
    /// once, in the arm's own scope.
    fn check_pattern(
        &mut self,
        pattern: &Pattern,
        scrutinee: Type,
        covered: &mut Vec<String>,
        has_wildcard: &mut bool,
        bindings: &mut Vec<Binding>,
    ) {
        match pattern {
            Pattern::Wildcard(_) => *has_wildcard = true,
            Pattern::Binding(ident) => {
                *has_wildcard = true;
                bindings.push(Binding {
                    name: ident.name.clone(),
                    ty: scrutinee,
                    mutability: Mutability::Immutable,
                    span: ident.span,
                    initialized: true,
                });
            }
            Pattern::Int(lit) => {
                let ty = self.check_int_literal(lit);
                self.expect_pattern_type(scrutinee, ty, lit.span);
            }
            Pattern::Str(lit) => self.expect_pattern_type(scrutinee, Type::STRING, lit.span),
            Pattern::Bool(lit) => {
                self.expect_pattern_type(scrutinee, Type::BOOLEAN, lit.span);
                // `Boolean` is a closed set of two values, so covering both is
                // as exhaustive as covering every variant of an enum.
                let name = lit.value.to_string();
                if !covered.contains(&name) {
                    covered.push(name);
                }
            }
            Pattern::Null(lit) => {
                if !scrutinee.admits_null() && !scrutinee.is_unknown() {
                    let name = self.name(scrutinee);
                    self.error(
                        codes::TYPE_MISMATCH,
                        lit.span,
                        format!("`{name}` is never null"),
                        "this arm could never be taken",
                        None,
                    );
                }
            }
            Pattern::Variant(v) => self.check_variant_pattern(v, scrutinee, covered, bindings),
        }
    }

    fn check_variant_pattern(
        &mut self,
        pattern: &VariantPattern,
        scrutinee: Type,
        covered: &mut Vec<String>,
        bindings: &mut Vec<Binding>,
    ) {
        let resolved = self.resolved_name(&pattern.enum_name.name, pattern.enum_name.span);
        let Some(index) = self.enums.iter().position(|e| e.name == resolved) else {
            self.error(
                codes::UNKNOWN_TYPE,
                pattern.enum_name.span,
                format!("`{}` is not a declared enum", pattern.enum_name.name),
                "a variant pattern names the enum it belongs to",
                None,
            );
            return;
        };

        let enum_type = &self.enums[index];
        let Some(variant) = enum_type.variant(&pattern.variant.name) else {
            let enum_name = enum_type.name.clone();
            let known: Vec<&str> = enum_type.variants.iter().map(|v| v.name.as_str()).collect();
            self.error(
                codes::UNKNOWN_VARIANT,
                pattern.variant.span,
                format!("`{enum_name}` has no variant `{}`", pattern.variant.name),
                format!("its variants are: {}", known.join(", ")),
                None,
            );
            return;
        };
        let associated = variant.associated.clone();

        self.expect_pattern_type(scrutinee, Type::of(Base::Enum(index as u32)), pattern.span);

        // A variant with data must be destructured to name it; one without
        // takes no `(...)`, the same rule construction follows.
        if associated.is_empty() && !pattern.bindings.is_empty() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                pattern.span,
                format!("`{}` carries no associated data", pattern.variant.name),
                "nothing to destructure, so it takes no `(...)`",
                None,
            );
        } else if !associated.is_empty() && pattern.bindings.is_empty() {
            let fields: Vec<&str> = associated.iter().map(|f| f.name.as_str()).collect();
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                pattern.span,
                format!("`{}` carries associated data", pattern.variant.name),
                format!("its fields are: {}", fields.join(", ")),
                Some(format!(
                    "destructure it: `{}({})`",
                    pattern.variant.name,
                    fields.join(", ")
                )),
            );
        } else if associated.len() != pattern.bindings.len() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                pattern.span,
                format!(
                    "`{}` carries {} associated value{}, not {}",
                    pattern.variant.name,
                    associated.len(),
                    if associated.len() == 1 { "" } else { "s" },
                    pattern.bindings.len()
                ),
                "a variant pattern destructures every field, in order",
                None,
            );
        } else {
            for (sub_pattern, field) in pattern.bindings.iter().zip(&associated) {
                // A literal sub-pattern's own contribution to exhaustiveness
                // and to the outer wildcard flag is not the outer match's:
                // `Loading(0.5)` does not make the arm cover every `Loading`.
                let mut discarded_covered = Vec::new();
                let mut discarded_wildcard = false;
                self.check_pattern(
                    sub_pattern,
                    field.ty,
                    &mut discarded_covered,
                    &mut discarded_wildcard,
                    bindings,
                );

                // Lowering destructures a variant's own fields (roadmap task
                // 11.4) by binding each one, not by testing it further: a
                // literal or a nested variant sub-pattern would need the
                // combined-condition compilation a plain binding does not,
                // which this pass does not build yet.
                if !matches!(sub_pattern, Pattern::Binding(_) | Pattern::Wildcard(_)) {
                    self.not_lowered(
                        sub_pattern.span(),
                        "a literal or nested pattern destructuring a variant's field",
                        "bind it to a name and test it in the arm's body instead",
                    );
                }
            }
        }

        if !covered.contains(&pattern.variant.name) {
            covered.push(pattern.variant.name.clone());
        }
    }

    fn expect_pattern_type(&mut self, scrutinee: Type, pattern: Type, span: Span) {
        // The scrutinee's nullability is irrelevant here: a `null` pattern is
        // what covers that half.
        if scrutinee.without_null().accepts(pattern) || scrutinee.is_unknown() {
            return;
        }

        let s = self.name(scrutinee);
        let p = self.name(pattern);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("this pattern matches {p}, but the value is {s}"),
            "a pattern must have the type of what it matches against",
            None,
        );
    }

    fn check_exhaustive(
        &mut self,
        expr: &MatchExpr,
        scrutinee: Type,
        covered: &[String],
        has_wildcard: bool,
    ) {
        if has_wildcard || scrutinee.is_unknown() {
            return;
        }

        // `Boolean` and enums are the closed sets this phase has: for anything
        // else, no finite list of arms can cover every value.
        if scrutinee.base == Base::Boolean && !scrutinee.nullable {
            let missing: Vec<&str> = ["true", "false"]
                .into_iter()
                .filter(|v| !covered.iter().any(|c| c == v))
                .collect();

            if missing.is_empty() {
                return;
            }

            self.error(
                codes::NON_EXHAUSTIVE_MATCH,
                expr.span,
                "the `match` does not cover every case of `Boolean`",
                format!("these values have no arm: {}", missing.join(", ")),
                Some("add the missing arm, or `_` for the rest".into()),
            );
            return;
        }

        if let Base::Enum(id) = scrutinee.base
            && !scrutinee.nullable
            && let Some(enum_type) = self.enums.get(id as usize)
        {
            let missing: Vec<String> = enum_type
                .variants
                .iter()
                .map(|v| v.name.clone())
                .filter(|name| !covered.contains(name))
                .collect();

            if missing.is_empty() {
                return;
            }

            let name = enum_type.name.clone();
            self.error(
                codes::NON_EXHAUSTIVE_MATCH,
                expr.span,
                format!("the `match` does not cover every case of `{name}`"),
                format!("these variants have no arm: {}", missing.join(", ")),
                Some("add an arm for each, or `_` for the rest".into()),
            );
            return;
        }

        let name = self.name(scrutinee);
        self.error(
            codes::NON_EXHAUSTIVE_MATCH,
            expr.span,
            format!("the `match` does not cover every value of `{name}`"),
            "only an enum has a set of values small enough to cover one by one",
            Some("end with `_ => ...` to cover the rest".into()),
        );
    }

    /// Reports a construct this phase parses and checks but cannot yet compile.
    ///
    /// The grammar and the type rules landed before the lowering did, and the
    /// gap is real: rather than let it reach a backend that would panic, the
    /// construct is reported the same way Phase 1 reported what it had not
    /// implemented (design D6). Each of these disappears as its lowering lands.
    fn not_lowered(&mut self, span: Span, what: &str, instead: &str) {
        self.error(
            codes::NOT_LOWERED,
            span,
            format!("{what} is not compilable yet"),
            "the grammar and the type rules for it exist, but its code generation does not",
            Some(instead.to_string()),
        );
    }

    /// `a.b`, which is an enum variant or a field of an object.
    ///
    /// The parser cannot tell them apart — `Direction.North` and `user.name`
    /// have the same shape — so the decision lands here, where it is known
    /// whether the base names a type or a value.
    fn check_field(&mut self, expr: &FieldExpr) -> Type {
        // A base that names a declared enum is a variant access. `?.` makes
        // no sense on it — a type name is never absent — so `safe` is not
        // consulted here.
        if let Expr::Path(base) = &*expr.object {
            let resolved = self.resolved_name(&base.name, base.span);
            if self.enums.iter().any(|e| e.name == resolved) {
                let variant = VariantExpr {
                    enum_name: base.clone(),
                    variant: expr.name.clone(),
                    span: expr.span,
                };
                let ty = self.check_variant(&variant);
                self.variant_accesses.insert(expr.span);
                return ty;
            }
        }

        let object = self.check_expr(&expr.object);
        self.member_type(object, &expr.name, expr.object.span(), expr.safe)
    }

    /// The type of a member read from a value, reporting why it cannot be.
    ///
    /// `safe` is `?.`: reading through an absent receiver answers `null`
    /// instead of rejecting the access, so the member's type comes back
    /// nullable even when the member itself is not (D7). It only changes
    /// what happens when `object` is nullable — the rest is the same lookup
    /// plain `.` would do, over the narrowed non-null type.
    fn member_type(&mut self, object: Type, member: &Ident, object_span: Span, safe: bool) -> Type {
        if object.is_unknown() {
            return Type::UNKNOWN;
        }

        if object.nullable {
            if !safe {
                self.reject_absent_receiver(object, object_span);
                return Type::UNKNOWN;
            }
            let ty = self.member_type(object.without_null(), member, object_span, false);
            return if ty.is_unknown() {
                ty
            } else {
                ty.as_nullable()
            };
        }

        if let Base::Param(id) = object.base {
            return self.param_field_type(id, member);
        }

        if let Base::Union(_) = object.base {
            let name = self.name(object);
            self.error(
                codes::UNKNOWN_MEMBER,
                member.span,
                format!("`{name}` has no member `{}`", member.name),
                "a union exposes only what every alternative has in common, and only after narrowing it",
                Some("narrow it first, e.g. with `match`".into()),
            );
            return Type::UNKNOWN;
        }

        if let Base::Instance(inst_id) = object.base {
            let instance = self.generic_instances[inst_id as usize].clone();
            let subst: Vec<(u32, Type)> = self.classes[instance.class as usize]
                .type_params
                .clone()
                .into_iter()
                .zip(instance.args)
                .collect();
            let ty = self.member_type(
                Type::of(Base::Class(instance.class)),
                member,
                object_span,
                false,
            );
            return if ty.is_unknown() {
                ty
            } else {
                self.substitute_type(ty, &subst)
            };
        }

        let Base::Class(id) = object.base else {
            let name = self.name(object);
            self.error(
                codes::UNKNOWN_MEMBER,
                member.span,
                format!("`{name}` has no member `{}`", member.name),
                "only a class has members in this phase",
                None,
            );
            return Type::UNKNOWN;
        };

        let class = &self.classes[id as usize];
        let Some(field) = class.field(&member.name) else {
            let class_name = class.name.clone();
            let known: Vec<&str> = class.fields.iter().map(|f| f.name.as_str()).collect();
            let help = if known.is_empty() {
                format!("`{class_name}` declares no fields")
            } else {
                format!("its fields are: {}", known.join(", "))
            };
            self.error(
                codes::UNKNOWN_MEMBER,
                member.span,
                format!("`{class_name}` has no member `{}`", member.name),
                help,
                None,
            );
            return Type::UNKNOWN;
        };

        let ty = field.ty;
        let visibility = field.visibility;
        let declared = field.span;
        let owner = field.owner;

        // A hidden member is a different mistake from one that does not
        // exist, and saying so is the difference between "you cannot reach
        // this" and "you misspelled it".
        if !self.can_access(visibility, owner) {
            self.report_inaccessible(&member.name, member.span, visibility, owner, declared);
        }

        ty
    }

    /// A field read through a generic type parameter, resolved against
    /// whichever of its `from` constraints is a class (task 7.4).
    ///
    /// A contract has no fields, so only a class constraint can answer this;
    /// method calls go through [`Self::check_param_call`] instead, which also
    /// looks at contract constraints.
    fn param_field_type(&mut self, id: u32, member: &Ident) -> Type {
        let constraints = self.type_params[id as usize].constraints.clone();
        for constraint in &constraints {
            if let Base::Class(cid) = constraint.base
                && let Some(field) = self.classes[cid as usize].field(&member.name).cloned()
            {
                if !self.can_access(field.visibility, field.owner) {
                    self.report_inaccessible(
                        &member.name,
                        member.span,
                        field.visibility,
                        field.owner,
                        field.span,
                    );
                }
                return field.ty;
            }
        }

        let param_name = self.type_params[id as usize].name.clone();
        self.error(
            codes::UNKNOWN_MEMBER,
            member.span,
            format!("`{param_name}` has no member `{}`", member.name),
            self.constraint_help(&constraints),
            Some("only what a `from` constraint promises is available".into()),
        );
        Type::UNKNOWN
    }

    /// A method called through a generic type parameter, resolved against its
    /// `from` constraints — a class or a contract, either may supply it
    /// (task 7.4).
    fn check_param_call(&mut self, expr: &CallExpr, field: &FieldExpr, id: u32) -> Type {
        let constraints = self.type_params[id as usize].constraints.clone();
        for constraint in &constraints {
            let found = match constraint.base {
                Base::Contract(cid) => {
                    self.contracts[cid as usize]
                        .method(&field.name.name)
                        .map(|m| Signature {
                            name: m.name.clone(),
                            params: m.params.clone(),
                            returns: m.returns,
                            shared: true,
                            span: m.span,
                            type_params: Vec::new(),
                        })
                }
                Base::Class(cid) => self.classes[cid as usize]
                    .method(&field.name.name)
                    .map(|m| Signature {
                        name: m.name.clone(),
                        params: m.params.clone(),
                        returns: m.returns,
                        shared: true,
                        span: m.span,
                        type_params: Vec::new(),
                    }),
                _ => None,
            };
            if let Some(signature) = found {
                return self.check_direct_call(expr, &signature);
            }
        }

        let param_name = self.type_params[id as usize].name.clone();
        self.error(
            codes::UNKNOWN_MEMBER,
            field.name.span,
            format!("`{param_name}` has no method `{}`", field.name.name),
            self.constraint_help(&constraints),
            Some("only what a `from` constraint promises is available".into()),
        );
        for arg in &expr.args {
            self.check_expr(&arg.value);
        }
        Type::UNKNOWN
    }

    /// The `= help:` line naming what a type parameter's constraints are, for
    /// a member that none of them turned out to promise.
    fn constraint_help(&self, constraints: &[Type]) -> String {
        if constraints.is_empty() {
            return "it has no `from` constraint, so it promises nothing".to_string();
        }
        let names: Vec<String> = constraints.iter().map(|c| self.name(*c)).collect();
        format!("its constraints are: {}", names.join(" & "))
    }

    /// Reports reaching a member through a value that may be absent.
    fn reject_absent_receiver(&mut self, object: Type, at: Span) {
        let name = self.name(object);
        self.error(
            codes::TYPE_MISMATCH,
            at,
            format!("`{name}` may be absent"),
            "reaching a member through a value that may be null is not allowed",
            Some("use `?.`, or provide a value with `??` first".into()),
        );
    }

    /// Whether the code being checked may see a member with this visibility.
    ///
    /// Inside its own class every member is reachable: visibility limits who
    /// looks in from outside, not what the class does with itself. `protected`
    /// extends that reach to the subclasses, which is the whole of what it
    /// adds over `private`.
    fn can_access(&self, visibility: Visibility, owner: u32) -> bool {
        if visibility == Visibility::Public {
            return true;
        }

        let Some(Type {
            base: Base::Class(current),
            ..
        }) = self.this_type
        else {
            return false;
        };

        if current == owner {
            return true;
        }

        visibility == Visibility::Protected && self.inherits_from(current, owner)
    }

    /// Whether a class has another somewhere up its chain.
    fn inherits_from(&self, mut current: u32, ancestor: u32) -> bool {
        while let Some(base) = self.classes[current as usize].base {
            if base == ancestor {
                return true;
            }
            current = base;
        }
        false
    }

    fn report_inaccessible(
        &mut self,
        member: &str,
        at: Span,
        visibility: Visibility,
        owner: u32,
        declared: Span,
    ) {
        let owner_name = self.classes[owner as usize].name.clone();
        let line = self.sources.location(declared).line;
        let help = match visibility {
            Visibility::Private => "a `private` member is reachable only inside its own class",
            Visibility::Protected => {
                "a `protected` member is reachable inside its class and its subclasses"
            }
            Visibility::Public => unreachable!("a public member is always reachable"),
        };
        self.error(
            codes::INACCESSIBLE_MEMBER,
            at,
            format!("`{member}` is not accessible here"),
            format!(
                "it is declared `{}` in `{owner_name}`, on line {line}",
                visibility.as_str()
            ),
            Some(help.into()),
        );
    }

    /// Reports a construct that parses but whose checking has not landed yet.
    ///
    /// Distinct from [`Self::not_lowered`]: there the rules exist and only the
    /// code generation is missing, while here the rules themselves are not
    /// written, so accepting it silently would let unverified code through.
    fn not_checked(&mut self, span: Span, what: &str, why: &str) {
        self.error(
            codes::PENDING_FEATURE,
            span,
            format!("{what} is not checked yet"),
            why,
            Some("classes parse in this phase; their checking lands next".into()),
        );
    }

    /// `Direction.North` in expression position.
    fn check_variant(&mut self, expr: &VariantExpr) -> Type {
        let resolved = self.resolved_name(&expr.enum_name.name, expr.enum_name.span);
        let Some(index) = self.enums.iter().position(|e| e.name == resolved) else {
            self.error(
                codes::UNKNOWN_TYPE,
                expr.enum_name.span,
                format!("`{}` is not a declared enum", expr.enum_name.name),
                "only an enum has variants to name in this phase",
                None,
            );
            return Type::UNKNOWN;
        };

        let declared = self.enums[index].span;
        let shared = self.enums[index].shared;
        self.require_visible(declared, shared, &expr.enum_name, "enum");

        let enum_type = &self.enums[index];
        let Some(variant) = enum_type.variant(&expr.variant.name) else {
            let enum_name = enum_type.name.clone();
            let known: Vec<&str> = enum_type.variants.iter().map(|v| v.name.as_str()).collect();
            self.error(
                codes::UNKNOWN_VARIANT,
                expr.variant.span,
                format!("`{enum_name}` has no variant `{}`", expr.variant.name),
                format!("its variants are: {}", known.join(", ")),
                None,
            );
            return Type::UNKNOWN;
        };

        if !variant.associated.is_empty() {
            let fields: Vec<&str> = variant.associated.iter().map(|f| f.name.as_str()).collect();
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                format!("`{}` carries associated data", expr.variant.name),
                format!("its fields are: {}", fields.join(", ")),
                Some(format!(
                    "construct it: `{}.{}({})`",
                    expr.enum_name.name,
                    expr.variant.name,
                    fields.join(", ")
                )),
            );
        }

        Type::of(Base::Enum(index as u32))
    }

    /// `LoadState.Loading(0.5)`: an algebraic variant constructed with its
    /// associated data (roadmap task 8.2).
    ///
    /// Dispatched the same way a class constructor is: a synthetic
    /// [`Signature`] built from the variant's fields, checked through
    /// [`Self::check_direct_call`] so arity, names and types follow the same
    /// rules a call already does.
    fn check_variant_construction(
        &mut self,
        expr: &CallExpr,
        field: &FieldExpr,
        enum_id: u32,
    ) -> Type {
        let variant_name = field.name.name.clone();
        let Some(variant) = self.enums[enum_id as usize].variant(&variant_name).cloned() else {
            let enum_name = self.enums[enum_id as usize].name.clone();
            let known: Vec<String> = self.enums[enum_id as usize]
                .variants
                .iter()
                .map(|v| v.name.clone())
                .collect();
            self.error(
                codes::UNKNOWN_VARIANT,
                field.name.span,
                format!("`{enum_name}` has no variant `{variant_name}`"),
                format!("its variants are: {}", known.join(", ")),
                None,
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::UNKNOWN;
        };

        if variant.associated.is_empty() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                format!("`{variant_name}` carries no associated data"),
                "nothing to construct with, so it takes no arguments",
                None,
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::of(Base::Enum(enum_id));
        }

        let params: Vec<ParamInfo> = variant
            .associated
            .iter()
            .map(|f| ParamInfo {
                name: f.name.clone(),
                ty: f.ty,
                optional: false,
                has_default: false,
                variadic: false,
            })
            .collect();
        let type_params = self.enums[enum_id as usize].type_params.clone();
        let signature = Signature {
            name: variant_name,
            params,
            returns: Type::of(Base::Enum(enum_id)),
            shared: true,
            span: field.span,
            type_params: type_params.clone(),
        };
        let (_, substitution) = self.check_direct_call_with_subst(expr, &signature);

        // A generic enum's own `T` is inferred from the associated data
        // supplied here, the same way a generic class's construction infers
        // it from its constructor's arguments (roadmap task 13.5, mirrors
        // 11.1) — `Iteration<T>` is the only enum a program can reach this
        // with today (any other generic enum's own declaration is gated at
        // `Self::enter_type_params`), but nothing here assumes that.
        if type_params.is_empty() {
            return Type::of(Base::Enum(enum_id));
        }
        let args: Vec<Type> = type_params
            .iter()
            .map(|id| substitution.get(id).copied().unwrap_or(Type::UNKNOWN))
            .collect();
        let instance = self.intern_enum_instance(GenericEnumInstance { enum_id, args });
        self.variant_constructions.insert(expr.span, instance);
        Type::of(Base::EnumInstance(instance))
    }

    fn check_lambda(&mut self, expr: &LambdaExpr) -> Type {
        let params: Vec<ParamInfo> = expr.params.iter().map(|p| self.resolve_param(p)).collect();
        let returns = self.resolve_type(&expr.return_type);

        let enclosing_return = self.current_return;
        // A lambda body is not inside the enclosing loop: `break` in it has
        // nothing to leave.
        let enclosing_depth = self.loop_depth;
        self.current_return = returns;
        self.loop_depth = 0;
        self.capture_stack.push(Vec::new());

        self.scopes.push_function();
        for (param, info) in expr.params.iter().zip(&params) {
            if let Some(default) = &param.default {
                let actual = self.check_expr(default);
                self.expect_assignable(info.ty, actual, default.span(), "the default value");
            }
            self.declare_local(Binding {
                name: info.name.clone(),
                ty: info.ty,
                mutability: Mutability::Immutable,
                span: param.name.span,
                initialized: true,
            });
        }

        match &*expr.body {
            LambdaBody::Expr(e) => {
                let actual = self.check_expr(e);
                self.expect_assignable(returns, actual, e.span(), "the lambda body");
            }
            LambdaBody::Block(b) => {
                let always_returns = self.check_block(b);
                if returns != Type::VOID && !returns.is_unknown() && !always_returns {
                    let name = self.name(returns);
                    self.error(
                        codes::MISSING_RETURN,
                        b.span,
                        "not every path of the lambda returns a value",
                        format!("it declares `{name}` as its return type"),
                        Some("add a `return` at the end".into()),
                    );
                }
            }
        }
        self.scopes.pop();

        let captures = self.capture_stack.pop().unwrap_or_default();
        self.current_return = enclosing_return;
        self.loop_depth = enclosing_depth;

        // A capture of an inner lambda is also a capture of the outer one when
        // the name lives further out still.
        for capture in &captures {
            if let Some(resolved) = self.scopes.resolve(&capture.name)
                && resolved.captured
            {
                self.record_capture(&resolved.binding);
            }
        }

        // Each lambda gets a type of its own rather than sharing one per
        // signature: its captures are part of its representation (D10), so two
        // lambdas of the same shape are not interchangeable. Interning them
        // together would let one be assigned over the other and leave the IR
        // holding a value whose layout no longer matches its slot.
        self.fn_types.push(FnType {
            params: params.iter().map(|p| p.ty).collect(),
            returns,
        });
        let fn_type = (self.fn_types.len() - 1) as u32;

        self.lambdas
            .insert(expr.span, LambdaInfo { captures, fn_type });

        Type::of(Base::Function(fn_type))
    }

    /// `value.method(...)` where `value` is reached through a contract.
    ///
    /// Only what the contract declares is available: which class is behind it
    /// is exactly what a contract exists not to say.
    fn check_contract_call(&mut self, expr: &CallExpr, field: &FieldExpr, id: u32) -> Type {
        let contract = &self.contracts[id as usize];
        let contract_name = contract.name.clone();
        let Some(method) = contract.method(&field.name.name) else {
            let known: Vec<&str> = contract.methods.iter().map(|m| m.name.as_str()).collect();
            let help = if known.is_empty() {
                format!("`{contract_name}` declares no methods")
            } else {
                format!("its methods are: {}", known.join(", "))
            };
            self.error(
                codes::UNKNOWN_MEMBER,
                field.name.span,
                format!("`{contract_name}` has no method `{}`", field.name.name),
                help,
                None,
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::UNKNOWN;
        };

        let signature = Signature {
            name: method.name.clone(),
            params: method.params.clone(),
            returns: method.returns,
            shared: true,
            span: method.span,
            type_params: Vec::new(),
        };
        self.check_direct_call(expr, &signature)
    }

    /// `super(...)`, which runs the base's constructor on this instance.
    fn check_super_construction(&mut self, expr: &CallExpr) -> Type {
        let Some(base) = self.enclosing_base(expr.span, "super(...)") else {
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::VOID;
        };

        if !self.in_constructor {
            self.error(
                codes::TYPE_MISMATCH,
                expr.span,
                "`super(...)` is only available inside a constructor",
                "it runs the base's constructor, which happens while the instance is built",
                Some("use `super.method()` to reach inherited behavior".into()),
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::VOID;
        }

        let class = &self.classes[base as usize];
        let name = class.name.clone();
        let span = class.span;
        let constructors = class.constructors.clone();

        let arity = expr.args.len();
        let Some(params) = constructors
            .iter()
            .find(|params| admits_arity(params, arity))
            .cloned()
        else {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                format!("no constructor of `{name}` takes {arity} arguments"),
                "`super(...)` selects a constructor of the base class",
                None,
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::VOID;
        };

        let signature = Signature {
            name,
            params,
            returns: Type::VOID,
            shared: true,
            span,
            type_params: Vec::new(),
        };
        self.check_direct_call(expr, &signature);
        Type::VOID
    }

    /// `super.method(...)`, which reaches the base's body rather than the
    /// object's own.
    fn check_super_method(&mut self, expr: &CallExpr, field: &FieldExpr) -> Type {
        let Some(base) = self.enclosing_base(field.span, "super.method()") else {
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::UNKNOWN;
        };
        self.check_method_call(expr, field, base, &[])
    }

    /// The base of the class whose body is being checked, reporting when there
    /// is none to reach.
    fn enclosing_base(&mut self, at: Span, what: &str) -> Option<u32> {
        let Some(Type {
            base: Base::Class(id),
            ..
        }) = self.this_type
        else {
            self.error(
                codes::UNDECLARED_NAME,
                at,
                format!("`{what}` is only available inside a class"),
                "it reaches the base of the class being defined",
                None,
            );
            return None;
        };

        let base = self.classes[id as usize].base;
        if base.is_none() {
            let name = self.classes[id as usize].name.clone();
            self.error(
                codes::UNDECLARED_NAME,
                at,
                format!("`{name}` has no base class"),
                format!("`{what}` needs an `extends` to reach"),
                None,
            );
        }
        base
    }

    /// Dispatches a method call to whichever of `Base::Class`,
    /// `Base::Contract` or `Base::Param` the (already narrowed to non-null)
    /// receiver is.
    ///
    /// Factored out of `check_call` so `?.` can run it on the narrowed
    /// receiver and wrap the result nullable, instead of duplicating the
    /// three-way dispatch for both the plain and the safe case.
    fn check_method_call_on(&mut self, object: Type, expr: &CallExpr, field: &FieldExpr) -> Type {
        if let Base::Class(id) = object.base {
            return self.check_method_call(expr, field, id, &[]);
        }
        if let Base::Contract(id) = object.base {
            return self.check_contract_call(expr, field, id);
        }
        if let Base::Param(id) = object.base {
            return self.check_param_call(expr, field, id);
        }
        if let Base::Instance(inst_id) = object.base {
            let instance = self.generic_instances[inst_id as usize].clone();
            let subst: Vec<(u32, Type)> = self.classes[instance.class as usize]
                .type_params
                .clone()
                .into_iter()
                .zip(instance.args)
                .collect();
            return self.check_method_call(expr, field, instance.class, &subst);
        }
        unreachable!("caller already matched object.base against these three")
    }

    /// `object.method(...)`.
    ///
    /// The receiver is checked once, by the caller, so a method call does not
    /// evaluate it twice. `subst` replaces a generic class's own type
    /// parameters with a specific instantiation's arguments (task 6.9's
    /// pattern extended to classes) — empty for an ordinary, non-generic
    /// receiver.
    fn check_method_call(
        &mut self,
        expr: &CallExpr,
        field: &FieldExpr,
        id: u32,
        subst: &[(u32, Type)],
    ) -> Type {
        let class = &self.classes[id as usize];
        let class_name = class.name.clone();
        let Some(method) = class.method(&field.name.name).cloned() else {
            // Naming a field where a method is called is its own mistake, and
            // saying "not callable" would send the reader looking for a typo.
            if class.field(&field.name.name).is_some() {
                self.error(
                    codes::NOT_CALLABLE,
                    field.name.span,
                    format!("`{}` is a field, not a method", field.name.name),
                    format!("`{class_name}` declares it as data"),
                    Some("read it without parentheses".into()),
                );
            } else {
                let known: Vec<&str> = class.methods.iter().map(|m| m.name.as_str()).collect();
                let help = if known.is_empty() {
                    format!("`{class_name}` declares no methods")
                } else {
                    format!("its methods are: {}", known.join(", "))
                };
                self.error(
                    codes::UNKNOWN_MEMBER,
                    field.name.span,
                    format!("`{class_name}` has no method `{}`", field.name.name),
                    help,
                    None,
                );
            }
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::UNKNOWN;
        };

        let params = if subst.is_empty() {
            method.params.clone()
        } else {
            method
                .params
                .iter()
                .map(|p| ParamInfo {
                    ty: self.substitute_type(p.ty, subst),
                    ..p.clone()
                })
                .collect()
        };
        let returns = if subst.is_empty() {
            method.returns
        } else {
            self.substitute_type(method.returns, subst)
        };
        let signature = Signature {
            name: method.name.clone(),
            params,
            returns,
            shared: true,
            span: method.span,
            type_params: Vec::new(),
        };
        let visibility = method.visibility;
        let declared = method.span;

        // Same rule as for a field: inside its own class everything is
        // reachable.
        let from_inside = self.this_type == Some(Type::of(Base::Class(id)));
        if visibility != Visibility::Public && !from_inside {
            let line = self.sources.location(declared).line;
            self.error(
                codes::INACCESSIBLE_MEMBER,
                field.name.span,
                format!("`{}` is not accessible here", field.name.name),
                format!(
                    "it is declared `{}` in `{class_name}`, on line {line}",
                    visibility.as_str()
                ),
                Some("only `public` members are reachable from outside the class".into()),
            );
        }

        self.check_direct_call(expr, &signature)
    }

    /// `User(...)`, which builds an instance.
    ///
    /// A class may declare several constructors, so this picks the one whose
    /// arity admits the call and checks against it. Resolving overlapping
    /// signatures by type and by argument label is the rest of the rule, and
    /// it needs the whole matching machinery that named arguments already use.
    fn check_construction(&mut self, expr: &CallExpr, id: u32, callee: &Ident) -> Type {
        let class = &self.classes[id as usize];
        let class_name = class.name.clone();
        let shared = class.shared;
        let span = class.span;
        let kind = class.kind;
        let constructors = class.constructors.clone();
        let type_params = class.type_params.clone();

        self.require_visible(span, shared, callee, kind.as_str());

        if kind != ClassKind::Class {
            return self.check_record_construction(expr, id);
        }

        if constructors.is_empty() {
            self.error(
                codes::UNKNOWN_MEMBER,
                expr.span,
                format!("`{class_name}` has no constructor"),
                "a class is built through a `construct` declaration",
                Some(format!("add `construct(...) {{ ... }}` to `{class_name}`")),
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::of(Base::Class(id));
        }

        let arity = expr.args.len();
        let chosen = constructors
            .iter()
            .find(|params| admits_arity(params, arity))
            .cloned();

        let Some(params) = chosen else {
            let arities: Vec<String> = constructors
                .iter()
                .map(|p| p.len().to_string())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                format!("no constructor of `{class_name}` takes {arity} arguments"),
                format!("its constructors take: {}", arities.join(", ")),
                None,
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::of(Base::Class(id));
        };

        let signature = Signature {
            name: class_name,
            params,
            returns: Type::of(Base::Class(id)),
            shared,
            span,
            type_params: type_params.clone(),
        };
        let (_, substitution) = self.check_direct_call_with_subst(expr, &signature);

        // A generic class's constructor call is typed as its own instantiation
        // (`Box<Int32>`, not the bare `Box` an ordinary call's return would
        // be) — `check_direct_call_with_subst` only substitutes a bare `T`
        // return, which a constructor's `Base::Class(id)` never is.
        if type_params.is_empty() {
            return Type::of(Base::Class(id));
        }
        let args: Vec<Type> = type_params
            .iter()
            .map(|pid| substitution.get(pid).copied().unwrap_or(Type::UNKNOWN))
            .collect();
        let instance = self.intern_instance(GenericInstance { class: id, args });
        self.generic_constructions.insert(expr.span, instance);
        Type::of(Base::Instance(instance))
    }

    /// `Point(x: 1, y: 2)`: the implicit constructor of a record or value
    /// class, over every field, named-only, an omitted field taking its
    /// type's default (`ZIRK_LANGUAGE_SPEC.md` section 7).
    ///
    /// Dispatched from [`Self::check_construction`] instead of matching
    /// arity against `class.constructors`: a record or value class never has
    /// one, its whole field list is the signature.
    fn check_record_construction(&mut self, expr: &CallExpr, id: u32) -> Type {
        let class = self.classes[id as usize].clone();
        let class_name = class.name.clone();
        let kind = class.kind;

        for arg in &expr.args {
            if arg.name.is_none() {
                self.error(
                    codes::UNKNOWN_ARGUMENT_NAME,
                    arg.span,
                    format!("`{class_name}` takes only named arguments"),
                    format!("a {} is constructed by naming its fields", kind.as_str()),
                    Some(format!("write `{class_name}(field: value, ...)`")),
                );
            }
        }

        let params: Vec<ParamInfo> = class
            .fields
            .iter()
            .map(|f| ParamInfo {
                name: f.name.clone(),
                ty: f.ty,
                optional: f.ty.has_default(),
                has_default: false,
                variadic: false,
            })
            .collect();
        let signature = Signature {
            name: class_name,
            params,
            returns: Type::of(Base::Class(id)),
            shared: class.shared,
            span: class.span,
            type_params: Vec::new(),
        };
        self.check_direct_call(expr, &signature)
    }

    fn check_call(&mut self, expr: &CallExpr) -> Type {
        if matches!(&*expr.callee, Expr::Super(_)) {
            return self.check_super_construction(expr);
        }

        if let Expr::Field(field) = &*expr.callee
            && matches!(&*field.object, Expr::Super(_))
        {
            return self.check_super_method(expr, field);
        }

        // `LoadState.Loading(0.5)` constructs an algebraic variant. Decided
        // here, ahead of the method-call branch below, for the same reason
        // `check_field` decides `LoadState.Loading` alone is a variant and
        // not a member access: the base names a type, not a value with a
        // field called `Loading` to evaluate.
        if let Expr::Field(field) = &*expr.callee
            && let Expr::Path(base) = &*field.object
        {
            let resolved = self.resolved_name(&base.name, base.span);
            if let Some(enum_id) = self.enums.iter().position(|e| e.name == resolved) {
                self.variant_accesses.insert(field.span);
                return self.check_variant_construction(expr, field, enum_id as u32);
            }
        }

        // `u.greeting()` calls a method. It is decided here and not by the
        // parser for the same reason `u.name` is: the shape does not say
        // whether the base is a value with members.
        if let Expr::Field(field) = &*expr.callee
            && !self.variant_accesses.contains(&field.span)
        {
            let object = self.check_expr(&field.object);

            // Calling through a value that may be absent is the same mistake
            // as reading through one, unless it is spelled `?.`: then it is
            // the same answer reading through one gets too — the method's
            // return type, made nullable (D7).
            if object.nullable
                && matches!(
                    object.base,
                    Base::Class(_) | Base::Contract(_) | Base::Param(_) | Base::Instance(_)
                )
            {
                if !field.safe {
                    self.reject_absent_receiver(object, field.object.span());
                    for arg in &expr.args {
                        self.check_expr(&arg.value);
                    }
                    return Type::UNKNOWN;
                }
                // A method call through `?.` lowers for a class, a specific
                // generic instantiation, or a contract receiver — the two
                // blocks `lower_safe_field` already builds for a field, with
                // the call itself inside the present one (roadmap task
                // 10.8). Through a generic parameter's constraint there is
                // no concrete method body to call yet, so that combination
                // stays gated.
                if !matches!(
                    object.base,
                    Base::Class(_) | Base::Contract(_) | Base::Instance(_)
                ) {
                    self.not_lowered(
                        field.object.span(),
                        "`?.` calling a method through a generic parameter",
                        "narrow the receiver first, e.g. with `?? <fallback>` or a null check, for now",
                    );
                }
                let ty = self.check_method_call_on(object.without_null(), expr, field);
                return if ty.is_unknown() {
                    ty
                } else {
                    ty.as_nullable()
                };
            }

            if matches!(
                object.base,
                Base::Class(_) | Base::Contract(_) | Base::Param(_) | Base::Instance(_)
            ) {
                return self.check_method_call_on(object, expr, field);
            }
            if object.is_unknown() {
                for arg in &expr.args {
                    self.check_expr(&arg.value);
                }
                return Type::UNKNOWN;
            }
        }

        // A call to a name resolves against the declared functions first, so a
        // named function keeps its optional and variadic parameters. Calling a
        // value goes through its function type, which has none of that.
        if let Expr::Path(callee) = &*expr.callee
            && self.scopes.lookup(&callee.name).is_none()
        {
            let declared = self.resolved_name(&callee.name, callee.span);
            if let Some(signature) = self.functions.get(&declared).cloned() {
                self.require_visible(signature.span, signature.shared, callee, "function");
                return self.check_direct_call(expr, &signature);
            }

            // `User(1, "x")` builds an instance. There is no `new`: the type
            // name is the constructor (`LANGUAGE_SPEC` section 7).
            if let Some(id) = self.classes.iter().position(|c| c.name == declared) {
                return self.check_construction(expr, id as u32, callee);
            }

            if let Some(id) = self.contract_id(&declared) {
                let kind = self.contracts[id as usize].kind;
                self.error(
                    codes::NOT_CALLABLE,
                    callee.span,
                    format!("`{}` cannot be constructed", callee.name),
                    format!("a {} describes behaviour, not an instance", kind.as_str()),
                    Some("build a class that implements it".into()),
                );
                for arg in &expr.args {
                    self.check_expr(&arg.value);
                }
                return Type::of(Base::Contract(id));
            }
        }

        let callee = self.check_expr(&expr.callee);
        let Base::Function(id) = callee.base else {
            if !callee.is_unknown() {
                let name = self.name(callee);
                self.error(
                    codes::NOT_CALLABLE,
                    expr.callee.span(),
                    format!("`{name}` is not a function"),
                    "only a function or a closure can be called",
                    None,
                );
            }
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::UNKNOWN;
        };

        let fn_type = self.fn_types[id as usize].clone();
        let arguments: Vec<Type> = expr
            .args
            .iter()
            .map(|a| self.check_expr(&a.value))
            .collect();

        // A closure value has no parameter names to match against.
        for arg in &expr.args {
            if let Some(name) = &arg.name {
                self.error(
                    codes::UNKNOWN_ARGUMENT_NAME,
                    name.span,
                    "a closure cannot take named arguments",
                    "only a declared function carries the parameter names",
                    Some("pass the arguments by position".into()),
                );
            }
        }

        if arguments.len() != fn_type.params.len() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                format!(
                    "this closure expects {} argument(s) and received {}",
                    fn_type.params.len(),
                    arguments.len()
                ),
                "the number of arguments must match the closure's type",
                None,
            );
            return fn_type.returns;
        }

        for (index, (actual, expected)) in arguments.iter().zip(&fn_type.params).enumerate() {
            self.expect_assignable(
                *expected,
                *actual,
                expr.args[index].span,
                &format!("argument {}", index + 1),
            );
        }

        fn_type.returns
    }

    /// A call to a declared function, where names and defaults apply.
    fn check_direct_call(&mut self, expr: &CallExpr, signature: &Signature) -> Type {
        self.check_direct_call_with_subst(expr, signature).0
    }

    /// [`Self::check_direct_call`], also returning the type-parameter
    /// substitution it inferred — what [`Self::check_construction`] needs to
    /// build the constructed instance's own type (`Box<Int32>`, not the bare
    /// `Box` an ordinary call's return type would be).
    fn check_direct_call_with_subst(
        &mut self,
        expr: &CallExpr,
        signature: &Signature,
    ) -> (Type, HashMap<u32, Type>) {
        let name = signature.name.clone();
        let slots = self.match_arguments(expr, signature);
        let substitution = self.infer_type_params(expr.span, &name, signature, &slots);

        for (index, slot) in slots.iter().enumerate() {
            let Some(param) = signature.params.get(index) else {
                continue;
            };
            let param_ty = self.substitute(param.ty, &substitution);

            match slot {
                ArgSlot::Given { ty, span } => {
                    self.expect_assignable(
                        param_ty,
                        *ty,
                        *span,
                        &format!("argument `{}`", param.name),
                    );
                }
                ArgSlot::Variadic(items) => {
                    for (ty, span) in items {
                        self.expect_assignable(
                            param_ty,
                            *ty,
                            *span,
                            &format!("a value of `...{}`", param.name),
                        );
                    }
                }
                // Absent and defaultable: nothing to check, the default was
                // checked where it was declared.
                ArgSlot::Default | ArgSlot::Absent => {}
                ArgSlot::Missing => {
                    self.error(
                        codes::WRONG_ARGUMENT_COUNT,
                        expr.span,
                        format!("`{name}` is missing an argument for `{}`", param.name),
                        "the parameter is neither optional nor has a default value",
                        None,
                    );
                }
            }
        }

        (
            self.substitute(signature.returns, &substitution),
            substitution,
        )
    }

    /// Infers each of a callable's own type parameters from the concrete
    /// arguments given to it (roadmap task 7.6).
    ///
    /// Only from arguments: the receiver, the expected result and the
    /// surrounding callable context that `zirk-generics`' spec also lists are
    /// not consulted yet. A parameter that no argument determines is left
    /// unsolved and reported rather than guessed — "SHALL fail rather than
    /// choose an arbitrary solution".
    fn infer_type_params(
        &mut self,
        call_span: Span,
        name: &str,
        signature: &Signature,
        slots: &[ArgSlot],
    ) -> HashMap<u32, Type> {
        if signature.type_params.is_empty() {
            return HashMap::new();
        }

        let mut substitution: HashMap<u32, Type> = HashMap::new();
        for (index, slot) in slots.iter().enumerate() {
            let Some(param) = signature.params.get(index) else {
                continue;
            };
            let Base::Param(id) = param.ty.base else {
                continue;
            };
            let actual = match slot {
                ArgSlot::Given { ty, .. } => Some(*ty),
                ArgSlot::Variadic(items) => items.first().map(|&(ty, _)| ty),
                ArgSlot::Default | ArgSlot::Absent | ArgSlot::Missing => None,
            };
            let Some(actual) = actual else { continue };
            if actual.is_unknown() {
                continue;
            }

            // A closure cannot be annotated as a parameter, return or field
            // type (D9), and inferring a generic parameter from one would be
            // exactly that under another name.
            if matches!(actual.base, Base::Function(_)) {
                let param_name = self.type_params[id as usize].name.clone();
                self.error(
                    codes::TYPE_MISMATCH,
                    call_span,
                    format!("`{name}` cannot infer `{param_name}` from a closure"),
                    "a closure may not be annotated as a parameter, return or field type, and a generic argument is no exception (decision D9)",
                    None,
                );
                continue;
            }

            match substitution.get(&id) {
                Some(&solved) if solved != actual => {
                    let a = self.name(solved);
                    let b = self.name(actual);
                    let param_name = self.type_params[id as usize].name.clone();
                    self.error(
                        codes::TYPE_MISMATCH,
                        call_span,
                        format!("`{name}` cannot infer `{param_name}`"),
                        format!(
                            "it would have to be both `{a}` and `{b}` at once, one per argument"
                        ),
                        None,
                    );
                }
                _ => {
                    substitution.insert(id, actual);
                }
            }
        }

        for &id in &signature.type_params {
            let Some(&solved) = substitution.get(&id) else {
                let param_name = self.type_params[id as usize].name.clone();
                self.error(
                    codes::TYPE_MISMATCH,
                    call_span,
                    format!("`{name}` cannot infer `{param_name}`"),
                    "no argument determines it, and neither the expected result nor an explicit type argument are consulted yet",
                    None,
                );
                continue;
            };

            let constraints = self.type_params[id as usize].constraints.clone();
            for constraint in &constraints {
                if self.satisfies_constraint(solved, *constraint) {
                    continue;
                }
                let param_name = self.type_params[id as usize].name.clone();
                let solved_name = self.name(solved);
                let constraint_name = self.name(*constraint);
                self.error(
                    codes::TYPE_MISMATCH,
                    call_span,
                    format!("`{solved_name}` does not satisfy `{param_name}`"),
                    format!(
                        "`{param_name}` requires `{constraint_name}`, which `{solved_name}` does not provide"
                    ),
                    None,
                );
            }
        }

        substitution
    }

    /// Replaces a callable's own type parameter with what it was inferred
    /// to be. Anything else — including another declaration's `T` — passes
    /// through unchanged.
    fn substitute(&self, ty: Type, substitution: &HashMap<u32, Type>) -> Type {
        let Base::Param(id) = ty.base else {
            return ty;
        };
        let Some(&solved) = substitution.get(&id) else {
            return ty;
        };
        if ty.nullable {
            solved.as_nullable()
        } else {
            solved
        }
    }

    /// Assigns each argument of a call to the parameter it fills.
    ///
    /// Named arguments match by name, the rest by position, and what is left
    /// over goes to the variadic. Decision D4: this happens here so the IR only
    /// ever sees a call of fixed arity.
    fn match_arguments(&mut self, expr: &CallExpr, signature: &Signature) -> Vec<ArgSlot> {
        let mut slots: Vec<ArgSlot> = signature
            .params
            .iter()
            .map(|p| {
                if p.variadic {
                    ArgSlot::Variadic(Vec::new())
                } else if p.has_default {
                    ArgSlot::Default
                } else if p.optional {
                    ArgSlot::Absent
                } else {
                    ArgSlot::Missing
                }
            })
            .collect();

        let mut next_position = 0usize;

        for arg in &expr.args {
            let ty = self.check_expr(&arg.value);

            if let Some(name) = &arg.name {
                let Some(index) = signature.params.iter().position(|p| p.name == name.name) else {
                    let known: Vec<&str> =
                        signature.params.iter().map(|p| p.name.as_str()).collect();
                    self.error(
                        codes::UNKNOWN_ARGUMENT_NAME,
                        name.span,
                        format!(
                            "`{}` has no parameter named `{}`",
                            signature.name, name.name
                        ),
                        format!("its parameters are: {}", known.join(", ")),
                        None,
                    );
                    continue;
                };

                if matches!(slots[index], ArgSlot::Given { .. }) {
                    self.error(
                        codes::WRONG_ARGUMENT_COUNT,
                        name.span,
                        format!("`{}` was already given a value", name.name),
                        "a parameter receives at most one argument",
                        None,
                    );
                    continue;
                }

                slots[index] = ArgSlot::Given { ty, span: arg.span };
                continue;
            }

            // Positional arguments fill the next unnamed slot, and everything
            // past the last fixed parameter belongs to the variadic.
            while next_position < slots.len()
                && matches!(slots[next_position], ArgSlot::Given { .. })
            {
                next_position += 1;
            }

            match slots.get_mut(next_position) {
                Some(ArgSlot::Variadic(items)) => items.push((ty, arg.span)),
                Some(slot) => {
                    *slot = ArgSlot::Given { ty, span: arg.span };
                    next_position += 1;
                }
                None => {
                    self.error(
                        codes::WRONG_ARGUMENT_COUNT,
                        arg.span,
                        format!("`{}` received too many arguments", signature.name),
                        format!("it declares {} parameter(s)", signature.params.len()),
                        None,
                    );
                }
            }
        }

        slots
    }

    // --- Shared checks ----------------------------------------------------

    /// Checks that a value may be stored where a type is expected.
    fn expect_assignable(&mut self, expected: Type, actual: Type, span: Span, context: &str) {
        if expected.accepts(actual)
            || self.is_subclass_of(actual, expected)
            || self.accepts_into_union(expected, actual)
            || self.bare_enum_matches_instance(actual, expected)
        {
            return;
        }

        // Two closures of the same shape still have different types, and
        // saying `(Int32) => Int32` is not `(Int32) => Int32` would be useless.
        if matches!(expected.base, Base::Function(_)) && matches!(actual.base, Base::Function(_)) {
            self.error(
                codes::TYPE_MISMATCH,
                span,
                "a closure cannot be replaced by another one",
                "each closure carries its own captures, so each has its own type",
                Some("declare a separate variable, or call a named `fn` instead".into()),
            );
            return;
        }

        let e = self.name(expected);
        let a = self.name(actual);

        // Narrowing from `T?` to `T` is the one mismatch with a specific fix.
        let help = if actual.admits_null() && !expected.admits_null() {
            Some(format!(
                "the value may be absent: use `?? <fallback>` to provide a `{e}`"
            ))
        } else {
            Some(format!("{context} is `{a}` and `{e}` was expected"))
        };

        self.error(
            codes::TYPE_MISMATCH,
            span,
            "incompatible types",
            format!("there is no implicit conversion from {a} to {e}"),
            help,
        );
    }

    fn expect_boolean(&mut self, actual: Type, span: Span, context: &str) {
        // The question is whether a `Boolean` slot could hold this value, not
        // whether this value's type could hold a `Boolean`: the second is the
        // widening direction, and it answers yes for `Boolean?`.
        if Type::BOOLEAN.accepts(actual) {
            return;
        }

        let found = self.name(actual);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("{context} must be Boolean"),
            format!("a value of type {found} was found"),
            // The most frequent mistake is expecting truthiness from another
            // language, so the help names the comparison explicitly.
            Some("there is no numeric truthiness: write an explicit comparison".into()),
        );
    }

    fn expect_numeric(&mut self, actual: Type, span: Span, op: BinaryOp) {
        if actual.is_unknown() {
            return;
        }
        if !actual.nullable && matches!(actual.base, Base::Int(_)) {
            return;
        }

        let found = self.name(actual);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("the `{}` operator requires numbers", op.as_str()),
            format!("a value of type {found} was found"),
            None,
        );
    }

    /// Like [`Self::expect_numeric`], but also accepts `Float` — used only by
    /// comparison, which has no reason to exclude it the way bitwise/shift
    /// do (roadmap Phase 3b).
    fn expect_comparable(&mut self, actual: Type, span: Span, op: BinaryOp) {
        if actual.is_unknown() {
            return;
        }
        if !actual.nullable && matches!(actual.base, Base::Int(_) | Base::Float(_)) {
            return;
        }

        let found = self.name(actual);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("the `{}` operator requires numbers", op.as_str()),
            format!("a value of type {found} was found"),
            None,
        );
    }

    /// Unlike [`Self::expect_numeric`], deliberately `Int32` only: a
    /// range's element type is hardcoded to `Int32` (`Self::element_type`'s
    /// `Base::Range` arm), so accepting another width here without also
    /// generalizing that would let `for i in a..b` bind `i` to a type its
    /// own endpoints were not written as (roadmap Phase 3b — a range over
    /// another width is future work, not a rename of this check).
    fn expect_numeric_value(&mut self, actual: Type, span: Span, context: &str) {
        if Type::INT32.accepts(actual) {
            return;
        }

        let found = self.name(actual);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("{context} must be Int32"),
            format!("a value of type {found} was found"),
            None,
        );
    }

    /// Rejects comparing values that may be absent.
    ///
    /// `ZIRK_LANGUAGE_SPEC.md` section 4 calls `==` structural equality but
    /// says nothing about how absence compares — whether two absent values are
    /// equal, and whether an absent one equals a present one. Guessing would
    /// fix a semantics the spec has not fixed.
    fn reject_nullable_comparison(&mut self, left: Type, right: Type, expr: &BinaryExpr) {
        if !left.admits_null() && !right.admits_null() {
            return;
        }
        if left.is_unknown() || right.is_unknown() {
            return;
        }

        self.error(
            codes::TYPE_MISMATCH,
            expr.op_span,
            "values that may be absent cannot be compared",
            "the language does not define yet how absence compares",
            Some("resolve it first with `?? <fallback>`, or match on `null`".into()),
        );
    }

    /// Rejects comparing closures.
    ///
    /// `ZIRK_LANGUAGE_SPEC.md` section 4 defines `==` as structural equality,
    /// and a closure has no structure to compare: it is a function pointer plus
    /// whatever it captured.
    fn reject_closure_comparison(&mut self, left: Type, right: Type, expr: &BinaryExpr) {
        if !matches!(left.base, Base::Function(_)) && !matches!(right.base, Base::Function(_)) {
            return;
        }

        self.error(
            codes::TYPE_MISMATCH,
            expr.op_span,
            "closures cannot be compared",
            "there is no structural equality for code",
            Some("compare the values they produce instead".into()),
        );
    }

    fn expect_same(&mut self, left: Type, right: Type, expr: &BinaryExpr) {
        if left.unify(right).is_some() {
            return;
        }

        let l = self.name(left);
        let r = self.name(right);
        self.error(
            codes::TYPE_MISMATCH,
            expr.op_span,
            format!("cannot compare {l} with {r}"),
            "structural equality requires both sides to share a type",
            None,
        );
    }

    fn undeclared(&mut self, ident: &Ident) {
        self.error(
            codes::UNDECLARED_NAME,
            ident.span,
            format!("`{}` is not declared", ident.name),
            "no variable, parameter or function with that name is in scope",
            None,
        );
    }
}

/// What fills one parameter slot at a call site.
enum ArgSlot {
    /// An argument was supplied.
    Given { ty: Type, span: Span },
    /// Nothing was supplied and the declaration provides a default.
    Default,
    /// Nothing was supplied and the parameter is optional, so it is null.
    Absent,
    /// Nothing was supplied and the parameter required something.
    Missing,
    /// The values collected by a `...` parameter.
    Variadic(Vec<(Type, Span)>),
}

/// The fields a constructor body assigns through `this`.
///
/// A syntactic walk, not flow analysis: a field assigned inside a branch counts
/// as assigned. Being stricter would mean deciding which paths run, and a
/// constructor that sets a field in both arms of an `if` is ordinary code, not
/// a mistake.
fn assigned_fields(body: &Block) -> std::collections::HashSet<String> {
    let mut found = std::collections::HashSet::new();
    collect_assigned_fields(&body.statements, &mut found);
    found
}

/// Whether a constructor body delegates to its base with `super(...)`.
fn calls_super(body: &Block) -> bool {
    body.statements.iter().any(|statement| match statement {
        Stmt::Expr(e) => {
            matches!(&e.expr, Expr::Call(call) if matches!(&*call.callee, Expr::Super(_)))
        }
        _ => false,
    })
}

fn collect_assigned_fields(statements: &[Stmt], found: &mut std::collections::HashSet<String>) {
    for statement in statements {
        match statement {
            Stmt::Assign(assign) => {
                if let AssignTarget::Field(field) = &assign.target
                    && matches!(&*field.object, Expr::This(_))
                {
                    found.insert(field.name.name.clone());
                }
            }
            Stmt::If(conditional) => {
                collect_assigned_fields(&conditional.then_branch.statements, found);
                match &conditional.else_branch {
                    Some(ElseBranch::Block(b)) => collect_assigned_fields(&b.statements, found),
                    Some(ElseBranch::If(nested)) => {
                        collect_assigned_fields(&nested.then_branch.statements, found);
                    }
                    None => {}
                }
            }
            Stmt::Loop(loop_stmt) => {
                collect_assigned_fields(&loop_stmt.body.statements, found);
            }
            Stmt::ForIn(for_in) => collect_assigned_fields(&for_in.body.statements, found),
            Stmt::Block(block) => collect_assigned_fields(&block.statements, found),
            _ => {}
        }
    }
}

/// Whether a constructor's parameters admit a call with this many arguments.
///
/// Only arity is consulted here: it is what tells the candidates apart without
/// checking the arguments, and checking them against the wrong constructor
/// would report errors about a signature the author never meant.
fn admits_arity(params: &[ParamInfo], arity: usize) -> bool {
    let required = params
        .iter()
        .filter(|p| !p.optional && !p.variadic && !p.has_default)
        .count();
    let variadic = params.iter().any(|p| p.variadic);

    arity >= required && (variadic || arity <= params.len())
}

/// The result of a native arithmetic operator, if the operands are native and
/// the operator is one they offer.
///
/// This is the whole table `ZIRK_LANGUAGE_SPEC.md` section 4 fixes for the
/// types the language owns. Anything outside it goes to a contract.
fn native_arithmetic(left: Type, right: Type, op: BinaryOp) -> Option<Type> {
    use BinaryOp::*;

    if left.nullable || right.nullable {
        return None;
    }

    match (left.base, right.base, op) {
        // Both operands must already be the same width and signedness —
        // mixed-width arithmetic needs an explicit conversion first (roadmap
        // Phase 3b, task 4.3), the same rule that already applied when
        // `Int32` was the only width there was to mismatch.
        (Base::Int(l), Base::Int(r), Add | Sub | Mul | Div | Rem) if l == r => {
            Some(Type::of(Base::Int(l)))
        }
        // Same rule, one family over: both `Float` operands must already
        // share a width.
        (Base::Float(l), Base::Float(r), Add | Sub | Mul | Div | Rem) if l == r => {
            Some(Type::of(Base::Float(l)))
        }
        // Mixed integer and `Float` arithmetic produces `Float`
        // (`ZIRK_LANGUAGE_SPEC.md` section 3), in either operand order: the
        // integer side is implicitly widened to the `Float` operand's own
        // width before the operation (`zirk-ir/lower.rs`'s `lower_binary`),
        // never the other way, so nothing here is lossy.
        (Base::Int(_), Base::Float(f), Add | Sub | Mul | Div | Rem)
        | (Base::Float(f), Base::Int(_), Add | Sub | Mul | Div | Rem) => {
            Some(Type::of(Base::Float(f)))
        }
        // `String + String` concatenates.
        (Base::String, Base::String, Add) => Some(Type::STRING),
        // `String * Integer` repeats, in either order: `"ja" * 3` and
        // `3 * "ja"` are the same request written two ways. `Int32`
        // specifically: the runtime's own `zirk_str_repeat` takes its count
        // as `Int32`, and nothing converts a wider or narrower count for it
        // yet.
        (Base::String, Base::Int(IntWidth::I32), Mul)
        | (Base::Int(IntWidth::I32), Base::String, Mul) => Some(Type::STRING),
        _ => None,
    }
}

/// The reserved method an operator resolves to on a user type.
///
/// `ZIRK_LANGUAGE_SPEC.md` section 4: an operator is overloaded only through
/// contracts of the language, and the overload changes neither precedence nor
/// arity — which is exactly what naming a method rather than the operator
/// guarantees.
fn operator_method(op: BinaryOp) -> &'static str {
    use BinaryOp::*;
    match op {
        Add => "_add",
        Sub => "_subtract",
        Mul => "_multiply",
        Div => "_divide",
        Rem => "_remainder",
        // The rest are not arithmetic and never reach here.
        _ => "_unsupported",
    }
}
