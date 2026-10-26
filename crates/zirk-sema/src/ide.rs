//! Compiler-owned semantic facts consumed by editor tooling.
//!
//! Identities are scoped to one analysis snapshot. Consumers must not retain
//! a [`SymbolId`] after the source snapshot changes; they should resolve the
//! position again against the new [`SemanticIndex`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use zirk_diagnostics::Span;

static NEXT_SNAPSHOT_ID: AtomicU64 = AtomicU64::new(1);

/// Opaque identity for a declaration in one semantic snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(u32);

impl SymbolId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// A declaration category exposed to language tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticSymbolKind {
    Function,
    Method,
    Variable,
    Parameter,
    Field,
    Type,
    Class,
    Contract,
    Enum,
    EnumVariant,
    TypeAlias,
    TypeParameter,
    Import,
    Module,
}

/// Visibility as understood by the language checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticVisibility {
    Public,
    Module,
    Private,
    Local,
}

/// Semantic role played by a resolved source occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceRole {
    Read,
    Write,
    Call,
    MemberRead,
    MemberCall,
    Import,
    PatternBinding,
    TypeUse,
}

/// A declaration record stored in the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticSymbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SemanticSymbolKind,
    /// Span of the full declaration when available.
    pub declaration_span: Span,
    /// Exact name token used for selection and edits.
    pub selection_span: Span,
    /// Containing function or method declaration for locals and parameters.
    pub owner_span: Option<Span>,
    /// Human-readable checked type or signature, if known.
    pub type_text: Option<String>,
    pub visibility: SemanticVisibility,
    pub is_standard_library: bool,
}

/// A source occurrence whose target was determined by the checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticReference {
    pub span: Span,
    pub target: SymbolId,
    pub role: ReferenceRole,
    /// Local import binding used for rename while navigation keeps following
    /// the compiler-resolved exported declaration.
    pub rename_target: Option<SymbolId>,
}

/// Intermediate declaration facts used while a snapshot is assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticSymbolData {
    pub name: String,
    pub kind: SemanticSymbolKind,
    pub declaration_span: Span,
    pub selection_span: Span,
    pub owner_span: Option<Span>,
    pub type_text: Option<String>,
    pub visibility: SemanticVisibility,
    pub is_standard_library: bool,
}

/// A resolved occurrence before snapshot-local identities are assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnresolvedSemanticReference {
    pub span: Span,
    pub target_span: Span,
    pub role: ReferenceRole,
    pub rename_target_span: Option<Span>,
}

/// Immutable semantic data for one compiler analysis.
#[derive(Debug, Clone, Default)]
pub struct SemanticIndex {
    snapshot_id: u64,
    symbols: Vec<SemanticSymbol>,
    expression_types: Vec<(Span, String)>,
    references: Vec<SemanticReference>,
    symbol_at_span: HashMap<Span, SymbolId>,
    references_by_symbol: HashMap<SymbolId, Vec<usize>>,
}

