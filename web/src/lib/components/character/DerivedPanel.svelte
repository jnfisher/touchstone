<script lang="ts">
	import { derivedStatName } from '$lib/character';
	import type { EvalState } from '$lib/character';
	import { headingCls } from '../calc/styles';

	let { ev }: { ev: EvalState | undefined } = $props();
	const rows = $derived((ev?.stats ?? []).map((v, i) => ({ i, name: derivedStatName(i), v })).filter((r) => r.i < 43 || r.v !== ''));
</script>

<div>
	<h3 class={headingCls}>Derived stats</h3>
	{#if ev}
		<dl class="grid grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-x-6 gap-y-1 text-sm" data-testid="char-derived">
			{#each rows as r (r.i)}
				<div class="flex justify-between gap-2 border-b py-0.5">
					<dt class="text-muted-foreground">{r.name}</dt>
					<dd class="tabular-nums" data-testid="char-derived-{r.i}">{r.v}</dd>
				</div>
			{/each}
		</dl>
	{:else}
		<p class="text-muted-foreground text-sm">Pick a class and race.</p>
	{/if}
</div>
