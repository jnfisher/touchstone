<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		left,
		right,
		storageKey = 'touchstone.split',
		min = 280,
		initial = 420
	}: { left: Snippet; right: Snippet; storageKey?: string; min?: number; initial?: number } = $props();

	// Width of the right (detail) pane in pixels; remembered per browser.
	function stored(): number {
		try {
			const v = Number(localStorage.getItem(storageKey));
			return v >= min ? v : initial;
		} catch {
			return initial;
		}
	}
	let width = $state(stored());
	let box: HTMLDivElement;
	let dragging = $state(false);

	function clamp(w: number) {
		const max = Math.max(min, (box?.clientWidth ?? 1200) - min);
		return Math.min(Math.max(w, min), max);
	}
	function save() {
		try {
			localStorage.setItem(storageKey, String(Math.round(width)));
		} catch {
			// storage unavailable: the width just is not remembered
		}
	}
	function move(e: PointerEvent) {
		if (!dragging) return;
		width = clamp(box.getBoundingClientRect().right - e.clientX);
	}
	function end() {
		if (dragging) save();
		dragging = false;
	}
	function key(e: KeyboardEvent) {
		const step = e.shiftKey ? 80 : 20;
		if (e.key === 'ArrowLeft') width = clamp(width + step);
		else if (e.key === 'ArrowRight') width = clamp(width - step);
		else return;
		e.preventDefault();
		e.stopPropagation(); // keep the grid's own arrow handling out of it
		save();
	}
</script>

<svelte:window onpointermove={move} onpointerup={end} />

<div bind:this={box} class="flex h-full min-h-0 w-full" class:select-none={dragging}>
	<div class="min-w-0 flex-1">{@render left()}</div>
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
	<div
		role="separator"
		aria-orientation="vertical"
		aria-label="Resize detail pane"
		aria-valuenow={Math.round(width)}
		tabindex="0"
		class="hover:bg-ring focus-visible:bg-ring mx-1 w-1 shrink-0 cursor-col-resize rounded bg-border transition-colors"
		class:bg-ring={dragging}
		onpointerdown={(e) => {
			dragging = true;
			e.preventDefault();
		}}
		onkeydown={key}
	></div>
	<div class="min-w-0 shrink-0" style:width="{width}px">{@render right()}</div>
</div>
