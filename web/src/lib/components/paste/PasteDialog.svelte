<script lang="ts">
	// MME's Paste Character window (frmPasteChar): paste the game's stat / inventory / spells output
	// or a party display, see what it would change and every box MME would show, then Apply.
	import { api, type ClassRec, type Item, type Race } from '$lib/api';
	import type { CharacterStore } from '$lib/character.svelte';
	import { getPasteStore } from '$lib/paste.svelte';
	import {
		characterChanges,
		emptyPartyForm,
		megaDamage,
		partyMembers,
		pasteClient,
		pasteIntoBox,
		rowCells,
		runCharacterPaste,
		runPartyContinue,
		runPartyPaste,
		VB_NO,
		VB_YES,
		type Answer,
		type CharacterPasteResponse,
		type Change,
		type PartyFilter,
		type PartyForm,
		type PasteClient,
		type Prompt
	} from '$lib/paste';
	import { getMonsterFilterStore } from '$lib/monsters.svelte';
	import { boxesFromFilter } from '$lib/partyrealm';
	import PartyTable from './PartyTable.svelte';
	import QuestionDialog from './QuestionDialog.svelte';

	let {
		store,
		onclose,
		client = pasteClient
	}: {
		store: CharacterStore;
		onclose: () => void;
		client?: PasteClient;
	} = $props();

	const paste = getPasteStore();
	const boxes = getMonsterFilterStore();
	let tab = $state<'character' | 'party'>('character');
	let charText = $state('');
	let partyText = $state('');
	let unequipMissing = $state(false);
	let busy = $state(false);
	let error = $state<string>();
	let note = $state<string>();

	let names = $state({ classes: new Map<number, string>(), races: new Map<number, string>(), items: new Map<number, string>() });
	$effect(() => {
		let alive = true;
		Promise.all([api.table('classes'), api.table('races'), api.table('items')])
			.then(([c, r, i]: [ClassRec[], Race[], Item[]]) => {
				if (!alive) return;
				names = {
					classes: new Map(c.map((x) => [x.number, x.name])),
					races: new Map(r.map((x) => [x.number, x.name])),
					items: new Map(i.map((x) => [x.number, x.name]))
				};
			})
			.catch(() => undefined);
		return () => (alive = false);
	});

	// ---- questions ----
	let question = $state<{ prompt: Prompt; title: string; resolve: (a: Answer | undefined) => void }>();
	function ask(prompt: Prompt, title = 'Paste'): Promise<Answer | undefined> {
		return new Promise((resolve) => {
			question = {
				prompt,
				title,
				resolve: (a) => {
					question = undefined;
					resolve(a);
				}
			};
		});
	}
	function close() {
		question?.resolve(undefined);
		onclose();
	}

	async function guarded(fn: () => Promise<void>) {
		busy = true;
		error = undefined;
		note = undefined;
		try {
			await fn();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}

	async function clipboard(): Promise<string | undefined> {
		try {
			const t = await navigator.clipboard.readText();
			if (t !== '') return t;
			note = 'The clipboard is empty.';
		} catch {
			error = 'The browser did not allow reading the clipboard. Paste into the box with Ctrl+V.';
		}
		return undefined;
	}

	// ---- character ----
	let result = $state.raw<CharacterPasteResponse>();
	let changes = $state.raw<Change[]>([]);
	let previewedText = '';
	const stale = $derived(result !== undefined && previewedText !== charText);

	async function fromClipboard() {
		const clip = await clipboard();
		if (clip === undefined) return;
		let clearFirst = true;
		if (charText.trim() !== '') {
			const a = await ask(
				{ kind: 'yes_no', text: 'Clear paste area first?', default: VB_YES, answer: null },
				'Clear?'
			);
			if (a === undefined) return;
			clearFirst = a === VB_YES;
		}
		charText = pasteIntoBox(charText, clip, clearFirst);
		await preview();
	}

	function preview() {
		return guarded(async () => {
			result = undefined;
			const before = $state.snapshot(store.doc);
			const res = await runCharacterPaste(
				client,
				{
					character: before,
					text: charText,
					rows: paste.rows.length ? paste.rows : undefined,
					char_loaded: before.name !== '' && store.storedName !== undefined,
					unequip_missing: unequipMissing
				},
				(p) => ask(p)
			);
			if (!res) return;
			previewedText = charText;
			result = res;
			changes = characterChanges(before, res.character, names);
		});
	}

	async function apply() {
		if (!result) return;
		const r = result;
		await store.applyPasted(r.character);
		paste.setRows(r.rows);
		onclose();
	}

	// ---- party ----
	let form = $state<PartyForm>(emptyPartyForm());
	let partyMessages = $state<string[]>([]);
	let partyDone = $state(false);
	let filter = $state.raw<PartyFilter>();
	let partyNote = $state<string>();

	async function partyFromClipboard() {
		const clip = await clipboard();
		if (clip === undefined) return;
		partyText = pasteIntoBox(partyText, clip, true);
		await partyPreview();
	}

	function partyPreview() {
		return guarded(async () => {
			filter = undefined;
			partyNote = undefined;
			const res = await runPartyPaste(
				client,
				{ text: partyText, form: $state.snapshot(form) },
				async (text) => {
					const a = await ask({ kind: 'yes_no', text, default: VB_NO, answer: null }, 'Party');
					return a === undefined ? undefined : a === VB_YES;
				},
				async (q) => {
					const a = await ask(
						{
							kind: 'input',
							text: `Attack for ${q.who ?? 'this character'}.\r\nEnter the attack short name, or cancel to skip.`,
							default: q.default ?? '',
							answer: null
						},
						'Party attack'
					);
					return a === undefined ? undefined : a === '' ? null : String(a);
				}
			);
			if (!res) return;
			form = res.form;
			partyMessages = res.messages;
			partyDone = res.status === 'done';
			if (res.status === 'canceled') partyNote = 'Paste at least 10 characters.';
			if (res.status === 'no_data_exit') partyNote = 'No party data in the paste.';
		});
	}

	function recount() {
		return guarded(async () => {
			const res = await client.party({ text: '', form: $state.snapshot(form) });
			form = res.form;
		});
	}

	async function mega(member: number, spell: boolean) {
		const clip = await clipboard();
		if (clip === undefined) return;
		const v = megaDamage(clip, spell);
		if (v === undefined) {
			note = 'No MegaMUD damage report on the clipboard.';
			return;
		}
		(spell ? form.spell_dmg : form.dmg)[member] = v;
		await recount();
	}

	function continueParty() {
		return guarded(async () => {
			const res = await runPartyContinue(
				client,
				{
					form: $state.snapshot(form),
					current_damage: 99999,
					current_damage_out: [0, 0]
				},
				(p) => ask(p, 'Update the filter?')
			);
			if (!res) return;
			if (res.status === 'info') partyNote = res.message ?? undefined;
			else if (res.status === 'canceled') partyNote = 'Nothing was changed.';
			else if (res.filter) {
				paste.setPartyFilter(res.filter);
				const m = boxesFromFilter(res.filter);
				boxes.applyParty(m.boxes, m.damage, form.name.slice(1).filter((n) => n.trim() !== ''));
				filter = res.filter;
				partyNote = 'Party filter values set.';
			}
		});
	}

	const FILTER_LABELS: [keyof PartyFilter, string][] = [
		['party_size', 'Party size'],
		['damage', 'Sustainable damage in'],
		['ac', 'AC'],
		['dr', 'DR'],
		['mr', 'MR'],
		['dodge', 'Dodge'],
		['hitpoints', 'Hit points'],
		['anti_magic', 'Anti-magic'],
		['rest_hp', 'Rest HP'],
		['accy', 'Accuracy'],
		['swings', 'Swings'],
		['damage_out_phys', 'Damage out (physical)'],
		['damage_out_spell', 'Damage out (spell)']
	];

	const tabBtn = (on: boolean) =>
		`h-8 rounded-md px-3 text-sm ${on ? 'bg-secondary text-secondary-foreground font-medium' : 'text-muted-foreground'}`;
	const btn = 'border-input h-8 rounded-md border px-3 text-sm disabled:opacity-50';
	const primary = 'bg-primary text-primary-foreground h-8 rounded-md px-3 text-sm disabled:opacity-50';
	const area =
		'bg-background border-input min-h-0 w-full flex-1 resize-none rounded-md border p-2 font-mono text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring/50';
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !question && close()} />

