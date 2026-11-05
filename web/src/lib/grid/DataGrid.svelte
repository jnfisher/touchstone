<script lang="ts" generics="T extends object">
	import {
		AllCommunityModule,
		ModuleRegistry,
		createGrid,
		themeQuartz,
		type ColDef,
		type GridApi
	} from 'ag-grid-community';
	import { onMount } from 'svelte';

	ModuleRegistry.registerModules([AllCommunityModule]);

	// Every colour is a CSS variable from app.css, so the grid follows the dark/light class
	// on <html> with no per-theme code here.
	const theme = themeQuartz.withParams({
		backgroundColor: 'var(--card)',
		foregroundColor: 'var(--foreground)',
		headerBackgroundColor: 'var(--muted)',
		headerTextColor: 'var(--foreground)',
		borderColor: 'var(--border)',
		oddRowBackgroundColor: 'var(--background)',
		rowHoverColor: 'var(--accent)',
		selectedRowBackgroundColor: 'var(--accent)',
		accentColor: 'var(--ring)',
		inputBorder: { color: 'var(--input)' },
		browserColorScheme: 'inherit'
	});

	let { rows, columnDefs }: { rows: T[]; columnDefs: ColDef<T>[] } = $props();

	let host: HTMLDivElement;
	let api: GridApi<T> | undefined;

	onMount(() => {
		api = createGrid<T>(host, {
			theme,
			columnDefs,
			rowData: rows,
			defaultColDef: { sortable: true, filter: true, resizable: true },
			multiSortKey: 'ctrl',
			enableCellTextSelection: true
		});
		return () => api?.destroy();
	});

	$effect(() => {
		api?.setGridOption('rowData', rows);
	});
	$effect(() => {
		api?.setGridOption('columnDefs', columnDefs);
	});
</script>

<div bind:this={host} class="h-full w-full"></div>
