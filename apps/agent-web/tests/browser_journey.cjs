const fs = require('node:fs');
const { chromium } = require(process.env.KEITH_PLAYWRIGHT_MODULE);

const origin = process.env.KEITH_WEB_ORIGIN;
const control = process.env.KEITH_UPSTREAM_CONTROL;
const commandLog = process.env.KEITH_COMMAND_LOG;
const credentialSecret = 'browser-credential-must-stay-write-only';
let lastCommandBody = '';

async function waitForLog(fragment) {
  const deadline = Date.now() + 5000;
  let contents = '';
  while (Date.now() < deadline) {
    contents = fs.existsSync(commandLog) ? fs.readFileSync(commandLog, 'utf8') : '';
    if (contents.includes(fragment)) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`command was not observed: ${fragment}; log=${contents}`);
}

async function applySurface(page, route, value, expected) {
  await page.locator(`[data-route="${route}"]`).click();
  const panel = page.locator(`[data-panel="${route}"]`);
  if (await panel.locator('form').getAttribute('data-rust-bound') !== 'true') {
    throw new Error(`Rust form handler was not bound for ${route}`);
  }
  await panel.locator('textarea[name="value"]').fill(value);
  await panel.locator('button[type="button"]').click();
  await waitForLog(expected);
}

let browser;
(async () => {
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ reducedMotion: 'reduce' });
  const page = await context.newPage();
  page.on('console', (message) => console.error(`browser console: ${message.type()} ${message.text()}`));
  page.on('pageerror', (error) => console.error(`browser page error: ${error.stack || error}`));
  page.on('requestfailed', (request) => console.error(`browser request failed: ${request.url()} ${request.failure()?.errorText}`));
  page.on('request', (request) => {
    if (request.url().includes('/commands')) lastCommandBody = request.postData() || '';
  });
  page.on('response', async (response) => {
    if (response.url().includes('/api/') && response.status() >= 400) {
      console.error(`browser API response: ${response.status()} ${response.url()} ${await response.text()}`);
    }
  });
  await page.goto(origin, { waitUntil: 'domcontentloaded' });
  await page.locator('#password').fill('browser-login-secret');
  await Promise.all([
    page.waitForURL(`${origin}/`),
    page.getByRole('button', { name: 'Sign in' }).click(),
  ]);
  await page.getByText('Connected', { exact: true }).waitFor({ timeout: 10000 });
  await page.getByText('streamed response', { exact: true }).waitFor({ timeout: 10000 });

  const conversation = await page.locator('#conversation').elementHandle();
  for (const route of [
    'sessions', 'goals', 'plans', 'children', 'tools', 'memory', 'knowledge',
    'schedules', 'commitments', 'channels', 'settings', 'artifacts', 'refinement',
  ]) {
    await page.locator(`[data-route="${route}"]`).click();
    await page.locator(`[data-panel="${route}"]`).waitFor({ state: 'visible' });
    const stillMounted = await page.locator('#conversation').evaluate((node, original) => node === original, conversation);
    if (!stillMounted) throw new Error(`conversation remounted while opening ${route}`);
  }

  await applySurface(page, 'memory', 'browser memory edit', 'Update durable memory');
  const csrf = await page.locator("meta[name='keith-csrf']").getAttribute('content');
  const activeProfile = await page.locator('#app').getAttribute('data-profile');
  const wrongCsrf = await page.evaluate(async ({ profile, body }) => {
    const response = await fetch(`/api/profiles/${profile}/commands`, {
      method: 'POST', headers: { 'content-type': 'application/json', 'x-keith-csrf': 'wrong' }, body,
    });
    return response.status;
  }, { profile: activeProfile, body: lastCommandBody });
  if (wrongCsrf !== 403) throw new Error(`wrong CSRF returned ${wrongCsrf}`);
  const otherProfile = '01ARZ3NDEKTSV4RRFFQ69G5FAV';
  const crossProfile = await page.evaluate(async ({ profile, body, csrf }) => {
    const response = await fetch(`/api/profiles/${profile}/commands`, {
      method: 'POST', headers: { 'content-type': 'application/json', 'x-keith-csrf': csrf }, body,
    });
    return response.status;
  }, { profile: otherProfile, body: lastCommandBody, csrf });
  if (crossProfile !== 403) throw new Error(`cross-profile command returned ${crossProfile}`);
  const hostileOrigin = await page.request.post(`${origin}/api/profiles/${activeProfile}/commands`, {
    headers: { origin: 'http://hostile.invalid', 'x-keith-csrf': csrf, 'content-type': 'application/json' },
    data: '{}',
  });
  if (hostileOrigin.status() !== 403) throw new Error(`hostile origin returned ${hostileOrigin.status()}`);
  await applySurface(page, 'schedules', 'browser schedule', 'create_schedule');
  await applySurface(page, 'channels', 'browser channel message', 'configured-channel');
  await applySurface(page, 'refinement', 'browser refinement request', 'guarded refinement');

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.getByText('Connected', { exact: true }).waitFor({ timeout: 10000 });
  await page.getByText('streamed response', { exact: true }).waitFor({ timeout: 10000 });

  fs.writeFileSync(control, 'down');
  await page.getByText('Disconnected; reconnecting', { exact: true }).waitFor({ timeout: 10000 });
  fs.writeFileSync(control, 'up');
  await page.getByText('Connected', { exact: true }).waitFor({ timeout: 15000 });

  await page.locator('[data-route="settings"]').click();
  await page.locator('#provider').fill('browser-provider');
  await page.locator('#credential-name').fill('browser-reference');
  await page.locator('#credential-secret').fill(credentialSecret);
  await Promise.all([
    page.waitForURL(`${origin}/?credential=configured`),
    page.locator('#save-credential').click(),
  ]);
  if (page.url().includes(credentialSecret)) throw new Error('credential entered the URL');
  const storage = await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length }));
  if (storage.local !== 0 || storage.session !== 0) throw new Error('browser storage was used');
  const bundle = await (await page.request.get(`${origin}/assets/agent_web.js`)).text();
  if (bundle.includes(credentialSecret)) throw new Error('credential entered the bundle');

  await browser.close();
  browser = undefined;
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
}).finally(async () => {
  if (browser) await browser.close();
});
