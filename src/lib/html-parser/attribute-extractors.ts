import type { AttributeExtractor } from './types';
import { isHTMLInputElement } from './utils';
import { INTERACTIVE_TAGS } from './constants';

/**
 * Extracts basic attributes (id, class, title) from elements
 */
export class BasicAttributeExtractor
  implements AttributeExtractor<{ id?: string; class?: string; title?: string }>
{
  canExtract(): boolean {
    return true;
  }

  extract(element: Element): { id?: string; class?: string; title?: string } {
    const result: { id?: string; class?: string; title?: string } = {};

    const id = element.getAttribute('id');
    if (id) result.id = id;

    const className = element.getAttribute('class');
    if (className?.trim()) result.class = className.trim();

    const title = element.getAttribute('title');
    if (title) result.title = title;

    return result;
  }
}

/**
 * Extracts link-related attributes (href, src, alt)
 */
export class LinkAttributeExtractor
  implements AttributeExtractor<{ href?: string; src?: string; alt?: string }>
{
  constructor(private includeLinks: boolean) {}

  canExtract(): boolean {
    return this.includeLinks;
  }

  extract(element: Element): { href?: string; src?: string; alt?: string } {
    const result: { href?: string; src?: string; alt?: string } = {};

    const href = element.getAttribute('href');
    if (href) result.href = href;

    const src = element.getAttribute('src');
    if (src) result.src = src;

    const alt = element.getAttribute('alt');
    if (alt) result.alt = alt;

    return result;
  }
}

/**
 * Extracts interactive element attributes (type, placeholder, value, name, role, aria-label)
 */
export class InteractiveAttributeExtractor
  implements
    AttributeExtractor<{
      type?: string;
      placeholder?: string;
      value?: string;
      name?: string;
      role?: string;
      ariaLabel?: string;
    }>
{
  canExtract(element: Element): boolean {
    return INTERACTIVE_TAGS.has(element.tagName.toUpperCase());
  }

  extract(element: Element): {
    type?: string;
    placeholder?: string;
    value?: string;
    name?: string;
    role?: string;
    ariaLabel?: string;
  } {
    const result: {
      type?: string;
      placeholder?: string;
      value?: string;
      name?: string;
      role?: string;
      ariaLabel?: string;
    } = {};

    const type = element.getAttribute('type');
    if (type) result.type = type;

    const placeholder = element.getAttribute('placeholder');
    if (placeholder) result.placeholder = placeholder;

    if (isHTMLInputElement(element)) {
      const value = element.value;
      if (value) result.value = value;
    }

    const name = element.getAttribute('name');
    if (name) result.name = name;

    const role = element.getAttribute('role');
    if (role) result.role = role;

    const ariaLabel = element.getAttribute('aria-label');
    if (ariaLabel) result.ariaLabel = ariaLabel;

    return result;
  }
}

/**
 * Manages multiple attribute extractors and combines their results
 */
export class AttributeExtractorManager<T> {
  private extractors: AttributeExtractor<Partial<T>>[] = [];

  addExtractor(extractor: AttributeExtractor<Partial<T>>): this {
    this.extractors.push(extractor);
    return this;
  }

  extractAll(element: Element): Partial<T> {
    let result: Partial<T> = {};

    for (const extractor of this.extractors) {
      if (extractor.canExtract(element)) {
        const extracted = extractor.extract(element);
        result = { ...result, ...extracted };
      }
    }

    return result;
  }
}
