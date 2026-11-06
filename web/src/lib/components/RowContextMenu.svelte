<script lang="ts">
	// MME's right-click menu for a list row: one component for every list. The page works out the items
	// (`buildMenu`) for the row under the pointer and runs the chosen one; this renders them with the
	// shadcn ContextMenu and shows the short message an action returns.
	import type { Snippet } from 'svelte';
	import * as ContextMenu from '$lib/components/ui/context-menu';
	import type { ActionId, MenuItem } from '$lib/contextmenu';

	let {
		items,
		onAction,
		onBefore,
		children
	}: {
		items: MenuItem[];
		/** Runs an item; resolves to a message to show, if any. */
		onAction: (id: ActionId) => Promise<string | undefined>;
		/** The pointer is about to open a menu: forget the previous row (the row under it sets its own). */
		onBefore?: () => void;
		children: Snippet;
	} = $props();

	let open = $state(false);
	let status = $state('');
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function run(id: ActionId) {
		let msg: string | undefined;
		try {
			msg = await onAction(id);
		} catch (e) {
			msg = e instanceof Error ? e.message : String(e);
		}
		if (!msg) return;
		status = msg;
		clearTimeout(timer);
		timer = setTimeout(() => (status = ''), 4000);
	}
</script>

<ContextMenu.Root bind:open>
	<ContextMenu.Trigger class="block h-full min-h-0 w-full" oncontextmenucapture={() => onBefore?.()}>
		{@render children()}
	</ContextMenu.Trigger>
	<!-- a right-click on nothing (the headings, a gap) has no menu in MME; the items arrive just after the event opens it -->
	{#if items.length}
	<ContextMenu.Content data-testid="row-menu">
		{#each items as it (it.id)}
			<div title={it.disabled}>
				<ContextMenu.Item disabled={!!it.disabled} onSelect={() => void run(it.id)} data-testid="menu-{it.id}">
					{it.label}
				</ContextMenu.Item>
			</div>
		{/each}
	</ContextMenu.Content>
	{/if}
</ContextMenu.Root>

{#if status}
	<div role="status" class="bg-popover text-popover-foreground fixed right-3 bottom-3 z-[70] rounded-md border px-3 py-2 text-sm shadow-md" data-testid="menu-status">{status}</div>
{/if}