impl SemanticIndex {
    pub(crate) fn from_facts(
        mut declarations: Vec<SemanticSymbolData>,
        unresolved_references: Vec<UnresolvedSemanticReference>,
        mut expression_types: Vec<(Span, String)>,
    ) -> Self {
        declarations.sort_by_key(|declaration| {
            (
                declaration.selection_span.file.0,
                declaration.selection_span.start,
                declaration.selection_span.end,
                declaration.kind as u8,
                declaration.name.clone(),
            )
        });
        declarations.dedup_by(|left, right| {
            left.selection_span == right.selection_span && left.kind == right.kind
        });

        let mut symbol_at_span = HashMap::new();
        let symbols: Vec<_> = declarations
            .into_iter()
            .enumerate()
            .map(|(index, declaration)| {
                let id = SymbolId(index as u32);
                symbol_at_span.insert(declaration.selection_span, id);
                symbol_at_span.insert(declaration.declaration_span, id);
                SemanticSymbol {
                    id,
                    name: declaration.name,
                    kind: declaration.kind,
                    declaration_span: declaration.declaration_span,
                    selection_span: declaration.selection_span,
                    owner_span: declaration.owner_span,
                    type_text: declaration.type_text,
                    visibility: declaration.visibility,
                    is_standard_library: declaration.is_standard_library,
                }
            })
            .collect();

        let mut references = Vec::new();
        let mut references_by_symbol: HashMap<SymbolId, Vec<usize>> = HashMap::new();
        for unresolved in unresolved_references {
            let Some(target) = symbol_at_span.get(&unresolved.target_span).copied() else {
                continue;
            };
            let index = references.len();
            references.push(SemanticReference {
                span: unresolved.span,
                target,
                role: unresolved.role,
                rename_target: unresolved
                    .rename_target_span
                    .and_then(|span| symbol_at_span.get(&span).copied()),
            });
            references_by_symbol.entry(target).or_default().push(index);
        }

        expression_types.sort_by_key(|(span, _)| (span.file.0, span.start, span.end));
        Self {
            snapshot_id: NEXT_SNAPSHOT_ID.fetch_add(1, Ordering::Relaxed),
            symbols,
            expression_types,
            references,
            symbol_at_span,
            references_by_symbol,
        }
    }

    /// Unique generation for this checked snapshot, useful for stale-result
    /// checks. It is not a document version and must not be persisted.
    pub fn snapshot_id(&self) -> u64 {
        self.snapshot_id
    }

    pub fn symbols(&self) -> &[SemanticSymbol] {
        &self.symbols
    }

    /// Checked type at a source position, preferring the narrowest expression fact.
    pub fn expression_type_at(&self, file: zirk_diagnostics::FileId, offset: u32) -> Option<&str> {
        self.expression_types
            .iter()
            .filter(|(span, _)| span.file == file && span.start <= offset && offset <= span.end)
            .min_by_key(|(span, _)| span.end.saturating_sub(span.start))
            .map(|(_, ty)| ty.as_str())
    }

    pub fn references(&self) -> &[SemanticReference] {
        &self.references
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&SemanticSymbol> {
        self.symbols
            .get(id.0 as usize)
            .filter(|symbol| symbol.id == id)
    }

    pub fn symbol_at_declaration(&self, span: Span) -> Option<&SemanticSymbol> {
        self.symbol_at_span
            .get(&span)
            .and_then(|id| self.symbol(*id))
    }

    /// References resolving to exactly the selected declaration identity.
    pub fn references_to(&self, id: SymbolId) -> impl Iterator<Item = &SemanticReference> {
        self.references_by_symbol
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|index| self.references.get(*index))
    }

    /// Occurrences that should be edited when renaming a declaration,
    /// including imported aliases whose navigation target is the export.
    pub fn rename_references_to(&self, id: SymbolId) -> impl Iterator<Item = &SemanticReference> {
        self.references.iter().filter(move |reference| {
            reference.rename_target == Some(id)
                || (reference.target == id
                    && reference
                        .rename_target
                        .is_none_or(|rename_target| rename_target == id))
        })
    }

    pub fn symbol_at_position(
        &self,
        file: zirk_diagnostics::FileId,
        offset: u32,
    ) -> Option<&SemanticSymbol> {
        self.symbols
            .iter()
            .filter(|symbol| {
                symbol.selection_span.file == file
                    && symbol.selection_span.start <= offset
                    && offset <= symbol.selection_span.end
            })
            .min_by_key(|symbol| symbol.selection_span.len())
    }

    pub fn reference_at_position(
        &self,
        file: zirk_diagnostics::FileId,
        offset: u32,
    ) -> Option<&SemanticReference> {
        self.references
            .iter()
            .filter(|reference| {
                reference.span.file == file
                    && reference.span.start <= offset
                    && offset <= reference.span.end
            })
            .min_by_key(|reference| reference.span.len())
    }
}
