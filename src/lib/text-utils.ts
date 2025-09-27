/**
 * @file Text processing utilities for content extraction and formatting.
 */

/**
 * Cleans up markdown text by removing excessive whitespace and normalizing line breaks,
 * while preserving the core markdown structure.
 *
 * @param text The markdown text to clean.
 * @returns The cleaned markdown text.
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
 * Normalizes all whitespace characters (spaces, tabs, newlines) in a string
 * into a single space, and trims leading/trailing whitespace.
 *
 * @param text The text to normalize.
 * @returns The text with normalized whitespace.
 */
export function normalizeWhitespace(text: string): string {
  return text
    .replace(/\s+/g, ' ') // Replace multiple whitespace with single space
    .trim();
}

/**
 * Truncates a string to a specified maximum length, appending a suffix if truncated.
 *
 * @param text The text to truncate.
 * @param maxLength The maximum length of the output string (including the suffix).
 * @param suffix The suffix to append if the text is truncated. Defaults to '...'.
 * @returns The truncated text, or the original text if it's within the length limit.
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
 * Sanitizes text extracted from an external source (like a web page) by removing
 * common unwanted characters and normalizing whitespace.
 *
 * @param text The text to sanitize.
 * @returns The sanitized text.
 */
export function sanitizeExtractedText(text: string): string {
  return text
    .replace(/[\u200B-\u200D\uFEFF]/g, '') // Remove zero-width characters
    .replace(/\u00A0/g, ' ') // Replace non-breaking spaces with regular spaces
    .replace(/[\r\n\t]+/g, ' ') // Replace line breaks and tabs with spaces
    .trim();
}

/**
 * Creates an ultra-compact version of a string by removing all unnecessary whitespace,
 * including around common punctuation, for efficient storage or transmission.
 *
 * @param text The text to make compact.
 * @returns The most compact version of the text.
 */
export function createCompactText(text: string): string {
  return text
    .replace(/[\r\n\t]+/g, ' ') // Replace all line breaks and tabs with spaces
    .replace(/\s{2,}/g, ' ') // Replace multiple spaces with single space
    .replace(/^\s+|\s+$/g, '') // Remove leading and trailing whitespace
    .replace(/\s*([{}[\],:])\s*/g, '$1') // Remove spaces around JSON punctuation
    .replace(/\s*([<>])\s*/g, '$1'); // Remove spaces around angle brackets
}
