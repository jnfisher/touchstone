<script lang="ts">
	// The Gear Planner (touchstone's own): the worn gear that does the most for one goal, slot by slot beside what
	// the character wears now, with what each swap is worth, the stats before and after, and where to get it all.
	import { goto } from '$app/navigation';
	import { api, type Item, type Monster } from '$lib/api';
	import { monsterChoices, type MonsterChoice } from '$lib/calc';
	import { evalRequest } from '$lib/character';
	import { itemSlug } from '$lib/detail';
	import { selection } from '$lib/explorer-state.svelte';
	import {
		GEAR_PLAN_DEFAULTS,
		GOALS,
		PLAN_TARGET_FIELDS,
		chanceText,
		changePct,
		changedSlots,
		closedText,
		gearPlanClient,
		gearPlanRequest,
		goalSwings,
		itemNote,
		killList,
		num,
		orderedSlots,
		otherText,
		otherWays,
		shopList,
		signed,
		skipItem,
		type GearPlanForm,
		type PlanItem,
		type SlotOrder,
		type Source
	} from '$lib/gearplan';
	import { TARGET_FIELDS, targetFromMonster } from '$lib/weaponsim';
	import { Button } from '$lib/components/ui/button';
	import { useCurrentCharacter } from './current.svelte';
	import { live } from './live.svelte';
	import PlanHeader from './PlanHeader.svelte';
	import { cardCls, headingCls, inputCls, labelCls } from './styles';
	import WeaponPicker from './WeaponPicker.svelte';

	let form = $state<GearPlanForm>(structuredClone(GEAR_PLAN_DEFAULTS));
	let order = $state<SlotOrder>('slot');
	let picked = $state<number>();
	let showPlan = $state(false);

	let items = $state.raw<Item[]>([]);
	let monsters = $state.raw<Monster[]>([]);
	let loadError = $state<string>();
	$effect(() => {
		let alive = true;
		Promise.all([api.table('items'), api.table('monsters')])
			.then(([i, m]) => {
				if (!alive) return;
				items = i;
				monsters = m;
			})
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});
	const monsterList = $derived<MonsterChoice[]>(monsterChoices(monsters));

	const current = useCurrentCharacter();
	const charBody = $derived(evalRequest($state.snapshot(current.doc), $state.snapshot(current.options)));

	const calc = live(
		() => (charBody ? gearPlanRequest($state.snapshot(form) as GearPlanForm, charBody) : undefined),
		gearPlanClient.run,
		250
	);
	const plan = $derived(charBody ? calc.result : undefined);
	const swings = $derived(goalSwings(form.goal));
	const goal = $derived(GOALS.find((g) => g.value === form.goal));

	const rows = $derived(plan ? orderedSlots(plan, order) : []);
	const changes = $derived(plan ? changedSlots(plan) : []);
	const selected = $derived(plan?.slots.find((s) => s.slot === picked) ?? changes[0]);
	const kills = $derived(plan ? killList(plan) : []);
	const shops = $derived(plan ? shopList(plan) : []);
	const others = $derived(plan ? otherWays(plan) : []);
	const changedStats = $derived(plan ? plan.stats.filter((s) => s.before !== s.after) : []);

	function pickMonster() {
		const m = monsters.find((x) => x.number === form.monster);
		if (m) form.target = targetFromMonster(m);
	}
	/** "Next best": plan again without this slot's pick. */
	function nextBest(slot: number, item: number) {
		form.skipped = skipItem(form.skipped, item);
		picked = slot;
	}
	function toggleLock(slot: number) {
		form.locked = form.locked.includes(slot) ? form.locked.filter((s) => s !== slot) : [...form.locked, slot];
	}

	function open(slug: string, id: string | number) {
		selection.set(slug, id);
		void goto(`/${slug}`);
	}
	function openItem(n: number) {
		const it = items.find((i) => i.number === n);
		open(it ? itemSlug(it) : 'armour', n);
	}
	function openSource(s: Source) {
		if (s.kind === 'monster' || s.kind === 'chest' || s.kind === 'npc') open('monsters', s.number);
		else if (s.kind === 'shop') open('shops', s.number);
		else if (s.kind === 'room') open('rooms', `${s.map}/${s.number}`);
	}

	const check = 'flex items-center gap-1.5 text-sm';
	const th = 'text-muted-foreground px-2 py-1.5 text-left text-[11px] font-medium tracking-wide uppercase';
	const td = 'px-2 py-1.5 align-top';
	const link = 'text-primary text-left hover:underline';
	const gainCls = (n: number) => (n > 0 ? 'text-positive' : n < 0 ? 'text-destructive' : 'text-muted-foreground');
