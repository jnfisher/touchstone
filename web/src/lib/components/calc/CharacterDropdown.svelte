<script lang="ts">
	// The header picker's list in a compact dropdown: one character ("use another" on a calculator) or several
	// (a party). Realm characters only; the Sandbox is the active picker's business.
	import { fullName, searchPicker, type RealmSummary } from '$lib/realm';

	let {
		characters,
		selected = [],
		multiple = false,
		max = 6,
		label,
		testid = 'char-dropdown',
		onpick
	}: {
		characters: RealmSummary[];
		selected?: number[];
		multiple?: boolean;
		/** Most characters a multi-select holds (MME's party table has six rows). */
		max?: number;
		label: string;
		testid?: string;
		/** Single mode: the picked id. Multi mode: the new selection. */
		onpick: (ids: number[]) => void;
	} = $props();

	let open = $state(false);
	let text = $state('');
	let box = $state<HTMLDivElement>();
	const matches = $derived(searchPicker(characters, text));

	function pick(id: number) {
		if (!multiple) {
			onpick([id]);
			open = false;
			text = '';
			return;
		}
		if (selected.includes(id)) onpick(selected.filter((s) => s !== id));
		else if (selected.length < max) onpick([...selected, id]);
	}
	function onWindowClick(e: MouseEvent) {
		if (open && box && !box.contains(e.target as Node)) open = false;
	}
</script>

<svelte:window onclick={onWindowClick} />

<div class="relative inline-block" bind:this={box} data-testid={testid}>
	<button
		type="button"
		class="bg-background border-input hover:bg-accent h-7 rounded-md border px-2 text-xs"
		aria-haspopup="listbox"
		aria-expanded={open}
		disabled={characters.length === 0}
		data-testid="{testid}-button"
		onclick={() => (open = !open)}
	>
		{label} <span aria-hidden="true">&#9662;</span>
	</button>
	{#if open}
		<div class="bg-popover text-popover-foreground absolute top-full left-0 z-50 mt-1 w-80 max-w-[90vw] rounded-md border p-2 shadow-md" data-testid="{testid}-panel">
			<input
				type="search"
				bind:value={text}
				placeholder="Search characters..."
				aria-label="Search characters"
				data-testid="{testid}-search"
				class="bg-background border-input mb-2 h-7 w-full rounded-md border px-2 text-xs outline-none"
			/>
			<ul role="listbox" aria-multiselectable={multiple} class="max-h-60 overflow-auto text-sm">
				{#each matches as c (c.id)}
					{@const on = selected.includes(c.id)}
					<li role="option" aria-selected={on}>
						<button
							type="button"
							class="hover:bg-accent flex w-full items-center justify-between gap-2 rounded px-2 py-1 text-left {on ? 'bg-accent' : ''}"
							disabled={multiple && !on && selected.length >= max}
							data-testid="{testid}-{c.id}"
							onclick={() => pick(c.id)}
						>
							<span class="truncate">{multiple ? (on ? '[x] ' : '[ ] ') : ''}{fullName(c)}</span>
							<span class="text-muted-foreground shrink-0 text-xs">{c.race_name} {c.class_name} L{c.level}</span>
						</button>
					</li>
				{/each}
				{#if !matches.length}<li class="text-muted-foreground px-2 py-1">No match.</li>{/if}
			</ul>
			{#if multiple}<p class="text-muted-foreground mt-1 text-xs">{selected.length} of at most {max} picked.</p>{/if}
		</div>
	{/if}
</div>
