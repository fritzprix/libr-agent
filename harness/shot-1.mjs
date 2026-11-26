import { chromium } from 'playwright-core';

const EXEC =
  '/home/fritzprix/.cache/ms-playwright/chromium_headless_shell-1208/chrome-headless-shell-linux64/chrome-headless-shell';

const states = ['inprogress', 'cancelled', 'timedOut', 'providerError', 'success'];
const outDir = process.argv[2];

const browser = await chromium.launch({
  executablePath: EXEC,
  args: ['--no-sandbox', '--disable-gpu'],
});
const page = await browser.newPage({ viewport: { width: 440, height: 400 } });

for (const state of states) {
  await page.goto(`http://localhost:5199/?state=${state}`, { waitUntil: 'load' });
  await page.waitForFunction('window.__shotReady === true', null, { timeout: 15000 });
  await page.waitForTimeout(200);
  const card = await page.$('#card-wrap');
  await (card ?? page).screenshot({ path: `${outDir}/oauth-${state}.png` });
  console.log(`captured ${state}`);
}

await browser.close();
