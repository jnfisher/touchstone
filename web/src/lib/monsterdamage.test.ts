import { describe, expect, it, vi } from 'vitest';
import { newCharacter } from './character';
import { DEFAULT_PARTY } from './monsters';
import {
	calcBody,
	canCalculate,
	createDamageClient,
	DamageError,
	emptyStatus,
	progressText,
	type DamageStatus
} from './monsterdamage';

const json = (body: unknown, status = 200) =>
	Promise.resolve(new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } }));

describe('monster damage requests', () => {
	it('needs a class and a race, as the sims need the defenses', () => {
		expect(canCalculate(undefined)).toBe(false);
		expect(canCalculate(newCharacter('x'))).toBe(false);
		expect(canCalculate({ ...newCharacter('x'), class: 1, race: 2 })).toBe(true);
	});

	it('sends the character, the mode and the party boxes', async () => {
		const fetchFn = vi.fn((_url: string, _init?: RequestInit) => json({ mode: 'party', monsters: 3, skipped: 0, elapsed_ms: 5 }));
		const doc = { ...newCharacter('x'), class: 1, race: 2 };
		const done = await createDamageClient(fetchFn as unknown as typeof fetch).calculate(calcBody(doc, 'party', DEFAULT_PARTY));
		expect(done.monsters).toBe(3);
		const [url, init] = fetchFn.mock.calls[0];
		expect(url).toBe('/api/monster-damage/calculate');
		expect(init?.method).toBe('POST');
		expect(JSON.parse(String(init?.body))).toEqual({ character: doc, mode: 'party', party: DEFAULT_PARTY });
	});

	it('clears both arrays unless a mode is named, dismisses with the prompt key and cancels', async () => {
		const fetchFn = vi.fn((_url: string, _init?: RequestInit) => json(emptyStatus()));
		const c = createDamageClient(fetchFn as unknown as typeof fetch);
		await c.clear();
		await c.clear('char');
		await c.dismiss({ mode: 'char', key: '71_50' });
		await c.cancel();
		const bodies = fetchFn.mock.calls.map(([u, i]) => [u, JSON.parse(String(i?.body))]);
		expect(bodies).toEqual([
			['/api/monster-damage/clear', {}],
			['/api/monster-damage/clear', { mode: 'char' }],
			['/api/monster-damage/dismiss', { mode: 'char', key: '71_50' }],
			['/api/monster-damage/cancel', {}]
		]);
	});

	it('reads the status without caching and reports a refusal with the server text', async () => {
		const status: DamageStatus = { ...emptyStatus(), char: { calculated: true }, generation: 4 };
		const fetchFn = vi.fn((_url: string, _init?: RequestInit) => json(status));
		expect(await createDamageClient(fetchFn as unknown as typeof fetch).status()).toEqual(status);
		expect(fetchFn.mock.calls[0][1]).toMatchObject({ cache: 'no-store' });
		const busy = vi.fn(() => Promise.resolve(new Response('a monster damage calculation is already running', { status: 409 })));
		await expect(
			createDamageClient(busy as unknown as typeof fetch).calculate(calcBody(newCharacter('x'), 'char', DEFAULT_PARTY))
		).rejects.toMatchObject({ status: 409, message: 'a monster damage calculation is already running' });
		expect(new DamageError(500, 'x').name).toBe('DamageError');
	});

	it('words the progress like the progress form', () => {
		expect(progressText({ mode: 'char', done: 1234, total: 5678, elapsed_ms: 4500 })).toBe(
			'Calculating monster damage vs character... 1,234 of 5,678 (4s)'
		);
		expect(progressText({ mode: 'party', done: 0, total: 9, elapsed_ms: 0 })).toContain('vs party');
	});
});

describe('the cancelling progress line', () => {
	it('says it is cancelling while the server stops', async () => {
		const { progressText } = await import('./monsterdamage');
		const r = { mode: 'char' as const, done: 4, total: 1200, elapsed_ms: 1000 };
		expect(progressText(r)).toMatch(/^Calculating monster damage vs character\.\.\. 4 of 1,200/);
		expect(progressText({ ...r, cancelling: true })).toBe('Cancelling the monster damage calculation... 4 of 1,200');
	});
});
