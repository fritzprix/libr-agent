import { describe, it, expect, vi } from 'vitest';
import { listInteractableTool } from '../ListInteractableTool';

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('ListInteractableTool', () => {
  it('should have correct tool name and schema', () => {
    expect(listInteractableTool.name).toBe('listInteractable');
    expect(listInteractableTool.inputSchema.required).toContain(
      'sessionId',
    );
  });

  it('should return error when sessionId is missing', async () => {
    const result = await listInteractableTool.execute({}, undefined);

    expect(result).toHaveProperty('error');
    expect(result.error?.message).toContain('Invalid sessionId');
  });

  it('should handle semantic_clickable filter with viewport scope', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue(
      JSON.stringify([
        {
          index: 0,
          tag: 'button',
          text: 'Submit',
          attributes: { id: 'btn1', class: 'primary' },
          selector: '#btn1',
        },
      ]),
    );

    const result = await listInteractableTool.execute(
      {
        sessionId: 'test-session',
        filterType: 'semantic_clickable',
        scope: 'viewport',
      },
      mockExecuteScript,
    );

    expect(result).toHaveProperty('result');
    const content = result.result?.content?.[0];
    expect(content).toBeDefined();
    if (content && 'text' in content) {
      expect(content.text).toContain('Found 1');
      expect(content.text).toContain('semantic clickable');
      expect(content.text).toContain('viewport');
    }
    expect(result.result?.structuredContent).toEqual({
      elementCount: 1,
      filterType: 'semantic_clickable',
      scope: 'viewport',
      sessionId: 'test-session',
    });
  });

  it('should handle empty results gracefully', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue(JSON.stringify([]));

    const result = await listInteractableTool.execute(
      { sessionId: 'test-session', filterType: 'semantic_input', scope: 'all' },
      mockExecuteScript,
    );

    const content = result.result?.content?.[0];
    expect(content).toBeDefined();
    if (content && 'text' in content) {
      expect(content.text).toContain('No semantic input elements found');
    }
    const structuredContent = result.result?.structuredContent as {
      elementCount: number;
    };
    expect(structuredContent.elementCount).toBe(0);
  });

  it('should use default values when parameters are missing', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue(JSON.stringify([]));

    await listInteractableTool.execute(
      { sessionId: 'test-session' }, // No filterType or scope
      mockExecuteScript,
    );

    const scriptCall = mockExecuteScript.mock.calls[0][1] as string;
    expect(scriptCall).toContain('semantic_clickable'); // Default filterType
    expect(scriptCall).toContain("'viewport'"); // Default scope
  });

  it('should handle JSON parse errors', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue('invalid json{{{');

    const result = await listInteractableTool.execute(
      { sessionId: 'test-session' },
      mockExecuteScript,
    );

    expect(result).toHaveProperty('error');
    expect(result.error?.message).toContain('Failed to parse');
  });

  it('should validate filterType enum', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue(JSON.stringify([]));

    await listInteractableTool.execute(
      {
        sessionId: 'test-session',
        filterType: 'invalid_type',
        scope: 'viewport',
      },
      mockExecuteScript,
    );

    // Should fall back to default 'semantic_clickable'
    const scriptCall = mockExecuteScript.mock.calls[0][1] as string;
    expect(scriptCall).toContain('semantic_clickable');
  });

  it('should validate scope enum', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue(JSON.stringify([]));

    await listInteractableTool.execute(
      {
        sessionId: 'test-session',
        filterType: 'semantic_clickable',
        scope: 'invalid_scope',
      },
      mockExecuteScript,
    );

    // Should fall back to default 'viewport'
    const scriptCall = mockExecuteScript.mock.calls[0][1] as string;
    expect(scriptCall).toContain("'viewport'");
  });

  it('should handle all three filter types', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue(JSON.stringify([]));

    // Test semantic_clickable
    await listInteractableTool.execute(
      { sessionId: 'test', filterType: 'semantic_clickable' },
      mockExecuteScript,
    );
    expect(mockExecuteScript.mock.calls[0][1]).toContain('semantic_clickable');

    // Test semantic_input
    await listInteractableTool.execute(
      { sessionId: 'test', filterType: 'semantic_input' },
      mockExecuteScript,
    );
    expect(mockExecuteScript.mock.calls[1][1]).toContain('semantic_input');

    // Test all_focusable
    await listInteractableTool.execute(
      { sessionId: 'test', filterType: 'all_focusable' },
      mockExecuteScript,
    );
    expect(mockExecuteScript.mock.calls[2][1]).toContain('all_focusable');
  });

  it('should handle both scope options', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue(JSON.stringify([]));

    // Test viewport scope
    await listInteractableTool.execute(
      { sessionId: 'test', scope: 'viewport' },
      mockExecuteScript,
    );
    expect(mockExecuteScript.mock.calls[0][1]).toContain("'viewport'");

    // Test all scope
    await listInteractableTool.execute(
      { sessionId: 'test', scope: 'all' },
      mockExecuteScript,
    );
    expect(mockExecuteScript.mock.calls[1][1]).toContain("'all'");
  });

  it('should return error when executeScript is undefined', async () => {
    const result = await listInteractableTool.execute(
      { sessionId: 'test-session' },
      undefined,
    );

    expect(result).toHaveProperty('error');
    expect(result.error?.message).toContain(
      'Browser script execution not available',
    );
  });

  it('should format multiple elements correctly', async () => {
    const mockExecuteScript = vi.fn().mockResolvedValue(
      JSON.stringify([
        {
          index: 0,
          tag: 'a',
          text: 'Home',
          attributes: { href: '/', class: 'nav-link' },
          selector: 'a.nav-link',
        },
        {
          index: 1,
          tag: 'button',
          text: 'Login',
          attributes: { id: 'login-btn', type: 'button' },
          selector: '#login-btn',
        },
      ]),
    );

    const result = await listInteractableTool.execute(
      { sessionId: 'test-session' },
      mockExecuteScript,
    );

    const content = result.result?.content?.[0];
    expect(content).toBeDefined();
    if (content && 'text' in content) {
      expect(content.text).toContain('Found 2');
      expect(content.text).toContain('[0]');
      expect(content.text).toContain('[1]');
      expect(content.text).toContain('Home');
      expect(content.text).toContain('Login');
    }
    const structuredContent = result.result?.structuredContent as {
      elementCount: number;
    };
    expect(structuredContent.elementCount).toBe(2);
  });
});
