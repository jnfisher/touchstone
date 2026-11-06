<script lang="ts">
	// MME's Results window as a dialog: a list of lines, each opening the record it names (the Items,
	// Spells, Monsters, Shops or Rooms tab with that record selected).
	import { goto } from '$app/navigation';
	import { api, type Item } from '$lib/api';
	import { linkFor, type RefTarget } from '$lib/detail';
	import { dialogs } from '$lib/dialogs.svelte';
	import { selection } from '$lib/explorer-state.svelte';
	import { Button } from '$lib/components/ui/button';

	let items: Map<number, Item> | undefined;

	async function follow(t: RefTarget) {
		if (t.table === 'items' && !items) items = new Map((await api.table('items')).map((i) => [i.number, i]));
		const to = linkFor(t, items);
		if (!to) return;
		selection.set(to.slug, to.id);
		dialogs.closeResults();
		void goto(`/${to.slug}`);
	}
</script>

{#if dialogs.results}
	{@const r = dialogs.results}
	<div class="bg-background/80 fixed inset-0 z-[60] flex items-center justify-center p-4" role="presentation">
		<div
			class="bg-card flex max-h-[80vh] w-full max-w-2xl flex-col gap-2 rounded-lg border p-4 shadow-xl"
			role="dialog"
			aria-modal="true"
			aria-label={r.title}
			data-testid="results-dialog"
		>
			<h2 class="text-base font-semibold" data-testid="results-title">{r.title}</h2>
			<div class="min-h-0 flex-1 overflow-auto rounded-md border">
				<table class="w-full text-sm" data-testid="results-table">
					{#if r.columns}
						<thead class="bg-muted text-muted-foreground sticky top-0 text-left text-xs">
							<tr>{#each r.columns as c (c)}<th class="px-2 py-1 font-medium">{c}</th>{/each}</tr>
						</thead>
					{/if}
					<tbody>
						{#each r.rows as row, i (i)}
							<tr class="hover:bg-accent border-t" data-testid="results-row">
								{#each row.cells as cell, k (k)}
									<td class="px-2 py-1">
										{#if k === 0 && row.link && r.columns === undefined}
											<button type="button" class="text-primary text-left hover:underline" onclick={() => follow(row.link!)}>{cell}</button>
										{:else if row.link && r.columns && k === row.cells.length - 1}
											<button type="button" class="text-primary text-left hover:underline" onclick={() => follow(row.link!)}>{cell}</button>
										{:else}
											{cell}
										{/if}
									</td>
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			<div class="flex justify-end">
				<Button size="sm" variant="outline" onclick={() => dialogs.closeResults()} data-testid="results-close">Close</Button>
			</div>
		</div>
	</div>
{/if}
