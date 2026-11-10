<script lang="ts">
	import '../app.css';
	import { onMount, untrack } from 'svelte';
	import favicon from '$lib/assets/favicon.svg';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { CALCULATORS } from '$lib/calc';
	import { SECTIONS, keyForSection, sectionForKey } from '$lib/sections';
	import { createThemeStore } from '$lib/theme.svelte';
	import { createViewStore } from '$lib/view.svelte';
	import { pageTitle } from '$lib/view';
	import { getActiveCharacter, getCharacterStore } from '$lib/character.svelte';
	import { getInfoStore } from '$lib/info.svelte';
	import ActivePicker from '$lib/components/character/ActivePicker.svelte';
	import GlobalFilterBar from '$lib/components/GlobalFilterBar.svelte';
	import MenuDialogs from '$lib/components/MenuDialogs.svelte';
	import ProfileBadge from '$lib/components/ProfileBadge.svelte';
	import InfoPopover from '$lib/components/InfoPopover.svelte';
	import OptionsMenu from '$lib/components/OptionsMenu.svelte';
	import PasteDialog from '$lib/components/paste/PasteDialog.svelte';
	import { getOptionsStore } from '$lib/options.svelte';
	import { getNavStore } from '$lib/navhistory.svelte';
	import SpellBookDialog from '$lib/components/SpellBookDialog.svelte';
	import { spellBook } from '$lib/spellbook.svelte';
	import SettingsDialog from '$lib/components/settings/SettingsDialog.svelte';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';

	let { children } = $props();

	const theme = createThemeStore(
		typeof localStorage === 'undefined' ? undefined : localStorage,
		(t) => document.documentElement.classList.toggle('dark', t === 'dark')
	);

	const view = createViewStore(typeof localStorage === 'undefined' ? undefined : localStorage);
	const character = getCharacterStore();
	const activeChar = getActiveCharacter();
	const infoStore = getInfoStore();
	// the roster, the open sheet and the active character follow the server's live reloads (the load number)
	$effect(() => infoStore.use());
	onMount(() => void activeChar.start());
	let lastLoad: number | undefined;
	$effect(() => {
		const n = infoStore.current?.load_count;
		untrack(() => {
			if (n !== undefined && lastLoad !== undefined && n !== lastLoad) void activeChar.sync();
			if (n !== undefined) lastLoad = n;
		});
	});
	const options = getOptionsStore();
	const nav = getNavStore();
	// the app's own back/forward list (MME's nav history): every page the router shows is a visit
	$effect(() => {
		const path = page.url.pathname;
		untrack(() => nav.shown(path));
	});
	const title = $derived(pageTitle(view.options, character.doc.name));

	// the pop-out map (frmMap) is a window of its own: no tabs or header
	const bare = $derived(page.url.pathname === '/map');
	const current = $derived(page.params.section ?? '');
	const calcName = $derived(page.url.pathname.startsWith('/calc/') ? page.params.name : undefined);

	function onKeydown(e: KeyboardEvent) {
		// Alt+Left / Alt+Right: the browser's own history shortcut is kept when ours has nowhere to go
		if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
			if ((e.key === 'ArrowLeft' ? nav.back() : nav.forward())) e.preventDefault();
			return;
		}
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
	{#if bare}
		<main class="h-screen overflow-auto p-3" data-testid="popout-main">
			{@render children()}
		</main>
	{:else}
	<div class="flex h-screen flex-col">
		<header class="flex items-center justify-between border-b px-4 py-1.5">
			<div class="flex min-w-0 items-center gap-3">
				<h1 class="text-lg font-semibold">touchstone</h1>
				<ActivePicker />
			</div>
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
			<div class="flex items-center gap-1">
				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<Button {...props} variant="outline" size="icon-sm" aria-label="Back" disabled={!nav.canBack} data-testid="nav-back" onclick={() => nav.back()}>
								<ArrowLeftIcon />
							</Button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>Back (Alt+Left)</Tooltip.Content>
				</Tooltip.Root>
				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<Button {...props} variant="outline" size="icon-sm" aria-label="Forward" disabled={!nav.canForward} data-testid="nav-forward" onclick={() => nav.forward()}>
								<ArrowRightIcon />
							</Button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>Forward (Alt+Right)</Tooltip.Content>
				</Tooltip.Root>
			</div>
			<div class="min-w-0 flex-1 overflow-x-auto">
				<Tabs.Root value={current} onValueChange={(v) => void goto(`/${v}`)}>
					<Tabs.List class="w-max">
						{#each SECTIONS as s (s.slug)}
							{@const key = keyForSection(s.slug)}
							<!-- the F-key hints need the room only wide screens have; the info popover lists every key -->
							<Tabs.Trigger value={s.slug} title={key ? `${s.label} (${key})` : s.label}>
								{s.label}
								{#if key}<span class="text-muted-foreground hidden text-xs 2xl:inline" data-testid="tab-key">{key}</span>{/if}
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
			<DropdownMenu.Root>
				<DropdownMenu.Trigger data-testid="tools-menu">
					{#snippet child({ props })}
						<Button {...props} variant="outline" size="sm">Tools</Button>
					{/snippet}
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end">
					<DropdownMenu.Item data-testid="tools-spell-book" onSelect={() => spellBook.show()}>
						Spell Book
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
			<OptionsMenu />
			<ProfileBadge />
			<InfoPopover />
			<Tooltip.Root>
				<Tooltip.Trigger>
					{#snippet child({ props })}
						<Button
							{...props}
							variant="outline"
							size="icon-sm"
							aria-label="Settings"
							data-testid="settings-button"
							onclick={() => options.openSettings()}
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
	{/if}
	<MenuDialogs />
	{#if options.notice}
		<div role="alert" class="bg-popover text-popover-foreground fixed right-3 bottom-3 z-[70] flex items-center gap-3 rounded-md border px-3 py-2 text-sm shadow-md" data-testid="options-notice">
			<span>{options.notice}</span>
			<button type="button" class="border-input hover:bg-accent h-6 rounded-md border px-2 text-xs" onclick={() => options.dismissNotice()}>OK</button>
		</div>
	{/if}
	<SettingsDialog bind:open={options.settingsOpen} {theme} {view} />
	<SpellBookDialog />
	{#if options.pasteOpen}
		<PasteDialog store={character} onclose={() => (options.pasteOpen = false)} />
	{/if}
</Tooltip.Provider>
