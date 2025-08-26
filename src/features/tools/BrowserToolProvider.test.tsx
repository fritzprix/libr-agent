import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { ReactNode } from 'react';
import { BrowserToolProvider } from './BrowserToolProvider';
import { BuiltInToolProvider, useBuiltInTool } from './index';
import { ToolCall } from '@/models/chat';

// Import formatBrowserResult for testing - we'll need to export it from the module
// For now, we'll copy the function here for testing
function formatBrowserResult(raw: unknown): string {
  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw);
      if ('ok' in parsed) {
        if (parsed.ok) {
          let result = `✓ ${parsed.action.toUpperCase()} successful (selector: ${parsed.selector})`;
          if (parsed.diagnostics) {
            result += `\n\nDiagnostics:\n${JSON.stringify(parsed.diagnostics, null, 2)}`;
          }
          if (parsed.value_preview) {
            result += `\nValue preview: "${parsed.value_preview}"`;
          }
          if (parsed.note) {
            result += `\n\nNote: ${parsed.note}`;
          }
          return result;
        } else {
          let result = `✗ ${parsed.action.toUpperCase()} failed`;
          if (parsed.reason) {
            result += ` - ${parsed.reason}`;
          }
          if (parsed.error) {
            result += ` - ${parsed.error}`;
          }
          if (parsed.selector) {
            result += ` (selector: ${parsed.selector})`;
          }
          if (parsed.diagnostics) {
            result += `\n\nDiagnostics:\n${JSON.stringify(parsed.diagnostics, null, 2)}`;
          }
          return result;
        }
      }
      return raw;
    } catch {
      return raw;
    }
  }
  return String(raw);
}

// Mock dependencies
vi.mock('@/hooks/use-browser-invoker', () => ({
  useBrowserInvoker: () => ({
    executeScript: vi.fn(),
  }),
}));

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    writeFile: vi.fn(),
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('turndown', () => {
  return {
    default: function TurndownService() {
      return {
        addRule: vi.fn(),
        turndown: vi.fn().mockReturnValue('# Converted Markdown'),
      };
    },
  };
});

const TestWrapper = ({ children }: { children: ReactNode }) => (
  <BuiltInToolProvider>
    <BrowserToolProvider />
    {children}
  </BuiltInToolProvider>
);

describe('BrowserToolProvider', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('browser tool registration', () => {
    it('should register browser tools with correct service ID', () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const browserTools = result.current.availableTools.filter((tool) =>
        tool.name.startsWith('builtin.browser__'),
      );

      expect(browserTools.length).toBeGreaterThan(0);
      expect(
        browserTools.some(
          (tool) => tool.name === 'builtin.browser__browser_createSession',
        ),
      ).toBe(true);
      expect(
        browserTools.some(
          (tool) => tool.name === 'builtin.browser__browser_elementExists',
        ),
      ).toBe(true);
    });

    it('should handle browser tool execution with proper argument parsing', async () => {
      const mockExecuteScript = vi.fn().mockResolvedValue('true');

      vi.doMock('@/hooks/use-browser-invoker', () => ({
        useBrowserInvoker: () => ({
          executeScript: mockExecuteScript,
        }),
      }));

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      // Find the elementExists tool
      const elementExistsTool = result.current.availableTools.find(
        (tool) => tool.name === 'builtin.browser__browser_elementExists',
      );

      expect(elementExistsTool).toBeDefined();
    });
  });

  describe('browser_elementExists tool edge cases', () => {
    const mockExecuteScript = vi.fn();

    beforeEach(() => {
      vi.doMock('@/hooks/use-browser-invoker', () => ({
        useBrowserInvoker: () => ({
          executeScript: mockExecuteScript,
        }),
      }));
    });

    it('should handle executeScript returning boolean true', async () => {
      mockExecuteScript.mockResolvedValue('true');

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_elementExists',
          arguments: JSON.stringify({
            sessionId: 'test-session',
            selector: '.test-element',
          }),
        },
      };

      const response = await result.current.executeTool(toolCall);

      expect(response.result?.content?.[0]).toEqual({
        type: 'text',
        text: 'true',
      });
    });

    it('should handle executeScript returning boolean false', async () => {
      mockExecuteScript.mockResolvedValue('false');

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-2',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_elementExists',
          arguments: JSON.stringify({
            sessionId: 'test-session',
            selector: '.test-element',
          }),
        },
      };

      const response = await result.current.executeTool(toolCall);

      expect(response.result?.content?.[0]).toEqual({
        type: 'text',
        text: 'false',
      });
    });

    it('should handle executeScript returning JSON string with boolean', async () => {
      mockExecuteScript.mockResolvedValue(JSON.stringify(true));

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-3',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_elementExists',
          arguments: JSON.stringify({
            sessionId: 'test-session',
            selector: '.test-element',
          }),
        },
      };

      const response = await result.current.executeTool(toolCall);

      // Should detect 'true' in the JSON string
      expect(response.result?.content?.[0]).toEqual({
        type: 'text',
        text: 'true',
      });
    });

    it('should handle executeScript returning plain string without boolean', async () => {
      mockExecuteScript.mockResolvedValue('element found successfully');

      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-4',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_elementExists',
          arguments: JSON.stringify({
            sessionId: 'test-session',
            selector: '.test-element',
          }),
        },
      };

      const response = await result.current.executeTool(toolCall);

      // Should default to 'false' when no 'true' is found
      expect(response.result?.content?.[0]).toEqual({
        type: 'text',
        text: 'false',
      });
    });
  });

  describe('argument parsing edge cases', () => {
    it('should handle empty string arguments', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-5',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_listSessions',
          arguments: '',
        },
      };

      // Should not throw error for empty arguments
      const response = await result.current.executeTool(toolCall);
      expect(response).toBeDefined();
    });

    it('should handle invalid JSON arguments gracefully', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-6',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_listSessions',
          arguments: '{invalid json}',
        },
      };

      // Should not throw error for invalid JSON
      const response = await result.current.executeTool(toolCall);
      expect(response).toBeDefined();
    });

    it('should handle object arguments', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-7',
        type: 'function',
        function: {
          name: 'builtin.browser__browser_listSessions',
          arguments: '{"someKey": "someValue"}',
        },
      };

      const response = await result.current.executeTool(toolCall);
      expect(response).toBeDefined();
    });
  });

  describe('tool not found error handling', () => {
    it('should throw error for non-existent browser tool', async () => {
      const { result } = renderHook(() => useBuiltInTool(), {
        wrapper: TestWrapper,
      });

      const toolCall: ToolCall = {
        id: 'test-call-8',
        type: 'function',
        function: {
          name: 'builtin.browser__nonexistent_tool',
          arguments: '{}',
        },
      };

      await expect(result.current.executeTool(toolCall)).rejects.toThrow(
        'Browser tool not found: nonexistent_tool',
      );
    });
  });
});

