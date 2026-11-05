import { describe, expect, it } from 'vitest';
import { isSection, SECTIONS, sectionForKey } from './sections';

describe('sections', () => {
	it('maps F1..F11 to the eleven tabs', () => {
		expect(SECTIONS).toHaveLength(11);
		expect(sectionForKey('F1')).toBe('weapons');
		expect(sectionForKey('F11')).toBe('compare');
		expect(sectionForKey('F12')).toBeUndefined();
		expect(sectionForKey('a')).toBeUndefined();
	});
	it('recognises slugs', () => {
		expect(isSection('rooms')).toBe(true);
		expect(isSection('nope')).toBe(false);
	});
});
