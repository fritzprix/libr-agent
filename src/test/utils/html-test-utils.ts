import { vi } from 'vitest';
import { readFileSync } from 'fs';
import { join } from 'path';
import { JSDOM } from 'jsdom';

/**
 * Creates a mock executeScript function that returns predefined responses
 * @param htmlResponses Array of HTML responses to return for each call
 * @returns Mocked executeScript function
 */
export const createMockExecuteScript = (htmlResponses: string[]) => {
  const mock = vi.fn();
  htmlResponses.forEach((response) => {
    mock.mockResolvedValueOnce(response);
  });
  return mock;
};

/**
 * Creates a mock executeScript function for JSON extraction
 * @param htmlResponse The HTML response for the first call
 * @param jsonResponse The JSON response for the second call
 * @returns Mocked executeScript function
 */
export const createMockExecuteScriptForJSON = (
  htmlResponse: string,
  jsonResponse: object | string,
) => {
  const mock = vi.fn();
  mock.mockResolvedValueOnce(htmlResponse);
  mock.mockResolvedValueOnce(
    typeof jsonResponse === 'string'
      ? jsonResponse
      : JSON.stringify(jsonResponse),
  );
  return mock;
};

/**
 * Loads HTML content from test fixtures
 * @param filename The fixture file name
 * @returns HTML content as string
 */
export const loadHtmlFixture = (filename: string): string => {
  const fixturePath = join(__dirname, '..', 'fixtures', filename);
  return readFileSync(fixturePath, 'utf-8');
};

/**
 * Extracts a specific section from the test HTML fixture
 * @param sectionId The ID of the section to extract
 * @returns HTML content of the specified section
 */
export const extractHtmlSection = (sectionId: string): string => {
  const html = loadHtmlFixture('extract-content-test.html');
  const dom = new JSDOM(html);
  const section = dom.window.document.querySelector(`#${sectionId}`);
  return section?.outerHTML || '';
};

/**
 * Creates a minimal HTML document for testing
 * @param content The content to include in the body
 * @param title Optional page title
 * @returns Complete HTML document as string
 */
export const createTestHTML = (
  content: string,
  title = 'Test Page',
): string => {
  return `
    <!DOCTYPE html>
    <html>
      <head>
        <title>${title}</title>
        <script>console.log('test');</script>
        <style>body { margin: 0; }</style>
      </head>
      <body>
        ${content}
      </body>
    </html>
  `;
};

/**
 * Validates that content is properly formatted as Markdown
 * @param content The content to validate
 * @returns Validation result with boolean and messages
 */
export const validateMarkdownOutput = (content: string) => {
  const validation = {
    isValid: true,
    messages: [] as string[],
  };

  if (!content || typeof content !== 'string') {
    validation.isValid = false;
    validation.messages.push('Content must be a non-empty string');
    return validation;
  }

  // Check if content contains unwanted HTML tags (be more lenient)
  const dangerousHtmlRegex =
    /<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/i;
  if (dangerousHtmlRegex.test(content)) {
    validation.isValid = false;
    validation.messages.push(
      'Content contains dangerous HTML tags that should be filtered',
    );
  }

  // Check if content contains unwanted script content
  if (
    content.includes('console.log') ||
    content.includes('alert(') ||
    content.includes('<script')
  ) {
    validation.isValid = false;
    validation.messages.push(
      'Content contains script code that should be filtered',
    );
  }

  return validation;
};

/**
 * Validates JSON structure from ExtractContentTool
 * @param data The JSON data to validate
 * @returns Validation result with boolean and messages
 */
