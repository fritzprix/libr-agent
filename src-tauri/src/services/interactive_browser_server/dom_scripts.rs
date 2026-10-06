//! Page scripts for sidecar CDP evaluate (userChrome uses extension-injected funcs instead).

pub fn click_script(selector: &str) -> Result<String, String> {
    let selector_json =
        serde_json::to_string(selector).map_err(|e| format!("Serialization error: {}", e))?;

    Ok(format!(
        r#"(function() {{
            const el = document.querySelector({});
            if (!el) return 'Element not found';

            const style = window.getComputedStyle(el);
            if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {{
                return 'Element not visible';
            }}

            el.scrollIntoView({{block: 'center'}});
            if (typeof el.focus === 'function') el.focus();
            if (typeof el.click === 'function') {{
                el.click();
            }} else {{
                el.dispatchEvent(new MouseEvent('click', {{ bubbles: true, cancelable: true, view: window }}));
            }}
            return 'Clicked element';
        }})()"#,
        selector_json
    ))
}

pub fn input_text_script(selector: &str, text: &str) -> Result<String, String> {
    let selector_json =
        serde_json::to_string(selector).map_err(|e| format!("Serialization error: {}", e))?;
    let text_json =
        serde_json::to_string(text).map_err(|e| format!("Serialization error: {}", e))?;

    Ok(format!(
        r#"(function() {{
            const el = document.querySelector({});
            if (!el) return 'Element not found';

            const style = window.getComputedStyle(el);
            if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {{
                return 'Element not visible';
            }}

            const text = {};
            el.scrollIntoView({{block: 'center'}});
            if (typeof el.focus === 'function') el.focus();

            const isTextArea = el instanceof HTMLTextAreaElement;
            const isInput = el instanceof HTMLInputElement;
            if (isInput || isTextArea) {{
                const prototype = isTextArea
                    ? window.HTMLTextAreaElement.prototype
                    : window.HTMLInputElement.prototype;
                const setter = Object.getOwnPropertyDescriptor(prototype, 'value')?.set;
                const prevValue = el.value;
                if (setter) {{
                    setter.call(el, text);
                }} else {{
                    el.value = text;
                }}
                if (el._valueTracker && typeof el._valueTracker.setValue === 'function') {{
                    el._valueTracker.setValue(prevValue);
                }}
                el.dispatchEvent(new InputEvent('input', {{
                    bubbles: true,
                    cancelable: true,
                    inputType: 'insertText',
                    data: text
                }}));
                el.dispatchEvent(new Event('change', {{bubbles: true}}));
                return 'Input successful';
            }}

            if (el.isContentEditable) {{
                let inserted = false;
                try {{
                    const selection = window.getSelection();
                    const range = document.createRange();
                    range.selectNodeContents(el);
                    selection.removeAllRanges();
                    selection.addRange(range);
                    inserted = document.execCommand('insertText', false, text);
                }} catch (_) {{
                    inserted = false;
                }}
                if (!inserted) {{
                    el.textContent = text;
                    el.dispatchEvent(new InputEvent('input', {{
                        bubbles: true,
                        cancelable: true,
                        inputType: 'insertText',
                        data: text
                    }}));
                }}
                return 'Input successful';
            }}

            return 'Element is not an input';
        }})()"#,
        selector_json, text_json
    ))
}
