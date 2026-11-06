<script lang="ts">
	import type { Item } from '$lib/api';
	import { EQUIP_SLOTS, slotChoices } from '$lib/character';
	import type { CharacterStore } from '$lib/character.svelte';
	import { headingCls } from '../calc/styles';
	import ItemPicker from './ItemPicker.svelte';

	let { store, items }: { store: CharacterStore; items: Item[] } = $props();
	// Built once per item table, not per keystroke.
	const perSlot = $derived(EQUIP_SLOTS.map((s) => slotChoices(items, s)));
</script>

<div>
	<h3 class={headingCls}>Equipment</h3>
	<div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
		{#each EQUIP_SLOTS as s (s.index)}
			<ItemPicker
				choices={perSlot[s.index]}
				value={store.doc.equipped[s.index]}
				label={s.label}
				testid="char-equip-{s.index}"
				onpick={(n) => store.edit((d) => (d.equipped[s.index] = n))}
			/>
		{/each}
	</div>
</div>
