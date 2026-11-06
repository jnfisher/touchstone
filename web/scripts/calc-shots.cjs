// Local helper, not run in CI: dark-theme screenshots of the calculators against a running
// `touchstone serve`.   node web/scripts/calc-shots.cjs [baseUrl] [outDir] [hit]
// With `hit` only the Hit Calculator is shot, in the dark and the light theme.
const { chromium } = require('playwright');
const path = require('node:path');
const fs = require('node:fs');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = process.argv[3] ?? '/tmp/P4-27a/shots';
const only = process.argv[4];

/** A realistic Hit Calculator case: a thief's backstab against an adult red dragon. */
async function hitShots(browser, theme, errors) {
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript((t) => localStorage.setItem('touchstone.theme', t), theme);
	const page = await ctx.newPage();
	page.on('pageerror', (e) => errors.push(String(e)));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
	await page.goto(`${base}/weapons`);
	await page.getByTestId('calc-menu').click();
	await page.getByTestId('calc-link-hit').click();
	await page.getByTestId('calc-hit').waitFor();
	await page.getByTestId('hit-stats').waitFor();
	// Pick a real monster through the picker, as a person would.
	await page.getByTestId('hit-monster').click();
	await page.getByTestId('hit-monster').fill('adult red dragon');
	await page.getByRole('option', { name: 'adult red dragon (185)' }).first().click();
	await page.getByTestId('hit-char-class').selectOption({ label: 'Thief' });
	await page.getByTestId('hit-char-stealth').fill('180');
	await page.getByTestId('hit-char-bs-accuracy').fill('40');
	await page.getByTestId('hit-char-agility').fill('90');
	await page.getByTestId('hit-char-accuracy').fill('210');
	await page.getByTestId('hit-type-backstab').check();
	await page.getByTestId('hit-defender-monster').check();
	await page.getByTestId('hit-see-hidden').waitFor();
	await page.waitForTimeout(700);
	console.log(theme, 'hit', await page.getByTestId('hit-stats').innerText().then((t) => t.replace(/\s+/g, ' ')));
	console.log(theme, 'boxes', await Promise.all([0, 1, 2, 3, 5].map((i) => page.getByTestId(`hit-box-${i}`).inputValue())));
	await page.getByTestId('calc-hit').evaluate((el) => el.scrollTo(0, 0));
	await page.screenshot({ path: path.join(out, theme === 'dark' ? 'hit.png' : 'hit-light.png') });
	await ctx.close();
}

(async () => {
	fs.mkdirSync(out, { recursive: true });
	const browser = await chromium.launch();
	const errors = [];
	if (only === 'hit') {
		await hitShots(browser, 'dark', errors);
		await hitShots(browser, 'light', errors);
		await browser.close();
		if (errors.length) {
			console.error(errors.join('\n'));
			process.exit(1);
		}
		return;
	}
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript(() => localStorage.setItem('touchstone.theme', 'dark'));
	const page = await ctx.newPage();
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
	await hitShots(browser, 'dark', errors);
	await hitShots(browser, 'light', errors);

	await browser.close();
	if (errors.length) {
		console.error(errors.join('\n'));
		process.exit(1);
	}
})();
