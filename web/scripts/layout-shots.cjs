// Local helper, not run in CI: layout screenshots for the explorer split view into
// scripts/screenshots/ (git-ignored), plus a check that the Shops price cells do not wrap.
//   node web/scripts/layout-shots.cjs [baseUrl]
const { chromium } = require('playwright');
const path = require('node:path');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = path.join(__dirname, 'screenshots');

async function shot(browser, { tab, theme, size, file, search, pick }) {
	const ctx = await browser.newContext({ viewport: size, deviceScaleFactor: 2 });
	await ctx.addInitScript((t) => localStorage.setItem('touchstone.theme', t), theme);
	const page = await ctx.newPage();
	await page.goto(`${base}/${tab}`);
	await page.waitForSelector('.ag-row');
	if (search) await page.getByPlaceholder('Search...').fill(search);
	await page.waitForTimeout(800);
	await page.locator('.ag-row').nth(pick ?? 0).locator('.ag-cell').nth(1).click();
	await page.getByTestId('detail').waitFor();
	await page.waitForTimeout(800);
	const info = await page.evaluate(() => ({
		list: document.querySelector('[data-testid=split-list]')?.getBoundingClientRect().width,
		detail: document.querySelector('[data-testid=split-detail]')?.getBoundingClientRect().width,
		wrapped: [...document.querySelectorAll('[data-testid=price-table] td')].filter((e) => e.getBoundingClientRect().height > 30).length,
		pageScroll: document.documentElement.scrollWidth > innerWidth
	}));
	console.log(file, JSON.stringify(info));
	await page.screenshot({ path: path.join(out, file) });
	await ctx.close();
}

(async () => {
	const browser = await chromium.launch();
	const big = { width: 1600, height: 1000 };
	await shot(browser, { tab: 'shops', theme: 'dark', size: big, file: 'shops-dark.png', pick: 3 });
	await shot(browser, { tab: 'shops', theme: 'light', size: big, file: 'shops-light.png', pick: 3 });
	await shot(browser, { tab: 'monsters', theme: 'dark', size: big, file: 'monsters-dark.png', search: 'dragon' });
	await shot(browser, { tab: 'races', theme: 'dark', size: big, file: 'races-dark.png' });
	await shot(browser, { tab: 'monsters', theme: 'dark', size: { width: 390, height: 844 }, file: 'monsters-mobile.png', search: 'dragon' });
	await browser.close();
})();
