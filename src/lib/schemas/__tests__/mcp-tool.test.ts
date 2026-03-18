import { describe, it, expect } from 'vitest';
import { isMCPTool, validateMCPTools, parseMCPTool } from '../mcp-tool';

describe('MCP Tool Validation', () => {
  const validTool = {
    name: 'test_tool',
    description: 'A test tool',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  };

  describe('isMCPTool', () => {
    it('returns true for a valid MCP tool', () => {
      expect(isMCPTool(validTool)).toBe(true);
    });

    it('returns false for null', () => {
      expect(isMCPTool(null)).toBe(false);
    });

    it('returns false for non-objects', () => {
      expect(isMCPTool('string')).toBe(false);
      expect(isMCPTool(123)).toBe(false);
      expect(isMCPTool(true)).toBe(false);
      expect(isMCPTool(undefined)).toBe(false);
    });

    it('returns false if name is missing or not a string', () => {
      expect(isMCPTool({ ...validTool, name: undefined })).toBe(false);
      expect(isMCPTool({ ...validTool, name: 123 })).toBe(false);
    });

    it('returns false if description is missing or not a string', () => {
      expect(isMCPTool({ ...validTool, description: undefined })).toBe(false);
      expect(isMCPTool({ ...validTool, description: 123 })).toBe(false);
    });

    it('returns false if inputSchema is missing, null, or not an object', () => {
      expect(isMCPTool({ ...validTool, inputSchema: undefined })).toBe(false);
      expect(isMCPTool({ ...validTool, inputSchema: null })).toBe(false);
      expect(isMCPTool({ ...validTool, inputSchema: 'string' })).toBe(false);
    });
  });

  describe('validateMCPTools', () => {
    it('returns an array of only valid tools', () => {
      const invalidTool1 = { name: 'tool1' }; // missing description and inputSchema
      const invalidTool2 = null;
      const validTool2 = {
        name: 'test_tool_2',
        description: 'Another test tool',
        inputSchema: {},
      };

      const result = validateMCPTools([validTool, invalidTool1, invalidTool2, validTool2]);

      expect(result).toHaveLength(2);
      expect(result[0]).toEqual(validTool);
      expect(result[1]).toEqual(validTool2);
    });

    it('returns an empty array if no valid tools are present', () => {
      expect(validateMCPTools([null, undefined, 'string'])).toEqual([]);
    });

    it('returns an empty array if input is empty', () => {
      expect(validateMCPTools([])).toEqual([]);
    });
  });

  describe('parseMCPTool', () => {
    it('returns the tool if it is valid', () => {
      expect(parseMCPTool(validTool)).toEqual(validTool);
    });

    it('returns undefined if the tool is invalid', () => {
      expect(parseMCPTool({ name: 'tool' })).toBeUndefined();
      expect(parseMCPTool(null)).toBeUndefined();
    });
  });
});
