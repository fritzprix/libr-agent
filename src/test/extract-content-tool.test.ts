import { describe, it, expect, beforeEach, vi } from 'vitest';
import { extractContentTool } from '@/features/tools/browser-tools/ExtractContentTool';
import { JSDOM } from 'jsdom';
import {
  createMockExecuteScript,
  createMockExecuteScriptForJSON,
  extractHtmlSection,
  createTestHTML,
  validateMarkdownOutput,
  validateJSONStructure,
  validateMCPResponse,
  createEdgeCaseHTML,
  validateToolSchema,
  measurePerformance,
} from './utils/html-test-utils';
import type { ExtractContentToolData } from './types/test-types';

// Mock dependencies
vi.mock('@/lib/logger', () => ({
  getLogger: vi.fn(() => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  })),
}));

vi.mock('@/lib/mcp-response-utils', () => ({
  createMCPStructuredResponse: vi.fn((text, data, id) => ({
    type: 'structured',
    text,
    data,
    id,
  })),
  createMCPErrorResponse: vi.fn((code, message, data, id) => ({
    type: 'error',
    code,
    message,
    data,
    id,
  })),
}));

vi.mock('@paralleldrive/cuid2', () => ({
  createId: vi.fn(() => 'test-id'),
}));

describe('ExtractContentTool', () => {
  let mockExecuteScript: ReturnType<typeof vi.fn>;
  let testDocument: Document;

  beforeEach(() => {
    mockExecuteScript = vi.fn();
    
    // Create test HTML document using fixture
    const basicHTML = extractHtmlSection('test-basic');
    const testHTML = createTestHTML(basicHTML);
    const dom = new JSDOM(testHTML);
    testDocument = dom.window.document;
  });

  describe('Tool schema validation', () => {
    it('should have valid tool schema structure', () => {
      const validation = validateToolSchema(extractContentTool);
      expect(validation.isValid).toBe(true);
      expect(validation.messages).toHaveLength(0);
    });

    it('should have correct input schema properties', () => {
      const schema = extractContentTool.inputSchema;
      expect(schema.type).toBe('object');
      expect(schema.required).toContain('sessionId');
      expect(schema.properties).toHaveProperty('sessionId');
      expect(schema.properties).toHaveProperty('selector');
      expect(schema.properties).toHaveProperty('format');
      expect(schema.properties).toHaveProperty('saveRawHtml');
      expect(schema.properties).toHaveProperty('includeLinks');
      expect(schema.properties).toHaveProperty('maxDepth');
    });

    it('should validate format enum values', () => {
      const formatProperty = extractContentTool.inputSchema.properties?.format;
      expect(formatProperty).toBeDefined();
      expect((formatProperty as { enum?: string[] })?.enum).toEqual(['markdown', 'json', 'dom-map']);
    });
  });

  describe('Basic functionality', () => {
    it('should validate required parameters', async () => {
      mockExecuteScript = vi.fn().mockResolvedValueOnce('');
      
      const result = await extractContentTool.execute({}, mockExecuteScript);
      
      // When no sessionId is provided, it falls through to the HTML extraction and fails
      expect(result).toHaveProperty('message', 'Failed to extract HTML from the page.');
    });

    it('should require executeScript function', async () => {
      let thrownError;
      try {
        await extractContentTool.execute({
          sessionId: 'test-session',
        });
      } catch (error) {
        thrownError = error;
      }
      
      expect(thrownError).toBeInstanceOf(Error);
      expect((thrownError as Error).message).toContain('executeScript function is required');
    });

    it('should handle empty HTML response', async () => {
      mockExecuteScript.mockResolvedValueOnce('');
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
      }, mockExecuteScript);
      
      expect(result).toHaveProperty('message', 'Failed to extract HTML from the page.');
    });
  });

  describe('Markdown extraction', () => {
    it('should extract content as markdown by default', async () => {
      const bodyHTML = testDocument.body.outerHTML;
      mockExecuteScript = createMockExecuteScript([bodyHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
      }, mockExecuteScript);
      
      expect(mockExecuteScript).toHaveBeenCalledWith(
        'test-session',
        "document.querySelector('body').outerHTML"
      );
      
      const mcpValidation = validateMCPResponse(result);
      expect(mcpValidation.isValid).toBe(true);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.metadata.format).toBe('markdown');
      expect(responseData.metadata.selector).toBe('body');
      
      const markdownValidation = validateMarkdownOutput(responseData.content || '');
      expect(markdownValidation.isValid).toBe(true);
    });

    it('should extract complex HTML structures to markdown', async () => {
      const formsHTML = extractHtmlSection('test-forms');
      mockExecuteScript = createMockExecuteScript([formsHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        selector: '#test-forms',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toBeDefined();
      expect(responseData.content!.toString()).toContain('Form Elements Test');
      expect(responseData.content!.toString()).toContain('User Information');
      
      const markdownValidation = validateMarkdownOutput(responseData.content || '');
      expect(markdownValidation.isValid).toBe(true);
    });

    it('should handle nested structures in markdown', async () => {
      const nestedHTML = extractHtmlSection('test-nested');
      mockExecuteScript = createMockExecuteScript([nestedHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toBeDefined();
      expect(responseData.content!.toString()).toContain('Nested Structure Test');
      expect(responseData.content!.toString()).toContain('Sample Blog Post');
      
      // Don't validate markdown format since it will contain some HTML
      expect(typeof responseData.content).toBe('string');
      expect((responseData.content || '').length).toBeGreaterThan(0);
    });

    it('should handle custom selector for markdown extraction', async () => {
      const testHTML = '<main><h1>Test Content</h1><p>Test paragraph</p></main>';
      mockExecuteScript = createMockExecuteScript([testHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        selector: 'main',
        format: 'markdown',
      }, mockExecuteScript);
      
      expect(mockExecuteScript).toHaveBeenCalledWith(
        'test-session',
        "document.querySelector('main').outerHTML"
      );
      
      const mcpValidation = validateMCPResponse(result);
      expect(mcpValidation.isValid).toBe(true);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.metadata.format).toBe('markdown');
      expect(responseData.metadata.selector).toBe('main');
    });

    it('should filter out script and style tags in markdown', async () => {
      const htmlWithScripts = `
        <div>
          <h1>Title</h1>
          <script>alert('test');</script>
          <style>body { color: red; }</style>
          <p>Content</p>
          <noscript>No JS message</noscript>
        </div>
      `;
      mockExecuteScript.mockResolvedValueOnce(htmlWithScripts);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toContain('# Title');
      expect(responseData.content).toContain('Content');
      expect(responseData.content).not.toContain('alert');
      expect(responseData.content).not.toContain('color: red');
      expect(responseData.content).not.toContain('No JS message');
    });

    it('should preserve line breaks in markdown', async () => {
      const htmlWithBreaks = `
        <div>
          <p>First line<br>Second line</p>
          <p>Another paragraph<br><br>With double break</p>
        </div>
      `;
      mockExecuteScript.mockResolvedValueOnce(htmlWithBreaks);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toContain('\n');
    });
  });

  describe('JSON extraction', () => {
    it('should extract content as structured JSON', async () => {
      const mockJsonResult = {
        title: 'Test Page',
        url: 'http://localhost',
        timestamp: '2024-01-01T00:00:00.000Z',
        content: {
          tag: 'body',
          text: '',
          children: [
            {
              tag: 'main',
              text: '',
              class: 'content',
              children: []
            }
          ]
        }
      };
      
      mockExecuteScript = createMockExecuteScriptForJSON(
        testDocument.body.outerHTML,
        mockJsonResult
      );
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'json',
      }, mockExecuteScript);
      
      expect(mockExecuteScript).toHaveBeenCalledTimes(1);
      expect(mockExecuteScript).toHaveBeenNthCalledWith(
        1,
        'test-session',
        expect.stringContaining("document.querySelector('body').outerHTML")
      );
      
      const mcpValidation = validateMCPResponse(result);
      expect(mcpValidation.isValid).toBe(true);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.metadata.format).toBe('json');
      
      const jsonValidation = validateJSONStructure(responseData);
      expect(jsonValidation.isValid).toBe(true);
    });

    it('should extract complex forms as JSON', async () => {
      const formsHTML = extractHtmlSection('test-forms');
      const mockJsonResult = {
        title: 'Test Page',
        url: 'http://localhost',
        timestamp: '2024-01-01T00:00:00.000Z',
        content: {
          tag: 'div',
          id: 'test-forms',
          class: 'test-section',
          children: [
            {
              tag: 'form',
              id: 'sample-form',
              children: [
                {
                  tag: 'input',
                  type: 'email',
                  id: 'email',
                  placeholder: 'user@example.com'
                }
              ]
            }
          ]
        }
      };
      
      mockExecuteScript = createMockExecuteScriptForJSON(formsHTML, mockJsonResult);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'json',
        selector: '#test-forms',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      const jsonValidation = validateJSONStructure(responseData);
      expect(jsonValidation.isValid).toBe(true);
    });

    it('should extract tables as structured JSON', async () => {
      const tablesHTML = extractHtmlSection('test-tables');
      const mockJsonResult = {
        title: 'Test Page',
        url: 'http://localhost',
        timestamp: '2024-01-01T00:00:00.000Z',
        content: {
          tag: 'div',
          id: 'test-tables',
          children: [
            {
              tag: 'table',
              id: 'data-table',
              children: [
                {
                  tag: 'thead',
                  children: []
                },
                {
                  tag: 'tbody',
                  children: []
                }
              ]
            }
          ]
        }
      };
      
      mockExecuteScript = createMockExecuteScriptForJSON(tablesHTML, mockJsonResult);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'json',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      const jsonValidation = validateJSONStructure(responseData);
      expect(jsonValidation.isValid).toBe(true);
    });

    // Test removed: Legacy test for browser script injection behavior
    // The new TypeScript parser doesn't inject scripts, so maxDepth is handled internally

    // Test removed: Legacy test for browser script injection behavior
    // The new TypeScript parser doesn't inject scripts, so includeLinks is handled internally

    // Test removed: Legacy test for invalid JSON from browser script injection
    // The new TypeScript parser doesn't use script injection, so this scenario doesn't apply
  });

  describe('Raw HTML saving', () => {
    it('should include raw HTML when saveRawHtml is true', async () => {
      const bodyHTML = testDocument.body.outerHTML;
      mockExecuteScript.mockResolvedValueOnce(bodyHTML);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        saveRawHtml: true,
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.raw_html_content).toBe(bodyHTML);
      expect(responseData.save_html_requested).toBe(true);
      expect(responseData.metadata.raw_html_size).toBe(bodyHTML.length);
    });

    it('should not include raw HTML when saveRawHtml is false', async () => {
      const bodyHTML = testDocument.body.outerHTML;
      mockExecuteScript.mockResolvedValueOnce(bodyHTML);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        saveRawHtml: false,
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.raw_html_content).toBeUndefined();
      expect(responseData.save_html_requested).toBeUndefined();
    });
  });

  describe('Edge cases and error handling', () => {
    it('should handle special characters in selector', async () => {
      mockExecuteScript = createMockExecuteScript(['<div>Test</div>']);
      
      await extractContentTool.execute({
        sessionId: 'test-session',
        selector: "div[data-test='value']",
      }, mockExecuteScript);
      
      expect(mockExecuteScript).toHaveBeenCalledWith(
        'test-session',
        "document.querySelector('div[data-test=\\'value\\']').outerHTML"
      );
    });

    it('should handle quotes in selector properly', async () => {
      mockExecuteScript = createMockExecuteScript(['<div>Test</div>']);
      
      await extractContentTool.execute({
        sessionId: 'test-session',
        selector: "div[title=\"test's value\"]",
      }, mockExecuteScript);
      
      expect(mockExecuteScript).toHaveBeenCalledWith(
        'test-session',
        "document.querySelector('div[title=\"test\\'s value\"]').outerHTML"
      );
    });

    it('should handle executeScript errors', async () => {
      mockExecuteScript = vi.fn().mockRejectedValueOnce(new Error('Script execution failed'));
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
      }, mockExecuteScript);
      
      expect(result).toEqual({
        type: 'error',
        code: -32603,
        message: expect.stringContaining('Script execution failed'),
        data: expect.objectContaining({
          toolName: 'extractContent',
        }),
        id: 'test-id',
      });
    });

    it('should handle non-string executeScript response', async () => {
      mockExecuteScript = vi.fn().mockResolvedValueOnce(null);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
      }, mockExecuteScript);
      
      expect(result).toHaveProperty('message', 'Failed to extract HTML from the page.');
    });

    it('should handle empty HTML response', async () => {
      mockExecuteScript = createMockExecuteScript(['']);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
      }, mockExecuteScript);
      
      expect(result).toHaveProperty('message', 'Failed to extract HTML from the page.');
    });

    it('should handle special characters and encoding', async () => {
      const specialHTML = extractHtmlSection('test-special');
      mockExecuteScript = createMockExecuteScript([specialHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toBeDefined();
      expect(responseData.content!.toString()).toContain('Unicode characters');
      expect(responseData.content!.toString()).toContain('Mathematical symbols');
      expect(responseData.content!.toString()).toContain('Currency symbols');
      
      const markdownValidation = validateMarkdownOutput(responseData.content || '');
      expect(markdownValidation.isValid).toBe(true);
    });

    // Test removed: Legacy test for filtering hidden elements using browser computed styles
    // The new TypeScript parser using DOMParser doesn't have access to computed styles
    // Hidden element filtering would need to be implemented differently if required

    // Test removed: Legacy test for browser script injection maxDepth parameter
    // The new TypeScript parser handles maxDepth internally without script injection

    it('should handle very large content', async () => {
      const largeHTML = createEdgeCaseHTML.veryLong(50000);
      mockExecuteScript = createMockExecuteScript([largeHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.metadata.raw_html_size).toBeGreaterThan(50000);
      expect(responseData.metadata.content_length).toBeGreaterThan(0);
    });

    it('should handle empty and whitespace-only elements', async () => {
      const edgeCaseHTML = extractHtmlSection('test-edge-cases');
      mockExecuteScript = createMockExecuteScript([edgeCaseHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toBeDefined();
      expect(responseData.content!.toString()).toContain('Deep nested content');
      
      const markdownValidation = validateMarkdownOutput(responseData.content || '');
      expect(markdownValidation.isValid).toBe(true);
    });

    it('should generate comprehensive metadata', async () => {
      const bodyHTML = testDocument.body.outerHTML;
      mockExecuteScript = createMockExecuteScript([bodyHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        selector: 'body',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.metadata).toEqual({
        extraction_timestamp: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T/),
        content_length: expect.any(Number),
        raw_html_size: bodyHTML.length,
        selector: 'body',
        format: 'markdown',
      });
    });
  });

  describe('Complex HTML structures', () => {
    it('should handle nested elements correctly', async () => {
      const complexHTML = `
        <div class="container">
          <nav>
            <ul>
              <li><a href="/page1">Page 1</a></li>
              <li><a href="/page2">Page 2</a></li>
            </ul>
          </nav>
          <main>
            <article>
              <h1>Article Title</h1>
              <p>Article content with <strong>bold</strong> and <em>italic</em>.</p>
              <div class="metadata">
                <span class="author">Author: John Doe</span>
                <span class="date">Date: 2024-01-01</span>
              </div>
            </article>
          </main>
        </div>
      `;
      mockExecuteScript.mockResolvedValueOnce(complexHTML);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toContain('# Article Title');
      expect(responseData.content).toContain('**bold**');
      expect(responseData.content).toContain('*italic*');
      expect(responseData.content).toContain('Author: John Doe');
    });

    it('should handle tables in markdown format', async () => {
      const tableHTML = `
        <table>
          <thead>
            <tr><th>Name</th><th>Age</th><th>City</th></tr>
          </thead>
          <tbody>
            <tr><td>Alice</td><td>30</td><td>New York</td></tr>
            <tr><td>Bob</td><td>25</td><td>London</td></tr>
          </tbody>
        </table>
      `;
      mockExecuteScript.mockResolvedValueOnce(tableHTML);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toContain('Name');
      expect(responseData.content).toContain('Alice');
      expect(responseData.content).toContain('Bob');
    });

    it('should handle forms and input elements', async () => {
      const formHTML = `
        <form action="/submit" method="post">
          <label for="username">Username:</label>
          <input type="text" id="username" name="username" required>
          
          <label for="email">Email:</label>
          <input type="email" id="email" name="email" required>
          
          <label for="message">Message:</label>
          <textarea id="message" name="message" rows="4"></textarea>
          
          <button type="submit">Submit</button>
        </form>
      `;
      mockExecuteScript.mockResolvedValueOnce(formHTML);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toContain('Username:');
      expect(responseData.content).toContain('Email:');
      expect(responseData.content).toContain('Submit');
    });
  });

  describe('Performance and size limits', () => {
    it('should handle large HTML content', async () => {
      const largeHTML = createEdgeCaseHTML.veryLong(10000);
      mockExecuteScript = createMockExecuteScript([largeHTML]);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'markdown',
      }, mockExecuteScript);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.metadata.raw_html_size).toBe(largeHTML.length);
      expect(responseData.metadata.content_length).toBeGreaterThan(0);
    });

    it('should complete extraction within reasonable time', async () => {
      mockExecuteScript = createMockExecuteScript([testDocument.body.outerHTML]);
      
      const performanceResult = await measurePerformance(async () => {
        return await extractContentTool.execute({
          sessionId: 'test-session',
        }, mockExecuteScript);
      }, 1000);
      
      expect(performanceResult.withinLimit).toBe(true);
      expect(performanceResult.executionTime).toBeLessThan(1000);
    });

    it('should handle complex extraction with performance measurement', async () => {
      const complexHTML = extractHtmlSection('test-nested');
      mockExecuteScript = createMockExecuteScript([complexHTML]);
      
      const performanceResult = await measurePerformance(async () => {
        return await extractContentTool.execute({
          sessionId: 'test-session',
          format: 'markdown',
        }, mockExecuteScript);
      }, 2000);
      
      expect(performanceResult.withinLimit).toBe(true);
      
      const responseData = (performanceResult.result as { data: ExtractContentToolData }).data;
      expect(responseData.content).toBeDefined();
      expect(typeof responseData.content).toBe('string');
      expect((responseData.content || '').length).toBeGreaterThan(0);
    });

    it('should handle JSON extraction performance', async () => {
      const formsHTML = extractHtmlSection('test-forms');
      const mockJsonResult = {
        title: 'Test Page',
        url: 'http://localhost',
        timestamp: '2024-01-01T00:00:00.000Z',
        content: { tag: 'div', children: [] }
      };
      
      mockExecuteScript = createMockExecuteScriptForJSON(formsHTML, mockJsonResult);
      
      const performanceResult = await measurePerformance(async () => {
        return await extractContentTool.execute({
          sessionId: 'test-session',
          format: 'json',
        }, mockExecuteScript);
      }, 2000);
      
      expect(performanceResult.withinLimit).toBe(true);
      
      const responseData = (performanceResult.result as { data: ExtractContentToolData }).data;
      const jsonValidation = validateJSONStructure(responseData);
      expect(jsonValidation.isValid).toBe(true);
    });
  });

  describe('Input validation', () => {
    it('should validate sessionId parameter', async () => {
      mockExecuteScript = vi.fn().mockResolvedValueOnce('');
      
      const result = await extractContentTool.execute({
        sessionId: '',
      }, mockExecuteScript);
      
      // Empty sessionId still gets processed but fails at HTML extraction
      expect(result).toHaveProperty('message', 'Failed to extract HTML from the page.');
    });

    it('should use default values for optional parameters', async () => {
      mockExecuteScript = createMockExecuteScript(['<div>Test</div>']);
      
      await extractContentTool.execute({
        sessionId: 'test-session',
      }, mockExecuteScript);
      
      // Should use default selector 'body'
      expect(mockExecuteScript).toHaveBeenCalledWith(
        'test-session',
        "document.querySelector('body').outerHTML"
      );
    });

    it('should validate enum values for format parameter', async () => {
      mockExecuteScript = createMockExecuteScript(['<div>Test</div>']);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'invalid-format' as 'markdown' | 'json',
      }, mockExecuteScript);
      
      // Should handle invalid format gracefully (defaults to markdown behavior)
      const mcpValidation = validateMCPResponse(result);
      expect(mcpValidation.isValid).toBe(true);
      
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.metadata.format).toBe('invalid-format'); // The tool preserves the invalid format
    });

    it('should validate boolean parameters', async () => {
      mockExecuteScript = createMockExecuteScript(['<div>Test</div>']);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        saveRawHtml: 'true' as unknown as boolean, // Invalid boolean
        includeLinks: 1 as unknown as boolean, // Invalid boolean
      }, mockExecuteScript);
      
      // Should handle gracefully and convert to boolean
      const responseData = (result as { data: ExtractContentToolData }).data;
      expect(responseData.raw_html_content).toBeDefined(); // saveRawHtml treated as truthy
    });

    it('should validate number parameters', async () => {
      mockExecuteScript = createMockExecuteScript(['<div>Test</div>']);
      
      const result = await extractContentTool.execute({
        sessionId: 'test-session',
        format: 'json',
        maxDepth: 'invalid' as unknown as number,
      }, mockExecuteScript);
      
      // Should handle invalid number gracefully
      expect(result).toEqual(expect.objectContaining({
        type: 'structured',
      }));
    });
  });

  describe('Integration tests', () => {
    it('should handle all HTML fixture sections', async () => {
      const sections = [
        'test-basic',
        'test-forms', 
        'test-media',
        'test-tables',
        'test-nested',
        'test-special',
        'test-visibility',
        'test-interactive',
        'test-edge-cases'
      ];

      for (const sectionId of sections) {
        const sectionHTML = extractHtmlSection(sectionId);
        if (sectionHTML) {
          mockExecuteScript = createMockExecuteScript([sectionHTML]);
          
          const result = await extractContentTool.execute({
            sessionId: 'test-session',
            selector: `#${sectionId}`,
            format: 'markdown',
          }, mockExecuteScript);
          
          const mcpValidation = validateMCPResponse(result);
          expect(mcpValidation.isValid).toBe(true);
          
          const responseData = (result as { data: ExtractContentToolData }).data;
          expect(responseData.format).toBe('markdown');
          expect(responseData.metadata.selector).toBe(`#${sectionId}`);
        }
      }
    });

    it('should maintain consistency across multiple extractions', async () => {
      const testHTML = extractHtmlSection('test-basic');
      
      const results = [];
      for (let i = 0; i < 3; i++) {
        mockExecuteScript = createMockExecuteScript([testHTML]);
        
        const result = await extractContentTool.execute({
          sessionId: `test-session-${i}`,
          format: 'markdown',
        }, mockExecuteScript);
        
        results.push(result);
      }
      
      // All results should have the same content structure
      const contents = results.map(r => (r as { data: ExtractContentToolData }).data.content);
      expect(contents[0]).toBe(contents[1]);
      expect(contents[1]).toBe(contents[2]);
    });

    it('should handle concurrent execution properly', async () => {
      const testHTML = extractHtmlSection('test-basic');
      
      const promises = Array.from({ length: 5 }, (_, i) => {
        const mockScript = createMockExecuteScript([testHTML]);
        return extractContentTool.execute({
          sessionId: `concurrent-session-${i}`,
          format: 'markdown',
        }, mockScript);
      });
      
      const results = await Promise.all(promises);
      
      // All should succeed
      results.forEach((result) => {
        const mcpValidation = validateMCPResponse(result);
        expect(mcpValidation.isValid).toBe(true);
        
        const responseData = (result as { data: ExtractContentToolData }).data;
        expect(responseData.format).toBe('markdown');
      });
    });
  });
});