<script lang="ts">
	import { boxNumber, STAT_NAMES } from '$lib/character';
	import type { CharacterStore } from '$lib/character.svelte';
	import { api } from '$lib/api';
	import { capNote, capOf, parseStat, statCapsClient, type StatCaps } from '$lib/statcaps';
	import { headingCls, inputCls } from '../calc/styles';

	let { store }: { store: CharacterStore } = $props();
	const doc = $derived(store.doc);
	const ev = $derived(store.evaluated);
	const cps = $derived(ev?.cps);

	function setStat(i: number, input: HTMLInputElement) {
		const n = parseStat(input.value, capOf(caps, i));
		store.edit((d) => (d.stats[i] = n));
		// a value past the training maximum was cut: the box shows what was kept
		if (n !== boxNumber(input.value)) input.value = String(n);
	}

	// Under the Quicksilver rules a stat can be trained past the race maximum, to a soft cap and 30 points beyond.
	let quicksilver = $state(false);
	let caps = $state.raw<StatCaps>();
	$effect(() => {
		let live = true;
		api
			.info()
			.then((i) => {
				if (live) quicksilver = i.profile?.name === 'quicksilver';
			})
			.catch(() => {});
		return () => (live = false);
	});
	$effect(() => {
		const race = doc.race;
		if (!quicksilver || race === null) {
			caps = undefined;
			return;
		}
		const ctl = new AbortController();
		statCapsClient
			.caps(race, ctl.signal)
			.then((c) => (caps = c))
			.catch(() => {
				if (!ctl.signal.aborted) caps = undefined;
			});
		return () => ctl.abort();
	});
</script>

<div>
	<h3 class={headingCls}>Stats</h3>
	<div class="grid {caps?.soft_caps ? 'grid-cols-[6rem_5rem_1fr_3rem_9rem]' : 'grid-cols-[6rem_5rem_1fr_3rem]'} items-center gap-x-2 gap-y-1.5 text-sm">
		<span></span><span class="text-muted-foreground text-xs">Base</span>
		<span class="text-muted-foreground text-xs">With bless and gear</span>
		<span class="text-muted-foreground text-xs">CP</span>
		{#if caps?.soft_caps}<span class="text-muted-foreground text-xs">Quicksilver range</span>{/if}
		{#each STAT_NAMES as name, i (name)}
			<span class="text-muted-foreground">{ev?.stat_labels[i] ?? `${name}:`}</span>
			<input
				type="text"
				inputmode="numeric"
				aria-label={name}
				value={doc.stats[i]}
				oninput={(e) => setStat(i, e.currentTarget)}
				data-testid="char-stat-{i}"
				class="{inputCls} w-16"
			/>
			<span class="tabular-nums" data-testid="char-stat-value-{i}">
				{ev ? ev.stat_values[i] : ''}
				{#if ev && ev.stat_bonus[i] !== 0}
					<span class="text-muted-foreground text-xs">({ev.stat_bonus[i] > 0 ? '+' : ''}{ev.stat_bonus[i]})</span>
				{/if}
			</span>
			<span class="text-muted-foreground text-xs tabular-nums">{cps ? cps.stat_costs[i] : ''}</span>
			{#if caps?.soft_caps}<span class="text-muted-foreground text-xs" data-testid="char-stat-cap-{i}">{capNote(capOf(caps, i))}</span>{/if}
		{/each}
	</div>
	{#if cps}
		<dl class="mt-3 grid grid-cols-2 gap-x-3 gap-y-1 text-sm" data-testid="char-cps">
			<dt class="text-muted-foreground">Total CP cost</dt><dd class="tabular-nums">{cps.total_cost}</dd>
			{#if cps.used !== null}
				<dt class="text-muted-foreground">CP used</dt><dd class="tabular-nums">{cps.used}</dd>
				<dt class="text-muted-foreground">CP available</dt><dd class="tabular-nums">{cps.avail}</dd>
				<dt class="text-muted-foreground">Level required</dt><dd class="tabular-nums">{cps.level_required}</dd>
			{/if}
			{#if cps.exp_kind === 'a_lot'}
				<dt class="text-muted-foreground">Exp required</dt><dd>a lot</dd>
			{:else if cps.exp_kind === 'value'}
				<dt class="text-muted-foreground">Exp required</dt><dd class="tabular-nums">{cps.exp}</dd>
			{/if}
		</dl>
		{#if cps.error_text}<p class="text-destructive mt-1 text-xs">{cps.error_text}</p>{/if}
	{/if}
</div>
