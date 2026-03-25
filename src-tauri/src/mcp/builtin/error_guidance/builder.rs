use super::categories::{ErrorCategory, ToolGroup};
use super::guidance::ErrorGuidance;
use crate::mcp::types::MCPResult;

/// Canonical builder for constructing `ErrorGuidance` instances.
///
/// Why this exists:
/// - Makes the "category + message + tool_group + guidance" contract explicit.
/// - Allows optional override of default guidance in a consistent way.
/// - Enables gradual migration without breaking the existing helper functions.
#[must_use]
pub struct ErrorBuilder {
    category: ErrorCategory,
    message: String,
    guidance: Option<Vec<String>>,
    tool_group: ToolGroup,
}

impl ErrorBuilder {
    /// Create a new builder.
    pub fn new(category: ErrorCategory, message: impl Into<String>, tool_group: ToolGroup) -> Self {
        Self {
            category,
            message: message.into(),
            guidance: None,
            tool_group,
        }
    }

    /// Override default guidance with custom recovery steps.
    pub fn guidance(mut self, guidance: Vec<String>) -> Self {
        self.guidance = Some(guidance);
        self
    }

    /// Alias for `guidance` to support builder pattern naming.
    pub fn with_guidance(self, guidance: Vec<String>) -> Self {
        self.guidance(guidance)
    }

    /// Build an `ErrorGuidance` instance.
    pub fn build(self) -> ErrorGuidance {
        let guidance = self
            .guidance
            .unwrap_or_else(|| ErrorGuidance::get_default_guidance(self.category, self.tool_group));

        ErrorGuidance {
            category: self.category,
            message: self.message,
            guidance,
            tool_group: self.tool_group,
        }
    }

    /// Convenience: Build and convert to `MCPResult`.
    pub fn to_mcp_result(self) -> MCPResult {
        self.build().to_mcp_result()
    }
}

/// Canonical entrypoint for creating guided errors.
///
/// Prefer this over calling `ErrorGuidance::new/with_guidance` directly in new code.
pub fn guided_error(
    category: ErrorCategory,
    message: impl Into<String>,
    tool_group: ToolGroup,
) -> ErrorBuilder {
    ErrorBuilder::new(category, message, tool_group)
}