export const validateJSONStructure = (data: unknown) => {
  const validation = {
    isValid: true,
    messages: [] as string[],
  };

  if (!data || typeof data !== 'object') {
    validation.isValid = false;
    validation.messages.push('Data must be an object');
    return validation;
  }

  // For ExtractContentTool, the JSON data might be directly in the content or might be the parsed JSON response
  // Let's be more flexible about the structure

  // Check if it's a parsed JSON response with title, url, timestamp, content
  if (
    'title' in data &&
    'url' in data &&
    'timestamp' in data &&
    'content' in data
  ) {
    // Validate content structure
    if (data.content && typeof data.content === 'object') {
      const contentProps = ['tag'];
      for (const prop of contentProps) {
        if (!(prop in data.content)) {
          validation.isValid = false;
          validation.messages.push(`Missing content property: ${prop}`);
        }
      }

      // Check children array if present
      if (
        'children' in data.content &&
        data.content.children &&
        !Array.isArray(data.content.children)
      ) {
        validation.isValid = false;
        validation.messages.push('Content children should be an array');
      }
    }

    // Validate timestamp format if present
    if (data.timestamp && typeof data.timestamp === 'string') {
      const timestampRegex =
        /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z?$/;
      if (!timestampRegex.test(data.timestamp)) {
        validation.isValid = false;
        validation.messages.push('Invalid timestamp format');
      }
    }
  }

  return validation;
};

/**
 * Validates MCP response structure
 * @param response The MCP response to validate
 * @returns Validation result with boolean and messages
 */
export const validateMCPResponse = (response: unknown) => {
  const validation = {
    isValid: true,
    messages: [] as string[],
  };

  if (!response || typeof response !== 'object') {
    validation.isValid = false;
    validation.messages.push('Response must be an object');
    return validation;
  }

  // Check response type
  if (
    !('type' in response) ||
    !['structured', 'error'].includes(response.type as string)
  ) {
    validation.isValid = false;
    validation.messages.push('Invalid or missing response type');
  }

  // Validate structured response
  if ('type' in response && response.type === 'structured') {
    // Text can be undefined for some responses
    if (
      'text' in response &&
      response.text !== undefined &&
      typeof response.text !== 'string'
    ) {
      validation.isValid = false;
      validation.messages.push(
        'Structured response text must be string when present',
      );
    }
    if (
      !('data' in response) ||
      !response.data ||
      typeof response.data !== 'object'
    ) {
      validation.isValid = false;
      validation.messages.push('Structured response must have data property');
    }
  }

  // Validate error response
  if ('type' in response && response.type === 'error') {
    if (!('code' in response) || typeof response.code !== 'number') {
      validation.isValid = false;
      validation.messages.push('Error response must have numeric code');
    }
    if (!('message' in response) || typeof response.message !== 'string') {
      validation.isValid = false;
      validation.messages.push('Error response must have message string');
    }
  }

  // Check for ID
  if (!('id' in response) || !response.id) {
    validation.isValid = false;
    validation.messages.push('Response must have an id');
  }

  return validation;
};

/**
 * Counts the number of specific elements in HTML content
 * @param html HTML content to analyze
 * @param selector CSS selector to count
 * @returns Number of matching elements
 */
export const countElementsInHTML = (html: string, selector: string): number => {
  const dom = new JSDOM(html);
  return dom.window.document.querySelectorAll(selector).length;
};

/**
 * Extracts all text content from HTML, similar to textContent
 * @param html HTML content
 * @returns Plain text content
 */
export const extractTextFromHTML = (html: string): string => {
  const dom = new JSDOM(html);
  return dom.window.document.body?.textContent || '';
};

/**
 * Creates a performance measurement wrapper for test functions
 * @param testFn The test function to measure
 * @param maxTimeMs Maximum allowed execution time in milliseconds
 * @returns Object with result and performance data
 */
export const measurePerformance = async <T>(
  testFn: () => Promise<T>,
  maxTimeMs = 5000,
): Promise<{
  result: T;
  executionTime: number;
  withinLimit: boolean;
}> => {
  const startTime = Date.now();
  const result = await testFn();
  const endTime = Date.now();
  const executionTime = endTime - startTime;

  return {
    result,
    executionTime,
    withinLimit: executionTime <= maxTimeMs,
  };
};

