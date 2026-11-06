<script lang="ts">
	// A searchable combobox for one equipment or bless slot: type to narrow, arrows and Enter pick,
	// and a "none" row clears the slot.
	import { filterChoices, type ItemChoice } from '$lib/character';
	import { inputCls, labelCls } from '../calc/styles';

	let {
		choices,
		value,
		label,
		testid,
		onpick,
		noneLabel = '(none)',
		flagged = false
	}: {
		choices: ItemChoice[];
		value: number;
		label: string;
		testid: string;
		onpick: (n: number) => void;
		noneLabel?: string;
		/** MME paints the slot red: the character cannot use the item, or a two-hander meets an off-hand. */
		flagged?: boolean;
	} = $props();

	const LIMIT = 100;
	let open = $state(false);
	let query = $state('');
	let active = $state(0);

	const selected = $derived(choices.find((c) => c.number === value));
	const shown = $derived(filterChoices(choices, query).slice(0, LIMIT));
	// Row 0 is "none"; the matches follow.
	const rows = $derived<ItemChoice[]>([{ number: 0, name: '', label: noneLabel }, ...shown]);

	function choose(c: ItemChoice) {
		onpick(c.number);
		open = false;
		query = '';
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			open = true;
			active = Math.min(active + 1, rows.length - 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			active = Math.max(active - 1, 0);
		} else if (e.key === 'Enter' && open && rows[active]) {
			e.preventDefault();
			choose(rows[active]);
		} else if (e.key === 'Escape') {
			open = false;
			query = '';
		}
	}

	$effect(() => {
		query;
		active = query ? 1 : 0;
	});
</script>

<div class="relative flex flex-col gap-1">
	<span class={labelCls}>{label}</span>
	<input
		type="text"
		role="combobox"
		aria-expanded={open}
		aria-controls="{testid}-list"
		aria-label={label}
		autocomplete="off"
		placeholder={open ? 'Type to search...' : noneLabel}
		value={open ? query : value === 0 ? '' : (selected?.label ?? `Item ${value}`)}
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
		data-testid={testid}
		data-flagged={flagged}
		class="{inputCls} {flagged ? 'border-destructive text-destructive' : ''}"
	/>
	{#if open}
		<ul
			id="{testid}-list"
			role="listbox"
			class="bg-popover text-popover-foreground absolute top-full right-0 left-0 z-20 mt-1 max-h-64 overflow-auto rounded-md border py-1 text-sm shadow-md"
		>
			{#each rows as c, i (c.number)}
				<!-- mousedown, not click: the input's blur would otherwise close the list first. -->
				<li
					role="option"
					aria-selected={c.number === value}
					class="cursor-pointer px-2 py-1 {i === active ? 'bg-accent text-accent-foreground' : ''} {c.number === 0
						? 'text-muted-foreground'
						: ''}"
					onmousedown={(e) => {
						e.preventDefault();
						choose(c);
					}}
					onmouseenter={() => (active = i)}
				>
					{c.label}
				</li>
			{/each}
		</ul>
	{/if}
</div>
