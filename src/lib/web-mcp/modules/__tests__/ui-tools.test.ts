import { describe, it, expect, beforeEach, vi } from 'vitest';
import uiTools from '../ui-tools';
import type { MCPResponse, MCPResult, JSONSchemaObject } from '@/lib/mcp-types';

interface UIResourceWithContent {
  serviceInfo?: { serverName: string; toolName: string; backendType: string };
  resource?: {
    uri: string;
    text?: string; // HTML content is stored in text property
  };
  type: 'resource';
}

interface TestMCPResponse extends MCPResponse<unknown> {
  result: MCPResult<unknown> & {
    content: Array<{ type: string; text?: string } | UIResourceWithContent>;
    structuredContent?: unknown;
  };
}

describe('UI Tools - Wait UI Functionality', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('wait_for_user_resume tool', () => {
    it('returns multipart response with text and uiResource', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Testing wait',
        context: {
          reason: 'Test reason',
          command: 'test command',
          nextAction: 'test next action',
        },
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(2);
      expect(response.result.content[0].type).toBe('text');
      expect(response.result.content[1].type).toBe('resource');

      // Check text content
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('⏳ Waiting: Testing wait');
      expect(textContent.text).toContain('Reason: Test reason');

      // Check metadata
      expect(response.result.structuredContent).toMatchObject({
        waiting: true,
        context: expect.objectContaining({
          reason: 'Test reason',
          command: 'test command',
          nextAction: 'test next action',
          startedAt: expect.any(String),
        }),
      });
    });

    it('automatically adds startedAt timestamp', async () => {
      const before = new Date().toISOString();
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        context: {
          reason: 'Test',
          command: 'test',
          nextAction: 'test',
        },
      })) as TestMCPResponse;
      const after = new Date().toISOString();

      const metadata = response.result.structuredContent as {
        context: { startedAt: string };
      };
      expect(metadata.context.startedAt).toBeDefined();
      expect(metadata.context.startedAt >= before).toBe(true);
      expect(metadata.context.startedAt <= after).toBe(true);
    });

    it('rejects empty message', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: '',
        context: {
          reason: 'Test',
          command: 'test',
          nextAction: 'test',
        },
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      expect(response.result.content[0].type).toBe('text');
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('Message is required');
    });

    it('rejects incomplete context - missing reason', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        context: {
          command: 'test',
          nextAction: 'test',
        },
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain(
        'Context with reason, command, nextAction required',
      );
    });

    it('rejects incomplete context - missing command', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        context: {
          reason: 'Test',
          nextAction: 'test',
        },
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain(
        'Context with reason, command, nextAction required',
      );
    });

    it('rejects incomplete context - missing nextAction', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        context: {
          reason: 'Test',
          command: 'test',
        },
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain(
        'Context with reason, command, nextAction required',
      );
    });

    it('generates valid UIResource', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test message',
        context: {
          reason: 'Test reason',
          command: 'test cmd',
          nextAction: 'next',
        },
      })) as TestMCPResponse;

      const uiResource = response.result.content[1] as UIResourceWithContent;
      expect(uiResource.type).toBe('resource');
      expect(uiResource.resource).toBeDefined();
      expect(uiResource.resource?.uri).toMatch(/^ui:\/\/wait\/\d+$/);
      expect(uiResource.serviceInfo).toMatchObject({
        serverName: 'ui',
        toolName: '',
        backendType: 'BuiltInWeb',
      });
    });
  });

  describe('resume_from_wait tool', () => {
    it('returns formatted agent message', async () => {
      const context = {
        startedAt: '2025-10-11T17:00:00Z',
        reason: 'Test wait',
        command: 'npm install',
        nextAction: 'Continue deployment',
      };

      const response = (await uiTools.callTool('resume_from_wait', {
        context,
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('✅ User resumed after waiting');
      expect(textContent.text).toContain('Test wait');
      expect(textContent.text).toContain('npm install');
      expect(textContent.text).toContain('Continue deployment');
      expect(textContent.text).toContain('What was waiting: Test wait');
      expect(textContent.text).toContain('Command/Task: npm install');
      expect(textContent.text).toContain('Next action: Continue deployment');
    });

    it('calculates duration correctly', async () => {
      const startedAt = new Date(Date.now() - 65000).toISOString(); // 1 minute 5 seconds ago
      const response = (await uiTools.callTool('resume_from_wait', {
        context: {
          startedAt,
          reason: 'Test',
          command: 'test',
          nextAction: 'test',
        },
      })) as TestMCPResponse;

      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toMatch(/1m \d+s/);

      const metadata = response.result.structuredContent as {
        duration: string;
      };
      expect(metadata.duration).toMatch(/1m \d+s/);
    });

    it('includes proper metadata', async () => {
      const context = {
        startedAt: '2025-10-11T17:00:00Z',
        reason: 'Test reason',
        command: 'test cmd',
        nextAction: 'next action',
      };

      const response = (await uiTools.callTool('resume_from_wait', {
        context,
      })) as TestMCPResponse;

      expect(response.result.structuredContent).toMatchObject({
        resumed: true,
        duration: expect.any(String),
        startedAt: '2025-10-11T17:00:00Z',
        reason: 'Test reason',
        command: 'test cmd',
        nextAction: 'next action',
        resumedAt: expect.any(String),
      });
    });

    it('rejects missing context', async () => {
      const response = (await uiTools.callTool(
        'resume_from_wait',
        {},
      )) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('Valid context required');
    });

    it('rejects context without startedAt', async () => {
      const response = (await uiTools.callTool('resume_from_wait', {
        context: {
          reason: 'Test',
          command: 'test',
          nextAction: 'test',
        },
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('Valid context required');
    });
  });

  describe('Helper functions', () => {
    it('formatDuration handles seconds correctly', async () => {
      // We test the internal formatDuration function through the resume_from_wait tool
      const testDurations = [
        { ms: 5000, expected: /5s/ },
        { ms: 65000, expected: /1m 5s/ },
        { ms: 3665000, expected: /1h 1m/ },
      ];

      for (const { ms, expected } of testDurations) {
        const startedAt = new Date(Date.now() - ms).toISOString();
        const response = (await uiTools.callTool('resume_from_wait', {
          context: {
            startedAt,
            reason: 'Test',
            command: 'test',
            nextAction: 'test',
          },
        })) as TestMCPResponse;

        const metadata = response.result.structuredContent as {
          duration: string;
        };
        expect(metadata.duration).toMatch(expected);
      }
    });
  });

  describe('HTML generation', () => {
    it('generates valid HTML structure', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test message',
        context: {
          reason: 'Test',
          command: 'test cmd',
          nextAction: 'next',
        },
      })) as TestMCPResponse;

      const uiResource = response.result.content[1] as UIResourceWithContent;
      const htmlContent = uiResource.resource?.text ?? '';

      expect(htmlContent).toContain('<!doctype html>');
      expect(htmlContent).toContain('Test message');
      expect(htmlContent).toContain('role="dialog"');
      expect(htmlContent).toContain('aria-modal="true"');
      expect(htmlContent).toContain('resume_from_wait');
      expect(htmlContent).toContain('postMessage');
      expect(htmlContent).toContain('계속'); // Continue button in Korean
    });

    it('properly escapes HTML in message', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: '<script>alert("xss")</script>',
        context: {
          reason: 'Test',
          command: 'test cmd',
          nextAction: 'next',
        },
      })) as TestMCPResponse;

      const uiResource = response.result.content[1] as UIResourceWithContent;
      const htmlContent = uiResource.resource?.text ?? '';

      expect(htmlContent).toContain(
        '&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;',
      );
      expect(htmlContent).not.toContain('<script>alert("xss")</script>');
    });

    it('includes accessibility features', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        context: {
          reason: 'Test',
          command: 'test',
          nextAction: 'test',
        },
      })) as TestMCPResponse;

      const uiResource = response.result.content[1] as UIResourceWithContent;
      const htmlContent = uiResource.resource?.text ?? '';

      expect(htmlContent).toContain('aria-modal="true"');
      expect(htmlContent).toContain('autofocus');
      expect(htmlContent).toContain('aria-labelledby');
      expect(htmlContent).toContain('aria-hidden="true"'); // for spinner
      expect(htmlContent).toContain('@media (prefers-reduced-motion: reduce)');
    });

    it('embeds context in JavaScript correctly', async () => {
      const context = {
        reason: 'Test reason',
        command: 'test "quoted" command',
        nextAction: 'next action',
      };

      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        context,
      })) as TestMCPResponse;

      const uiResource = response.result.content[1] as UIResourceWithContent;
      const htmlContent = uiResource.resource?.text ?? '';

      // Should contain properly escaped JSON
      expect(htmlContent).toContain('const context = ');
      expect(htmlContent).toContain('Test reason');
      expect(htmlContent).toContain('test \\&quot;quoted\\&quot; command');
    });
  });

  describe('Error handling', () => {
    it('handles null/undefined arguments gracefully', async () => {
      const response1 = (await uiTools.callTool(
        'wait_for_user_resume',
        null,
      )) as TestMCPResponse;
      expect(response1.result.content[0].type).toBe('text');

      const response2 = (await uiTools.callTool(
        'resume_from_wait',
        undefined,
      )) as TestMCPResponse;
      expect(response2.result.content[0].type).toBe('text');
    });

    it('handles malformed context objects', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        context: 'not an object',
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain(
        'Context with reason, command, nextAction required',
      );
    });
  });

  describe('Integration with existing tools', () => {
    it('does not interfere with existing prompt_user tool', async () => {
      const response = (await uiTools.callTool('prompt_user', {
        prompt: 'Test prompt',
        type: 'text',
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(2);
      expect(response.result.content[0].type).toBe('text');
      expect(response.result.content[1].type).toBe('resource');

      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('📋 User prompt created');
    });

    it('does not interfere with existing visualize_data tool', async () => {
      const response = (await uiTools.callTool('visualize_data', {
        type: 'bar',
        data: [
          { label: 'A', value: 10 },
          { label: 'B', value: 20 },
        ],
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(2);
      expect(response.result.content[0].type).toBe('text');
      expect(response.result.content[1].type).toBe('resource');

      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('📊 BAR Chart');
    });
  });

  describe('Tool schema validation', () => {
    it('has correct wait_for_user_resume schema', () => {
      const waitTool = uiTools.tools.find((t) => t.name === 'wait_for_user_resume');
      expect(waitTool).toBeDefined();
      expect(waitTool!.inputSchema.required).toEqual(['message', 'context']);

      const contextSchema = waitTool!.inputSchema.properties?.context as JSONSchemaObject;
      expect(contextSchema?.required).toEqual(['reason', 'command', 'nextAction']);
    });

    it('has correct resume_from_wait schema', () => {
      const resumeTool = uiTools.tools.find((t) => t.name === 'resume_from_wait');
      expect(resumeTool).toBeDefined();
      expect(resumeTool!.inputSchema.required).toEqual(['context']);

      const contextSchema = resumeTool!.inputSchema.properties?.context as JSONSchemaObject;
      expect(contextSchema?.required).toEqual([
        'startedAt',
        'reason',
        'command',
        'nextAction',
      ]);
    });
  });
});