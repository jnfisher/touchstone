import { describe, expect, it } from 'vitest';
import { boxText, pairFor, spellbookNote } from './builder';

describe('builder helpers', () => {
	it('pairs the selected row with the picked one', () => {
		expect(pairFor('race', 4, 9)).toEqual({ race: 4, class: 9 });
		expect(pairFor('class', 9, 4)).toEqual({ race: 4, class: 9 });
		expect(pairFor('class', 9, undefined)).toEqual({ race: 0, class: 9 });
	});

	it('words the Spellbook button like MME', () => {
		const none = { class: null, level: null, magery: null, magery_level: null };
		expect(spellbookNote({ action: 'nothing', ...none }, 'Bard')).toBe('');
		expect(spellbookNote({ action: 'no_magic', ...none }, 'Warrior')).toBe('No Magic');
		expect(
			spellbookNote({ action: 'open', class: 9, level: 999, magery: 4, magery_level: 2 }, 'Bard')
		).toBe('Spell book for Bard: level 999, Bard magery, magery level 2');
	});

	it('shows MME line breaks as plain lines', () => {
		expect(boxText('a\r\nb')).toBe('a\nb');
		expect(boxText(null)).toBe('');
	});
});
