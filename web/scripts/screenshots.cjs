// Local helper: two screenshots (Monsters dark, Weapons light) into scripts/screenshots/, which is
// git-ignored. Needs a running `touchstone serve`.
//   node web/scripts/screenshots.cjs [baseUrl]
const { chromium } = require('playwright');
const path = require('node:path');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = path.join(__dirname, 'screenshots');

async function shot(browser, tab, theme, file, pick) {
	const ctx = await browser.newContext({ viewport: { width: 1600, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript((t) => localStorage.setItem('touchstone.theme', t), theme);
	const page = await ctx.newPage();
	await page.goto(`${base}/${tab}`);
	await page.waitForSelector('.ag-row');
	await page.getByPlaceholder('Search...').fill(pick);
	await page.waitForTimeout(300);
	await page.locator('.ag-row').first().locator('.ag-cell').nth(1).click();
	await page.getByTestId('detail').waitFor();
	await page.waitForTimeout(500);
	await page.screenshot({ path: path.join(out, file) });
	await ctx.close();
}

(async () => {
	const browser = await chromium.launch();
	await shot(browser, 'monsters', 'dark', 'monsters-dark.png', 'dragon');
	await shot(browser, 'weapons', 'light', 'weapons-light.png', 'sword');
	await browser.close();
})();
