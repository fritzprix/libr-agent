use crate::define_mcp_tool;

define_mcp_tool! {
    const EXPORT_FILE = "exportFile";
    fn create_export_file_tool();
    title: "Export Single File";
    description: "Export a single file from workspace for download with interactive UI";
    inputs: props => {
        props.insert(
            "path".to_string(),
            string_prop(
                Some(1),
                Some(1000),
                Some("Relative path to the file to export (from workspace root)"),
            ),
        );
        props.insert(
            "displayName".to_string(),
            string_prop(
                None,
                None,
                Some("Filename to display for download (optional)"),
            ),
        );
        props.insert(
            "description".to_string(),
            string_prop(None, None, Some("File description (optional)")),
        );
    };
    required: vec!["path".to_string()];
}

define_mcp_tool! {
    const EXPORT_ZIP = "exportZip";
    fn create_export_zip_tool();
    title: "Export ZIP Package";
    description: "Export multiple files or directories as a ZIP package for download with interactive UI";
    inputs: props => {
        props.insert(
            "files".to_string(),
            array_schema(
                string_prop(Some(1), Some(1000), None),
                Some(
                    "Array of relative file or directory paths to export (from workspace root). Directories are included recursively",
                ),
            ),
        );
        props.insert(
            "packageName".to_string(),
            string_prop(
                None,
                Some(50),
                Some("ZIP package name (optional, default: workspace_export)"),
            ),
        );
        props.insert(
            "description".to_string(),
            string_prop(None, None, Some("Package description (optional)")),
        );
    };
    required: vec!["files".to_string()];
}
