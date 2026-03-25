pub mod builder;
/// Error Guidance System for Built-in MCP Tools
///
/// This module provides a centralized error guidance system that ensures consistent,
/// actionable error messages across all built-in tools. It follows the best practices
/// documented in docs/guides/builtin_tool_bp.md.
///
/// Key principles:
/// - Every error includes visual markers (✗)
/// - Errors provide 2-3 actionable recovery steps
/// - Tool group isolation: Browser tools suggest browser tools, etc.
/// - Never expose internal state in error messages
/// - Consistent formatting across all tool groups
pub mod categories;
pub mod convenience;
pub mod guidance;

pub use builder::{guided_error, ErrorBuilder};
pub use categories::{ErrorCategory, ToolGroup};
pub use convenience::*;
pub use guidance::{ErrorGuidance, SuccessHint};

#[cfg(test)]
mod tests;
