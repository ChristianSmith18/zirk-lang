//! Scopes and name resolution.
//!
//! `ZIRK_LANGUAGE_SPEC.md` section 2 defines block, function, file and module
//! scopes. This phase implements the first two: modules arrive in Phase 2.

use crate::types::Type;
use zirk_ast::Mutability;
use zirk_diagnostics::Span;

/// A variable binding in a scope.
#[derive(Debug, Clone)]
pub struct Binding {
    pub name: String,
    pub ty: Type,
    pub mutability: Mutability,
    /// Where it was declared, so a diagnostic can point back at it.
    pub span: Span,
    /// Whether the variable already holds a value.
    ///
    /// Flow analysis prevents reading one that is not yet available
    /// (`LANGUAGE_SPEC` section 2).
    pub initialized: bool,
}

/// One nesting level.
#[derive(Debug, Default)]
struct Level {
    bindings: Vec<Binding>,
    /// Whether this level starts a new function body.
    ///
    /// A name found beyond a barrier is not a local of the function being
    /// checked: it is a capture. This is the whole mechanism that tells a
    /// closure what it closes over.
    barrier: bool,
}

/// A stack of nested scopes.
///
/// A `Vec` of levels is used rather than a map with qualified keys because
/// shadowing is resolved by walking outwards from the innermost scope, and
/// leaving a block is just truncating the stack.
#[derive(Debug, Default)]
pub struct Scopes {
    levels: Vec<Level>,
}

/// The outcome of resolving a name.
#[derive(Debug, Clone)]
pub struct Resolution {
    pub binding: Binding,
    /// Whether the name lives outside the innermost function body.
    pub captured: bool,
}

impl Scopes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self) {
        self.levels.push(Level::default());
    }

    /// Opens a scope that begins a new function body.
    pub fn push_function(&mut self) {
        self.levels.push(Level {
            bindings: Vec::new(),
            barrier: true,
        });
    }

    pub fn pop(&mut self) {
        self.levels.pop();
    }

    /// Declares a binding in the innermost scope.
    ///
    /// An inner block may shadow an outer name: `LANGUAGE_SPEC` section 2 allows
    /// it, and the linter reports confusing shadowing as a warning in a later
    /// phase.
    pub fn declare(&mut self, binding: Binding) {
        if let Some(level) = self.levels.last_mut() {
            level.bindings.push(binding);
        }
    }

    /// Looks a name up from the innermost scope outwards.
    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        self.levels
            .iter()
            .rev()
            .find_map(|level| level.bindings.iter().rev().find(|b| b.name == name))
    }

    /// Looks a name up, reporting whether reaching it crossed a function body.
    pub fn resolve(&self, name: &str) -> Option<Resolution> {
        let mut crossed = false;

        for level in self.levels.iter().rev() {
            if let Some(binding) = level.bindings.iter().rev().find(|b| b.name == name) {
                return Some(Resolution {
                    binding: binding.clone(),
                    captured: crossed,
                });
            }
            if level.barrier {
                crossed = true;
            }
        }

        None
    }

    /// Marks a variable as holding a value.
    pub fn mark_initialized(&mut self, name: &str) {
        for level in self.levels.iter_mut().rev() {
            if let Some(binding) = level.bindings.iter_mut().rev().find(|b| b.name == name) {
                binding.initialized = true;
                return;
            }
        }
    }
}

/// A function signature, for checking calls.
///
/// Carries the whole parameter shape, not just the types: matching a named or
/// omitted argument to its position needs the names and the defaults.
#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub returns: Type,
    /// Marked `share`, so files that import it may name it.
    pub shared: bool,
    pub span: Span,
}

impl Signature {
    /// The types of the parameters, in declaration order.
    pub fn param_types(&self) -> Vec<Type> {
        self.params.iter().map(|p| p.ty).collect()
    }

    /// The number of leading parameters that must receive an argument.
    pub fn required_count(&self) -> usize {
        self.params
            .iter()
            .filter(|p| !p.optional && !p.variadic && !p.has_default)
            .count()
    }

    pub fn variadic(&self) -> Option<&ParamInfo> {
        self.params.last().filter(|p| p.variadic)
    }
}

/// One parameter of a signature.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamInfo {
    pub name: String,
    pub ty: Type,
    pub optional: bool,
    pub has_default: bool,
    pub variadic: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Span = Span::new(0, 1);

    fn binding(name: &str, ty: Type) -> Binding {
        Binding {
            name: name.into(),
            ty,
            mutability: Mutability::Mutable,
            span: S,
            initialized: true,
        }
    }

    #[test]
    fn a_declared_name_resolves() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.declare(binding("x", Type::INT32));

        assert_eq!(scopes.lookup("x").map(|b| b.ty), Some(Type::INT32));
    }

    #[test]
    fn an_undeclared_name_does_not_resolve() {
        let mut scopes = Scopes::new();
        scopes.push();

        assert!(scopes.lookup("x").is_none());
    }

    #[test]
    fn an_inner_block_sees_the_outer_scope() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.declare(binding("outer", Type::INT32));
        scopes.push();

        assert!(scopes.lookup("outer").is_some());
    }

    #[test]
    fn leaving_a_block_removes_its_names() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.push();
        scopes.declare(binding("inner", Type::INT32));
        scopes.pop();

        assert!(scopes.lookup("inner").is_none());
    }

    #[test]
    fn an_inner_block_shadows_the_outer_name() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.declare(binding("x", Type::INT32));
        scopes.push();
        scopes.declare(binding("x", Type::STRING));

        assert_eq!(scopes.lookup("x").map(|b| b.ty), Some(Type::STRING));

        scopes.pop();
        assert_eq!(scopes.lookup("x").map(|b| b.ty), Some(Type::INT32));
    }

    #[test]
    fn a_variable_can_be_marked_as_initialized() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.declare(Binding {
            initialized: false,
            ..binding("x", Type::INT32)
        });

        assert_eq!(scopes.lookup("x").map(|b| b.initialized), Some(false));
        scopes.mark_initialized("x");
        assert_eq!(scopes.lookup("x").map(|b| b.initialized), Some(true));
    }
}
