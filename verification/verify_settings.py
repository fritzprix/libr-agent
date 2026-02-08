import time
from playwright.sync_api import sync_playwright, expect

def run(playwright):
    browser = playwright.chromium.launch(headless=True)
    page = browser.new_page()

    # Mock Tauri IPC
    page.add_init_script("""
        window.__TAURI__ = {
            core: {
                invoke: async (cmd, args) => {
                    console.log('Invoke:', cmd, args);
                    if (cmd === 'list_settings') {
                        return [
                            {
                                key: 'systemSettings',
                                value: {
                                    maxFileUploadSizeMB: 50,
                                    workspaceCapacityMB: 10,
                                    webActionTimeoutSeconds: 30,
                                    mcpServerStartupTimeoutSeconds: 60,
                                    mcpToolTimeoutSeconds: 60,
                                    searchIndexFrequencyMinutes: 5,
                                    activeSessionRetentionHours: 24,
                                    shellIsolationLevel: 'medium',
                                    skillsDirectory: '',
                                    httpServerPort: 3030
                                },
                                createdAt: 0,
                                updatedAt: 0
                            }
                        ];
                    }
                    if (cmd === 'get_setting') {
                         return null;
                    }
                    return null;
                }
            }
        };
        window.__TAURI_INTERNALS__ = {
            invoke: async () => {},
            transformCallback: (x) => x
        };
    """)

    try:
        # Navigate directly to settings
        page.goto("http://localhost:1420/settings")

        # Wait for the tabs to appear
        expect(page.get_by_role("tablist")).to_be_visible(timeout=10000)

        # Click on "Advanced" tab
        page.get_by_role("tab", name="Advanced").click()

        # Wait for the content to render
        time.sleep(1)

        # Scroll to the "HTTP Server Port" label
        port_label = page.get_by_text("HTTP Server Port")
        port_label.scroll_into_view_if_needed()
        expect(port_label).to_be_visible()

        # Take screenshot
        page.screenshot(path="verification/advanced_settings_port.png")
        print("Screenshot saved to verification/advanced_settings_port.png")

    except Exception as e:
        print(f"Error: {e}")
        page.screenshot(path="verification/error.png")
        print("Error screenshot saved to verification/error.png")

    browser.close()

if __name__ == "__main__":
    with sync_playwright() as p:
        run(p)
