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
 * @param options.description The description of the string schema.
 * @param options.minLength The minimum length of the string.
 * @param options.maxLength The maximum length of the string.
 * @param options.pattern The pattern of the string.
 * @param options.format The format of the string.
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
 * @param options.description The description of the number schema.
 * @param options.minimum The minimum value of the number.
 * @param options.maximum The maximum value of the number.
 * @param options.exclusiveMinimum The exclusive minimum value of the number.
 * @param options.exclusiveMaximum The exclusive maximum value of the number.
 * @param options.multipleOf The multiple of the number.
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
 * @param options.description The description of the integer schema.
 * @param options.minimum The minimum value of the integer.
 * @param options.maximum The maximum value of the integer.
 * @param options.exclusiveMinimum The exclusive minimum value of the integer.
 * @param options.exclusiveMaximum The exclusive maximum value of the integer.
 * @param options.multipleOf The multiple of the integer.
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
 * @param options.description The description of the boolean schema.
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
 * @param options.description The description of the array schema.
 * @param options.items The items of the array schema.
 * @param options.minItems The minimum items of the array schema.
 * @param options.maxItems The maximum items of the array schema.
 * @param options.uniqueItems The unique items of the array schema.
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
 * @param options.description The description of the object schema.
 * @param options.properties The properties of the object schema.
 * @param options.required The required properties of the object schema.
 * @param options.additionalProperties The additional properties of the object schema.
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

/**
 * Creates a JSON schema for an enum (string with restricted values).
 * @param values Array of allowed string values.
 * @param options Optional description and default value.
 * @param options.description The description of the enum schema.
 * @param options.default The default value of the enum schema.
 * @returns A `JSONSchemaString` object with enum constraint.
 */
export function createEnumSchema(
  values: string[],
  options?: {
    description?: string;
    default?: string;
  },
): JSONSchemaString {
  return {
    type: 'string',
    enum: values,
    ...options,
  };
}
