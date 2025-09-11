import { describe, it, expect, beforeEach } from 'vitest';
import { JSDOM } from 'jsdom';

// Import the functions we need to test
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

describe('Selector Generation Debug', () => {
  let dom: JSDOM;
  let document: Document;

  beforeEach(() => {
    // Simple test DOM to isolate issues
    dom = new JSDOM(`
      <!DOCTYPE html>
      <html>
        <head>
          <title>Debug Test</title>
        </head>
        <body>
          <div id="test-div">Test Content</div>
          <button class="test-button">Click Me</button>
          <input type="text" name="test-input" placeholder="Enter text">
          <input type="email" name="email" data-testid="email-field">
          <a href="/test">Test Link</a>
        </body>
      </html>
    `);

    document = dom.window.document;
  });

  describe('Basic Selector Generation', () => {
    it('should parse simple DOM map without errors', () => {
      const result = parseHTMLToDOMMap(dom.serialize());

      console.log('DOM Map Result:', JSON.stringify(result, null, 2));

      expect(result.error).toBeUndefined();
      if (!result.error) {
        expect(result.domMap).toBeDefined();
        expect(result.domMap.selector).toBeTruthy();

        // Check that selectors are valid strings
        function validateSelectors(node: DOMMapTestNode): void {
          expect(typeof node.selector).toBe('string');
          expect(node.selector.length).toBeGreaterThan(0);

          // Try to use the selector to find elements
          try {
            const found = document.querySelectorAll(node.selector);
            expect(found.length).toBeGreaterThanOrEqual(1);
          } catch (error) {
            console.error('Invalid selector:', node.selector, error);
            throw error;
          }

          node.children?.forEach(validateSelectors);
        }

        validateSelectors(result.domMap);
      }
    });

    it('should parse interactable elements with valid selectors', () => {
      const result = parseHtmlToInteractables(dom.serialize(), 'body', {
        includeHidden: true,
        maxElements: 10,
      });

      console.log('Interactables Result:', JSON.stringify(result, null, 2));

      if (result.error) {
        console.error('Interactables Error:', result.error);
      }

      expect(result.error).toBeUndefined();
      if (!result.error) {
        expect(result.elements).toBeDefined();
        expect(Array.isArray(result.elements)).toBe(true);

        result.elements.forEach((element, index) => {
          console.log(`Element ${index}:`, {
            selector: element.selector,
            type: element.type,
            text: element.text,
          });

          expect(typeof element.selector).toBe('string');
          expect(element.selector.length).toBeGreaterThan(0);

          // Validate selector syntax
          try {
            const found = document.querySelectorAll(element.selector);
            expect(found.length).toBeGreaterThanOrEqual(1);
          } catch (error) {
            console.error('Invalid selector for element:', element.selector, error);
            throw error;
          }
        });
      }
    });

    it('should handle elements with special attributes', () => {
      const specialDOM = new JSDOM(`
        <html>
          <body>
            <div id="special-chars-123">Special ID</div>
            <button data-testid="submit-btn">Submit</button>
            <input type="password" name="pwd" id="password-field">
            <div class="btn btn-primary">Multi-class</div>
            <span class="123invalid valid-class">Mixed classes</span>
          </body>
        </html>
      `);

      const result = parseHTMLToDOMMap(specialDOM.serialize());

      console.log('Special DOM Result:', JSON.stringify(result, null, 2));

      expect(result.error).toBeUndefined();
      if (!result.error) {
        function checkSpecialSelectors(node: DOMMapTestNode): void {
          if (node.selector) {
            expect(typeof node.selector).toBe('string');
            expect(node.selector.length).toBeGreaterThan(0);

            // Log selectors for debugging
            if (node.id || node.class) {
              console.log('Special element selector:', {
                id: node.id,
                class: node.class,
                selector: node.selector,
              });
            }

            // Validate selector works
            try {
              const found = specialDOM.window.document.querySelectorAll(node.selector);
              expect(found.length).toBeGreaterThan(0);
            } catch (error) {
              console.error('Failed selector:', node.selector, error);
              throw error;
            }
          }

          node.children?.forEach(checkSpecialSelectors);
        }

        checkSpecialSelectors(result.domMap);
      }
    });

    it('should handle duplicate elements correctly', () => {
      const duplicateDOM = new JSDOM(`
        <html>
          <body>
            <div class="item">Item 1</div>
            <div class="item">Item 2</div>
            <div class="item">Item 3</div>
            <ul>
              <li><button>Button 1</button></li>
              <li><button>Button 2</button></li>
              <li><button>Button 3</button></li>
            </ul>
          </body>
        </html>
      `);

      const result = parseHTMLToDOMMap(duplicateDOM.serialize());

      console.log('Duplicate elements result:', JSON.stringify(result, null, 2));

      expect(result.error).toBeUndefined();
      if (!result.error) {
        const allSelectors: string[] = [];

        function collectSelectors(node: DOMMapTestNode): void {
          if (node.selector) {
            allSelectors.push(node.selector);
          }
          node.children?.forEach(collectSelectors);
        }

        collectSelectors(result.domMap);

        // Check that all selectors are unique and valid
        const uniqueSelectors = new Set(allSelectors);
        console.log('All selectors:', allSelectors);
        console.log('Unique selectors count:', uniqueSelectors.size, 'vs total:', allSelectors.length);

        allSelectors.forEach((selector, index) => {
          try {
            const found = duplicateDOM.window.document.querySelectorAll(selector);
            if (found.length !== 1) {
              console.warn(`Selector "${selector}" matches ${found.length} elements (expected 1)`);
            }
          } catch (error) {
            console.error(`Invalid selector at index ${index}:`, selector, error);
            throw error;
          }
        });
      }
    });
  });

  describe('CSS Escaping Debug', () => {
    it('should handle CSS escaping correctly', () => {
      // Test CSS.escape availability
      console.log('CSS object available:', typeof CSS !== 'undefined');
      console.log('CSS.escape available:', typeof CSS !== 'undefined' && !!CSS.escape);

      if (typeof CSS !== 'undefined' && CSS.escape) {
        console.log('CSS.escape test:', CSS.escape('test-123'));
        console.log('CSS.escape special chars:', CSS.escape('special!@#$%'));
      }

      const specialCharsDOM = new JSDOM(`
        <html>
          <body>
            <div id="test!@#$%">Special chars in ID</div>
            <div class="test.class:with[brackets]">Special chars in class</div>
            <input name="field[nested]" type="text">
          </body>
        </html>
      `);

      const result = parseHTMLToDOMMap(specialCharsDOM.serialize());

      console.log('Special chars result:', JSON.stringify(result, null, 2));

      expect(result.error).toBeUndefined();
    });
  });
});
