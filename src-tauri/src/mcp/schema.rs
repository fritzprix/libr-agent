use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JSONSchemaType {
  #[serde(rename = "string")]
  String {
    #[serde(skip_serializing_if = "Option::is_none")]
    min_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
  },
  #[serde(rename = "number")]
  Number {
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclusive_minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclusive_maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiple_of: Option<f64>,
  },
  #[serde(rename = "integer")]
  Integer {
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclusive_minimum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclusive_maximum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiple_of: Option<i64>,
  },
  #[serde(rename = "boolean")]
  Boolean,
  #[serde(rename = "array")]
  Array {
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<Box<JSONSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_items: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_items: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unique_items: Option<bool>,
  },
  #[serde(rename = "object")]
  Object {
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<std::collections::HashMap<String, JSONSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_properties: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_properties: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_properties: Option<u32>,
  },
  #[serde(rename = "null")]
  Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONSchema {
  #[serde(flatten)]
  pub schema_type: JSONSchemaType,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub description: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub default: Option<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub examples: Option<Vec<serde_json::Value>>,
  #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
  pub enum_values: Option<Vec<serde_json::Value>>,
  #[serde(rename = "const", skip_serializing_if = "Option::is_none")]
  pub const_value: Option<serde_json::Value>,
}

impl JSONSchema {
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

// For backward compatibility, create a type alias
pub type MCPToolInputSchema = JSONSchema;

impl Default for MCPToolInputSchema {
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