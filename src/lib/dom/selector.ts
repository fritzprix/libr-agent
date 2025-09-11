// Selector utilities extracted from html-parser.ts for reuse

export function isValidCSSIdentifier(str: string): boolean {
  return /^[a-zA-Z_-][a-zA-Z0-9_-]*$/.test(str);
}

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
