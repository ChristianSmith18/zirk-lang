//! Deterministic resource accounting for compile-time type computations.
//!
//! Callers attach the returned limit error to the source span that triggered
//! the operation. Counters are intentionally monotone within one evaluation,
//! relation query, or expansion so retry paths cannot evade a bound.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetKind {
    ConstantFuel,
    ConstantCallDepth,
    AggregateDepth,
    CanonicalValueBytes,
    RelationPairVisits,
    RelationDepth,
    NormalizationMembers,
    TemplateMembers,
    TemplateSourceBytes,
    FlowIterations,
}

impl BudgetKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ConstantFuel => "constant evaluation fuel",
            Self::ConstantCallDepth => "constant call depth",
            Self::AggregateDepth => "constant aggregate depth",
            Self::CanonicalValueBytes => "canonical constant size",
            Self::RelationPairVisits => "type relation visits",
            Self::RelationDepth => "type relation depth",
            Self::NormalizationMembers => "normalized type members",
            Self::TemplateMembers => "template union members",
            Self::TemplateSourceBytes => "template expansion bytes",
            Self::FlowIterations => "flow analysis iterations",
        }
    }

    pub const fn limit(self) -> u64 {
        match self {
            Self::ConstantFuel => 100_000,
            // Debug builds use comparatively large evaluator frames. Keep
            // recursive constant calls below the default Rust test-thread
            // stack limit rather than relying on the process main-thread stack.
            Self::ConstantCallDepth => 32,
            Self::AggregateDepth => 64,
            Self::CanonicalValueBytes => 1_048_576,
            Self::RelationPairVisits => 100_000,
            Self::RelationDepth => 256,
            // Intersection contradiction and subtype absorption proofs still
            // compare members pairwise. This bound keeps that work below
            // 65,536 pair checks per normalization operation.
            Self::NormalizationMembers => 256,
            Self::TemplateMembers => 10_000,
            Self::TemplateSourceBytes => 1_048_576,
            Self::FlowIterations => 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetExceeded {
    pub kind: BudgetKind,
    pub limit: u64,
    pub attempted: u64,
}

impl std::fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} exceeded limit {} (attempted {})",
            self.kind.label(),
            self.limit,
            self.attempted
        )
    }
}

impl BudgetExceeded {
    /// Builds the stable semantic diagnostic; the caller attaches the source
    /// location and snippet from the operation that exhausted the budget.
    pub fn diagnostic(self) -> zirk_diagnostics::Diagnostic {
        zirk_diagnostics::Diagnostic::error(crate::codes::TYPE_COMPUTATION_LIMIT, self.to_string())
            .with_cause(format!(
                "{} exceeded its compile-time resource budget",
                self.kind.label()
            ))
    }
}

#[derive(Debug, Default, Clone)]
pub struct TypeComputationBudget {
    constant_fuel: u64,
    constant_call_depth: u64,
    aggregate_depth: u64,
    canonical_value_bytes: u64,
    relation_pair_visits: u64,
    relation_depth: u64,
    normalization_members: u64,
    template_members: u64,
    template_source_bytes: u64,
    flow_iterations: u64,
}

impl TypeComputationBudget {
    pub fn charge(&mut self, kind: BudgetKind, amount: u64) -> Result<(), BudgetExceeded> {
        let usage = match kind {
            BudgetKind::ConstantFuel => &mut self.constant_fuel,
            BudgetKind::ConstantCallDepth => &mut self.constant_call_depth,
            BudgetKind::AggregateDepth => &mut self.aggregate_depth,
            BudgetKind::CanonicalValueBytes => &mut self.canonical_value_bytes,
            BudgetKind::RelationPairVisits => &mut self.relation_pair_visits,
            BudgetKind::RelationDepth => &mut self.relation_depth,
            BudgetKind::NormalizationMembers => &mut self.normalization_members,
            BudgetKind::TemplateMembers => &mut self.template_members,
            BudgetKind::TemplateSourceBytes => &mut self.template_source_bytes,
            BudgetKind::FlowIterations => &mut self.flow_iterations,
        };
        let limit = kind.limit();
        let attempted = usage.saturating_add(amount);
        if attempted > limit {
            return Err(BudgetExceeded {
                kind,
                limit,
                attempted,
            });
        }
        *usage = attempted;
        Ok(())
    }

    pub fn release_depth(&mut self, kind: BudgetKind) {
        let depth = match kind {
            BudgetKind::ConstantCallDepth => &mut self.constant_call_depth,
            BudgetKind::AggregateDepth => &mut self.aggregate_depth,
            BudgetKind::RelationDepth => &mut self.relation_depth,
            _ => return,
        };
        *depth = depth.saturating_sub(1);
    }

