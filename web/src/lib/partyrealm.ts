// A party from realm characters. MME averages a pasted party's members into the Monsters party boxes; here
// each member's numbers come from the realm character instead (its evaluation and its combat preview), go
// into the same party form, and the server's own paste-party code recounts the averages and decides what the
// boxes get. Nothing is calculated here beyond reading a caption.
import { characterClient, evalRequest, type CharacterClient, type CharacterDoc } from './character';
import { captionNumber } from './calc';
import { combatClient, recoveryOnly, type CombatClient } from './combat';
import { emptyPartyForm, pasteClient, runPartyContinue, type PartyFilter, type PartyForm, type PasteClient } from './paste';
import { fullName, realmClient, type RealmClient, type RealmSummary } from './realm';
import type { PartyBoxes } from './monsters';

/** MME's party table has six member rows. */
export const PARTY_MAX = 6;

export interface PartyDeps {
	realm: Pick<RealmClient, 'input'>;
	chars: Pick<CharacterClient, 'evaluate'>;
	combat: Pick<CombatClient, 'preview'>;
	paste: Pick<PasteClient, 'party' | 'partyContinue'>;
}

export const defaultPartyDeps: PartyDeps = { realm: realmClient, chars: characterClient, combat: combatClient, paste: pasteClient };

/** A damage figure the combat preview did not really calculate (the one-shot attack's 999999). */
const real = (n: number) => n > 0 && n < 999_999;

/** One member's boxes: defenses and recovery from the evaluation, damage and swings from the combat preview. */
export async function memberColumns(doc: CharacterDoc, deps: PartyDeps, recovery = false) {
	const req = evalRequest(doc, { hide_class_race: false, min_item_level: 0 });
	if (!req) throw new Error(`${doc.name} has no class and race`);
	const ev = await deps.chars.evaluate(req);
	const tag = (i: number) => ev.calc?.stat_tags[i] ?? 0;
	const text = (n: number | null | undefined) => (n === null || n === undefined ? '' : String(Math.trunc(n)));
	const col = {
		ac: text(tag(2)),
		dr: text(captionNumber(ev.stats[3])),
		mr: text(ev.magic_res),
		dodge: text(ev.dodge),
		hitpoints: text(ev.hp.average),
		regen_hp: text(ev.hp.rest_normal),
		rest_hp: text(ev.hp.rest_resting),
		accy: text(tag(10)),
		dmg: '',
		spell_dmg: '',
		swings: '',
		anti_magic: ev.anti_magic ? 1 : 0
	};
	try {
		const p = await deps.combat.preview(doc, doc.attack, recovery);
		if (!p.error_text) {
			if (real(p.attack.damage)) {
				col.dmg = String(p.attack.damage);
				col.swings = p.attack.swings === null ? '' : String(p.attack.swings);
			} else if (real(p.attack.spell_value)) col.spell_dmg = String(p.attack.spell_value);
		}
	} catch {
		// a member with no usable attack just leaves its damage boxes empty, as an unpasted one does
	}
	return col;
}

/** The party table for these characters, in the order given (members 1 to 6). */
export async function buildPartyForm(
	picks: { summary: Pick<RealmSummary, 'first_name' | 'last_name'>; doc: CharacterDoc }[],
	deps: PartyDeps
): Promise<PartyForm> {
	const form = emptyPartyForm();
	const members = picks.slice(0, PARTY_MAX);
	for (let k = 0; k < members.length; k++) {
		const i = k + 1;
		const c = await memberColumns(members[k].doc, deps);
		form.name[i] = fullName(members[k].summary);
		form.ac[i] = c.ac;
		form.dr[i] = c.dr;
		form.mr[i] = c.mr;
		form.dodge[i] = c.dodge;
		form.hitpoints[i] = c.hitpoints;
		form.regen_hp[i] = c.regen_hp;
		form.rest_hp[i] = c.rest_hp;
		form.accy[i] = c.accy;
		form.dmg[i] = c.dmg;
		form.spell_dmg[i] = c.spell_dmg;
		form.swings[i] = c.swings;
		form.anti_magic[i] = c.anti_magic;
	}
	return form;
}

/** What Continue writes becomes the party boxes (`cmdContinue_Click` fills `txtMonsterLairFilter`). */
export function boxesFromFilter(f: PartyFilter): { boxes: Partial<PartyBoxes>; damage: string | null } {
	const boxes: Partial<PartyBoxes> = {};
	const put = (k: keyof PartyBoxes, v: string | null) => {
		if (v !== null) boxes[k] = v;
	};
	put('size', f.party_size);
	put('ac', f.ac);
	put('dr', f.dr);
	put('mr', f.mr);
	put('dodge', f.dodge);
	put('hp', f.hitpoints);
	put('anti_magic', f.anti_magic);
	put('rest_hp', f.rest_hp);
	put('accuracy', f.accy);
	put('swings', f.swings);
	put('damage_out_physical', f.damage_out_phys);
	put('damage_out_magic', f.damage_out_spell);
	return { boxes, damage: f.damage };
}

export interface PartyResult {
	/** Set when the party was applied. */
	filter?: PartyFilter;
	boxes?: Partial<PartyBoxes>;
	damage?: string | null;
	/** MME's own words when it leaves the boxes alone (a party of one). */
	note?: string;
	form: PartyForm;
}

/** Recounts the averages (the paste-party code), then Continue. */
export async function partyFromRealm(
	picks: { summary: Pick<RealmSummary, 'first_name' | 'last_name'>; doc: CharacterDoc }[],
	deps: PartyDeps = defaultPartyDeps
): Promise<PartyResult> {
	const built = await buildPartyForm(picks, deps);
	const counted = await deps.paste.party({ text: '', form: built });
	const res = await runPartyContinue(
		deps.paste,
		{ form: counted.form, current_damage: 99999, current_damage_out: [0, 0] },
		async () => undefined
	);
	if (!res) return { form: counted.form, note: 'Nothing was changed.' };
	if (res.status === 'applied' && res.filter) {
		const m = boxesFromFilter(res.filter);
		return { form: counted.form, filter: res.filter, boxes: m.boxes, damage: m.damage };
	}
	return { form: counted.form, note: res.message ?? 'Nothing was changed.' };
}
