// Local helper, not run in CI: dark-theme screenshots of the four calculators against a running
// `touchstone serve`.   node web/scripts/calc-shots.cjs [baseUrl] [outDir]
const { chromium } = require('playwright');
const path = require('node:path');
const fs = require('node:fs');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = process.argv[3] ?? '/tmp/P4-28a/shots';

(async () => {
	fs.mkdirSync(out, { recursive: true });
	const browser = await chromium.launch();
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript(() => localStorage.setItem('touchstone.theme', 'dark'));
	const page = await ctx.newPage();
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e)));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
	const shot = (name) => page.screenshot({ path: path.join(out, `${name}.png`) });
	const pick = async (query, label) => {
		await page.getByTestId('weapon-input').click();
		await page.getByTestId('weapon-input').fill(query);
		await page.getByRole('option', { name: label }).first().click();
	};

	// Reach each calculator through the shell's menu, as a person would.
	await page.goto(`${base}/weapons`);
	await page.getByTestId('calc-menu').click();
	await page.getByTestId('calc-link-exp').click();
	await page.getByTestId('calc-exp').waitFor();
	await page.getByTestId('exp-class').selectOption({ label: 'Mage' });
	await page.getByTestId('exp-race').selectOption({ label: 'Dwarf' });
	await page.getByTestId('exp-to').fill('120');
	await page.getByTestId('exp-rows').locator('tbody tr').nth(100).waitFor();
	console.log('exp table', await page.getByTestId('exp-table-number').inputValue());
	await shot('exp');

	await page.getByTestId('calc-menu').click();
	await page.getByTestId('calc-link-coin').click();
	await page.getByTestId('coin-0').waitFor();
	await page.getByTestId('coin-0-platinum').click();
	await page.getByTestId('coin-0').fill('3.5');
	await page.waitForTimeout(500);
	console.log('coin', await page.getByTestId('coin-1').inputValue());
	await shot('coin');

	await page.getByTestId('calc-menu').click();
	await page.getByTestId('calc-link-swing').click();
	await page.getByTestId('weapon-input').waitFor();
	await pick('starsteel mace', 'starsteel mace (875)');
	await page.getByTestId('swing-combat').selectOption({ label: '4 (Good)' });
	await page.getByTestId('swing-level').fill('90');
	await page.getByTestId('swing-strength').fill('150');
	await page.getByTestId('swing-agility').fill('110');
	await page.getByTestId('swing-encum').fill('2400');
	await page.waitForTimeout(600);
	console.log('swing', await page.getByTestId('swing-energy').innerText(), await page.getByTestId('swing-damage').innerText());
	await shot('swing');

	await page.getByTestId('calc-menu').click();
	await page.getByTestId('calc-link-backstab').click();
	await page.getByTestId('weapon-input').waitFor();
	await pick('starsteel mace', 'starsteel mace (875)');
	await page.getByTestId('bs-level').fill('80');
	await page.getByTestId('bs-stealth').fill('150');
	await page.getByTestId('bs-strength').fill('160');
	await page.getByTestId('bs-max-dmg').fill('12');
	await page.getByTestId('bs-class-stealth').check();
	await page.waitForTimeout(600);
	console.log('backstab', await page.getByTestId('bs-damage').innerText());
	await shot('backstab');

	await browser.close();
	if (errors.length) {
		console.error(errors.join('\n'));
		process.exit(1);
	}
})();
