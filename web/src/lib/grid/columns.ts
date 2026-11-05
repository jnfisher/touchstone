import type { ColDef } from 'ag-grid-community';
import type { ColumnSpec } from '../explorers';

/** Turn an explorer's column specs into AG Grid column definitions. */
export function toColDefs<T extends object>(
	cols: ColumnSpec<T>[],
	sort?: { col: string; dir: 'asc' | 'desc' }
): ColDef<T>[] {
	return cols.map((c) => {
		const def: ColDef<T> = {
			colId: c.id,
			headerName: c.header,
			width: c.width,
			valueGetter: (p) => (p.data ? c.get(p.data) : undefined)
		};
		if (c.format) {
			const format = c.format;
			def.valueFormatter = (p) => (p.data ? format(p.value, p.data) : '');
		}
		if (c.numeric) {
			def.filter = 'agNumberColumnFilter';
			def.type = 'rightAligned';
		}
		if (c.sortKey) {
			const key = c.sortKey;
			def.comparator = (_a, _b, na, nb) => (na.data && nb.data ? key(na.data) - key(nb.data) : 0);
		}
		if (sort?.col === c.id) def.sort = sort.dir;
		return def;
	});
}
