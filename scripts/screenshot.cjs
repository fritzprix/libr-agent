const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1920, height: 1080 },
  });

  try {
    await page.goto('http://localhost:1420', {
      waitUntil: 'networkidle',
      timeout: 30000,
    });
    await new Promise((r) => setTimeout(r, 2000)); // Additional wait for rendering

    await page.screenshot({
      path: 'assets/promo/libragent-homepage.png',
      fullPage: false,
    });
    console.log('Screenshot saved: assets/promo/libragent-homepage.png');
  } catch (error) {
    console.error('Screenshot failed:', error.message);
    // Fallback: screenshot even if error
    await page.screenshot({
      path: 'assets/promo/libragent-homepage-fallback.png',
      fullPage: false,
    });
  } finally {
    await browser.close();
  }
})();
