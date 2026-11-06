// Local helper, not run in CI: screenshots the header at 1280 and 1440 px against a running `touchstone serve` and
// fails when the tab strip scrolls (a clipped tab) or the page scrolls sideways.
//   node web/scripts/header-shots.cjs <baseUrl> <outDir>
const { chromium } = require('playwright');
const path = require('node:path');
const assert = require('node:assert');

const base = process.argv[2] ?? 'http://127.0.0.1:8370';
const out = process.argv[3] ?? '/tmp/F-002/shots';

(async () => {
	const browser = await chromium.launch();
	for (const w of [1280, 1440]) {
		const ctx = await browser.newContext({ viewport: { width: w, height: 800 } });
		await ctx.addInitScript(() => localStorage.setItem('touchstone.theme', 'dark'));
		const page = await ctx.newPage();
		await page.goto(`${base}/monsters`);
		await page.getByTestId('profile-badge').waitFor();
		await page.waitForTimeout(500);
		const m = await page.evaluate(() => {
			const strip = document.querySelector('[data-testid=tabs] .overflow-x-auto');
			return { scrollW: strip.scrollWidth, clientW: strip.clientWidth, docW: document.documentElement.scrollWidth };
		});
		assert.ok(m.scrollW <= m.clientW, `${w}px: the tab strip scrolls (${m.scrollW} > ${m.clientW})`);
		assert.ok(m.docW <= w, `${w}px: the page scrolls sideways`);
		await page.screenshot({ path: path.join(out, `header-${w}.png`), clip: { x: 0, y: 0, width: w, height: 120 } });
		await ctx.close();
	}
	await browser.close();
})();
