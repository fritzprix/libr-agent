import os
from playwright.sync_api import sync_playwright

def run(playwright):
    browser = playwright.chromium.launch(headless=True)
    context = browser.new_context()
    page = context.new_page()

    # Mock Tauri IPC to prevent errors and simulate backend responses
    page.add_init_script("""
        window.__TAURI__ = {
            core: {
                invoke: async (cmd, args) => {
                    console.log('Invoke:', cmd, args);
                    if (cmd === 'agent_get_session') {
                        return {
                            id: 'test-session',
                            title: 'Test Session',
                            assistant_id: 'test-assistant',
                            created_at: new Date().toISOString(),
                            updated_at: new Date().toISOString(),
                        };
                    }
                    if (cmd === 'agent_get_sessions') {
                        return [];
                    }
                    if (cmd === 'agent_get_messages') {
                        return [
                            {
                                id: 'msg-1',
                                role: 'user',
                                content: [{ type: 'text', text: 'Hello Agent' }],
                                created_at: new Date().toISOString(),
                            },
                            {
                                id: 'msg-2',
                                role: 'assistant',
                                content: [{ type: 'text', text: 'Hello User, how can I help you?' }],
                                created_at: new Date().toISOString(),
                            }
                        ];
                    }
                    if (cmd === 'get_assistants') {
                        return [
                            {
                                id: 'test-assistant',
                                name: 'Test Agent',
                                description: 'A test agent',
                            }
                        ];
                    }
                    return null;
                }
            }
        };
        window.__TAURI_INTERNALS__ = {
            invoke: window.__TAURI__.core.invoke
        };
    """)

    try:
        page.goto("http://localhost:1420/")

        # Wait for chat to load
        # We look for the message content
        page.wait_for_selector("text=Hello Agent", timeout=5000)
        page.wait_for_selector("text=Hello User", timeout=5000)

        # Take a screenshot
        os.makedirs("verification", exist_ok=True)
        page.screenshot(path="verification/chat_verification.png")
        print("Screenshot taken: verification/chat_verification.png")

    except Exception as e:
        print(f"Error: {e}")
        page.screenshot(path="verification/error.png")
    finally:
        browser.close()

with sync_playwright() as playwright:
    run(playwright)
