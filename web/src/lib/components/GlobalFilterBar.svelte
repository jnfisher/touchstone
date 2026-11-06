<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type ClassRec, type Race } from '$lib/api';
	import { ALIGNMENTS, boxNumber } from '$lib/character';
	import { getCharacterStore } from '$lib/character.svelte';
	import { characterReady, isFiltering, MAX_BOX } from '$lib/globalfilter';
	import { getGlobalFilterStore } from '$lib/globalfilter.svelte';

	// MME's global filter frame: it applies to every explorer list at once. "Use character"
	// mirrors the Character tab (class, race, level, alignment are then its own and read-only
	// here); off, the boxes are the filter's. The defaults filter nothing.
	const bar = getGlobalFilterStore();
	const character = getCharacterStore();
	let classes = $state.raw<ClassRec[]>([]);
	let races = $state.raw<Race[]>([]);
	onMount(async () => {
		try {
			[classes, races] = await Promise.all([api.table('classes'), api.table('races')]);
		} catch {
			// the lists stay empty; the explorers report the missing data themselves
		}
	});

	const f = $derived(bar.filter);
	const ready = $derived(characterReady(character.doc));
	const mirroring = $derived(f.use_character && ready);
	const doc = $derived(character.doc);
	const shown = $derived(
		mirroring
			? { class: doc.class ?? 0, race: doc.race ?? 0, level: doc.level, alignment: doc.alignment }
			: { class: f.class, race: f.race, level: f.level, alignment: f.alignment }
	);
	const active = $derived(isFiltering(f, doc));

	const field =
		'bg-background border-input focus-visible:ring-ring/50 h-7 rounded-md border px-2 text-sm tabular-nums outline-none focus-visible:ring-2 disabled:opacity-60';
	const lab = 'text-muted-foreground flex items-center gap-1.5 text-xs font-medium';
</script>

<div
	class="flex flex-wrap items-center gap-x-4 gap-y-1 border-b px-4 py-1"
	role="group"
	aria-label="Global filter"
	data-testid="global-filter"
	data-active={active}
>
	<span class="text-xs font-semibold tracking-wide uppercase {active ? 'text-primary' : 'text-muted-foreground'}">
		Global filter
	</span>
	<label class="flex items-center gap-1.5 text-sm" title={ready ? '' : 'Pick a class and race on the Character tab'}>
		<input
			type="checkbox"
			checked={mirroring}
			disabled={!ready}
			onchange={(e) => bar.set({ use_character: e.currentTarget.checked })}
			data-testid="gf-use-character"
		/>
		Use character{#if mirroring}<span class="text-muted-foreground">({doc.name || 'unnamed'})</span>{/if}
	</label>
	<label class={lab}>
		Class
		<select
			class={field}
			value={shown.class}
			disabled={mirroring}
			onchange={(e) => bar.set({ class: Number(e.currentTarget.value) })}
			data-testid="gf-class"
		>
			<option value={0}>(any)</option>
			{#each classes as c (c.number)}<option value={c.number}>{c.name}</option>{/each}
		</select>
	</label>
	<label class={lab}>
		Race
		<select
			class={field}
			value={shown.race}
			disabled={mirroring}
			onchange={(e) => bar.set({ race: Number(e.currentTarget.value) })}
			data-testid="gf-race"
		>
			<option value={0}>(any)</option>
			{#each races as r (r.number)}<option value={r.number}>{r.name}</option>{/each}
		</select>
	</label>
	<label class={lab}>
		Level
		<input
			type="text"
			inputmode="numeric"
			maxlength="3"
			class="{field} w-14"
			value={shown.level}
			disabled={mirroring}
			oninput={(e) => bar.set({ level: boxNumber(e.currentTarget.value) })}
			data-testid="gf-level"
		/>
	</label>
	<label class={lab}>
		Alignment
		<select
			class={field}
			value={shown.alignment}
			disabled={mirroring}
			onchange={(e) => bar.set({ alignment: Number(e.currentTarget.value) })}
			data-testid="gf-alignment"
		>
			{#each ALIGNMENTS as a (a.value)}<option value={a.value}>{a.label}</option>{/each}
		</select>
	</label>
	<label class={lab}>
		Min item level
		<input
			type="text"
			inputmode="numeric"
			maxlength="3"
			class="{field} w-14"
			value={f.min_item_level}
			oninput={(e) => bar.set({ min_item_level: boxNumber(e.currentTarget.value) })}
			data-testid="gf-min-level"
		/>
	</label>
	<button
		type="button"
		class="bg-background border-input hover:bg-accent h-7 rounded-md border px-2 text-xs"
		onclick={() => bar.reset()}
		data-testid="gf-reset"
		title="Level {MAX_BOX}, any class, race and alignment: nothing filtered"
	>
		Reset
	</button>
</div>
