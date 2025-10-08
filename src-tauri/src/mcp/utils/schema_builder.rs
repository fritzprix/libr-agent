use crate::mcp::schema::{JSONSchema, JSONSchemaType};
use serde_json::Value;
use std::collections::HashMap;

/// Creates a string property schema with common options
pub fn string_prop(
    min_length: Option<u32>,
    max_length: Option<u32>,
    description: Option<&str>,
) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::String {
            min_length,
            max_length,
            pattern: None,
            format: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates an integer property schema with common options
pub fn integer_prop(
    minimum: Option<i64>,
    maximum: Option<i64>,
    description: Option<&str>,
) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Integer {
            minimum,
            maximum,
            exclusive_minimum: None,
            exclusive_maximum: None,
            multiple_of: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates a number property schema with common options
pub fn number_prop(
    minimum: Option<f64>,
    maximum: Option<f64>,
    description: Option<&str>,
) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Number {
            minimum,
            maximum,
            exclusive_minimum: None,
            exclusive_maximum: None,
            multiple_of: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates an integer property schema with a default value
pub fn integer_prop_with_default(
    minimum: Option<i64>,
    maximum: Option<i64>,
    default: i64,
    description: Option<&str>,
) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Integer {
            minimum,
            maximum,
            exclusive_minimum: None,
            exclusive_maximum: None,
            multiple_of: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: Some(Value::Number(serde_json::Number::from(default))),
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates a boolean property schema
pub fn boolean_prop(description: Option<&str>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Boolean,
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates a boolean property schema with a default value
pub fn boolean_prop_with_default(default: bool, description: Option<&str>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Boolean,
        title: None,
        description: description.map(|s| s.to_string()),
        default: Some(Value::Bool(default)),
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates an object property schema
pub fn object_prop(
    description: Option<&str>,
    _properties: HashMap<String, JSONSchema>,
) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Object {
            properties: None,
            required: None,
            additional_properties: Some(true),
            min_properties: None,
            max_properties: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates an object schema with properties and required fields
pub fn object_schema(properties: HashMap<String, JSONSchema>, required: Vec<String>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Object {
            properties: Some(properties),
            required: Some(required),
            additional_properties: Some(true),
            min_properties: None,
            max_properties: None,
        },
        title: None,
        description: None,
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates an array schema with item type
pub fn array_schema(items: JSONSchema, description: Option<&str>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Array {
            items: Some(Box::new(items)),
            min_items: None,
            max_items: None,
            unique_items: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
    }
}

/// Creates a string property with examples
pub fn string_prop_with_examples(
    min_length: Option<u32>,
    max_length: Option<u32>,
    description: Option<&str>,
    examples: Vec<Value>,
) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::String {
            min_length,
            max_length,
            pattern: None,
            format: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: Some(examples),
        enum_values: None,
        const_value: None,
    }
}
