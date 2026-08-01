/**
 * Close incomplete markdown constructs during streaming so ReactMarkdown
 * does not produce broken layouts (unclosed fences, dangling emphasis, etc.).
 * When streaming finishes, callers should pass the original content unchanged.
 */

const FENCE_RE = /(^|\n)(```|~~~)/g;
const INCOMPLETE_HTML_TAG_RE = /<[A-Za-z][^>]*$/;

function countUnescapedOccurrences(text: string, token: string): number {
  let count = 0;
  let index = 0;

  while (index < text.length) {
    const found = text.indexOf(token, index);
    if (found === -1) {
      break;
    }
    if (found > 0 && text[found - 1] === '\\') {
      index = found + token.length;
      continue;
    }
    count += 1;
    index = found + token.length;
  }

  return count;
}

function isInsideOpenFence(content: string): boolean {
  let open = false;
  FENCE_RE.lastIndex = 0;
  while (FENCE_RE.exec(content) !== null) {
    open = !open;
  }
  return open;
}

function closeUnpairedMarker(content: string, marker: string): string {
  if (countUnescapedOccurrences(content, marker) % 2 === 0) {
    return content;
  }
  return `${content}${marker}`;
}

/**
 * Count single-char emphasis markers that are not part of a double marker
 * (e.g. `*` not consumed by `**`).
 */
function countSingleEmphasis(
  content: string,
  single: '*' | '_',
  double: '**' | '__',
): number {
  const withoutDouble = content.split(double).join('');
  return countUnescapedOccurrences(withoutDouble, single);
}

function closeSingleEmphasis(
  content: string,
  single: '*' | '_',
  double: '**' | '__',
): string {
  if (countSingleEmphasis(content, single, double) % 2 === 0) {
    return content;
  }
  return `${content}${single}`;
}

function stripIncompleteHtmlTag(content: string): string {
  const match = content.match(INCOMPLETE_HTML_TAG_RE);
  if (!match || match.index === undefined) {
    return content;
  }
  return content.slice(0, match.index);
}

/**
 * Returns display-safe markdown while `isStreaming` is true.
 * Returns `content` unchanged when streaming has finished.
 */
export function preprocessStreamMarkdown(
  content: string,
  isStreaming: boolean,
): string {
  if (!isStreaming || content.length === 0) {
    return content;
  }

  let result = content;

  if (isInsideOpenFence(result)) {
    result = `${result}\n\`\`\``;
  } else {
    // Close unpaired inline code only outside an open fence
    result = closeUnpairedMarker(result, '`');
  }

  result = closeUnpairedMarker(result, '**');
  result = closeUnpairedMarker(result, '__');
  result = closeSingleEmphasis(result, '*', '**');
  result = closeSingleEmphasis(result, '_', '__');
  result = stripIncompleteHtmlTag(result);

  return result;
}
