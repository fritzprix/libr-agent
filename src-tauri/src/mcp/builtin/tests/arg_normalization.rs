/// Argument normalization must not rewrite valid `command` / `code` values.
#[cfg(test)]
mod arg_normalization_tests {
    use crate::mcp::builtin::BuiltinServerRegistry;
    use serde_json::json;

    #[test]
    fn command_args_pass_through() {
        for cmd in [
            "echo \"\"",
            "X=\"\" ; echo \"[$X]\"",
            "cat f && echo \"\" && echo \"=== B ===\" || echo \"no\"",
            "echo \"it's done\"",
            "echo \"a\\\"b\"",
            "ls # don't care",
            "cat <<'EOF' > f.txt\nit's fine\nEOF",
            "echo \"$(printf '%s' \"it's\")\"",
            "python -c \"print('it' + \\\"'\\\" + 's')\"",
        ] {
            let out = BuiltinServerRegistry::normalize_json_args(json!({ "command": cmd }));
            assert_eq!(out["command"], cmd);
        }
    }

    #[test]
    fn code_args_pass_through() {
        for code in [
            "print(\"it's\")",
            "# don't\nprint(1)",
            "s = \"\"\"a \"quoted\" word\"\"\"",
        ] {
            let out = BuiltinServerRegistry::normalize_json_args(json!({ "code": code }));
            assert_eq!(out["code"], code);
        }
    }
}
