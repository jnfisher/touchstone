<script lang="ts">
	// Damage, heal, duration and effect text of a spell at a caster level (GetSpellMinDamage,
	// GetSpellMaxDamage, GetSpellDuration, PullSpellEQ). Starts at the spell's required level.
	import { untrack } from 'svelte';
	import { api, type SpellEffect } from '$lib/api';

	let { number, reqLevel }: { number: number; reqLevel: number } = $props();

	let level = $state<number | null>(untrack(() => reqLevel));
	let data = $state.raw<SpellEffect>();
	let error = $state<string>();
	let ticket = 0;

	$effect(() => {
		const n = number;
		const l = level;
		if (l === null || !Number.isInteger(l) || l < 0 || l > 32767) return;
		const mine = ++ticket;
		api
			.spellEffect(n, l)
			.then((d) => {
				if (mine !== ticket) return;
				data = d;
				error = undefined;
			})
			.catch((e) => {
				if (mine !== ticket) return;
				error = e instanceof Error ? e.message : String(e);
			});
	});

	const range = (r: { min: number; max: number }) => (r.min === r.max ? String(r.min) : `${r.min} - ${r.max}`);
</script>

<section class="mt-4" data-testid="spell-effect">
	<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">At caster level</h3>
	<label class="mb-2 flex items-center gap-2">
		<span>Level</span>
		<input
			type="number"
			min="0"
			max="32767"
			bind:value={level}
			data-testid="spell-level"
			class="bg-background border-input h-7 w-20 rounded-md border px-2 text-center tabular-nums"
		/>
	</label>
	{#if error}
		<p class="text-destructive text-sm" role="alert">Could not load the spell numbers: {error}</p>
	{:else if data}
		<dl class="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-0.5" data-testid="spell-numbers">
			{#if data.used_level !== data.level}
				<dt class="text-muted-foreground whitespace-nowrap">Level used</dt>
				<dd class="tabular-nums">{data.used_level} (required {data.req_level}{data.cap ? `, cap ${data.cap}` : ''})</dd>
			{/if}
			{#if data.damage}
				<dt class="text-muted-foreground whitespace-nowrap">Damage / round</dt>
				<dd class="tabular-nums" data-testid="spell-damage">{range(data.damage)}</dd>
			{/if}
			{#if data.heal}
				<dt class="text-muted-foreground whitespace-nowrap">Heal / round</dt>
				<dd class="tabular-nums">{range(data.heal)}</dd>
			{/if}
			<dt class="text-muted-foreground whitespace-nowrap">Duration</dt>
			<dd class="tabular-nums" data-testid="spell-duration">
				{data.duration > 0 ? `${data.duration} rounds` : 'Instant'}
			</dd>
			{#if data.effect}
				<dt class="text-muted-foreground whitespace-nowrap">Effect</dt>
				<dd class="break-words whitespace-pre-wrap" data-testid="spell-eq">{data.effect}</dd>
			{/if}
		</dl>
	{/if}
</section>
