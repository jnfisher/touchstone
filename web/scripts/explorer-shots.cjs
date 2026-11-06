// Local helper, not run in CI: screenshots of the computed explorer lists with a saved character
// (default "Warrior50"), dark theme, into /tmp/P4-05/shots/. Needs `touchstone serve` and the web
// app (vite dev with its /api proxy, or the embedded build).
//   node web/scripts/explorer-shots.cjs [baseUrl] [characterName]
const { chromium } = require('playwright');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const name = process.argv[3] ?? 'Warrior50';
const out = '/tmp/P4-05/shots';

(async () => {
	const browser = await chromium.launch();
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript(
		([n]) => {
			localStorage.setItem('touchstone.theme', 'dark');
			localStorage.setItem('touchstone.character.last', n);
		},
		[name]
	);
	const page = await ctx.newPage();
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e)));
	for (const tab of ['weapons', 'spells', 'armour']) {
		await page.goto(`${base}/${tab}`);
		await page.getByTestId('use-character').waitFor({ timeout: 20000 });
		await page.waitForSelector('.ag-row');
		await page.getByTestId('toggle-detail').click();
		await page.waitForTimeout(800);
		console.log(tab, (await page.getByTestId('count').innerText()).trim(), (await page.locator('.ag-header-cell-text').allInnerTexts()).join('|'));
		await page.screenshot({ path: `${out}/${tab}.png` });
	}
	await browser.close();
	if (errors.length) {
		console.error(errors.join('\n'));
		process.exit(1);
	}
})();
