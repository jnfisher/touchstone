<script lang="ts">
	import type { ClassRec, Race } from '$lib/api';
	import { ALIGNMENTS, boxNumber } from '$lib/character';
	import type { CharacterStore } from '$lib/character.svelte';
	import { headingCls, inputCls, labelCls } from '../calc/styles';

	let { store, classes, races }: { store: CharacterStore; classes: ClassRec[]; races: Race[] } = $props();
	const doc = $derived(store.doc);
</script>

<div>
	<h3 class={headingCls}>Character</h3>
	<div class="grid grid-cols-2 gap-3">
		<label class="col-span-2 flex flex-col gap-1">
			<span class={labelCls}>Name</span>
			<input
				type="text"
				autocomplete="off"
				maxlength="64"
				value={doc.name}
				oninput={(e) => store.edit((d) => (d.name = e.currentTarget.value))}
				data-testid="char-name"
				class={inputCls}
			/>
		</label>
		<label class="flex flex-col gap-1">
			<span class={labelCls}>Class</span>
			<select
				value={doc.class ?? ''}
				onchange={(e) => store.edit((d) => (d.class = e.currentTarget.value === '' ? null : Number(e.currentTarget.value)))}
				data-testid="char-class"
				class={inputCls}
			>
				<option value="">Pick a class</option>
				{#each classes as c (c.number)}<option value={c.number}>{c.name}</option>{/each}
			</select>
		</label>
		<label class="flex flex-col gap-1">
			<span class={labelCls}>Race</span>
			<select
				value={doc.race ?? ''}
				onchange={(e) => store.edit((d) => (d.race = e.currentTarget.value === '' ? null : Number(e.currentTarget.value)))}
				data-testid="char-race"
				class={inputCls}
			>
				<option value="">Pick a race</option>
				{#each races as r (r.number)}<option value={r.number}>{r.name}</option>{/each}
			</select>
		</label>
		<label class="flex flex-col gap-1">
			<span class={labelCls}>Level</span>
			<input
				type="text"
				inputmode="numeric"
				value={doc.level}
				oninput={(e) => store.edit((d) => (d.level = boxNumber(e.currentTarget.value)))}
				data-testid="char-level"
				class={inputCls}
			/>
		</label>
		<label class="flex flex-col gap-1">
			<span class={labelCls}>Alignment</span>
			<select
				value={doc.alignment}
				onchange={(e) => store.edit((d) => (d.alignment = Number(e.currentTarget.value)))}
				data-testid="char-alignment"
				class={inputCls}
			>
				{#each ALIGNMENTS as a (a.value)}<option value={a.value}>{a.label}</option>{/each}
			</select>
		</label>
	</div>
</div>
