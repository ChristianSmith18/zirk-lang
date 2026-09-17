//! Versioned, serialization-safe browser boundary types.

use serde::{Deserialize, Serialize};

/// Limits imposed by the package, or tightened by a caller. Zero is rejected
/// by the operation layer; this type only carries the contract values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub source_bytes: usize,
    pub diagnostics: usize,
    pub instructions: u64,
    pub call_depth: u32,
    pub heap_bytes: usize,
    pub output_bytes: usize,
    pub output_events: u32,
    pub stack_frames: u32,
}

impl Limits {
    pub const MAX: Self = Self {
        source_bytes: 512 * 1024,
        diagnostics: 100,
        instructions: zirk_interpreter::InterpreterLimits::MAX_INSTRUCTIONS,
        call_depth: zirk_interpreter::InterpreterLimits::MAX_CALL_DEPTH,
        heap_bytes: zirk_interpreter::InterpreterLimits::MAX_HEAP_BYTES,
        output_bytes: zirk_interpreter::InterpreterLimits::MAX_OUTPUT_BYTES,
        output_events: zirk_interpreter::InterpreterLimits::MAX_OUTPUT_EVENTS,
        stack_frames: zirk_interpreter::InterpreterLimits::MAX_STACK_FRAMES,
    };

    pub const DEFAULT: Self = Self {
        source_bytes: 128 * 1024,
        diagnostics: 50,
        instructions: 2_000_000,
        call_depth: 128,
        heap_bytes: 16 * 1024 * 1024,
        output_bytes: 256 * 1024,
        output_events: 2_048,
        stack_frames: 64,
    };

    /// Clamps every field to the package maxima, so a request can never
    /// increase authority or resource use above the reviewed package policy.
    pub const fn tightened_by(self, requested: Self) -> Self {
        Self {
            source_bytes: min_usize(self.source_bytes, requested.source_bytes),
            diagnostics: min_usize(self.diagnostics, requested.diagnostics),
            instructions: min_u64(self.instructions, requested.instructions),
            call_depth: min_u32(self.call_depth, requested.call_depth),
            heap_bytes: min_usize(self.heap_bytes, requested.heap_bytes),
            output_bytes: min_usize(self.output_bytes, requested.output_bytes),
            output_events: min_u32(self.output_events, requested.output_events),
            stack_frames: min_u32(self.stack_frames, requested.stack_frames),
        }
    }
}

const fn min_usize(left: usize, right: usize) -> usize {
    if left < right { left } else { right }
}
const fn min_u64(left: u64, right: u64) -> u64 {
    if left < right { left } else { right }
}
const fn min_u32(left: u32, right: u32) -> u32 {
    if left < right { left } else { right }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub api_version: u32,
    pub source: String,
    pub virtual_name: String,
    #[serde(default)]
    pub limits: Option<Limits>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub stage: String,
    pub file: Option<String>,
    pub start: Option<u32>,
    pub end: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputEvent {
    pub text: String,
    pub destination: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Completion {
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureKind {
    Request,
    Runtime,
    Capability,
    Limit,
    Internal,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub kind: FailureKind,
    pub code: String,
    pub message: String,
    pub limit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub api_version: u32,
    pub operation: String,
    pub compiler_version: String,
    pub compiler_revision: String,
    pub limits: Limits,
    pub diagnostics: Vec<Diagnostic>,
    pub diagnostics_truncated: bool,
    pub output: Vec<OutputEvent>,
    pub completion: Option<Completion>,
    pub failure: Option<Failure>,
    /// Canonical source returned by the additive `format` operation only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted_source: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{Limits, Request};

    #[test]
    fn caller_limits_can_only_tighten_the_package_policy() {
        let requested = Limits {
            instructions: u64::MAX,
            source_bytes: 1,
            ..Limits::MAX
        };
        let effective = Limits::DEFAULT.tightened_by(requested);
        assert_eq!(effective.instructions, Limits::DEFAULT.instructions);
        assert_eq!(effective.source_bytes, 1);
    }

    #[test]
    fn requests_reject_unknown_contract_fields() {
        assert!(
            serde_json::from_str::<Request>(
                r#"{\"apiVersion\":1,\"source\":\"\",\"virtualName\":\"a.zrk\",\"extra\":true}"#
            )
            .is_err()
        );
    }
}
