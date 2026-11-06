<script lang="ts">
	// MME's route results window: the caption, then one row per action (and per waypoint), each opening its room.
	import { goto } from '$app/navigation';
	import { dialogs } from '$lib/dialogs.svelte';
	import { selection } from '$lib/explorer-state.svelte';
	import type { RouteRow } from '$lib/routedialog';
	import { Button } from '$lib/components/ui/button';

	function open(r: RouteRow) {
		const w = dialogs.route;
		dialogs.closeRoute();
		if (w?.onRoom) {
			w.onRoom(r.map, r.room);
			return;
		}
		selection.set('rooms', `${r.map}/${r.room}`);
		void goto('/rooms');
	}
</script>

{#if dialogs.route}
	{@const w = dialogs.route}
	<div class="bg-background/80 fixed inset-0 z-[60] flex items-center justify-center p-4" role="presentation">
		<div
			class="bg-card flex max-h-[80vh] w-full max-w-4xl flex-col gap-2 rounded-lg border p-4 shadow-xl"
			role="dialog"
			aria-modal="true"
			aria-label={w.data.caption || 'Route'}
			data-testid="route-dialog"
		>
			{#if w.data.message !== null && w.data.rows.length === 0}
				<p class="text-sm" data-testid="route-message">{w.data.message}</p>
			{:else}
				<h2 class="text-base font-semibold" data-testid="route-caption">{w.data.caption}</h2>
				<div class="min-h-0 flex-1 overflow-auto rounded-md border">
					<table class="w-full text-sm" data-testid="route-table">
						<thead class="bg-muted text-muted-foreground sticky top-0 text-left text-xs">
							<tr><th class="w-16 px-2 py-1 font-medium">Step</th><th class="px-2 py-1 font-medium">Location</th></tr>
						</thead>
						<tbody>
							{#each w.data.rows as r, i (i)}
								<tr class="hover:bg-accent border-t" data-testid="route-row">
									<td class="px-2 py-1 tabular-nums">{r.step}</td>
									<td class="px-2 py-1">
										<button type="button" class="text-primary text-left hover:underline" onclick={() => open(r)}>{r.text}</button>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
			<div class="flex justify-end">
				<Button size="sm" variant="outline" onclick={() => dialogs.closeRoute()} data-testid="route-close">Close</Button>
			</div>
		</div>
	</div>
{/if}
