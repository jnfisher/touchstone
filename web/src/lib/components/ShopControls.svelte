<script lang="ts">
	// The Shops tab's charm box and buy/sell checks (MME's txtShopCharm, chkShopShowCharm). They only
	// hold the inputs; the pane refetches and the server prices the stock.
	import { shopView } from '$lib/explorer-state.svelte';

	let { note = '' }: { note?: string } = $props();

	// MME's box takes three digits.
	const clampCharm = (n: number) => (Number.isFinite(n) ? Math.min(999, Math.max(0, Math.trunc(n))) : 0);

	// Only one of the two checks can be on, as in MME.
	function setBuying(on: boolean) {
		shopView.buying = on;
		if (on) shopView.selling = false;
	}
	function setSelling(on: boolean) {
		shopView.selling = on;
		if (on) shopView.buying = false;
	}
</script>

<div class="mb-3 flex flex-wrap items-center gap-x-3 gap-y-1.5" data-testid="shop-controls">
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
		<input type="checkbox" checked={shopView.buying} onchange={(e) => setBuying(e.currentTarget.checked)} />
		Show Buying
	</label>
	<label class="flex items-center gap-1.5">
		<input type="checkbox" checked={shopView.selling} onchange={(e) => setSelling(e.currentTarget.checked)} />
		Show Selling
	</label>
	{#if note}
		<span class="text-muted-foreground" data-testid="charm-note">{note}</span>
	{/if}
</div>
