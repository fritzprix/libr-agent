import { describe, it, expect } from 'vitest';
import { isMCPErrorContent } from '../content';
import { MCPContent, MCPErrorContent, MCPTextContent } from '../content';

describe('MCP Content Types', () => {
  describe('isMCPErrorContent', () => {
    const errorContent: MCPErrorContent = {
      type: 'text',
      text: 'Error message',
      isError: true,
    };

    const textContent: MCPTextContent = {
      type: 'text',
      text: 'Regular message',
    };

    const imageContent: MCPContent = {
      type: 'image',
      data: 'base64...',
      mimeType: 'image/png',
    };

    it('should return true for error content', () => {
      expect(isMCPErrorContent(errorContent)).toBe(true);
    });

    it('should return false for regular text content', () => {
      expect(isMCPErrorContent(textContent)).toBe(false);
    });

    it('should return false for other content types', () => {
      expect(isMCPErrorContent(imageContent)).toBe(false);
    });
  });
});
