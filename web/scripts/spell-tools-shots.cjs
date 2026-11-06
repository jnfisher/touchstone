// Local helper, not run in CI: dark-theme screenshots of the Spell Book and the "What casts this spell?"
// dialog against a running `touchstone serve`, into /tmp/P4-32/shots/.
//   node web/scripts/spell-tools-shots.cjs [baseUrl]
const { chromium } = require('playwright');

const base = process.argv[2] ?? 'http://127.0.0.1:8391';
const out = '/tmp/P4-32/shots';

(async () => {
	const browser = await chromium.launch();
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript(() => localStorage.setItem('touchstone.theme', 'dark'));
	const page = await ctx.newPage();
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e)));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));

	// the Spell Book: a mage at level 40
	await page.goto(`${base}/spells`);
	await page.waitForSelector('.ag-row');
	await page.getByTestId('tools-menu').click();
	await page.getByTestId('tools-spell-book').click();
	await page.getByTestId('spell-book').waitFor();
	await page.getByTestId('book-class').selectOption({ label: 'Mage' });
	await page.getByTestId('book-level').fill('40');
	await page.getByTestId('book-list').click();
	await page.locator('[data-testid="spell-book"] .ag-row').first().waitFor();
	await page.waitForTimeout(800);
	console.log('book', (await page.getByTestId('book-count').innerText()).trim(),
		await page.getByTestId('book-magery').inputValue(), await page.getByTestId('book-magery-level').inputValue());
	await page.screenshot({ path: `${out}/spell-book.png` });
	await page.getByTestId('book-close').click();

	// What casts this spell?
	await page.getByPlaceholder('Search...').fill('serpent scourge-hits');
	await page.waitForTimeout(500);
	const row = page.locator('.ag-row').first();
	await row.click();
	await row.click({ button: 'right' });
	await page.getByTestId('menu-whatCasts').click();
	await page.getByTestId('results-dialog').waitFor();
	console.log('casts', await page.getByTestId('results-title').innerText(),
		(await page.getByTestId('results-row').allInnerTexts()).join(' | '));
	await page.screenshot({ path: `${out}/what-casts.png` });
	await page.getByTestId('results-close').click();

	console.log('errors', errors.length ? errors : 'none');
	await browser.close();
})();
