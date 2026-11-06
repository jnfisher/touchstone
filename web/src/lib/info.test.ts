import { describe, expect, it } from 'vitest';
import type { Info } from './api';
import { infoLines, isQuicksilver, profileBadge, profileBadgeTitle, profileLines, versionLabel } from './info';

const base: Info = {
	counts: { rooms: 12000, items: 3000 },
	data_version: 1.11,
	load_count: 1,
	loaded_at: '',
	source: { kind: 'mdb', path: '/data/x.mdb' },
	version: { custom: '\0', dat_file_version: 'v1.11l', date: '1/2/2020', nmr_version: '1.83', time: '10:00' }
};

describe('data version info', () => {
	it('capitalises v1.11l and adds the custom label as LoadInfo does', () => {
		expect(versionLabel(base.version)).toBe('v1.11L');
		expect(versionLabel({ ...base.version, custom: 'Mine', dat_file_version: 'v1.12' })).toBe('Mine - v1.12');
	});
	it('lists version, creation date, NMR version, source and counts', () => {
		const l = infoLines(base);
		expect(l.slice(0, 4).map((x) => x.label)).toEqual(['Database version', 'Created', 'NMR version', 'Source']);
		expect(l.find((x) => x.label === 'rooms')?.value).toBe('12,000');
	});
});

const qs: Info = {
	...base,
	profile: { name: 'quicksilver', setting: 'auto', content_version: '1.20.0', balance_overrides: ['crit_knee', 'scalecap'] }
};
const stock: Info = { ...base, profile: { name: 'stock', setting: 'auto', content_version: null, balance_overrides: [] } };

describe('rules profile', () => {
	it('badges Quicksilver with its content version and names the overrides in the tooltip', () => {
		expect(isQuicksilver(qs)).toBe(true);
		// short in the header, in full in the tooltip
		expect(profileBadge(qs)).toBe('QS 1.20.0');
		expect(profileBadgeTitle(qs)).toContain('Quicksilver 1.20.0 rules');
		expect(profileBadgeTitle(qs)).toContain('crit_knee, scalecap');
		expect(profileBadgeTitle({ ...qs, profile: { ...qs.profile!, balance_overrides: [] } })).toContain('as shipped');
	});
	it('badges a stock database as Stock', () => {
		expect(isQuicksilver(stock)).toBe(false);
		expect(profileBadge(stock)).toBe('Stock');
		expect(profileBadgeTitle(stock)).toBe('Stock MajorMUD rules');
	});
	it('treats a server with no profile as stock and shows no section', () => {
		expect(isQuicksilver(base)).toBe(false);
		expect(profileBadge(base)).toBe('Stock');
		expect(profileLines(base)).toEqual([]);
		expect(isQuicksilver(undefined)).toBe(false);
	});
	it('lists name, setting, content version and overrides', () => {
		expect(profileLines(qs)).toEqual([
			{ label: 'Profile', value: 'Quicksilver' },
			{ label: 'Setting', value: 'auto' },
			{ label: 'Content version', value: '1.20.0' },
			{ label: 'Balance overrides', value: 'crit_knee, scalecap' }
		]);
		expect(profileLines(stock).map((l) => l.value)).toEqual(['Stock', 'auto', 'none', 'none']);
	});
});
