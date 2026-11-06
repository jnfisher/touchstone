import { describe, expect, it } from 'vitest';
import type { Info } from './api';
import { infoLines, versionLabel } from './info';

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
