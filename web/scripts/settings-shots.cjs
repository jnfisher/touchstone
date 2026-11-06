// Local helper, not run in CI: dark screenshot of the Settings dialog against a running
// `touchstone serve`, after a round trip (edit, Save, reopen) that checks the server clamps.
//   node web/scripts/settings-shots.cjs [baseUrl] [outDir]
const { chromium } = require('playwright');
const path = require('node:path');
const assert = require('node:assert');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = process.argv[3] ?? '/tmp/P4-29/shots';

(async () => {
	const browser = await chromium.launch();
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript(() => localStorage.setItem('touchstone.theme', 'dark'));
	const page = await ctx.newPage();
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e)));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
	await page.goto(`${base}/weapons`);
	await page.getByTestId('settings-button').click();
	await page.getByTestId('settings-dialog').waitFor();

	// Save an out-of-range set; the server answers with the clamped values.
	await page.getByTestId('settings-knob_dmg').fill('12');
	await page.getByTestId('settings-knob_mana').fill('0.5');
	await page.getByTestId('settings-sim-rounds').fill('20');
	await page.getByTestId('settings-model_d').uncheck();
	await page.getByTestId('settings-show-all').check();
	assert.match(await page.getByTestId('settings-knob_dmg-note').innerText(), /saved as 10/);
	assert.match(await page.getByTestId('settings-sim-rounds-note').innerText(), /saved as 100/);
	await page.getByTestId('settings-save').click();
	await page.getByTestId('settings-dialog').waitFor({ state: 'detached' });

	await page.getByTestId('settings-button').click();
	await page.getByTestId('settings-dialog').waitFor();
	await page.waitForTimeout(300);
	assert.equal(await page.getByTestId('settings-knob_dmg').inputValue(), '10');
	assert.equal(await page.getByTestId('settings-sim-rounds').inputValue(), '100');
	assert.equal(await page.getByTestId('settings-model_d').isChecked(), false);
	assert.equal(await page.getByTestId('settings-show-all').isChecked(), true);

	// The shot shows the live notes: type new out-of-range values, a model help open.
	await page.getByTestId('settings-knob_dmg').fill('12.345');
	await page.getByTestId('settings-sim-rounds').fill('20');
	await page.getByTestId('settings-waypoint').fill('0');
	await page.getByRole('button', { name: 'About Model D' }).click();
	await page.waitForTimeout(300);
	await page.screenshot({ path: path.join(out, 'settings.png') });

	// Put the server back to defaults.
	await page.getByTestId('settings-reset').click();
	await page.getByTestId('settings-sim-rounds').fill('500');
	await page.getByTestId('settings-waypoint').fill('25');
	await page.getByTestId('settings-show-all').uncheck();
	await page.getByTestId('settings-save').click();
	await page.getByTestId('settings-dialog').waitFor({ state: 'detached' });

	assert.deepEqual(errors, []);
	await browser.close();
})();
