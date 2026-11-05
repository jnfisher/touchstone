<script lang="ts">
	import { page } from '$app/state';
	import DataGrid from '$lib/grid/DataGrid.svelte';
	import { SECTIONS } from '$lib/sections';
	import type { ColDef } from 'ag-grid-community';

	type Row = { id: number; name: string; level: number; damage: number };

	// Placeholder data to show that the grid scrolls 1000 rows smoothly.
	const rows: Row[] = Array.from({ length: 1000 }, (_, i) => ({
		id: i + 1,
		name: `Sample ${i + 1}`,
		level: (i * 7) % 50,
		damage: (i * 13) % 200
	}));
	const columnDefs: ColDef<Row>[] = [
		{ field: 'id', width: 90 },
		{ field: 'name', flex: 1 },
		{ field: 'level', width: 110 },
		{ field: 'damage', width: 110 }
	];

	const label = $derived(SECTIONS.find((s) => s.slug === page.params.section)?.label ?? '');
</script>

{#if page.params.section === 'weapons'}
	<DataGrid {rows} {columnDefs} />
{:else}
	<p class="text-muted-foreground">{label}: not built yet.</p>
{/if}
