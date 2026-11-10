import { describe, expect, it, vi } from 'vitest';
import { newCharacter, type CharacterDoc } from './character';
import { boxesFromFilter, buildPartyForm, partyFromRealm, PARTY_MAX, type PartyDeps } from './partyrealm';
import { emptyPartyForm, type PartyFilter } from './paste';

const doc = (name: string): CharacterDoc => ({ ...newCharacter(name), class: 5, race: 2, level: 10 });
const who = (f: string) => ({ first_name: f, last_name: 'Fixture' });

function deps(over: Partial<{ damage: number; spell: number }> = {}): PartyDeps {
	const evaluated = {
		calc: { stat_tags: Array.from({ length: 47 }, (_, i) => (i === 2 ? 41.6 : i === 10 ? 250 : 0)) },
		stats: ['', '', '', '17'],
		magic_res: 33,
		dodge: 12,
		anti_magic: true,
		hp: { average: 180, rest_normal: 7, rest_resting: 15 }
	};
	return {
		realm: { input: vi.fn() },
		chars: { evaluate: vi.fn().mockResolvedValue(evaluated) },
		combat: {
			preview: vi.fn().mockResolvedValue({
				error_text: null,
				attack: { damage: over.damage ?? 0, spell_value: over.spell ?? 0, swings: 2.5 }
			})
		},
		paste: {
			party: vi.fn(async (b: { form: unknown }) => ({ form: b.form })),
			partyContinue: vi.fn()
		}
	} as unknown as PartyDeps;
}

describe('party from realm characters', () => {
	it('fills one row per member from the evaluation and the combat preview', async () => {
		const d = deps({ damage: 120 });
		const form = await buildPartyForm([{ summary: who('Cormac'), doc: doc('Cormac') }], d);
		expect(form.name[1]).toBe('Cormac Fixture');
		const row = (k: keyof typeof form) => (form[k] as unknown[])[1];
		expect(row('ac')).toBe('41');
		expect(row('dr')).toBe('17');
		expect(row('mr')).toBe('33');
		expect(row('dodge')).toBe('12');
		expect(row('hitpoints')).toBe('180');
		expect(row('regen_hp')).toBe('7');
		expect(row('rest_hp')).toBe('15');
		expect(row('accy')).toBe('250');
		expect(row('dmg')).toBe('120');
		expect(row('swings')).toBe('2.5');
		expect(row('anti_magic')).toBe(1);
		expect(form.spell_dmg[1]).toBe('');
	});

	it('uses the spell round when there is no weapon damage, and ignores the one-shot figure', async () => {
		const spell = await buildPartyForm([{ summary: who('A'), doc: doc('A') }], deps({ damage: 999999, spell: 80 }));
		expect(spell.dmg[1]).toBe('');
		expect(spell.spell_dmg[1]).toBe('80');
	});

	it('takes at most six members, in the order given', async () => {
		const picks = Array.from({ length: 8 }, (_, i) => ({ summary: who(`M${i}`), doc: doc(`M${i}`) }));
		const form = await buildPartyForm(picks, deps());
		expect(form.name.slice(1)).toEqual(['M0 Fixture', 'M1 Fixture', 'M2 Fixture', 'M3 Fixture', 'M4 Fixture', 'M5 Fixture']);
		expect(PARTY_MAX).toBe(6);
	});

	it('maps what Continue wrote onto the party boxes', () => {
		const f = { party_size: '3', damage: '12', ac: '40', dr: '15', mr: '30', dodge: '10', hitpoints: '200', anti_magic: '1', rest_hp: '14', accy: '240', swings: '2', damage_out_phys: '300', damage_out_spell: null } as PartyFilter;
		const m = boxesFromFilter(f);
		expect(m.damage).toBe('12');
		expect(m.boxes).toEqual({ size: '3', ac: '40', dr: '15', mr: '30', dodge: '10', hp: '200', anti_magic: '1', rest_hp: '14', accuracy: '240', swings: '2', damage_out_physical: '300' });
	});

	it('applies the server filter, or passes on MME\'s note for a party of one', async () => {
		const d = deps();
		(d.paste.partyContinue as ReturnType<typeof vi.fn>).mockResolvedValueOnce({ status: 'info', message: 'Data only updated when party size > 1.' });
		const one = await partyFromRealm([{ summary: who('A'), doc: doc('A') }], d);
		expect(one.boxes).toBeUndefined();
		expect(one.note).toBe('Data only updated when party size > 1.');
		(d.paste.partyContinue as ReturnType<typeof vi.fn>).mockResolvedValueOnce({ status: 'applied', filter: { ...emptyFilter(), party_size: '2', ac: '40' } });
		const two = await partyFromRealm([{ summary: who('A'), doc: doc('A') }, { summary: who('B'), doc: doc('B') }], d);
		expect(two.boxes).toEqual({ size: '2', ac: '40' });
		expect(emptyPartyForm().name).toHaveLength(7);
	});
});

function emptyFilter(): PartyFilter {
	return { party_size: null, damage: null, ac: null, dr: null, mr: null, dodge: null, hitpoints: null, anti_magic: null, rest_hp: null, accy: null, swings: null, damage_out_phys: null, damage_out_spell: null };
}
