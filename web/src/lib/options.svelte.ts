// The Options menu's actions (frmMain `mnuOptionsItems_Click` and the two checked items under it), wired to
// the page's stores. The rules run on the server; this routes between the character, the filters, the compare
// lists and the monster damage / reachability calculations, and asks MME's confirmation questions.
import { api, type ReachabilityStatus } from './api';
import { getCharacterStore, type CharacterStore } from './character.svelte';
import { readLists, writeLists } from './compare';
import { getCompareStore, type CompareStore } from './compare.svelte';
import { dialogs } from './dialogs.svelte';
import { filterReset, spellFilters } from './explorer-state.svelte';
import { DEFAULT_SPELL_FILTERS } from './spelltools';
import { getGlobalFilterStore, type GlobalFilterStore } from './globalfilter.svelte';
import { getMonsterFilterStore, type MonsterFilterStore } from './monsters.svelte';
import { canCalculate } from './monsterdamage';
import { getDamageStore, type DamageStore } from './monsterdamage.svelte';
import { getPasteStore, type PasteStore } from './paste.svelte';
import { getSettingsStore, type SettingsStore } from './settings.svelte';

export type Ask = (
	title: string,
	text: string,
	choices: { value: string; label: string }[]
) => Promise<string | undefined>;

export interface OptionsDeps {
	chars?: CharacterStore;
	cmp?: CompareStore;
	damage?: DamageStore;
	boxes?: MonsterFilterStore;
	bar?: GlobalFilterStore;
	settings?: SettingsStore;
	paste?: PasteStore;
	ask?: Ask;
	reachability?: Pick<typeof api, 'reachabilityStatus' | 'computeReachability'>;
}

const YES_NO = [
	{ value: 'yes', label: 'Yes' },
	{ value: 'no', label: 'No' }
];

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function createOptionsStore(deps: OptionsDeps = {}) {
	const chars = deps.chars ?? getCharacterStore();
	const cmp = deps.cmp ?? getCompareStore();
	const damage = deps.damage ?? getDamageStore();
	const boxes = deps.boxes ?? getMonsterFilterStore();
	const bar = deps.bar ?? getGlobalFilterStore();
	const settings = deps.settings ?? getSettingsStore();
	const paste = deps.paste ?? getPasteStore();
	const ask: Ask = deps.ask ?? ((t, x, c) => dialogs.ask(t, x, c));
	const reachApi = deps.reachability ?? api;

	let pasteOpen = $state(false);
	let settingsOpen = $state(false);
	let reach = $state.raw<ReachabilityStatus>();
	let reachBusy = $state(false);
	let notice = $state<string>();

	async function refreshReach() {
		try {
			reach = await reachApi.reachabilityStatus();
		} catch {
			// a courtesy read; the chip just stays off
		}
	}

	return {
		get pasteOpen() {
			return pasteOpen;
		},
		set pasteOpen(v: boolean) {
			pasteOpen = v;
		},
		get settingsOpen() {
			return settingsOpen;
		},
		set settingsOpen(v: boolean) {
			settingsOpen = v;
		},
		get reach() {
			return reach;
		},
		get reachBusy() {
			return reachBusy;
		},
		/** The last message MME would have shown in a MsgBox. */
		get notice() {
			return notice;
		},
		dismissNotice() {
			notice = undefined;
		},
		/** Whether the vs Char item can run: a class and race are chosen. */
		get canCalcDamage() {
			return canCalculate(chars.doc) && !damage.busy;
		},
		/** The vs Party item reads only the party boxes, so MME offers it with no character loaded. */
		get canCalcPartyDamage() {
			return !damage.busy;
		},
		/** The two checked items. */
		get jumpToCompare() {
			return settings.current.jump_to_compare;
		},
		get shopsFirst() {
			return settings.current.shops_first;
		},
		refreshReach,

		/** `PasteCharacter`: the Paste dialog. */
		pasteCharacter() {
			pasteOpen = true;
		},
		/** `ReachRecompute`: map every monster's level bands, then the list and pane ask again. */
		async calculateReachability(): Promise<void> {
			if (reachBusy) return;
			reachBusy = true;
			notice = undefined;
			try {
				reach = await reachApi.computeReachability();
				boxes.bumpReach();
				if (!reach.computed) notice = 'Reachability could not be computed for this database.';
			} catch (e) {
				notice = `Reachability could not be computed: ${message(e)}`;
			} finally {
				reachBusy = false;
			}
		},
		/**
		 * A double-click on the monster pane's "(double-click to compute)" row (`reach_compute`): compute unless
		 * something else did since the pane was drawn, then draw the pane again.
		 */
		async reachRow(): Promise<void> {
			await refreshReach();
			if (reach?.computed) {
				boxes.bumpReach();
				return;
			}
			await this.calculateReachability();
		},
		/** Calc. Monster Dmg vs Char / vs Party. */
		async calcDamage(mode: 'char' | 'party'): Promise<boolean> {
			return damage.calculate(chars.doc, mode, boxes.controls.party);
		},
		/** Clear Calculated Monster Dmg. */
		async clearDamage(): Promise<void> {
			await damage.clear();
		},
		/** Clear Learned Spells: "Are you sure?", default No. */
		async clearLearnedSpells(): Promise<boolean> {
			if ((await ask('Reset Learned Spells', 'Are you sure?', YES_NO)) !== 'yes') return false;
			chars.edit((d) => {
				d.learned_spells = [];
				d.learned_class = 0;
			});
			return true;
		},
		/**
		 * Reset All Character Fields: a blank character, the Item Manager emptied, the global filter back to its
		 * defaults; the saved compare lists stay unless the second question says to clear them.
		 */
		async resetAllCharacterFields(): Promise<boolean> {
			if ((await ask('Reset All Character Fields', 'Are you sure?', YES_NO)) !== 'yes') return false;
			const lists = readLists(chars.doc.extra);
			let clearLists = true;
			if (Object.values(lists).some((l) => l.length > 0)) {
				const a = await ask(
					'Reset All Character Fields',
					'Clear saved weapon, armour, spell, and monsters lists?  The item manager is cleared regardless.',
					YES_NO
				);
				// Cancel stops here
				if (a === undefined) return false;
				clearLists = a === 'yes';
			}
			await chars.flush();
			chars.startNew();
			if (!clearLists) {
				const keep = lists;
				chars.edit((d) => {
					d.extra = writeLists(d.extra, keep);
				});
			}
			paste.setRows([]);
			bar.reset();
			return true;
		},
		/** Remove All Filters: the bar, the Monsters and Spells boxes, and every explorer's search and column filters. */
		removeAllFilters() {
			bar.reset();
			boxes.reset();
			// the Spells boxes are filters too (ResetFilterOptions)
			Object.assign(spellFilters, DEFAULT_SPELL_FILTERS);
			filterReset.epoch++;
		},
		openSettings() {
			settingsOpen = true;
		},
		async toggleJumpToCompare(): Promise<void> {
			await settings.setToggle('jump_to_compare', !settings.current.jump_to_compare);
			if (settings.error) notice = settings.error;
		},
		async toggleShopsFirst(): Promise<void> {
			await settings.setToggle('shops_first', !settings.current.shops_first);
			if (settings.error) notice = settings.error;
		}
	};
}

export type OptionsStore = ReturnType<typeof createOptionsStore>;

let shared: OptionsStore | undefined;
export function getOptionsStore(): OptionsStore {
	return (shared ??= createOptionsStore());
}

