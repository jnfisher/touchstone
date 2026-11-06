<script lang="ts">
	// Shown once above the Monsters list when the Quicksilver rules hide the exp/hr Models A to C that the
	// settings have on (the monster pane and the settings dialog carry the same text).
	import { getSettingsStore } from '$lib/settings.svelte';
	import { modelsHidden, QUICKSILVER_MODEL_NOTE } from '$lib/settings';
	import { isQuicksilver } from '$lib/info';
	import { getInfoStore } from '$lib/info.svelte';
	import { onMount } from 'svelte';

	const settings = getSettingsStore();
	const infoStore = getInfoStore();
	const quicksilver = $derived(isQuicksilver(infoStore.current));
	onMount(() => {
		void settings.load();
		return infoStore.use();
	});
	const show = $derived(modelsHidden(quicksilver, settings.current.exp_per_hour.global));
</script>

{#if show}
	<p class="bg-muted text-muted-foreground rounded-md px-3 py-1.5 text-xs" data-testid="monsters-quicksilver-models">
		{QUICKSILVER_MODEL_NOTE}
	</p>
{/if}
