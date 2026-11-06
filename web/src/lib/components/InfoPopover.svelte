<script lang="ts">
	// The data-version info (`/api/info`): MME shows it in the "Database Version (Created ...)" frame.
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { api, type Info } from '$lib/api';
	import { infoLines, profileLines } from '$lib/info';
	import { keyLines } from '$lib/sections';
	import InfoIcon from '@lucide/svelte/icons/info';

	let info = $state<Info>();
	let error = $state<string>();
	async function load(open: boolean) {
		if (!open) return;
		try {
			info = await api.info();
			error = undefined;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}
	const lines = $derived(info ? infoLines(info) : []);
	const rules = $derived(info ? profileLines(info) : []);
	const keys = keyLines();
</script>

<DropdownMenu.Root onOpenChange={load}>
	<DropdownMenu.Trigger data-testid="info-button">
		{#snippet child({ props })}
			<Button {...props} variant="outline" size="icon-sm" aria-label="Database info"><InfoIcon /></Button>
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="w-80 p-3" data-testid="info-popover">
		<h2 class="mb-2 text-sm font-semibold">Database</h2>
		{#if error}
			<p class="text-destructive text-sm" role="alert">{error}</p>
		{:else if !info}
			<p class="text-muted-foreground text-sm">Loading...</p>
		{:else}
			<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-sm">
				{#each lines as l (l.label)}
					<dt class="text-muted-foreground">{l.label}</dt>
					<dd class="min-w-0 break-words text-right tabular-nums">{l.value}</dd>
				{/each}
			</dl>
		{/if}
		{#if rules.length}
			<h2 class="mt-3 mb-1 text-sm font-semibold">Rules profile</h2>
			<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-sm" data-testid="profile-lines">
				{#each rules as l (l.label)}
					<dt class="text-muted-foreground">{l.label}</dt>
					<dd class="min-w-0 break-words text-right tabular-nums">{l.value}</dd>
				{/each}
			</dl>
		{/if}
		<h2 class="mt-3 mb-1 text-sm font-semibold">Keys</h2>
		<p class="text-muted-foreground mb-1 text-xs">The tabs follow a different order from MME's F1 to F12; MME's key is on the right.</p>
		<dl class="grid grid-cols-[auto_1fr_auto] gap-x-3 gap-y-0.5 text-sm" data-testid="key-map">
			{#each keys as k (k.key)}
				<dt class="font-medium tabular-nums">{k.key}</dt>
				<dd>{k.label}</dd>
				<dd class="text-muted-foreground text-right tabular-nums">MME {k.mme}</dd>
			{/each}
		</dl>
	</DropdownMenu.Content>
</DropdownMenu.Root>
