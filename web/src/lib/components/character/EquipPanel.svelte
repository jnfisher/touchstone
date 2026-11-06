<script lang="ts">
	// The EQ tab: the twenty slots with MME's pick lists, "equip anywhere" (InvenEquipItem), the red
	// warning flags and the stat totals. Every rule runs on the server.
	import type { Item } from '$lib/api';
	import {
		EQUIP_SLOTS,
		characterClient,
		derivedStatName,
		equippableChoices,
		flaggedSlots,
		toChoices,
		type EquipStateDoc
	} from '$lib/character';
	import type { CharacterStore } from '$lib/character.svelte';
	import { headingCls, labelCls } from '../calc/styles';
	import ItemPicker from './ItemPicker.svelte';

	let { store, items }: { store: CharacterStore; items: Item[] } = $props();

	/** The stats the EQ tab shows beside the slots (`lblInvenCharStat` indexes). */
	const TOTALS = [0, 1, 2, 3, 4, 10, 11, 30, 7, 8, 24, 31, 19, 12];

	let eqState = $state.raw<EquipStateDoc>();
	let messages = $state<string[]>([]);
	let problem = $state<string>();
	let use2nd = $state(false);
	let toggleOff = $state(true);

	const anywhere = $derived(equippableChoices(items));
	const perSlot = $derived(EQUIP_SLOTS.map((s) => toChoices(eqState?.choices[s.index] ?? [])));
	const flags = $derived(eqState?.flags ?? []);
	const flagged = $derived(flaggedSlots(flags));
	const ev = $derived(store.evaluated);

	// Lists depend on the filter and the character; flags also on what is worn.
	const key = $derived(
		JSON.stringify([
			store.doc.class,
			store.doc.race,
			store.doc.level,
			store.doc.alignment,
			store.doc.global_filter,
			store.options.min_item_level,
			store.doc.equipped
		])
	);

	$effect(() => {
		key;
		let alive = true;
		const t = setTimeout(async () => {
			try {
				const out = await characterClient.equipState({ ...$state.snapshot(store.doc), options: $state.snapshot(store.options) });
				if (alive) {
					eqState = out;
					problem = undefined;
				}
			} catch (e) {
				if (alive) problem = e instanceof Error ? e.message : String(e);
			}
		}, 120);
		return () => {
			alive = false;
			clearTimeout(t);
		};
	});

	async function act(item: number, opts: { slot?: number; unequip?: boolean } = {}) {
		try {
			const out = await characterClient.equip({
				character: { ...$state.snapshot(store.doc), options: $state.snapshot(store.options) },
				item,
				use_2nd_wrist: use2nd,
				...opts
			});
			messages = out.messages;
			problem = undefined;
			store.edit((d) => (d.equipped = out.character.equipped));
		} catch (e) {
			problem = e instanceof Error ? e.message : String(e);
		}
	}
</script>

<div data-testid="equip-panel">
	<div class="mb-2 flex flex-wrap items-center gap-x-4 gap-y-1">
		<h3 class="{headingCls} !mb-0">Equipment</h3>
		<label class="flex items-center gap-1.5 text-xs">
			<input type="checkbox" bind:checked={store.doc.global_filter} onchange={() => store.edit(() => {})} data-testid="equip-filter" />
			Only items this character can use
		</label>
		<label class="flex items-center gap-1.5 text-xs">
			<input type="checkbox" bind:checked={use2nd} data-testid="equip-2nd-wrist" /> New wrist items go to the second wrist
		</label>
		<button
			type="button"
			class="border-input ml-auto h-7 rounded-md border px-2 text-xs"
			onclick={() => store.edit((d) => (d.equipped = Array<number>(20).fill(0)))}
			data-testid="equip-clear">Empty all slots</button
		>
	</div>

	<div class="grid items-start gap-4 xl:grid-cols-[1fr_17rem]">
		<div>
			<div class="mb-3 max-w-xl">
				<ItemPicker
					choices={anywhere}
					value={0}
					label="Equip any item (it picks its own slot)"
					noneLabel="Search all armour and weapons..."
					testid="equip-anywhere"
					onpick={(n) => {
						if (n) void act(n, { unequip: toggleOff });
					}}
				/>
				<label class="mt-1 flex items-center gap-1.5 text-xs">
					<input type="checkbox" bind:checked={toggleOff} data-testid="equip-toggle" /> Choosing a worn item takes it off
				</label>
			</div>
			<div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
				{#each EQUIP_SLOTS as s (s.index)}
					<ItemPicker
						choices={perSlot[s.index]}
						value={store.doc.equipped[s.index]}
						label={s.label}
						flagged={flags[s.index]}
						testid="char-equip-{s.index}"
						onpick={(n) => void act(n, { slot: s.index })}
					/>
				{/each}
			</div>
			{#if flagged.length}
				<p class="text-destructive mt-2 text-xs" data-testid="equip-flags">
					Red: {flagged.join(', ')} (the character cannot use the item, or a two-handed weapon is paired with an off-hand item).
				</p>
			{/if}
			{#each messages as m (m)}<p class="text-muted-foreground mt-2 text-xs" data-testid="equip-message">{m}</p>{/each}
			{#if problem}<p class="text-destructive mt-2 text-xs" data-testid="equip-error">{problem}</p>{/if}
		</div>

		<div>
			<span class={labelCls}>Stat totals</span>
			{#if ev}
				<dl class="mt-1 text-sm" data-testid="equip-totals">
					{#each TOTALS as i (i)}
						<div class="flex justify-between gap-2 border-b py-0.5">
							<dt class="text-muted-foreground">{derivedStatName(i)}</dt>
							<dd class="tabular-nums" data-testid="equip-total-{i}">{ev.stats[i]}</dd>
						</div>
					{/each}
				</dl>
			{:else}
				<p class="text-muted-foreground mt-1 text-sm">Pick a class and race.</p>
			{/if}
		</div>
	</div>
</div>
