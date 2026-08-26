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
    IntWidth, MethodInfo, Type, TypeNames, TypeParamInfo, describe, is_ffi_safe, pending_type,
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
    /// Interned `Pointer<T>` pointee types, indexed by the id their
    /// [`Base::Pointer`] carries (roadmap Phase 4e, design D1).
    pub pointer_types: Vec<Type>,
    /// Interned `Weak<T>` referent types, indexed by the id their
    /// [`Base::Weak`] carries (roadmap Phase 4e, `fase-4e-weak`, design D1).
    pub weak_types: Vec<Type>,
    /// Interned `NativeSlice<T>` element types, indexed by the id their
    /// [`Base::NativeSlice`] carries (roadmap Phase 4e,
    /// `fase-4e-native-slice`, design D1).
    pub native_slice_types: Vec<Type>,
    /// Interned `NativeSliceMut<T>` element types, indexed by the id their
    /// [`Base::NativeSliceMut`] carries (roadmap Phase 4e,
    /// `fase-4e-native-slice`, design D1).
    pub native_slice_mut_types: Vec<Type>,
    /// Declared `extern "C" fn` signatures, keyed by name (roadmap Phase 4e,
    /// `ADR-015`).
    pub externs: HashMap<String, ExternSignature>,
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
    /// The id of the compiler-known `Result<T,E>` enum, in `enums` (roadmap
    /// Phase 4a) — `zirk-ir/lower.rs` needs this to recognize a `Result`
    /// method call (`is_ok`, `unwrap`, …) structurally, by receiver type,
    /// the same way `Checker::is_native_to_string_call` recognizes
    /// `to_string()` on a native scalar: `Result`'s methods are not a real
    /// method table (`ZIRK_LANGUAGE_SPEC.md` section 7 forbids enums from
    /// having user-declarable ones at all), so there is no `MethodInfo` for
    /// lowering to look up — only this id, to tell "a `Result` value" apart
    /// from any other enum.
    pub native_result: Option<u32>,
    /// The compiler-known exception hierarchy's class ids (roadmap Phase
    /// 4b): `Error`, `Throwable`, `RuntimeError`, `StackTrace`, in that
    /// registration order — `zirk-ir/lower.rs` needs `throwable` to build
    /// the `IsInstance` check a `catch` clause tests against, and
    /// `stack_trace` to construct the empty stub `Throwable::stack_trace()`
    /// returns without a real unwind-time frame capture.
    pub native_exceptions: Option<NativeExceptions>,
}

/// See [`CheckedProgram::native_exceptions`].
#[derive(Debug, Clone, Copy)]
pub struct NativeExceptions {
    pub error: u32,
    pub throwable: u32,
    pub runtime_error: u32,
    pub stack_trace: u32,
    /// The four compiler-known, concrete `RuntimeError` subclasses
    /// `fase-4d-runtimeerror` registers (D9): division by zero, an
    /// out-of-range shift, a negative string-repeat count, and a `Float`
    /// operation producing `NaN`. Unlike `error`/`throwable`/`runtime_error`
    /// above, these are real, instantiable classes — `zirk-ir/src/lower.rs`
    /// both builds one of these directly whenever the matching native check
    /// fails (D10) and synthesizes their four method bodies by hand (D8).
    pub division_by_zero: u32,
    pub invalid_shift: u32,
    pub invalid_repeat: u32,
    pub float_nan: u32,
    /// `IndexOutOfBoundsError` (roadmap Phase 4e, `fase-4e-native-slice`,
    /// design D3): thrown when a `NativeSlice<T>`/`NativeSliceMut<T>` index
    /// is out of range — the "controlled bounds error" the spec's own "View
    /// indexing stays bounds-checked" scenario requires. Registered the
    /// same way the four failures above are.
    pub index_out_of_bounds: u32,
    /// `NativeError` (roadmap Phase 4e, `fase-4e-native-slice`, design's
    /// task 1.2): the error type `pointer.as_slice(length)`/
    /// `.as_slice_mut(length)` produce in `Result<..., NativeError>` when
    /// construction fails — a `Result`-carried value, not thrown, but
    /// registered through the exact same `register_native_failure` closure
    /// as the five classes above since its shape (a `RuntimeError`
    /// subclass with one `reason: String` field) is identical.
    pub native_error: u32,
}

/// What the checker learned about one lambda.
#[derive(Debug, Clone)]
pub struct LambdaInfo {
    /// Names captured from the enclosing scope, in a stable order.
    pub captures: Vec<Capture>,
    pub fn_type: u32,
    /// The name of this lambda's own binding, when it is a *recursive*
    /// lambda referring to itself (`ZIRK_LANGUAGE_SPEC.md` section 6: a
    /// lambda calling itself through its own explicitly `Fn(...) => R`
    /// typed binding, roadmap Phase 4d) — present in `captures` too (it is
    /// still an ordinary capture for type-checking purposes), but `zirk-ir`
    /// reads this to lower a self-call directly rather than through a
    /// runtime capture, since no closure value for it exists yet at the
    /// point this literal is still being built (`Checker::check_let`'s own
    /// pre-declaration).
    pub recursive_binding: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub name: String,
    pub ty: Type,
}

/// A checked `extern "C" fn` declaration (roadmap Phase 4e, `ADR-015`).
#[derive(Debug, Clone)]
pub struct ExternSignature {
    pub params: Vec<Type>,
    pub returns: Type,
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
    pointer_types: &'t [Type],
    weak_types: &'t [Type],
    native_slice_types: &'t [Type],
    native_slice_mut_types: &'t [Type],
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

    fn pointer_element(&self, id: u32) -> Type {
        self.pointer_types
            .get(id as usize)
            .copied()
            .unwrap_or(Type::UNKNOWN)
    }

    fn weak_element(&self, id: u32) -> Type {
        self.weak_types
            .get(id as usize)
            .copied()
            .unwrap_or(Type::UNKNOWN)
    }

    fn native_slice_element(&self, id: u32) -> Type {
        self.native_slice_types
            .get(id as usize)
            .copied()
            .unwrap_or(Type::UNKNOWN)
    }

