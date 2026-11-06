// Small hand-offs between a context-menu action and the page it opens. The menu sets one, navigates,
// and the page takes it once on mount (the calculators' boxes, the combat setup's choice, the
// weapon and spell lists' "versus this monster" numbers).

export interface CalcPrefill {
	/** Swing and backstab calculators: the weapon, and for swings whether it is bashing. */
	weapon?: number;
	bashing?: boolean;
	/** Hit calculator: the monster and which side it takes (MME asks "attacker [yes] or defender [no]"). */
	monster?: number;
	side?: 'attacker' | 'defender';
}

let calc = $state<{ for: 'swing' | 'backstab' | 'hit'; with: CalcPrefill } | undefined>();

export const calcPrefill = {
	set(name: 'swing' | 'backstab' | 'hit', value: CalcPrefill) {
		calc = { for: name, with: value };
	},
	/** Takes the hand-off meant for this calculator (once). */
	take(name: 'swing' | 'backstab' | 'hit'): CalcPrefill | undefined {
		if (calc?.for !== name) return undefined;
		const v = calc.with;
		calc = undefined;
		return v;
	}
};

/** PopUpChooseCombatGUI's two optional arguments: a backstab weapon and a spell to go to. */
let combat = $state<{ backstab?: number; spell?: number } | undefined>();

export const combatPrefill = {
	set(v: { backstab?: number; spell?: number }) {
		combat = v;
	},
	get pending() {
		return combat;
	},
	take() {
		const v = combat;
		combat = undefined;
		return v;
	}
};

/**
 * "Set Weapon/Spell Combat Calc. to Mob": the monster's AC, DR and dodge (weapons) or magic
 * resistance (spells) become the numbers the lists calculate against, with combat values switched on.
 */
export interface VsMob {
	name: string;
	ac: string;
	dr: string;
	dodge: string;
	mr: string;
}

let vs = $state<{ weapons?: VsMob; spells?: VsMob }>({});

export const vsMob = {
	get weapons() {
		return vs.weapons;
	},
	get spells() {
		return vs.spells;
	},
	setWeapons(v: VsMob | undefined) {
		vs.weapons = v;
	},
	setSpells(v: VsMob | undefined) {
		vs.spells = v;
	}
};

/** What the Compare tab's weapons and spells lists also use. */
export function vsMobOptions(slug: string): { calc_combat?: boolean; vs_ac?: string; vs_dr?: string; vs_dodge?: string; vs_mr?: string } {
	if (slug === 'weapons') {
		const w = vs.weapons;
		return w ? { calc_combat: true, vs_ac: w.ac, vs_dr: w.dr, vs_dodge: w.dodge } : {};
	}
	if (slug === 'spells') {
		const s = vs.spells;
		return s ? { calc_combat: true, vs_mr: s.mr } : {};
	}
	return {};
}
