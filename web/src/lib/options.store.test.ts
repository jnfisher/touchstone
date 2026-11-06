import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReachabilityStatus } from './api';
import { newCharacter, type CharacterClient } from './character';
import { createCharacterStore } from './character.svelte';
import { createCompareStore } from './compare.svelte';
import { filterReset } from './explorer-state.svelte';
import { createGlobalFilterStore } from './globalfilter.svelte';
import { createMonsterFilterStore } from './monsters.svelte';
import { createDamageStore } from './monsterdamage.svelte';
import { emptyStatus, type DamageClient } from './monsterdamage';
import { createOptionsStore } from './options.svelte';
import { createPasteStore } from './paste.svelte';
import { createSettingsStore } from './settings.svelte';
import { defaultSettings, type Settings, type SettingsClient } from './settings';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const reached = (computed: boolean): ReachabilityStatus => ({
	computed,
	rooms: 1,
	edges: 1,
	random_groups: 0,
	level_bands: 1,
	monsters: 1,
	located: 1
});

function setup(answers: Record<string, string | undefined> = {}) {
	const charClient = {
		evaluate: vi.fn().mockResolvedValue({}),
		blessSpells: vi.fn().mockResolvedValue([]),
		list: vi.fn().mockResolvedValue([]),
		get: vi.fn(),
		put: vi.fn(async (_n: string, d: unknown) => d),
		remove: vi.fn(),
		importMmec: vi.fn()
	};
	const chars = createCharacterStore(charClient as unknown as CharacterClient);
	chars.set({ ...newCharacter('Arden'), class: 1, race: 2, learned_spells: [3, 4] });
	const cmp = createCompareStore(chars, { classify: vi.fn() } as never);
	const status = emptyStatus();
	const dmgClient = {
		status: vi.fn(async () => structuredClone(status)),
		calculate: vi.fn(async () => ({ mode: 'char', monsters: 3, skipped: 0, elapsed_ms: 1 })),
		clear: vi.fn(async () => structuredClone(status)),
		dismiss: vi.fn()
	};
	const damage = createDamageStore(dmgClient as unknown as DamageClient);
	const boxes = createMonsterFilterStore(undefined);
	const bar = createGlobalFilterStore(undefined);
	let server: Settings = defaultSettings();
	const settingsClient = {
		get: vi.fn(async () => structuredClone(server)),
		put: vi.fn(async (s: Settings) => ((server = structuredClone(s)), structuredClone(s)))
	};
	const settings = createSettingsStore(settingsClient as unknown as SettingsClient);
	const paste = createPasteStore();
	const asked: string[] = [];
	const ask = vi.fn(async (title: string, text: string) => {
		asked.push(`${title}: ${text}`);
		return answers[text];
	});
	let reachOk = true;
	const reachability = {
		reachabilityStatus: vi.fn(async () => reached(false)),
		computeReachability: vi.fn(async () => reached(reachOk))
	};
	const opts = createOptionsStore({ chars, cmp, damage, boxes, bar, settings, paste, ask, reachability });
	return {
		opts, chars, cmp, damage, boxes, bar, settings, paste, asked, ask, dmgClient, settingsClient, reachability,
		failReach: () => (reachOk = false)
	};
}

