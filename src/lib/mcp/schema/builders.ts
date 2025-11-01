/**
 * @file JSON Schema Builder Functions
 * @description Helper functions for creating JSON Schema objects
 */

import type {
  JSONSchemaString,
  JSONSchemaNumber,
  JSONSchemaBoolean,
  JSONSchemaArray,
  JSONSchemaObject,
  JSONSchema,
} from './json-schema';

/**
 * Creates a JSON schema for a string.
 * @param options Optional constraints for the string schema.
 * @returns A `JSONSchemaString` object.
 */
export function createStringSchema(options?: {
  description?: string;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  format?: string;
}): JSONSchemaString {
  return {
    type: 'string',
    ...options,
  };
}

/**
 * Creates a JSON schema for a number.
 * @param options Optional constraints for the number schema.
 * @returns A `JSONSchemaNumber` object.
 */
export function createNumberSchema(options?: {
  description?: string;
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  multipleOf?: number;
}): JSONSchemaNumber {
  return {
    type: 'number',
    ...options,
  };
}

/**
 * Creates a JSON schema for an integer.
 * @param options Optional constraints for the integer schema.
 * @returns A `JSONSchemaNumber` object with type 'integer'.
 */
export function createIntegerSchema(options?: {
  description?: string;
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  multipleOf?: number;
}): JSONSchemaNumber {
  return {
    type: 'integer',
    ...options,
  };
}

/**
 * Creates a JSON schema for a boolean.
 * @param options Optional description for the boolean schema.
 * @returns A `JSONSchemaBoolean` object.
 */
export function createBooleanSchema(options?: {
  description?: string;
}): JSONSchemaBoolean {
  return {
    type: 'boolean',
    ...options,
  };
}

/**
 * Creates a JSON schema for an array.
 * @param options Optional constraints for the array schema.
 * @returns A `JSONSchemaArray` object.
 */
export function createArraySchema(options?: {
  description?: string;
  items?: JSONSchema;
  minItems?: number;
  maxItems?: number;
  uniqueItems?: boolean;
}): JSONSchemaArray {
  return {
    type: 'array',
    ...options,
  };
}

/**
 * Creates a JSON schema for an object.
 * @param options Optional constraints for the object schema.
 * @returns A `JSONSchemaObject` object.
 */
export function createObjectSchema(options?: {
  description?: string;
  properties?: Record<string, JSONSchema>;
  required?: string[];
  additionalProperties?: boolean;
}): JSONSchemaObject {
  return {
    type: 'object',
    ...options,
  };
}
