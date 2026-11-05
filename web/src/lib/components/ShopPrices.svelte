<script lang="ts">
	// The Shops tab's stock list with MME's buy/sell prices at a given Charm (PullShopDetail).
	// The prices come from the server; this only holds the inputs.
	import { api, type ShopPrices } from '$lib/api';
	import { shopView } from '$lib/explorer-state.svelte';

	let { shop }: { shop: number } = $props();

	let data = $state.raw<ShopPrices>();
	let error = $state<string>();
	let ticket = 0;

	// MME's box takes three digits.
	const clampCharm = (n: number) => (Number.isFinite(n) ? Math.min(999, Math.max(0, Math.trunc(n))) : 0);

	// MME computes prices only when a check box is ticked; with both clear charm counts as 0 and
	// the buy price is shown.
	const charm = $derived(shopView.buying || shopView.selling ? clampCharm(shopView.charm) : 0);
	const showSell = $derived(shopView.selling);

	$effect(() => {
		const n = shop;
		const c = charm;
		const mine = ++ticket;
		api
			.shopPrices(n, c)
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

	// Only one of the two checks can be on, as in MME.
	function setBuying(on: boolean) {
		shopView.buying = on;
		if (on) shopView.selling = false;
	}
	function setSelling(on: boolean) {
		shopView.selling = on;
		if (on) shopView.buying = false;
	}
	const note = $derived(
		!data || data.training
			? ''
			: showSell
				? data.sell_note
				: shopView.buying
					? data.buy_note
					: ''
	);
	const fmt = (n: number) => n.toLocaleString('en-US');
	const th = 'text-muted-foreground bg-card sticky top-0 px-2 py-0.5 text-left text-[11px] font-medium whitespace-nowrap';
	const td = 'px-2 py-0.5 whitespace-nowrap';
</script>

<section class="mt-4" data-testid="shop-prices">
	{#if data && !data.training}
		<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">Stock</h3>
		<div class="mb-2 flex flex-wrap items-center gap-x-3 gap-y-1.5">
			<label class="flex items-center gap-2">
				<span>Charm</span>
				<button
					type="button"
					class="bg-background border-input h-7 w-7 rounded-md border"
					aria-label="Decrease charm"
					onclick={() => (shopView.charm = clampCharm(shopView.charm - 1))}>-</button
				>
				<input
					type="number"
					min="0"
					max="999"
					data-testid="charm"
					value={shopView.charm}
					oninput={(e) => (shopView.charm = clampCharm(e.currentTarget.valueAsNumber))}
					class="bg-background border-input h-7 w-16 [appearance:textfield] rounded-md border px-2 text-center tabular-nums"
				/>
				<button
					type="button"
					class="bg-background border-input h-7 w-7 rounded-md border"
					aria-label="Increase charm"
					onclick={() => (shopView.charm = clampCharm(shopView.charm + 1))}>+</button
				>
			</label>
			<label class="flex items-center gap-1.5">
				<input
					type="checkbox"
					checked={shopView.buying}
					onchange={(e) => setBuying(e.currentTarget.checked)}
				/>
				Show Buying
			</label>
			<label class="flex items-center gap-1.5">
				<input
					type="checkbox"
					checked={shopView.selling}
					onchange={(e) => setSelling(e.currentTarget.checked)}
				/>
				Show Selling
			</label>
			{#if note}
				<span class="text-muted-foreground" data-testid="charm-note">{note}</span>
			{/if}
		</div>
	{:else if data?.training}
		<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">Training</h3>
	{/if}

	{#if error}
		<p class="text-destructive text-sm" role="alert">Could not load prices: {error}</p>
	{:else if !data}
		<p class="text-muted-foreground text-sm">Loading prices...</p>
	{:else if data.training}
		<div class="overflow-x-auto rounded-md border"><table class="w-max min-w-full" data-testid="train-table">
			<thead><tr><th class={th}>Level</th><th class={th}>Cost</th></tr></thead>
			<tbody>
				{#each data.levels as r (r.level)}
					<tr class="border-t"><td class="{td} tabular-nums">{r.level}</td><td class={td}>{r.cost}</td></tr>
				{/each}
			</tbody>
		</table></div>
	{:else if data.items.length === 0}
		<p class="text-muted-foreground text-sm">This shop stocks nothing.</p>
	{:else}
		<div class="overflow-x-auto rounded-md border"><table class="w-max min-w-full" data-testid="price-table">
			<thead>
				<tr>
					<th class={th}>#</th><th class={th}>Name</th><th class={th + ' text-right'}>Max</th>
					<th class={th}>Regen</th><th class={th}>Cost</th>
				</tr>
			</thead>
			<tbody>
				{#each data.items as r (r.slot)}
					<tr class="border-t">
						<td class="{td} text-muted-foreground tabular-nums">{r.item}</td>
						<td class={td}>{r.name}</td>
						<td class="{td} text-right tabular-nums">{fmt(r.max)}</td>
						<td class={td}>{r.regen}</td>
						<td class="{td} tabular-nums">{showSell ? r.sell : r.buy}</td>
					</tr>
				{/each}
			</tbody>
		</table></div>
	{/if}
</section>