describe('Options menu actions', () => {
	it('Paste Character opens the paste dialog', () => {
		const x = setup();
		x.opts.pasteCharacter();
		expect(x.opts.pasteOpen).toBe(true);
	});

	it('Calculate Reachability computes, tells the Monsters list to ask again, and says when it could not', async () => {
		const x = setup();
		const before = x.boxes.reachRev;
		await x.opts.calculateReachability();
		expect(x.reachability.computeReachability).toHaveBeenCalledOnce();
		expect(x.boxes.reachRev).toBe(before + 1);
		expect(x.opts.reach?.computed).toBe(true);
		expect(x.opts.notice).toBeUndefined();
		x.failReach();
		await x.opts.calculateReachability();
		// mnuOptionsItems_Click case 1: the MsgBox when ReachIsComputed is false
		expect(x.opts.notice).toBe('Reachability could not be computed for this database.');
	});

	it('Calc. Monster Dmg vs Char / Party and Clear go through the damage store', async () => {
		const x = setup();
		expect(x.opts.canCalcDamage).toBe(true);
		expect(await x.opts.calcDamage('char')).toBe(true);
		expect(x.dmgClient.calculate).toHaveBeenCalledOnce();
		const v = x.damage.version;
		await x.opts.clearDamage();
		expect(x.dmgClient.clear).toHaveBeenCalledOnce();
		expect(x.damage.version).toBe(v + 1);
		x.chars.edit((d) => (d.race = null));
		expect(x.opts.canCalcDamage).toBe(false);
	});

	it('Clear Learned Spells asks first (default No) and empties the learned spells', async () => {
		const no = setup({ 'Are you sure?': 'no' });
		expect(await no.opts.clearLearnedSpells()).toBe(false);
		expect(no.chars.doc.learned_spells).toEqual([3, 4]);
		expect(no.asked[0]).toBe('Reset Learned Spells: Are you sure?');

		const yes = setup({ 'Are you sure?': 'yes' });
		expect(await yes.opts.clearLearnedSpells()).toBe(true);
		expect(yes.chars.doc.learned_spells).toEqual([]);
	});

	it('Reset All Character Fields blanks the character and the Item Manager, and keeps the lists on No', async () => {
		const listQ = 'Clear saved weapon, armour, spell, and monsters lists?  The item manager is cleared regardless.';
		const x = setup({ 'Are you sure?': 'yes', [listQ]: 'no' });
		x.cmp.toggle('weapons', 5);
		x.paste.setRows([{ number: 1 } as never]);
		x.bar.set({ level: 40 });
		await x.opts.resetAllCharacterFields();
		expect(x.chars.doc.name).toBe('');
		expect(x.chars.doc.class).toBeNull();
		expect(x.chars.doc.learned_spells).toEqual([]);
		expect(x.paste.rows).toEqual([]);
		expect(x.bar.filter.level).toBe(999);
		expect(x.cmp.lists.weapons).toEqual([5]);
		expect(x.asked).toHaveLength(2);

		const y = setup({ 'Are you sure?': 'yes', [listQ]: 'yes' });
		y.cmp.toggle('weapons', 5);
		await y.opts.resetAllCharacterFields();
		expect(y.cmp.lists.weapons).toEqual([]);

		// Cancel on the lists question stops
		const z = setup({ 'Are you sure?': 'yes', [listQ]: undefined });
		z.cmp.toggle('spells', 9);
		expect(await z.opts.resetAllCharacterFields()).toBe(false);
		expect(z.chars.doc.name).toBe('Arden');
	});

	it('Reset All Character Fields does not ask about lists when there are none, and stops on No', async () => {
		const x = setup({ 'Are you sure?': 'yes' });
		await x.opts.resetAllCharacterFields();
		expect(x.asked).toHaveLength(1);
		const n = setup({ 'Are you sure?': 'no' });
		expect(await n.opts.resetAllCharacterFields()).toBe(false);
		expect(n.chars.doc.name).toBe('Arden');
	});

	it('Remove All Filters resets the bar, the monster boxes and asks the explorers to clear theirs', () => {
		const x = setup();
		x.bar.set({ level: 12, class: 3 });
		x.boxes.set({ magic: true });
		const epoch = filterReset.epoch;
		x.opts.removeAllFilters();
		expect(x.bar.filter.level).toBe(999);
		expect(x.bar.filter.class).toBe(0);
		expect(x.boxes.controls.magic).toBe(false);
		expect(filterReset.epoch).toBe(epoch + 1);
	});

	it('Settings opens the settings dialog', () => {
		const x = setup();
		x.opts.openSettings();
		expect(x.opts.settingsOpen).toBe(true);
	});

	it('the two checked items persist through the settings API and keep the other settings', async () => {
		const x = setup();
		await x.settings.load();
		expect(x.opts.jumpToCompare).toBe(false);
		await x.opts.toggleJumpToCompare();
		expect(x.opts.jumpToCompare).toBe(true);
		await x.opts.toggleShopsFirst();
		const sent = x.settingsClient.put.mock.calls.at(-1)![0];
		expect(sent.jump_to_compare).toBe(true);
		expect(sent.shops_first).toBe(true);
		expect(sent.monster_sim_rounds).toBe(500);
		await x.opts.toggleJumpToCompare();
		expect(x.opts.jumpToCompare).toBe(false);
		expect(x.opts.shopsFirst).toBe(true);
	});
});