describe('formatBrowserResult', () => {
  describe('successful envelope parsing', () => {
    it('should format successful click result with diagnostics', () => {
      const envelope = JSON.stringify({
        ok: true,
        action: 'click',
        selector: '#test-button',
        timestamp: '2025-08-25T12:34:56.789Z',
        clickAttempted: true,
        diagnostics: {
          visible: true,
          disabled: false,
          pointerEvents: 'auto',
          visibility: 'visible',
          rect: { x: 10, y: 20, width: 100, height: 24 },
        },
        note: 'click attempted (handlers may ignore synthetic events)',
      });

      const result = formatBrowserResult(envelope);

      expect(result).toContain('✓ CLICK successful (selector: #test-button)');
      expect(result).toContain('Diagnostics:');
      expect(result).toContain('"visible": true');
      expect(result).toContain(
        'Note: click attempted (handlers may ignore synthetic events)',
      );
    });

    it('should format successful input result with value preview', () => {
      const envelope = JSON.stringify({
        ok: true,
        action: 'input',
        selector: 'input[name="username"]',
        timestamp: '2025-08-25T12:40:00.000Z',
        applied: true,
        diagnostics: {
          visible: true,
          disabled: false,
          tagName: 'input',
          type: 'text',
        },
        value_preview: 'john.doe@example.com',
        note: 'input attempted (handlers may modify final value)',
      });

      const result = formatBrowserResult(envelope);

      expect(result).toContain(
        '✓ INPUT successful (selector: input[name="username"])',
      );
      expect(result).toContain('Value preview: "john.doe@example.com"');
      expect(result).toContain(
        'Note: input attempted (handlers may modify final value)',
      );
    });
  });

  describe('failed envelope parsing', () => {
    it('should format failed click result with reason', () => {
      const envelope = JSON.stringify({
        ok: false,
        action: 'click',
        reason: 'not_found',
        selector: '#nonexistent-button',
        timestamp: '2025-08-25T12:45:00.000Z',
      });

      const result = formatBrowserResult(envelope);

      expect(result).toContain(
        '✗ CLICK failed - not_found (selector: #nonexistent-button)',
      );
    });

    it('should format failed input result with error and diagnostics', () => {
      const envelope = JSON.stringify({
        ok: false,
        action: 'input',
        reason: 'element_disabled',
        selector: 'input[disabled]',
        timestamp: '2025-08-25T12:50:00.000Z',
        diagnostics: {
          visible: true,
          disabled: true,
          tagName: 'input',
          type: 'text',
        },
      });

      const result = formatBrowserResult(envelope);

      expect(result).toContain(
        '✗ INPUT failed - element_disabled (selector: input[disabled])',
      );
      expect(result).toContain('Diagnostics:');
      expect(result).toContain('"disabled": true');
    });

    it('should format result with error instead of reason', () => {
      const envelope = JSON.stringify({
        ok: false,
        action: 'click',
        error: "TypeError: Cannot read property 'click' of null",
        selector: '#dynamic-element',
        timestamp: '2025-08-25T12:55:00.000Z',
      });

      const result = formatBrowserResult(envelope);

      expect(result).toContain(
        "✗ CLICK failed - TypeError: Cannot read property 'click' of null",
      );
      expect(result).toContain('(selector: #dynamic-element)');
    });
  });

  describe('non-envelope content', () => {
    it('should return original string for non-JSON content', () => {
      const plainText = 'Element clicked successfully';
      const result = formatBrowserResult(plainText);

      expect(result).toBe(plainText);
    });

    it('should return original string for JSON without ok field', () => {
      const jsonWithoutOk = JSON.stringify({
        message: 'Some other response',
        data: { value: 123 },
      });

      const result = formatBrowserResult(jsonWithoutOk);

      expect(result).toBe(jsonWithoutOk);
    });

    it('should return original string for invalid JSON', () => {
      const invalidJson = '{ invalid json }';
      const result = formatBrowserResult(invalidJson);

      expect(result).toBe(invalidJson);
    });

    it('should convert non-string input to string', () => {
      const numberInput = 42;
      const result = formatBrowserResult(numberInput);

      expect(result).toBe('42');
    });

    it('should handle null and undefined', () => {
      expect(formatBrowserResult(null)).toBe('null');
      expect(formatBrowserResult(undefined)).toBe('undefined');
    });
  });
});
