import { chromium } from 'playwright-core';

const EXEC =
  '/home/fritzprix/.cache/ms-playwright/chromium_headless_shell-1208/chrome-headless-shell-linux64/chrome-headless-shell';

const expectations = {
  inprogress: { button: 'oauth-cancel', notice: null },
  cancelled: {
    button: 'oauth-authorize',
    notice: 'Authorization cancelled.',
  },
  timedOut: {
    button: 'oauth-authorize',
    notice: 'Authorization timed out. Try Authorize again.',
  },
  providerError: {
    button: 'oauth-authorize',
    notice:
      'Authorization failed: OAuth provider error (access_denied): The user cancelled the request',
  },
  success: { button: 'oauth-disconnect', notice: null },
};

const browser = await chromium.launch({
  executablePath: EXEC,
  args: ['--no-sandbox', '--disable-gpu'],
});
const page = await browser.newPage();
let fail = 0;
for (const [state, exp] of Object.entries(expectations)) {
  await page.goto(`http://localhost:5199/?state=${state}`, { waitUntil: 'load' });
  await page.waitForFunction('window.__shotReady === true', null, { timeout: 15000 });
  const btn = await page.$(`[data-testid="${exp.button}"]`);
  const noticeEl = await page.$('[data-testid="oauth-result"]');
  const noticeText = noticeEl ? (await noticeEl.innerText()).trim() : null;
  const okBtn = btn !== null;
  const okNotice =
    exp.notice === null ? noticeEl === null : noticeText === exp.notice;
  if (!okBtn || !okNotice) fail++;
  console.log(
    `${state}: button=${exp.button} ${okBtn ? 'OK' : 'MISSING'} | notice=${JSON.stringify(
      noticeText,
    )} ${okNotice ? 'OK' : 'WRONG (expected ' + JSON.stringify(exp.notice) + ')'}`,
  );
}
await browser.close();
process.exit(fail === 0 ? 0 : 1);
