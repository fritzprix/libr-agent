import type { MCPContent } from './mcp-types';

/**
 * UI content types that should be rendered visually
 */
export const UI_CONTENT_TYPES = [
  'resource',
  'image',
  'audio',
  'resource_link',
] as const;

/**
 * Separates content into UI content and text content
 * @param content - Array of MCPContent items
 * @returns Object with uiContent and textContent arrays
 */
export function separateContent(content: MCPContent[]) {
  const uiContent = content.filter((item) =>
    UI_CONTENT_TYPES.includes(item.type as (typeof UI_CONTENT_TYPES)[number]),
  );
  const textContent = content.filter(
    (item) =>
      !UI_CONTENT_TYPES.includes(
        item.type as (typeof UI_CONTENT_TYPES)[number],
      ),
  );

  return { uiContent, textContent };
}
