<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { DetailGroup } from '$lib/explorers';

	let { groups, children }: { groups: DetailGroup[]; children?: Snippet } = $props();
	const head = $derived(groups[0]);
</script>

<div class="bg-card h-full overflow-auto rounded-md border text-[13px] leading-5" data-testid="detail">
	{#if head}
		<!-- The record's name stays in view while the rest scrolls. -->
		<h3 class="bg-card sticky top-0 z-10 truncate border-b px-3 py-2 text-sm font-semibold" title={head.title}>
			{head.title}
		</h3>
	{/if}
	<div class="p-3">
		{#each groups as g, i (i)}
			<section class="mb-4 last:mb-0">
				{#if i > 0}
					<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">
						{g.title}
					</h3>
				{/if}
				<dl class="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-0.5">
					{#each g.rows as r, j (j)}
						<dt class="text-muted-foreground whitespace-nowrap">{r.label}</dt>
						<dd class={r.long ? 'break-words whitespace-pre-wrap' : 'tabular-nums break-words'}>{r.value}</dd>
					{/each}
				</dl>
			</section>
		{/each}
		{@render children?.()}
	</div>
</div>
