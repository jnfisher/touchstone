<script lang="ts">
	// A searchable stand-in for MME's weapon combo: type to narrow the list, arrows and Enter pick.
	import { filterWeapons, type WeaponChoice } from '$lib/calc';
	import { inputCls, labelCls } from './styles';

	let {
		choices,
		value = $bindable(),
		label = 'Weapon',
		noun = 'weapon',
		testid = 'weapon-input',
		clearable = false,
		disabled = false,
		onchoose
	}: {
		choices: WeaponChoice[];
		value: number | null;
		label?: string;
		/** What the list holds, for the placeholder ("monster"). */
		noun?: string;
		testid?: string;
		/** Offer "None" first, which sets the value to null. */
		clearable?: boolean;
		/** Read-only, as when the current character supplies the value. */
		disabled?: boolean;
		/** Called after a pick (not on every change of `value`). */
		onchoose?: () => void;
	} = $props();

	const LIMIT = 150;
	let open = $state(false);
	let query = $state('');
	let active = $state(0);
	let box: HTMLInputElement | undefined = $state();

	const selected = $derived(choices.find((c) => c.number === value));
	const matches = $derived(filterWeapons(choices, query));
	const shown = $derived(matches.slice(0, LIMIT));

	function choose(c: WeaponChoice | null) {
		value = c?.number ?? null;
		onchoose?.();
		open = false;
		query = '';
		box?.blur();
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			open = true;
			active = Math.min(active + 1, shown.length - 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			active = Math.max(active - 1, 0);
		} else if (e.key === 'Enter' && open && shown[active]) {
			e.preventDefault();
			choose(shown[active]);
		} else if (e.key === 'Escape') {
			open = false;
			query = '';
		}
	}

	$effect(() => {
		query;
		active = 0;
	});
</script>

<div class="relative flex flex-col gap-1" data-testid="{testid}-picker">
	<span class={labelCls}>{label}</span>
	<input
		bind:this={box}
		type="text"
		role="combobox"
		aria-expanded={open}
		aria-controls="{testid}-list"
		autocomplete="off"
		placeholder={open ? `Type to search ${noun}s...` : `Pick a ${noun}`}
		value={open ? query : (selected?.label ?? (clearable ? 'None' : ''))}
		oninput={(e) => (query = e.currentTarget.value)}
		onfocus={(e) => {
			open = true;
			e.currentTarget.select();
		}}
		onblur={() => {
			open = false;
			query = '';
		}}
		onkeydown={onKey}
		{disabled}
		data-testid={testid}
		class="{inputCls} disabled:opacity-60"
	/>
	{#if open}
		<ul
			id="{testid}-list"
			role="listbox"
			class="bg-popover text-popover-foreground absolute top-full right-0 left-0 z-20 mt-1 max-h-64 overflow-auto rounded-md border py-1 text-sm shadow-md"
		>
			{#if clearable}
				<li
					role="option"
					aria-selected={value === null}
					class="text-muted-foreground cursor-pointer px-2 py-1 italic"
					onmousedown={(e) => {
						e.preventDefault();
						choose(null);
					}}
				>
					None
				</li>
			{/if}
			{#each shown as c, i (c.number)}
				<!-- mousedown, not click: the input's blur would otherwise close the list first. -->
				<li
					role="option"
					aria-selected={c.number === value}
					class="cursor-pointer px-2 py-1 {i === active ? 'bg-accent text-accent-foreground' : ''}"
					onmousedown={(e) => {
						e.preventDefault();
						choose(c);
					}}
					onmouseenter={() => (active = i)}
				>
					{c.label}
				</li>
			{:else}
				<li class="text-muted-foreground px-2 py-1">No {noun} matches</li>
			{/each}
			{#if matches.length > LIMIT}
				<li class="text-muted-foreground px-2 py-1 text-xs">
					{matches.length - LIMIT} more, keep typing to narrow
				</li>
			{/if}
		</ul>
	{/if}
</div>
