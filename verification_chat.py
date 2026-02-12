import time
from playwright.sync_api import sync_playwright

def run():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = browser.new_context()
        page = context.new_page()

        # Mock Tauri API
        page.add_init_script("""
            window.__TAURI__ = {
                core: {
                    invoke: async (cmd, args) => {
                        console.log('Invoke:', cmd, args);
                        if (cmd === 'get_default_skills_directory') return '/tmp/skills';
                        if (cmd === 'scan_skills_directory') return [];
                        if (cmd === 'settings_get_all') return { theme: 'system', language: 'en' };
                        if (cmd === 'mcp_get_servers') return [];
                        if (cmd === 'agent_get_all_sessions') return [];
                        if (cmd === 'ai_get_models') return [];
                        if (cmd === 'skills_get_all') return [];
                        if (cmd === 'agent_get_session') {
                            return {
                                id: 'session-123',
                                name: 'Test Session',
                                status: 'idle',
                                model: 'gpt-4',
                                provider: 'openai',
                                createdAt: Date.now(),
                                updatedAt: Date.now()
                            };
                        }
                        if (cmd === 'messages_get_page') {
                            return {
                                items: [
                                    {
                                        id: 'msg_1',
                                        sessionId: 'session-123',
                                        role: 'user',
                                        content: [{type: 'text', text: 'Hello, world!'}],
                                        createdAt: Date.now(),
                                        updatedAt: Date.now()
                                    },
                                    {
                                        id: 'msg_2',
                                        sessionId: 'session-123',
                                        role: 'assistant',
                                        content: [{type: 'text', text: 'Hi! This is a test message.'}],
                                        createdAt: Date.now(),
                                        updatedAt: Date.now()
                                    }
                                ],
                                total: 2,
                                page: 1,
                                pageSize: 1000
                            };
                        }
                        if (cmd === 'agent_get_service_contexts') return {};
                        if (cmd === 'agent_resume_session') return {};
                        if (cmd === 'agent_init_session_with_messages') return {};
                        if (cmd === 'agent_get_available_tools') return [];
                        return null;
                    }
                },
                event: {
                    listen: async (event, handler) => {
                        console.log('Listen:', event);
                        // Mock subscription
                        return () => {};
                    }
                }
            };

            window.__TAURI_INTERNALS__ = {
                invoke: window.__TAURI__.core.invoke,
                transformCallback: (cb) => cb
            };
        """)

        # Navigate to session
        print("Navigating to session...")
        page.goto("http://localhost:1420/agent/session-123")

        # Wait for content
        try:
            page.wait_for_selector("text=Hello, world!", timeout=10000)
            print("Found user message")
            page.wait_for_selector("text=Hi! This is a test message.", timeout=10000)
            print("Found assistant message")

            # Take screenshot
            page.screenshot(path="verification_chat.png")
            print("Screenshot taken: verification_chat.png")
        except Exception as e:
            print(f"Error: {e}")
            page.screenshot(path="error_screenshot.png")

        browser.close()

if __name__ == "__main__":
    run()