/**
 * Creates test data for edge cases
 */
export const createEdgeCaseHTML = {
  empty: () => '<div></div>',

  whitespaceOnly: () => '<div>   \n  \t  </div>',

  veryLong: (length = 10000) => `<div>${'x'.repeat(length)}</div>`,

  deeplyNested: (depth = 10) => {
    let html = '';
    for (let i = 0; i < depth; i++) {
      html += `<div class="level-${i}">`;
    }
    html += '<p>Deep content</p>';
    for (let i = 0; i < depth; i++) {
      html += '</div>';
    }
    return html;
  },

  specialCharacters: () => `
    <div>
      <p>Unicode: ñ, ö, ü, ß, æ, ø, å</p>
      <p>Math: ∑, ∆, π, ∞, ≤, ≥, ≠</p>
      <p>Currency: $, €, £, ¥, ₹, ₽</p>
      <p>Emojis: 😀, 🌟, 🚀, 💻, 📱, 🎯</p>
      <p>HTML entities: &amp;, &lt;, &gt;, &quot;, &#39;</p>
    </div>
  `,

  withScripts: () => `
    <div>
      <h1>Title</h1>
      <script>alert('This should be filtered');</script>
      <style>body { color: red; }</style>
      <p>Content</p>
      <noscript>No script content</noscript>
    </div>
  `,

  hiddenElements: () => `
    <div>
      <p>Visible content</p>
      <div style="display: none;">Hidden with display none</div>
      <div style="visibility: hidden;">Hidden with visibility</div>
      <p>More visible content</p>
    </div>
  `,

  complexForm: () => `
    <form>
      <input type="text" name="text" placeholder="Text input">
      <input type="email" name="email" placeholder="Email input">
      <input type="password" name="password" placeholder="Password">
      <select name="select">
        <option value="1">Option 1</option>
        <option value="2">Option 2</option>
      </select>
      <textarea name="textarea" placeholder="Textarea"></textarea>
      <button type="submit">Submit</button>
    </form>
  `,

  mediaElements: () => `
    <div>
      <img src="test.jpg" alt="Test image" title="Image title">
      <video src="test.mp4" controls></video>
      <audio src="test.mp3" controls></audio>
      <iframe src="https://example.com" title="Test iframe"></iframe>
    </div>
  `,

  table: () => `
    <table>
      <caption>Test Table</caption>
      <thead>
        <tr><th>Header 1</th><th>Header 2</th></tr>
      </thead>
      <tbody>
        <tr><td>Cell 1</td><td>Cell 2</td></tr>
        <tr><td>Cell 3</td><td>Cell 4</td></tr>
      </tbody>
      <tfoot>
        <tr><td colspan="2">Footer</td></tr>
      </tfoot>
    </table>
  `,
};

/**
 * Test helper to validate tool input schema
 * @param tool The tool object to validate
 * @returns Validation result
 */
export const validateToolSchema = (tool: unknown) => {
  const validation = {
    isValid: true,
    messages: [] as string[],
  };

  if (!tool || typeof tool !== 'object') {
    validation.isValid = false;
    validation.messages.push('Tool must be an object');
    return validation;
  }

  if (!('name' in tool) || typeof tool.name !== 'string') {
    validation.isValid = false;
    validation.messages.push('Tool must have a name');
  }

  if (!('description' in tool) || typeof tool.description !== 'string') {
    validation.isValid = false;
    validation.messages.push('Tool must have a description');
  }

  if (
    !('inputSchema' in tool) ||
    !tool.inputSchema ||
    typeof tool.inputSchema !== 'object'
  ) {
    validation.isValid = false;
    validation.messages.push('Tool must have an inputSchema');
  }

  if (!('execute' in tool) || typeof tool.execute !== 'function') {
    validation.isValid = false;
    validation.messages.push('Tool must have an execute function');
  }

  return validation;
};
