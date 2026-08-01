import { useMemo } from 'react';
import { preprocessStreamMarkdown } from '../utils/streamMarkdownPreprocess';

/**
 * During streaming, returns markdown with unclosed fences / markers closed
 * so the renderer stays stable. When streaming ends, returns original content.
 */
export function useStreamMarkdownPreprocess(
  content: string,
  isStreaming: boolean,
): string {
  return useMemo(
    () => preprocessStreamMarkdown(content, isStreaming),
    [content, isStreaming],
  );
}
