import { describe, it, expect } from 'vitest';
import type { MCPContent, MCPTextContent } from '../content';

describe('MCP Content Types', () => {
  it('represents plain text content without item-level isError', () => {
    const textContent: MCPTextContent = {
      type: 'text',
      text: 'Regular message',
    };

    const imageContent: MCPContent = {
      type: 'image',
      data: 'base64...',
      mimeType: 'image/png',
    };

    expect(textContent.type).toBe('text');
    expect(imageContent.type).toBe('image');
    expect('isError' in textContent).toBe(false);
  });
});
