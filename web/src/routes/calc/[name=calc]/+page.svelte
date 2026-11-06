<script lang="ts">
	import { page } from '$app/state';
	import BackstabCalc from '$lib/components/calc/BackstabCalc.svelte';
	import CoinCalc from '$lib/components/calc/CoinCalc.svelte';
	import ExpCalc from '$lib/components/calc/ExpCalc.svelte';
	import SwingCalc from '$lib/components/calc/SwingCalc.svelte';

	const name = $derived(page.params.name ?? '');
</script>

<!-- Keyed so a calculator starts from its own defaults when you switch. -->
{#key name}
	{#if name === 'exp'}
		<ExpCalc />
	{:else if name === 'coin'}
		<CoinCalc />
	{:else if name === 'swing'}
		<SwingCalc />
	{:else if name === 'backstab'}
		<BackstabCalc />
	{/if}
{/key}
