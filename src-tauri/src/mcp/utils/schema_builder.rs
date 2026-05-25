use crate::mcp::schema::{JSONSchema, JSONSchemaAdditionalProperties, JSONSchemaType};
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
        one_of: None,
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
        one_of: None,
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
        one_of: None,
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
        one_of: None,
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
        one_of: None,
    }
}

/// Creates an object schema with properties and required fields
/// Note: If required is empty, it will be set to None instead of an empty array
/// to avoid DeepSeek/Fireworks JSON Schema validation errors
pub fn object_schema(properties: HashMap<String, JSONSchema>, required: Vec<String>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Object {
            properties: Some(properties),
            // Only set required if it has values, otherwise use None
            required: if required.is_empty() {
                None
            } else {
                Some(required)
            },
            additional_properties: Some(JSONSchemaAdditionalProperties::Boolean(false)),
            property_names: None,
            min_properties: None,
            max_properties: None,
        },
        title: None,
        description: None,
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
        one_of: None,
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
        one_of: None,
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
        one_of: None,
    }
}

/// Creates a required string property schema
pub fn string_prop_required(description: &str) -> JSONSchema {
    string_prop(None, None, Some(description))
}

/// Creates an enum property schema with allowed values and default
pub fn enum_prop(values: Vec<&str>, default: &str, description: Option<&str>) -> JSONSchema {
    let enum_values: Vec<Value> = values
        .iter()
        .map(|v| Value::String(v.to_string()))
        .collect();

    JSONSchema {
        schema_type: JSONSchemaType::String {
            min_length: None,
            max_length: None,
            pattern: None,
            format: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: Some(Value::String(default.to_string())),
        examples: None,
        enum_values: Some(enum_values),
        const_value: None,
        one_of: None,
    }
}

/// Creates an optional enum property schema without a default value
pub fn enum_prop_optional(values: Vec<&str>, description: Option<&str>) -> JSONSchema {
    let enum_values: Vec<Value> = values
        .iter()
        .map(|v| Value::String(v.to_string()))
        .collect();

    JSONSchema {
        schema_type: JSONSchemaType::String {
            min_length: None,
            max_length: None,
            pattern: None,
            format: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: Some(enum_values),
        const_value: None,
        one_of: None,
    }
}

/// Creates an enum property schema with allowed values and default (required version)
pub fn enum_prop_required(values: Vec<&str>, description: &str) -> JSONSchema {
    let enum_values: Vec<Value> = values
        .iter()
        .map(|v| Value::String(v.to_string()))
        .collect();

    JSONSchema {
        schema_type: JSONSchemaType::String {
            min_length: None,
            max_length: None,
            pattern: None,
            format: None,
        },
        title: None,
        description: Some(description.to_string()),
        default: None,
        examples: None,
        enum_values: Some(enum_values),
        const_value: None,
        one_of: None,
    }
}

/// Creates an object property schema with nested properties
pub fn object_prop(
    properties: Vec<(String, JSONSchema)>,
    required: Vec<String>,
    description: Option<&str>,
) -> JSONSchema {
    let props: HashMap<String, JSONSchema> = properties.into_iter().collect();

    JSONSchema {
        schema_type: JSONSchemaType::Object {
            properties: Some(props),
            required: if required.is_empty() { None } else { Some(required) },
            additional_properties: Some(JSONSchemaAdditionalProperties::Boolean(false)),
            property_names: None,
            min_properties: None,
            max_properties: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
        one_of: None,
    }
}

/// Creates an object property schema that allows arbitrary string-keyed fields.
pub fn object_map_prop(description: Option<&str>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Object {
            properties: Some(HashMap::new()),
            required: None,
            additional_properties: Some(JSONSchemaAdditionalProperties::Boolean(true)),
            property_names: None,
            min_properties: None,
            max_properties: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
        one_of: None,
    }
}

/// Creates a string property constrained to a single constant value.
pub fn string_const_prop(value: &str, description: Option<&str>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::String {
            min_length: None,
            max_length: None,
            pattern: None,
            format: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: Some(Value::String(value.to_string())),
        one_of: None,
    }
}

/// Creates an integer property constrained to a single constant value.
pub fn integer_const_prop(value: i64, description: Option<&str>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Integer {
            minimum: None,
            maximum: None,
            exclusive_minimum: None,
            exclusive_maximum: None,
            multiple_of: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: Some(Value::Number(serde_json::Number::from(value))),
        one_of: None,
    }
}

/// Creates an object schema that validates exactly one of the provided variants.
pub fn one_of_object_schema(variants: Vec<JSONSchema>, description: Option<&str>) -> JSONSchema {
    JSONSchema {
        schema_type: JSONSchemaType::Object {
            properties: None,
            required: None,
            additional_properties: None,
            property_names: None,
            min_properties: None,
            max_properties: None,
        },
        title: None,
        description: description.map(|s| s.to_string()),
        default: None,
        examples: None,
        enum_values: None,
        const_value: None,
        one_of: Some(variants),
    }
}
