import { describe, expect, it } from 'vitest';
import { disableLegacyInteractiveForm } from '../disableLegacyInteractiveForm';

const FORM_HTML =
  '<html><body><form><input id="a"><select id="b"></select><textarea id="c"></textarea><button id="d">Send</button></form></body></html>';

function legacy(text: string) {
  return { uri: 'ui://interactive/msg-1', mimeType: 'text/html', text };
}

describe('disableLegacyInteractiveForm', () => {
  it('disables every form control of an old interactive form', () => {
    const resource = disableLegacyInteractiveForm(legacy(FORM_HTML));
    const text = resource.text ?? '';
    expect(text.indexOf('<script')).toBeLessThan(text.indexOf('</body>'));

    const doc = document.implementation.createHTMLDocument('form');
    doc.documentElement.innerHTML = text;
    const script = doc.querySelector('script')?.textContent ?? '';
    new Function('document', script)(doc);

    for (const id of ['a', 'b', 'c', 'd']) {
      const control = doc.getElementById(id);
      expect(control?.hasAttribute('disabled'), id).toBe(true);
    }
  });

  it('leaves other resources unchanged', () => {
    const result = {
      uri: 'ui://result/msg-1',
      mimeType: 'text/html',
      text: FORM_HTML,
    };
    expect(disableLegacyInteractiveForm(result)).toBe(result);
  });

  it('adds the block only once', () => {
    const once = disableLegacyInteractiveForm(legacy(FORM_HTML));
    expect(disableLegacyInteractiveForm(once)).toBe(once);
  });
});
