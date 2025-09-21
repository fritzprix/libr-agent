import type { MCPContent } from './mcp-types';

/**
 * A constant array of MCPContent types that are considered to be UI content.
 * These content types are typically rendered as visual elements rather than plain text.
 * It includes types like resources, images, audio, and resource links.
 */
export const UI_CONTENT_TYPES = [
  'resource',
  'image',
  'audio',
  'resource_link',
] as const;

/**
 * Separates an array of MCPContent objects into two distinct arrays:
 * one for UI-related content and one for text-based content.
 * This is useful for rendering different types of content in different ways.
 *
 * @param content The array of MCPContent items to be separated.
 * @returns An object containing two arrays: `uiContent` for UI-related items
 *          and `textContent` for all other items.
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
