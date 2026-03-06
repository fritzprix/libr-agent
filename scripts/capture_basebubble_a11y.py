import asyncio
import argparse
import os
from playwright.async_api import async_playwright

async def run(base_url, output_path):
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        page = await browser.new_page()

        print(f"Navigating to {base_url}...")
        try:
            await page.goto(base_url)
            await page.wait_for_timeout(3000)
            
            # Ensure output directory exists
            os.makedirs(os.path.dirname(output_path), exist_ok=True)
            
            print(f"Capturing screenshot to {output_path}...")
            await page.screenshot(path=output_path)
        except Exception as e:
            print(f"Error during capture: {e}")
        finally:
            await browser.close()

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Capture BaseBubble accessibility states.")
    parser.add_argument("--base-url", default="http://localhost:1420", help="Base URL of the running app")
    parser.add_argument("--output", default="scripts/artifacts/basebubble_a11y.png", help="Output path for the screenshot")
    
    args = parser.parse_args()
    asyncio.run(run(args.base_url, args.output))
