<script lang="ts">
	// Ten bless slots. The choices come from the server (`FilterBlessSpells` needs
	// `SpellIsUsable`, which is not a plain predicate), refetched when class, level or alignment change.
	import { BLESS_SLOT_COUNT, characterClient, type BlessChoice } from '$lib/character';
	import type { CharacterStore } from '$lib/character.svelte';
	import { headingCls, inputCls, labelCls } from '../calc/styles';

	let { store }: { store: CharacterStore } = $props();
	const doc = $derived(store.doc);

	let all = $state(false);
	let choices = $state.raw<BlessChoice[]>([]);
	let error = $state<string>();

	$effect(() => {
		const q = {
			class: doc.class ?? 0,
			level: doc.level,
			alignment: doc.alignment,
			all,
			include: $state.snapshot(doc.bless)
		};
		let alive = true;
		const t = setTimeout(() => {
			characterClient
				.blessSpells(q)
				.then((c) => {
					if (!alive) return;
					choices = c;
					error = undefined;
				})
				.catch((e: unknown) => alive && (error = e instanceof Error ? e.message : String(e)));
		}, 100);
		return () => {
			alive = false;
			clearTimeout(t);
		};
	});
</script>

<div>
	<div class="mb-2 flex items-center justify-between">
		<h3 class="{headingCls} !mb-0">Bless</h3>
		<label class="flex items-center gap-1.5 text-xs">
			<input type="checkbox" bind:checked={all} data-testid="char-bless-all" />
			Show every bless spell
		</label>
	</div>
	<div class="grid grid-cols-2 gap-2">
		{#each { length: BLESS_SLOT_COUNT } as _, i (i)}
			<label class="flex flex-col gap-1">
				<span class={labelCls}>Slot {i + 1}</span>
				<select
					value={doc.bless[i]}
					onchange={(e) => store.edit((d) => (d.bless[i] = Number(e.currentTarget.value)))}
					data-testid="char-bless-{i}"
					class={inputCls}
				>
					<option value={0}>(none)</option>
					{#each choices as c (c.number)}<option value={c.number}>{c.label}</option>{/each}
				</select>
			</label>
		{/each}
	</div>
	{#if store.evaluated?.bless_mana}
		<p class="text-muted-foreground mt-2 text-xs">Bless mana per six rounds: {store.evaluated.bless_mana}</p>
	{/if}
	{#if error}<p class="text-destructive mt-1 text-xs">{error}</p>{/if}
	<button
		type="button"
		class="text-muted-foreground hover:text-foreground mt-2 text-xs underline"
		onclick={() => store.edit((d) => d.bless.fill(0))}
		data-testid="char-bless-reset">Clear all slots</button
	>
</div>
