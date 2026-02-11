from playwright.sync_api import sync_playwright, expect
import time

def verify_tooltips():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()

        # Mock Tauri v2 environment
        page.add_init_script("""
            window.__TAURI__ = {
                core: {
                    invoke: async (cmd, args) => {
                        console.log('Invoke:', cmd, args);
                        if (cmd === 'agent_get_session') {
                            return {
                                id: 'test-session-id',
                                name: 'Test Session',
                                status: 'idle',
                                model: 'gpt-4o',
                                provider: 'openai',
                                createdAt: Date.now(),
                                updatedAt: Date.now(),
                                agentConfig: null
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
                        if (cmd === 'files_list_session') {
                            return [];
                        }
                        if (cmd === 'plugin:event|listen') {
                            return 123; // Subscription ID
                        }
                        if (cmd === 'settings_get_all') {
                            return [];
                        }
                        // Void commands
                        if (['agent_resume_session', 'agent_init_session_with_messages', 'agent_list_sessions'].includes(cmd)) {
                            return null;
                        }

                        return null;
                    }
                }
            };

            // Mock Tauri Internals
            window.__TAURI_INTERNALS__ = {
                invoke: window.__TAURI__.core.invoke,
                transformCallback: (callback) => {
                    return function (response) {
                        return callback(response);
                    };
                },
                convertFileSrc: (path) => path
            };
        """)

        # Navigate to a session page
        try:
            page.goto("http://localhost:1420/agent/test-session-id")
        except Exception as e:
            print(f"Error navigating: {e}")
            return

        # Wait for the chat input to be visible
        try:
            # Increase timeout
            page.wait_for_selector("textarea[aria-label='Chat input']", timeout=20000)
            print("Chat input visible")
        except Exception as e:
            print(f"Chat input not found: {e}")
            page.screenshot(path="verification_fail_2.png")
            return

        # Hover over "Attach files" button (Paperclip)
        try:
            attach_btn = page.locator("button[aria-label='Attach files']")
            attach_btn.hover()
            page.wait_for_timeout(1000) # Wait for tooltip
            page.screenshot(path="verification_attach.png")
            print("Captured attach tooltip")
        except Exception as e:
            print(f"Error hovering attach: {e}")

        # Hover over "Send message" button (Send icon)
        try:
            page.fill("textarea[aria-label='Chat input']", "Hello")

            send_btn = page.locator("button[aria-label='Send message']")
            send_btn.hover()
            page.wait_for_timeout(1000)
            page.screenshot(path="verification_send.png")
            print("Captured send tooltip")
        except Exception as e:
             print(f"Error hovering send: {e}")

        browser.close()

if __name__ == "__main__":
    verify_tooltips()
