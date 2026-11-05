// Local helper, not run in CI: screenshots of the first computed values (Shops tab with prices,
// Races tab with the exp table) into scripts/screenshots/, which is git-ignored. Needs a running
// `touchstone serve`.   node web/scripts/computed-shots.cjs [baseUrl]
const { chromium } = require('playwright');
const path = require('node:path');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = path.join(__dirname, 'screenshots');

(async () => {
	const browser = await chromium.launch();
	const ctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 2 });
	await ctx.addInitScript(() => localStorage.setItem('touchstone.theme', 'dark'));
	const page = await ctx.newPage();
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e)));

	await page.goto(`${base}/shops`);
	await page.waitForSelector('.ag-row');
	await page.getByPlaceholder('Search...').fill('General Store');
	await page.waitForTimeout(300);
	await page.locator('.ag-row').first().locator('.ag-cell').nth(1).click();
	await page.getByTestId('price-table').waitFor();
	await page.getByTestId('charm').fill('100');
	await page.getByTestId('charm-note').filter({ hasText: 'Discount' }).waitFor();
	console.log('shops note:', await page.getByTestId('charm-note').innerText());
	await page.waitForTimeout(300);
	await page.screenshot({ path: path.join(out, 'shops-prices.png') });

	await page.goto(`${base}/races`);
	await page.waitForSelector('.ag-row');
	await page.getByPlaceholder('Search...').fill('Dwarf');
	await page.waitForTimeout(300);
	await page.locator('.ag-row').first().locator('.ag-cell').nth(1).click();
	await page.getByTestId('exp-table').locator('tbody tr').nth(27).waitFor();
	await page.getByTestId('exp-other').selectOption({ label: 'Mage' });
	await page.waitForTimeout(600);
	await page.getByTestId('exp-table').scrollIntoViewIfNeeded();
	console.log('level 28:', (await page.getByTestId('exp-table').locator('tbody tr').nth(27).innerText()).replace(/\s+/g, ' '));
	await page.screenshot({ path: path.join(out, 'races-exp-table.png') });
	await browser.close();
	if (errors.length) {
		console.error(errors.join('\n'));
		process.exit(1);
	}
})();
