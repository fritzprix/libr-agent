
#[macro_export]
macro_rules! define_mcp_tool {
    (
        const $const_name:ident = $tool_name:expr;
        fn $fn_name:ident();
        title: $title:expr;
        description: $desc:expr;
        inputs: $props_ident:ident => $props_block:block;
        required: $required:expr;
    ) => {
        pub const $const_name: &str = $tool_name;

        pub fn $fn_name() -> crate::mcp::MCPTool {
            use std::collections::HashMap;
            #[allow(unused_imports)]
            use crate::mcp::utils::schema_builder::*;
            #[allow(unused_imports)]
            use serde_json::json;

            let mut $props_ident = HashMap::new();
            $props_block

            crate::mcp::MCPTool {
                name: $const_name.to_string(),
                title: Some($title.to_string()),
                description: $desc.to_string(),
                input_schema: object_schema($props_ident, $required),
                output_schema: None,
                annotations: None,
            }
        }
    };
}
