import { describe, expect, it } from 'vitest';
import { isSection, keyForSection, keyLines, SECTIONS, sectionForKey } from './sections';

describe('sections', () => {
	it('maps F1..F11 to the eleven MME tabs; Chests has no key', () => {
		expect(SECTIONS).toHaveLength(12);
		expect(SECTIONS.map((s) => s.slug).indexOf('chests')).toBe(SECTIONS.map((s) => s.slug).indexOf('items') + 1);
		expect(keyForSection('chests')).toBeUndefined();
		expect(keyForSection('rooms')).toBe('F9');
		expect(sectionForKey('F6')).toBe('items');
		expect(sectionForKey('F9')).toBe('rooms');
		expect(sectionForKey('F1')).toBe('weapons');
		expect(sectionForKey('F11')).toBe('compare');
		expect(sectionForKey('F12')).toBeUndefined();
		expect(sectionForKey('a')).toBeUndefined();
	});
	it('recognises slugs', () => {
		expect(isSection('rooms')).toBe(true);
		expect(isSection('nope')).toBe(false);
	});
	it('lists the keys as bound, with the MME key of each tab', () => {
		const lines = keyLines();
		expect(lines).toHaveLength(SECTIONS.length - 1);
		expect(lines.find((l) => l.label === 'Chests')).toBeUndefined();
		// every listed key opens the tab it is listed for
		for (const l of lines) expect(SECTIONS.find((s) => s.label === l.label)?.slug).toBe(sectionForKey(l.key));
		// the F9 / F12 confusion: Rooms is F9 here and F12 in MME
		expect(lines.find((l) => l.label === 'Rooms')).toEqual({ key: 'F9', label: 'Rooms', mme: 'F12' });
		expect(lines.find((l) => l.key === 'F12')).toBeUndefined();
		expect(lines.find((l) => l.label === 'Weapons')).toEqual({ key: 'F1', label: 'Weapons', mme: 'F5' });
	});
});
