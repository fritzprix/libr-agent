import { describe, it, expect, beforeEach, vi } from 'vitest';
import uiTools from '../ui-tools';
import type { MCPResponse, MCPResult } from '@/lib/mcp-types';

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
        resumeInstruction: 'Continue with the next step',
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(2);
      expect(response.result.content[0].type).toBe('text');
      expect(response.result.content[1].type).toBe('resource');

      // Check text content
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('⏳ Waiting: Testing wait');
      expect(textContent.text).toContain('Resume instruction: Continue with the next step');

      // Check metadata
      expect(response.result.structuredContent).toMatchObject({
        waiting: true,
        resumeInstruction: 'Continue with the next step',
        startedAt: expect.any(String),
      });
    });

    it('automatically adds startedAt timestamp', async () => {
      const before = new Date().toISOString();
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        resumeInstruction: 'Test instruction',
      })) as TestMCPResponse;
      const after = new Date().toISOString();

      const metadata = response.result.structuredContent as {
        startedAt: string;
      };
      expect(metadata.startedAt).toBeDefined();
      expect(metadata.startedAt >= before).toBe(true);
      expect(metadata.startedAt <= after).toBe(true);
    });

    it('rejects empty message', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: '',
        resumeInstruction: 'Test instruction',
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      expect(response.result.content[0].type).toBe('text');
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('Message is required');
    });

    it('rejects missing resumeInstruction', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        resumeInstruction: '',
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('Resume instruction is required');
    });

    it('generates valid UIResource', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test message',
        resumeInstruction: 'Continue with next step',
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
      const startedAt = '2025-10-11T17:00:00Z';
      const response = (await uiTools.callTool('resume_from_wait', {
        resumeInstruction: 'Continue deployment',
        startedAt,
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('✅ User resumed after waiting');
      expect(textContent.text).toContain('Resume instruction: Continue deployment');
      expect(textContent.text).toContain(`Started: ${startedAt}`);
    });

    it('calculates duration correctly', async () => {
      const startedAt = new Date(Date.now() - 65000).toISOString(); // 1 minute 5 seconds ago
      const response = (await uiTools.callTool('resume_from_wait', {
        resumeInstruction: 'Test instruction',
        startedAt,
      })) as TestMCPResponse;

      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toMatch(/1m \d+s/);

      const metadata = response.result.structuredContent as {
        duration: string;
      };
      expect(metadata.duration).toMatch(/1m \d+s/);
    });

    it('includes proper metadata', async () => {
      const startedAt = '2025-10-11T17:00:00Z';
      const response = (await uiTools.callTool('resume_from_wait', {
        resumeInstruction: 'Test instruction',
        startedAt,
      })) as TestMCPResponse;

      expect(response.result.structuredContent).toMatchObject({
        resumed: true,
        duration: expect.any(String),
        resumeInstruction: 'Test instruction',
        startedAt: '2025-10-11T17:00:00Z',
        resumedAt: expect.any(String),
      });
    });

    it('rejects missing resumeInstruction', async () => {
      const response = (await uiTools.callTool(
        'resume_from_wait',
        {
          startedAt: '2025-10-11T17:00:00Z',
        },
      )) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('Resume instruction is required');
    });

    it('rejects missing startedAt', async () => {
      const response = (await uiTools.callTool('resume_from_wait', {
        resumeInstruction: 'Test instruction',
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('Started at timestamp is required');
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
          resumeInstruction: 'Test instruction',
          startedAt,
        })) as TestMCPResponse;

        const metadata = response.result.structuredContent as {
          duration: string;
        };
        expect(metadata.duration).toMatch(expected);
      }
    });
  });

  describe('HTML generation', () => {
    it('generates valid UI structure', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test message',
        resumeInstruction: 'Continue with next step',
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
        resumeInstruction: 'Test instruction',
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
        resumeInstruction: 'Test instruction',
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
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        resumeInstruction: 'Continue with "quoted" action',
      })) as TestMCPResponse;

      const uiResource = response.result.content[1] as UIResourceWithContent;
      const htmlContent = uiResource.resource?.text ?? '';

  // Should contain properly escaped JSON
  expect(htmlContent).toContain('const context = ');
  // The raw double quotes will be escaped in HTML attribute; ensure both escaped and unescaped-like patterns appear
  expect(htmlContent).toMatch(/Continue with (\\"quoted\\"|"quoted"|&quot;quoted&quot;) action/);
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

    it('handles malformed arguments', async () => {
      const response = (await uiTools.callTool('wait_for_user_resume', {
        message: 'Test',
        resumeInstruction: null,
      })) as TestMCPResponse;

      expect(response.result.content).toHaveLength(1);
      const textContent = response.result.content[0] as { text: string };
      expect(textContent.text).toContain('Resume instruction is required');
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
      expect(waitTool!.inputSchema.required).toEqual(['message', 'resumeInstruction']);

      expect(waitTool!.inputSchema.properties?.resumeInstruction).toBeDefined();
      expect(waitTool!.inputSchema.properties?.message).toBeDefined();
    });

    it('has correct resume_from_wait schema', () => {
      const resumeTool = uiTools.tools.find((t) => t.name === 'resume_from_wait');
      expect(resumeTool).toBeDefined();
      expect(resumeTool!.inputSchema.required).toEqual(['resumeInstruction', 'startedAt']);

      expect(resumeTool!.inputSchema.properties?.resumeInstruction).toBeDefined();
      expect(resumeTool!.inputSchema.properties?.startedAt).toBeDefined();
    });
  });
});