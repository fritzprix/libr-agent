/**
 * @file JSON Schema Type Definitions
 * @description Core JSON Schema types adhering to JSON Schema Draft 7
 * @see https://json-schema.org/draft-07/json-schema-release-notes.html
 */

/**
 * Represents the possible data types in a JSON Schema.
 */
export type JSONSchemaType =
  | 'string'
  | 'number'
  | 'integer'
  | 'boolean'
  | 'object'
  | 'array'
  | 'null';

/**
 * The base interface for all JSON Schema definitions.
 */
export interface JSONSchemaBase {
  /** The data type of the schema. */
  type?: JSONSchemaType | JSONSchemaType[];
  /** A title for the schema. */
  title?: string;
  /** A description of the schema. */
  description?: string;
  /** The default value for the schema. */
  default?: unknown;
  /** An array of example values. */
  examples?: unknown[];
  /** An array of allowed values. */
  enum?: unknown[];
  /** A constant value that the schema must have. */
  const?: unknown;
}

/**
 * Represents a JSON Schema for a string type.
 */
export interface JSONSchemaString extends JSONSchemaBase {
  type: 'string';
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  format?: string;
}

/**
 * Represents a JSON Schema for a number or integer type.
 */
export interface JSONSchemaNumber extends JSONSchemaBase {
  type: 'number' | 'integer';
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  multipleOf?: number;
}

/**
 * Represents a JSON Schema for a boolean type.
 */
export interface JSONSchemaBoolean extends JSONSchemaBase {
  type: 'boolean';
}

/**
 * Represents a JSON Schema for a null type.
 */
export interface JSONSchemaNull extends JSONSchemaBase {
  type: 'null';
}

/**
 * Represents a JSON Schema for an array type.
 */
export interface JSONSchemaArray extends JSONSchemaBase {
  type: 'array';
  items?: JSONSchema | JSONSchema[];
  minItems?: number;
  maxItems?: number;
  uniqueItems?: boolean;
  additionalItems?: boolean | JSONSchema;
}

/**
 * Represents a JSON Schema for an object type.
 */
export interface JSONSchemaObject extends JSONSchemaBase {
  type: 'object';
  properties?: Record<string, JSONSchema>;
  required?: string[];
  additionalProperties?: boolean | JSONSchema;
  patternProperties?: Record<string, JSONSchema>;
  minProperties?: number;
  maxProperties?: number;
  dependencies?: Record<string, JSONSchema | string[]>;
}

/**
 * A union type representing any valid JSON Schema.
 */
export type JSONSchema =
  | JSONSchemaString
  | JSONSchemaNumber
  | JSONSchemaBoolean
  | JSONSchemaNull
  | JSONSchemaArray
  | JSONSchemaObject
  | (JSONSchemaBase & { type?: JSONSchemaType | JSONSchemaType[] });