<div class="bg-background/80 fixed inset-0 z-50 flex items-center justify-center p-4" role="presentation">
	<div
		class="bg-card text-card-foreground relative flex h-[min(52rem,100%)] w-full max-w-6xl flex-col overflow-hidden rounded-lg border shadow-xl"
		role="dialog"
		aria-modal="true"
		aria-label="Paste character"
		data-testid="paste-dialog"
	>
		<header class="flex items-center gap-2 border-b p-3">
			<h2 class="mr-3 text-base font-semibold">Paste</h2>
			<button type="button" class={tabBtn(tab === 'character')} onclick={() => (tab = 'character')} data-testid="paste-tab-character">Character</button>
			<button type="button" class={tabBtn(tab === 'party')} onclick={() => (tab = 'party')} data-testid="paste-tab-party">Party</button>
			<button type="button" class="{btn} ml-auto" onclick={close} data-testid="paste-close">Close</button>
		</header>

		{#if error}<p class="text-destructive px-3 pt-2 text-sm" data-testid="paste-error">{error}</p>{/if}
		{#if note}<p class="text-muted-foreground px-3 pt-2 text-sm" data-testid="paste-note">{note}</p>{/if}

		{#if tab === 'character'}
			<div class="grid min-h-0 flex-1 gap-3 p-3 md:grid-cols-2">
				<section class="flex min-h-0 flex-col gap-2" aria-label="Pasted text">
					<p class="text-muted-foreground text-xs">
						Paste the output of <code>stat</code>, <code>inventory</code>, <code>spells</code> and
						<code>powers</code> from the game (a macro such as <code>sp^Mi^Mstat^M</code> gets them all at once).
					</p>
					<textarea
						bind:value={charText}
						class={area}
						spellcheck="false"
						placeholder="Paste the game output here"
						aria-label="Pasted game output"
						data-testid="paste-text"
					></textarea>
					<label class="flex items-center gap-2 text-sm">
						<input type="checkbox" bind:checked={unequipMissing} data-testid="paste-unequip" />
						Unequip slots the pasted inventory does not fill
					</label>
					<div class="flex flex-wrap gap-2">
						<button type="button" class={btn} onclick={fromClipboard} disabled={busy} data-testid="paste-clipboard">Paste from clipboard</button>
						<button type="button" class={primary} onclick={preview} disabled={busy || charText.trim() === ''} data-testid="paste-preview">Preview</button>
					</div>
				</section>

				<section class="flex min-h-0 flex-col gap-2 overflow-auto" aria-label="Preview" data-testid="paste-preview-panel">
					{#if !result}
						<p class="text-muted-foreground text-sm">{busy ? 'Reading the paste...' : 'Preview shows what the paste would change before anything is applied.'}</p>
					{:else}
						{#if stale}<p class="text-sm text-amber-500">The text changed since this preview.</p>{/if}
						<h3 class="text-muted-foreground text-[11px] font-medium tracking-wide uppercase">Changes to the character</h3>
						{#if changes.length === 0}
							<p class="text-sm" data-testid="paste-nochanges">Nothing on the character changes.</p>
						{:else}
							<table class="w-full text-sm" data-testid="paste-changes">
								<tbody>
									{#each changes as c (c.label)}
										<tr class="border-b last:border-0">
											<td class="text-muted-foreground py-1 pr-3">{c.label}</td>
											<td class="py-1 pr-2 text-right tabular-nums line-through opacity-60">{c.before}</td>
											<td class="py-1 tabular-nums">{c.after}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						{/if}

						{#if result.prompts.length || result.events.length}
							<h3 class="text-muted-foreground mt-2 text-[11px] font-medium tracking-wide uppercase">Messages</h3>
							<ul class="space-y-1 text-sm" data-testid="paste-messages">
								{#each result.prompts as p, k (k)}
									<li class="rounded border p-2">
										<span class="whitespace-pre-wrap">{p.text}</span>
										{#if p.answer !== null}
											<span class="text-muted-foreground block text-xs">Answered: {typeof p.answer === 'string' ? (p.answer || 'Cancel') : { 6: 'Yes', 7: 'No', 2: 'Cancel' }[p.answer] ?? p.answer}</span>
										{/if}
									</li>
								{/each}
								{#each result.events as e, k (k)}
									<li class="rounded border p-2">
										{#if e.kind === 'message'}{e.text}
										{:else if e.kind === 'save'}The previous character is saved first.
										{:else if e.kind === 'clear_character'}The character tab was reset before the paste.
										{:else}MME hit an error in {e.source} ({e.number}) and stopped there.{/if}
									</li>
								{/each}
							</ul>
						{/if}

						{#if result.rows.length}
							<h3 class="text-muted-foreground mt-2 text-[11px] font-medium tracking-wide uppercase">
								Item Manager ({result.rows.length}{result.sort_after ? ', sorted by value' : ''})
							</h3>
							<table class="w-full text-xs" data-testid="paste-rows">
								<thead>
									<tr class="text-muted-foreground text-left"><th>Item</th><th>Flag</th><th class="text-right">Qty</th><th>Source</th><th>Value</th><th>Shop</th></tr>
								</thead>
								<tbody>
									{#each result.rows as r, k (k)}
										{@const c = rowCells(r)}
										<tr class="border-b last:border-0">
											<td class="py-0.5">{c.name}</td><td>{c.flag}</td><td class="text-right tabular-nums">{c.qty}</td><td>{c.source}</td><td>{c.value}</td><td>{c.shop}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						{/if}
					{/if}
				</section>
			</div>
			<footer class="flex justify-end gap-2 border-t p-3">
				<button type="button" class={btn} onclick={close}>Cancel</button>
				<button type="button" class={primary} disabled={!result || stale || busy} onclick={apply} data-testid="paste-apply">Apply to character</button>
			</footer>
		{:else}
			<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-auto p-3">
				<div class="grid min-h-40 gap-3 md:grid-cols-[1fr_auto]">
					<div class="flex min-h-0 flex-col gap-2">
						<p class="text-muted-foreground text-xs">
							Paste each character's stat and inventory output one after another (or a MegaMUD party display). Spell buffs for dodge and hp regen are not on the stat sheet.
						</p>
						<textarea bind:value={partyText} class="{area} h-28 flex-none" spellcheck="false" placeholder="Paste the party's game output here" aria-label="Pasted party output" data-testid="party-text"></textarea>
					</div>
					<div class="flex flex-row gap-2 md:flex-col">
						<button type="button" class={btn} onclick={partyFromClipboard} disabled={busy} data-testid="party-clipboard">Paste from clipboard</button>
						<button type="button" class={primary} onclick={partyPreview} disabled={busy || partyText.trim() === ''} data-testid="party-preview">Read party</button>
					</div>
				</div>

				<PartyTable bind:form onedit={recount} onmega={mega} />
				<p class="text-muted-foreground text-xs">
					Only boxes with a value count in the averages; only the average boxes with a value are written when you continue. {partyMembers(form).length} member(s) read.
				</p>
				{#each partyMessages as m, k (k)}<p class="text-sm">{m}</p>{/each}
				{#if partyNote}<p class="text-sm" data-testid="party-note">{partyNote}</p>{/if}
				{#if filter}
					<div class="rounded border p-2" data-testid="party-filter">
						<p class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">Written to the party filter</p>
						<dl class="grid grid-cols-2 gap-x-4 text-sm sm:grid-cols-4">
							{#each FILTER_LABELS as [key, label] (key)}
								{#if filter[key] !== null}<div class="flex justify-between gap-2"><dt class="text-muted-foreground">{label}</dt><dd class="tabular-nums">{filter[key]}</dd></div>{/if}
							{/each}
						</dl>
					</div>
				{/if}
			</div>
			<footer class="flex justify-end gap-2 border-t p-3">
				<button type="button" class={btn} onclick={close}>Cancel</button>
				<button type="button" class={primary} disabled={busy || !partyDone && partyMembers(form).length === 0} onclick={continueParty} data-testid="party-continue">Continue</button>
			</footer>
		{/if}

		{#if question}
			<QuestionDialog prompt={question.prompt} title={question.title} onanswer={question.resolve} />
		{/if}
	</div>
</div>
