<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { DetailGroup } from '$lib/explorers';
	import { groupRows, refCells, refColumns, type Pane, type RefRow, type RefTarget } from '$lib/detail';
	import { lineColorClass } from '$lib/monsters';

	let {
		groups,
		pane,
		error,
		compare = $bindable(true),
		canCompare = false,
		onLink,
		linkable,
		onMenu,
		before,
		children
	}: {
		groups: DetailGroup[];
		/** The server's pane for the record (`/api/detail`); absent while it loads. */
		pane?: Pane;
		error?: string;
		/** Show the "Compared to <equipped item>" blocks. */
		compare?: boolean;
		/** Weapons and armour offer the compare toggle. */
		canCompare?: boolean;
		onLink?: (r: { target: RefTarget | null }) => void;
		linkable?: (r: RefRow) => boolean;
		/** A right-click on a monster line, a shop stock line or a reference row (its index in that list). */
		onMenu?: (e: { part: 'line' | 'shopLine' | 'ref'; index: number }) => void;
		/** Controls that sit above the rows (the shop's charm box). */
		before?: Snippet;
		children?: Snippet;
	} = $props();
	const head = $derived(groups[0]);
	const paneGroups = $derived(pane ? groupRows(pane.rows) : []);
	const cols = $derived(pane ? refColumns(pane.references) : 0);
</script>

<div class="bg-card h-full overflow-auto rounded-md border text-[13px] leading-5" data-testid="detail">
	{#if head}
		<!-- The record's name stays in view while the rest scrolls. -->
		<h3 class="bg-card sticky top-0 z-10 truncate border-b px-3 py-2 text-sm font-semibold" title={head.title}>
			{head.title}
		</h3>
	{/if}
	<div class="p-3">
		{@render before?.()}
		{#each groups as g, i (i)}
			{#if g.rows.length}
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
			{/if}
		{/each}

		{#if error}
			<p class="text-destructive mb-3" role="alert">Could not load the details: {error}</p>
		{:else if !pane && head}
			<p class="text-muted-foreground mb-3" data-testid="pane-loading">Loading details...</p>
		{/if}

		{#if pane?.lines?.length}
			<!-- The monster pane: MME's lvMonsterDetail, line by line with its own colours and bold. -->
			<dl class="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-0.5" data-testid="pane-lines">
				{#each pane.lines as l, k (k)}
					{#if !l.label && !l.cells.some((c) => c.text)}
						<div class="col-span-2 h-1.5" aria-hidden="true"></div>
					{:else}
						<dt oncontextmenu={() => onMenu?.({ part: 'line', index: k })} class="text-muted-foreground whitespace-nowrap {l.bold ? 'font-semibold' : ''} {lineColorClass(l.color)}">
							{l.label}
						</dt>
						<dd oncontextmenu={() => onMenu?.({ part: 'line', index: k })} class="min-w-0 break-words whitespace-pre-wrap">
							{#each l.cells as c, j (j)}
								{#if l.target && onLink && j === 0}
									<button
										type="button"
										class="text-primary text-left hover:underline {c.bold ? 'font-semibold' : ''}"
										data-testid="pane-link"
										onclick={() => onLink({ target: l.target })}
									>
										{c.text}
									</button>
								{:else}
									<span class="{c.bold ? 'font-semibold' : ''} {lineColorClass(c.color)}">{c.text}</span>
								{/if}
								{#if j < l.cells.length - 1}<br />{/if}
							{/each}
						</dd>
					{/if}
				{/each}
			</dl>
		{/if}

		{#if pane}
			{#if canCompare && pane.compare.length}
				<label class="mb-3 flex items-center gap-1.5">
					<input type="checkbox" bind:checked={compare} data-testid="compare-toggle" />
					Compare with equipped
				</label>
			{/if}
			{#each paneGroups as g (g.title)}
				<section class="mb-4" data-testid="pane-group">
					{#if g.title}
						<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">{g.title}</h3>
					{/if}
					<dl class="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-0.5">
						{#each g.rows as r, j (j)}
							{#if r.label}
								<dt oncontextmenu={() => r.kind === 'shop_line' && onMenu?.({ part: 'shopLine', index: j })} class="text-muted-foreground whitespace-nowrap">{r.label}</dt>
								<dd oncontextmenu={() => r.kind === 'shop_line' && onMenu?.({ part: 'shopLine', index: j })} class="break-words whitespace-pre-wrap">{r.value}</dd>
							{:else}
								<dd class="col-span-2 break-words whitespace-pre-wrap">{r.value}</dd>
							{/if}
						{/each}
					</dl>
				</section>
			{/each}
			{#if canCompare && compare && pane.compare.length}
				<section class="mb-4" data-testid="pane-compare">
					{#each pane.compare as c, k (k)}
						<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">{c.label}</h3>
						<p class="mb-2 break-words whitespace-pre-wrap">{c.text}</p>
					{/each}
				</section>
			{/if}
			{#if pane.references.length}
				<section class="mb-4" data-testid="pane-references">
					<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">Where it is found</h3>
					<div class="overflow-x-auto rounded-md border">
						<table class="w-max min-w-full">
							<tbody>
								{#each pane.references as r, k (k)}
									<tr class="border-t first:border-t-0" oncontextmenu={() => onMenu?.({ part: 'ref', index: k })}>
										{#each refCells(r, cols) as cell, c (c)}
											<td class="px-2 py-0.5 {c === 0 && cols > 1 ? 'text-muted-foreground tabular-nums' : ''}">
												{#if cell && onLink && r.target && (linkable?.(r) ?? true) && c === cols - 1}
													<button type="button" class="text-primary text-left hover:underline" onclick={() => onLink(r)}>
														{cell}
													</button>
												{:else}
													{cell}
												{/if}
											</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</section>
			{/if}
		{/if}
		{@render children?.()}
	</div>
</div>
