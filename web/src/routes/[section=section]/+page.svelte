<script lang="ts">
	import { page } from '$app/state';
	import CharacterTab from '$lib/components/character/CharacterTab.svelte';
	import ChestsTab from '$lib/components/chests/ChestsTab.svelte';
	import CompareTab from '$lib/components/CompareTab.svelte';
	import ExplorerView from '$lib/components/ExplorerView.svelte';
	import RoomsTab from '$lib/components/RoomsTab.svelte';
	import { explorerFor } from '$lib/explorers';
	import { SECTIONS } from '$lib/sections';

	const slug = $derived(page.params.section ?? '');
	const label = $derived(SECTIONS.find((s) => s.slug === slug)?.label ?? '');
	const explorer = $derived(explorerFor(slug));
</script>

{#if slug === 'character'}
	<CharacterTab />
{:else if slug === 'chests'}
	<ChestsTab />
{:else if slug === 'rooms'}
	<RoomsTab />
{:else if slug === 'compare'}
	<CompareTab />
{:else if explorer}
	<!-- Keyed by tab so each tab gets its own grid, search text and scroll position. -->
	{#key slug}
		<ExplorerView {explorer} title={label} />
	{/key}
{:else}
	<p class="text-muted-foreground">{label}: not built yet.</p>
{/if}
