/**
 * Checks if a string is a valid CSS identifier.
 * A valid CSS identifier must start with a letter, underscore, or hyphen,
 * and can be followed by letters, digits, underscores, or hyphens.
 *
 * @param str The string to validate.
 * @returns True if the string is a valid CSS identifier, false otherwise.
 */
export function isValidCSSIdentifier(str: string): boolean {
  return /^[a-zA-Z_-][a-zA-Z0-9_-]*$/.test(str);
}

/**
 * Escapes a string for use in a CSS selector.
 * It uses the `CSS.escape` method if available, otherwise it falls back
 * to a regular expression-based replacement.
 *
 * @param value The string to escape.
 * @returns The escaped string, safe to use in a CSS selector.
 */
export function safeCssEscape(value: string): string {
  const cssObj =
    typeof CSS !== 'undefined'
      ? (CSS as unknown as { escape?: (v: string) => string })
      : undefined;
  if (cssObj && typeof cssObj.escape === 'function') {
    try {
      return cssObj.escape(value);
    } catch {
      // ignore and fallback
    }
  }
  return value.replace(/([!"#$%&'()*+,\-./:;<=>?@[\\\]^`{|}~])/g, '\\$1');
}

/**
 * Checks if a given CSS selector uniquely identifies the target element within the document.
 *
 * @param selector The CSS selector to test.
 * @param targetElement The element that the selector should uniquely identify.
 * @param doc The document to search within.
 * @returns True if the selector is unique to the target element, false otherwise.
 */
function isSelectorUnique(
  selector: string,
  targetElement: Element,
  doc: Document,
): boolean {
  try {
    const elements = doc.querySelectorAll(selector);
    return elements.length === 1 && elements[0] === targetElement;
  } catch {
    return false;
  }
}

/**
 * Builds a unique CSS selector for a given DOM element.
 *
 * The function tries several strategies in order to find a unique selector:
 * 1.  Use the element's `id` if it's unique.
 * 2.  Use the `data-testid` attribute if it's unique.
 * 3.  For `<input>` elements, use a combination of `name` and `type` attributes if unique.
 * 4.  Use a valid class name if it's unique for that tag.
 * 5.  Construct a selector path from the element up to the document root, trying to find
 *     a unique selector by combining tag names, classes, and `:nth-of-type` pseudo-classes.
 *
 * @param element The DOM element for which to build a unique selector.
 * @param doc The document context of the element.
 * @returns A string representing the unique CSS selector.
 */
export function buildUniqueSelector(element: Element, doc: Document): string {
  const id = element.getAttribute('id');
  if (id && id.trim()) {
    const selector = `#${safeCssEscape(id)}`;
    if (isSelectorUnique(selector, element, doc)) return selector;
  }

  const testId = element.getAttribute('data-testid');
  if (testId && testId.trim()) {
    const selector = `[data-testid="${safeCssEscape(testId)}"]`;
    if (isSelectorUnique(selector, element, doc)) return selector;
  }

  if (element.tagName.toUpperCase() === 'INPUT') {
    const name = element.getAttribute('name');
    const type = element.getAttribute('type');
    if (name && type && name.trim() && type.trim()) {
      const selector = `input[name="${safeCssEscape(name)}"][type="${safeCssEscape(type)}"]`;
      if (isSelectorUnique(selector, element, doc)) return selector;
    }
  }

  const className = element.getAttribute('class');
  if (className && className.trim()) {
    const classes = className.trim().split(/\s+/);
    const validClass = classes.find((cls) => isValidCSSIdentifier(cls));
    if (validClass) {
      const selector = `${element.tagName.toLowerCase()}.${validClass}`;
      if (isSelectorUnique(selector, element, doc)) return selector;
    }
  }

  const parts: string[] = [];
  let current: Element | null = element;
  const maxDepth = 8;
  while (
    current &&
    current !== doc.documentElement &&
    parts.length < maxDepth
  ) {
    const tag = current.tagName.toLowerCase();
    const currentId = current.getAttribute('id');
    if (currentId && currentId.trim() && isValidCSSIdentifier(currentId)) {
      parts.unshift(`${tag}#${currentId}`);
      break;
    }
    const currentClass = current.getAttribute('class');
    if (currentClass && currentClass.trim()) {
      const classes = currentClass.trim().split(/\s+/);
      const validClass = classes.find((cls) => isValidCSSIdentifier(cls));
      if (validClass) {
        const parent = current.parentElement;
        if (parent) {
          const sameClassSiblings = Array.from(parent.children).filter(
            (child) => {
              if (child.tagName !== current!.tagName) return false;
              const cc = child.getAttribute('class');
              if (!cc) return false;
              return cc.trim().split(/\s+/).includes(validClass);
            },
          );
          if (sameClassSiblings.length === 1) {
            parts.unshift(`${tag}.${validClass}`);
            current = current.parentElement;
            continue;
          }
        }
      }
    }
    const parent = current.parentElement;
    if (parent) {
      const siblings = Array.from(parent.children).filter(
        (child) => child.tagName === current!.tagName,
      );
      if (siblings.length > 1) {
        const index = siblings.indexOf(current) + 1;
        parts.unshift(`${tag}:nth-of-type(${index})`);
      } else {
        parts.unshift(tag);
      }
      current = parent;
    } else {
      parts.unshift(tag);
      break;
    }
  }
  for (let i = 0; i < parts.length; i++) {
    const selector = parts.slice(i).join(' > ');
    if (isSelectorUnique(selector, element, doc)) return selector;
  }
  if (parts.length > 0) {
    const parent = element.parentElement;
    if (parent) {
      const siblings = Array.from(parent.children).filter(
        (child) => child.tagName === element.tagName,
      );
      if (siblings.length > 1) {
        const index = siblings.indexOf(element) + 1;
        const tag = element.tagName.toLowerCase();
        parts[parts.length - 1] = `${tag}:nth-of-type(${index})`;
        const uniqueSelector = parts.join(' > ');
        if (isSelectorUnique(uniqueSelector, element, doc))
          return uniqueSelector;
      }
    }
  }
  return parts.join(' > ') || element.tagName.toLowerCase();
}
