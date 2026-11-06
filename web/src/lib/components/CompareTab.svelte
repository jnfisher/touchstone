<script lang="ts">
	// The Compare tab (F11): MME's four saved lists. Each is the explorer's own list, shown for the
	// records on it, with the same detail pane. The lists belong to the open character.
	import ExplorerView from '$lib/components/ExplorerView.svelte';
	import { COMPARE_KINDS, COMPARE_LABEL } from '$lib/compare';
	import { getCompareStore } from '$lib/compare.svelte';
	import { explorerFor } from '$lib/explorers';

	const cmp = getCompareStore();
	const lists = $derived(cmp.lists);
	const kind = $derived(cmp.active);
	const explorer = $derived(explorerFor(kind));
	const total = $derived(COMPARE_KINDS.reduce((n, k) => n + lists[k].length, 0));
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
	<div class="flex flex-wrap items-center gap-2" role="tablist" aria-label="Saved lists">
		{#each COMPARE_KINDS as k (k)}
			<button
				type="button"
				role="tab"
				aria-selected={kind === k}
				class="h-8 rounded-md border px-3 text-sm {kind === k
					? 'bg-accent border-ring font-semibold'
					: 'bg-background border-input hover:bg-accent'}"
				data-testid="compare-nav-{k}"
				onclick={() => cmp.setActive(k)}
			>
				{COMPARE_LABEL[k]}
				<span class="text-muted-foreground tabular-nums">({lists[k].length})</span>
			</button>
		{/each}
		<button
			type="button"
			class="bg-background border-input hover:bg-accent ml-auto h-8 rounded-md border px-3 text-sm disabled:opacity-50"
			disabled={total === 0}
			data-testid="compare-clear-all"
			onclick={() => {
				// cmdCompareClear_Click(4): "Clear Compare Lists?" with No as the default
				if (confirm('Clear all four compare lists?')) cmp.clearAll();
			}}
		>
			Clear all
		</button>
	</div>

	<div class="min-h-0 flex-1">
		{#if explorer && lists[kind].length > 0}
			{#key kind}
				<ExplorerView
					{explorer}
					title={COMPARE_LABEL[kind]}
					list={{
						numbers: lists[kind],
						onRemove: (i) => cmp.removeAt(kind, i),
						onClear: () => cmp.clear(kind)
					}}
				/>
			{/key}
		{:else}
			<p class="text-muted-foreground text-sm" data-testid="compare-empty">
				No {COMPARE_LABEL[kind].toLowerCase()} saved. Use "Add to compare" on a {kind === 'armour'
					? 'piece of armour'
					: kind === 'weapons'
						? 'weapon'
						: kind === 'spells'
							? 'spell'
							: 'monster'} in its tab{kind === 'weapons' || kind === 'armour' ? ', or "Add all to compare" on the Character tab' : ''}.
			</p>
		{/if}
	</div>
</div>
