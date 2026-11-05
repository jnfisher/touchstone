// Local helper, not run in CI: screenshots of the detail panes that show MME's text (a weapon
// with abilities, a spell with damage, a monster with max damage) into scripts/screenshots/,
// which is git-ignored. Needs a running `touchstone serve`.
//   node web/scripts/detail-shots.cjs [baseUrl]
const { chromium } = require('playwright');
const path = require('node:path');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = path.join(__dirname, 'screenshots');

async function pick(page, tab, term) {
	await page.goto(`${base}/${tab}`);
	await page.waitForSelector('.ag-row');
	await page.getByPlaceholder('Search...').fill(term);
	await page.waitForTimeout(500);
	await page.locator('.ag-row').first().locator('.ag-cell').nth(1).click();
	await page.getByTestId('detail').waitFor();
}

(async () => {
	const browser = await chromium.launch();
	const ctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 2 });
	await ctx.addInitScript(() => localStorage.setItem('touchstone.theme', 'dark'));
	const page = await ctx.newPage();
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e)));

	await pick(page, 'weapons', 'hellblade');
	await page.getByText('CastsSp').waitFor();
	console.log('weapon abilities:', (await page.getByTestId('detail').innerText()).split('ABILITIES')[1]?.split('\n').slice(0, 10).join(' | '));
	await page.waitForTimeout(300);
	await page.screenshot({ path: path.join(out, 'detail-weapon.png') });

	await pick(page, 'spells', 'frost jet');
	await page.getByTestId('spell-damage').waitFor();
	await page.getByTestId('spell-level').fill('9');
	await page.getByTestId('spell-damage').filter({ hasText: '-' }).waitFor();
	await page.waitForTimeout(400);
	console.log('spell at level 9:', (await page.getByTestId('spell-numbers').innerText()).replace(/\s+/g, ' '));
	await page.screenshot({ path: path.join(out, 'detail-spell.png') });

	await pick(page, 'monsters', 'Sheriff Lionheart');
	await page.getByText('Max damage / round').waitFor();
	await page.getByText('CastsSp').or(page.getByText('Abilities')).first().waitFor();
	await page.waitForTimeout(300);
	console.log('monster:', (await page.getByText('Max damage / round').locator('xpath=following-sibling::dd[1]').innerText()));
	await page.screenshot({ path: path.join(out, 'detail-monster.png') });

	await browser.close();
	if (errors.length) {
		console.error(errors.join('\n'));
		process.exit(1);
	}
})();
