<script lang="ts">
	import { questOptionLabels, QUESTS } from '$lib/character';
	import type { CharacterStore } from '$lib/character.svelte';
	import { headingCls, inputCls } from '../calc/styles';

	let { store }: { store: CharacterStore } = $props();
	const doc = $derived(store.doc);
	// The server runs the stock engine, which ignores the later quests.
	const stock = QUESTS.filter((q) => q.stock);
</script>

<div>
	<h3 class={headingCls}>Quests</h3>
	<div class="flex flex-col gap-1.5 text-sm">
		{#each stock as q (q.index)}
			<div class="flex flex-wrap items-center gap-2">
				<label class="flex min-w-48 items-center gap-2">
					<input
						type="checkbox"
						checked={doc.quests[q.index]}
						onchange={(e) =>
							store.edit((d) => {
								d.quests[q.index] = e.currentTarget.checked;
								// MME clears a quest's choice when the box is unticked.
								if (!e.currentTarget.checked && q.option !== undefined) d.quest_options[q.option] = 0;
							})}
						data-testid="char-quest-{q.index}"
					/>
					{q.label}
				</label>
				{#if q.option !== undefined && doc.quests[q.index]}
					<select
						value={doc.quest_options[q.option]}
						onchange={(e) => store.edit((d) => (d.quest_options[q.option!] = Number(e.currentTarget.value)))}
						data-testid="char-quest-opt-{q.option}"
						class="{inputCls} w-auto"
					>
						{#each questOptionLabels(q.option) as label, i (i)}<option value={i}>{label}</option>{/each}
					</select>
				{/if}
			</div>
		{/each}
	</div>
</div>
