// The two little dialogs a menu action can need: the selectable text shown when the browser will not
// copy for us, and a three-way question (MME's MsgBox with Yes, No and Cancel).

export interface ChoiceQuestion {
	title: string;
	text: string;
	choices: { value: string; label: string }[];
	resolve: (value: string | undefined) => void;
}

import type { ResultsDialog } from './spelltools';

export interface PromptQuestion {
	title: string;
	text: string;
	value: string;
	resolve: (value: string | undefined) => void;
}

let results = $state.raw<ResultsDialog | undefined>();
let prompting = $state<PromptQuestion | undefined>();
let fallback = $state<{ text: string; note?: string } | undefined>();
let question = $state<ChoiceQuestion | undefined>();

export const dialogs = {
	get fallback() {
		return fallback;
	},
	showText(text: string, note?: string) {
		fallback = { text, note };
	},
	closeText() {
		fallback = undefined;
	},
	get results() {
		return results;
	},
	/** MME's Results window: a list whose rows open their records. */
	showResults(r: ResultsDialog) {
		results = r;
	},
	closeResults() {
		results = undefined;
	},
	get prompt() {
		return prompting;
	},
	/** MME's InputBox: the typed text, or undefined for Cancel. */
	askText(title: string, text: string, value = ''): Promise<string | undefined> {
		return new Promise((resolve) => {
			prompting = {
				title,
				text,
				value,
				resolve: (v) => {
					prompting = undefined;
					resolve(v);
				}
			};
		});
	},
	get question() {
		return question;
	},
	ask(title: string, text: string, choices: { value: string; label: string }[]): Promise<string | undefined> {
		return new Promise((resolve) => {
			question = {
				title,
				text,
				choices,
				resolve: (v) => {
					question = undefined;
					resolve(v);
				}
			};
		});
	}
};