</script>

{#snippet itemCell(i: PlanItem | null, level: number)}
	{#if i}
		<button type="button" class={link} data-item={i.number} onclick={(e) => (e.stopPropagation(), openItem(i.number))}>{i.name}</button>
		{#if itemNote(i, level)}<span class="text-muted-foreground text-xs"> ({itemNote(i, level)})</span>{/if}
	{:else}
		<span class="text-muted-foreground">(nothing)</span>
	{/if}
{/snippet}

<svelte:window onkeydown={(e) => e.key === 'Escape' && (showPlan = false)} />

<div class="flex h-full min-h-0 flex-col gap-2 overflow-auto lg:overflow-hidden" data-testid="calc-gear-plan">
	<PlanHeader title="Gear Planner" {current} ready={charBody !== undefined} testid="gear" />

	{#if !charBody && current.override === undefined}
		<div class="{cardCls} text-sm" data-testid="gear-no-character">
			<p class="font-medium">No character is loaded.</p>
			<p class="text-muted-foreground mt-1">
				Pick one in the header, or give the Sandbox a class and race on the Character tab, and the gear that suits it best is planned here.
			</p>
		</div>
	{:else}
		<section class="{cardCls} flex flex-col gap-3 !p-3" aria-label="Options">
			<div class="flex flex-wrap items-end gap-x-5 gap-y-2">
				<label class="flex flex-col gap-1">
					<span class={labelCls}>Goal</span>
					<select bind:value={form.goal} class="{inputCls} !w-44" data-testid="gear-goal" title={goal?.hint}>
						{#each GOALS as g (g.value)}<option value={g.value}>{g.label}</option>{/each}
					</select>
				</label>
				<div class="flex flex-col gap-1.5">
					<span class={labelCls}>Which items</span>
					<div class="flex flex-wrap gap-x-4 gap-y-1">
						<label class={check} title="Keep items above the character's level, to plan ahead. The stats still use the character's level as it is now.">
							<input type="checkbox" bind:checked={form.ignoreLevel} data-testid="gear-ignore-level" />
							Ignore level
						</label>
						<label class={check} title="Keep items the character's alignment cannot wear">
							<input type="checkbox" bind:checked={form.ignoreAlignment} data-testid="gear-ignore-alignment" />
							Ignore alignment
						</label>
						<label class={check} title="Limited items exist in small numbers in the realm">
							<input type="checkbox" bind:checked={form.includeLimited} data-testid="gear-limited" />
							Limited items
						</label>
						<label class={check} title="Skip items that no room, shop, monster, chest or script in the data gives">
							<input type="checkbox" bind:checked={form.onlyObtainable} data-testid="gear-obtainable" />
							Obtainable only
						</label>
						<label class={check} title="Fill the second wrist slot too">
							<input type="checkbox" bind:checked={form.secondWrist} data-testid="gear-second-wrist" />
							Second wrist
						</label>
					</div>
				</div>
				<div class="flex flex-col gap-1">
					<span class={labelCls}>Must keep</span>
					<div class="flex flex-wrap items-center gap-2">
						<label class="flex items-center gap-1 text-sm" title="The set must keep at least this armour class; empty for no limit">
							<span class="text-muted-foreground text-xs">AC at least</span>
							<input type="text" inputmode="numeric" autocomplete="off" bind:value={form.minAc} data-testid="gear-min-ac" class="{inputCls} !w-16" />
						</label>
						<label class="flex items-center gap-1 text-sm" title="The set must stay at or under this encumbrance, in percent; empty for no limit">
							<span class="text-muted-foreground text-xs">Enc. at most %</span>
							<input type="text" inputmode="numeric" autocomplete="off" bind:value={form.maxEncum} data-testid="gear-max-encum" class="{inputCls} !w-16" />
						</label>
					</div>
				</div>
			</div>
			{#if swings}
				<div class="flex flex-col gap-1">
					<span class={labelCls}>Target</span>
					<div class="flex flex-wrap items-center gap-2">
						<div class="w-52">
							<WeaponPicker choices={monsterList} bind:value={form.monster} label="" noun="monster" testid="gear-monster" clearable onchoose={pickMonster} />
						</div>
						{#each TARGET_FIELDS.filter((f) => PLAN_TARGET_FIELDS.includes(f.key)) as f (f.key)}
							<label class="flex items-center gap-1 text-sm" title={f.hint}>
								<span class="text-muted-foreground text-xs">{f.label}</span>
								<input type="text" inputmode="numeric" autocomplete="off" bind:value={form.target[f.key]} data-testid="gear-target-{f.key}" class="{inputCls} !w-16" />
							</label>
						{/each}
					</div>
				</div>
			{/if}
		</section>

		{#if calc.error || loadError}
			<p class="text-destructive text-sm" role="alert" data-testid="gear-error">{calc.error ?? loadError}</p>
		{:else if !plan}
			<p class="text-muted-foreground text-sm">Planning...</p>
		{:else}
			<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
				<span class="text-sm" data-testid="gear-summary">
					<span class="text-muted-foreground">{plan.metric}</span>
					<span class="font-semibold tabular-nums">{num(plan.before)} to {num(plan.after)}</span>
					{#if changePct(plan.before, plan.after)}<span class="text-positive tabular-nums">{changePct(plan.before, plan.after)}</span>{/if}
					<span class="text-muted-foreground">
						with {changes.length} {changes.length === 1 ? 'change' : 'changes'}; {plan.evaluations.toLocaleString('en-US')} sets tried from {plan.candidates} items in {Math.round(plan.elapsed_ms)} ms
					</span>
				</span>
				<div class="ml-auto flex items-center gap-2">
					<label class="flex items-center gap-1 text-sm">
						<span class="text-muted-foreground text-xs">Order</span>
						<select bind:value={order} class="{inputCls} !w-44" data-testid="gear-order">
							<option value="slot">By slot</option>
							<option value="gain">Biggest gain first</option>
						</select>
					</label>
					<Button size="sm" disabled={changes.length === 0} onclick={() => (showPlan = true)} data-testid="gear-where">Where to get it</Button>
				</div>
			</div>
			{#if plan.skipped.length}
				<div class="flex flex-wrap items-center gap-1.5 text-xs" data-testid="gear-skipped">
					<span class="text-muted-foreground">Planned without:</span>
					{#each plan.skipped as sk (sk.number)}
						<button
							type="button"
							class="bg-muted hover:bg-accent rounded-md border px-2 py-0.5"
							title="Allow {sk.name} again"
							data-item={sk.number}
							onclick={() => (form.skipped = form.skipped.filter((n) => n !== sk.number))}
						>
							{sk.name} ×
						</button>
					{/each}
					<button type="button" class="text-primary underline" data-testid="gear-skipped-clear" onclick={() => (form.skipped = [])}>Allow all again</button>
				</div>
			{/if}
			{#each plan.unmet as u (u)}
				<p class="text-destructive text-sm" role="alert" data-testid="gear-unmet">Nothing this character can wear meets the limit: {u}.</p>
			{/each}

			<div class="grid gap-3 lg:min-h-0 lg:flex-1 lg:grid-cols-[minmax(0,3fr)_minmax(18rem,2fr)]">
				<div class="bg-card rounded-md border lg:min-h-0 lg:overflow-auto">
					<table class="w-full border-collapse text-[13px]" data-testid="gear-slots">
						<thead class="bg-muted sticky top-0 z-10">
							<tr>
								<th class={th} title="Leave this slot as it is">Lock</th>
								<th class={th}>Slot</th>
								<th class={th}>Now</th>
								<th class={th}>Suggested</th>
								<th class="{th} text-right" title="What this swap alone adds to the goal on today's gear">Gain</th>
							</tr>
						</thead>
						<tbody>
							{#each rows as s (s.slot)}
								{@const fixed = s.closed !== null && s.closed !== 'locked'}
								<tr
									class="hover:bg-accent/60 cursor-pointer border-t {selected?.slot === s.slot ? 'bg-accent' : ''} {s.changed ? '' : 'text-muted-foreground'}"
									data-testid="gear-slot-{s.slot}"
									onclick={() => (picked = s.slot)}
								>
									<td class={td}>
										{#if !fixed}
											<input
												type="checkbox"
												checked={form.locked.includes(s.slot)}
												aria-label="Lock {s.name}"
												onclick={(e) => e.stopPropagation()}
												onchange={() => toggleLock(s.slot)}
											/>
										{/if}
									</td>
									<td class="{td} whitespace-nowrap">{s.name}</td>
									<td class={td}>{@render itemCell(s.current, plan.level)}</td>
									<td class={td}>
										{#if s.changed}
											<span class="flex items-start justify-between gap-2">
												<span class="text-foreground">{@render itemCell(s.suggested, plan.level)}</span>
												{#if s.suggested}
													{@const pick = s.suggested}
													<button
														type="button"
														class="border-input bg-background text-foreground hover:bg-accent shrink-0 rounded-md border px-1.5 py-0.5 text-xs whitespace-nowrap"
														title="Plan again without {pick.name}: the next best for this slot. Click again to go further down the list."
														data-testid="gear-next-{s.slot}"
														onclick={(e) => (e.stopPropagation(), nextBest(s.slot, pick.number))}
													>
														Next best
													</button>
												{/if}
											</span>
										{:else if s.closed}
											<span class="text-xs">{closedText(s.closed)}</span>
										{:else}
											<span class="text-xs">keep</span>
										{/if}
									</td>
									<td class="{td} text-right tabular-nums {s.changed ? gainCls(s.gain) : ''}">{s.changed ? signed(s.gain) : ''}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>

				<div class="flex flex-col gap-3 lg:min-h-0 lg:overflow-auto">
					{#if selected}
						<section class="{cardCls} !p-3 text-[13px]" data-testid="gear-detail">
							<h3 class="mb-1 text-sm font-semibold">{selected.name}</h3>
							{#if selected.changed}
								<dl class="mb-2 grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-0.5">
									<dt class="text-muted-foreground">Wear</dt>
									<dd>
										{@render itemCell(selected.suggested, plan.level)}
										{#if selected.suggested}
											<div class="text-muted-foreground text-xs">
												AC {num(selected.suggested.armour_class / 10)}, DR {num(selected.suggested.damage_resist / 10)}, weight {selected.suggested.encum}
											</div>
										{/if}
									</dd>
									<dt class="text-muted-foreground">In place of</dt>
									<dd>{@render itemCell(selected.current, plan.level)}</dd>
									<dt class="text-muted-foreground" title="What this swap alone adds on today's gear">On its own</dt>
									<dd><span class="tabular-nums {gainCls(selected.gain)}">{signed(selected.gain)}</span> {plan.metric.toLowerCase()}</dd>
								</dl>
								{#if selected.acquisition}
									<h4 class={headingCls}>Where to get it</h4>
									{#if selected.acquisition.sources.length}
										<ul class="mb-2 flex flex-col gap-0.5">
											{#each selected.acquisition.sources as src, k (k)}
												<li>
													{#if src.kind === 'monster'}
														Kill <button type="button" class={link} onclick={() => openSource(src)}>{src.name}</button>: {chanceText(src)}
													{:else if src.kind === 'chest'}
														Kill <button type="button" class={link} onclick={() => openSource(src)}>{src.name}</button> for
														{src.chest_name}: {chanceText(src)}
													{:else if src.kind === 'shop'}
														Buy at <button type="button" class={link} onclick={() => openSource(src)}>{src.name}</button>{src.one_time ? ' (not restocked)' : ''}
													{:else if src.kind === 'script'}
														{otherText(src)}
													{:else}
														<button type="button" class={link} onclick={() => openSource(src)}>{otherText(src)}</button>
													{/if}
												</li>
											{/each}
										</ul>
										{#if selected.acquisition.more}<p class="text-muted-foreground text-xs">{selected.acquisition.more} more not shown; the item's own page lists them all.</p>{/if}
									{:else}
										<p class="text-muted-foreground mb-2">Nothing in the data says where this comes from.</p>
									{/if}
								{/if}
							{:else}
								<p class="text-muted-foreground mb-2">
									{selected.closed ? `Left alone: ${closedText(selected.closed)}.` : 'Nothing offered here does more for this goal than what is worn.'}
								</p>
							{/if}
							{#if selected.alternatives.length}
								<h4 class={headingCls}>Next best here</h4>
								<ul class="flex flex-col gap-0.5">
									{#each selected.alternatives as a (a.item.number)}
										<li class="flex justify-between gap-3">
											<span>{@render itemCell(a.item, plan.level)}</span>
											<span class="tabular-nums {gainCls(a.delta)}" title="{plan.metric} with this item in the suggested set, against the suggestion">{signed(a.delta)}</span>
										</li>
									{/each}
								</ul>
							{/if}
						</section>
					{/if}
					<section class="{cardCls} !p-3 text-[13px]" data-testid="gear-stats">
						<h3 class={headingCls}>What changes</h3>
						{#if changedStats.length}
							<table class="w-full border-collapse">
								<tbody>
									{#each changedStats as st (st.label)}
										<tr class="border-b last:border-0">
											<td class="py-0.5">{st.label}</td>
											<td class="text-muted-foreground py-0.5 text-right tabular-nums">{num(st.before)}</td>
											<td class="py-0.5 text-right tabular-nums">{num(st.after)}</td>
											<td class="w-16 py-0.5 text-right tabular-nums {st.label === 'Weight carried' || st.label === 'Encumbrance %' ? 'text-muted-foreground' : gainCls(st.after - st.before)}">{signed(Number((st.after - st.before).toFixed(1)))}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						{:else}
							<p class="text-muted-foreground">The gear worn now is already the best found for this goal.</p>
						{/if}
						<p class="text-muted-foreground mt-2 text-xs">
							Each set is judged by the character's full sheet, so weight, strength and accuracy count for what they do. The search improves one slot at a time until no swap helps; a pair of items that only pays off together can be missed.
						</p>
					</section>
				</div>
			</div>
		{/if}
	{/if}
</div>

{#if showPlan && plan}
	<div class="bg-background/80 fixed inset-0 z-[60] flex items-center justify-center p-4" role="presentation" onclick={() => (showPlan = false)}>
		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
		<div
			class="bg-card flex max-h-[85vh] w-full max-w-3xl flex-col gap-3 rounded-lg border p-4 text-[13px] shadow-xl"
			role="dialog"
			tabindex="-1"
			aria-modal="true"
			aria-label="Where to get the suggested gear"
			data-testid="gear-where-dialog"
			onclick={(e) => e.stopPropagation()}
		>
			<h2 class="text-base font-semibold">Where to get the suggested gear</h2>
			<div class="flex min-h-0 flex-col gap-4 overflow-auto">
				<section data-testid="gear-kill-list">
					<h3 class={headingCls}>Kill list ({kills.length} {kills.length === 1 ? 'monster' : 'monsters'})</h3>
					{#if kills.length}
						<table class="w-full border-collapse">
							<tbody>
								{#each kills as k (k.monster)}
									<tr class="border-b align-top last:border-0">
										<td class="w-56 py-1 pr-3">
											<button type="button" class={link} onclick={() => open('monsters', k.monster)}>{k.name}</button>
										</td>
										<td class="py-1">
											{#each k.drops as d (d.item)}
												<div>
													<button type="button" class={link} data-item={d.item} onclick={() => openItem(d.item)}>{d.itemName}</button>
													<span class="text-muted-foreground">({d.slot})</span>:
													<span class="tabular-nums">{chanceText(d)}</span>{#if d.chestName}<span class="text-muted-foreground">, in {d.chestName}</span>{/if}
												</div>
											{/each}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					{:else}
						<p class="text-muted-foreground">No monster drops any of the suggested items.</p>
					{/if}
				</section>
				<section data-testid="gear-shop-list">
					<h3 class={headingCls}>Shops ({shops.length})</h3>
					{#if shops.length}
						<table class="w-full border-collapse">
							<tbody>
								{#each shops as s (s.shop)}
									<tr class="border-b align-top last:border-0">
										<td class="w-56 py-1 pr-3"><button type="button" class={link} onclick={() => open('shops', s.shop)}>{s.name}</button></td>
										<td class="py-1">
											{#each s.items as i (i.item)}
												<div>
													<button type="button" class={link} data-item={i.item} onclick={() => openItem(i.item)}>{i.itemName}</button>
													<span class="text-muted-foreground">({i.slot}{i.oneTime ? ', not restocked' : ''})</span>
												</div>
											{/each}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					{:else}
						<p class="text-muted-foreground">No shop sells any of the suggested items.</p>
					{/if}
				</section>
				{#if others.length}
					<section data-testid="gear-other-list">
						<h3 class={headingCls}>Found, given or scripted</h3>
						<table class="w-full border-collapse">
							<tbody>
								{#each others as w (w.item)}
									<tr class="border-b align-top last:border-0">
										<td class="w-56 py-1 pr-3">
											<button type="button" class={link} data-item={w.item} onclick={() => openItem(w.item)}>{w.itemName}</button>
											<span class="text-muted-foreground">({w.slot})</span>
										</td>
										<td class="py-1">
											{#if w.nothing}
												<span class="text-muted-foreground">Nothing in the data says where this comes from.</span>
											{/if}
											{#each w.sources as src, k (k)}
												<div>
													{#if src.kind === 'script'}{otherText(src)}{:else}<button type="button" class={link} onclick={() => openSource(src)}>{otherText(src)}</button>{/if}
												</div>
											{/each}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</section>
				{/if}
				<p class="text-muted-foreground text-xs">
					Kills are the average to expect at the drop chance shown; a chest's chance is the monster's chance to drop the chest times the item's chance inside it. An item with several sources appears under each. Up to six sources of each kind are listed for an item.
				</p>
			</div>
			<div class="flex justify-end">
				<Button size="sm" variant="outline" onclick={() => (showPlan = false)} data-testid="gear-where-close">Close</Button>
			</div>
		</div>
	</div>
{/if}
