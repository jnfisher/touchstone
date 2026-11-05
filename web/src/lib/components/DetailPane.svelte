<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { DetailGroup } from '$lib/explorers';

	let { groups, children }: { groups: DetailGroup[]; children?: Snippet } = $props();
</script>

<div class="bg-card h-full overflow-y-auto rounded-md border p-4" data-testid="detail">
	{#each groups as g, i (i)}
		<section class="mb-5 last:mb-0">
			<h3
				class={i === 0
					? 'mb-2 text-base font-semibold'
					: 'text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase'}
			>
				{g.title}
			</h3>
			<dl class="grid grid-cols-[minmax(7rem,auto)_1fr] gap-x-4 gap-y-1 text-sm">
				{#each g.rows as r, j (j)}
					<dt class="text-muted-foreground">{r.label}</dt>
					<dd class={r.long ? 'break-words whitespace-pre-wrap' : 'tabular-nums'}>{r.value}</dd>
				{/each}
			</dl>
		</section>
	{/each}
	{@render children?.()}
</div>
