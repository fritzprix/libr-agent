import { describe, expect, it } from 'vitest';
import { preprocessStreamMarkdown } from '../streamMarkdownPreprocess';

describe('preprocessStreamMarkdown', () => {
  it('returns content unchanged when not streaming', () => {
    const content = '```ts\nconst x = 1';
    expect(preprocessStreamMarkdown(content, false)).toBe(content);
  });

  it('closes an unclosed fenced code block while streaming', () => {
    const content = '```ts\nconst x = 1;';
    expect(preprocessStreamMarkdown(content, true)).toBe(
      '```ts\nconst x = 1;\n```',
    );
  });

  it('does not double-close a complete fence', () => {
    const content = '```ts\nconst x = 1;\n```';
    expect(preprocessStreamMarkdown(content, true)).toBe(content);
  });

  it('closes unclosed bold markers', () => {
    expect(preprocessStreamMarkdown('hello **world', true)).toBe(
      'hello **world**',
    );
  });

  it('closes italic after completed bold without double-counting', () => {
    expect(preprocessStreamMarkdown('**bold** and *italic', true)).toBe(
      '**bold** and *italic*',
    );
  });

  it('strips an incomplete trailing HTML tag', () => {
    expect(preprocessStreamMarkdown('hello <div class="x', true)).toBe(
      'hello ',
    );
  });

  it('closes unpaired inline code outside fences', () => {
    expect(preprocessStreamMarkdown('use `code', true)).toBe('use `code`');
  });
});
