/**
 * Text processing utilities for content extraction and formatting
 */

/**
 * Cleans up markdown text by removing excessive whitespace and normalizing line breaks
 * @param text The markdown text to clean
 * @returns Cleaned markdown text
 */
export function cleanMarkdownText(text: string): string {
  return text
    .replace(/\n{2,}/g, '\n') // Replace 2+ newlines with 1
    .replace(/[ \t]+\n/g, '\n') // Remove trailing spaces before newline
    .replace(/\n[ \t]+/g, '\n') // Remove leading spaces after newline
    .replace(/[ \t]{2,}/g, ' ') // Replace multiple spaces/tabs with single space
    .trim();
}

/**
 * Normalizes whitespace in regular text content
 * @param text The text to normalize
 * @returns Text with normalized whitespace
 */
export function normalizeWhitespace(text: string): string {
  return text
    .replace(/\s+/g, ' ') // Replace multiple whitespace with single space
    .trim();
}

/**
 * Truncates text to a specified length with ellipsis
 * @param text The text to truncate
 * @param maxLength Maximum length before truncation
 * @param suffix Suffix to append when truncated (default: '...')
 * @returns Truncated text
 */
export function truncateText(
  text: string,
  maxLength: number,
  suffix = '...',
): string {
  if (text.length <= maxLength) {
    return text;
  }
  return text.substring(0, maxLength - suffix.length) + suffix;
}

/**
 * Removes common unwanted characters and patterns from extracted text
 * @param text The text to sanitize
 * @returns Sanitized text
 */
export function sanitizeExtractedText(text: string): string {
  return text
    .replace(/[\u200B-\u200D\uFEFF]/g, '') // Remove zero-width characters
    .replace(/\u00A0/g, ' ') // Replace non-breaking spaces with regular spaces
    .replace(/[\r\n\t]+/g, ' ') // Replace line breaks and tabs with spaces
    .trim();
}

/**
 * Creates ultra-compact text by removing all unnecessary whitespace
 * @param text The text to make compact
 * @returns Most compact version of the text
 */
export function createCompactText(text: string): string {
  return text
    .replace(/[\r\n\t]+/g, ' ') // Replace all line breaks and tabs with spaces
    .replace(/\s{2,}/g, ' ') // Replace multiple spaces with single space
    .replace(/^\s+|\s+$/g, '') // Remove leading and trailing whitespace
    .replace(/\s*([{}[\],:])\s*/g, '$1') // Remove spaces around JSON punctuation
    .replace(/\s*([<>])\s*/g, '$1'); // Remove spaces around angle brackets
}
