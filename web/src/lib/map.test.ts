import { describe, expect, it, vi } from 'vitest';
import { render } from 'svelte/server';
import MapGrid from './components/map/MapGrid.svelte';
import MapLegend from './components/map/MapLegend.svelte';
import hub from './map-fixtures/m-hub-both.json';
import town from './map-fixtures/m-town.json';
import legend from './map-fixtures/legend.json';
import {
	DEFAULT_MAP_OPTIONS,
	cellNumber,
	createMapClient,
	directionForKey,
	loadPrefs,
	mapQuery,
	optionsFromList,
	optionsList,
	parsePopout,
	popoutUrl,
	presetPage,
	savePrefs,
	withPreset,
	type LegendEntry,
	type MapResponse,
	type Preset
} from './map';

// Responses of GET /api/map for fixtures/rules/map.json cases m-hub-both (9001/1) and m-town (1/224), main grid,
// without the oracle-only `draws` list.
const hubGrid = hub as unknown as MapResponse;
const townGrid = town as unknown as MapResponse;

const html = (grid: MapResponse, props: object = {}) =>
	render(MapGrid, { props: { grid, size: 'main', ...props } }).body;
const count = (s: string, needle: string) => s.split(needle).length - 1;

describe('MapGrid', () => {
	it('draws a rect per placed cell and a line per segment', () => {
		const out = html(townGrid);
		expect(count(out, 'data-testid="map-cell"')).toBe(townGrid.cells.length);
		expect(count(out, 'data-testid="map-segment"')).toBe(townGrid.segments.length);
		expect(out).toContain(`viewBox="0 0 ${townGrid.cols * 10} ${townGrid.rows * 10}"`);
	});

	it('colours by theme token, never by a literal colour', () => {
		const out = html(hubGrid);
		expect(out).toContain('var(--map-cell-updown)');
		expect(out).toContain('var(--map-cell-black)');
		expect(out).toContain('var(--map-line-general)');
		expect(out).not.toMatch(/#[0-9a-f]{3,6}\b|rgb\(/i);
	});

	it('marks the start room and the markers the cell carries', () => {
		const out = html(hubGrid);
		expect(count(out, 'data-testid="map-start"')).toBe(1);
		for (const m of ['command', 'npc', 'lair', 'missing', 'overwritten']) expect(out).toContain(`var(--map-mark-${m})`);
	});

	it('outlines every block only when asked', () => {
		expect(html(hubGrid)).not.toContain('map-block');
		expect(count(html(hubGrid, { showBlocks: true }), 'data-testid="map-block"')).toBe(hubGrid.cols * hubGrid.rows);
	});
});

describe('MapLegend', () => {
	it('lists the entries with the same tokens', () => {
		const out = render(MapLegend, { props: { entries: legend as LegendEntry[], onclose: () => {} } }).body;
		expect(count(out, 'data-testid="legend-')).toBe(legend.length);
		expect(out).toContain('var(--map-line-keyitemtoll)');
		expect(out).toContain('Class / Race / Level / Alignment / Ability');
	});
});

describe('options and queries', () => {
	it('round-trip through the server list', () => {
		const o = { ...DEFAULT_MAP_OPTIONS, follow: true, noLairs: true, overwrite: true, alsoMark: 'shops' as const };
		expect(optionsList(o)).toBe('follow,no_lairs,overwrite,mark_shops');
		expect(optionsFromList(optionsList(o))).toEqual(o);
		expect(optionsFromList('bogus,no_tips').noTips).toBe(true);
	});

	it('change the /api/map query', () => {
		const at = { map: 1, room: 224 };
		expect(mapQuery(at, 'main', DEFAULT_MAP_OPTIONS)).toBe('map=1&room=224&size=main');
		expect(mapQuery(at, '50x50', { ...DEFAULT_MAP_OPTIONS, noNpcs: true, alsoMark: 'spells' }, 1275)).toBe(
			'map=1&room=224&size=50x50&options=no_npcs%2Cmark_spells&center=1275'
		);
	});

	it('number cells as MME does', () => {
		expect(cellNumber('main', 14, 11)).toBe(345);
		expect(cellNumber('50x50', 24, 25)).toBe(1275);
		expect(cellNumber('zoom21', 0, 0)).toBe(9001);
	});

	it('carry the room and options to the pop-out and back', () => {
		const o = { ...DEFAULT_MAP_OPTIONS, dupes: true, noTips: true };
		const url = popoutUrl({ map: 1, room: 224 }, '50x50', o);
		expect(url).toBe('/map?map=1&room=224&size=50x50&options=dupes%2Cno_tips');
		const p = parsePopout(url.slice(4));
		expect(p.at).toEqual({ map: 1, room: 224 });
		expect(p.size).toBe('50x50');
		expect(p.options).toEqual(o);
		expect(parsePopout('?map=x').at).toBeUndefined();
	});

	it('remember per browser, surviving bad storage', () => {
		const data = new Map<string, string>();
		const store = { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
		savePrefs('window', { size: '30x30', options: { ...DEFAULT_MAP_OPTIONS, follow: true } }, store);
		expect(loadPrefs('window', store).size).toBe('30x30');
		expect(loadPrefs('window', store).options.follow).toBe(true);
		expect(loadPrefs('tab', store).size).toBe('main');
		data.set('touchstone.map.tab', '{nope');
		expect(loadPrefs('tab', store).size).toBe('main');
		const throwing = { getItem: () => { throw new Error('blocked'); }, setItem: () => { throw new Error('blocked'); } };
		expect(loadPrefs('tab', throwing).size).toBe('main');
		expect(() => savePrefs('tab', loadPrefs('tab', throwing), throwing)).not.toThrow();
	});
});

describe('keys', () => {
	const k = (key: string) => directionForKey({ key, ctrlKey: false, altKey: false, metaKey: false });
	it('map the number pad as frmMap does', () => {
		expect([k('8'), k('2'), k('4'), k('6'), k('7'), k('9'), k('1'), k('3'), k('0'), k('.')]).toEqual([
			'N', 'S', 'W', 'E', 'NW', 'NE', 'SW', 'SE', 'U', 'D'
		]);
		expect(k('ArrowUp')).toBe('N');
		expect(k('x')).toBeUndefined();
		expect(directionForKey({ key: 'ArrowUp', ctrlKey: true, altKey: false, metaKey: false })).toBeUndefined();
	});
});

describe('client', () => {
	const reply = (body: unknown, status = 200) => Promise.resolve(new Response(JSON.stringify(body), { status }));

	it('walking calls the go endpoint, with the window rules in the pop-out', async () => {
		const f = vi.fn((..._a: unknown[]) => reply({ Go: { map: 9001, room: 2 } }));
		const c = createMapClient(f as unknown as typeof fetch);
		expect(await c.go({ map: 9001, room: 1 }, 'N', false)).toEqual({ Go: { map: 9001, room: 2 } });
		expect(f.mock.calls[0][0]).toBe('/api/map/go?map=9001&room=1&dir=N');
		await c.go({ map: 9001, room: 1 }, 'U', true);
		expect(f.mock.calls[1][0]).toBe('/api/map/go?map=9001&room=1&dir=U&window=true');
	});

	it('draws with the options in the query and reports a refusal', async () => {
		const f = vi.fn((url: string, ..._a: unknown[]) => (url.includes('room=9999') ? reply('Room not found', 404) : reply(hub)));
		const c = createMapClient(f as unknown as typeof fetch);
		const g = await c.map({ map: 9001, room: 1 }, '20x20', { ...DEFAULT_MAP_OPTIONS, follow: true });
		expect(g.cells.length).toBe(hubGrid.cells.length);
		expect(f.mock.calls[0][0]).toBe('/api/map?map=9001&room=1&size=20x20&options=follow');
		await expect(c.map({ map: 9001, room: 9999 }, 'main', DEFAULT_MAP_OPTIONS)).rejects.toThrow('Room not found');
	});
});

describe('presets', () => {
	const all: Preset[] = Array.from({ length: 50 }, (_, index) => ({ index, name: `p${index}`, map: 1, room: 1 }));
	it('page ten to a page', () => {
		expect(presetPage(all, 2).map((p) => p.index)).toEqual([20, 21, 22, 23, 24, 25, 26, 27, 28, 29]);
	});

	it('an edit replaces its entry and keeps the others, as typed (Val on the numbers)', () => {
		const saved = [{ index: 4, name: 'Old', map: 2, room: 3 }];
		const next = withPreset(saved, 1, { name: 'Home', map: ' 12abc', room: '' });
		expect(next).toEqual([
			{ index: 1, name: 'Home', map: 12, room: 0 },
			{ index: 4, name: 'Old', map: 2, room: 3 }
		]);
		expect(withPreset(next, 1, { name: 'Again', map: '1', room: '2' })[0]).toEqual({ index: 1, name: 'Again', map: 1, room: 2 });
	});

	it('round-trip through the settings endpoint', async () => {
		const { createSettingsClient, defaultSettings } = await import('./settings');
		let stored = defaultSettings();
		const f = vi.fn(async (_u: string, init?: RequestInit) => {
			if (init?.method === 'PUT') stored = JSON.parse(String(init.body));
			return new Response(JSON.stringify(stored));
		});
		const client = createSettingsClient(f as unknown as typeof fetch);
		const edited = withPreset(stored.map_presets, 7, { name: 'Camp', map: '3', room: '9' });
		const back = await client.put({ ...stored, map_presets: edited });
		expect(back.map_presets).toEqual([{ index: 7, name: 'Camp', map: 3, room: 9 }]);
	});
});
