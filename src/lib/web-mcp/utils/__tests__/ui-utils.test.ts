import { describe, it, expect } from 'vitest';
import { renderTemplate, escapeHtml } from '../ui-utils';

describe('renderTemplate', () => {
  it('should replace simple string placeholders', () => {
    const template = 'Hello {{name}}!';
    const result = renderTemplate(template, { name: 'World' });
    expect(result).toBe('Hello World!');
  });

  it('should replace multiple placeholders', () => {
    const template = '{{greeting}} {{name}}, you are {{age}} years old.';
    const result = renderTemplate(template, {
      greeting: 'Hello',
      name: 'Alice',
      age: 30,
    });
    expect(result).toBe('Hello Alice, you are 30 years old.');
  });

  it('should handle number values', () => {
    const template = 'The answer is {{number}}';
    const result = renderTemplate(template, { number: 42 });
    expect(result).toBe('The answer is 42');
  });

  it('should handle boolean values', () => {
    const template = 'const isActive = {{flag}};';
    const result = renderTemplate(template, { flag: true });
    expect(result).toBe('const isActive = true;');
  });

  it('should handle JSON stringified values', () => {
    const template = 'const data = {{{json}}};';
    const data = { key: 'value', items: [1, 2, 3] };
    const result = renderTemplate(template, {
      json: JSON.stringify(data),
    });
    expect(result).toBe('const data = {"key":"value","items":[1,2,3]};');
  });

  it('should handle JavaScript string literals with quotes', () => {
    const template = "const messageId = '{{id}}';";
    const result = renderTemplate(template, { id: 'msg-123' });
    expect(result).toBe("const messageId = 'msg-123';");
  });

  it('should replace undefined/null with empty string', () => {
    const template = 'Value: {{missing}}';
    const result = renderTemplate(template, {});
    expect(result).toBe('Value: ');
  });

  it('should handle whitespace in placeholders', () => {
    const template = 'Hello {{ name }}!';
    const result = renderTemplate(template, { name: 'World' });
    expect(result).toBe('Hello World!');
  });

  it('should not replace placeholders without matching params', () => {
    const template = 'Hello {{name}}, {{greeting}}!';
    const result = renderTemplate(template, { name: 'Alice' });
    expect(result).toBe('Hello Alice, !');
  });

  describe('JavaScript code generation scenarios', () => {
    it('should generate valid JS for boolean without quotes', () => {
      const template = 'const isMultiselect = {{flag}};';
      const result = renderTemplate(template, { flag: true });
      expect(result).toBe('const isMultiselect = true;');
      // Verify it's valid JS by using eval (safe in test)
      expect(() => eval(result)).not.toThrow();
    });

    it('should generate valid JS for JSON array without quotes', () => {
      const template = 'const options = {{{array}}};';
      const result = renderTemplate(template, {
        array: JSON.stringify(['option1', 'option2', 'option3']),
      });
      expect(result).toBe(
        'const options = ["option1","option2","option3"];',
      );
      // Verify it's valid JS
      expect(() => eval(result)).not.toThrow();
    });

    it('should generate valid JS for JSON object without quotes', () => {
      const template = 'const context = {{{json}}};';
      const contextObj = { resumeInstruction: 'test', startedAt: '2024-01-01' };
      const result = renderTemplate(template, {
        json: JSON.stringify(contextObj),
      });
      expect(result).toBe(
        'const context = {"resumeInstruction":"test","startedAt":"2024-01-01"};',
      );
      // Verify it's valid JS
      expect(() => eval(result)).not.toThrow();
    });

    it('should generate valid JS for string with quotes', () => {
      const template = "const messageId = '{{id}}';";
      const result = renderTemplate(template, { id: 'ui-123-456' });
      expect(result).toBe("const messageId = 'ui-123-456';");
      // Verify it's valid JS
      expect(() => eval(result)).not.toThrow();
    });

    it('should automatically escape HTML for XSS protection', () => {
      const template = 'const message = "{{msg}}";';
      const result = renderTemplate(template, {
        msg: '<script>alert("xss")</script>',
      });
      // Handlebars automatically escapes HTML
      expect(result).toBe(
        'const message = "&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;";',
      );
    });
  });

  describe('Real-world template scenarios', () => {
    it('should handle wait.html context template', () => {
      const template = 'const context = {{{contextJson}}};';
      const context = {
        resumeInstruction: 'Continue with task',
        startedAt: '2024-01-01T00:00:00Z',
      };
      const result = renderTemplate(template, {
        contextJson: JSON.stringify(context),
      });
      expect(result).toContain('const context = {');
      expect(result).toContain('"resumeInstruction":"Continue with task"');
      expect(result).toContain('"startedAt":"2024-01-01T00:00:00Z"');
    });

    it('should handle select-prompt.html options template', () => {
      const template = `const messageId = '{{messageId}}';
const isMultiselect = {{multiselect}};
const options = {{{optionsJson}}};`;

      const result = renderTemplate(template, {
        messageId: 'ui-123',
        multiselect: true,
        optionsJson: JSON.stringify(['Option 1', 'Option 2', 'Option 3']),
      });

      expect(result).toContain("const messageId = 'ui-123';");
      expect(result).toContain('const isMultiselect = true;');
      expect(result).toContain(
        'const options = ["Option 1","Option 2","Option 3"];',
      );

      // Verify all lines are valid JS
      const lines = result.split('\n');
      lines.forEach((line) => {
        if (line.trim()) {
          expect(() => eval(line)).not.toThrow();
        }
      });
    });

    it('should handle text-prompt.html messageId template', () => {
      const template = "const messageId = '{{messageId}}';";
      const result = renderTemplate(template, {
        messageId: 'ui-1234567890-1',
      });
      expect(result).toBe("const messageId = 'ui-1234567890-1';");
      expect(() => eval(result)).not.toThrow();
    });
  });
});

describe('escapeHtml', () => {
  it('should escape HTML special characters', () => {
    expect(escapeHtml('<script>alert("xss")</script>')).toBe(
      '&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;',
    );
  });

  it('should escape ampersand', () => {
    expect(escapeHtml('Tom & Jerry')).toBe('Tom &amp; Jerry');
  });

  it('should escape single quotes', () => {
    expect(escapeHtml("It's a test")).toBe('It&#39;s a test');
  });

  it('should handle empty string', () => {
    expect(escapeHtml('')).toBe('');
  });

  it('should handle string with no special characters', () => {
    expect(escapeHtml('Hello World')).toBe('Hello World');
  });
});
