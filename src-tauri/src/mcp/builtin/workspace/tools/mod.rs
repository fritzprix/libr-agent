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
        file_tools::create_search_files_tool(),
        file_tools::create_replace_lines_in_file_tool(),
        file_tools::create_grep_tool(),
    ]
}

pub fn code_tools() -> Vec<MCPTool> {
    vec![
        code_tools::create_execute_python_tool(),
        code_tools::create_execute_typescript_tool(),
        code_tools::create_execute_shell_tool(),
    ]
}

pub fn export_tools() -> Vec<MCPTool> {
    vec![
        export_tools::create_export_file_tool(),
        export_tools::create_export_zip_tool(),
    ]
}
