import { describe, it, expect } from 'vitest';
import { parseHtmlToInteractables } from '../../lib/html-parser';

describe('Visibility Fix Test', () => {
  const testHTML = `
<!DOCTYPE html>
<html>
<head><title>Test Page</title></head>
<body>
  <!-- Visible links -->
  <a href="https://example.com/page1">Page 1</a>
  <a href="https://example.com/page2">Page 2</a>
  
  <!-- Hidden links (should be filtered out) -->
  <a href="https://example.com/hidden1" style="display: none">Hidden 1</a>
  <a href="https://example.com/hidden2" hidden>Hidden 2</a>
  <a href="https://example.com/hidden3" class="hidden">Hidden 3</a>
  
  <!-- Buttons -->
  <button>Click Me</button>
  <button style="visibility: hidden">Hidden Button</button>
  
  <!-- Inputs -->
  <input type="text" placeholder="Enter text">
  <input type="text" style="display:none" placeholder="Hidden input">
</body>
</html>
`;

  it('should correctly identify visible elements (includeHidden: false)', () => {
    const result = parseHtmlToInteractables(testHTML);
    
    console.log('\n=== With includeHidden: false (default) ===');
    console.log('Total elements found:', result.elements.length);
    result.elements.forEach((el, i) => {
      console.log(`${i + 1}. ${el.type}: ${el.text || el.selector} (visible: ${el.visible})`);
    });

    // Should find only visible elements
    expect(result.elements.length).toBeGreaterThan(0);
    
    // All returned elements should be visible
    result.elements.forEach(el => {
      expect(el.visible).toBe(true);
    });
    
    // Should contain the visible links
    const visibleLinks = result.elements.filter(el => el.type === 'link');
    expect(visibleLinks.length).toBe(2); // Page 1, Page 2
  });

  it('should include hidden elements when includeHidden: true', () => {
    const result = parseHtmlToInteractables(testHTML, 'body', { includeHidden: true });
    
    console.log('\n=== With includeHidden: true ===');
    console.log('Total elements found:', result.elements.length);
    result.elements.forEach((el, i) => {
      console.log(`${i + 1}. ${el.type}: ${el.text || el.selector} (visible: ${el.visible})`);
    });

    // Should find more elements including hidden ones
    expect(result.elements.length).toBeGreaterThan(4);
    
    // Should contain both visible and hidden elements
    const visibleElements = result.elements.filter(el => el.visible);
    const hiddenElements = result.elements.filter(el => !el.visible);
    
    expect(visibleElements.length).toBeGreaterThan(0);
    expect(hiddenElements.length).toBeGreaterThan(0);
    
    // Should contain all links (visible + hidden)
    const allLinks = result.elements.filter(el => el.type === 'link');
    expect(allLinks.length).toBe(5); // Page 1, Page 2, Hidden 1, Hidden 2, Hidden 3
  });
});