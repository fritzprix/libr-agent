use serde::{Deserialize, Serialize};

/// Represents the different types a JSON Schema can have, along with their specific constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JSONSchemaType {
    /// A string type, with optional length and pattern constraints.
    #[serde(rename = "string")]
    String {
        /// The minimum length of the string.
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<u32>,
        /// The maximum length of the string.
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<u32>,
        /// A regular expression pattern for the string to match.
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
        /// A specific format for the string (e.g., "date-time", "email").
        #[serde(skip_serializing_if = "Option::is_none")]
        format: Option<String>,
    },
    /// A number type, with optional range and multiple-of constraints.
    #[serde(rename = "number")]
    Number {
        /// The inclusive minimum value.
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<f64>,
        /// The inclusive maximum value.
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<f64>,
        /// The exclusive minimum value.
        #[serde(skip_serializing_if = "Option::is_none")]
        exclusive_minimum: Option<f64>,
        /// The exclusive maximum value.
        #[serde(skip_serializing_if = "Option::is_none")]
        exclusive_maximum: Option<f64>,
        /// Specifies that the value must be a multiple of this number.
        #[serde(skip_serializing_if = "Option::is_none")]
        multiple_of: Option<f64>,
    },
    /// An integer type, with optional range and multiple-of constraints.
    #[serde(rename = "integer")]
    Integer {
        /// The inclusive minimum value.
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<i64>,
        /// The inclusive maximum value.
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<i64>,
        /// The exclusive minimum value.
        #[serde(skip_serializing_if = "Option::is_none")]
        exclusive_minimum: Option<i64>,
        /// The exclusive maximum value.
        #[serde(skip_serializing_if = "Option::is_none")]
        exclusive_maximum: Option<i64>,
        /// Specifies that the value must be a multiple of this number.
        #[serde(skip_serializing_if = "Option::is_none")]
        multiple_of: Option<i64>,
    },
    /// A boolean type.
    #[serde(rename = "boolean")]
    Boolean,
    /// An array type, with optional constraints on its items and size.
    #[serde(rename = "array")]
    Array {
        /// The schema for the items in the array.
        #[serde(skip_serializing_if = "Option::is_none")]
        items: Option<Box<JSONSchema>>,
        /// The minimum number of items in the array.
        #[serde(skip_serializing_if = "Option::is_none")]
        min_items: Option<u32>,
        /// The maximum number of items in the array.
        #[serde(skip_serializing_if = "Option::is_none")]
        max_items: Option<u32>,
        /// If true, all items in the array must be unique.
        #[serde(skip_serializing_if = "Option::is_none")]
        unique_items: Option<bool>,
    },
    /// An object type, with optional constraints on its properties.
    #[serde(rename = "object")]
    Object {
        /// A map of property names to their schemas.
        #[serde(skip_serializing_if = "Option::is_none")]
        properties: Option<std::collections::HashMap<String, JSONSchema>>,
        /// A list of required property names.
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<Vec<String>>,
        /// If false, no additional properties are allowed.
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_properties: Option<bool>,
        /// The minimum number of properties.
        #[serde(skip_serializing_if = "Option::is_none")]
        min_properties: Option<u32>,
        /// The maximum number of properties.
        #[serde(skip_serializing_if = "Option::is_none")]
        max_properties: Option<u32>,
    },
    /// A null type.
    #[serde(rename = "null")]
    Null,
}

/// Represents a JSON Schema object, combining the base type with common metadata fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONSchema {
    /// The core type and constraints of the schema.
    #[serde(flatten)]
    pub schema_type: JSONSchemaType,
    /// A human-readable title for the schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A detailed description of the schema's purpose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A default value for the schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// An array of example values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<serde_json::Value>>,
    /// A list of allowed values.
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<serde_json::Value>>,
    /// A constant value that the schema must match.
    #[serde(rename = "const", skip_serializing_if = "Option::is_none")]
    pub const_value: Option<serde_json::Value>,
}

impl JSONSchema {
    /// Creates a new `JSONSchema` representing a `null` type.
    pub fn null() -> Self {
        Self {
            schema_type: JSONSchemaType::Null,
            title: None,
            description: None,
            default: None,
            examples: None,
            enum_values: None,
            const_value: None,
        }
    }
}

/// A type alias for `JSONSchema` used specifically for tool input schemas,
/// enhancing readability and maintaining backward compatibility.
pub type MCPToolInputSchema = JSONSchema;

impl Default for MCPToolInputSchema {
    /// Provides a default `MCPToolInputSchema`, which is an empty object schema.
    fn default() -> Self {
        Self {
            schema_type: JSONSchemaType::Object {
                properties: Some(std::collections::HashMap::new()),
                required: None,
                additional_properties: None,
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
}
