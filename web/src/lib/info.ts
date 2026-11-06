// The data-version box (MME's `LoadInfo` fills `lblDatVer` and the "Database Version (Created ...)" frame
// from the info table). Shown in the header's info popover.
import type { Info } from './api';

export interface InfoLine {
	label: string;
	value: string;
}

/** `lblDatVer`: the dat file version ("v1.11l" is shown as "v1.11L"), led by the custom label of a custom database. */
export function versionLabel(v: Info['version']): string {
	const base = v.dat_file_version.toLowerCase() === 'v1.11l' ? 'v1.11L' : v.dat_file_version;
	// a stock database leaves Custom empty (MME stores Chr(0))
	const custom = v.custom.replace(/\0/g, '').trim();
	return custom ? `${custom} - ${base}` : base;
}

export function infoLines(info: Info): InfoLine[] {
	const lines: InfoLine[] = [
		{ label: 'Database version', value: versionLabel(info.version) },
		{ label: 'Created', value: [info.version.date, info.version.time].filter(Boolean).join(' ') },
		{ label: 'NMR version', value: info.version.nmr_version },
		{ label: 'Source', value: info.source.path ? `${info.source.kind}: ${info.source.path}` : info.source.kind }
	];
	for (const [name, n] of Object.entries(info.counts).sort(([a], [b]) => a.localeCompare(b))) {
		lines.push({ label: name, value: n.toLocaleString('en-US') });
	}
	return lines.filter((l) => l.value !== '');
}
