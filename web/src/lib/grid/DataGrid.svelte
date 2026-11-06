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
	import { findNext, fitWidth, loadWidths, saveWidths, toTsv, type Widths } from './helpers';

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
		storageKey = 'default',
		rowState,
		onSelect,
		onCount,
		onWidth
	}: {
		rows: T[];
		columnDefs: ColDef<T>[];
		getRowId?: (row: T) => string;
		/** Row id to show as selected; changes are applied without echoing back through onSelect. */
		selectedId?: string;
		quickFilter?: string;
		/** Names this grid's saved column widths; one per section. */
		storageKey?: string;
		/** Row colouring hook (MME colours rows by state, e.g. not in game); themed tokens only. */
		rowState?: (row: T) => 'muted' | 'danger' | undefined;
		onSelect?: (row: T) => void;
		/** Rows left after filtering. */
		onCount?: (n: number) => void;
		/** Total width of the columns plus scrollbar, once sized; the list pane's natural width. */
		onWidth?: (w: number) => void;
	} = $props();

	let host: HTMLDivElement;
	let api: GridApi<T> | undefined;

	const stored = () => (typeof localStorage === 'undefined' ? undefined : localStorage);

	/** Saved width if the user resized the column, else a width that fits header and every value. */
	function sizeColumns() {
		if (!api) return;
		const cs = getComputedStyle(host.querySelector('.ag-cell') ?? host);
		const ctx = document.createElement('canvas').getContext('2d');
		const measure = ctx
			? (t: string) => {
					ctx.font = cs.font || '13px sans-serif';
					return ctx.measureText(t).width;
				}
			: (t: string) => t.length * 8;
		const saved = loadWidths(stored(), storageKey);
		const nodes: NonNullable<ReturnType<GridApi<T>['getRowNode']>>[] = [];
		api.forEachNode((n) => nodes.push(n));
		const widths = api.getAllDisplayedColumns().map((col) => {
			const key = col.getColId();
			if (saved[key]) return { key, newWidth: saved[key] };
			const header = col.getColDef().headerName ?? key;
			const values = nodes.map((n) => String(api!.getCellValue({ rowNode: n, colKey: key, useFormatter: true }) ?? ''));
			return { key, newWidth: fitWidth(header, values, measure) };
		});
		api.setColumnWidths(widths, true, 'api');
		onWidth?.(widths.reduce((a, w) => a + w.newWidth, 0) + 20);
	}

	function currentWidths(): Widths {
		const w: Widths = {};
		for (const c of api?.getAllDisplayedColumns() ?? []) w[c.getColId()] = c.getActualWidth();
		return w;
	}

	function copySelected() {
		if (!api) return false;
		const cols = api.getAllDisplayedColumns();
		const rows = api.getSelectedNodes().map((n) =>
			cols.map((c) => String(api!.getCellValue({ rowNode: n, colKey: c.getColId(), useFormatter: true }) ?? ''))
		);
		if (!rows.length) return false;
		const text = toTsv(cols.map((c) => c.getColDef().headerName ?? c.getColId()), rows);
		void navigator.clipboard?.writeText(text).catch(() => {});
		return true;
	}

	// Find-as-you-type on the name column: typing jumps to the first name starting with the text,
	// Right arrow to the next match (MME's find box).
	let find = '';
	let findTimer: ReturnType<typeof setTimeout> | undefined;

	function jump(from: number) {
		if (!api) return;
		const col = api.getColumn('name') ?? api.getAllDisplayedColumns()[0];
		if (!col) return;
		const names: string[] = [];
		api.forEachNodeAfterFilterAndSort((n) => names.push(String(api!.getCellValue({ rowNode: n, colKey: col.getColId() }) ?? '')));
		const i = findNext(names, find, from);
		if (i < 0) return;
		const node = api.getDisplayedRowAtIndex(i);
		api.ensureIndexVisible(i);
		api.setFocusedCell(i, col);
		node?.setSelected(true, true, 'rowClicked');
	}

	function onKeydown(e: KeyboardEvent) {
		if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') {
			if (!window.getSelection()?.toString() && copySelected()) e.preventDefault();
			return;
		}
		if (e.ctrlKey || e.metaKey || e.altKey) return;
		const target = e.target as HTMLElement;
		if (target.closest('input, textarea, select, .ag-popup')) return;
		const cur = api?.getFocusedCell()?.rowIndex ?? 0;
		if (e.key.length === 1 && e.key !== ' ') {
			find += e.key;
			clearTimeout(findTimer);
			findTimer = setTimeout(() => (find = ''), 1500);
			// Start at the current row so adding a letter keeps the match when it still fits.
			jump(cur);
			e.preventDefault();
			e.stopPropagation();
		} else if (e.key === 'ArrowRight' && find) {
			clearTimeout(findTimer);
			findTimer = setTimeout(() => (find = ''), 6000);
			jump(cur + 1);
			e.preventDefault();
			e.stopPropagation();
		} else if (e.key === 'Escape' || e.key === 'ArrowUp' || e.key === 'ArrowDown') {
			find = '';
		}
	}

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
			defaultColDef: { sortable: true, filter: true, resizable: true, minWidth: 48 },
			getRowClass: (p) => {
				const st = p.data && rowState?.(p.data);
				return st ? `ts-row-${st}` : undefined;
			},
			onColumnResized: (e) => {
				if (e.finished && e.source === 'uiColumnResized') saveWidths(stored(), storageKey, currentWidths());
			},
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
			onFirstDataRendered: () => {
				sizeColumns();
				select(selectedId);
			}
		});
		host.addEventListener('keydown', onKeydown, true);
		return () => {
			host.removeEventListener('keydown', onKeydown, true);
			clearTimeout(findTimer);
			api?.destroy();
		};
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

<style>
	/* Row states use theme tokens only, so they follow dark and light. */
	:global(.ag-row.ts-row-muted .ag-cell) {
		color: var(--muted-foreground);
	}
	:global(.ag-row.ts-row-danger .ag-cell) {
		color: var(--destructive);
	}
	/* MME colours race stat ranges well below / above the average red / green. */
	:global(.ag-cell.ts-cell-below) {
		color: var(--destructive);
	}
	:global(.ag-cell.ts-cell-above) {
		color: var(--positive);
	}
</style>
