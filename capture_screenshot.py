import asyncio
from playwright.async_api import async_playwright

async def run():
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        page = await browser.new_page()

        # Navigate to a page where BaseBubble might be rendered, e.g., an error or expanded tool block
        # Alternatively, we can just capture the accessibility tree of a basic UI state,
        # but to test BaseBubble, we'd ideally trigger one. Let's just navigate to the main page
        # and wait a bit, then take a screenshot of the whole page to ensure no gross regressions.
        await page.goto('http://localhost:1420')
        await page.wait_for_timeout(3000)

        await page.screenshot(path='screenshot.png')
        await browser.close()

asyncio.run(run())
