// Local smoke test, not run in CI: loads every explorer tab against a running `touchstone serve`
// and checks the grid is populated, a record selects, and the arrow keys move the selection.
//   node web/scripts/smoke.cjs [baseUrl]   (default http://127.0.0.1:8370)
const { chromium } = require('playwright');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const tabs = ['weapons', 'armour', 'spells', 'monsters', 'shops', 'items', 'races', 'classes', 'rooms'];

async function main() {
	const browser = await chromium.launch();
	const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e)));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));

	// Reaching the page by tab key (F1..F9) also checks the shortcuts.
	await page.goto(`${base}/weapons`);
	let failed = false;
	for (const [i, tab] of tabs.entries()) {
		if (i > 0) await page.keyboard.press(`F${i + 1}`);
		await page.waitForURL(`**/${tab}`);
		await page.waitForSelector('.ag-row', { timeout: 20000 });
		const rendered = await page.locator('.ag-row').count();
		const count = (await page.getByTestId('count').innerText()).trim();
		const total = Number(count.split(' of ')[1].replace(/,/g, ''));
		const headers = await page.locator('.ag-header-cell-text').allInnerTexts();

		await page.locator('.ag-row').first().locator('.ag-cell').nth(1).click();
		await page.getByTestId('detail').waitFor();
		const first = await page.getByTestId('detail').locator('h3').first().innerText();
		await page.keyboard.press('ArrowDown');
		await page.waitForFunction((t) => document.querySelector('[data-testid=detail] h3')?.textContent !== t, first);
		const second = await page.getByTestId('detail').locator('h3').first().innerText();

		const ok = rendered > 0 && total > 0 && first !== second;
		if (!ok) failed = true;
		console.log(`${ok ? 'ok  ' : 'FAIL'} ${tab.padEnd(8)} ${count.padEnd(16)} rendered=${rendered} "${first}" -> "${second}" [${headers.join(' | ')}]`);
	}
	// Selection is kept per tab.
	await page.keyboard.press('F1');
	await page.waitForURL('**/weapons');
	await page.waitForSelector('.ag-row-selected');
	console.log('ok   weapons selection restored');

	if (errors.length) {
		failed = true;
		console.log('console errors:', errors);
	}
	await browser.close();
	process.exit(failed ? 1 : 0);
}
main().catch((e) => {
	console.error(e);
	process.exit(1);
});
