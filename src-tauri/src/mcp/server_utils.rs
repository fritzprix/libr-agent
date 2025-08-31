use log::warn;
use serde_json::Value;

use crate::mcp::schema::JSONSchema;

/// Convert JSON schema to structured JSONSchema with defensive parsing
pub fn convert_input_schema(schema: Value) -> JSONSchema {
  // First try direct deserialization
  match serde_json::from_value::<JSONSchema>(schema.clone()) {
    Ok(json_schema) => json_schema,
    Err(e) => {
      // Log detailed error information for debugging
      warn!("Failed to parse JSON schema: {e}");
      warn!(
        "Schema content: {}",
        serde_json::to_string_pretty(&schema)
          .unwrap_or_else(|_| "invalid JSON".to_string())
      );

      // Try to fix common issues and re-parse
      if let Ok(fixed_schema) = fix_schema_issues(schema.clone()) {
        match serde_json::from_value::<JSONSchema>(fixed_schema) {
          Ok(json_schema) => {
            warn!("Successfully parsed schema after applying fixes");
            return json_schema;
          }
          Err(fix_error) => {
            warn!("Schema still invalid after fixes: {fix_error}");
          }
        }
      }

      warn!("Using null schema as fallback");
      JSONSchema::null()
    }
  }
}

/// Attempt to fix common schema parsing issues
pub fn fix_schema_issues(
  mut schema: Value,
) -> Result<Value, serde_json::Error> {
  if let Some(obj) = schema.as_object_mut() {
    // Fix missing type field
    if !obj.contains_key("type") {
      // Infer type from structure
      if obj.contains_key("properties") {
        obj.insert(
          "type".to_string(),
          Value::String("object".to_string()),
        );
      } else if obj.contains_key("items") {
        obj.insert(
          "type".to_string(),
          Value::String("array".to_string()),
        );
      } else {
        obj.insert(
          "type".to_string(),
          Value::String("object".to_string()),
        );
      }
    }

    // Fix array-type type fields (convert sequence to single string)
    if let Some(type_value) = obj.get_mut("type") {
      if type_value.is_array() {
        if let Some(first_type) = type_value.as_array().and_then(|arr| arr.first()) {
          if let Some(type_str) = first_type.as_str() {
            *type_value = Value::String(type_str.to_string());
          }
        }
      }
    }

    // Recursively fix properties
    if let Some(properties) = obj.get_mut("properties") {
      if let Some(props_obj) = properties.as_object_mut() {
        for (_, prop_value) in props_obj.iter_mut() {
          if let Ok(fixed_prop) = fix_schema_issues(prop_value.clone()) {
            *prop_value = fixed_prop;
          }
        }
      }
    }

    // Recursively fix array items
    if let Some(items) = obj.get_mut("items") {
      if items.is_array() {
        if let Some(items_array) = items.as_array_mut() {
          for item in items_array.iter_mut() {
            if let Ok(fixed_item) = fix_schema_issues(item.clone()) {
              *item = fixed_item;
            }
          }
        }
      } else if let Ok(fixed_items) = fix_schema_issues(items.clone()) {
        *items = fixed_items;
      }
    }
  }

  Ok(schema)
}