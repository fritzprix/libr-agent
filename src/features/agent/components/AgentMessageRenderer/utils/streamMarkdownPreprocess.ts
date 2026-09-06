/**
 * Close incomplete markdown constructs during streaming so ReactMarkdown
 * does not produce broken layouts (unclosed fences, dangling emphasis, etc.).
 * When streaming finishes, callers should pass the original content unchanged.
 */

const INCOMPLETE_HTML_TAG_RE = /<[A-Za-z][^>]*$/;
const CURRENCY_RE = /^\$\d+([.,]\d+)*(\s|$)/;

function shouldCloseInlineMath(segment: string): boolean {
  // If it has LaTeX commands or math operators/subscripts, it is math
  if (/[\\{}_^=]/.test(segment)) {
    return true;
  }
  // If it looks like plain currency "$100" or "$19.99 / mo", don't close
  if (CURRENCY_RE.test(segment)) {
    return false;
  }
  return true;
}

/**
 * Robust stream markdown preprocessor that parses the text stream
 * while respecting code fences, inline code, display/inline math ($ and $$),
 * and CommonMark emphasis flanking rules.
 */
export function preprocessStreamMarkdown(
  content: string,
  isStreaming: boolean,
): string {
  if (!isStreaming || content.length === 0) {
    return content;
  }

  const len = content.length;
  let i = 0;

  let inFence = false;
  let fenceToken = '';
  let inInlineCode = false;
  let inDisplayMath = false;
  let inInlineMath = false;
  let inlineMathStartIndex = -1;

  // Track open bold / italic markers
  let openBoldAsterisk = false;
  let openBoldUnderscore = false;
  let openItalicAsterisk = false;
  let openItalicUnderscore = false;

  while (i < len) {
    // Check for escape character
    if (content[i] === '\\') {
      i += 2; // skip escaped character
      continue;
    }

    // 1. Inside fenced code block
    if (inFence) {
      if (
        (i === 0 || content[i - 1] === '\n') &&
        content.startsWith(fenceToken, i)
      ) {
        inFence = false;
        i += fenceToken.length;
        continue;
      }
      i++;
      continue;
    }

    // Check for start of fenced code block at line start
    if (i === 0 || content[i - 1] === '\n') {
      if (content.startsWith('```', i)) {
        inFence = true;
        fenceToken = '```';
        i += 3;
        continue;
      }
      if (content.startsWith('~~~', i)) {
        inFence = true;
        fenceToken = '~~~';
        i += 3;
        continue;
      }
    }

    // 2. Inside inline code
    if (inInlineCode) {
      if (content[i] === '`') {
        inInlineCode = false;
      }
      i++;
      continue;
    }

    if (content[i] === '`') {
      inInlineCode = true;
      i++;
      continue;
    }

    // 3. Inside display math ($$)
    if (inDisplayMath) {
      if (content.startsWith('$$', i)) {
        inDisplayMath = false;
        i += 2;
        continue;
      }
      i++;
      continue;
    }

    // 4. Inside inline math ($)
    if (inInlineMath) {
      if (content[i] === '$') {
        const prevChar = content[i - 1];
        const prevIsSpace =
          prevChar === ' ' || prevChar === '\t' || prevChar === '\n';

        if (!prevIsSpace) {
          inInlineMath = false;
          inlineMathStartIndex = -1;
        }
      }
      i++;
      continue;
    }

    // Check start of display math $$
    if (content.startsWith('$$', i)) {
      inDisplayMath = true;
      i += 2;
      continue;
    }

    // Check start of inline math $
    // Opening $ must NOT be followed by whitespace or another $
    if (content[i] === '$') {
      const nextChar = i + 1 < len ? content[i + 1] : '';
      const nextIsSpace =
        nextChar === ' ' || nextChar === '\t' || nextChar === '\n';
      const nextIsDollar = nextChar === '$';

      if (!nextIsSpace && !nextIsDollar && nextChar !== '') {
        inInlineMath = true;
        inlineMathStartIndex = i;
        i++;
        continue;
      }
    }

    // 5. Check list items (bullet list at line start)
    if ((i === 0 || content[i - 1] === '\n') && content[i] === '*') {
      const nextChar = i + 1 < len ? content[i + 1] : '';
      if (nextChar === ' ' || nextChar === '\t') {
        i++;
        continue;
      }
    }

    // 6. Bold & Italic emphasis delimiters outside code & math
    // Check for **
    if (content.startsWith('**', i)) {
      openBoldAsterisk = !openBoldAsterisk;
      i += 2;
      continue;
    }

    // Check for __
    if (content.startsWith('__', i)) {
      openBoldUnderscore = !openBoldUnderscore;
      i += 2;
      continue;
    }

    // Check for single *
    if (content[i] === '*') {
      const prevChar = i > 0 ? content[i - 1] : ' ';
      const nextChar = i + 1 < len ? content[i + 1] : ' ';
      const prevIsSpace = /\s/.test(prevChar);
      const nextIsSpace = /\s/.test(nextChar);

      // If surrounded by whitespace (e.g. ` 2 * 3 ` or ` tokens * 출력 `), it is literal asterisk
      if (prevIsSpace && nextIsSpace) {
        i++;
        continue;
      }

      openItalicAsterisk = !openItalicAsterisk;
      i++;
      continue;
    }

    // Check for single _
    if (content[i] === '_') {
      const prevChar = i > 0 ? content[i - 1] : ' ';
      const nextChar = i + 1 < len ? content[i + 1] : ' ';
      const prevIsSpace = /\s/.test(prevChar);
      const nextIsSpace = /\s/.test(nextChar);
      const prevIsWord = /\w/.test(prevChar);
      const nextIsWord = /\w/.test(nextChar);

      // In GFM/CommonMark, intra-word underscores (e.g. foo_bar) do not trigger emphasis
      if (prevIsWord && nextIsWord) {
        i++;
        continue;
      }

      // If surrounded by whitespace, it is literal underscore
      if (prevIsSpace && nextIsSpace) {
        i++;
        continue;
      }

      openItalicUnderscore = !openItalicUnderscore;
      i++;
      continue;
    }

    i++;
  }

  let result = content;

  // Close open constructs in reverse nesting order
  if (inFence) {
    result = `${result}\n${fenceToken}`;
  } else if (inInlineCode) {
    result = `${result}\``;
  } else if (inDisplayMath) {
    result = `${result}$$`;
  } else if (inInlineMath && inlineMathStartIndex >= 0) {
    const unclosedSegment = content.slice(inlineMathStartIndex);
    if (shouldCloseInlineMath(unclosedSegment)) {
      result = `${result}$`;
    }
  }

  if (openBoldAsterisk) {
    result = `${result}**`;
  }
  if (openBoldUnderscore) {
    result = `${result}__`;
  }
  if (openItalicAsterisk) {
    result = `${result}*`;
  }
  if (openItalicUnderscore) {
    result = `${result}_`;
  }

  // Strip incomplete HTML tag at the end
  const htmlMatch = result.match(INCOMPLETE_HTML_TAG_RE);
  if (htmlMatch && htmlMatch.index !== undefined) {
    result = result.slice(0, htmlMatch.index);
  }

  return result;
}
