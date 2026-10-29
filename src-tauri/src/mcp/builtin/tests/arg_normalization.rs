/// Argument normalization must not rewrite valid shell commands.
#[cfg(test)]
mod arg_normalization_tests {
    use crate::mcp::builtin::BuiltinServerRegistry;
    use serde_json::json;

    #[test]
    fn empty_string_args_in_command_pass_through() {
        for cmd in [
            "echo \"\"",
            "X=\"\" ; echo \"[$X]\"",
            "cat f && echo \"\" && echo \"=== B ===\" || echo \"no\"",
        ] {
            let out = BuiltinServerRegistry::normalize_json_args(json!({ "command": cmd }));
            assert_eq!(out["command"], cmd);
        }
    }
}
