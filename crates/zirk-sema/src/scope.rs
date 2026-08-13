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

/// A stack of nested scopes.
///
/// A `Vec` of levels is used rather than a map with qualified keys because
/// shadowing is resolved by walking outwards from the innermost scope, and
/// leaving a block is just truncating the stack.
#[derive(Debug, Default)]
pub struct Scopes {
    levels: Vec<Vec<Binding>>,
}

impl Scopes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self) {
        self.levels.push(Vec::new());
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
            level.push(binding);
        }
    }

    /// Looks a name up from the innermost scope outwards.
    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        self.levels
            .iter()
            .rev()
            .find_map(|level| level.iter().rev().find(|b| b.name == name))
    }

    /// Marks a variable as holding a value.
    pub fn mark_initialized(&mut self, name: &str) {
        for level in self.levels.iter_mut().rev() {
            if let Some(binding) = level.iter_mut().rev().find(|b| b.name == name) {
                binding.initialized = true;
                return;
            }
        }
    }
}

/// A function signature, for checking calls.
#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub params: Vec<Type>,
    pub returns: Type,
    pub span: Span,
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
        scopes.declare(binding("x", Type::Int32));

        assert_eq!(scopes.lookup("x").map(|b| b.ty), Some(Type::Int32));
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
        scopes.declare(binding("outer", Type::Int32));
        scopes.push();

        assert!(scopes.lookup("outer").is_some());
    }

    #[test]
    fn leaving_a_block_removes_its_names() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.push();
        scopes.declare(binding("inner", Type::Int32));
        scopes.pop();

        assert!(scopes.lookup("inner").is_none());
    }

    #[test]
    fn an_inner_block_shadows_the_outer_name() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.declare(binding("x", Type::Int32));
        scopes.push();
        scopes.declare(binding("x", Type::String));

        assert_eq!(scopes.lookup("x").map(|b| b.ty), Some(Type::String));

        scopes.pop();
        assert_eq!(scopes.lookup("x").map(|b| b.ty), Some(Type::Int32));
    }

    #[test]
    fn a_variable_can_be_marked_as_initialized() {
        let mut scopes = Scopes::new();
        scopes.push();
        scopes.declare(Binding {
            initialized: false,
            ..binding("x", Type::Int32)
        });

        assert_eq!(scopes.lookup("x").map(|b| b.initialized), Some(false));
        scopes.mark_initialized("x");
        assert_eq!(scopes.lookup("x").map(|b| b.initialized), Some(true));
    }
}
