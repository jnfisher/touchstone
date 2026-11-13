<script lang="ts">
	// A realm character's sheet: identity, then touchstone's calculation beside the game's stored values (a
	// difference is highlighted, with the cause on hover), the derived numbers, gear, pack, spells and abilities.
	// Items, spells and the location link to their records.
	import { goto } from '$app/navigation';
	import { api, type Item } from '$lib/api';
	import { linkFor } from '$lib/detail';
	import { selection } from '$lib/explorer-state.svelte';
	import { coinText, fullName, lastOnText, sheetGroups, storedOnly, unexplained, wornLines, type RealmDetail } from '$lib/realm';
	import { headingCls } from '../calc/styles';
	import DerivedPanel from './DerivedPanel.svelte';

	let {
		detail,
		isActive,
		onActive,
		onCopy
	}: { detail: RealmDetail; isActive: boolean; onActive: () => void; onCopy: () => void } = $props();

	const id = $derived(detail.identity);
	const groups = $derived(sheetGroups(detail.comparison));
	const worn = $derived(wornLines(detail));
	const stored = $derived(storedOnly(detail.game));
	const odd = $derived(unexplained(detail.comparison));

	let items: ReadonlyMap<number, Item> | undefined;
	async function openItem(n: number) {
		items ??= new Map((await api.table('items')).map((i) => [i.number, i]));
		const to = linkFor({ table: 'items', number: n }, items);
		if (!to) return;
		selection.set(to.slug, to.id);
		void goto(`/${to.slug}`);
	}
	function openSpell(n: number) {
		selection.set('spells', n);
		void goto('/spells');
	}
	function openRoom() {
		selection.set('rooms', `${id.map}/${id.room}`);
		void goto('/rooms');
	}

	const card = 'bg-card text-card-foreground rounded-lg border p-3';
	const link = 'hover:underline text-left';
</script>

