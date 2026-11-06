// Local helper, not run in CI: dark-theme screenshots of the calculators against a running
// `touchstone serve`.   node web/scripts/calc-shots.cjs [baseUrl] [outDir] [hit|char]
// With `hit` only the Hit Calculator is shot, in the dark and the light theme. With `char` the Swing
// Calculator is shot using the saved character (the first saved one) and after a MegaMUD paste;
// the server needs at least one saved character with a class, race and level. The other shots type
// their numbers in, so they switch "Use current character" off first.
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

/** The Swing Calculator filled from the current character, then after pasting a MegaMUD stat line. */
async function charShots(browser, errors) {
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
	await ctx.addInitScript(() => localStorage.setItem('touchstone.theme', 'dark'));
	const page = await ctx.newPage();
	page.on('pageerror', (e) => errors.push(String(e)));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
	await page.goto(`${base}/weapons`);
	await page.getByTestId('calc-menu').click();
	await page.getByTestId('calc-link-swing').click();
	await page.getByTestId('calc-swing').waitFor();
	// The switch turns itself on once the saved character has been opened and evaluated.
	await page.waitForFunction(() => document.querySelector('[data-testid="use-character-toggle"]')?.checked === true);
	await page.waitForTimeout(800);
	const val = (id) => page.getByTestId(id).inputValue();
	console.log('note', await page.getByTestId('use-character-note').innerText());
	console.log('swing from character', await Promise.all(['swing-level', 'swing-strength', 'swing-agility', 'swing-encum', 'swing-max-encum'].map(val)));
	console.log('energy', await page.getByTestId('swing-energy').innerText());
	await page.screenshot({ path: path.join(out, 'swing-char.png') });

	const clip = [
		'Hit:   43%   Rng:11-58      Avg:34',
		'Extra: 12%   Rng:2-9        Avg:6',
		'Crit:  5%    Rng:30-80      Avg:55'
	].join('\n');
	await page.getByTestId('swing-mega-text').fill(clip);
	await page.getByTestId('swing-mega-apply').click();
	await page.getByTestId('swing-mega-message').waitFor();
	await page.waitForTimeout(600);
	console.log('mega', await Promise.all(['swing-hit-pct', 'swing-hit-avg', 'swing-extra-pct', 'swing-extra-avg', 'swing-crit-pct', 'swing-crit-avg'].map(val)));
	console.log('true average', await page.getByTestId('swing-true-average').innerText());
	await page.screenshot({ path: path.join(out, 'swing-mega.png') });

	// A rejected paste shows MME's message and changes nothing.
	await page.getByTestId('swing-mega-text').fill('Hit: 43%');
	await page.getByTestId('swing-mega-apply').click();
	await page.waitForTimeout(300);
	console.log('rejected', await page.getByTestId('swing-mega-message').innerText());
	await ctx.close();
}

(async () => {
	fs.mkdirSync(out, { recursive: true });
	const browser = await chromium.launch();
	const errors = [];
	if (only === 'char') {
		await charShots(browser, errors);
		await browser.close();
		if (errors.length) {
			console.error(errors.join('\n'));
			process.exit(1);
		}
		return;
	}
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

	// With a saved character the fields are read-only; these shots type their numbers in.
	const manual = async () => {
		const toggle = page.getByTestId('use-character-toggle');
		await toggle.waitFor();
		await page.waitForTimeout(500);
		if (await toggle.isChecked()) await toggle.uncheck();
		await page.getByTestId('weapon-input').waitFor();
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
	await manual();
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
	await manual();
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
