<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { Button } from '$lib/components/ui/button';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { SECTIONS, sectionForKey } from '$lib/sections';
	import { createThemeStore } from '$lib/theme.svelte';

	let { children } = $props();

	const theme = createThemeStore(
		typeof localStorage === 'undefined' ? undefined : localStorage,
		(t) => document.documentElement.classList.toggle('dark', t === 'dark')
	);

	const current = $derived(page.params.section ?? '');

	function onKeydown(e: KeyboardEvent) {
		if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
		const slug = sectionForKey(e.key);
		if (slug) {
			e.preventDefault(); // F1 would otherwise open browser help
			void goto(`/${slug}`);
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />
<svelte:head>
	<title>touchstone</title>
	<link rel="icon" href={favicon} />
</svelte:head>

<Tooltip.Provider>
	<div class="flex h-screen flex-col">
		<header class="flex items-center justify-between border-b px-4 py-2">
			<h1 class="text-lg font-semibold">touchstone</h1>
			<Tooltip.Root>
				<Tooltip.Trigger>
					{#snippet child({ props })}
						<Button {...props} variant="outline" size="sm" onclick={() => theme.toggle()}>
							{theme.current === 'dark' ? 'Light' : 'Dark'} mode
						</Button>
					{/snippet}
				</Tooltip.Trigger>
				<Tooltip.Content>Switch to {theme.current === 'dark' ? 'light' : 'dark'} theme</Tooltip.Content>
			</Tooltip.Root>
		</header>

		<nav class="border-b px-4 py-1">
			<Tabs.Root value={current} onValueChange={(v) => void goto(`/${v}`)}>
				<Tabs.List>
					{#each SECTIONS as s, i (s.slug)}
						<Tabs.Trigger value={s.slug}>
							{s.label} <span class="text-muted-foreground text-xs">F{i + 1}</span>
						</Tabs.Trigger>
					{/each}
				</Tabs.List>
			</Tabs.Root>
		</nav>

		<main class="min-h-0 flex-1 p-4">
			{@render children()}
		</main>
	</div>
</Tooltip.Provider>
