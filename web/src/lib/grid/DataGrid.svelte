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

	let {
		rows,
		columnDefs,
		getRowId,
		selectedId,
		quickFilter = '',
		onSelect,
		onCount
	}: {
		rows: T[];
		columnDefs: ColDef<T>[];
		getRowId?: (row: T) => string;
		/** Row id to show as selected; changes are applied without echoing back through onSelect. */
		selectedId?: string;
		quickFilter?: string;
		onSelect?: (row: T) => void;
		/** Rows left after filtering. */
		onCount?: (n: number) => void;
	} = $props();

	let host: HTMLDivElement;
	let api: GridApi<T> | undefined;

	function select(id: string | undefined) {
		if (!api || id === undefined) return;
		const node = api.getRowNode(id);
		if (!node) return;
		if (!node.isSelected()) node.setSelected(true, true, 'api');
		if (node.rowIndex !== null) api.ensureIndexVisible(node.rowIndex);
	}

	onMount(() => {
		api = createGrid<T>(host, {
			theme,
			columnDefs,
			rowData: rows,
			getRowId: getRowId ? (p) => getRowId(p.data) : undefined,
			defaultColDef: { sortable: true, filter: true, resizable: true },
			multiSortKey: 'ctrl',
			enableCellTextSelection: true,
			rowSelection: { mode: 'singleRow', checkboxes: false, enableClickSelection: true },
			quickFilterText: quickFilter,
			onSelectionChanged: (e) => {
				const row = e.api.getSelectedRows()[0];
				if (row && e.source !== 'api') onSelect?.(row);
			},
			// Arrow keys move the focused cell; the selection (and detail pane) follows it.
			onCellFocused: (e) => {
				if (e.rowIndex === null || e.rowPinned) return;
				const node = e.api.getDisplayedRowAtIndex(e.rowIndex);
				if (node && !node.isSelected()) node.setSelected(true, true, 'rowClicked');
			},
			onModelUpdated: (e) => onCount?.(e.api.getDisplayedRowCount()),
			onFirstDataRendered: () => select(selectedId)
		});
		return () => api?.destroy();
	});

	$effect(() => {
		api?.setGridOption('quickFilterText', quickFilter);
	});
	$effect(() => {
		select(selectedId);
	});

	$effect(() => {
		api?.setGridOption('rowData', rows);
	});
	$effect(() => {
		api?.setGridOption('columnDefs', columnDefs);
	});
</script>

<div bind:this={host} class="h-full w-full"></div>
