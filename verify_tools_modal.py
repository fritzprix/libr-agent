import time
from playwright.sync_api import sync_playwright

def verify_tools_modal():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = browser.new_context(viewport={"width": 1280, "height": 720})
        page = context.new_page()

        # Mock Tauri IPC
        page.add_init_script("""
            window.__TAURI__ = {
                core: {
                    invoke: async (cmd, args) => {
                        console.log('Invoke:', cmd, args);
                        if (cmd === 'plugin:store|get') {
                            return null;
                        }
                        if (cmd === 'agent_list_sessions') {
                            return { items: [], total: 0, page: 1, pageSize: 10 };
                        }
                        if (cmd === 'agent_get_session') {
                             return {
                                id: 'session-123',
                                title: 'Test Session',
                                created_at: new Date().toISOString(),
                                updated_at: new Date().toISOString(),
                                agent_id: 'agent-1',
                                model: 'gpt-4o',
                                provider: 'openai',
                            };
                        }
                        if (cmd === 'agent_get_tools') {
                            return [
                                {
                                    name: 'builtin_tool_1',
                                    description: 'A built-in tool',
                                    inputSchema: { type: 'object', properties: { arg: { type: 'string' } } }
                                },
                                {
                                    name: 'mcp_tool_1',
                                    description: 'An MCP tool',
                                    inputSchema: { type: 'object', properties: { arg: { type: 'string' } } }
                                }
                            ];
                        }
                        if (cmd === 'messages_get_page') {
                             return { items: [], total: 0, page: 1, pageSize: 50 };
                        }
                         if (cmd === 'plugin:event|listen') {
                            return 1; // Return a dummy subscription ID
                        }
                        return null;
                    }
                }
            };
            window.__TAURI_INTERNALS__ = {
                invoke: window.__TAURI__.core.invoke,
                transformCallback: (callback) => callback
            };
        """)

        try:
            # Navigate to the agent page directly if possible, or home then click
            # Assuming the route is /agent/:sessionId
            page.goto("http://localhost:1420/agent/session-123")

            # Wait for the page to load
            page.wait_for_load_state("networkidle")

            # The tools modal is usually opened via a button in the UI.
            # I need to find that button. It's likely the wrench icon or similar.
            # Searching for a button with "Tools" or an icon.

            # Let's wait a bit for the session to load
            time.sleep(2)

            # Find the tools button. It might have a tooltip "View Tools" or similar.
            # I'll look for a button with SVG icon Wrench.
            # Or assume it's in the header or input area.

            # Let's take a screenshot of the main page first to see where we are.
            page.screenshot(path="main_page.png")

            # Try to find a button that opens the tools modal.
            # In AgentChatInput or AgentChatHeader?
            # Let's look for a button with Wrench icon.
            # Locate by aria-label if available.

            tools_button = page.locator("button:has(svg.lucide-wrench)")
            if tools_button.count() > 0:
                tools_button.first.click()
                time.sleep(1)
                page.screenshot(path="/home/jules/verification/tools_modal.png")
                print("Clicked tools button and took screenshot.")
            else:
                print("Tools button not found.")

        except Exception as e:
            print(f"Error: {e}")
            page.screenshot(path="/home/jules/verification/error.png")
        finally:
            browser.close()

if __name__ == "__main__":
    verify_tools_modal()