    fn native_slice_mut_element(&self, id: u32) -> Type {
        self.native_slice_mut_types
            .get(id as usize)
            .copied()
            .unwrap_or(Type::UNKNOWN)
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
    /// The type the expression about to be checked is expected to produce,
    /// if the immediate surrounding context already knows one (roadmap
    /// Phase 4a — a `let` with an explicit annotation, a `return` against
    /// the function's declared type) — consulted only as a fallback by a
    /// generic construct's own type-parameter inference
    /// (`Self::infer_type_params`'s own doc comment names this exact gap:
    /// "the expected result... not consulted yet"), when the arguments
    /// alone leave a parameter unsolved (`Result.Ok(v)` cannot determine
    /// `E` from `v` alone).
    ///
    /// Single-shot by construction: `check_expr` takes (clears) it the
    /// instant it starts, so it is never implicitly inherited by a nested
    /// expression `check_expr` recurses into — only the exact expression
    /// a caller set it in front of ever sees it. This is what keeps
    /// `mut r: Result<Int32,String> = wrap(Result<Boolean,String>.Ok(true));`
    /// from leaking the outer `Result<Int32,String>` into the inner
    /// construction's own inference.
    expected_type: Option<Type>,
    /// The class whose body is being checked, if any. It is what `this` names.
    this_type: Option<Type>,
    /// Whether the body being checked is a constructor, which is the one place
    /// an `inmut` field may be written.
    in_constructor: bool,
    /// How many loops enclose the statement being checked.
    ///
    /// Zero means `break` and `continue` have nothing to jump out of.
    loop_depth: u32,
    /// How many `unsafe` boundaries (an `unsafe {}` block or an `unsafe fn`
    /// body) enclose the statement being checked (design D3, roadmap Phase
    /// 4e). Zero means a pointer operation or an extern call has nothing
    /// justifying it. Mirrors `loop_depth` exactly.
    unsafe_depth: u32,
    /// How many `commit {}` boundaries enclose the statement being checked
    /// (design D3, roadmap Phase 4e). Zero means an extern call is still
    /// missing its irreversible-effect boundary even inside `unsafe`.
    commit_depth: u32,
    /// Captures collected for the lambda being checked, innermost last.
    capture_stack: Vec<Vec<Capture>>,
    /// The name of a recursive lambda's own binding, set only around
    /// checking the lambda literal that directly initializes it
    /// (`Self::check_let`) — consumed (`Option::take`'d) by
    /// `Self::check_lambda` for exactly that one lambda, so a lambda
    /// nested inside it never mistakes itself for the recursive one too.
    recursive_binding: Option<String>,
    /// The `throws` set of the function or method being checked (roadmap
    /// Phase 4b) — empty when it declares none. What
    /// [`Self::check_function`]/the method equivalent verifies every
    /// remaining entry of [`Self::pending_throws`] is covered by, once the
    /// body has been fully checked.
    current_throws: Vec<Type>,
    /// Every exception type a `throw`, a rethrow, or a call to a `throws`
    /// function/method has produced since the last point that resolved it
    /// (roadmap Phase 4b) — reset and drained at each `try`'s own `catch`
    /// coverage check, and at the end of the enclosing function/method's
    /// body, where whatever remains must be covered by `current_throws`.
    ///
    /// A flat accumulator rather than real per-statement dataflow, the same
    /// simplification `current_return`/`loop_depth` already make: "did
    /// *something* in this stretch of code throw a type nothing here
    /// catches" is answerable without tracking exactly which statement.
    pending_throws: Vec<Type>,
    /// The declared type of the innermost active `catch`, if the statement
    /// being checked is (transitively) inside one — what a bare `throw;`
    /// rethrows (roadmap Phase 4b). `None` outside any `catch`, which is
    /// exactly when a bare rethrow is illegal.
    catch_type: Option<Type>,
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
    /// See [`CheckedProgram::native_result`].
    native_result: Option<u32>,
    /// See [`CheckedProgram::native_exceptions`].
    native_exceptions: Option<NativeExceptions>,
    /// Ids of the language's own `Resource<E>` interface and its `E`
    /// parameter, minted by [`Self::register_native_resource_contract`]
    /// (roadmap Phase 4c) — only consulted while checking `implements
    /// Resource<...>` and a `match ... with` binding; `zirk-ir/lower.rs`
    /// needs none of this, since it reaches `close()`/`is_closed()` by
    /// ordinary static method dispatch on the binding's own concrete class.
    native_resource: Option<NativeResource>,
    /// Interned `Pointer<T>` pointee types, indexed by the id their
    /// [`Base::Pointer`] carries (roadmap Phase 4e, design D1).
    pointer_types: Vec<Type>,
    /// Interned `Weak<T>` referent types, indexed by the id their
    /// [`Base::Weak`] carries (roadmap Phase 4e, `fase-4e-weak`, design D1).
    weak_types: Vec<Type>,
    /// Interned `NativeSlice<T>` element types, indexed by the id their
    /// [`Base::NativeSlice`] carries (roadmap Phase 4e,
    /// `fase-4e-native-slice`, design D1).
    native_slice_types: Vec<Type>,
    /// Interned `NativeSliceMut<T>` element types, indexed by the id their
    /// [`Base::NativeSliceMut`] carries (roadmap Phase 4e,
    /// `fase-4e-native-slice`, design D1).
    native_slice_mut_types: Vec<Type>,
    /// See [`CheckedProgram::externs`].
    externs: HashMap<String, ExternSignature>,
    /// Id of the language's own `Clone` contract, minted by
    /// [`Self::register_native_clone_contract`] (roadmap Phase 4e,
    /// `fase-4e-clone`, design D1) — unlike `Iterable`/`Resource`, it
    /// carries no methods of its own: whether a type actually satisfies it
    /// is decided structurally by [`Self::is_clone_type`], not by dispatch
    /// through this contract's (empty) method table. Registered so
    /// `implements Clone` and `T from Clone` both resolve to a real name
    /// instead of `UNKNOWN_TYPE`.
    native_clone: Option<u32>,
    /// Per-class memoized answer to "is every field of this class's
    /// declared shape `Clone`" (design D1) — populated by
    /// [`Self::class_is_clone`], only once its own top-level call (not a
    /// nested one reached through a cycle) has fully unwound, so a
    /// provisional cycle-breaking `true` seen mid-computation is never
    /// cached as final ahead of the fields that would actually disprove it.
    clone_cache: HashMap<u32, bool>,
}

/// See [`Checker::native_resource`].
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct NativeResource {
    resource: u32,
    e: u32,
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
            expected_type: None,
            this_type: None,
            in_constructor: false,
            loop_depth: 0,
            unsafe_depth: 0,
            commit_depth: 0,
            capture_stack: Vec::new(),
            recursive_binding: None,
            current_throws: Vec::new(),
            pending_throws: Vec::new(),
            catch_type: None,
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
            native_result: None,
            native_exceptions: None,
            native_resource: None,
            for_in_iteration: HashMap::new(),
            variant_constructions: HashMap::new(),
            pointer_types: Vec::new(),
            weak_types: Vec::new(),
            native_slice_types: Vec::new(),
            native_slice_mut_types: Vec::new(),
            externs: HashMap::new(),
            native_clone: None,
            clone_cache: HashMap::new(),
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
                pointer_types: &self.pointer_types,
                weak_types: &self.weak_types,
                native_slice_types: &self.native_slice_types,
                native_slice_mut_types: &self.native_slice_mut_types,
            },
        )
    }

    // --- Program ----------------------------------------------------------

    fn run(mut self, program: &Program) -> CheckedProgram {
        self.register_native_iteration_contracts();
        self.register_native_clone_contract();
        self.register_native_functions();
        self.register_native_result_enum();
        self.register_native_exception_hierarchy();
        self.register_native_resource_contract();
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
        //
        // Only registered here (name, arity, type params, an empty
        // `variants` placeholder) — the same split `register_class` uses
        // for exactly the same reason: a variant's own associated field may
        // name the enum currently being declared (`Cons(tail: IntList)`) or
        // one declared later in the file, and resolving that needs every
        // enum's (and every class's) name already known. Associated field
        // types are resolved later, in `declare_members`, once every class
        // and enum name in the file is registered.
        for e in &program.enums {
            self.register_enum(e);
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
        for e in &program.externs {
            self.declare_extern_fn(e);
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
            native_result: self.native_result,
            native_exceptions: self.native_exceptions,
            pointer_types: self.pointer_types,
            weak_types: self.weak_types,
            native_slice_types: self.native_slice_types,
            native_slice_mut_types: self.native_slice_mut_types,
            externs: self.externs,
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
            // `Float16` prints by widening to `Float32` first — always
            // exact, since every `f16` value is representable in `f32`
            // without loss (`Type::accepts`'s float-widening rule). No such
            // safe fallback exists for `Float128`.
            Base::Float(w) => matches!(w, FloatWidth::F16 | FloatWidth::F32 | FloatWidth::F64),
            Base::Class(id) => self.classes[id as usize]
                .method("to_string")
                .is_some_and(|m| m.params.is_empty() && m.returns == Type::STRING),
            // A value reached through a contract that itself declares
            // `to_string()` — every implementer supplies one (either its own
            // or the contract's default body), so the dispatch table always
            // has a real method to call through (roadmap Phase 3b task 8's
            // follow-up).
            Base::Contract(id) => self.contracts[id as usize]
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
                "Float128 has no stable conversion to text yet (roadmap Phase 3b, task 8.3)"
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
                throws: Vec::new(),
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
                throws: Vec::new(),
            });

        self.native_iteration = Some(NativeIteration {
            iterable,
            iterator,
            iteration,
        });
    }

    /// Registers `Clone` as if it were declared by the language itself
    /// (roadmap Phase 4e, `fase-4e-clone`, design D1), the same
    /// "inject the tables directly" mechanism
    /// [`Self::register_native_iteration_contracts`] uses.
    ///
    /// Unlike `Iterable`/`Iterator`/`Resource`, `Clone` carries no methods
    /// in its own table: whether a type satisfies it is never decided by
    /// dispatching through a `Clone`-shaped vtable entry (there is no
    /// vtable entry — `.clone()` is either an ordinary method a class wrote
    /// itself, or the compiler's own derived operation, chosen by
    /// [`Self::check_method_call_on`] before it ever reaches contract
    /// dispatch). Registering it here exists only so `implements Clone`
    /// (the grammar's own scenario) and `T from Clone` (the type-system's
    /// "Generic projection needs Clone" scenario) both resolve to a real
    /// name instead of `UNKNOWN_TYPE` — [`Self::satisfies_constraint`] and
    /// [`Self::check_method_call_on`]'s own derivation check
    /// (`Self::is_clone_type`) are what actually decide `Clone`-ness,
    /// structurally, not this table entry.
    fn register_native_clone_contract(&mut self) {
        let at = Span::empty(0);
        let clone = self.contracts.len() as u32;
        self.contracts.push(ContractType {
            name: "Clone".into(),
            kind: ContractKind::Interface,
            methods: Vec::new(),
            type_params: Vec::new(),
            shared: true,
            span: at,
        });
        self.native_clone = Some(clone);
    }

    /// Registers `fatalError(message: String): Never` as if it were an
    /// ordinary declared function (roadmap Phase 4a, `ZIRK_LANGUAGE_SPEC.md`
    /// section 9) — reuses every bit of ordinary call checking
    /// (`Self::check_direct_call`) for free, the same way this checker
    /// already treats `Int8`/`String`/etc. as pre-resolved rather than
    /// parsed. Its lowering is a compiler intrinsic (`zirk-ir/lower.rs`'s
    /// `lower_fatal_error_call`), not a call to a Zirk-level body — there is
    /// none — which is unrelated to how it type-checks.
    fn register_native_functions(&mut self) {
        self.functions.insert(
            "fatalError".to_string(),
            Signature {
                name: "fatalError".to_string(),
                params: vec![ParamInfo {
                    name: "message".to_string(),
                    ty: Type::STRING,
                    optional: false,
                    has_default: false,
                    variadic: false,
                }],
                returns: Type::of(Base::Never),
                shared: true,
                span: Span::empty(0),
                type_params: Vec::new(),
                throws: Vec::new(),
            },
        );
    }

    /// Registers `Result<T,E>` as `enum Result<T, E> { Ok(T); Error(E); }`
    /// (roadmap Phase 4a, `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`
    /// section 2, the authorial source of truth for `Result`'s exact
    /// shape), the same "inject the tables directly" mechanism
    /// `register_native_iteration_contracts` already uses for `Iteration<T>`.
    ///
    /// Its method API (`is_ok`, `unwrap`, …) is deliberately *not* stored
    /// here as an `EnumType.methods` table — no such field exists, and none
    /// should: `ZIRK_LANGUAGE_SPEC.md` section 7 forbids an enum from
    /// declaring user methods at all, and `Result` does not get a silent
    /// exception to that rule. Its methods are recognized structurally, by
    /// receiver type and name, the same way `to_string()` on a native
    /// scalar already is (`Self::is_native_result_call`, below).
    fn register_native_result_enum(&mut self) {
        let at = Span::empty(0);

        let t = self.type_params.len() as u32;
        self.type_params.push(TypeParamInfo {
            name: "T".into(),
            constraints: Vec::new(),
            span: at,
        });
        let e = self.type_params.len() as u32;
        self.type_params.push(TypeParamInfo {
            name: "E".into(),
            constraints: Vec::new(),
            span: at,
        });

        let result = self.enums.len() as u32;
        self.enums.push(EnumType {
            name: "Result".into(),
            variants: vec![
                EnumVariantInfo {
                    name: "Ok".into(),
                    associated: vec![AssociatedFieldInfo {
                        name: "value".into(),
                        ty: Type::of(Base::Param(t)),
                    }],
                    span: at,
                },
                EnumVariantInfo {
                    name: "Error".into(),
                    associated: vec![AssociatedFieldInfo {
                        name: "error".into(),
                        ty: Type::of(Base::Param(e)),
                    }],
                    span: at,
                },
            ],
            type_params: vec![t, e],
            shared: true,
            span: at,
        });

        self.native_result = Some(result);
    }

    /// Registers `Error`/`Throwable`/`RuntimeError`/`StackTrace` (roadmap
    /// Phase 4b, `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3),
    /// the same "inject the tables directly" mechanism
    /// [`Self::register_native_iteration_contracts`] and
    /// [`Self::register_native_result_enum`] already use.
    ///
    /// Unlike `Result`, these are ordinary (abstract) classes — a program's
    /// own exception type `implements Throwable` the same way it would
    /// adopt any other `abstract class`'s requirements, so no structural
    /// method dispatch is needed here: the existing class machinery
    /// (`implements` against an `abstract class`, `abstract_bases`,
    /// `require_abstract_conformance`) does the rest once these exist in
    /// `self.classes`.
    ///
    /// Two narrowings from the doc's exact pseudocode: `suppressed(): List<Error>`
    /// is dropped from `Error` — `List<T>` is a Phase 7 collection that does
    /// not exist yet, and populating `suppressed` from a `finally` cleanup
    /// failure is already out of scope for this pass (`fase-4b-excepciones`
    /// proposal). `stack_trace(): StackTrace` stays exactly as declared —
    /// abstract, so a concrete exception class must implement it — since a
    /// compiler-supplied default body would need the same structural
    /// dispatch machinery `Result` uses, for one method, on a receiver type
    /// otherwise indistinguishable from an ordinary class's.
    fn register_native_exception_hierarchy(&mut self) {
        let at = Span::empty(0);

        let stack_trace = self.classes.len() as u32;
        self.classes.push(ClassType {
            name: "StackTrace".into(),
            kind: ClassKind::Class,
            base: None,
            fields: Vec::new(),
            // One zero-argument constructor: `StackTrace()` builds the empty
            // stub every `stack_trace()` override in this pass returns —
            // there are no frames to capture yet.
            constructors: vec![Vec::new()],
            methods: Vec::new(),
            contracts: Vec::new(),
            contract_instances: Vec::new(),
            abstract_bases: Vec::new(),
            type_params: Vec::new(),
            shared: true,
            span: at,
        });

        let error = self.classes.len() as u32;
        let error_methods = vec![
            MethodInfo {
                name: "message".into(),
                params: Vec::new(),
                returns: Type::STRING,
                visibility: Visibility::Public,
                span: at,
                index: 0,
                owner: error,
                from_contract: None,
                overridden: true,
                throws: Vec::new(),
            },
            MethodInfo {
                name: "code".into(),
                params: Vec::new(),
                returns: Type::STRING,
                visibility: Visibility::Public,
                span: at,
                index: 1,
                owner: error,
                from_contract: None,
                overridden: true,
                throws: Vec::new(),
            },
            MethodInfo {
                name: "cause".into(),
                params: Vec::new(),
                returns: Type::of(Base::Class(error)).as_nullable(),
                visibility: Visibility::Public,
                span: at,
                index: 2,
                owner: error,
                from_contract: None,
                overridden: true,
                throws: Vec::new(),
            },
        ];
        self.classes.push(ClassType {
            name: "Error".into(),
            kind: ClassKind::Abstract,
            base: None,
            fields: Vec::new(),
            constructors: Vec::new(),
            methods: error_methods.clone(),
            contracts: Vec::new(),
            contract_instances: Vec::new(),
            abstract_bases: Vec::new(),
            type_params: Vec::new(),
            shared: true,
            span: at,
        });

        let throwable = self.classes.len() as u32;
        let mut throwable_methods = error_methods;
        throwable_methods.push(MethodInfo {
            name: "stack_trace".into(),
            params: Vec::new(),
            returns: Type::of(Base::Class(stack_trace)),
            visibility: Visibility::Public,
            span: at,
            index: throwable_methods.len(),
            owner: throwable,
            from_contract: None,
            overridden: true,
            throws: Vec::new(),
        });
        self.classes.push(ClassType {
            name: "Throwable".into(),
            kind: ClassKind::Abstract,
            base: None,
            fields: Vec::new(),
            constructors: Vec::new(),
            methods: throwable_methods.clone(),
            contracts: Vec::new(),
            contract_instances: Vec::new(),
            abstract_bases: vec![error],
            type_params: Vec::new(),
            shared: true,
            span: at,
        });

        let runtime_error = self.classes.len() as u32;
        self.classes.push(ClassType {
            name: "RuntimeError".into(),
            kind: ClassKind::Abstract,
            base: None,
            fields: Vec::new(),
            constructors: Vec::new(),
            methods: throwable_methods,
            contracts: Vec::new(),
            contract_instances: Vec::new(),
            abstract_bases: vec![error, throwable],
            type_params: Vec::new(),
            shared: true,
            span: at,
        });

        // The four compiler-known, concrete `RuntimeError` subclasses
        // `fase-4d-runtimeerror` adds (D9): each `implements RuntimeError`
        // with a single `reason: String` field and the same four methods
        // (`message`/`code`/`cause`/`stack_trace`), at the same indices
        // `throwable_methods` above already uses — that alignment is what
        // lets `catch RuntimeError(e)`/`catch Throwable(e)` dispatch through
        // one of these exactly like it does through a user's own
        // `implements RuntimeError` class (the mechanism is D4's, unchanged
        // here). Unlike `Error`/`Throwable`/`RuntimeError`, these are
        // `ClassKind::Class` — concrete and instantiable — since a program
        // (or the compiler itself, from `zirk-ir/src/lower.rs`) constructs
        // one whenever the matching native check actually fails.
        let native_methods = |owner: u32| {
            vec![
                MethodInfo {
                    name: "message".into(),
                    params: Vec::new(),
                    returns: Type::STRING,
                    visibility: Visibility::Public,
                    span: at,
                    index: 0,
                    owner,
                    from_contract: None,
                    overridden: false,
                    throws: Vec::new(),
                },
                MethodInfo {
                    name: "code".into(),
                    params: Vec::new(),
                    returns: Type::STRING,
                    visibility: Visibility::Public,
                    span: at,
                    index: 1,
                    owner,
                    from_contract: None,
                    overridden: false,
                    throws: Vec::new(),
                },
                MethodInfo {
                    name: "cause".into(),
                    params: Vec::new(),
                    returns: Type::of(Base::Class(error)).as_nullable(),
                    visibility: Visibility::Public,
                    span: at,
                    index: 2,
                    owner,
                    from_contract: None,
                    overridden: false,
                    throws: Vec::new(),
                },
                MethodInfo {
                    name: "stack_trace".into(),
                    params: Vec::new(),
                    returns: Type::of(Base::Class(stack_trace)),
                    visibility: Visibility::Public,
                    span: at,
                    index: 3,
                    owner,
                    from_contract: None,
                    overridden: false,
                    throws: Vec::new(),
                },
            ]
        };

        let register_native_failure = |classes: &mut Vec<ClassType>, name: &str| -> u32 {
            let id = classes.len() as u32;
            classes.push(ClassType {
                name: name.into(),
                kind: ClassKind::Class,
                base: None,
                fields: vec![FieldInfo {
                    name: "reason".into(),
                    ty: Type::STRING,
                    visibility: Visibility::Private,
                    mutability: Mutability::Immutable,
                    span: at,
                    owner: id,
                }],
                // `construct(reason: String)`: one parameter, no user-visible
                // body — `zirk-ir::lower_construction`'s own special case for
                // these four classes builds it directly (an `Alloc` plus a
                // `StoreField`), the same "no `program.classes` entry, no
                // real constructor symbol" shape `StackTrace` already has,
                // just with one argument instead of zero (D8's own note on
                // reconciling this with the sixteen hand-built bodies).
                constructors: vec![vec![ParamInfo {
                    name: "reason".into(),
                    ty: Type::STRING,
                    optional: false,
                    has_default: false,
                    variadic: false,
                }]],
                methods: native_methods(id),
                contracts: Vec::new(),
                contract_instances: Vec::new(),
                // Direct only, matching `implements_abstract_class`'s own
                // doc comment on why a user class implementing `RuntimeError`
                // gets `[runtime_error]` here, not the fully flattened
                // `[error, throwable, runtime_error]` — `zirk-ir`'s own
                // `ObjectLayout::ancestors` construction is what flattens it
                // transitively, for every class alike.
                abstract_bases: vec![runtime_error],
                type_params: Vec::new(),
                shared: true,
                span: at,
            });
            id
        };

        let division_by_zero = register_native_failure(&mut self.classes, "DivisionByZeroError");
        let invalid_shift = register_native_failure(&mut self.classes, "InvalidShiftError");
        let invalid_repeat = register_native_failure(&mut self.classes, "InvalidRepeatError");
        let float_nan = register_native_failure(&mut self.classes, "FloatNanError");
        // Roadmap Phase 4e, `fase-4e-native-slice`: `IndexOutOfBoundsError`
        // (thrown by a bounds-checked `view[i]`, design D3) and
        // `NativeError` (returned in `Result<..., NativeError>` by
        // `pointer.as_slice(length)`/`.as_slice_mut(length)`, task 1.2) —
        // registered through the same closure as the four failures above.
        let index_out_of_bounds =
            register_native_failure(&mut self.classes, "IndexOutOfBoundsError");
        let native_error = register_native_failure(&mut self.classes, "NativeError");

        self.native_exceptions = Some(NativeExceptions {
            error,
            throwable,
            runtime_error,
            stack_trace,
            division_by_zero,
            invalid_shift,
            invalid_repeat,
            float_nan,
            index_out_of_bounds,
            native_error,
        });
    }

    /// Registers `interface Resource<E from Error> { fn close(): Result<Void,E>;
    /// fn is_closed(): Boolean; }` (roadmap Phase 4c,
    /// `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 4), the same
    /// "inject the tables directly" mechanism
    /// [`Self::register_native_iteration_contracts`] uses for `Iterable<T>`.
    ///
    /// Unlike `Iterable<T>`/`Iterator<T>`, `E`'s own `from Error` constraint
    /// is expressed directly with [`TypeParamInfo::constraints`] — the same
    /// field a user-declared interface's own `from` clause resolves into
    /// (`Self::resolve_class_type_param_constraints`) — since `Error` (and
    /// therefore a real constraint to check against) already exists once
    /// [`Self::register_native_exception_hierarchy`] has run, unlike when
    /// `Iterable<T>`'s own unconstrained `T` was registered.
    ///
    /// A class that `implements Resource<SomeError>` reaches `close()`/
    /// `is_closed()` by ordinary static dispatch on its own concrete type —
    /// `match ... with` never calls through a `Resource`-typed reference —
    /// so unlike `Iterable`/`Iterator` this contract needs no dispatch-table
    /// specialization to be usable; only [`Self::resolve_implements_args`]'s
    /// own `NOT_LOWERED` gate has to name it as another exception, alongside
    /// `Iterable`/`Iterator`.
    fn register_native_resource_contract(&mut self) {
        let at = Span::empty(0);
        let native_exceptions = self
            .native_exceptions
            .expect("register_native_exception_hierarchy runs first");

        let e = self.type_params.len() as u32;
        self.type_params.push(TypeParamInfo {
            name: "E".into(),
            constraints: vec![Type::of(Base::Class(native_exceptions.error))],
            span: at,
        });
        let resource = self.contracts.len() as u32;
        self.contracts.push(ContractType {
            name: "Resource".into(),
            kind: ContractKind::Interface,
            methods: Vec::new(),
            type_params: vec![e],
            shared: true,
            span: at,
        });

        let result = self
            .native_result
            .expect("register_native_result_enum runs first");

        // `close(): Result<Void,E>`
        let close_returns = self.intern_enum_instance(GenericEnumInstance {
            enum_id: result,
            args: vec![Type::VOID, Type::of(Base::Param(e))],
        });
        self.contracts[resource as usize]
            .methods
            .push(ContractMethod {
                name: "close".into(),
                params: Vec::new(),
                returns: Type::of(Base::EnumInstance(close_returns)),
                span: at,
                has_default: false,
                index: 0,
                throws: Vec::new(),
            });

        // `is_closed(): Boolean`
        self.contracts[resource as usize]
            .methods
            .push(ContractMethod {
                name: "is_closed".into(),
                params: Vec::new(),
                returns: Type::BOOLEAN,
                span: at,
                has_default: false,
                index: 1,
                throws: Vec::new(),
            });

        self.native_resource = Some(NativeResource { resource, e });
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

            let returns = self.resolve_type(&method.return_type);
            let throws = self.resolve_throws_clause(&method.throws);
            methods.push(ContractMethod {
                name: method.name.name.clone(),
                params: method
                    .params
                    .iter()
                    .map(|p| self.resolve_param(p))
                    .collect(),
                returns,
                span: method.name.span,
                has_default: method.body.is_some(),
                index,
                throws,
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
    /// (task 6.9, roadmap Phase 4c): `Iterable`, `Iterator` and `Resource`,
    /// injected directly into the tables rather than parsed, so application
    /// code cannot reopen them.
    fn is_native_contract_name(name: &str) -> bool {
        matches!(name, "Iterable" | "Iterator" | "Resource" | "Clone")
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
        // nothing else specializes a class's own generic contract table yet
        // — except `implements Resource<E>` (roadmap Phase 4c), which needs
        // no specialized dispatch table at all: `match ... with` reaches
        // `close()`/`is_closed()` by ordinary static dispatch on the
        // binding's own concrete class, never through a `Resource`-typed
        // reference.
        if self
            .native_iteration
            .is_none_or(|n| contract != n.iterable && contract != n.iterator)
            && self.native_resource.is_none_or(|n| contract != n.resource)
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
            // `Self::declare_class_members` seeds `class`'s own flattened
            // method list from the abstract class's (D1/D3, so every adopter
            // shares its virtual index) before this runs, so `.method(...)`
            // below finds an entry even when `class` never wrote an
            // `override` at all — still exactly the abstract's own
            // placeholder, `owner == abstract_id`, unless some declaration
            // in `class`'s own chain replaced it. Treating that placeholder
            // as "supplied" would turn a genuinely missing override into a
            // spurious `MISSING_OVERRIDE` pointing at the abstract class's
            // own declaration instead of the right `MISSING_IMPLEMENTATION`.
            let supplied = self.classes[class as usize]
                .method(&method.name)
                .filter(|m| m.owner != abstract_id)
                .cloned();
            let Some(supplied) = supplied else {
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
            throws: method.throws.clone(),
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
            throws: method.throws.clone(),
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

        // Enums have no base/`implements` hierarchy to order against, so
        // declaration order relative to `program.enums` (or relative to the
        // class loop above) does not matter here — every class and every
        // enum is already registered by this point (`register_class` and
        // `register_enum` both ran ahead of this pass), which is exactly
        // what lets a variant's associated field name any of them, in
        // either declaration order, including the enum's own type.
        for e in &program.enums {
            self.declare_enum_variants(e);
        }

        // `fase-3-recursive-enums` lets a variant's associated field name
        // the enclosing enum's own type (declaration order no longer
        // matters), but every enum still lowers to an inline-flattened
        // struct (`EnumLayout`) with no indirection anywhere in the
        // pipeline — a variant field whose type embeds the enum's own
        // type with nothing in between (no `Pointer<T>`, no class
        // reference, nothing heap-allocated to break the cycle) asks for
        // an infinitely-sized LLVM type and crashes the compiler with a
        // stack overflow (confirmed, not assumed: `IntList`'s own `Cons`
        // reproduces this). Boxing a genuinely self-referential field is
        // its own future design (a new `IrType` case, a runtime
        // allocation kind, GC root/tracing rules) — out of this pass's
        // scope. Until that exists, reject the specific unindirected
        // shape with a clear diagnostic instead of letting it reach IR
        // lowering at all.
        self.reject_unindirected_enum_cycles(program);
    }

    /// Rejects a variant field whose type embeds its own enclosing enum's
    /// type with no indirection in between, directly or through another
    /// enum's own inline embedding — see [`Self::declare_members`]'s own
    /// comment for why this can never lower correctly today.
    fn reject_unindirected_enum_cycles(&mut self, program: &Program) {
        for decl in &program.enums {
            let Some(id) = self.enums.iter().position(|e| e.name == decl.name.name) else {
                continue;
            };
            let mut visiting = vec![id as u32];
            if let Some(cycle_at) = self.enum_embeds_inline(id as u32, &mut visiting) {
                let name = self.enums[id].name.clone();
                let via = self.enums[cycle_at as usize].name.clone();
                let path = if cycle_at == id as u32 {
                    format!("`{name}` names itself")
                } else {
                    format!("`{name}` reaches itself back through `{via}`")
                };
                self.not_lowered(
                    self.enums[id].span,
                    &format!("a self-referential enum with no indirection ({path})"),
                    "wrap the recursive field in `Pointer<T>` or a class reference for now — boxing a directly self-referential enum field is not implemented yet",
                );
            }
        }
    }

    /// Depth-first search over "enum variant field names enum" edges,
    /// starting from `start`, tracking the path in `visiting` (all inline,
    /// non-indirected embeddings). Returns the id of the enum where a cycle
    /// back to `start` was found, if any.
    fn enum_embeds_inline(&self, start: u32, visiting: &mut Vec<u32>) -> Option<u32> {
        let current = *visiting.last().unwrap();
        let variants = self.enums[current as usize].variants.clone();
        for variant in &variants {
            for field in &variant.associated {
                let embedded = match field.ty.base {
                    Base::Enum(embedded_id) => Some(embedded_id),
                    Base::EnumInstance(inst_id) => {
                        Some(self.enum_instances[inst_id as usize].enum_id)
                    }
                    // Every other type either carries no enum at all, or
                    // reaches one only through a heap pointer (`Object`,
                    // `Contract`, `Pointer<T>`, `Weak<T>`) — a real
                    // indirection that breaks the cycle, so it is not
                    // followed here.
                    _ => None,
                };
                let Some(embedded_id) = embedded else {
                    continue;
                };
                if embedded_id == start {
                    return Some(current);
                }
                if visiting.contains(&embedded_id) {
                    // A cycle among other enums that never reaches `start`
                    // — some other starting point in the outer loop will
                    // catch and report it from its own perspective.
                    continue;
                }
                visiting.push(embedded_id);
                if let Some(found) = self.enum_embeds_inline(start, visiting) {
                    return Some(found);
                }
                visiting.pop();
            }
        }
        None
    }

    /// The declarations ordered so a class always follows its base, and
    /// follows any `abstract class` it `implements` too.
    ///
    /// The latter is what lets [`Self::declare_class_members`] seed an
    /// adopter's flattened method list from the abstract class's own (the
    /// same way it already seeds from `base`): that seeding reads
    /// `self.classes[abstract_id].methods`, which is only populated once
    /// *that* class has had its own turn through
    /// [`Self::declare_class_members`]. Ignoring `implements` here — as a
    /// version of this ordering that only looked at `base` did — would seed
    /// from whatever the abstract class's methods happened to be zero-valued
    /// at (the pre-declaration default) whenever the adopter is declared
    /// earlier in the file than the abstract class it implements.
    fn in_hierarchy_order<'p>(&mut self, program: &'p Program) -> Vec<&'p ClassDecl> {
        let mut ordered: Vec<&ClassDecl> = Vec::new();
        let mut pending: Vec<&ClassDecl> = program.classes.iter().collect();

        // The chain is acyclic by now, so every round places at least one
        // class and the loop terminates.
        while !pending.is_empty() {
            let mut placed = Vec::new();
            pending.retain(|decl| {
                let base_ready = match self
                    .class_id(&decl.name.name)
                    .map(|id| self.classes[id as usize].base)
                {
                    Some(Some(base)) => ordered
                        .iter()
                        .any(|d| self.class_id(&d.name.name) == Some(base)),
                    _ => true,
                };
                let implements_ready = decl.implements.iter().all(|named| {
                    let resolved = self.resolved_name(&named.name, named.span);
                    match self
                        .classes
                        .iter()
                        .position(|c| c.name == resolved && c.kind == ClassKind::Abstract)
                        .map(|i| i as u32)
                    {
                        Some(abstract_id) => ordered
                            .iter()
                            .any(|d| self.class_id(&d.name.name) == Some(abstract_id)),
                        None => true,
                    }
                });
                let ready = base_ready && implements_ready;
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
                // layout. A value typed through it dispatches dynamically
                // through its adopter, the same mechanism the native
                // `Throwable` hierarchy already uses: every method declared
                // here is unconditionally `overridden` (below), and every
                // adopter seeds its own flattened method list from this
                // class's (below too), so `CallVirtual` has a shared index to
                // read regardless of which concrete adopter is behind it.
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

        // Seeded here too, ahead of `check_conformance` (which resolves
        // `implements` generally, too late for this): a class implementing
        // an `abstract class` — the compiler-known `Error`/`Throwable`/
        // `RuntimeError` (roadmap Phase 4b) included, since those are
        // `ClassKind::Abstract` too and this subsumes their old
        // three-id-only special case — needs its required methods to land
        // at the *same* indices the abstract class itself uses, or the
        // `overridden: true` virtual dispatch through a value statically
        // typed as the abstract class (`catch Throwable(e)`, `e.message()`;
        // a user `Shape`'s `e.area()`) calls whatever happens to sit at that
        // index in an unrelated class instead.
        //
        // Scoped to a single abstract class per adopter — the first one
        // found while `methods` is still empty, mirroring how the old
        // three-id-only version already behaved when a class named more
        // than one of them — since reconciling *several* abstract classes'
        // method lists into one adopter's table is a conflict/diamond
        // resolution problem this pass does not build (`design.md`'s
        // explicitly out-of-scope "multiple abstract-class inheritance").
        // `Self::in_hierarchy_order` guarantees the abstract class named
        // here has already had its own turn through this function, so its
        // `methods` below is the real flattened list, not the pre-
        // declaration default.
        for named in &decl.implements {
            let resolved = self.resolved_name(&named.name, named.span);
            let Some(id) = self
                .classes
                .iter()
                .position(|c| c.name == resolved && c.kind == ClassKind::Abstract)
                .map(|i| i as u32)
            else {
                continue;
            };
            if methods.is_empty() {
                methods = self.classes[id as usize].methods.clone();
            }
        }

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
            let returns = self.resolve_type(&method.return_type);
            let throws = self.resolve_throws_clause(&method.throws);
            let resolved = MethodInfo {
                name: method.name.name.clone(),
                params: method
                    .params
                    .iter()
                    .map(|p| self.resolve_param(p))
                    .collect(),
                returns,
                visibility: method.visibility,
                span: method.name.span,
                index: 0,
                owner: id,
                // An abstract class's own method has no body of its own
                // (rejected above if it tried to write one): every adopter
                // necessarily supplies its own override, so a call through a
                // value statically typed as the abstract class always needs
                // `CallVirtual`, the same way the native `Throwable`
                // hierarchy's own methods are unconditionally `overridden:
                // true` at registration (`checker.rs`'s
                // `register_native_exception_hierarchy`).
                overridden: decl.kind == ClassKind::Abstract,
                from_contract: None,
                throws,
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

    /// Whether `class` is `target` itself, or (transitively) adopts it
    /// through `implements`/its own `extends` chain (roadmap Phase 4b).
    ///
    /// Unlike [`Self::is_subclass_of`]'s abstract-class arm — which checks
    /// only the *direct* `abstract_bases` entry, sound for an ordinary
    /// `expect_assignable` call because [`Self::check_conformance`] already
    /// flattens a class's own base's `abstract_bases` into it — this walks
    /// every entry recursively. It has to: `class Foo implements
    /// RuntimeError` gives `Foo` only `[runtime_error]` in `abstract_bases`
    /// (`check_conformance` pushes the named id directly, not that id's own
    /// `abstract_bases`), so a direct-only check would miss that `Foo` is
    /// also a `Throwable` — exactly the case `catch Throwable(e)` "catches
    /// every recoverable throwable" depends on.
    fn implements_abstract_class(&self, class: u32, target: u32) -> bool {
        if class == target {
            return true;
        }
        if let Some(base) = self.classes[class as usize].base
            && self.implements_abstract_class(base, target)
        {
            return true;
        }
        self.classes[class as usize]
            .abstract_bases
            .iter()
            .any(|&b| self.implements_abstract_class(b, target))
    }

    /// Whether `ty` is a value that may be thrown or caught (roadmap Phase
    /// 4b) — a non-nullable class that is, or (transitively) implements,
    /// the compiler-known `Throwable`. A value that may be absent is
    /// rejected the same way a `to_string()` receiver is: there is no
    /// meaningful "absent exception" to throw or catch.
    fn is_throwable_type(&self, ty: Type) -> bool {
        if ty.nullable {
            return false;
        }
        let Base::Class(id) = ty.base else {
            return false;
        };
        let Some(native) = self.native_exceptions else {
            return false;
        };
        self.implements_abstract_class(id, native.throwable)
    }

    /// Resolves a `throws Type (| Type)*` clause into its individual
    /// alternatives (roadmap Phase 4b), reporting each one that does not
    /// name a `Throwable`.
    ///
    /// Deliberately not [`Self::resolve_type`]/[`Self::resolve_union`]: that
    /// path interns a real union *value* type, and reports `NOT_LOWERED`
    /// the first time a given normalized union is interned (unions are not
    /// lowered yet, roadmap task 11.x) — exactly wrong here, where `A | B`
    /// after `throws` names a set of alternative exception types the
    /// program never holds a value of at once, not a union-typed value.
    fn resolve_throws_clause(&mut self, throws: &Option<TypeRef>) -> Vec<Type> {
        let Some(reference) = throws else {
            return Vec::new();
        };

        let atoms = std::iter::once(reference).chain(reference.union_with.iter());
        let mut resolved = Vec::new();
        for atom in atoms {
            let ty = self.resolve_type_atom(atom);
            if ty.is_unknown() {
                continue;
            }
            if !self.is_throwable_type(ty) {
                let name = self.name(ty);
                self.error(
                    codes::TYPE_MISMATCH,
                    atom.span,
                    format!("`{name}` cannot be thrown"),
                    "`throws` names a class that implements `Throwable`",
                    Some("implement `Throwable`, or throw a type that already does".into()),
                );
                continue;
            }
            resolved.push(ty);
        }
        resolved
    }

    /// Whether a `try`'s `catch` clauses, taken together, cover `thrown`
    /// (roadmap Phase 4b) — a later catch than the one that already
    /// handles it is unreachable code, not a second chance.
    fn throw_is_covered(&self, thrown: Type, catches: &[Type]) -> bool {
        let Base::Class(thrown_id) = thrown.base else {
            return false;
        };
        catches.iter().any(|&caught| {
            let Base::Class(caught_id) = caught.base else {
                return false;
            };
            self.implements_abstract_class(thrown_id, caught_id)
        })
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
            let outer_pending = std::mem::take(&mut self.pending_throws);
            self.current_throws = Vec::new();
            let ctor_span = constructor.body.span;
            self.check_member_body(class_type, &constructor.params, Type::VOID, |checker| {
                checker.check_block(&constructor.body);
            });
            self.report_uncaught_throws(ctor_span, "construct");
            self.pending_throws = outer_pending;
            self.in_constructor = false;
            self.require_fields_initialized(decl, constructor);
        }

        for method in &decl.methods {
            let Some(body) = &method.body else { continue };
            self.enter_type_params(&method.type_params);
            let returns = self.resolve_type(&method.return_type);
            let throws = self.resolve_throws_clause(&method.throws);
            let name = method.name.name.clone();
            let span = body.span;
            let outer_pending = std::mem::take(&mut self.pending_throws);
            self.current_throws = throws;
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
            self.report_uncaught_throws(span, &method.name.name);
            self.pending_throws = outer_pending;
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

    /// Registers an enum's name, arity and type parameters immediately,
    /// ahead of any variant's associated field type resolution — mirroring
    /// [`Self::register_class`] for exactly the same reason: a variant's
    /// associated field may name this enum itself (`Cons(tail: IntList)`)
    /// or one declared later in the file, and resolving that needs the
    /// enum's own arity known first. [`Self::declare_enum_variants`], run
    /// later once every class and enum in the file is registered, resolves
    /// the actual associated field types and populates this placeholder's
    /// `variants`.
    fn register_enum(&mut self, decl: &EnumDecl) {
        if matches!(decl.name.name.as_str(), "Iteration" | "Result") {
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("`{}` is an enum of the language", decl.name.name),
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

        // Unlike a function's or method's, an enum's own type parameters
        // (fase-3-generic-enums) are not gated `NOT_LOWERED`: `enter_type_params`
        // reports that unconditionally on first use, which is right for a
        // generic function (truly unimplemented) but wrong here, since a
        // generic enum's `T` lowers by direct substitution the same way a
        // class's own does — a class avoids the same blanket gate by minting
        // its type parameter ids through `Self::mint_type_param_ids` instead
        // (see `Self::register_class`), so an enum does the same here rather
        // than going through `enter_type_params`.
        let type_params = self.mint_type_param_ids(&decl.type_params);

        self.enums.push(EnumType {
            name: decl.name.name.clone(),
            variants: Vec::new(),
            type_params,
            shared: decl.shared,
            span: decl.name.span,
        });
    }

    /// Resolves an enum's variants' associated field types and its own type
    /// parameters' constraints, and populates the placeholder
    /// [`Self::register_enum`] already pushed into `self.enums`.
    ///
    /// Run once every class and enum in the file is registered
    /// (`Self::declare_members`), so a field may name this enum's own type —
    /// including a self-instantiation of its own generic parameters — or
    /// any class or enum regardless of declaration order.
    fn declare_enum_variants(&mut self, decl: &EnumDecl) {
        let Some(id) = self.enums.iter().position(|e| e.name == decl.name.name) else {
            // Registration itself failed (duplicate name, or a reserved
            // native name) and already reported its own diagnostic there;
            // nothing to populate.
            return;
        };
        let type_params = self.enums[id].type_params.clone();

        self.type_param_scope.push(
            decl.type_params
                .iter()
                .zip(&type_params)
                .map(|(p, &tid)| (p.name.name.clone(), tid))
                .collect(),
        );
        self.report_declared_variance(&decl.type_params);
        for (p, &tid) in decl.type_params.iter().zip(&type_params) {
            let constraints: Vec<Type> =
                p.constraints.iter().map(|c| self.resolve_type(c)).collect();
            self.type_params[tid as usize].constraints = constraints;
        }

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
        // language already spells that `Never`; here it is a typo, and
        // accepting it would give the concept a second spelling.
        if variants.is_empty() {
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("enum `{}` has no variants", decl.name.name),
                "a type with no values can never be constructed",
                Some("write `Never` instead of an empty enum".into()),
            );
        }

        self.leave_type_params();

        self.enums[id].variants = variants;
    }

    fn declare_function(&mut self, f: &FnDecl) {
        let type_params = self.enter_type_params(&f.type_params);
        let params = f
            .params
            .iter()
            .map(|p| self.resolve_param(p))
            .collect::<Vec<_>>();

        let returns = self.resolve_type(&f.return_type);
        let throws = self.resolve_throws_clause(&f.throws);
        let signature = Signature {
            name: f.name.name.clone(),
            params,
            returns,
            type_params,
            shared: f.shared,
            span: f.name.span,
            throws,
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

    /// `extern "C" fn name(params): ReturnType;` (roadmap Phase 4e,
    /// `ADR-015`): resolves the signature and rejects any parameter or
    /// return type without a stable C-ABI layout (task 4.5, design D2) —
    /// return additionally allows `Void`, which [`is_ffi_safe`] itself
    /// deliberately excludes (task 4.3).
    fn declare_extern_fn(&mut self, f: &ExternFnDecl) {
        let params: Vec<Type> = f
            .params
            .iter()
            .map(|p| {
                let ty = self.resolve_type(&p.ty);
                if !ty.is_unknown() && !is_ffi_safe(ty, &self.pointer_types) {
                    let name = self.name(ty);
                    self.error(
                        codes::NOT_FFI_SAFE,
                        p.ty.span,
                        format!("`{name}` cannot appear in an `extern \"C\" fn` signature"),
                        "only Boolean, fixed-width integers, Float32/64, and Pointer<T> have a stable C-ABI layout",
                        Some("pass an ABI-stable type, or marshal it manually through Pointer<Byte>".into()),
                    );
                }
                ty
            })
            .collect();

        let returns = self.resolve_type(&f.return_type);
        if !returns.is_unknown()
            && returns != Type::VOID
            && !is_ffi_safe(returns, &self.pointer_types)
        {
            let name = self.name(returns);
            self.error(
                codes::NOT_FFI_SAFE,
                f.return_type.span,
                format!("`{name}` cannot be an `extern \"C\" fn`'s return type"),
                "only Void, Boolean, fixed-width integers, Float32/64, and Pointer<T> have a stable C-ABI layout",
                None,
            );
        }

        if self.functions.contains_key(&f.name.name) || self.externs.contains_key(&f.name.name) {
            self.error(
                codes::DUPLICATE_FUNCTION,
                f.name.span,
                format!("`{}` is already defined", f.name.name),
                "there is no overloading, and an extern declaration shares the same namespace as an ordinary function",
                None,
            );
            return;
        }

        self.externs
            .insert(f.name.name.clone(), ExternSignature { params, returns });
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
                // A union is meant to drop `Never` entirely, not just sort
                // it (`ZIRK_LANGUAGE_SPEC.md` section 7: "removes
                // duplicates, `Never`, and alternatives subsumed by a
                // supertype") — union normalization does not implement that
                // rule yet, so this key exists only so the match stays
                // exhaustive, not because a `Never` alternative surviving
                // into a union is expected.
                Base::Never => (9, 0),
                Base::Enum(id) => (10, id),
                Base::Function(id) => (11, id),
                Base::Contract(id) => (12, id),
                Base::Class(id) => (13, id),
                Base::Param(id) => (14, id),
                Base::Instance(id) => (15, id),
                Base::ContractInstance(id) => (16, id),
                Base::EnumInstance(id) => (17, id),
                Base::Union(id) => (18, id),
                Base::Pointer(id) => (19, id),
                Base::Weak(id) => (20, id),
                Base::NativeSlice(id) => (21, id),
                Base::NativeSliceMut(id) => (22, id),
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

    /// `Fn(P...) => R` / `Function(P...) => R` (roadmap Phase 4d): resolves
    /// each written parameter and the result, then interns the shape
    /// *structurally* (`Self::intern_fn_type`) — the same table a named
    /// function reference or a capture-less lambda's own type lands in
    /// (`Self::check_path`, `Self::check_lambda`), so `Fn(Int32) => Int32`
    /// written twice, or written once and matched by a same-shaped named
    /// function, is one id, not two merely-equal ones (design D12).
    ///
    /// Labels/`?`/`...` are parsed (`zirk-parser`'s `parse_fn_type_param`)
    /// but not carried into the interned [`FnType`]: invoking a value of
    /// this type is always positional (`Self::check_call`'s closure-call
    /// path already rejects named arguments for any closure), so a label
    /// here documents intent without being load-bearing the way a declared
    /// function's own parameter name is.
    fn resolve_fn_type_ref(&mut self, function: &FnTypeRef, nullable: bool) -> Type {
        let params: Vec<Type> = function
            .params
            .iter()
            .map(|p| self.resolve_type(&p.ty))
            .collect();
        let returns = self.resolve_type(&function.returns);
        let id = self.intern_fn_type(FnType { params, returns });
        let ty = Type::of(Base::Function(id));
        if nullable { ty.as_nullable() } else { ty }
    }

    /// `Pointer<T>` (roadmap Phase 4e, design D1/D2): resolves `T` and
    /// rejects it when it has no stable C-ABI layout, naming `T` itself
    /// (spec scenario "Disallowed element type") — checked here so every
    /// position `Pointer<T>` can be written in (annotation, `Pointer.from`,
    /// `.cast<U>()`) shares the one check.
    fn resolve_pointer_type_ref(&mut self, reference: &TypeRef) -> Type {
        if reference.arguments.len() != 1 {
            self.error(
                codes::UNKNOWN_TYPE,
                reference.span,
                "`Pointer<T>` takes exactly one type argument",
                format!("found {} type argument(s)", reference.arguments.len()),
                Some("write `Pointer<T>` naming the pointee type".into()),
            );
            return Type::UNKNOWN;
        }

        let pointee = self.resolve_type(&reference.arguments[0]);
        if !pointee.is_unknown() && !is_ffi_safe(pointee, &self.pointer_types) {
            let name = self.name(pointee);
            self.error(
                codes::NOT_FFI_SAFE,
                reference.arguments[0].span,
                format!("`{name}` has no stable C-ABI layout"),
                "Pointer<T> only allows Boolean, fixed-width integers, Float32/64, and another Pointer<U>",
                Some("use an ABI-stable element type".into()),
            );
        }

        let id = self.intern_pointer_type(pointee);
        let ty = Type::of(Base::Pointer(id));
        if reference.nullable {
            ty.as_nullable()
        } else {
            ty
        }
    }

    /// Interns a `Pointer<T>` pointee type, returning the id its
    /// [`Base::Pointer`] carries.
    fn intern_pointer_type(&mut self, pointee: Type) -> u32 {
        if let Some(index) = self.pointer_types.iter().position(|&t| t == pointee) {
            return index as u32;
        }
        self.pointer_types.push(pointee);
        (self.pointer_types.len() - 1) as u32
    }

    /// `Weak<T>` (roadmap Phase 4e, `fase-4e-weak`, design D1): resolves `T`
    /// and rejects it when it is not a reference type (spec scenario
    /// "Disallowed value-type referent") — a value type has no identity for
    /// a weak reference to observe independently of its content. Checked
    /// here so every position `Weak<T>` can be written in (annotation,
    /// `Weak.from`) shares the one check, mirroring
    /// [`Self::resolve_pointer_type_ref`].
    fn resolve_weak_type_ref(&mut self, reference: &TypeRef) -> Type {
        if reference.arguments.len() != 1 {
            self.error(
                codes::UNKNOWN_TYPE,
                reference.span,
                "`Weak<T>` takes exactly one type argument",
                format!("found {} type argument(s)", reference.arguments.len()),
                Some("write `Weak<T>` naming the referent type".into()),
            );
            return Type::UNKNOWN;
        }

        let referent = self.resolve_type(&reference.arguments[0]);
        if !referent.is_unknown() && !self.is_reference_type(referent) {
            let name = self.name(referent);
            self.error(
                codes::WEAK_DISALLOWED_REFERENT,
                reference.arguments[0].span,
                format!("`{name}` is not a reference type"),
                "Weak<T> only allows a class or contract instance, since a value type has no identity to observe weakly",
                Some("use a class or contract type as Weak<T>'s referent".into()),
            );
        }

        let id = self.intern_weak_type(referent);
        let ty = Type::of(Base::Weak(id));
        if reference.nullable {
            ty.as_nullable()
        } else {
            ty
        }
    }

    /// Interns a `Weak<T>` referent type, returning the id its
    /// [`Base::Weak`] carries.
    fn intern_weak_type(&mut self, referent: Type) -> u32 {
        if let Some(index) = self.weak_types.iter().position(|&t| t == referent) {
            return index as u32;
        }
        self.weak_types.push(referent);
        (self.weak_types.len() - 1) as u32
    }

    /// `NativeSlice<T>`/`NativeSliceMut<T>` (roadmap Phase 4e,
    /// `fase-4e-native-slice`, design D1): resolves `T` and rejects it when
    /// it has no stable C-ABI layout (spec scenario "NativeSlice element
    /// type is ABI-safe only") — the exact same predicate
    /// `Self::resolve_pointer_type_ref` already applies to `Pointer<T>`
    /// (task 1.1's own instruction: reuse the actual current list, not the
    /// design's restated summary of it). `mutable` selects
    /// `Base::NativeSliceMut`/`native_slice_mut_types` over
    /// `Base::NativeSlice`/`native_slice_types`.
    fn resolve_native_slice_type_ref(&mut self, reference: &TypeRef, mutable: bool) -> Type {
        let type_name = if mutable {
            "NativeSliceMut<T>"
        } else {
            "NativeSlice<T>"
        };
        if reference.arguments.len() != 1 {
            self.error(
                codes::UNKNOWN_TYPE,
                reference.span,
                format!("`{type_name}` takes exactly one type argument"),
                format!("found {} type argument(s)", reference.arguments.len()),
                Some(format!("write `{type_name}` naming the element type")),
            );
            return Type::UNKNOWN;
        }

        let element = self.resolve_type(&reference.arguments[0]);
        if !element.is_unknown() && !is_ffi_safe(element, &self.pointer_types) {
            let name = self.name(element);
            self.error(
                codes::NOT_FFI_SAFE,
                reference.arguments[0].span,
                format!("`{name}` has no stable C-ABI layout"),
                "NativeSlice<T>/NativeSliceMut<T> only allow Boolean, fixed-width integers, Float32/64, and Pointer<U>, the same element restriction Pointer<T> has",
                Some("use an ABI-stable element type".into()),
            );
        }

        let ty = if mutable {
            let id = self.intern_native_slice_mut_type(element);
            Type::of(Base::NativeSliceMut(id))
        } else {
            let id = self.intern_native_slice_type(element);
            Type::of(Base::NativeSlice(id))
        };
        if reference.nullable {
            ty.as_nullable()
        } else {
            ty
        }
    }

    /// Interns a `NativeSlice<T>` element type, returning the id its
    /// [`Base::NativeSlice`] carries.
    fn intern_native_slice_type(&mut self, element: Type) -> u32 {
        if let Some(index) = self.native_slice_types.iter().position(|&t| t == element) {
            return index as u32;
        }
        self.native_slice_types.push(element);
        (self.native_slice_types.len() - 1) as u32
    }

    /// Interns a `NativeSliceMut<T>` element type, returning the id its
    /// [`Base::NativeSliceMut`] carries.
    fn intern_native_slice_mut_type(&mut self, element: Type) -> u32 {
        if let Some(index) = self
            .native_slice_mut_types
            .iter()
            .position(|&t| t == element)
        {
            return index as u32;
        }
        self.native_slice_mut_types.push(element);
        (self.native_slice_mut_types.len() - 1) as u32
    }

    /// One alternative of a type reference on its own — never a union; see
    /// [`Self::resolve_type`] for that.
    fn resolve_type_atom(&mut self, reference: &TypeRef) -> Type {
        if let Some(function) = &reference.function {
            return self.resolve_fn_type_ref(function, reference.nullable);
        }
        if reference.name == "Pointer" {
            return self.resolve_pointer_type_ref(reference);
        }
        if reference.name == "Weak" {
            return self.resolve_weak_type_ref(reference);
        }
        if reference.name == "NativeSlice" {
            return self.resolve_native_slice_type_ref(reference, false);
        }
        if reference.name == "NativeSliceMut" {
            return self.resolve_native_slice_type_ref(reference, true);
        }

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
    ///
    /// `Clone` (roadmap Phase 4e, `fase-4e-clone`, design D1/D4) is special:
    /// unlike an ordinary contract, satisfying it is never decided by an
    /// explicit `implements Clone` alone (`is_subclass_of`'s
    /// `Base::Contract` arm, which only checks the class's own
    /// `contracts` list) — it is decided structurally, by whether `arg`'s
    /// whole field graph is itself `Clone` (`Self::is_clone_type`), the
    /// same derivation `.clone()` itself requires. This is what makes
    /// `T from Clone` (the "Generic projection needs Clone" scenario) work
    /// without every `Clone`-eligible class needing to spell out
    /// `implements Clone` by hand.
    fn satisfies_constraint(&mut self, arg: Type, constraint: Type) -> bool {
        if let Some(clone_id) = self.native_clone
            && constraint == Type::of(Base::Contract(clone_id))
        {
            let mut in_progress = Vec::new();
            return self.is_clone_type(arg, &mut in_progress);
        }
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

    /// `Result<value, NativeError>` (roadmap Phase 4e,
    /// `fase-4e-native-slice`, task 1.2): the same compiler-known `Result`
    /// enum `register_native_result_enum` mints, instantiated with the
    /// caller's own success type and the compiler's `NativeError` class.
    fn native_result_type(&mut self, value: Type) -> Type {
        let native_result = self
            .native_result
            .expect("register_native_result_enum runs before any type is checked");
        let native_error = self
            .native_exceptions
            .expect("register_native_exception_hierarchy runs before any type is checked")
            .native_error;
        let id = self.intern_enum_instance(GenericEnumInstance {
            enum_id: native_result,
            args: vec![value, Type::of(Base::Class(native_error))],
        });
        Type::of(Base::EnumInstance(id))
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

        // `Iteration<T>` and `Result<T,E>` were the first generic enum
        // instantiations verified to lower: a dedicated specialization pass
        // (`specialize_enum`, `zirk-ir`) builds one concrete `EnumLayout` per
        // instantiation the program actually names, the same way a generic
        // class's does (roadmap task 13.5/11.1). That pass runs generically
        // over every enum instantiation `checked.enum_instances` records, so
        // a user-declared generic enum lowers the same way (fase-3-generic-
        // enums) — there is nothing native-specific left to gate here.
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

    /// The span of the lambda literal that owns `fn_type`, when it is a
    /// *capturing* lambda's own id (design D14). `None` for a named
    /// function's interned id, a capture-less lambda's interned id (both
    /// freely interchangeable with any compatible shape, D12), or an id no
    /// lambda ever produced.
    fn capturing_lambda_span(&self, fn_type: u32) -> Option<Span> {
        self.lambdas
            .iter()
            .find(|(_, info)| info.fn_type == fn_type && !info.captures.is_empty())
            .map(|(span, _)| *span)
    }

    /// Whether `expr` is a lambda literal that captures something — the one
    /// shape design D14 lets satisfy a `Fn(...) => R` position at all, and
    /// only when written directly at that position (not through a variable
    /// that merely holds one).
    fn capturing_literal_at(&self, expr: &Expr) -> bool {
        let Expr::Lambda(lambda) = expr else {
            return false;
        };
        self.lambdas
            .get(&lambda.span)
            .is_some_and(|info| !info.captures.is_empty())
    }

    /// Whether `actual`'s callable shape may stand in for `expected`'s:
    /// same arity, parameters contravariant, result covariant
    /// (`zirk-callables` spec) — structural compatibility, not necessarily
    /// the same interned id (interning only merges *identical* shapes;
    /// D12's whole point is that a merely-*compatible* shape also
    /// qualifies).
    fn fn_shapes_compatible(&self, expected_id: u32, actual_id: u32) -> bool {
        if expected_id == actual_id {
            return true;
        }
        let expected = &self.fn_types[expected_id as usize];
        let actual = &self.fn_types[actual_id as usize];
        if expected.params.len() != actual.params.len() {
            return false;
        }
        expected
            .params
            .iter()
            .zip(&actual.params)
            .all(|(e, a)| a.accepts(*e))
            && expected.returns.accepts(actual.returns)
    }

    /// [`Self::fn_shapes_compatible`], applied to two full [`Type`]s rather
    /// than bare ids — also checks nullability the ordinary way.
    fn callable_assignable(&self, expected: Type, actual: Type) -> bool {
        let Base::Function(expected_id) = expected.base else {
            return false;
        };
        let Base::Function(actual_id) = actual.base else {
            return false;
        };
        if actual.nullable && !expected.nullable {
            return false;
        }
        self.fn_shapes_compatible(expected_id, actual_id)
    }

    /// Whether `actual` — already known to be a capturing closure literal
    /// written directly at this position (`Self::capturing_literal_at`) —
    /// may be adopted as the position's own static type (design D14).
    ///
    /// `expected` must not already be locked to a *different* capturing
    /// literal: once `Self::check_let`/`Self::check_assign` adopt one
    /// literal's own id here, the position's storage is sized for that
    /// literal's `ClosureLayout` alone, so a second, textually different
    /// literal — even of a compatible shape — cannot land here too (that
    /// would need the captures boxed behind a uniform representation, D13).
    fn accepts_capturing_literal(&self, expected: Type, actual: Type) -> bool {
        let Base::Function(expected_id) = expected.base else {
            return false;
        };
        if self.capturing_lambda_span(expected_id).is_some() {
            return false;
        }
        self.callable_assignable(expected, actual)
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
        self.current_throws = signature
            .as_ref()
            .map(|s| s.throws.clone())
            .unwrap_or_default();
        let outer_pending = std::mem::take(&mut self.pending_throws);

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

        // `unsafe fn` runs its whole body as if wrapped in `unsafe {}`
        // (roadmap Phase 4e) — no separate journal here: an `unsafe fn`'s
        // caller is the one who decides whether its own enclosing `unsafe`
        // block's journal covers the call, so this only opens the context a
        // pointer operation/extern call inside the body checks against.
        if f.is_unsafe {
            self.unsafe_depth += 1;
        }
        let always_returns = self.check_block(&f.body);
        if f.is_unsafe {
            self.unsafe_depth -= 1;
        }
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

        // D14: a `return` of a capturing closure literal narrows
        // `current_return` to that literal's own id (`Self::check_return`) —
        // the function's *signature*, which every caller reads, must learn
        // the same narrowed id, or a caller checked before this point would
        // still see the generic (capture-less-shaped) written annotation
        // and accept a value whose actual layout does not match it.
        //
        // This is sound for every caller checked *after* this function in
        // `Self::run`'s declaration-order body pass; one checked *before* it
        // (calling ahead to a function whose body has not run yet) still
        // sees the pre-narrowed signature — a known limitation of doing this
        // structurally, in one pass, without real interprocedural
        // dataflow (see `design.md`'s D14 addendum).
        if let Some(mut updated) = self.functions.get(&f.name.name).cloned()
            && updated.returns != self.current_return
        {
            updated.returns = self.current_return;
            self.functions.insert(f.name.name.clone(), updated);
        }

        self.report_uncaught_throws(f.body.span, &f.name.name);
        self.pending_throws = outer_pending;
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
            Stmt::MultiLet(s) => {
                self.check_multi_let(s);
                false
            }
            Stmt::Assign(s) => {
                self.check_assign(s);
                false
            }
            Stmt::MultiAssign(s) => {
                self.check_multi_assign(s);
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
                let ty = self.check_expr(&s.expr);
                self.require_result_consumed(ty, s.span);
                false
            }
            Stmt::Block(b) => self.check_block(b),
            Stmt::Throw(s) => {
                self.check_throw(s);
                // A `throw` diverges the same way `return` does: nothing
                // after it in this block runs.
                true
            }
            Stmt::Try(s) => self.check_try(s),
            Stmt::Unsafe(s) => self.check_unsafe_block(s),
            Stmt::Commit(s) => self.check_commit_block(s),
        }
    }

    /// `unsafe { ... }` (roadmap Phase 4e, design D3): opens the context a
    /// pointer operation or `commit {}` inside the block checks against.
    fn check_unsafe_block(&mut self, stmt: &UnsafeBlock) -> bool {
        self.unsafe_depth += 1;
        let returns = self.check_block(&stmt.body);
        self.unsafe_depth -= 1;
        returns
    }

    /// `commit { ... }` (roadmap Phase 4e, design D3): requires an enclosing
    /// `unsafe {}`, checked before opening `commit`'s own context — mirrors
    /// `Self::check_loop`'s pattern of validating context before entering a
    /// nested checked region.
    fn check_commit_block(&mut self, stmt: &CommitBlock) -> bool {
        if self.unsafe_depth == 0 {
            self.error(
                codes::COMMIT_OUTSIDE_UNSAFE,
                stmt.span,
                "`commit {}` outside `unsafe`",
                "a commit boundary only makes sense inside a reversible unsafe transaction",
                Some("wrap this in `unsafe { commit { ... } }`".into()),
            );
        }
        self.commit_depth += 1;
        let returns = self.check_block(&stmt.body);
        self.commit_depth -= 1;
        returns
    }

    fn check_let(&mut self, stmt: &LetStmt) {
        let annotated = stmt.ty.as_ref().map(|t| self.resolve_type(t));

        // A recursive lambda calling itself by its own binding name needs
        // that name in scope *while its own body is checked*
        // (`ZIRK_LANGUAGE_SPEC.md` section 6: "recursive lambdas require an
        // explicit binding type", roadmap Phase 4d) — impossible for an
        // ordinary initializer (`Self::declare_local` below runs only
        // *after* the initializer is checked, so `x` in `mut x = x + 1;`
        // reads the outer one, not itself). Pre-declared here, and only for
        // exactly this shape — an explicitly `Fn(...) => R`-typed local
        // whose initializer is a lambda literal — so an ordinary `mut x: T
        // = x;` still reads the outer `x` unchanged.
        let self_recursive = matches!(
            annotated,
            Some(Type {
                base: Base::Function(_),
                ..
            })
        ) && matches!(stmt.init.as_ref(), Some(Expr::Lambda(_)));

        if self_recursive {
            self.declare_local(Binding {
                name: stmt.name.name.clone(),
                ty: annotated.expect("checked by `self_recursive` above"),
                mutability: stmt.mutability,
                span: stmt.name.span,
                initialized: true,
            });
        }

        // An explicit annotation is the expected type of the initializer —
        // what lets `mut r: Result<Int32,String> = Result.Ok(5);` infer `E`
        // from context instead of only from `Ok`'s own argument (roadmap
        // Phase 4a, `expected_type`'s own doc comment).
        let outer_recursive_binding = self.recursive_binding.take();
        self.recursive_binding = self_recursive.then(|| stmt.name.name.clone());
        let initializer = stmt.init.as_ref().map(|e| {
            self.expected_type = annotated;
            self.check_expr(e)
        });
        self.recursive_binding = outer_recursive_binding;

        let ty = match (annotated, initializer) {
            (Some(declared), Some(actual)) => {
                let span = stmt.init.as_ref().map(|e| e.span()).unwrap_or(stmt.span);

                // D14: a capturing closure literal written directly as this
                // local's initializer becomes the local's own static type —
                // its exact `Base::Function` id — rather than the written
                // annotation's: the storage this local's slot gets is sized
                // for that one literal's `ClosureLayout`, so every later use
                // of the local (a call, a reassignment, a read) is checked
                // against it. `Self::expect_assignable`'s own generic
                // callable-vs-callable path never accepts a capturing
                // closure, so this is checked ahead of it, not through it.
                let literal_captures = stmt
                    .init
                    .as_ref()
                    .is_some_and(|e| self.capturing_literal_at(e));
                if literal_captures && self.accepts_capturing_literal(declared, actual) {
                    actual
                } else {
                    self.expect_assignable(declared, actual, span, "the initial value");
                    declared
                }
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

        if self_recursive {
            // Already declared above (so the lambda body could call itself)
            // — only its final type might still need updating, in case D14
            // adopted the literal's own id; declaring it a second time
            // would report `ORDINARY_SHADOWING` against the entry this is
            // finishing.
            self.scopes.retype(&stmt.name.name, ty);
        } else {
            self.declare_local(Binding {
                name: stmt.name.name.clone(),
                ty,
                mutability: stmt.mutability,
                span: stmt.name.span,
                initialized: stmt.init.is_some(),
            });
        }
    }

    /// `mut first, second: String;` (roadmap Phase 4d) — the comma-grouped
    /// form of `Self::check_let`. Applies the shared type and permission to
    /// every name independently (design D1), and does not support the
    /// self-recursive-lambda pre-declaration `Self::check_let` does: that
    /// shape is single-name only.
    fn check_multi_let(&mut self, stmt: &MultiLetStmt) {
        let annotated = stmt.ty.as_ref().map(|t| self.resolve_type(t));

        if !stmt.inits.is_empty() && stmt.inits.len() != stmt.names.len() {
            self.error(
                codes::MULTI_LET_ARITY_MISMATCH,
                stmt.span,
                format!(
                    "{} name(s) declared but {} initializer(s) given",
                    stmt.names.len(),
                    stmt.inits.len()
                ),
                "a comma-grouped declaration with an initializer list needs exactly one value per name",
                Some("add or remove initializer expressions until the counts match".into()),
            );
        }

        let arity_matches = !stmt.inits.is_empty() && stmt.inits.len() == stmt.names.len();

        // Without an explicit annotation, the first initializer's type is
        // the shared type — the same inference `Self::check_let` applies to
        // its own single name. A mismatched arity has no sound per-name
        // pairing to infer from, so it falls back to `Unknown` and the arity
        // diagnostic above stands alone.
        let ty = match annotated {
            Some(t) => t,
            None if arity_matches => {
                self.expected_type = None;
                let inferred = self.check_expr(&stmt.inits[0]);
                if matches!(inferred.base, Base::Null) {
                    self.error(
                        codes::UNKNOWN_TYPE,
                        stmt.span,
                        "cannot infer the type of this declaration",
                        "`null` alone does not say which type is absent",
                        Some("write a shared `: Type` annotation".into()),
                    );
                    Type::UNKNOWN
                } else {
                    inferred
                }
            }
            None => Type::UNKNOWN,
        };

        if ty == Type::VOID {
            self.error(
                codes::VOID_VARIABLE,
                stmt.span,
                "a variable cannot be of type Void",
                "`Void` represents the absence of a value, so it cannot be stored",
                None,
            );
        }

        for (i, name) in stmt.names.iter().enumerate() {
            if arity_matches {
                // Position 0 was already checked above to infer `ty` when
                // there was no annotation — checking it again here would
                // double-report anything it found.
                if !(i == 0 && annotated.is_none()) {
                    self.expected_type = annotated;
                    let actual = self.check_expr(&stmt.inits[i]);
                    self.expect_assignable(ty, actual, stmt.inits[i].span(), "the initial value");
                }
                self.check_strict_alias(&stmt.inits[i], ty, stmt.mutability, &name.name, name.span);
            }

            self.declare_local(Binding {
                name: name.name.clone(),
                ty,
                mutability: stmt.mutability,
                span: name.span,
                // No initializer list (or an arity mismatch already
                // reported) still leaves every binding usable when its type
                // has a default (`ZIRK_TYPE_SYSTEM` "Default
                // initialization"), exactly as a comma-grouped declaration's
                // spec requires.
                initialized: arity_matches || ty.has_default(),
            });
        }
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

    /// Whether `id` `implements Resource<E>` for some `E` — the same test
    /// [`Self::check_resource_binding_type`] applies, factored out so
    /// [`Self::class_is_clone`] can reuse it (design D1:
    /// `zirk-resources`'s "`Resource` SHALL NOT imply `Clone`").
    fn class_implements_resource(&self, id: u32) -> bool {
        let Some(native_resource) = self.native_resource else {
            return false;
        };
        self.classes[id as usize]
            .contract_instances
            .iter()
            .any(|&inst| {
                self.contract_instances[inst as usize].contract == native_resource.resource
            })
    }

    /// Whether `ty` is `Clone` (roadmap Phase 4e, `fase-4e-clone`, design
    /// D1): a scalar carries no identity, so it is trivially independent
    /// already; a class is `Clone` iff every field of its declared shape is
    /// (recursively, [`Self::class_is_clone`]); an enum (algebraic or not)
    /// is `Clone` iff every variant's associated field types are, the same
    /// shape one level over. `Pointer<T>`, `Weak<T>`, a contract-typed
    /// member (its concrete implementor is not statically known, so it is
    /// not safe to assume all of them are `Clone`), a generic class
    /// instantiation, and a function/closure value are all conservatively
    /// not `Clone` — matching the spec's explicit rejection list
    /// (`Resource`/`Pointer<T>`/lock/`Task<T>`) plus this change's own
    /// scope limits for the members the spec does not name at all
    /// (`Weak<T>`'s clone semantics are genuinely undefined by the spec and
    /// out of this change's stated scope; a contract-typed field the same
    /// way `Instance`/`Function` already are for unrelated reasons
    /// elsewhere in this checker).
    ///
    /// `in_progress` is [`Self::class_is_clone`]'s own cycle guard, threaded
    /// through so a class reached again through one of its own fields (a
    /// self-referential or mutually-recursive graph, `design` risk "Recursive
    /// traversal") is treated as provisionally `Clone` rather than recursing
    /// forever — see that function's own doc comment for why this is sound.
    fn is_clone_type(&mut self, ty: Type, in_progress: &mut Vec<u32>) -> bool {
        if ty.nullable {
            return self.is_clone_type(ty.without_null(), in_progress);
        }
        match ty.base {
            Base::Void
            | Base::Int(_)
            | Base::Float(_)
            | Base::Char
            | Base::Boolean
            | Base::String
            | Base::Null => true,
            Base::Class(id) => self.class_is_clone(id, in_progress),
            // `Pointer<T>`, `Weak<T>`, a contract/contract-instance-typed
            // member, a generic class instantiation and a function/closure
            // value: see this function's own doc comment.
            //
            // An enum (`Base::Enum`/`Base::EnumInstance`), algebraic or not,
            // is conservatively excluded too — not for a structural reason
            // (a genuinely scalar-only enum's own field graph is trivially
            // `Clone`), but because of a runtime layout hazard this change's
            // own end-to-end tests found while exercising `Node?` (found
            // and fixed for `Nullable` specifically —
            // `crates/zirk-codegen-llvm/src/emit.rs`'s `InstKind::NullValue`
            // arm's own doc comment tells that story): `BuildEnum`'s own
            // codegen (`emit.rs`) only inserts the *active* variant's own
            // fields into the flattened enum struct, leaving every other
            // variant's own field slots undefined — but
            // `gc_field_offsets`/`gc_reference_paths` walks *every*
            // variant's fields unconditionally, regardless of which one is
            // active, the same class of hazard `NullValue`'s undef payload
            // was. Unlike `Nullable` (one small, fully-audited fix), fixing
            // this for every enum shape is a larger, separate surface this
            // change's own scope and test coverage do not reach — excluding
            // enum-typed fields from `Clone` derivation entirely is the
            // correctness-first choice until that is addressed on its own.
            _ => false,
        }
    }

    /// Whether class `id` derives `Clone` (design D1): every field of its
    /// declared shape (inherited fields included, since [`ClassType::fields`]
    /// already flattens the hierarchy) is itself `Clone`, and it does not
    /// `implement Resource<E>` (`zirk-resources`'s "`Resource` SHALL NOT
    /// imply `Clone`", enforced here regardless of whether every field
    /// would otherwise qualify).
    ///
    /// Memoized in [`Checker::clone_cache`], but **only once the call that
    /// is genuinely outermost for `id` (not one reached through a cycle)
    /// has fully unwound**: a class reached again through its own field
    /// graph returns a provisional `true` (`in_progress.contains(&id)`)
    /// without being cached, since that provisional answer is only correct
    /// if the *rest* of the cycle turns out `Clone` too — something not yet
    /// known partway through the recursion that produced it. Caching only
    /// at the true top level means a provisional answer is never persisted
    /// ahead of the fields that would actually disprove it; a nested call's
    /// answer is simply recomputed next time, which costs nothing this
    /// checker cannot afford (class graphs are small, and derivation runs
    /// only at a `.clone()`/`T from Clone` use site, not per statement).
    fn class_is_clone(&mut self, id: u32, in_progress: &mut Vec<u32>) -> bool {
        if let Some(cached) = self.clone_cache.get(&id) {
            return *cached;
        }
        // A `record`/`value class` has no identity of its own (task 11.5:
        // inline, no allocation, no `is`), so it never needs the reference-
        // graph traversal this change builds — its own field values are
        // already independently copied wherever it is copied (assignment,
        // pass, return), by ordinary value semantics. Deep-cloning the
        // *reference*-typed fields it might itself carry (a record holding
        // a class reference) would need the runtime's own generic clone
        // traversal to have an `IrType::Value` entry point too, which this
        // change's own IR/codegen (`InstKind::Clone`, `is_derived_clone_call`)
        // does not build — scoped out deliberately (reference-graph `Clone`
        // is this change's stated focus; a record wanting the same needs a
        // manual `clone()` implementation, the general escape hatch, same
        // as any other member this checker cannot derive automatically).
        if self.classes[id as usize].kind != ClassKind::Class {
            self.clone_cache.insert(id, false);
            return false;
        }
        if in_progress.contains(&id) {
            return true;
        }
        if self.class_implements_resource(id) {
            self.clone_cache.insert(id, false);
            return false;
        }
        let is_outermost = in_progress.is_empty();
        in_progress.push(id);
        let fields = self.classes[id as usize].fields.clone();
        let eligible = fields.iter().all(|f| self.is_clone_type(f.ty, in_progress));
        in_progress.pop();
        if is_outermost {
            self.clone_cache.insert(id, eligible);
        }
        eligible
    }

    /// For a diagnostic only (design D4): `ty` is assumed itself not
    /// `Clone` (a prior [`Self::is_clone_type`] call said so) — this walks
    /// the same shape again, this time collecting a human-readable path to
    /// the first offending field, so the diagnostic names the concrete
    /// member that broke the chain rather than a generic "is not `Clone`"
    /// message (matching the `STRICT_ALIAS_VIOLATION`-style precedent of
    /// naming the concrete offending member).
    fn find_non_clone_path(&mut self, ty: Type) -> String {
        if ty.nullable {
            return self.find_non_clone_path(ty.without_null());
        }
        match ty.base {
            Base::Class(id) => {
                if self.class_implements_resource(id) {
                    return format!("`{}` (it implements `Resource<E>`)", self.name(ty));
                }
                let fields = self.classes[id as usize].fields.clone();
                for f in &fields {
                    let mut probe = Vec::new();
                    if !self.is_clone_type(f.ty, &mut probe) {
                        let deeper = self.find_non_clone_path(f.ty);
                        return format!("field `{}` of `{}`, {deeper}", f.name, self.name(ty));
                    }
                }
                format!("`{}`", self.name(ty))
            }
            Base::Pointer(_) => format!("`{}`, an unsafe pointer", self.name(ty)),
            Base::Weak(_) => format!("`{}`, a weak reference", self.name(ty)),
            Base::Contract(_) | Base::ContractInstance(_) => format!(
                "`{}` (a contract-typed member — not every implementor is known to be `Clone`)",
                self.name(ty)
            ),
            Base::Instance(_) => {
                format!("`{}` (a generic class instantiation)", self.name(ty))
            }
            Base::Function(_) => format!("`{}`, a function/closure value", self.name(ty)),
            _ => format!("`{}`", self.name(ty)),
        }
    }

    fn check_assign(&mut self, stmt: &AssignStmt) {
        let value = self.check_expr(&stmt.value);

        // `_ = expr;` discards `value` deliberately — the escape hatch a
        // mandatory-consumption type like `Result` needs
        // (`Self::require_result_consumed`) — rather than assigning to a
        // real binding named `_`.
        if let AssignTarget::Name(name) = &stmt.target
            && name.name == "_"
        {
            return;
        }

        self.check_assign_target(&stmt.target, value, stmt.value.span());
    }

    /// The rules a single `(target, value)` pair of an assignment must obey
    /// — `inmut`/`inmut::strict` rebinding rejection, `inmut::strict`
    /// projection-mutation rejection, and positional type-checking.
    ///
    /// Shared by `Self::check_assign` and, once per position, by
    /// `Self::check_multi_assign` (design D4) — a simultaneous assignment
    /// obeys exactly the rules a single assignment already does, applied to
    /// every destination.
    fn check_assign_target(&mut self, target: &AssignTarget, value: Type, value_span: Span) {
        let AssignTarget::Name(name) = target else {
            let target_ty = match target {
                AssignTarget::Field(field) => {
                    let target_ty = self.check_writable_field(field);
                    if self.reject_pointer_escape(value, value_span, "assigned to a field") {
                        return;
                    }
                    target_ty
                }
                // `view[0] = 0x7f;` (roadmap Phase 4e, `fase-4e-native-slice`,
                // design D5): a write is accepted only through the receiver
                // types `Self::indexable_element`'s dispatch table marks
                // writable (today, `NativeSliceMut<T>`; `NativeSlice<T>` is
                // read-only, spec scenario exercised by task 5.5).
                AssignTarget::Index(index) => {
                    let (receiver_ty, element, writable) = self.check_index(index);
                    if element.is_unknown() {
                        return;
                    }
                    if !writable {
                        let name = self.name(receiver_ty);
                        self.error(
                            codes::INDEX_NOT_WRITABLE,
                            index.span,
                            format!("cannot write through `{name}`"),
                            "this receiver's indexing entry is read-only",
                            Some("use a NativeSliceMut<T> to write through an index".into()),
                        );
                        return;
                    }
                    element
                }
                AssignTarget::Name(_) => unreachable!("handled by the outer let-else"),
            };
            self.expect_assignable(target_ty, value, value_span, "the assigned value");
            return;
        };

        let Some(target_ty) = self.require_writable(name) else {
            return;
        };

        // Design D14 is deliberately narrower here than in `Self::check_let`:
        // a capturing closure literal only ever adopts a position's static
        // type through a `let`/`mut` *initializer* — never through a later
        // plain assignment. A local's IR storage is sized once, at its own
        // declaration (`zirk-ir`'s `lower_let`); a bare `mut f: Fn(...) =>
        // R;` with no initializer has already committed that slot to the
        // written annotation's own (capture-less) shape by the time any
        // assignment reaches it, so a capturing literal landing here later
        // would need the slot re-sized under it — unsound, not merely
        // unimplemented. `Self::expect_assignable` below reports it with
        // the same dedicated diagnostic a second, differently-captured
        // literal gets, which is an accurate description either way: this
        // position cannot hold this closure.
        self.expect_assignable(target_ty, value, value_span, "the assigned value");
        self.scopes.mark_initialized(&name.name);
    }

    /// `left, right = right, left;` (roadmap Phase 4d) — the comma-grouped
    /// form of `Self::check_assign`.
    ///
    /// Requires equal arity (targeted diagnostic otherwise), rejects
    /// duplicate destinations, and then applies `Self::check_assign_target`
    /// once per position (design D4) — the same rules a single assignment
    /// already enforces, fanned out.
    fn check_multi_assign(&mut self, stmt: &MultiAssignStmt) {
        if stmt.targets.len() != stmt.values.len() {
            self.error(
                codes::MULTI_ASSIGN_ARITY_MISMATCH,
                stmt.span,
                format!(
                    "{} destination(s) but {} source(s)",
                    stmt.targets.len(),
                    stmt.values.len()
                ),
                "a simultaneous assignment must have exactly one source per destination",
                Some("add or remove source expressions until the counts match".into()),
            );
        }

        let keys: Vec<Option<String>> = stmt.targets.iter().map(assign_target_key).collect();
        for i in 0..keys.len() {
            let Some(key) = &keys[i] else { continue };
            if let Some(j) = keys[..i].iter().position(|k| k.as_ref() == Some(key)) {
                self.error(
                    codes::DUPLICATE_ASSIGN_TARGET,
                    stmt.targets[i].span(),
                    "this destination is written twice in the same simultaneous assignment",
                    format!("also written at position {}", j + 1),
                    Some("assign to each destination at most once".into()),
                );
            }
        }

        let n = stmt.targets.len().min(stmt.values.len());
        for i in 0..n {
            let value = self.check_expr(&stmt.values[i]);

            // `_` discards this position's value exactly as it does in a
            // single assignment (`Self::check_assign`'s own handling).
            if let AssignTarget::Name(name) = &stmt.targets[i]
                && name.name == "_"
            {
                continue;
            }

            self.check_assign_target(&stmt.targets[i], value, stmt.values[i].span());
        }

        // Beyond the shorter list's length, only the checker's ordinary
        // read-checking of extra sources still applies — there is no
        // destination left to type-check them against.
        for value in &stmt.values[n..] {
            self.check_expr(value);
        }
    }

    /// The type of a field being written to, reporting why it cannot be.
    ///
    /// A field is writable when its own `inmut` allows it, and when the
    /// reference used to reach it is not `inmut::strict` (design D2 of
    /// `fase-4e-inmut-strict-proyeccion`: whether the *reference* permits
    /// mutation is the `mut`/`inmut`/`inmut::strict` matrix, checked here via
    /// `Self::root_binding_mutability`).
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

        if let Some((root_name, Mutability::Strict)) = self.root_binding_mutability(&field.object) {
            self.error(
                codes::STRICT_ALIAS_VIOLATION,
                field.name.span,
                format!("cannot write to `{}` through `{root_name}`", field.name.name),
                format!(
                    "`{root_name}` is `inmut::strict`; writing through a projection of an `inmut::strict` reference is a mutation of the same guarantee it protects"
                ),
                Some(format!(
                    "declare `{root_name}` with `mut` if the fields it reaches must change"
                )),
            );
        }

        ty
    }

    /// Walks a projection's root binding (design D1 of
    /// `fase-4e-inmut-strict-proyeccion`): `Expr::Path` resolves the binding
    /// and returns its name and declared mutability; `Expr::Field` recurses
    /// into its own object; every other shape (`Expr::This`, a call result,
    /// an index expression, …) returns `None` — deliberately conservative,
    /// matching `Self::check_strict_alias`'s own precedent of only handling
    /// shapes the checker can prove something about.
    fn root_binding_mutability(&self, expr: &Expr) -> Option<(String, Mutability)> {
        match expr {
            Expr::Path(ident) => {
                let resolved = self.scopes.resolve(&ident.name)?;
                Some((ident.name.clone(), resolved.binding.mutability))
            }
            Expr::Field(inner) => self.root_binding_mutability(&inner.object),
            _ => None,
        }
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
            // A `String` iterates by grapheme, binding a `Char` — the debt
            // Phase 2 first noted and Phase 3b's task 6.3 retires: `zirk-ir`
            // lowers this as a byte-offset walk over the string's own
            // graphemes (`lower_for_in_string`), the same shape a range loop
            // already threads its own counter with.
            Base::String => Type::of(Base::Char),
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
        // The function's own declared return type is the expected type of
        // the returned expression (roadmap Phase 4a, `expected_type`'s own
        // doc comment) — the same idea `check_let` applies for an explicit
        // annotation.
        let actual = match &stmt.value {
            Some(expr) => {
                self.expected_type = Some(self.current_return);
                self.check_expr(expr)
            }
            None => Type::VOID,
        };

        if let Some(expr) = &stmt.value
            && self.reject_pointer_escape(actual, expr.span(), "returned from a function")
        {
            return;
        }

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

        // D14, mirroring `Self::check_let`'s initializer: a capturing
        // closure literal returned directly adopts its own id as the
        // function's declared return type from here on — every later
        // `return` (and every caller, once `Self::check_function`'s
        // post-body fix-up updates the signature) is checked against it.
        let literal_captures = stmt
            .value
            .as_ref()
            .is_some_and(|e| self.capturing_literal_at(e));
        if literal_captures && self.accepts_capturing_literal(self.current_return, actual) {
            self.current_return = actual;
            return;
        }

        // A capturing closure that either targets an already-adopted,
        // *different* literal, or was not written directly here (reached
        // through a variable), gets the dedicated diagnostic instead of the
        // generic mismatch below — both need the captures boxed behind a
        // uniform representation this pass does not build (design D13).
        if let Base::Function(actual_id) = actual.base
            && self.capturing_lambda_span(actual_id).is_some()
            && self.callable_assignable(self.current_return, actual)
        {
            self.error(
                codes::AMBIGUOUS_CAPTURING_CALLABLE,
                stmt.value.as_ref().map(|e| e.span()).unwrap_or(stmt.span),
                "this function cannot return this closure",
                "a capturing closure only satisfies the declared return type when its own literal is returned directly, and only one such literal per function; a different one, or one reached through a variable, would need its captures boxed behind a uniform representation, which is not implemented yet",
                Some("general callable-type polymorphism across captures is future work (design D13/D14 of fase-4d-callables)".into()),
            );
            return;
        }

        if !self.current_return.accepts(actual)
            && !self.is_subclass_of(actual, self.current_return)
            && !self.bare_enum_matches_instance(actual, self.current_return)
            && !self.callable_assignable(self.current_return, actual)
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

    /// `throw expr;` / `throw;` (roadmap Phase 4b).
    ///
    /// Either way, the thrown type is recorded into [`Self::pending_throws`]
    /// rather than resolved against `catch`/`throws` here directly: only the
    /// enclosing `try`/function knows what covers it, the same reason a
    /// `return`'s value is checked against `current_return` rather than each
    /// `return` walking up to find its own function.
    fn check_throw(&mut self, stmt: &ThrowStmt) {
        let thrown = match &stmt.value {
            Some(expr) => {
                let ty = self.check_expr(expr);
                if !ty.is_unknown() && !self.is_throwable_type(ty) {
                    let name = self.name(ty);
                    self.error(
                        codes::TYPE_MISMATCH,
                        expr.span(),
                        format!("`{name}` cannot be thrown"),
                        "only a class that implements `Throwable` may be thrown",
                        None,
                    );
                    return;
                }
                ty
            }
            None => {
                let Some(caught) = self.catch_type else {
                    self.error(
                        codes::RETHROW_OUTSIDE_CATCH,
                        stmt.span,
                        "`throw;` is only legal inside a `catch`",
                        "with no value, it rethrows whatever the enclosing `catch` bound",
                        Some("write `throw <expr>;` to throw a new value".into()),
                    );
                    return;
                };
                caught
            }
        };

        if !thrown.is_unknown() {
            self.pending_throws.push(thrown);
        }
    }

    /// `try { } catch Type(name) { } ... finally { }` (roadmap Phase 4b).
    ///
    /// The try body's own escapees are isolated (saved and cleared from
    /// [`Self::pending_throws`]) so this `try`'s own `catch` clauses are
    /// judged only against what its own body produced — an outer `try`
    /// around this one must not see a type this one already caught, and
    /// must still see one this one's catches do not cover.
    fn check_try(&mut self, stmt: &TryStmt) -> bool {
        let mut catch_types: Vec<Type> = Vec::new();

        let outer_pending = std::mem::take(&mut self.pending_throws);
        let body_always_returns = self.check_block(&stmt.body);
        let body_escapees = std::mem::replace(&mut self.pending_throws, outer_pending);

        // Every path out of the `try` as a whole guarantees a return only
        // when the body does *and* every `catch` does too — a `catch` that
        // falls through leaves a path this analysis cannot follow further
        // (`docs/handbook`'s own missing-return diagnostic, `MISSING_RETURN`),
        // the same way an `if` needs both branches to.
        let mut catches_always_return = true;

        for catch in &stmt.catches {
            let ty = self.resolve_type_atom(&catch.ty);
            if !ty.is_unknown() && !self.is_throwable_type(ty) {
                let name = self.name(ty);
                self.error(
                    codes::UNREACHABLE_CATCH,
                    catch.ty.span,
                    format!("`{name}` cannot be caught"),
                    "`catch` names a class that implements `Throwable`",
                    None,
                );
                continue;
            }
            if !ty.is_unknown() && self.throw_is_covered(ty, &catch_types) {
                self.error(
                    codes::UNREACHABLE_CATCH,
                    catch.ty.span,
                    "this `catch` can never run",
                    "an earlier `catch` in this `try` already covers everything it would",
                    Some("reorder it before the one that already covers it".into()),
                );
                continue;
            }

            self.scopes.push();
            self.declare_local(Binding {
                name: catch.binding.name.clone(),
                ty,
                mutability: Mutability::Immutable,
                span: catch.binding.span,
                initialized: true,
            });
            let previous_catch = self.catch_type.replace(ty);
            let catch_always_returns = self.check_block(&catch.body);
            self.catch_type = previous_catch;
            self.scopes.pop();
            if !catch_always_returns {
                catches_always_return = false;
            }

            catch_types.push(ty);
        }

        // Whatever the body could throw that no catch here covers keeps
        // propagating, exactly like an ordinary `throws` call's own type
        // would from this point.
        for thrown in body_escapees {
            if !self.throw_is_covered(thrown, &catch_types) {
                self.pending_throws.push(thrown);
            }
        }

        if let Some(finally) = &stmt.finally {
            self.reject_outcome_in_finally(finally);
            self.check_block(finally);
        }

        body_always_returns && !stmt.catches.is_empty() && catches_always_return
    }

    /// Rejects a `return`/`break`/`continue`/`throw` written directly inside
    /// a `finally` block (roadmap Phase 4b) — see
    /// [`codes::FINALLY_REPLACES_OUTCOME`]'s own doc comment for how this
    /// simplifies the spec's narrower "only when it would replace an active
    /// outcome" rule.
    fn reject_outcome_in_finally(&mut self, finally: &Block) {
        for stmt in &finally.statements {
            let (word, span) = match stmt {
                Stmt::Return(s) => ("return", s.span),
                Stmt::Break(s) => ("break", s.span),
                Stmt::Continue(s) => ("continue", s.span),
                Stmt::Throw(s) => ("throw", s.span),
                _ => continue,
            };
            self.error(
                codes::FINALLY_REPLACES_OUTCOME,
                span,
                format!("`{word}` cannot appear directly inside a `finally`"),
                "it would silently replace whichever outcome triggered this `finally`",
                Some("move it outside the `finally`, or restructure the cleanup".into()),
            );
        }
    }

    /// Reports each type in [`Self::pending_throws`] the just-checked
    /// function/method's own `current_throws` does not cover (roadmap Phase
    /// 4b) — "catch or declare".
    fn report_uncaught_throws(&mut self, at: Span, name: &str) {
        let current_throws = self.current_throws.clone();
        for thrown in self.pending_throws.clone() {
            if self.throw_is_covered(thrown, &current_throws) {
                continue;
            }
            let type_name = self.name(thrown);
            self.error(
                codes::UNCAUGHT_THROW,
                at,
                format!("`{type_name}` escapes `{name}` uncaught"),
                "an explicit exception must be caught or declared",
                Some(format!("catch it in a `try`, or add `throws {type_name}`")),
            );
        }
    }

    // --- Expressions ------------------------------------------------------

    fn check_expr(&mut self, expr: &Expr) -> Type {
        // Single-shot: whatever the immediate caller set, this expression —
        // and only this one — sees it. A nested `check_expr` this dispatch
        // recurses into (an operand, an argument, …) starts fresh at `None`
        // unless something explicitly sets it again first. See
        // `expected_type`'s own doc comment for why that matters.
        let expected = self.expected_type.take();
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
            Expr::Call(e) => self.check_call(e, expected),
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
            Expr::Index(e) => self.check_index(e).1,
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
            Expr::Unsafe(e) => self.check_unsafe_expr(e),
            Expr::Commit(e) => self.check_commit_expr(e),
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

        if matches!(
            (actual.base, target.base),
            (Base::Pointer(_), Base::Pointer(_))
        ) {
            self.require_unsafe(expr.span, "`.cast<U>()`");
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
        // `.cast<U>()` (design D8) — an LLVM pointer bitcast, unchecked.
        if let (Base::Pointer(_), Base::Pointer(_)) = (target.base, actual.base) {
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
        // `ptr as Pointer<U>` (roadmap Phase 4e, design D8): `.cast<U>()`'s
        // spelling — no explicit-generic-argument call syntax exists in
        // this grammar, so the pointer reinterpret-cast reuses the
        // existing `as` cast expression instead of inventing one, since
        // `Pointer<T>` is already an ordinary type reference there.
        if matches!(
            (actual_bare.base, target_bare.base),
            (Base::Pointer(_), Base::Pointer(_))
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
        let already_captured = self
            .capture_stack
            .last()
            .is_some_and(|captures| captures.iter().any(|c| c.name == binding.name));
        if already_captured {
            return;
        }
        self.reject_pointer_escape(binding.ty, binding.span, "captured by a closure");
        let Some(captures) = self.capture_stack.last_mut() else {
            return;
        };
        captures.push(Capture {
            name: binding.name.clone(),
            ty: binding.ty,
        });
    }

    /// The blanket `Pointer<T>` escape rule (roadmap Phase 4e, design D4):
    /// any value of type `Pointer<T>`, regardless of how it was produced, is
    /// rejected as a `return` value, a field-assignment source, or a value
    /// captured by a closure — sound but deliberately more conservative than
    /// per-value provenance tracking (see `design.md`'s D4 for why this
    /// slice does not build that instead). Returns whether it rejected.
    fn reject_pointer_escape(&mut self, ty: Type, span: Span, escape: &str) -> bool {
        // Roadmap Phase 4e, `fase-4e-native-slice`, design D4: generalized
        // to match any "dependent-reference" type — `Pointer<T>` OR
        // `NativeSlice<T>` OR `NativeSliceMut<T>` — reusing this one pass
        // rather than duplicating it, per the existing "Native view escapes
        // its borrow" scenario now actually enforced.
        if !matches!(
            ty.base,
            Base::Pointer(_) | Base::NativeSlice(_) | Base::NativeSliceMut(_)
        ) {
            return false;
        }
        let name = self.name(ty);
        let kind = if matches!(ty.base, Base::Pointer(_)) {
            "A Pointer<T>"
        } else {
            "A native view"
        };
        self.error(
            codes::POINTER_ESCAPES,
            span,
            format!("`{name}` cannot be {escape}"),
            format!("{kind} value cannot outlive the frame it was obtained in — this rule is conservative and does not track individual provenance"),
            Some("copy what it points to instead, or restructure so the value stays local".into()),
        );
        true
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
                    // A closure's identity is its whole value — function
                    // pointer plus captures (design D15) —
                    // `CORE_LANGUAGE_SEMANTICS.md`'s "callable identity uses
                    // `is`". `expect_same` above already requires both
                    // sides to share one exact `Base::Function` id, so this
                    // is comparing two values of the *same* callable shape,
                    // not deciding cross-shape compatibility.
                    Base::Contract(_) | Base::String | Base::Function(_) => true,
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
        // with a reserved method the way a plain class's is (`fase-3-
        // structural-equality`, design D1/D2). `zirk-ir`'s
        // `FunctionLowering::lower_structural_equality` lowers it to a
        // conjunction of per-field comparisons — but only for a field type
        // that comparison chain knows how to compare (a scalar/`String`/
        // `Char`, a nested `record`/`value class` recursively, or a
        // `class` reference via its own `_equals`-or-identity rule, D2).
        // A residual unsupported field type (`T?`, a closure, a contract,
        // an algebraic enum with payload, …) keeps the same
        // checked-but-not-compilable diagnostic this whole comparison used
        // to get unconditionally, but now scoped to just that field (D3).
        if let Base::Class(id) = left.base
            && matches!(
                self.classes[id as usize].kind,
                ClassKind::Record | ClassKind::ValueClass
            )
        {
            if let Some(unsupported) = self.structural_equality_unsupported_field(left) {
                let name = self.name(unsupported);
                self.not_lowered(
                    expr.span,
                    &format!(
                        "structural equality on a record or value class with a field of type `{name}`"
                    ),
                    "compare its fields individually for now",
                );
            }
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

    /// Whether `ty` is a field type derived structural equality's own
    /// lowering (`zirk-ir`'s `lower_structural_equality`) knows how to
    /// compare, checked recursively for a nested `record`/`value class`
    /// field — returns the first unsupported type found, if any.
    ///
    /// A scalar (`Int*`/`Float*`/`Boolean`/`Char`), `String`, and a
    /// payload-less (traditional) enum compare directly; a `record`/`value
    /// class` field recurses into its own fields the same way; a `class`
    /// reference is always fine, whatever its own equality resolves to
    /// (D2 — its own `_equals` if declared, `is` identity otherwise, never
    /// an error at this recursive position). Everything else — `T?`, a
    /// closure, a contract, an algebraic enum carrying a payload, a
    /// generic parameter — has no comparison this lowering builds yet.
    fn structural_equality_unsupported_field(&self, ty: Type) -> Option<Type> {
        // `T?` (`Type::nullable`, not a `Base` variant of its own) has no
        // comparison this lowering builds yet: `IrType::Nullable` is a
        // `{present, payload}` struct, not one of the shapes
        // `lower_field_equality`'s fallback `Binary { op: Eq }` arm
        // actually codegens (a scalar/`String`/`Char`) or its two other
        // arms specifically handle (`Value`/`Object`).
        if ty.nullable {
            return Some(ty);
        }
        match ty.base {
            Base::Int(_) | Base::Float(_) | Base::Boolean | Base::Char | Base::String => None,
            Base::Enum(id) => {
                let all_bare = self.enums[id as usize]
                    .variants
                    .iter()
                    .all(|v| v.associated.is_empty());
                if all_bare { None } else { Some(ty) }
            }
            Base::Class(id) => match self.classes[id as usize].kind {
                ClassKind::Record | ClassKind::ValueClass => self.classes[id as usize]
                    .fields
                    .iter()
                    .find_map(|f| self.structural_equality_unsupported_field(f.ty)),
                // A plain `class` field always resolves (D2): its own
                // `_equals` if it declares one, identity otherwise — never
                // the "does not define equality" error that only applies
                // to a bare top-level `==` between two of them.
                ClassKind::Class => None,
                ClassKind::Abstract => Some(ty),
            },
            _ => Some(ty),
        }
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
                "incrementing a field or an indexed element",
                "writing a member or an index needs its own increment rule, not implemented yet",
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
            Stmt::Unsafe(nested) => self.check_unsafe_expr(nested),
            Stmt::Commit(nested) => self.check_commit_expr(nested),
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

    /// `unsafe { ... }` used where a value is expected (roadmap Phase 4e),
    /// e.g. `mut result = unsafe { ptr.read() };` — same context-opening as
    /// the statement form (`Self::check_unsafe_block`), but its body is
    /// checked as a value via [`Self::check_block_value`].
    fn check_unsafe_expr(&mut self, e: &UnsafeBlock) -> Type {
        self.unsafe_depth += 1;
        let ty = self.check_block_value(&e.body);
        self.unsafe_depth -= 1;
        ty
    }

    /// `commit { ... }` used where a value is expected (roadmap Phase 4e).
    /// See [`Self::check_commit_block`] for the context rule this shares.
    fn check_commit_expr(&mut self, e: &CommitBlock) -> Type {
        if self.unsafe_depth == 0 {
            self.error(
                codes::COMMIT_OUTSIDE_UNSAFE,
                e.span,
                "`commit {}` outside `unsafe`",
                "a commit boundary only makes sense inside a reversible unsafe transaction",
                Some("wrap this in `unsafe { commit { ... } }`".into()),
            );
        }
        self.commit_depth += 1;
        let ty = self.check_block_value(&e.body);
        self.commit_depth -= 1;
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

    /// `match ... with binding` (roadmap Phase 4c) requires a `Result<R,Err>`
    /// scrutinee — acquisition "normally returns `Result<Resource,OpenError>`"
    /// (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 4) — so a
    /// grouped or non-`Result` acquisition, out of scope for this pass, is
    /// rejected here rather than silently accepted with no cleanup wired up.
    fn check_resource_match_scrutinee(&mut self, with_binding: &Ident, scrutinee: Type) {
        if scrutinee.is_unknown() {
            return;
        }
        let is_result = matches!(scrutinee.without_null().base, Base::EnumInstance(inst)
            if self.native_result == Some(self.enum_instances[inst as usize].enum_id));
        if is_result && !scrutinee.nullable {
            return;
        }
        let name = self.name(scrutinee);
        self.error(
            codes::INVALID_RESOURCE_MATCH,
            with_binding.span,
            format!("`match ... with` needs a `Result<R,Err>` scrutinee, not `{name}`"),
            "acquisition normally returns `Result<Resource,OpenError>`",
            None,
        );
    }

    /// Whether `ty` — whatever a `match ... with binding` arm bound
    /// `binding` to — is a value that may be closed: a non-nullable class
    /// that `implements Resource<E>` for some `E` (roadmap Phase 4c).
    fn check_resource_binding_type(&mut self, with_binding: &Ident, ty: Type) {
        if ty.is_unknown() {
            return;
        }
        let Some(native_resource) = self.native_resource else {
            return;
        };
        let implements_resource = !ty.nullable
            && matches!(ty.base, Base::Class(id) if self.classes[id as usize]
                .contract_instances
                .iter()
                .any(|&inst| self.contract_instances[inst as usize].contract == native_resource.resource));
        if implements_resource {
            return;
        }
        let name = self.name(ty);
        self.error(
            codes::INVALID_RESOURCE_MATCH,
            with_binding.span,
            format!("`{}` does not implement `Resource<E>`, so it cannot be closed automatically", name),
            "`match ... with` closes whatever its binding names on every exit from the arm that acquires it",
            Some("implement `Resource<E>` (`close(): Result<Void,E>`, `is_closed(): Boolean`) on it, or drop `with`".into()),
        );
    }

    fn check_match(&mut self, expr: &MatchExpr, as_value: bool) -> Type {
        let scrutinee = self.check_expr(&expr.scrutinee);
        self.matches.insert(expr.span, scrutinee);

        if let Some(with_binding) = &expr.with_binding {
            self.check_resource_match_scrutinee(with_binding, scrutinee);
        }

        let mut arm_types: Vec<(Type, Span)> = Vec::new();
        let mut covered: Vec<String> = Vec::new();
        let mut has_wildcard = false;
        let mut resource_binding_found = false;

        // A `null` arm already took the only case in which the scrutinee is
        // absent, so every other arm sees a value that cannot be — the same
        // narrowing `??` gives a coalesced expression, applied here to
        // whatever a sibling arm's own pattern binds.
        let has_null_arm = expr
            .arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Null(_)));

        for arm in &expr.arms {
            let arm_scrutinee = if has_null_arm && !matches!(arm.pattern, Pattern::Null(_)) {
                scrutinee.without_null()
            } else {
                scrutinee
            };

            let mut bindings = Vec::new();
            self.check_pattern(
                &arm.pattern,
                arm_scrutinee,
                &mut covered,
                &mut has_wildcard,
                &mut bindings,
            );

            if let Some(with_binding) = &expr.with_binding
                && let Some(resource) = bindings.iter().find(|b| b.name == with_binding.name)
            {
                resource_binding_found = true;
                self.check_resource_binding_type(with_binding, resource.ty);
            }

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

        if let Some(with_binding) = &expr.with_binding
            && !resource_binding_found
        {
            self.error(
                codes::INVALID_RESOURCE_MATCH,
                with_binding.span,
                format!("no arm's pattern binds `{}`", with_binding.name),
                "`match ... with` names the resource one of the arms acquires by binding it in its own pattern",
                Some(format!(
                    "destructure it into `{}` in the arm that acquires it, e.g. `Result.Ok({})`",
                    with_binding.name, with_binding.name
                )),
            );
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
        let type_params = enum_type.type_params.clone();
        let associated = variant.associated.clone();

        self.expect_pattern_type(scrutinee, Type::of(Base::Enum(index as u32)), pattern.span);

        // A generic enum's variant carries its own bare `Base::Param` field
        // types (`Result.Ok`'s `value: T`) — substituted here against the
        // scrutinee's own concrete instantiation, the same way a generic
        // class's field read already substitutes (`Self::member_type`'s
        // `Base::Instance` arm), so a destructured binding gets `Int32`,
        // not the type parameter itself (roadmap Phase 4a).
        let substitution: HashMap<u32, Type> =
            if let Base::EnumInstance(inst) = scrutinee.without_null().base {
                let instance = &self.enum_instances[inst as usize];
                if instance.enum_id == index as u32 {
                    type_params
                        .iter()
                        .copied()
                        .zip(instance.args.clone())
                        .collect()
                } else {
                    HashMap::new()
                }
            } else {
                HashMap::new()
            };
        let associated: Vec<AssociatedFieldInfo> = associated
            .into_iter()
            .map(|f| AssociatedFieldInfo {
                name: f.name,
                ty: self.substitute(f.ty, &substitution),
            })
            .collect();

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
        //
        // A variant pattern (`Result.Ok(value)`) types itself against the
        // bare, unparameterized enum — the same reason `bare_enum_matches_instance`'s
        // own doc comment gives for construction and `return`: nothing here
        // reads the pattern's own type arguments, only which variant it
        // names, so a bare `Result` pattern matching a `Result<Int32,String>`
        // scrutinee is not a real mismatch.
        if scrutinee.without_null().accepts(pattern)
            || self.bare_enum_matches_instance(pattern, scrutinee)
            || scrutinee.is_unknown()
        {
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

        // A generic enum instantiation (`Result<Int32,String>`, roadmap
        // Phase 4a) has the same closed variant set as the bare enum it
        // instantiates — only the field types differ, and exhaustiveness
        // never looks at those.
        let enum_id = match scrutinee.base {
            Base::Enum(id) => Some(id),
            Base::EnumInstance(inst) => Some(self.enum_instances[inst as usize].enum_id),
            _ => None,
        };
        if let Some(id) = enum_id
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

    /// The extensible receiver-type-keyed dispatch table `receiver[index]`
    /// consults (roadmap Phase 4e, `fase-4e-native-slice`, design D5) —
    /// today exactly two entries: `NativeSlice<T>` (read-only element `T`)
    /// and `NativeSliceMut<T>` (read/write element `T`). Returns the
    /// element type and whether a write through it is permitted; `None`
    /// means this receiver type has no indexing entry at all.
    ///
    /// Deliberately its own small function, not a hardcoded two-armed match
    /// buried in `Self::check_index` — this is where Phase 7's `Array<T>`/
    /// `List<T>` register their own entries later, without reworking the
    /// parser, AST, or place-classification logic D5 adds.
    fn indexable_element(&self, receiver: Type) -> Option<(Type, bool)> {
        match receiver.base {
            Base::NativeSlice(id) => Some((
                self.native_slice_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(Type::UNKNOWN),
                false,
            )),
            Base::NativeSliceMut(id) => Some((
                self.native_slice_mut_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(Type::UNKNOWN),
                true,
            )),
            _ => None,
        }
    }

    /// `receiver[index]` (roadmap Phase 4e, `fase-4e-native-slice`, design
    /// D5): the grammar accepts any receiver — this is where the checker
    /// restricts it, through `Self::indexable_element`'s dispatch table.
    /// Returns the receiver's own type, the element type, and whether the
    /// place is writable, so both a read (`Self::check_expr`) and a write
    /// (`Self::check_assign_target`) share this one lookup.
    fn check_index(&mut self, expr: &IndexExpr) -> (Type, Type, bool) {
        let receiver = self.check_expr(&expr.receiver);
        let index = self.check_expr(&expr.index);

        if receiver.is_unknown() {
            return (receiver, Type::UNKNOWN, false);
        }

        if receiver.nullable {
            self.reject_absent_receiver(receiver, expr.receiver.span());
            return (receiver, Type::UNKNOWN, false);
        }

        if !index.is_unknown() && !matches!(index.base, Base::Int(_)) || index.nullable {
            let found = self.name(index);
            self.error(
                codes::TYPE_MISMATCH,
                expr.index.span(),
                "an index must be an integer",
                format!("found {found}"),
                None,
            );
        }

        let Some((element, writable)) = self.indexable_element(receiver) else {
            let name = self.name(receiver);
            self.error(
                codes::INDEXING_NOT_SUPPORTED,
                expr.receiver.span(),
                format!("`{name}` does not support indexing"),
                "indexing is only defined for a type with an entry in the checker's own indexing dispatch table",
                Some("index a NativeSlice<T>/NativeSliceMut<T> instead".into()),
            );
            return (receiver, Type::UNKNOWN, false);
        };

        (receiver, element, writable)
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

        // `.is_null` (roadmap Phase 4e): the one `Pointer<T>` operation that
        // needs no `unsafe` (spec scenario "Null raw pointer is inspected").
        if let Base::Pointer(_) = object.base
            && member.name == "is_null"
        {
            return Type::BOOLEAN;
        }

        // `.is_alive` (roadmap Phase 4e, `fase-4e-weak`, design D4): an
        // observational read of the same underlying state `.upgrade()`
        // checks — never produces a strong reference, so it needs no method
        // call and no `unsafe` (spec scenario "Is-alive is observational
        // only").
        if let Base::Weak(_) = object.base
            && member.name == "is_alive"
        {
            return Type::BOOLEAN;
        }

        // `.length`/`.is_empty` (roadmap Phase 4e, `fase-4e-native-slice`,
        // design's proposal): usable in ordinary safe code on an
        // already-constructed `NativeSlice<T>`/`NativeSliceMut<T>` — no
        // `unsafe` needed to read/write through a view, only to construct
        // one (task 1.3).
        if matches!(object.base, Base::NativeSlice(_) | Base::NativeSliceMut(_)) {
            match member.name.as_str() {
                "length" => return Type::of(Base::Int(IntWidth::U64)),
                "is_empty" => return Type::BOOLEAN,
                _ => {}
            }
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

        if safe {
            self.reject_redundant_safe(object, object_span);
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

        // `value.clone()` where `T from Clone` (roadmap Phase 4e,
        // `fase-4e-clone`, the type-system spec's "Generic projection needs
        // Clone" scenario): `Clone`'s own contract table carries no
        // `clone` method (`Self::register_native_clone_contract`'s own doc
        // comment), so the ordinary constraint-method loop below would
        // never find it — typed directly to `T` itself here instead.
        if field.name.name == "clone"
            && expr.args.is_empty()
            && let Some(clone_id) = self.native_clone
            && constraints.contains(&Type::of(Base::Contract(clone_id)))
        {
            return Type::of(Base::Param(id));
        }

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
                            throws: m.throws.clone(),
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
                        throws: m.throws.clone(),
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

    /// Reports `?.` on a receiver that is never absent — the same judgment
    /// `Self::check_coalesce` makes for `??` on a non-nullable left side: an
    /// operator whose entire purpose is handling absence has nothing to do
    /// when absence is not a possibility, so it is flagged rather than
    /// silently accepted and left to widen a type (or, for a method
    /// returning through a receiver the lowering never expects to be
    /// non-nullable, to reach the IR broken).
    fn reject_redundant_safe(&mut self, object: Type, at: Span) {
        let name = self.name(object);
        self.error(
            codes::REDUNDANT_OPERATOR,
            at,
            "`?.` on a receiver that is never null",
            format!("the receiver has type `{name}`, which always holds a value"),
            Some("use `.` instead".into()),
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
        expected: Option<Type>,
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
            throws: Vec::new(),
        };
        // `Result.Ok(v)` cannot determine `E` from `v` alone — no argument
        // ever will, since `E` names the *other* variant's payload. Seeding
        // from the expected type (`Self::expected_type`'s own doc comment)
        // is what makes a two-parameter generic enum constructible at all,
        // not just a convenience: `Iteration<T>`'s own single parameter is
        // always determined by `Item(value)`'s argument, so this gap never
        // mattered until a second, cross-variant parameter did (roadmap
        // Phase 4a).
        let seed: HashMap<u32, Type> = match expected {
            Some(Type {
                base: Base::EnumInstance(inst),
                ..
            }) if self.enum_instances[inst as usize].enum_id == enum_id => type_params
                .iter()
                .copied()
                .zip(self.enum_instances[inst as usize].args.iter().copied())
                .collect(),
            _ => HashMap::new(),
        };
        let (_, substitution) = self.check_direct_call_with_subst_seeded(expr, &signature, seed);

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

    /// Walks a block for `Self::check_recursive_reference`, statement by
    /// statement.
    fn check_recursive_reference_block(&mut self, block: &Block, name: &str, nested: bool) {
        for stmt in &block.statements {
            self.check_recursive_reference_stmt(stmt, name, nested);
        }
    }

    /// Walks a statement for `Self::check_recursive_reference`.
    fn check_recursive_reference_stmt(&mut self, stmt: &Stmt, name: &str, nested: bool) {
        match stmt {
            Stmt::Let(s) => {
                if let Some(init) = &s.init {
                    self.check_recursive_reference(init, name, nested);
                }
            }
            Stmt::MultiLet(s) => {
                for init in &s.inits {
                    self.check_recursive_reference(init, name, nested);
                }
            }
            Stmt::Assign(s) => {
                match &s.target {
                    AssignTarget::Field(f) => {
                        self.check_recursive_reference(&f.object, name, nested)
                    }
                    AssignTarget::Index(i) => {
                        self.check_recursive_reference(&i.receiver, name, nested);
                        self.check_recursive_reference(&i.index, name, nested);
                    }
                    AssignTarget::Name(_) => {}
                }
                self.check_recursive_reference(&s.value, name, nested);
            }
            Stmt::MultiAssign(s) => {
                for target in &s.targets {
                    match target {
                        AssignTarget::Field(f) => {
                            self.check_recursive_reference(&f.object, name, nested)
                        }
                        AssignTarget::Index(i) => {
                            self.check_recursive_reference(&i.receiver, name, nested);
                            self.check_recursive_reference(&i.index, name, nested);
                        }
                        AssignTarget::Name(_) => {}
                    }
                }
                for value in &s.values {
                    self.check_recursive_reference(value, name, nested);
                }
            }
            Stmt::If(s) => {
                self.check_recursive_reference(&s.condition, name, nested);
                self.check_recursive_reference_block(&s.then_branch, name, nested);
                match &s.else_branch {
                    Some(ElseBranch::Block(b)) => {
                        self.check_recursive_reference_block(b, name, nested)
                    }
                    Some(ElseBranch::If(inner)) => self.check_recursive_reference_stmt(
                        &Stmt::If((**inner).clone()),
                        name,
                        nested,
                    ),
                    None => {}
                }
            }
            Stmt::Loop(s) => {
                if let Some(init) = &s.init {
                    self.check_recursive_reference_stmt(init, name, nested);
                }
                if let Some(condition) = &s.condition {
                    self.check_recursive_reference(condition, name, nested);
                }
                if let Some(step) = &s.step {
                    self.check_recursive_reference_stmt(step, name, nested);
                }
                self.check_recursive_reference_block(&s.body, name, nested);
            }
            Stmt::ForIn(s) => {
                self.check_recursive_reference(&s.iterable, name, nested);
                self.check_recursive_reference_block(&s.body, name, nested);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
            Stmt::Return(s) => {
                if let Some(value) = &s.value {
                    self.check_recursive_reference(value, name, nested);
                }
            }
            Stmt::Expr(s) => self.check_recursive_reference(&s.expr, name, nested),
            Stmt::Block(b) => self.check_recursive_reference_block(b, name, nested),
            Stmt::Throw(s) => {
                if let Some(value) = &s.value {
                    self.check_recursive_reference(value, name, nested);
                }
            }
            Stmt::Try(s) => {
                self.check_recursive_reference_block(&s.body, name, nested);
                for catch in &s.catches {
                    self.check_recursive_reference_block(&catch.body, name, nested);
                }
                if let Some(finally) = &s.finally {
                    self.check_recursive_reference_block(finally, name, nested);
                }
            }
            Stmt::Unsafe(s) => self.check_recursive_reference_block(&s.body, name, nested),
            Stmt::Commit(s) => self.check_recursive_reference_block(&s.body, name, nested),
        }
    }

    /// Rejects every reference to a recursive lambda's own binding (`name`)
    /// inside its own body other than as the direct callee of a call
    /// (`E0440`) — see `Self::check_lambda`'s own call site for why.
    ///
    /// `nested` is `true` once the walk has descended into another lambda
    /// literal: `zirk-ir`'s self-call rewrite only ever covers the
    /// recursive lambda's *own* immediate body — a reference reached from
    /// inside a lambda nested within it is an ordinary (but unimplemented)
    /// capture-of-a-capture, not a self-call, so it is rejected even in
    /// call position there.
    fn check_recursive_reference(&mut self, expr: &Expr, name: &str, nested: bool) {
        match expr {
            Expr::Path(ident) if ident.name == name => {
                self.error(
                    codes::RECURSIVE_BINDING_NOT_A_VALUE,
                    ident.span,
                    format!("`{name}` can only be called here, not used as a value"),
                    "a recursive lambda refers to itself only through a direct call inside its own body (`ZIRK_LANGUAGE_SPEC.md` section 6); using its own binding any other way — assigned, passed as an argument, compared, or reached from inside a nested lambda — is not implemented yet",
                    Some(format!("call it directly, as in `{name}(...)`")),
                );
            }
            Expr::Int(_)
            | Expr::Float(_)
            | Expr::Char(_)
            | Expr::Str(_)
            | Expr::Bool(_)
            | Expr::Null(_)
            | Expr::Path(_)
            | Expr::This(_)
            | Expr::Super(_)
            | Expr::Variant(_) => {}
            Expr::Unary(e) => self.check_recursive_reference(&e.operand, name, nested),
            Expr::Binary(e) => {
                self.check_recursive_reference(&e.left, name, nested);
                self.check_recursive_reference(&e.right, name, nested);
            }
            Expr::Call(e) => {
                match &*e.callee {
                    // The one shape a recursive binding is actually allowed
                    // in: called directly, and only outside a nested lambda.
                    Expr::Path(ident) if !nested && ident.name == name => {}
                    other => self.check_recursive_reference(other, name, nested),
                }
                for arg in &e.args {
                    self.check_recursive_reference(&arg.value, name, nested);
                }
            }
            Expr::Range(e) => {
                self.check_recursive_reference(&e.start, name, nested);
                self.check_recursive_reference(&e.end, name, nested);
            }
            Expr::If(e) => {
                self.check_recursive_reference(&e.condition, name, nested);
                self.check_recursive_reference_block(&e.then_branch, name, nested);
                match &e.else_branch {
                    Some(ElseBranch::Block(b)) => {
                        self.check_recursive_reference_block(b, name, nested)
                    }
                    Some(ElseBranch::If(inner)) => self.check_recursive_reference_stmt(
                        &Stmt::If((**inner).clone()),
                        name,
                        nested,
                    ),
                    None => {}
                }
            }
            Expr::Ternary(e) => {
                self.check_recursive_reference(&e.condition, name, nested);
                self.check_recursive_reference(&e.when_true, name, nested);
                self.check_recursive_reference(&e.when_false, name, nested);
            }
            Expr::Increment(e) => match &e.target {
                AssignTarget::Field(f) => self.check_recursive_reference(&f.object, name, nested),
                AssignTarget::Index(i) => {
                    self.check_recursive_reference(&i.receiver, name, nested);
                    self.check_recursive_reference(&i.index, name, nested);
                }
                AssignTarget::Name(_) => {}
            },
            Expr::Field(e) => self.check_recursive_reference(&e.object, name, nested),
            Expr::Index(e) => {
                self.check_recursive_reference(&e.receiver, name, nested);
                self.check_recursive_reference(&e.index, name, nested);
            }
            Expr::Match(e) => {
                self.check_recursive_reference(&e.scrutinee, name, nested);
                for arm in &e.arms {
                    match &arm.body {
                        ArmBody::Expr(body) => self.check_recursive_reference(body, name, nested),
                        ArmBody::Block(b) => self.check_recursive_reference_block(b, name, nested),
                    }
                }
            }
            Expr::Lambda(l) => {
                // Everything reachable from inside a nested lambda literal
                // is walked with `nested = true` from here on — see this
                // method's own doc comment.
                for p in &l.params {
                    if let Some(default) = &p.default {
                        self.check_recursive_reference(default, name, true);
                    }
                }
                match &*l.body {
                    LambdaBody::Expr(body) => self.check_recursive_reference(body, name, true),
                    LambdaBody::Block(b) => self.check_recursive_reference_block(b, name, true),
                }
            }
            Expr::Println(e) => self.check_recursive_reference(&e.arg, name, nested),
            Expr::Cast(e) => self.check_recursive_reference(&e.expr, name, nested),
            Expr::Interpolated(e) => {
                for part in &e.parts {
                    if let InterpolatedPart::Expr(inner) = part {
                        self.check_recursive_reference(inner, name, nested);
                    }
                }
            }
            Expr::Unsafe(e) => self.check_recursive_reference_block(&e.body, name, nested),
            Expr::Commit(e) => self.check_recursive_reference_block(&e.body, name, nested),
        }
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

        // `Option::take` so a lambda nested inside this one (if this is the
        // recursive lambda `Self::check_let` just pre-declared) never
        // mistakes *itself* for the recursive one too — only the one
        // literal directly initializing the pre-declared binding consumes
        // this.
        let own_recursive_binding = self.recursive_binding.take();

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

        let shape = FnType {
            params: params.iter().map(|p| p.ty).collect(),
            returns,
        };

        // A lambda that captures nothing is, at the representation level,
        // exactly as uniform as a named function (design D12: an empty
        // `ClosureLayout`, `{function pointer}` and nothing else) — so it is
        // interned structurally, the same table `Self::resolve_fn_type_ref`
        // and `Self::check_path` share, letting it freely interchange with
        // any other value of the same shape.
        //
        // A lambda that captures something keeps a type of its own rather
        // than sharing one per signature: its captures are part of its
        // representation (D10), so two lambdas of the same shape are not
        // interchangeable in general (design D13, deferred). Interning them
        // together would let one be assigned over the other and leave the IR
        // holding a value whose layout no longer matches its slot. A single
        // such literal may still satisfy a `Fn(...) => R` position written
        // directly at that literal (design D14) — `Self::expect_assignable`
        // recognizes that case by looking this id up in `self.lambdas`.
        let fn_type = if captures.is_empty() {
            self.intern_fn_type(shape)
        } else {
            self.fn_types.push(shape);
            (self.fn_types.len() - 1) as u32
        };

        // Only a real, actually-captured self-reference counts — a
        // pre-declared binding the body never ends up calling is simply an
        // ordinary (if unused) local, not a recursive lambda.
        let recursive_binding =
            own_recursive_binding.filter(|name| captures.iter().any(|c| &c.name == name));

        // `zirk-ir::lower_lambda` never materializes the recursive binding
        // as a real runtime capture (design addendum in `design.md`): a
        // direct call to it is rewritten into an ordinary recursive call,
        // but any other use has no closure value to give at all — the
        // value the name refers to does not exist yet at the point this
        // literal is still being built. Walked here, structurally, so a
        // program using it any other way is rejected with a diagnostic
        // instead of reaching that gap during lowering.
        if let Some(name) = &recursive_binding {
            match &*expr.body {
                LambdaBody::Expr(body) => self.check_recursive_reference(body, name, false),
                LambdaBody::Block(b) => self.check_recursive_reference_block(b, name, false),
            }
        }

        self.lambdas.insert(
            expr.span,
            LambdaInfo {
                captures,
                fn_type,
                recursive_binding,
            },
        );

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
            throws: method.throws.clone(),
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
            // A constructor cannot declare `throws` in this pass (roadmap
            // Phase 4b's own scope).
            throws: Vec::new(),
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
            // `.clone()` (roadmap Phase 4e, `fase-4e-clone`, design D1/D4):
            // a class that declares its own `clone` method (the manual
            // implementation escape hatch) is dispatched exactly like any
            // other method call, below — this only intercepts the case
            // nothing declares, which is the compiler-derived operation.
            // Gated on a bare, no-argument `.clone()` so `.clone(x)` (a
            // typo, or a real user method with that name and a different
            // arity) still reaches the ordinary "unknown/wrong-arity"
            // diagnostics via `Self::check_method_call` instead of being
            // silently swallowed here.
            if field.name.name == "clone"
                && expr.args.is_empty()
                && self.classes[id as usize].method("clone").is_none()
            {
                return self.check_derived_clone_call(object, id, field);
            }
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

    /// The compiler-derived `.clone(): T` (roadmap Phase 4e, `fase-4e-clone`,
    /// design D1/D4) — reached only when `id` declares no `clone` method of
    /// its own (`Self::check_method_call_on`'s own gate). Types to `T`
    /// itself (`object`, unchanged) when `id`'s field graph is `Clone`;
    /// otherwise rejects with `NOT_CLONE`, naming the specific field/path
    /// that broke the chain (design D4).
    fn check_derived_clone_call(&mut self, object: Type, id: u32, field: &FieldExpr) -> Type {
        let mut in_progress = Vec::new();
        if self.class_is_clone(id, &mut in_progress) {
            return object;
        }
        let name = self.classes[id as usize].name.clone();
        let path = self.find_non_clone_path(object);
        self.error(
            codes::NOT_CLONE,
            field.name.span,
            format!("`{name}` is not `Clone`"),
            format!("its field graph reaches {path}, which is not `Clone`"),
            Some(
                "implement `Clone` manually (declare your own `clone(): T`), or remove/replace the offending member"
                    .into(),
            ),
        );
        Type::UNKNOWN
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
            throws: method.throws.clone(),
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
            // A constructor cannot declare `throws` in this pass (roadmap
            // Phase 4b's own scope).
            throws: Vec::new(),
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
            throws: Vec::new(),
        };
        self.check_direct_call(expr, &signature)
    }

    /// Rejects a `Result<T,E>` produced by an expression statement and
    /// never consumed (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`
    /// section 2: "ignoring it is a compile-time error"). `_ = expr;`
    /// (`Self::check_assign`'s own `_` handling) is the explicit escape
    /// hatch, and it is a `Stmt::Assign`, not a `Stmt::Expr` — so it never
    /// reaches this check at all.
    fn require_result_consumed(&mut self, ty: Type, span: Span) {
        let Base::EnumInstance(inst) = ty.base else {
            return;
        };
        if self.native_result != Some(self.enum_instances[inst as usize].enum_id) {
            return;
        }
        self.error(
            codes::DISCARDED_RESULT,
            span,
            "a `Result` must be handled",
            "an operation that can fail must be matched or checked, not silently dropped",
            Some("use `match`, one of its methods (`is_ok`, `get_or`, …), or `_ = expr;` to discard it deliberately".into()),
        );
    }

    /// `Result<T,E>`'s seven in-scope methods (`fase-4a-errores/proposal.md`
    /// — `get_or_else` and the four generic combinators need machinery this
    /// phase does not build). `t`/`e` are the receiver's own resolved type
    /// arguments. `None` when `field` does not name one of them, so the
    /// call falls through to the ordinary unknown-method error the way any
    /// other unrecognized field access on the receiver would.
    ///
    /// Building a throwaway [`Signature`] and handing it to
    /// [`Self::check_direct_call`] reuses the same argument-matching,
    /// naming, and arity diagnostics an ordinary function call gets, rather
    /// than duplicating that logic by hand for eight names.
    fn check_result_method_call(
        &mut self,
        expr: &CallExpr,
        field: &FieldExpr,
        t: Type,
        e: Type,
    ) -> Option<Type> {
        let (params, returns): (Vec<(&str, Type)>, Type) = match field.name.name.as_str() {
            "is_ok" | "is_error" => (Vec::new(), Type::BOOLEAN),
            "ok_or_null" => (Vec::new(), t.as_nullable()),
            "error_or_null" => (Vec::new(), e.as_nullable()),
            "unwrap" => (Vec::new(), t),
            "unwrap_error" => (Vec::new(), e),
            "get_or" => (vec![("default_value", t)], t),
            _ => return None,
        };

        let signature = Signature {
            name: field.name.name.clone(),
            params: params
                .into_iter()
                .map(|(name, ty)| ParamInfo {
                    name: name.to_string(),
                    ty,
                    optional: false,
                    has_default: false,
                    variadic: false,
                })
                .collect(),
            returns,
            shared: true,
            span: field.name.span,
            type_params: Vec::new(),
            throws: Vec::new(),
        };
        Some(self.check_direct_call(expr, &signature))
    }

    /// A `Pointer<T>` operation requiring `unsafe` used outside one (design
    /// D3, spec scenario "Pointer operation outside unsafe") — shared by
    /// every pointer operation except `.is_null` (`Self::member_type`'s own
    /// branch, exempted per the type-system spec delta).
    fn require_unsafe(&mut self, span: Span, what: &str) {
        if self.unsafe_depth == 0 {
            self.error(
                codes::POINTER_OP_OUTSIDE_UNSAFE,
                span,
                format!("{what} requires an enclosing `unsafe` boundary"),
                "raw pointer construction, dereference and arithmetic are in the closed unsafe operation set",
                Some("wrap it in an `unsafe { ... }` block".into()),
            );
        }
    }

    /// Whether `expr` is an lvalue `Pointer.from` may take the address of —
    /// a local/parameter name, or a field projection (task 6.1).
    fn is_addressable_place(&self, expr: &Expr) -> bool {
        matches!(expr, Expr::Path(_) | Expr::Field(_))
    }

    /// `Pointer.from(place)` (roadmap Phase 4e, design D8): the address of
    /// an addressable local, parameter, or field, typed `Pointer<T>` where
    /// `T` is `place`'s own type — which must itself be FFI-safe.
    fn check_pointer_from(&mut self, expr: &CallExpr) -> Type {
        self.require_unsafe(expr.span, "`Pointer.from`");

        if expr.args.len() != 1 || expr.args[0].name.is_some() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                "`Pointer.from` takes exactly one positional argument",
                format!("received {}", expr.args.len()),
                Some("write `Pointer.from(place)`".into()),
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::UNKNOWN;
        }

        let place = &expr.args[0].value;
        if !self.is_addressable_place(place) {
            self.error(
                codes::TYPE_MISMATCH,
                place.span(),
                "`Pointer.from` needs an addressable place",
                "only a local, a parameter, or a field access has an address to take",
                Some("pass a variable or a field access directly".into()),
            );
        }

        let ty = self.check_expr(place);
        if ty.is_unknown() {
            return Type::UNKNOWN;
        }
        if !is_ffi_safe(ty, &self.pointer_types) {
            let name = self.name(ty);
            self.error(
                codes::NOT_FFI_SAFE,
                place.span(),
                format!("`{name}` has no stable C-ABI layout"),
                "Pointer<T> only allows Boolean, fixed-width integers, Float32/64, and another Pointer<U>",
                None,
            );
            return Type::UNKNOWN;
        }

        let id = self.intern_pointer_type(ty);
        Type::of(Base::Pointer(id))
    }

    /// `.read()`/`.write(v)`/`.offset(n)`/`.offset_bytes(n)` (design D8) —
    /// `Pointer<T>`'s closed operation set beyond `.is_null` (handled in
    /// `Self::member_type` instead, since it needs no `unsafe`). Returns
    /// `None` for a name this receiver has no such method by, so the
    /// generic "unknown member" diagnostic still applies to a typo.
    fn check_pointer_method_call(
        &mut self,
        expr: &CallExpr,
        field: &FieldExpr,
        object: Type,
    ) -> Option<Type> {
        let Base::Pointer(id) = object.base else {
            return None;
        };
        let t = self
            .pointer_types
            .get(id as usize)
            .copied()
            .unwrap_or(Type::UNKNOWN);

        // `.as_slice(length)`/`.as_slice_mut(length)` (roadmap Phase 4e,
        // `fase-4e-native-slice`, task 1.2): construct a validated
        // `NativeSlice<T>`/`NativeSliceMut<T>` from this `Pointer<T>` — same
        // `unsafe` treatment `Pointer<T>`'s other operations already have,
        // returning `Result<..., NativeError>` (`register_native_failure`'s
        // own shape, task 1.2's own instruction to reuse it rather than
        // invent a different one).
        if field.name.name == "as_slice" || field.name.name == "as_slice_mut" {
            let mutable = field.name.name == "as_slice_mut";
            let view = if mutable {
                let slice_id = self.intern_native_slice_mut_type(t);
                Type::of(Base::NativeSliceMut(slice_id))
            } else {
                let slice_id = self.intern_native_slice_type(t);
                Type::of(Base::NativeSlice(slice_id))
            };
            let returns = self.native_result_type(view);

            self.require_unsafe(expr.span, &format!("`.{}()`", field.name.name));

            let signature = Signature {
                name: field.name.name.clone(),
                params: vec![ParamInfo {
                    // `Int32`, matching `.offset(n: Int32)`'s own existing
                    // choice on `Pointer<T>` (`check_pointer_method_call`)
                    // rather than `UInt64` (what `.length` itself reads as):
                    // an ordinary integer literal always types `Int32`
                    // (`Self::check_int_literal`, no context-directed
                    // literal inference exists yet), and `Type::accepts`
                    // requires matching signedness for implicit widening —
                    // `UInt64` here would make `p.as_slice(3)` reject a bare
                    // literal, forcing an explicit `3 as UInt64` at every
                    // call site for no safety benefit (a negative length is
                    // still caught, now by the runtime validation instead
                    // of by the type). `zirk-ir` widens to `UInt64`
                    // internally for the runtime call and the view's own
                    // representation either way.
                    name: "length".to_string(),
                    ty: Type::INT32,
                    optional: false,
                    has_default: false,
                    variadic: false,
                }],
                returns,
                shared: true,
                span: field.name.span,
                type_params: Vec::new(),
                throws: Vec::new(),
            };
            return Some(self.check_direct_call(expr, &signature));
        }

        let (params, returns): (Vec<(&str, Type)>, Type) = match field.name.name.as_str() {
            "read" => (Vec::new(), t),
            "write" => (vec![("value", t)], Type::VOID),
            "offset" => (vec![("n", Type::INT32)], object),
            "offset_bytes" => (vec![("n", Type::INT32)], object),
            _ => return None,
        };

        self.require_unsafe(expr.span, &format!("`.{}()`", field.name.name));

        let signature = Signature {
            name: field.name.name.clone(),
            params: params
                .into_iter()
                .map(|(name, ty)| ParamInfo {
                    name: name.to_string(),
                    ty,
                    optional: false,
                    has_default: false,
                    variadic: false,
                })
                .collect(),
            returns,
            shared: true,
            span: field.name.span,
            type_params: Vec::new(),
            throws: Vec::new(),
        };
        Some(self.check_direct_call(expr, &signature))
    }

    /// `Weak.from(value)` (roadmap Phase 4e, `fase-4e-weak`, design D1):
    /// constructs a weak handle to `value`'s referent, typed `Weak<T>`
    /// where `T` is `value`'s own type — which must itself be a reference
    /// type. No `unsafe` boundary required (spec: "without requiring an
    /// unsafe boundary"), unlike `Pointer.from`.
    fn check_weak_from(&mut self, expr: &CallExpr) -> Type {
        if expr.args.len() != 1 || expr.args[0].name.is_some() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                "`Weak.from` takes exactly one positional argument",
                format!("received {}", expr.args.len()),
                Some("write `Weak.from(value)`".into()),
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::UNKNOWN;
        }

        let value = &expr.args[0].value;
        let ty = self.check_expr(value);
        if ty.is_unknown() {
            return Type::UNKNOWN;
        }
        if !self.is_reference_type(ty) {
            let name = self.name(ty);
            self.error(
                codes::WEAK_DISALLOWED_REFERENT,
                value.span(),
                format!("`{name}` is not a reference type"),
                "Weak<T> only allows a class or contract instance, since a value type has no identity to observe weakly",
                None,
            );
            return Type::UNKNOWN;
        }

        let id = self.intern_weak_type(ty);
        Type::of(Base::Weak(id))
    }

    /// `.upgrade()` (roadmap Phase 4e, `fase-4e-weak`, design D4) —
    /// `Weak<T>`'s one method: returns the referent when it is still
    /// reachable, `null` otherwise, typed `T?`. `.is_alive` is handled in
    /// `Self::member_type` instead, since it needs no method call. Returns
    /// `None` for a name this receiver has no such method by, so the
    /// generic "unknown member" diagnostic still applies to a typo.
    fn check_weak_method_call(
        &mut self,
        expr: &CallExpr,
        field: &FieldExpr,
        object: Type,
    ) -> Option<Type> {
        let Base::Weak(id) = object.base else {
            return None;
        };
        if field.name.name != "upgrade" {
            return None;
        }
        let referent = self
            .weak_types
            .get(id as usize)
            .copied()
            .unwrap_or(Type::UNKNOWN);

        let signature = Signature {
            name: field.name.name.clone(),
            params: Vec::new(),
            returns: referent.as_nullable(),
            shared: true,
            span: field.name.span,
            type_params: Vec::new(),
            throws: Vec::new(),
        };
        Some(self.check_direct_call(expr, &signature))
    }

    fn check_call(&mut self, expr: &CallExpr, expected: Option<Type>) -> Type {
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
                return self.check_variant_construction(expr, field, enum_id as u32, expected);
            }
        }

        // `Pointer.from(place)` (roadmap Phase 4e, design D1/D8): a static
        // call on the compiler-built-in `Pointer` name, decided here for the
        // same reason `LoadState.Loading(...)` above is — `Pointer` names a
        // type, not a value with a member called `from`.
        if let Expr::Field(field) = &*expr.callee
            && let Expr::Path(base) = &*field.object
            && base.name == "Pointer"
            && field.name.name == "from"
            && self.scopes.lookup(&base.name).is_none()
        {
            return self.check_pointer_from(expr);
        }

        // `Weak.from(value)` (roadmap Phase 4e, `fase-4e-weak`, design D1):
        // same treatment as `Pointer.from(place)` just above — `Weak` names
        // a type, not a value with a member called `from`.
        if let Expr::Field(field) = &*expr.callee
            && let Expr::Path(base) = &*field.object
            && base.name == "Weak"
            && field.name.name == "from"
            && self.scopes.lookup(&base.name).is_none()
        {
            return self.check_weak_from(expr);
        }

        // `u.greeting()` calls a method. It is decided here and not by the
        // parser for the same reason `u.name` is: the shape does not say
        // whether the base is a value with members.
        if let Expr::Field(field) = &*expr.callee
            && !self.variant_accesses.contains(&field.span)
        {
            let object = self.check_expr(&field.object);

            // `myInt.to_string()`: the explicit spelling of the same
            // conversion `println`/interpolation reach implicitly (roadmap
            // Phase 3b, task 8's own follow-up). A native scalar has no
            // method table to resolve a call against — `Base::Class`/
            // `Base::Contract`/etc. already go through `check_method_call_on`
            // below, which finds a real declared method — so this is
            // recognized by name here instead, restricted to exactly the
            // scalars `is_printable` already accepts.
            if field.name.name == "to_string"
                && !field.safe
                && !object.nullable
                && !matches!(
                    object.base,
                    Base::Class(_) | Base::Contract(_) | Base::Param(_) | Base::Instance(_)
                )
                && self.is_printable(object)
            {
                if !expr.args.is_empty() {
                    self.error(
                        codes::WRONG_ARGUMENT_COUNT,
                        expr.span,
                        "`to_string` takes no arguments",
                        format!("received {}", expr.args.len()),
                        None,
                    );
                }
                for arg in &expr.args {
                    self.check_expr(&arg.value);
                }
                return Type::STRING;
            }

            // `result.is_ok()`, `.unwrap()`, … : `Result<T,E>`'s own method
            // API (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 2),
            // recognized structurally by receiver and name, the same way
            // `to_string()` above is — `EnumType` has no method table, and
            // `Result` gets no exception to that rule
            // (`register_native_result_enum`'s doc comment). The generic
            // combinators, `or_throw`, and `get_or_else` are explicitly out
            // of scope for this pass (`fase-4a-errores/proposal.md`: the
            // first four need a per-method type parameter `MethodInfo` does
            // not have, and `get_or_else` needs the closure-typed-parameter
            // syntax decision D9 leaves to a future phase).
            if !field.safe
                && !object.nullable
                && let Base::EnumInstance(inst) = object.base
                && self.native_result == Some(self.enum_instances[inst as usize].enum_id)
            {
                let args = self.enum_instances[inst as usize].args.clone();
                if let Some(ty) = self.check_result_method_call(expr, field, args[0], args[1]) {
                    return ty;
                }
            }

            // `.read()`, `.write(v)`, `.offset(n)`, `.offset_bytes(n)`
            // (roadmap Phase 4e, design D8) — `Pointer<T>`'s own closed
            // operation set, recognized structurally by receiver type, the
            // same way `Result<T,E>`'s is just above. `.is_null` is not
            // here: it needs no `unsafe` (spec), so it is handled as a
            // member read in `Self::check_field` instead.
            if !field.safe
                && !object.nullable
                && let Base::Pointer(_) = object.base
                && let Some(ty) = self.check_pointer_method_call(expr, field, object)
            {
                return ty;
            }

            // `.upgrade()` (roadmap Phase 4e, `fase-4e-weak`, design D4) —
            // `Weak<T>`'s only method beyond `.is_alive` (handled as a member
            // read in `Self::member_type` instead, since it needs no method
            // call), recognized structurally by receiver type the same way
            // `Pointer<T>`'s own methods are just above.
            if !field.safe
                && !object.nullable
                && let Base::Weak(_) = object.base
                && let Some(ty) = self.check_weak_method_call(expr, field, object)
            {
                return ty;
            }

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
                // `Void` has no value to be absent (`Void?` is rejected as a
                // type above, for the same reason), so a `Void`-returning
                // method reached through `?.` stays `Void`: the call runs
                // or does not, but there is nothing to wrap either way.
                let ty = self.check_method_call_on(object.without_null(), expr, field);
                return if ty.is_unknown() || ty == Type::VOID {
                    ty
                } else {
                    ty.as_nullable()
                };
            }

            if matches!(
                object.base,
                Base::Class(_) | Base::Contract(_) | Base::Param(_) | Base::Instance(_)
            ) {
                if field.safe {
                    self.reject_redundant_safe(object, field.object.span());
                }
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

            // Calling an `extern "C" fn` (roadmap Phase 4e, design D3,
            // `ADR-015`) — every call is treated as potentially
            // irreversible, so it needs both `unsafe` and a nested `commit`,
            // with no purity inference.
            if let Some(signature) = self.externs.get(&declared).cloned() {
                if self.unsafe_depth == 0 || self.commit_depth == 0 {
                    self.error(
                        codes::EXTERN_CALL_OUTSIDE_UNSAFE_COMMIT,
                        expr.span,
                        format!("calling `{}` needs `unsafe` and `commit`", callee.name),
                        "every extern call is treated as potentially irreversible (ADR-015), with no purity inference",
                        Some("wrap it in `unsafe { commit { ... } }`".into()),
                    );
                }
                let synthetic = Signature {
                    name: callee.name.clone(),
                    params: signature
                        .params
                        .iter()
                        .map(|&ty| ParamInfo {
                            name: "_".to_string(),
                            ty,
                            optional: false,
                            has_default: false,
                            variadic: false,
                        })
                        .collect(),
                    returns: signature.returns,
                    shared: true,
                    span: callee.span,
                    type_params: Vec::new(),
                    throws: Vec::new(),
                };
                return self.check_direct_call(expr, &synthetic);
            }

            if let Some(signature) = self.functions.get(&declared).cloned() {
                self.require_visible(signature.span, signature.shared, callee, "function");
                return self.check_direct_call(expr, &signature);
            }

            // `Float(3 / 4)` establishes a deep contextual conversion domain
            // over the arithmetic tree directly inside it, and `String("x=" + 42)`
            // does the same over concatenation (roadmap Phase 3b, task 7) —
            // decided here, ahead of class construction, since a native
            // scalar name can never collide with one (type names are
            // reserved).
            if let Some(target) = Type::from_name(&callee.name)
                && matches!(target.base, Base::Int(_) | Base::Float(_) | Base::String)
            {
                return self.check_context_conversion(target, expr);
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

    /// `Target(expr)`, a deep contextual conversion (`ZIRK_LANGUAGE_SPEC.md`
    /// section 3, roadmap Phase 3b task 7): `Float(3 / 4)` converts `3` and
    /// `4` to `Float` *before* dividing, yielding `0.75`, not `(3 / 4) as
    /// Float` which would divide as `Int32` first and convert the
    /// already-truncated `0` after. `String("x=" + 42)` is the same idea
    /// over concatenation.
    ///
    /// Exactly one positional argument — the syntax reads like a
    /// constructor call, and a constructor takes what it is given, not a
    /// list of things to convert independently.
    fn check_context_conversion(&mut self, target: Type, expr: &CallExpr) -> Type {
        if expr.args.len() != 1 || expr.args[0].name.is_some() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                format!("`{}` takes exactly one argument", self.name(target)),
                "a contextual conversion converts one expression",
                None,
            );
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return target;
        }

        self.check_context_tree(target, &expr.args[0].value);
        target
    }

    /// Walks an operand tree deciding, at each node, whether it still
    /// belongs to `target`'s own compatible operator family (arithmetic for
    /// a numeric target, `+` alone for `String` — repeating a `String` with
    /// `*` is already native and needs no context) — recursing while it
    /// does, and treating anything else as a boundary: checked with its own
    /// type, then required to convert into `target` at that point.
    ///
    /// A boundary is also where this naturally stops at a call: a function
    /// or method call is never one of the recognized operator shapes, so it
    /// is always a leaf here — the context never reaches into a called
    /// function's body, without needing a special case for it (task 7.3).
    /// The tree itself is never rewritten, only read twice (once here to
    /// check it, again in `zirk-ir/lower.rs`'s `lower_context_tree` to
    /// lower it) — task 7.3's "does not mutate operands" the same way.
    fn check_context_tree(&mut self, target: Type, expr: &Expr) {
        let numeric = matches!(target.base, Base::Int(_) | Base::Float(_));
        let is_string = matches!(target.base, Base::String);

        let compatible_op = |op: BinaryOp| {
            (numeric
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ))
                || (is_string && op == BinaryOp::Add)
        };

        match expr {
            Expr::Binary(b) if compatible_op(b.op) => {
                self.check_context_tree(target, &b.left);
                self.check_context_tree(target, &b.right);
            }
            Expr::Unary(u) if numeric && u.op == UnaryOp::Neg => {
                self.check_context_tree(target, &u.operand);
            }
            _ => {
                let actual = self.check_expr(expr);
                self.expect_context_convertible(target, actual, expr.span());
            }
        }
    }

    /// Whether a leaf's own type can convert into a contextual target —
    /// always unchecked where it applies, the same as an explicit `as`
    /// (roadmap Phase 3b, task 4.3/7.2): a numeric leaf reaches any other
    /// numeric target, and any printable value reaches `String` through
    /// `to_string()` (task 8).
    fn expect_context_convertible(&mut self, target: Type, actual: Type, span: Span) {
        if actual.is_unknown() || target == actual {
            return;
        }
        let numeric_target = matches!(target.base, Base::Int(_) | Base::Float(_));
        let numeric_actual = matches!(actual.base, Base::Int(_) | Base::Float(_));
        if !actual.nullable && numeric_target && numeric_actual {
            return;
        }
        if !actual.nullable && matches!(target.base, Base::String) && self.is_printable(actual) {
            return;
        }

        let target_name = self.name(target);
        let actual_name = self.name(actual);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("cannot convert {actual_name} into the `{target_name}` context"),
            format!("`{target_name}(...)` requires every leaf to reach {target_name}"),
            None,
        );
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
        self.check_direct_call_with_subst_seeded(expr, signature, HashMap::new())
    }

    /// [`Self::check_direct_call_with_subst`], pre-populating the inferred
    /// substitution with `seed` before arguments are considered — what lets
    /// [`Self::check_variant_construction`] resolve a type parameter no
    /// argument determines (`Result.Ok(v)`'s own `E`) from the surrounding
    /// expected type instead. An argument that disagrees with the seed is
    /// still a real error: seeding happens before the argument loop, so the
    /// existing "cannot infer, would have to be both X and Y" conflict check
    /// runs unchanged.
    fn check_direct_call_with_subst_seeded(
        &mut self,
        expr: &CallExpr,
        signature: &Signature,
        seed: HashMap<u32, Type>,
    ) -> (Type, HashMap<u32, Type>) {
        let name = signature.name.clone();
        let slots = self.match_arguments(expr, signature);
        let substitution = self.infer_type_params(expr.span, &name, signature, &slots, seed);

        // A call to a `throws` function/method propagates its declared
        // types into whatever this point in the program can itself throw
        // (roadmap Phase 4b) — the enclosing `try`'s catches or the
        // enclosing function's own `throws` resolve it, the same way a
        // `throw` statement's own type does.
        self.pending_throws.extend(signature.throws.iter().copied());

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
    /// arguments given to it (roadmap task 7.6), pre-populated with `seed`
    /// (roadmap Phase 4a — see [`Self::check_direct_call_with_subst_seeded`]).
    ///
    /// Beyond `seed`, only from arguments: the receiver and the surrounding
    /// callable context that `zirk-generics`' spec also lists are still not
    /// consulted. A parameter neither an argument nor the seed determines is
    /// left unsolved and reported rather than guessed — "SHALL fail rather
    /// than choose an arbitrary solution".
    fn infer_type_params(
        &mut self,
        call_span: Span,
        name: &str,
        signature: &Signature,
        slots: &[ArgSlot],
        seed: HashMap<u32, Type>,
    ) -> HashMap<u32, Type> {
        if signature.type_params.is_empty() {
            return HashMap::new();
        }

        let mut substitution: HashMap<u32, Type> = seed;
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
                    "no argument or expected type determines it, and an explicit type argument is not consulted yet",
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

        // A named function or a capture-less lambda is already structurally
        // uniform (design D12: an empty `ClosureLayout`, `{function
        // pointer}` and nothing else), so any shape-compatible one of those
        // freely interchanges with another at a `Fn(...) => R` position.
        //
        // A capturing closure's captures are part of its representation
        // (D10), so two differently-captured closures are never
        // interchangeable this way — only the exact same literal, written
        // directly at the position, may occupy it (design D14); the callers
        // that recognize that case (`Self::check_let`, `Self::check_return`,
        // `Self::check_assign`) accept it themselves before ever reaching
        // this generic check, so a capturing closure arriving here always
        // means either a second, different one targets an
        // already-committed position, or one reached a position D14 does
        // not cover (a field, a parameter, an argument through a variable).
        // Both need the captures boxed behind a uniform representation this
        // pass does not build (design D13, deferred).
        if matches!(expected.base, Base::Function(_)) && matches!(actual.base, Base::Function(_)) {
            if self.callable_assignable(expected, actual) {
                let Base::Function(actual_id) = actual.base else {
                    unreachable!("checked by the guard above")
                };
                if self.capturing_lambda_span(actual_id).is_none() {
                    return;
                }
                self.error(
                    codes::AMBIGUOUS_CAPTURING_CALLABLE,
                    span,
                    "this position cannot hold this closure",
                    "a capturing closure only satisfies a callable type when its own literal is written directly at that position; a different closure — or one reached through a variable — would need its captures boxed behind a uniform representation, which is not implemented yet",
                    Some("general callable-type polymorphism across captures is future work (design D13/D14 of fase-4d-callables)".into()),
                );
                return;
            }
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

/// A textual key identifying the place a simultaneous-assignment destination
/// names, for `Checker::check_multi_assign`'s cross-destination duplicate
/// check (design D4). Two destinations with the same key are provably the
/// same place; `None` means the base is not a simple enough shape (`Path`,
/// `This`, or a chain of safe-free `Field`s over one of those) to prove
/// either way, so it is never reported as a duplicate.
fn assign_target_key(target: &AssignTarget) -> Option<String> {
    match target {
        AssignTarget::Name(name) => Some(format!("name:{}", name.name)),
        AssignTarget::Field(field) => {
            let base = expr_place_key(&field.object)?;
            Some(format!("{base}.{}", field.name.name))
        }
        // Conservative, matching this function's own doc comment: an index
        // expression is not a simple enough shape to prove two destinations
        // the same or different, so it is never reported as a duplicate.
        AssignTarget::Index(_) => None,
    }
}

/// The place an expression names, for `Self::assign_target_key`'s base.
fn expr_place_key(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(ident) => Some(format!("name:{}", ident.name)),
        Expr::This(_) => Some("this".to_string()),
        Expr::Field(field) if !field.safe => {
            let base = expr_place_key(&field.object)?;
            Some(format!("{base}.{}", field.name.name))
        }
        _ => None,
    }
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
            Stmt::MultiAssign(assign) => {
                for target in &assign.targets {
                    if let AssignTarget::Field(field) = target
                        && matches!(&*field.object, Expr::This(_))
                    {
                        found.insert(field.name.name.clone());
                    }
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
