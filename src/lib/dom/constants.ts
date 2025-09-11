// Centralized constants for DOM parsing utilities

export const DEFAULT_PARSE_OPTIONS = {
  maxDepth: 5,
  includeLinks: true,
  maxTextLength: 1000,
} as const;

export const DEFAULT_DOM_MAP_OPTIONS = {
  maxDepth: 10,
  maxChildren: 20,
  maxTextLength: 100,
  includeInteractiveOnly: false,
} as const;

export const DEFAULT_INTERACTABLE_OPTIONS = {
  includeHidden: false,
  maxElements: 100,
} as const;

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
