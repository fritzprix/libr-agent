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
  describe('createStringSchema', () => {
    it('creates a basic string schema', () => {
      expect(createStringSchema()).toEqual({ type: 'string' });
    });

    it('creates a string schema with options', () => {
      const schema = createStringSchema({
        description: 'A test string',
        minLength: 1,
        maxLength: 10,
        pattern: '^[a-z]+$',
        format: 'email',
      });
      expect(schema).toEqual({
        type: 'string',
        description: 'A test string',
        minLength: 1,
        maxLength: 10,
        pattern: '^[a-z]+$',
        format: 'email',
      });
    });
  });

  describe('createNumberSchema', () => {
    it('creates a basic number schema', () => {
      expect(createNumberSchema()).toEqual({ type: 'number' });
    });

    it('creates a number schema with options', () => {
      const schema = createNumberSchema({
        description: 'A test number',
        minimum: 0,
        maximum: 100,
        exclusiveMinimum: 0,
        exclusiveMaximum: 100,
        multipleOf: 2,
      });
      expect(schema).toEqual({
        type: 'number',
        description: 'A test number',
        minimum: 0,
        maximum: 100,
        exclusiveMinimum: 0,
        exclusiveMaximum: 100,
        multipleOf: 2,
      });
    });
  });

  describe('createIntegerSchema', () => {
    it('creates a basic integer schema', () => {
      expect(createIntegerSchema()).toEqual({ type: 'integer' });
    });

    it('creates an integer schema with options', () => {
      const schema = createIntegerSchema({
        description: 'A test integer',
        minimum: -10,
        maximum: 10,
      });
      expect(schema).toEqual({
        type: 'integer',
        description: 'A test integer',
        minimum: -10,
        maximum: 10,
      });
    });
  });

  describe('createBooleanSchema', () => {
    it('creates a basic boolean schema', () => {
      expect(createBooleanSchema()).toEqual({ type: 'boolean' });
    });

    it('creates a boolean schema with options', () => {
      const schema = createBooleanSchema({
        description: 'A test boolean',
      });
      expect(schema).toEqual({
        type: 'boolean',
        description: 'A test boolean',
      });
    });
  });

  describe('createArraySchema', () => {
    it('creates a basic array schema', () => {
      expect(createArraySchema()).toEqual({ type: 'array' });
    });

    it('creates an array schema with options', () => {
      const schema = createArraySchema({
        description: 'A test array',
        items: { type: 'string' },
        minItems: 1,
        maxItems: 5,
        uniqueItems: true,
      });
      expect(schema).toEqual({
        type: 'array',
        description: 'A test array',
        items: { type: 'string' },
        minItems: 1,
        maxItems: 5,
        uniqueItems: true,
      });
    });
  });

  describe('createObjectSchema', () => {
    it('creates a basic object schema', () => {
      expect(createObjectSchema()).toEqual({ type: 'object' });
    });

    it('creates an object schema with options', () => {
      const schema = createObjectSchema({
        description: 'A test object',
        properties: {
          name: { type: 'string' },
          age: { type: 'integer' },
        },
        required: ['name'],
        additionalProperties: false,
      });
      expect(schema).toEqual({
        type: 'object',
        description: 'A test object',
        properties: {
          name: { type: 'string' },
          age: { type: 'integer' },
        },
        required: ['name'],
        additionalProperties: false,
      });
    });
  });

  describe('createEnumSchema', () => {
    it('creates a basic enum schema', () => {
      expect(createEnumSchema(['a', 'b', 'c'])).toEqual({
        type: 'string',
        enum: ['a', 'b', 'c'],
      });
    });

    it('creates an enum schema with options', () => {
      const schema = createEnumSchema(['yes', 'no'], {
        description: 'A test enum',
        default: 'yes',
      });
      expect(schema).toEqual({
        type: 'string',
        enum: ['yes', 'no'],
        description: 'A test enum',
        default: 'yes',
      });
    });
  });
});