<div class="h-full space-y-3 overflow-auto pr-1" data-testid="realm-sheet">
	<div class={card}>
		<div class="flex flex-wrap items-start gap-2">
			<div class="min-w-0">
				<div class="text-base font-semibold" data-testid="sheet-name">{fullName(id)}</div>
				<div class="text-muted-foreground text-sm">{id.title ? `${id.title}, ` : ''}{id.race_name} {id.class_name}, level {id.level}</div>
			</div>
			<div class="ml-auto flex gap-2">
				<button
					type="button"
					class="h-8 rounded-md px-3 text-sm {isActive ? 'bg-secondary text-secondary-foreground' : 'bg-primary text-primary-foreground'}"
					disabled={isActive}
					data-testid="make-active"
					onclick={onActive}>{isActive ? 'Active character' : 'Make active'}</button
				>
				<button type="button" class="border-input hover:bg-accent h-8 rounded-md border px-3 text-sm" data-testid="copy-to-sandbox" onclick={onCopy}>Copy to sandbox</button>
			</div>
		</div>
		<dl class="mt-3 grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-x-6 gap-y-1 text-sm">
			{#each [['Exp', id.exp.toLocaleString('en-US')], ['Alignment', id.alignment_name], ['Gang', id.gang || 'none'], ['Lives', String(id.lives)], ['Last on', lastOnText(id)], ['Unspent / total CP', `${id.unspent_cp} / ${id.total_cp}`], ['Coins', coinText(id.coins)], ['Attack the calculators assume', 'Normal attack (wielded weapon)']] as [k, v] (k)}
				<div class="flex justify-between gap-2 border-b py-0.5"><dt class="text-muted-foreground">{k}</dt><dd class="tabular-nums">{v}</dd></div>
			{/each}
			<div class="flex justify-between gap-2 border-b py-0.5">
				<dt class="text-muted-foreground">Location</dt>
				<dd><button type="button" class="{link} text-info tabular-nums" data-testid="sheet-location" title="Open in the Rooms tab" onclick={openRoom}>{id.map}/{id.room}</button></dd>
			</div>
		</dl>
	</div>

	{#if detail.unmapped.length}
		<div class="border-warning bg-warning/10 rounded-lg border p-3 text-sm" data-testid="sheet-unmapped">
			<div class="text-warning mb-1 font-medium">Unmapped ({detail.unmapped.length})</div>
			<p class="text-muted-foreground mb-1 text-xs">These count in the game but have no place in MME's model, so touchstone's calculation leaves them out.</p>
			<ul class="space-y-0.5">
				{#each detail.unmapped as u, i (i)}
					<li><span class="font-medium">{u.name}</span> <span class="text-muted-foreground">({u.kind}): {u.reason}</span></li>
				{/each}
			</ul>
		</div>
	{/if}

	{#if detail.sheet_error}
		<p class="text-destructive text-sm" data-testid="sheet-error">No sheet: {detail.sheet_error}</p>
	{:else}
		<div class={card}>
			<div class="mb-2 flex flex-wrap items-baseline gap-2">
				<h3 class={headingCls + ' mb-0'}>Touchstone beside the game</h3>
				<span class="text-muted-foreground text-xs" data-testid="differs-summary">
					{detail.differs === 0 ? 'No differences' : `${detail.differs} differ${detail.differs === 1 ? 's' : ''}`}{odd ? `, ${odd} with no known cause` : ''}
				</span>
			</div>
			<div class="grid items-start gap-x-6 gap-y-3 xl:grid-cols-2">
				{#each groups as g (g.title)}
					<table class="w-full text-sm" data-testid="sheet-{g.title.toLowerCase().replace(/ /g, '-')}">
						<thead>
							<tr class="text-muted-foreground text-[11px] tracking-wide uppercase">
								<th class="py-0.5 text-left font-medium">{g.title}</th><th class="pl-3 text-right font-medium">Touchstone</th><th class="pl-3 text-right font-medium">Game</th>
							</tr>
						</thead>
						<tbody>
							{#each g.rows as r (r.field)}
								<tr class="border-t {r.differs ? 'bg-warning/15' : ''}" title={r.tip} data-differs={r.differs} data-field={r.field}>
									<td class="py-0.5">{r.label}</td>
									<td class="pl-3 text-right tabular-nums {r.differs ? 'text-warning font-medium' : ''}">{r.touchstone}</td>
									<td class="pl-3 text-right tabular-nums">{r.game}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/each}
			</div>
			{#if stored.length}
				<p class="text-muted-foreground mt-3 text-xs">
					Also stored by the game (no counterpart in MME):
					{#each stored as s, i (s.label)}{i ? ', ' : ' '}{s.label} {s.value}{/each}.
				</p>
			{/if}
		</div>
		<div class={card}><DerivedPanel ev={detail.sheet ?? undefined} /></div>
	{/if}

	<div class="grid items-start gap-3 xl:grid-cols-2">
		<div class={card} data-testid="sheet-worn">
			<h3 class={headingCls}>Worn ({worn.length})</h3>
			{#each worn as w (w.slot + w.item)}
				<div class="flex justify-between gap-2 border-b py-0.5 text-sm">
					<span class="text-muted-foreground">{w.slot}</span>
					<button type="button" class={link} data-item={w.item} onclick={() => void openItem(w.item)}>{w.name}</button>
				</div>
			{:else}
				<p class="text-muted-foreground text-sm">Nothing worn.</p>
			{/each}
		</div>
		<div class={card} data-testid="sheet-inventory">
			<h3 class={headingCls}>Inventory ({detail.inventory.length})</h3>
			{#each detail.inventory as it (it.slot)}
				<div class="flex justify-between gap-2 border-b py-0.5 text-sm">
					<button type="button" class={link} data-item={it.item} onclick={() => void openItem(it.item)}>{it.name}</button>
					<span class="text-muted-foreground tabular-nums">{it.readied ? 'readied ' : ''}{it.uses >= 0 ? `${it.uses} uses` : ''}</span>
				</div>
			{:else}
				<p class="text-muted-foreground text-sm">Empty.</p>
			{/each}
		</div>
		<div class={card} data-testid="sheet-spellbook">
			<h3 class={headingCls}>Spellbook ({detail.spellbook.length})</h3>
			<div class="flex flex-wrap gap-x-3 gap-y-0.5 text-sm">
				{#each detail.spellbook as s (s.spell)}<button type="button" class={link} onclick={() => openSpell(s.spell)}>{s.name}</button>{:else}<span class="text-muted-foreground">No spells learned.</span>{/each}
			</div>
		</div>
		<div class={card} data-testid="sheet-active">
			<h3 class={headingCls}>Active spells ({detail.active_spells.length})</h3>
			{#each detail.active_spells as s (s.slot)}
				<div class="flex justify-between gap-2 border-b py-0.5 text-sm">
					<button type="button" class={link} onclick={() => openSpell(s.spell)}>{s.name}</button>
					<span class="text-muted-foreground tabular-nums">{s.rounds} rounds{s.bless_slot === null || s.bless_slot === undefined ? ', no bless slot' : ''}</span>
				</div>
			{:else}
				<p class="text-muted-foreground text-sm">None running.</p>
			{/each}
			<h3 class="{headingCls} mt-3">Abilities ({detail.abilities.length})</h3>
			{#each detail.abilities as a (a.ability)}
				<div class="flex justify-between gap-2 border-b py-0.5 text-sm"><span>{a.name}</span><span class="text-muted-foreground tabular-nums">{a.value}</span></div>
			{:else}
				<p class="text-muted-foreground text-sm">None.</p>
			{/each}
		</div>
	</div>
</div>
