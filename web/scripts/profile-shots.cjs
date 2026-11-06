// Local helper, not run in CI: dark screenshots of the rules-profile badge, the info popover and the
// Monsters pane against a running `touchstone serve`.
//   node web/scripts/profile-shots.cjs <baseUrl> <outDir> <prefix: qs|stock>
const { chromium } = require('playwright');
const path = require('node:path');
const assert = require('node:assert');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = process.argv[3] ?? '/tmp/Q-015/shots';
const prefix = process.argv[4] ?? 'qs';
// a saved level-50 Warrior (PUT /api/characters/Warrior50) so the Monsters pane has exp/hr to show
const character = process.argv[5] ?? 'Warrior50';

(async () => {
	const browser = await chromium.launch();
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript((c) => { localStorage.setItem('touchstone.theme', 'dark'); localStorage.setItem('touchstone.character.last', c); }, character);
	const page = await ctx.newPage();
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e)));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));

	await page.goto(`${base}/monsters`);
	const badge = page.getByTestId('profile-badge');
	await badge.waitFor();
	console.log('badge:', (await badge.innerText()).trim());
	await page.waitForSelector('.ag-row');
	await page.getByTestId('info-button').click();
	await page.getByTestId('info-popover').waitFor();
	await page.getByTestId('profile-lines').waitFor();
	await page.waitForTimeout(300);
	await page.screenshot({ path: path.join(out, `${prefix}-badge.png`) });
	await page.keyboard.press('Escape');

	if (prefix === 'qs') {
		await page.getByTestId('monsters-quicksilver-models').waitFor();
		assert.match(await page.getByTestId('monsters-quicksilver-models').innerText(), /only Model D/);
		await page.waitForSelector('.ag-row');
		await page.getByTestId('use-character').waitFor({ timeout: 20000 }).catch(() => {});
		await page.locator('.ag-row').nth(10).click();
		await page.waitForTimeout(1500);
		await page.screenshot({ path: path.join(out, `${prefix}-monster.png`) });
	}
	assert.deepEqual(errors, []);
	await browser.close();
})();
