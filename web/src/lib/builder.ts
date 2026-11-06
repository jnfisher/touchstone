// The Classes/Races builder panel's small rules: the picker list and the Spellbook button's note.
// The text and the button effects come from the server (`/api/class-race-builder`).
import type { BuilderSpellbook } from './api';

/** `Classes.MageryType` names, in the order of the codes. */
const MAGERY = ['None', 'Mage', 'Priest', 'Druid', 'Bard', 'Kai'];

/** What the Spellbook button reports. MME has a spell book form; touchstone has none yet, so the
 * seed it would open with is shown instead. An empty note means the click did nothing. */
export function spellbookNote(sb: BuilderSpellbook, className: string): string {
	if (sb.action === 'nothing') return '';
	if (sb.action === 'no_magic') return 'No Magic';
	const kind = MAGERY[sb.magery ?? 0] ?? String(sb.magery);
	return `Spell book for ${className}: level ${sb.level}, ${kind} magery, magery level ${sb.magery_level}`;
}

/** The pair the builder sends: the selected row on its own side and the picked row on the other. */
export function pairFor(kind: 'race' | 'class', selected: number, picked: number | undefined) {
	const other = picked ?? 0;
	return kind === 'race' ? { race: selected, class: other } : { race: other, class: selected };
}

/** The text box shows MME's CRLF as plain lines. */
export function boxText(text: string | null | undefined): string {
	return (text ?? '').replace(/\r\n/g, '\n');
}
