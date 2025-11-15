/// Schema validation tests for all built-in MCP tools
/// Ensures all tools have complete and valid schemas for LLM consumption
#[cfg(test)]
mod schema_validation_tests {
    use crate::mcp::builtin::BuiltinServerRegistry;
    use crate::mcp::schema::JSONSchemaType;
    use crate::session::SessionManager;
    use std::sync::Arc;

    /// Helper function to create a test registry
    fn create_test_registry() -> BuiltinServerRegistry {
        let session_manager =
            Arc::new(SessionManager::new().expect("Failed to create SessionManager"));
        BuiltinServerRegistry::new_with_session_manager(session_manager)
    }

    #[tokio::test]
    async fn test_all_builtin_tools_have_valid_schemas() {
        let registry = create_test_registry();
        let all_tools = registry.list_all_tools();

        assert!(!all_tools.is_empty(), "Registry should have tools");

        let mut errors = Vec::new();

        for tool in all_tools.iter() {
            // Check tool-level fields
            if tool.name.is_empty() {
                errors.push(format!("Tool has empty name"));
            }

            if tool.description.is_empty() {
                errors.push(format!("Tool '{}' has empty description", tool.name));
            }

            // Check input schema
            match &tool.input_schema.schema_type {
                JSONSchemaType::Object {
                    properties,
                    required,
                    ..
                } => {
                    // Check if properties exist
                    if let Some(props) = properties {
                        for (prop_name, prop_schema) in props.iter() {
                            // Check property description
                            if prop_schema.description.is_none() {
                                errors.push(format!(
                                    "Tool '{}' property '{}' has no description (None)",
                                    tool.name, prop_name
                                ));
                            } else if let Some(desc) = &prop_schema.description {
                                if desc.is_empty() {
                                    errors.push(format!(
                                        "Tool '{}' property '{}' has empty description ('')",
                                        tool.name, prop_name
                                    ));
                                }
                            }

                            // Check property type is valid
                            match &prop_schema.schema_type {
                                JSONSchemaType::String { .. } => {}
                                JSONSchemaType::Integer { .. } => {}
                                JSONSchemaType::Number { .. } => {}
                                JSONSchemaType::Boolean => {}
                                JSONSchemaType::Array { .. } => {}
                                JSONSchemaType::Object { .. } => {}
                                JSONSchemaType::Null => {
                                    errors.push(format!(
                                        "Tool '{}' property '{}' has null type",
                                        tool.name, prop_name
                                    ));
                                }
                            }
                        }
                    }

                    // Check required array
                    if let Some(req) = required {
                        if req.is_empty() {
                            errors.push(format!(
                                "Tool '{}' has empty required array (should be None instead)",
                                tool.name
                            ));
                        }

                        // Verify all required fields exist in properties
                        if let Some(props) = properties {
                            for req_field in req.iter() {
                                if !props.contains_key(req_field) {
                                    errors.push(format!(
                                        "Tool '{}' required field '{}' not in properties",
                                        tool.name, req_field
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => {
                    errors.push(format!(
                        "Tool '{}' input_schema is not an object type",
                        tool.name
                    ));
                }
            }
        }

        if !errors.is_empty() {
            eprintln!("\n=== SCHEMA VALIDATION ERRORS ===");
            eprintln!("Total tools checked: {}", all_tools.len());
            eprintln!("Total errors found: {}\n", errors.len());
            for error in errors.iter() {
                eprintln!("  ❌ {}", error);
            }
            eprintln!("\n");
            panic!(
                "Found {} schema validation errors (see above)",
                errors.len()
            );
        }

        println!("✅ All {} tools have valid schemas", all_tools.len());
    }

    #[tokio::test]
    async fn test_no_tools_with_empty_required_arrays() {
        let registry = create_test_registry();
        let all_tools = registry.list_all_tools();

        let mut tools_with_empty_required = Vec::new();

        for tool in all_tools.iter() {
            if let JSONSchemaType::Object { required, .. } = &tool.input_schema.schema_type {
                if let Some(req) = required {
                    if req.is_empty() {
                        tools_with_empty_required.push(tool.name.clone());
                    }
                }
            }
        }

        if !tools_with_empty_required.is_empty() {
            eprintln!("\n❌ Tools with empty required arrays (should use None instead):");
            for tool_name in tools_with_empty_required.iter() {
                eprintln!("  - {}", tool_name);
            }
            panic!(
                "{} tools have empty required arrays",
                tools_with_empty_required.len()
            );
        }
    }

    #[tokio::test]
    async fn test_serialization_to_json_has_no_empty_strings() {
        let registry = create_test_registry();
        let all_tools = registry.list_all_tools();

        let mut tools_with_issues = Vec::new();

        for tool in all_tools.iter() {
            // Serialize tool to JSON
            let json = serde_json::to_value(tool).expect("Failed to serialize tool");

            // Check for empty strings in the JSON
            if let Some(issues) = check_json_for_empty_strings(&json, &tool.name) {
                tools_with_issues.extend(issues);
            }
        }

        if !tools_with_issues.is_empty() {
            eprintln!("\n=== EMPTY STRING ISSUES IN JSON ===");
            for issue in tools_with_issues.iter() {
                eprintln!("  ❌ {}", issue);
            }
            panic!("{} issues found", tools_with_issues.len());
        }
    }

    /// Recursively check JSON for empty strings
    fn check_json_for_empty_strings(
        value: &serde_json::Value,
        tool_name: &str,
    ) -> Option<Vec<String>> {
        let mut issues = Vec::new();

        match value {
            serde_json::Value::String(s) if s.is_empty() => {
                issues.push(format!("Tool '{}' has empty string in JSON", tool_name));
            }
            serde_json::Value::Object(map) => {
                for (key, val) in map.iter() {
                    if let serde_json::Value::String(s) = val {
                        if s.is_empty() && (key == "description" || key == "type") {
                            issues.push(format!("Tool '{}' has empty '{}' field", tool_name, key));
                        }
                    }

                    if let Some(nested_issues) = check_json_for_empty_strings(val, tool_name) {
                        issues.extend(nested_issues);
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr.iter() {
                    if let Some(nested_issues) = check_json_for_empty_strings(item, tool_name) {
                        issues.extend(nested_issues);
                    }
                }
            }
            _ => {}
        }

        if issues.is_empty() {
            None
        } else {
            Some(issues)
        }
    }

    #[tokio::test]
    async fn test_all_tools_have_required_fields_when_needed() {
        let registry = create_test_registry();
        let all_tools = registry.list_all_tools();

        println!("\n=== TOOL REQUIRED FIELDS SUMMARY ===");
        for tool in all_tools.iter() {
            if let JSONSchemaType::Object {
                properties,
                required,
                ..
            } = &tool.input_schema.schema_type
            {
                let prop_count = properties.as_ref().map(|p| p.len()).unwrap_or(0);
                let req_count = required.as_ref().map(|r| r.len()).unwrap_or(0);

                println!(
                    "Tool: {} | Properties: {} | Required: {}",
                    tool.name, prop_count, req_count
                );

                if prop_count > 0 && req_count == 0 {
                    println!("  ⚠️  Has properties but no required fields");
                }
            }
        }
    }
}
