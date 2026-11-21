<script lang="ts">
	// Reads the live game database again (`POST /api/reload`). Shown only for a live source: nothing reloads on its
	// own, because the game commits constantly and a rebuild costs the server about a second of CPU.
	import { Button } from '$lib/components/ui/button';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { getInfoStore, isLive } from '$lib/info.svelte';
	import { onMount } from 'svelte';
	import RefreshIcon from '@lucide/svelte/icons/refresh-cw';

	const store = getInfoStore();
	onMount(() => store.use());
	const live = $derived(isLive(store.current));
	let note = $state<string>();
	let noteTimer: ReturnType<typeof setTimeout> | undefined;

	async function refresh() {
		const r = await store.reload();
		note = r && !r.error ? (r.reloaded ? 'Game data reloaded' : 'No changes since the last load') : undefined;
		// the outcome is news for a moment; after that the tip says what the button does again
		clearTimeout(noteTimer);
		noteTimer = setTimeout(() => (note = undefined), 5000);
	}
	const tip = $derived(store.reloadError ? `Refresh failed: ${store.reloadError}` : (note ?? 'Reload the game data'));
</script>

{#if live}
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="outline"
					size="icon-sm"
					aria-label="Reload the game data"
					data-testid="refresh-button"
					disabled={store.reloading}
					class={store.reloadError ? 'text-destructive' : ''}
					onclick={refresh}
				>
					<RefreshIcon class={store.reloading ? 'animate-spin' : ''} />
				</Button>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content data-testid="refresh-tip">{tip}</Tooltip.Content>
	</Tooltip.Root>
{/if}
