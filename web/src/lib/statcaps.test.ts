import { describe, expect, it, vi } from 'vitest';
import { capNote, capOf, createStatCapsClient, parseStat, StatCapsError, type StatCaps } from './statcaps';

const qs: StatCaps = {
	soft_caps: true,
	stats: [
		{ race_max: 100, soft_cap: 100, train_max: 130 },
		{ race_max: 80, soft_cap: 90, train_max: 120 },
		{ race_max: 80, soft_cap: 90, train_max: 120 },
		{ race_max: 80, soft_cap: 90, train_max: 120 },
		{ race_max: 80, soft_cap: 90, train_max: 120 },
		{ race_max: 80, soft_cap: 90, train_max: 120 }
	]
};

describe('stat range', () => {
	it('keeps stock behaviour: three digits, nothing more', () => {
		expect(parseStat('255', undefined)).toBe(255);
		expect(parseStat('1500', undefined)).toBe(999);
		expect(parseStat('', undefined)).toBe(0);
		expect(capOf({ soft_caps: false, stats: [] }, 0)).toBeUndefined();
		expect(capOf(undefined, 0)).toBeUndefined();
		expect(capNote(undefined)).toBe('');
	});

	it('takes values up to the training maximum under Quicksilver', () => {
		expect(parseStat('100', capOf(qs, 1))).toBe(100);
		expect(parseStat('120', capOf(qs, 1))).toBe(120);
		expect(parseStat('121', capOf(qs, 1))).toBe(120);
		expect(parseStat('125', capOf(qs, 0))).toBe(125);
		expect(parseStat('999', capOf(qs, 0))).toBe(130);
		expect(parseStat('abc', capOf(qs, 0))).toBe(0);
	});

	it('shows the soft cap and the maximum', () => {
		expect(capNote(capOf(qs, 1))).toBe('soft cap 90, max 120');
		expect(capNote(capOf(qs, 0))).toBe('soft cap 100, max 130');
	});

	it('reads the caps of a race', async () => {
		const fetchFn = vi.fn(async () => new Response(JSON.stringify(qs)));
		const c = createStatCapsClient(fetchFn as unknown as typeof fetch);
		expect(await c.caps(7)).toEqual(qs);
		expect((fetchFn.mock.calls[0] as unknown as [string])[0]).toBe('/api/races/7/stat-caps');
		const missing = vi.fn(async () => new Response('no race 99', { status: 404 }));
		const err = await createStatCapsClient(missing as unknown as typeof fetch).caps(99).catch((e) => e);
		expect(err).toBeInstanceOf(StatCapsError);
		expect(err.message).toBe('no race 99');
	});
});
