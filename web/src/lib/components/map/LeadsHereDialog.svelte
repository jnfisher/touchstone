<script lang="ts">
	// cmdMapLeadsHere_Click's results window in the shared grid: a row per room, spell or monster that gets you to the
	// room, each opening its record.
	import type { ColDef } from 'ag-grid-community';
	import DataGrid from '$lib/grid/DataGrid.svelte';
	import { Button } from '$lib/components/ui/button';
	import type { LeadRow, LeadsHere } from '$lib/map';

	let {
		result,
		onpick,
		onclose
	}: { result: LeadsHere | null; onpick: (r: LeadRow) => void; onclose: () => void } = $props();
	const cols: ColDef<LeadRow>[] = [{ colId: 'text', headerName: 'Result', field: 'text', flex: 1 }];
	const rows = $derived(result?.rows ?? []);
</script>

<div class="bg-background/80 fixed inset-0 z-[60] flex items-center justify-center p-4" role="presentation">
	<div
		class="bg-card flex h-[70vh] w-full max-w-2xl flex-col gap-2 rounded-lg border p-4 shadow-xl"
		role="dialog"
		aria-modal="true"
		aria-label="What Leads Here?"
		data-testid="leads-dialog"
	>
		<h2 class="text-base font-semibold" data-testid="leads-caption">{result?.caption ?? 'What Leads Here?'}</h2>
		{#if !result || result.nothing || rows.length === 0}
			<p class="text-sm" data-testid="leads-nothing">Nothing.</p>
		{:else}
			<div class="min-h-0 flex-1">
				<DataGrid
					{rows}
					columnDefs={cols}
					getRowId={(r: LeadRow) => r.kind + r.tag + r.text}
					selectedId={undefined}
					storageKey="map-leads-here"
					onSelect={onpick}
				/>
			</div>
		{/if}
		<div class="flex justify-end">
			<Button size="sm" variant="outline" onclick={onclose} data-testid="leads-close">Close</Button>
		</div>
	</div>
</div>
