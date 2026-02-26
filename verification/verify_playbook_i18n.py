from playwright.sync_api import Page, expect, sync_playwright
import time

def verify_playbook_i18n(page: Page):
    print("Navigating to Playbooks page...")
    # 1. Go to Playbooks page
    page.goto("http://localhost:1420/playbooks")

    print("Waiting for Playbooks heading...")
    # 2. Wait for the page to load (title should be visible even if data fails)
    # The title is h1 with text "Playbooks" (from t('playbook.title'))
    expect(page.get_by_role("heading", name="Playbooks")).to_be_visible(timeout=10000)

    print("Checking description...")
    # 3. Check description
    # "Browse and execute automated workflows"
    expect(page.get_by_text("Browse and execute automated workflows")).to_be_visible()

    print("Checking search placeholder...")
    # 4. Check search placeholder
    # "Search playbooks..."
    search_input = page.get_by_placeholder("Search playbooks...")
    expect(search_input).to_be_visible()

    print("Checking Display button...")
    # 5. Check "Display" button in SortControls
    expect(page.get_by_role("button", name="Display")).to_be_visible()

    print("Taking screenshot...")
    # 6. Take screenshot
    page.screenshot(path="verification/playbook_i18n.png")
    print("Screenshot saved.")

if __name__ == "__main__":
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        try:
            verify_playbook_i18n(page)
        except Exception as e:
            print(f"Verification failed: {e}")
            page.screenshot(path="verification/error.png")
            raise e
        finally:
            browser.close()
