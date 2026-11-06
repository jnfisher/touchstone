<script lang="ts">
	// The Item Manager (MME's lvItemManager on the EQ tab): the items the character carries, stashes
	// or has flagged to sell, drop or pick up. The buttons are MME's; every rule runs on the server.
	import type { Item } from '$lib/api';
	import { toChoices } from '$lib/character';
	import type { CharacterStore } from '$lib/character.svelte';
	import { toColDefs } from '$lib/grid/columns';
	import DataGrid from '$lib/grid/DataGrid.svelte';
	import {
		ACTION_BUTTONS,
		HELP_TEXT,
		IM_COLUMNS,
		idsToSelection,
		rowId,
		rowState,
		selectionToIds,
		toGridRows,
		type ImRow
	} from '$lib/itemmanager';
	import { createItemManager } from '$lib/itemmanager.svelte';
	import type { Answer, Prompt } from '$lib/paste';
	import { headingCls } from '../calc/styles';
	import QuestionDialog from '../paste/QuestionDialog.svelte';
	import ItemPicker from './ItemPicker.svelte';

	let { store, items, onpaste }: { store: CharacterStore; items: Item[]; onpaste: () => void } = $props();

	// the shared store lives for the page, so reading it once here is intended
	// svelte-ignore state_referenced_locally
	const im = createItemManager(store);
	const gridRows = $derived<ImRow[]>(toGridRows(im.rows));
	const columnDefs = toColDefs(IM_COLUMNS);
	const addChoices = $derived(
		toChoices(
			items
				.filter((i) => i.in_game && i.name !== '')
				.map((i) => ({ number: i.number, label: `${i.name} (${i.number})` }))
		)
	);
	let showHelp = $state(false);

	// The list is rebuilt whenever a different character is opened (LoadCharacter), not on a paste.
	let seen = -1;
	$effect(() => {
		const epoch = store.loadEpoch;
		if (epoch === seen) return;
		const first = seen === -1;
		seen = epoch;
		// the first look keeps what a paste left behind; the saved lists fill an empty list
		if (!first || im.rows.length === 0) void im.reload();
	});

	let question = $state<{ prompt: Prompt; resolve: (a: Answer | undefined) => void }>();
	function ask(prompt: Prompt): Promise<Answer | undefined> {
		return new Promise((resolve) => {
			question = {
				prompt,
				resolve: (a) => {
					question = undefined;
					resolve(a);
				}
			};
		});
	}

	const btn = 'border-input h-7 rounded-md border px-2 text-xs disabled:opacity-50';
	const none = $derived(im.selected.length === 0);
</script>

<div class="relative" data-testid="item-manager">
	<div class="mb-2 flex flex-wrap items-center gap-x-2 gap-y-1">
		<h3 class="{headingCls} !mb-0">Item Manager</h3>
		<span class="text-muted-foreground text-xs" data-testid="im-count">
			{im.rows.length} rows, {im.selected.length} selected
		</span>
		<button type="button" class={btn} onclick={onpaste} data-testid="im-paste" title="Paste items from the game's inventory output">Paste</button>
		<button type="button" class={btn} disabled={none || im.busy} onclick={() => void im.remove(ask)} data-testid="im-delete" title="Delete the selected rows">Delete</button>
		<button type="button" class={btn} disabled={im.rows.length === 0} onclick={() => im.invert()} data-testid="im-invert" title="Select what is not selected">Invert</button>
		<span class="bg-border mx-1 h-5 w-px"></span>
		{#each ACTION_BUTTONS as b (b.action)}
			<button
				type="button"
				class={btn}
				disabled={none || im.busy}
				title={b.title}
				onclick={() => void im.action(b.action, ask)}
				data-testid="im-action-{b.action}">{b.label}</button
			>
		{/each}
		<span class="bg-border mx-1 h-5 w-px"></span>
		<button type="button" class={btn} disabled={none || im.busy} onclick={() => void im.copy()} data-testid="im-copy" title="Copy the game commands for the selected rows">Copy</button>
		<button type="button" class={btn} onclick={() => (showHelp = !showHelp)} data-testid="im-help" aria-expanded={showHelp}>Help</button>
		<div class="ml-auto w-64">
			<ItemPicker
				choices={addChoices}
				value={0}
				label="Add an item"
				noneLabel="Add an item..."
				testid="im-add"
				onpick={(n) => {
					if (n) void im.add(n);
				}}
			/>
		</div>
	</div>
	{#if showHelp}<p class="text-muted-foreground mb-2 max-w-4xl text-xs" data-testid="im-help-text">{HELP_TEXT}</p>{/if}
	{#if im.note}<p class="text-muted-foreground mb-2 text-xs" data-testid="im-note">{im.note}</p>{/if}
	{#if im.problem}<p class="text-destructive mb-2 text-xs" data-testid="im-error">{im.problem}</p>{/if}

	<div class="h-[27rem]" data-testid="im-grid">
		<DataGrid
			rows={gridRows}
			{columnDefs}
			getRowId={rowId}
			storageKey="item-manager"
			{rowState}
			multiSelect
			selectedIds={selectionToIds(im.selected)}
			onSelectionIds={(ids) => im.select(idsToSelection(ids))}
		/>
	</div>

	{#if question}
		<QuestionDialog prompt={question.prompt} title="Item Manager" onanswer={question.resolve} />
	{/if}
</div>
