import type { ParseOptions, DOMMapOptions, InteractableOptions } from './types';

// Configuration constants for parsing
export const DEFAULT_PARSE_OPTIONS: Required<ParseOptions> = {
  maxDepth: 5,
  includeLinks: true,
  maxTextLength: 1000,
};

export const DEFAULT_DOM_MAP_OPTIONS: Required<DOMMapOptions> = {
  maxDepth: 10,
  maxChildren: 20,
  maxTextLength: 100,
  includeInteractiveOnly: false,
};

export const DEFAULT_INTERACTABLE_OPTIONS: Required<InteractableOptions> = {
  includeHidden: false,
  maxElements: 100,
};

// Tag sets for filtering
export const EXCLUDE_TAGS = new Set([
  'SCRIPT',
  'STYLE',
  'NOSCRIPT',
  'META',
  'LINK',
  'HEAD',
]);

export const EXCLUDE_CLASSES = [
  'ad',
  'banner',
  'popup',
  'sidebar',
  'advertisement',
  'tracking',
];

export const INTERACTIVE_TAGS = new Set([
  'A',
  'BUTTON',
  'INPUT',
  'SELECT',
  'TEXTAREA',
  'FORM',
  'IFRAME',
]);

export const MEANINGFUL_ELEMENTS = new Set([
  'a',
  'button',
  'input',
  'img',
  'video',
  'audio',
  'iframe',
  'form',
  'table',
]);

// Selectors for interactable elements
export const INTERACTABLE_SELECTORS = [
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'a[href]:not([href="#"]):not([href=""])',
  '[role="button"]:not([aria-disabled="true"])',
  '[onclick]',
  '[data-action]',
].join(',');
