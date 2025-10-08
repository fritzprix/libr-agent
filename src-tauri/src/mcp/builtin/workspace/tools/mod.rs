// 기존 create_*_tool 함수들을 기능별로 분리
pub mod code_tools;
pub mod export_tools;
pub mod file_tools;

use crate::mcp::MCPTool;

pub fn file_tools() -> Vec<MCPTool> {
    vec![
        file_tools::create_read_file_tool(),
        file_tools::create_write_file_tool(),
        file_tools::create_list_directory_tool(),
        file_tools::create_replace_lines_in_file_tool(),
        file_tools::create_import_file_tool(),
    ]
}

pub fn code_tools() -> Vec<MCPTool> {
    vec![
        // Python/TypeScript execution tools removed from public interface to avoid
        // pulling external runtime dependencies and to prevent agents from
        // setting isolation levels. Only shell execution remains exposed.
        code_tools::create_execute_shell_tool(),
        code_tools::create_eval_javascript_tool(),
    ]
}

pub fn export_tools() -> Vec<MCPTool> {
    vec![
        export_tools::create_export_file_tool(),
        export_tools::create_export_zip_tool(),
    ]
}
