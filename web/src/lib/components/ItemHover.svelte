<script lang="ts">
	// One popover for the whole app: resting the pointer on an item's name (anything marked `data-item`, or the
	// name cell of a grid marked `data-item-rows`) shows the item's numbers, abilities and how it compares with
	// what the active character wears. Mounted once in the layout; pages only mark their item names.
	import { api, type Item } from '$lib/api';
	import { evalRequest } from '$lib/character';
	import { getCharacterStore } from '$lib/character.svelte';
	import { paneClient, type Pane } from '$lib/detail';
	import { DEFAULT_FILTER } from '$lib/globalfilter';
	import { hoveredItem, itemFacts, paneLines, placeCard } from '$lib/itemhover';

	/** Long enough that sweeping the pointer across a list shows nothing. */
	const DELAY = 350;
	const WIDTH = 340;

	const character = getCharacterStore();
	let items: Promise<Map<number, Item>> | undefined;
	const panes = new Map<string, Pane>();

	let shown = $state.raw<{ item: Item; pane?: Pane; failed?: boolean }>();
	let pos = $state({ left: 0, top: 0 });
	let card = $state<HTMLDivElement>();
	let anchor: Element | undefined;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let ticket = 0;

	const table = () => (items ??= api.table('items').then((all) => new Map(all.map((i) => [i.number, i]))));

	function place() {
		if (!anchor || !card) return;
		const a = anchor.getBoundingClientRect();
		pos = placeCard(
			{ left: a.left, top: a.top, width: a.width, height: a.height },
			{ width: card.offsetWidth, height: card.offsetHeight },
			{ width: window.innerWidth, height: window.innerHeight }
		);
	}

	function hide() {
		clearTimeout(timer);
		ticket++;
		anchor = undefined;
		shown = undefined;
	}

	async function show(n: number, el: Element) {
		const mine = ++ticket;
		const item = (await table().catch(() => undefined))?.get(n);
		if (mine !== ticket || !item) return;
		anchor = el;
		// the character the rest of the app calls "the character": the comparison is with what it wears
		const body = evalRequest($state.snapshot(character.doc), $state.snapshot(character.options));
		const key = `${n}|${body ? JSON.stringify(body) : ''}`;
		shown = { item, pane: panes.get(key) };
		if (panes.has(key)) return;
		try {
			const pane = await paneClient.pane('items', n, {
				...(body ? { character: body } : {}),
				global_filter: { ...DEFAULT_FILTER, use_character: body !== undefined },
				options: {}
			});
			if (panes.size > 200) panes.clear();
			panes.set(key, pane);
			if (mine === ticket) shown = { item, pane };
		} catch {
			if (mine === ticket) shown = { item, failed: true };
		}
	}

	function over(e: Event) {
		const hit = hoveredItem(e.target as Element | null);
		if (!hit) {
			// off the name: a pending card is dropped and a shown one goes
			clearTimeout(timer);
			if (anchor) hide();
			return;
		}
		if (hit.anchor === anchor) return;
		clearTimeout(timer);
		timer = setTimeout(() => void show(hit.item, hit.anchor), DELAY);
	}

	$effect(() => {
		void shown;
		// after the card has its size (and again when the pane's text makes it taller)
		queueMicrotask(place);
	});

	const facts = $derived(shown ? itemFacts(shown.item) : []);
	const lines = $derived(shown?.pane ? paneLines(shown.pane) : []);
</script>

<svelte:document onmouseover={over} onmousedown={hide} onkeydown={hide} />
<svelte:window onscrollcapture={hide} onblur={hide} />

{#if shown}
	<div
		bind:this={card}
		class="bg-popover text-popover-foreground pointer-events-none fixed z-[80] rounded-md border p-3 text-[13px] leading-5 shadow-lg"
		style:left="{pos.left}px"
		style:top="{pos.top}px"
		style:width="{WIDTH}px"
		role="tooltip"
		data-testid="item-hover"
	>
		<div class="mb-1.5 font-semibold">{shown.item.name} <span class="text-muted-foreground font-normal">({shown.item.number})</span></div>
		<dl class="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-0.5">
			{#each facts as f (f.label)}
				<dt class="text-muted-foreground">{f.label}</dt>
				<dd class="tabular-nums">{f.value}</dd>
			{/each}
			{#each lines as l, k (k)}
				<dt class="text-muted-foreground {l.label.startsWith('Compared') ? 'col-span-2 mt-1.5 text-[11px] font-medium tracking-wide uppercase' : ''}">{l.label}</dt>
				<dd class="break-words {l.label.startsWith('Compared') ? 'col-span-2' : ''}">{l.value}</dd>
			{/each}
		</dl>
		{#if !shown.pane && !shown.failed}
			<p class="text-muted-foreground mt-1 text-xs">Loading abilities...</p>
		{:else if shown.failed}
			<p class="text-muted-foreground mt-1 text-xs">The abilities could not be loaded.</p>
		{/if}
	</div>
{/if}
