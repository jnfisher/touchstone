<script lang="ts">
	// The Characters tab (F10): the realm's characters (read-only, live) or the Sandbox editor.
	import { getRealmStore } from '$lib/realm.svelte';
	import RealmView from './RealmView.svelte';
	import SandboxView from './SandboxView.svelte';

	const realm = getRealmStore();
	const views = [
		{ id: 'realm', label: 'Realm' },
		{ id: 'sandbox', label: 'Sandbox' }
	] as const;
</script>

<div class="flex h-full min-h-0 flex-col" data-testid="character-tab">
	<div class="mb-3 flex items-center gap-3">
		<h2 class="text-base font-semibold">Characters</h2>
		<div class="bg-muted inline-flex rounded-md p-0.5" role="tablist" aria-label="Characters view">
			{#each views as v (v.id)}
				<button
					type="button"
					role="tab"
					aria-selected={realm.view === v.id}
					data-testid="view-{v.id}"
					class="rounded px-3 py-1 text-sm {realm.view === v.id ? 'bg-background shadow-sm' : 'text-muted-foreground hover:text-foreground'}"
					onclick={() => realm.setView(v.id)}>{v.label}</button
				>
			{/each}
		</div>
		<span class="text-muted-foreground text-xs">
			{realm.view === 'realm' ? 'Read-only, live from the game database' : 'A what-if editor; changes never reach the realm'}
		</span>
	</div>
	<div class="min-h-0 flex-1 {realm.view === 'sandbox' ? 'overflow-auto' : ''}">
		{#if realm.view === 'realm'}
			<RealmView />
		{:else}
			<SandboxView />
		{/if}
	</div>
</div>
