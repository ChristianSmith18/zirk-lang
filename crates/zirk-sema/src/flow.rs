//! Flow-sensitive types are overlays on declarations, never replacements for
//! the types recorded on bindings and properties.

use std::collections::HashMap;

use crate::types::Type;
use zirk_diagnostics::Span;

/// A local or property path rooted at one binding declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FlowPath {
    declaration: Span,
    members: Vec<String>,
}

impl FlowPath {
    pub(crate) fn local(declaration: Span) -> Self {
        Self {
            declaration,
            members: Vec::new(),
        }
    }

    pub(crate) fn project(&self, member: impl Into<String>) -> Self {
        let mut path = self.clone();
        path.members.push(member.into());
        path
    }

    pub(crate) fn root(&self) -> Self {
        Self {
            declaration: self.declaration,
            members: Vec::new(),
        }
    }

    pub(crate) fn last_member(&self) -> Option<&str> {
        self.members.last().map(String::as_str)
    }

    fn contains(&self, other: &Self) -> bool {
        self.declaration == other.declaration && other.members.starts_with(&self.members)
    }
}

/// Declared types remain stable while branch-local refinements come and go.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FlowState {
    declared: HashMap<FlowPath, Type>,
    refined: HashMap<FlowPath, Type>,
}

impl FlowState {
    pub(crate) fn declare(&mut self, path: FlowPath, ty: Type) {
        self.declared.insert(path, ty);
    }

    pub(crate) fn declared(&self, path: &FlowPath) -> Option<Type> {
        self.declared.get(path).copied()
    }

    pub(crate) fn declare_if_absent(&mut self, path: FlowPath, ty: Type) {
        self.declared.entry(path).or_insert(ty);
    }

    pub(crate) fn current(&self, path: &FlowPath) -> Option<Type> {
        self.refined
            .get(path)
            .or_else(|| self.declared.get(path))
            .copied()
    }

    #[allow(dead_code)] // Consumed by the following narrowing implementation tasks.
    pub(crate) fn refine(&mut self, path: FlowPath, ty: Type) {
        self.refined.insert(path, ty);
    }

    /// A write to a place invalidates facts about that place and anything
    /// projected from it. Facts about siblings and enclosing bindings remain
    /// valid.
    pub(crate) fn invalidate(&mut self, path: &FlowPath) {
        self.refined
            .retain(|candidate, _| !path.contains(candidate));
    }

    pub(crate) fn invalidate_preserving(&mut self, path: &FlowPath, preserved: &[FlowPath]) {
        self.refined.retain(|candidate, _| {
            !path.contains(candidate) || preserved.iter().any(|stable| stable == candidate)
        });
    }

    pub(crate) fn clear_refinements(&mut self) {
        self.refined.clear();
    }

    pub(crate) fn read(&self, path: &FlowPath, declared: Type) -> Type {
        self.refined
            .get(path)
            .copied()
            .or_else(|| self.declared(path))
            .unwrap_or(declared)
    }

    pub(crate) fn checkpoint(&self) -> Self {
        self.clone()
    }

    pub(crate) fn restore(&mut self, checkpoint: Self) {
        *self = checkpoint;
    }

    pub(crate) fn paths(&self) -> Vec<FlowPath> {
        let mut paths = self
            .declared
            .keys()
            .chain(self.refined.keys())
            .cloned()
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zirk_diagnostics::{FileId, Span};

    #[test]
    fn refinement_overlays_but_never_replaces_declared_local_or_property_types() {
        let root = FlowPath::local(Span {
            file: FileId::ENTRY,
            start: 10,
            end: 15,
        });
        let property = root.project("kind");
        let mut state = FlowState::default();
        state.declare(root.clone(), Type::BOOLEAN);
        state.declare(property.clone(), Type::STRING);

        let before_branch = state.checkpoint();
        state.refine(root.clone(), Type::of(crate::types::Base::Singleton(1)));
        state.refine(property.clone(), Type::of(crate::types::Base::Singleton(2)));
        assert_eq!(
            state.read(&root, Type::UNKNOWN),
            Type::of(crate::types::Base::Singleton(1))
        );
        assert_eq!(state.declared(&root), Some(Type::BOOLEAN));
        assert_eq!(state.declared(&property), Some(Type::STRING));

        state.restore(before_branch);
        assert_eq!(state.read(&root, Type::UNKNOWN), Type::BOOLEAN);
        assert_eq!(state.read(&property, Type::UNKNOWN), Type::STRING);
    }

    #[test]
    fn writes_invalidate_the_path_and_descendants_but_can_preserve_stable_facts() {
        let root = FlowPath::local(Span {
            file: FileId::ENTRY,
            start: 20,
            end: 25,
        });
        let stable = root.project("kind");
        let mutable = root.project("payload");
        let nested = mutable.project("value");
        let mut state = FlowState::default();
        for path in [&root, &stable, &mutable, &nested] {
            state.refine(path.clone(), Type::BOOLEAN);
        }

        state.invalidate_preserving(&root, std::slice::from_ref(&stable));
        assert_eq!(state.current(&root), None);
        assert_eq!(state.current(&stable), Some(Type::BOOLEAN));
        assert_eq!(state.current(&mutable), None);
        assert_eq!(state.current(&nested), None);
    }
}
