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

  it('does not corrupt LaTeX math with underscores or multiplication asterisks', () => {
    const text =
      '$N_{\\text{seq}}$): $2,048\\text{ tokens}$ * 출력 생성 ($N_{\\text{out}}$';
    expect(preprocessStreamMarkdown(text, true)).toBe(text);
  });

  it('does not treat isolated multiplication asterisks as emphasis', () => {
    const text = 'A * B = C';
    expect(preprocessStreamMarkdown(text, true)).toBe(text);
  });

  it('does not treat intra-word underscores as emphasis', () => {
    const text = 'my_variable_name is awesome';
    expect(preprocessStreamMarkdown(text, true)).toBe(text);
  });

  it('closes incomplete inline math expression during streaming', () => {
    const text = '계산 중: $N_{\\text{seq';
    expect(preprocessStreamMarkdown(text, true)).toBe('계산 중: $N_{\\text{seq$');
  });

  it('closes incomplete display math block during streaming', () => {
    const text = '수식:\n$$E = mc^2';
    expect(preprocessStreamMarkdown(text, true)).toBe('수식:\n$$E = mc^2$$');
  });

  it('does not close plain currency values as math', () => {
    const text = '가격은 $100 이고 수수료는 $20 입니다.';
    expect(preprocessStreamMarkdown(text, true)).toBe(text);
  });
});
