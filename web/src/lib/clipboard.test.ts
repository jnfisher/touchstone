import { describe, expect, it, vi } from 'vitest';
import { createClipboardClient, deliver, writeClipboard } from './clipboard';

describe('writeClipboard', () => {
	it('uses navigator.clipboard', async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		expect(await writeClipboard('x', { clipboard: { writeText } })).toBe(true);
		expect(writeText).toHaveBeenCalledWith('x');
	});
	it('reports false without the API (no secure context) or when it refuses', async () => {
		expect(await writeClipboard('x', {})).toBe(false);
		expect(await writeClipboard('x', undefined)).toBe(false);
		const writeText = vi.fn().mockRejectedValue(new Error('denied'));
		expect(await writeClipboard('x', { clipboard: { writeText } })).toBe(false);
	});
});

describe('deliver', () => {
	it('copies the text', async () => {
		const write = vi.fn().mockResolvedValue(true);
		expect(await deliver({ text: 'a\r\nb', error: null }, write)).toEqual({ copied: true, error: undefined });
		expect(write).toHaveBeenCalledWith('a\r\nb');
	});
	it('hands the text to the fallback dialog when the browser refuses', async () => {
		const out = await deliver({ text: 'a', error: null }, async () => false);
		expect(out.copied).toBe(false);
		expect(out.fallback).toBe('a');
	});
	it('says so when MME copies nothing, and passes on its error', async () => {
		expect((await deliver({ text: null, error: null })).empty).toBe(true);
		const e = await deliver({ text: null, error: 'Invalid procedure call or argument (run-time error 5)' });
		expect(e.empty).toBe(false);
		expect(e.error).toContain('error 5');
	});
	it('keeps the partial text of a failed second step', async () => {
		const out = await deliver({ text: 'stats', error: 'boom' }, async () => true);
		expect(out).toEqual({ copied: true, error: 'boom' });
	});
});

describe('client', () => {
	it('posts to /api/clipboard/<kind>', async () => {
		const fetchFn = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ text: 'x', error: null }) });
		const c = createClipboardClient(fetchFn as unknown as typeof fetch);
		await c.shop(5, { nameOnly: true });
		const [url, init] = fetchFn.mock.calls[0];
		expect(url).toBe('/api/clipboard/shop');
		expect(JSON.parse(init.body)).toEqual({ number: 5, charm: 0, show_sell: false, name_only: true });
	});
	it('throws the server reason', async () => {
		const fetchFn = vi.fn().mockResolvedValue({ ok: false, status: 400, text: async () => 'no shop 9' });
		const c = createClipboardClient(fetchFn as unknown as typeof fetch);
		await expect(c.monster([], false)).rejects.toThrow('no shop 9');
	});
});
