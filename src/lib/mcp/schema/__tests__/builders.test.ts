import { describe, it, expect } from 'vitest';
import {
  createStringSchema,
  createNumberSchema,
  createIntegerSchema,
  createBooleanSchema,
  createArraySchema,
  createObjectSchema,
  createEnumSchema,
} from '../builders';

describe('JSON Schema Builders', () => {
  it('should create string schema', () => {
    expect(createStringSchema({ minLength: 1, maxLength: 10, pattern: '^[a-z]+$', format: 'email', description: 'test string' })).toEqual({
      type: 'string',
      minLength: 1,
      maxLength: 10,
      pattern: '^[a-z]+$',
      format: 'email',
      description: 'test string',
    });

    expect(createStringSchema()).toEqual({ type: 'string' });
  });

  it('should create number schema', () => {
    expect(createNumberSchema({ minimum: 0, maximum: 100, exclusiveMinimum: 1, exclusiveMaximum: 99, multipleOf: 5, description: 'test number' })).toEqual({
      type: 'number',
      minimum: 0,
      maximum: 100,
      exclusiveMinimum: 1,
      exclusiveMaximum: 99,
      multipleOf: 5,
      description: 'test number',
    });

    expect(createNumberSchema()).toEqual({ type: 'number' });
  });

  it('should create integer schema', () => {
    expect(createIntegerSchema({ minimum: 1, multipleOf: 2, maximum: 10, exclusiveMinimum: 2, exclusiveMaximum: 9, description: 'test int' })).toEqual({
      type: 'integer',
      minimum: 1,
      multipleOf: 2,
      maximum: 10,
      exclusiveMinimum: 2,
      exclusiveMaximum: 9,
      description: 'test int',
    });

    expect(createIntegerSchema()).toEqual({ type: 'integer' });
  });

  it('should create boolean schema', () => {
    expect(createBooleanSchema({ description: 'A flag' })).toEqual({
      type: 'boolean',
      description: 'A flag',
    });

    expect(createBooleanSchema()).toEqual({ type: 'boolean' });
  });

  it('should create array schema', () => {
    const itemsSchema = createStringSchema();
    expect(createArraySchema({ items: itemsSchema, minItems: 1, maxItems: 10, uniqueItems: true, description: 'test array' })).toEqual({
      type: 'array',
      items: { type: 'string' },
      minItems: 1,
      maxItems: 10,
      uniqueItems: true,
      description: 'test array',
    });

    expect(createArraySchema()).toEqual({ type: 'array' });
  });

  it('should create object schema', () => {
    const properties = {
      name: createStringSchema(),
      age: createIntegerSchema(),
    };
    expect(createObjectSchema({ properties, required: ['name'], additionalProperties: false, description: 'test object' })).toEqual({
      type: 'object',
      properties,
      required: ['name'],
      additionalProperties: false,
      description: 'test object',
    });

    expect(createObjectSchema()).toEqual({ type: 'object' });
  });

  it('should create enum schema', () => {
    expect(createEnumSchema(['RED', 'BLUE'], { default: 'RED', description: 'test enum' })).toEqual({
      type: 'string',
      enum: ['RED', 'BLUE'],
      default: 'RED',
      description: 'test enum',
    });

    expect(createEnumSchema(['RED'])).toEqual({
      type: 'string',
      enum: ['RED'],
    });
  });
});
