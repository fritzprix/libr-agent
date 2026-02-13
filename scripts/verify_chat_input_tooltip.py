import argparse
import os
from pathlib import Path

from playwright.sync_api import sync_playwright, Page, expect


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description='Capture chat input tooltip screenshot with mocked Tauri IPC.',
    )
    parser.add_argument(
        '--base-url',
        default='http://localhost:1420',
        help='Frontend base URL (default: http://localhost:1420)',
    )
    parser.add_argument(
        '--session-id',
        default='test-session',
        help='Agent session ID used in route (default: test-session)',
    )
    parser.add_argument(
        '--output-dir',
        default=os.environ.get('CHAT_INPUT_TOOLTIP_OUTPUT_DIR', 'scripts/artifacts'),
        help='Directory for screenshots (default: CHAT_INPUT_TOOLTIP_OUTPUT_DIR or scripts/artifacts)',
    )
    return parser.parse_args()


def test_tooltip(page: Page, base_url: str, session_id: str, output_dir: Path):
    # Mock Tauri IPC
    page.add_init_script("""
        window.__TAURI__ = {
            core: {
                invoke: async (cmd, args) => {
                    console.log('Invoke:', cmd, args);
                    if (cmd === 'agent_get_session') {
                        return {
                            id: 'test-session',
                            title: 'Test Session',
                            created_at: new Date().toISOString(),
                            updated_at: new Date().toISOString(),
                        };
                    }
                    if (cmd === 'agent_init_session_with_messages') {
                         return {
                            session: {
                                id: 'test-session',
                                title: 'Test Session',
                                created_at: new Date().toISOString(),
                                updated_at: new Date().toISOString(),
                            },
                            messages: [],
                            total: 0,
                            page: 1,
                            pageSize: 50
                        };
                    }
                     if (cmd === 'messages_get_page') {
                        return {
                            items: [],
                            total: 0,
                            page: 1,
                            pageSize: 50
                        };
                    }
                     if (cmd === 'agent_get_available_tools') {
                        return [];
                    }
                     if (cmd === 'plugin:store|load') {
                        return {};
                     }
                    if (cmd === 'plugin:store|get') {
                        return null;
                    }
                    return null;
                }
            }
        };
        window.__TAURI_INTERNALS__ = {
            invoke: window.__TAURI__.core.invoke,
            transformCallback: (callback) => callback,
        };
    """)

    # Navigate to the agent page
    page.goto(f"{base_url}/agent/{session_id}")

    # Wait for the input area to be visible
    # The textarea has aria-label="Chat input"
    page.wait_for_selector('textarea[aria-label="Chat input"]')

    # Type something to enable the button
    page.fill('textarea[aria-label="Chat input"]', 'Hello')

    # Find the send button
    # It has aria-label="Send message"
    send_button = page.get_by_label("Send message")
    expect(send_button).to_be_visible()
    expect(send_button).to_be_enabled()

    # Hover over the button to trigger tooltip
    send_button.hover()

    # Wait for tooltip content to appear
    # Tooltip content usually has role="tooltip" or we can find by text
    # The text "Send message" should appear in the tooltip
    tooltip = page.get_by_role("tooltip", name="Send message")
    expect(tooltip).to_be_visible()

    # Take a screenshot
    verification_path = output_dir / 'verification.png'
    page.screenshot(path=str(verification_path))
    print(f"Screenshot saved to {verification_path}")

if __name__ == "__main__":
    args = parse_args()
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        try:
            test_tooltip(page, args.base_url, args.session_id, output_dir)
        except Exception as e:
            print(f"Error: {e}")
            error_path = output_dir / 'error.png'
            page.screenshot(path=str(error_path))
        finally:
            browser.close()
