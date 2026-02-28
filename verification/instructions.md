# Frontend Verification Script

This script verifies the new "Clear Search" button functionality in the Session History Panel.

## Steps to Verify

1.  **Start the Dev Server**: Ensure the application is running locally.

    ```bash
    pnpm dev &
    ```

    Wait for the server to start (usually on http://localhost:1420).

2.  **Run the Playwright Script**:
    The script will:
    - Navigate to the main page.
    - Locate the Session History search input.
    - Type a search query (e.g., "test").
    - Verify the "Clear search" button (X icon) appears.
    - Take a screenshot of the search input with the clear button.
    - Click the "Clear search" button.
    - Verify the input is cleared and the button disappears.
    - Take a screenshot of the cleared state.

## Playwright Script (`verification/verify_search_clear.py`)

```python
import os
from playwright.sync_api import sync_playwright, expect

def run(playwright):
    browser = playwright.chromium.launch(headless=True)
    page = browser.new_page()

    # Update with your local dev server URL
    page.goto("http://localhost:1420")

    # Wait for the session history panel to load
    # Adjust selector based on your app structure, e.g., heading or specific ID
    page.wait_for_selector("input[placeholder*='Search sessions']")

    search_input = page.get_by_placeholder("Search sessions")

    # 1. Type in search query
    search_input.fill("test query")

    # 2. Verify Clear Button appears
    clear_btn = page.get_by_label("Clear search")
    expect(clear_btn).to_be_visible()

    # Screenshot 1: Search active with clear button
    if not os.path.exists("verification"):
        os.makedirs("verification")
    page.screenshot(path="verification/search_active.png")
    print("Screenshot taken: verification/search_active.png")

    # 3. Click Clear Button
    clear_btn.click()

    # 4. Verify Input is empty and Button is gone
    expect(search_input).to_have_value("")
    expect(clear_btn).not_to_be_visible()

    # Screenshot 2: Search cleared
    page.screenshot(path="verification/search_cleared.png")
    print("Screenshot taken: verification/search_cleared.png")

    browser.close()

with sync_playwright() as playwright:
    run(playwright)
```
