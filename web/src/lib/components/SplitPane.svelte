<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		left,
		right,
		storageKey = 'touchstone.split',
		autoWidth,
		collapsed = $bindable(false),
		minList = 240,
		minDetail = 320,
		stackBelow = 1100
	}: {
		left: Snippet;
		right: Snippet;
		/** localStorage key for this tab's divider position; the collapsed state adds ".collapsed". */
		storageKey?: string;
		/** The list's natural width (its grid's columns); the default list width, capped. */
		autoWidth?: number;
		collapsed?: boolean;
		minList?: number;
		minDetail?: number;
		/** Window width under which the detail pane sits below the list instead of beside it. */
		stackBelow?: number;
	} = $props();

	function read(key: string): string | null {
		try {
			return localStorage.getItem(key);
		} catch {
			return null;
		}
	}
	function write(key: string, v: string) {
		try {
			localStorage.setItem(key, v);
		} catch {
			// storage unavailable: the layout just is not remembered
		}
	}

	// A width the user dragged to wins over the automatic one until they drag again. Reloaded if
	// the pane is reused for another tab.
	let userW = $state<number>();
	let loadedKey: string | undefined;
	$effect.pre(() => {
		if (loadedKey === storageKey) return;
		loadedKey = storageKey;
		const w = Number(read(storageKey));
		userW = w >= 100 ? w : undefined;
		collapsed = read(storageKey + '.collapsed') === '1';
	});
	$effect(() => write(storageKey + '.collapsed', collapsed ? '1' : '0'));

	let box: HTMLDivElement;
	let boxW = $state(1200);
	let winW = $state(1600);
	let dragging = $state(false);
	const stacked = $derived(winW < stackBelow);

	const maxList = $derived(Math.max(minList, boxW - minDetail - 12));
	const listW = $derived(
		Math.min(maxList, Math.max(minList, userW ?? Math.min(autoWidth ?? 520, boxW * 0.6)))
	);

	function set(w: number) {
		userW = Math.min(maxList, Math.max(minList, Math.round(w)));
	}
	function save() {
		if (userW !== undefined) write(storageKey, String(userW));
	}
	function move(e: PointerEvent) {
		if (dragging) set(e.clientX - box.getBoundingClientRect().left);
	}
	function end() {
		if (dragging) save();
		dragging = false;
	}
	function key(e: KeyboardEvent) {
		const step = e.shiftKey ? 80 : 20;
		if (e.key === 'ArrowLeft') set(listW - step);
		else if (e.key === 'ArrowRight') set(listW + step);
		else if (e.key === 'Home' || e.key === 'End') userW = undefined;
		else return;
		e.preventDefault();
		e.stopPropagation(); // keep the grid's own arrow handling out of it
		save();
	}
</script>

<svelte:window bind:innerWidth={winW} onpointermove={move} onpointerup={end} />

<div
	bind:this={box}
	bind:clientWidth={boxW}
	class="flex h-full min-h-0 w-full {stacked ? 'flex-col gap-2' : 'flex-row'}"
	class:select-none={dragging}
	data-testid="split"
	data-stacked={stacked}
>
	<div
		class="min-h-0 min-w-0 {collapsed ? 'flex-1' : stacked ? 'h-[45%] shrink-0' : 'shrink-0'}"
		style:width={stacked || collapsed ? undefined : `${listW}px`}
		data-testid="split-list"
	>
		{@render left()}
	</div>
	{#if !collapsed}
		{#if !stacked}
			<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
			<div
				role="separator"
				aria-orientation="vertical"
				aria-label="Resize list and detail panes"
				aria-valuenow={Math.round(listW)}
				title="Drag to resize; Home resets"
				tabindex="0"
				class="hover:bg-ring focus-visible:bg-ring bg-border mx-1 w-1 shrink-0 cursor-col-resize rounded transition-colors"
				class:bg-ring={dragging}
				onpointerdown={(e) => {
					dragging = true;
					e.preventDefault();
				}}
				ondblclick={() => {
					userW = undefined;
					write(storageKey, '');
				}}
				onkeydown={key}
			></div>
		{/if}
		<div class="min-h-0 min-w-0 flex-1" data-testid="split-detail">{@render right()}</div>
	{/if}
</div>
