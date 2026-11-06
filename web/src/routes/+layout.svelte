<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { CALCULATORS } from '$lib/calc';
	import { SECTIONS, sectionForKey } from '$lib/sections';
	import { createThemeStore } from '$lib/theme.svelte';
	import { createViewStore } from '$lib/view.svelte';
	import { pageTitle } from '$lib/view';
	import { getCharacterStore } from '$lib/character.svelte';
	import GlobalFilterBar from '$lib/components/GlobalFilterBar.svelte';
	import SettingsDialog from '$lib/components/settings/SettingsDialog.svelte';
	import SettingsIcon from '@lucide/svelte/icons/settings';

	let { children } = $props();

	const theme = createThemeStore(
		typeof localStorage === 'undefined' ? undefined : localStorage,
		(t) => document.documentElement.classList.toggle('dark', t === 'dark')
	);

	const view = createViewStore(typeof localStorage === 'undefined' ? undefined : localStorage);
	const character = getCharacterStore();
	let settingsOpen = $state(false);
	const title = $derived(pageTitle(view.options, character.doc.name));

	const current = $derived(page.params.section ?? '');
	const calcName = $derived(page.url.pathname.startsWith('/calc/') ? page.params.name : undefined);

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
	<title>{title}</title>
	<link rel="icon" href={favicon} />
</svelte:head>

<Tooltip.Provider>
	<div class="flex h-screen flex-col">
		<header class="flex items-center justify-between border-b px-4 py-1.5">
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

		<nav class="flex items-center gap-2 border-b px-4 py-1" data-testid="tabs">
			<div class="min-w-0 flex-1 overflow-x-auto">
				<Tabs.Root value={current} onValueChange={(v) => void goto(`/${v}`)}>
					<Tabs.List class="w-max">
						{#each SECTIONS as s, i (s.slug)}
							<Tabs.Trigger value={s.slug}>
								{s.label} <span class="text-muted-foreground text-xs">F{i + 1}</span>
							</Tabs.Trigger>
						{/each}
					</Tabs.List>
				</Tabs.Root>
			</div>
			<DropdownMenu.Root>
				<DropdownMenu.Trigger data-testid="calc-menu">
					{#snippet child({ props })}
						<Button {...props} variant={calcName ? 'secondary' : 'outline'} size="sm">
							Calculators
						</Button>
					{/snippet}
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end">
					{#each CALCULATORS as c (c.slug)}
						<DropdownMenu.Item
							data-testid="calc-link-{c.slug}"
							onSelect={() => void goto(`/calc/${c.slug}`)}
						>
							{c.label}
						</DropdownMenu.Item>
					{/each}
				</DropdownMenu.Content>
			</DropdownMenu.Root>
			<Tooltip.Root>
				<Tooltip.Trigger>
					{#snippet child({ props })}
						<Button
							{...props}
							variant="outline"
							size="icon-sm"
							aria-label="Settings"
							data-testid="settings-button"
							onclick={() => (settingsOpen = true)}
						>
							<SettingsIcon />
						</Button>
					{/snippet}
				</Tooltip.Trigger>
				<Tooltip.Content>Settings</Tooltip.Content>
			</Tooltip.Root>
		</nav>
		<GlobalFilterBar />

		<main class="min-h-0 flex-1 p-2 sm:p-4">
			{@render children()}
		</main>
	</div>
	<SettingsDialog bind:open={settingsOpen} {theme} {view} />
</Tooltip.Provider>