    /// Runs one recursive operation while guaranteeing its depth counter is
    /// restored on both success and failure.
    pub fn with_depth<T, E>(
        &mut self,
        kind: BudgetKind,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, DepthBudgetError<E>> {
        if !matches!(
            kind,
            BudgetKind::ConstantCallDepth | BudgetKind::AggregateDepth | BudgetKind::RelationDepth
        ) {
            return Err(DepthBudgetError::InvalidDimension(kind));
        }
        self.charge(kind, 1).map_err(DepthBudgetError::Exceeded)?;
        let result = operation(self);
        self.release_depth(kind);
        result.map_err(DepthBudgetError::Operation)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DepthBudgetError<E> {
    Exceeded(BudgetExceeded),
    InvalidDimension(BudgetKind),
    Operation(E),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_budget_accepts_its_limit_and_rejects_the_next_unit() {
        for kind in [
            BudgetKind::ConstantFuel,
            BudgetKind::ConstantCallDepth,
            BudgetKind::AggregateDepth,
            BudgetKind::CanonicalValueBytes,
            BudgetKind::RelationPairVisits,
            BudgetKind::RelationDepth,
            BudgetKind::NormalizationMembers,
            BudgetKind::TemplateMembers,
            BudgetKind::TemplateSourceBytes,
            BudgetKind::FlowIterations,
        ] {
            let mut budget = TypeComputationBudget::default();
            budget.charge(kind, kind.limit()).unwrap();
            assert_eq!(
                budget.charge(kind, 1),
                Err(BudgetExceeded {
                    kind,
                    limit: kind.limit(),
                    attempted: kind.limit() + 1
                }),
                "{}",
                kind.label()
            );
        }
    }

    #[test]
    fn exhausted_budget_does_not_silently_charge_past_the_limit() {
        for kind in [
            BudgetKind::ConstantFuel,
            BudgetKind::ConstantCallDepth,
            BudgetKind::AggregateDepth,
            BudgetKind::CanonicalValueBytes,
            BudgetKind::RelationPairVisits,
            BudgetKind::RelationDepth,
            BudgetKind::NormalizationMembers,
            BudgetKind::TemplateMembers,
            BudgetKind::TemplateSourceBytes,
        ] {
            let mut budget = TypeComputationBudget::default();
            budget.charge(kind, kind.limit() - 1).unwrap();
            assert!(budget.charge(kind, 2).is_err(), "{}", kind.label());
            assert_eq!(budget.charge(kind, 1), Ok(()), "{}", kind.label());
        }
    }

    #[test]
    fn depth_can_be_released_without_refunding_work_or_fuel() {
        let mut budget = TypeComputationBudget::default();
        budget.charge(BudgetKind::ConstantCallDepth, 2).unwrap();
        budget.release_depth(BudgetKind::ConstantCallDepth);
        assert_eq!(budget.charge(BudgetKind::ConstantCallDepth, 31), Ok(()));
        assert_eq!(budget.charge(BudgetKind::ConstantFuel, 100_000), Ok(()));
    }

    #[test]
    fn limit_diagnostic_has_stable_code_and_names_the_exhausted_dimension() {
        for kind in [
            BudgetKind::ConstantFuel,
            BudgetKind::ConstantCallDepth,
            BudgetKind::AggregateDepth,
            BudgetKind::CanonicalValueBytes,
            BudgetKind::RelationPairVisits,
            BudgetKind::RelationDepth,
            BudgetKind::NormalizationMembers,
            BudgetKind::TemplateMembers,
            BudgetKind::TemplateSourceBytes,
        ] {
            let mut budget = TypeComputationBudget::default();
            let error = budget.charge(kind, kind.limit() + 1).unwrap_err();
            let diagnostic = error.diagnostic();
            assert_eq!(diagnostic.code, crate::codes::TYPE_COMPUTATION_LIMIT);
            assert!(diagnostic.message.contains(kind.label()));
            assert!(diagnostic.message.contains(&kind.limit().to_string()));
            assert!(diagnostic.cause.is_some());
        }
    }

    #[test]
    fn recursive_depth_is_restored_after_success_and_error() {
        let mut budget = TypeComputationBudget::default();
        let success: Result<(), DepthBudgetError<()>> =
            budget.with_depth(BudgetKind::RelationDepth, |_| Ok(()));
        assert_eq!(success, Ok(()));
        let failure: Result<(), DepthBudgetError<&str>> =
            budget.with_depth(BudgetKind::RelationDepth, |_| Err("stop"));
        assert_eq!(failure, Err(DepthBudgetError::Operation("stop")));
        assert_eq!(budget.charge(BudgetKind::RelationDepth, 256), Ok(()));
    }
}
