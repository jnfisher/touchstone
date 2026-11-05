// Local check, not run in CI: for every explorer tab at 1600 px, reports truncated headers or
// cells (text wider than its box), then exercises find-as-you-type and Ctrl+C, and saves a
// screenshot per tab into scripts/screenshots/ (git-ignored). Needs a running `touchstone serve`.
//   node web/scripts/grid-check.cjs [baseUrl]
const { chromium } = require('playwright');
const path = require('node:path');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = path.join(__dirname, 'screenshots');
const tabs = ['weapons', 'armour', 'spells', 'monsters', 'shops', 'items', 'races', 'classes', 'rooms'];

(async () => {
	const browser = await chromium.launch();
	const ctx = await browser.newContext({ viewport: { width: 1600, height: 900 }, deviceScaleFactor: 2 });
	await ctx.grantPermissions(['clipboard-read', 'clipboard-write']);
	const page = await ctx.newPage();
	let bad = 0;
	for (const tab of tabs) {
		await page.goto(`${base}/${tab}`);
		await page.waitForSelector('.ag-row');
		await page.waitForTimeout(600);
		const cut = await page.evaluate(() => {
			const over = (el) => el.scrollWidth > el.clientWidth + 1;
			const heads = [...document.querySelectorAll('.ag-header-cell-text')].filter(over).map((e) => e.textContent);
			const cells = [...document.querySelectorAll('.ag-center-cols-container .ag-cell')]
				.filter(over)
				.map((e) => e.textContent);
			return { heads, cells: cells.slice(0, 5) };
		});
		if (cut.heads.length || cut.cells.length) bad++;
		console.log(tab, 'truncated', JSON.stringify(cut));
		await page.screenshot({ path: path.join(out, `${tab}.png`) });
	}
	// Find + copy on weapons.
	await page.goto(`${base}/weapons`);
	await page.waitForSelector('.ag-row');
	await page.locator('.ag-row').first().locator('.ag-cell').nth(1).click();
	await page.keyboard.type('bro');
	await page.waitForTimeout(200);
	const sel = async () => page.locator('.ag-row-selected .ag-cell').nth(1).innerText();
	const a = await sel();
	await page.keyboard.press('ArrowRight');
	await page.waitForTimeout(200);
	const b = await sel();
	console.log('find:', a, '->', b);
	await page.keyboard.press('Control+c');
	await page.waitForTimeout(200);
	console.log('copy:', JSON.stringify((await page.evaluate(() => navigator.clipboard.readText())).slice(0, 160)));
	// Resize persistence.
	const h = page.locator('.ag-header-cell').nth(1);
	const box = await h.boundingBox();
	await page.mouse.move(box.x + box.width - 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width + 60, box.y + box.height / 2, { steps: 5 });
	await page.mouse.up();
	await page.reload();
	await page.waitForSelector('.ag-row');
	await page.waitForTimeout(500);
	const after = (await page.locator('.ag-header-cell').nth(1).boundingBox()).width;
	console.log('width before/after resize+reload', Math.round(box.width), Math.round(after));
	await browser.close();
	process.exit(bad ? 1 : 0);
})();
