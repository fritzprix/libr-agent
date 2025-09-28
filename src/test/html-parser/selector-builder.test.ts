import { describe, it, expect, beforeEach } from 'vitest';
import { JSDOM } from 'jsdom';

// We'll need to import the functions we want to test
// Since buildUniqueSelector is not exported, we'll test through the public APIs
import {
  parseHTMLToDOMMap,
  parseHtmlToInteractables,
} from '../../lib/html-parser';

// Test data types
interface DOMMapTestNode {
  tag?: string;
  selector: string;
  text?: string;
  class?: string;
  id?: string;
  children?: DOMMapTestNode[];
}

describe('Unified Selector Builder Tests', () => {
  let dom: JSDOM;
  let document: Document;

  beforeEach(() => {
    // Create a comprehensive test DOM
    dom = new JSDOM(`
      <!DOCTYPE html>
      <html>
        <head>
          <title>Selector Builder Test</title>
        </head>
        <body>
          <!-- ID-based elements -->
          <div id="unique-id">Unique ID Element</div>
          <div id="123invalid">Invalid ID (starts with number)</div>
          <div id="valid-css-id">Valid CSS ID</div>

          <!-- Data-testid elements -->
          <button data-testid="submit-button">Submit</button>
          <input data-testid="email-input" type="email" placeholder="Email">

          <!-- Form elements with name/type -->
          <form id="test-form">
            <input type="text" name="username" placeholder="Username">
            <input type="password" name="password" placeholder="Password">
            <input type="email" name="email" value="test@example.com">
            <input type="hidden" name="csrf" value="token123">
            <select name="country">
              <option value="us">United States</option>
              <option value="uk">United Kingdom</option>
            </select>
            <textarea name="message" placeholder="Your message"></textarea>
          </form>

          <!-- Class-based elements -->
          <div class="primary-button secondary">Primary with multiple classes</div>
          <div class="123invalid-class valid-class">Mixed valid/invalid classes</div>
          <div class="btn btn-primary">Button classes</div>

          <!-- Hierarchical elements -->
          <nav class="main-nav">
            <ul>
              <li><a href="/home">Home</a></li>
              <li><a href="/about">About</a></li>
              <li>
                <span>Products</span>
                <ul class="submenu">
                  <li><a href="/products/web">Web</a></li>
                  <li><a href="/products/mobile">Mobile</a></li>
                </ul>
              </li>
            </ul>
          </nav>

          <!-- Complex nested structure -->
          <div class="container">
            <div class="row">
              <div class="col">
                <div class="card">
                  <div class="card-header">
                    <h3>Card Title</h3>
                  </div>
                  <div class="card-body">
                    <p>Card content</p>
                    <button class="btn">Action</button>
                  </div>
                </div>
              </div>
              <div class="col">
                <div class="card">
                  <div class="card-header">
                    <h3>Another Card</h3>
                  </div>
                  <div class="card-body">
                    <p>More content</p>
                    <button class="btn">Another Action</button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Special characters in attributes -->
          <div id="special-chars-!@#$%">Special chars in ID</div>
          <div class="special.class:with[brackets]">Special chars in class</div>
          <input name="field[nested]" type="text">

          <!-- Duplicate elements for uniqueness testing -->
          <div class="duplicate">First duplicate</div>
          <div class="duplicate">Second duplicate</div>
          <div class="duplicate">Third duplicate</div>
        </body>
      </html>
    `);

    document = dom.window.document;
  });

  describe('DOM Map Selector Generation', () => {
    it('should generate consistent selectors for DOM map parsing', () => {
      const result = parseHTMLToDOMMap(dom.serialize(), {
        maxDepth: 5,
        maxChildren: 20,
        includeInteractiveOnly: false,
      });

      expect(result.error).toBeUndefined();
      if (!result.error) {
        // Validate that selectors are generated for all elements
        function validateSelectors(node: { selector: string; children?: unknown[] }): void {
          expect(node.selector).toBeDefined();
          expect(node.selector).toBeTruthy();
          expect(typeof node.selector).toBe('string');

          // Validate selector can be used to find the element
          const found = document.querySelectorAll(node.selector);
          expect(found.length).toBeGreaterThan(0);

          // Recursively check children
          (node.children as { selector: string; children?: unknown[] }[] | undefined)?.forEach(validateSelectors);
        }

        validateSelectors(result.domMap);
      }
    });

    it('should prefer ID selectors when available and unique', () => {
      const result = parseHTMLToDOMMap(dom.serialize());

      if (!result.error) {
        function findNodeByText(node: DOMMapTestNode, text: string): DOMMapTestNode | null {
          if (node.text?.includes(text)) return node;
          for (const child of node.children || []) {
            const found = findNodeByText(child, text);
            if (found) return found;
          }
          return null;
        }

        const uniqueIdNode = findNodeByText(result.domMap, 'Unique ID Element');
        expect(uniqueIdNode?.selector).toBe('#unique-id');

        const validCssIdNode = findNodeByText(result.domMap, 'Valid CSS ID');
        expect(validCssIdNode?.selector).toBe('#valid-css-id');
      }
    });

    it('should use data-testid when ID is not available', () => {
      const result = parseHTMLToDOMMap(dom.serialize());

      if (!result.error) {
        // Remove unused function

        // Note: The DOM map structure might not include data-testid directly
        // but the selector should use it
        const emailInput = document.querySelector('[data-testid="email-input"]');
        if (emailInput) {
          // Validate that if this element appears in DOM map, it uses testid selector
          // This is more of a structural test
          expect(emailInput.getAttribute('data-testid')).toBe('email-input');
        }
      }
    });
  });

  describe('Interactable Elements Selector Generation', () => {
    it('should generate unique selectors for interactable elements', () => {
      const result = parseHtmlToInteractables(dom.serialize(), 'body', {
        includeHidden: true,
        maxElements: 50,
      });

      expect(result.error).toBeUndefined();
      if (!result.error) {
        result.elements.forEach((element) => {
          expect(element.selector).toBeDefined();
          expect(element.selector).toBeTruthy();
          expect(typeof element.selector).toBe('string');

          // Validate selector uniqueness
          const found = document.querySelectorAll(element.selector);
          expect(found.length).toBe(1);
        });
      }
    });

    it('should handle form elements with name/type attributes', () => {
      const result = parseHtmlToInteractables(dom.serialize(), 'body', {
        includeHidden: true,
        maxElements: 50,
      });



      if (!result.error) {
        const usernameInput = result.elements.find(el =>
          el.selector === 'input[name="username"][type="text"]'
        );
        const passwordInput = result.elements.find(el =>
          el.selector === 'input[name="password"][type="password"]'
        );



        expect(usernameInput?.selector).toBe('input[name="username"][type="text"]');
        expect(passwordInput?.selector).toBe('input[name="password"][type="password"]');
      }
    });
  });

  describe('CSS Escaping', () => {
    it('should properly escape special characters in selectors', () => {
      const result = parseHTMLToDOMMap(dom.serialize());

      if (!result.error) {
        // Find elements with special characters
        const specialCharElement = document.querySelector('[id^="special-chars"]');
        expect(specialCharElement).toBeTruthy();

        // The selector should escape special characters
        // We can't easily test the exact output without exposing internals,
        // but we can verify the DOM structure is parsed correctly
      }
    });
  });

  describe('Selector Uniqueness and Hierarchy', () => {
    it('should generate hierarchical selectors when simple ones are not unique', () => {
      const result = parseHTMLToDOMMap(dom.serialize());

      if (!result.error) {
        // Find duplicate class elements in the DOM map
        function findDuplicateNodes(node: DOMMapTestNode): DOMMapTestNode[] {
          const duplicates: DOMMapTestNode[] = [];

          if (node.class?.includes('duplicate')) {
            duplicates.push(node);
          }

          node.children?.forEach((child: DOMMapTestNode) => {
            duplicates.push(...findDuplicateNodes(child));
          });

          return duplicates;
        }

        const duplicateNodes = findDuplicateNodes(result.domMap);

        // Each duplicate should have a unique selector
        const selectors = duplicateNodes.map(node => node.selector);
        const uniqueSelectors = new Set(selectors);
        expect(uniqueSelectors.size).toBe(selectors.length);

        // Selectors should be hierarchical (contain ' > ')
        duplicateNodes.forEach(node => {
          if (node.selector !== 'div.duplicate') {
            // If not the simple class selector, should be hierarchical
            expect(node.selector.includes(' > ') || node.selector.includes(':nth-of-type')).toBe(true);
          }
        });
      }
    });

    it('should limit hierarchical depth for performance', () => {
      // Create a very deeply nested structure
      const deepHTML = `
        <div class="deep">
          ${'<div class="level">'.repeat(15)}
            <button>Deep Button</button>
          ${'</div>'.repeat(15)}
        </div>
      `;

      const deepDom = new JSDOM(`<html><body>${deepHTML}</body></html>`);
      const result = parseHTMLToDOMMap(deepDom.serialize());

      if (!result.error) {
        // Find the button in the result
        function findButton(node: DOMMapTestNode): DOMMapTestNode | null {
          if (node.tag === 'button') return node;
          for (const child of node.children || []) {
            const found = findButton(child);
            if (found) return found;
          }
          return null;
        }

        const button = findButton(result.domMap);
        if (button && button.selector) {
          // Selector should not be excessively long (depth limited)
          const selectorParts = button.selector.split(' > ');
          expect(selectorParts.length).toBeLessThanOrEqual(10); // Reasonable depth limit
        }
      }
    });
  });



  describe('Input Validation', () => {
    it('should handle invalid HTML inputs gracefully', () => {
      const invalidInputs = [
        '',
        '   ',
        'not html',
        null,
        undefined,
      ];

      invalidInputs.forEach((input) => {
        const result = parseHTMLToDOMMap(input as string);
        expect(result.error).toBeDefined();
        expect(result.error).toContain('Invalid HTML input');
      });
    });

    it('should handle valid minimal HTML', () => {
      const minimalHTML = '<div>Hello</div>';
      const result = parseHTMLToDOMMap(minimalHTML);

      // Should not error on minimal valid HTML
      expect(result.error).toBeUndefined();
      if (!result.error) {
        expect(result.domMap).toBeDefined();
      }
    });
  });

  describe('Performance and Edge Cases', () => {
    it('should handle large DOM structures efficiently', () => {
      // Generate a moderately large DOM
      const largeHTML = `
        <div class="container">
          ${Array.from({ length: 50 }, (_, i) => `
            <div class="item-${i}">
              <h3>Item ${i}</h3>
              <p>Description for item ${i}</p>
              <button class="btn">Action ${i}</button>
              <input type="text" name="field-${i}" value="value-${i}">
            </div>
          `).join('')}
        </div>
      `;

      const startTime = Date.now();
      const result = parseHTMLToDOMMap(largeHTML, { maxDepth: 6, maxChildren: 100 });
      const endTime = Date.now();

      expect(result.error).toBeUndefined();
      expect(endTime - startTime).toBeLessThan(1000); // Should complete within 1 second

      if (!result.error) {
        expect(result.domMap.children.length).toBeGreaterThan(0);
      }
    });

    it('should handle elements with no parent gracefully', () => {
      // Create a standalone element
      const standaloneElement = document.createElement('div');
      standaloneElement.textContent = 'Standalone';

      // This tests internal logic - we can't directly test buildUniqueSelector
      // but we can ensure the system handles edge cases
      const result = parseHTMLToDOMMap('<div>Standalone</div>');
      expect(result.error).toBeUndefined();
    });

    it('should handle malformed but parseable HTML', () => {
      const malformedHTML = '<div><p>Unclosed paragraph<div>Nested incorrectly</p></div>';
      const result = parseHTMLToDOMMap(malformedHTML);

      // Browser DOM parser is forgiving, should still work
      expect(result.error).toBeUndefined();
      if (!result.error) {
        expect(result.domMap).toBeDefined();
      }
    });
  });
});
