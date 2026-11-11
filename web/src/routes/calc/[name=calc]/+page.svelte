<script lang="ts">
	import { page } from '$app/state';
	import BackstabCalc from '$lib/components/calc/BackstabCalc.svelte';
	import CoinCalc from '$lib/components/calc/CoinCalc.svelte';
	import ExpCalc from '$lib/components/calc/ExpCalc.svelte';
	import HitCalc from '$lib/components/calc/HitCalc.svelte';
	import MonsterSimCalc from '$lib/components/calc/MonsterSimCalc.svelte';
	import SwingCalc from '$lib/components/calc/SwingCalc.svelte';
	import WeaponSimCalc from '$lib/components/calc/WeaponSimCalc.svelte';

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
	{:else if name === 'hit'}
		<HitCalc />
	{:else if name === 'monster-sim'}
		<MonsterSimCalc />
	{:else if name === 'weapon-sim'}
		<WeaponSimCalc />
	{/if}
{/key}
