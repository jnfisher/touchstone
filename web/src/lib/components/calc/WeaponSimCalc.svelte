<script lang="ts">
	// The Weapon Simulator (touchstone's own): the best weapons for the character in use, by simulated damage.
	// Regular rounds (attack and bash) and backstab rounds are simulated apart and listed in their own views; the
	// selected weapon's spread, MME's figure beside the simulated one, and its item pane sit on the right.
	import { goto } from '$app/navigation';
	import { untrack } from 'svelte';
	import { api, type Item, type Monster } from '$lib/api';
	import { monsterChoices, weaponChoices, type MonsterChoice, type WeaponChoice } from '$lib/calc';
	import { evalRequest } from '$lib/character';
	import { linkFor, paneClient, type Pane, type RefTarget } from '$lib/detail';
	import { selection } from '$lib/explorer-state.svelte';
	import { DEFAULT_FILTER } from '$lib/globalfilter';
	import DataGrid from '$lib/grid/DataGrid.svelte';
	import {
		CANDIDATE_CHOICES,
		MAX_PINNED,
		MODE_LABEL,
		ROUND_CHOICES,
		TARGET_FIELDS,
		WEAPON_SIM_DEFAULTS,
		bestRegular,
		killText,
		rowNote,
		signedPct,
		targetFromMonster,
		versusEquipped,
		viewMode,
		viewRows,
		weaponSimClient,
		weaponSimColumns,
		weaponSimRequest,
		type ModeKey,
		type SimMode,
		type SimView,
		type WeaponSimForm,
		type WeaponSimRow
	} from '$lib/weaponsim';
	import DetailPane from '../DetailPane.svelte';
	import SplitPane from '../SplitPane.svelte';
	import PlanHeader from './PlanHeader.svelte';
	import { useCurrentCharacter } from './current.svelte';
	import { live } from './live.svelte';
	import { cardCls, headingCls, inputCls, labelCls } from './styles';
	import WeaponPicker from './WeaponPicker.svelte';

	let form = $state<WeaponSimForm>(structuredClone(WEAPON_SIM_DEFAULTS));
	let view = $state<SimView>('regular');
	let picked = $state<number>();
	let search = $state('');
	let shown = $state(0);
	let gridWidth = $state<number>();
	let collapsed = $state(false);
	let pin = $state<number | null>(null);

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
	const weapons = $derived<WeaponChoice[]>(weaponChoices(items));
	const monsterList = $derived<MonsterChoice[]>(monsterChoices(monsters));

	const current = useCurrentCharacter();
	// The evaluate body of the character in use; undefined until it has a class and a race.
	const charBody = $derived(evalRequest($state.snapshot(current.doc), $state.snapshot(current.options)));

	const calc = live(
		() => (charBody ? weaponSimRequest($state.snapshot(form) as WeaponSimForm, charBody) : undefined),
		weaponSimClient.run,
		200
	);
	const result = $derived(charBody ? calc.result : undefined);

	// A character whose class and race cannot backstab has nothing to show in that view.
	$effect(() => {
		if (result && !result.can_backstab && view === 'backstab') view = 'regular';
	});

	const rows = $derived(result ? viewRows(result, view) : []);
	const columns = $derived(result ? weaponSimColumns(view, result) : []);
	const selected = $derived(rows.find((r) => r.number === picked));
	// Nothing picked yet, or the pick left the list: show the view's best weapon.
	$effect(() => {
		if (!rows.length || rows.some((r) => r.number === picked)) return;
		const v = untrack(() => view);
		picked = rows.reduce((a, b) => ((viewMode(b, v)?.sim.mean ?? 0) > (viewMode(a, v)?.sim.mean ?? 0) ? b : a)).number;
	});

	/** Switching views starts at that view's best weapon. */
	function show(v: SimView) {
		if (v === view) return;
		view = v;
		picked = undefined;
	}
	function pickMonster() {
		const m = monsters.find((x) => x.number === form.monster);
		if (m) form.target = targetFromMonster(m);
	}
	function addPin() {
		if (pin !== null && !form.pinned.includes(pin) && form.pinned.length < MAX_PINNED) form.pinned = [...form.pinned, pin];
		pin = null;
	}
	const pinnedNames = $derived(
		form.pinned.map((n) => ({ number: n, name: items.find((i) => i.number === n)?.name ?? `#${n}` }))
	);

	// ---- the selected weapon's item pane (MME's detail text, where it comes from) ----
	let pane = $state.raw<Pane>();
	let paneError = $state<string>();
	$effect(() => {
		const n = selected?.number;
		const body = charBody;
		if (n === undefined || !body) {
			pane = undefined;
			return;
		}
		const ctl = new AbortController();
		const timer = setTimeout(() => {
			paneClient
				.pane('items', n, { character: body, global_filter: { ...DEFAULT_FILTER, use_character: true }, options: {} }, ctl.signal)
				.then((p) => {
					pane = p;
					paneError = undefined;
				})
				.catch((e: unknown) => {
					if (!ctl.signal.aborted) paneError = e instanceof Error ? e.message : String(e);
				});
		}, 100);
		return () => {
			clearTimeout(timer);
			ctl.abort();
		};
	});
	const livePane = $derived(pane && pane.number === selected?.number ? pane : undefined);

	function follow(t: RefTarget | null) {
		if (!t) return;
		const to = linkFor(t, new Map(items.map((i) => [i.number, i])));
		if (!to) return;
		selection.set(to.slug, to.id);
		void goto(`/${to.slug}`);
	}
	function openInWeapons(n: number) {
		selection.set('weapons', n);
		void goto('/weapons');
	}

	const MODES: ModeKey[] = ['attack', 'bash', 'backstab'];
	const modesOf = (r: WeaponSimRow) =>
		MODES.map((k) => ({ key: k, mode: r[k] })).filter((m): m is { key: ModeKey; mode: SimMode } => m.mode !== null);
	/** The widest round among the weapon's attacks, so their spread bars share one scale. */
	const scaleOf = (r: WeaponSimRow) => Math.max(1, ...modesOf(r).map((m) => m.mode.sim.max));
	const pct = (n: number, of: number) => `${Math.min(100, (n / of) * 100)}%`;

	const check = 'flex items-center gap-1.5 text-sm';
	const th = 'text-muted-foreground px-2 py-1 text-right text-[11px] font-medium tracking-wide uppercase';
	const td = 'px-2 py-1 text-right tabular-nums';
	const tab = (on: boolean) =>
		`h-8 px-3 text-sm first:rounded-l-md last:rounded-r-md border ${on ? 'bg-secondary text-secondary-foreground font-medium' : 'bg-background hover:bg-accent'} disabled:opacity-50`;
</script>

<div class="flex h-full min-h-0 flex-col gap-2" data-testid="calc-weapon-sim">
	<PlanHeader title="Weapon Simulator" {current} ready={charBody !== undefined} testid="wsim" />

	{#if !charBody && current.override === undefined}
		<div class="{cardCls} text-sm" data-testid="wsim-no-character">
			<p class="font-medium">No character is loaded.</p>
			<p class="text-muted-foreground mt-1">
				Pick one in the header, or give the Sandbox a class and race on the Character tab, and the best weapons for it are listed here.
			</p>
		</div>
	{:else}
		<section class="{cardCls} flex flex-col gap-3 !p-3" aria-label="Options">
			<div class="flex flex-wrap items-end gap-x-5 gap-y-2">
				<div class="flex flex-col gap-1.5">
					<span class={labelCls}>Which weapons</span>
					<div class="flex flex-wrap gap-x-4 gap-y-1">
						<label class={check} title="Keep weapons above the character's level, to plan ahead. Damage still uses the character's stats as they are now.">
							<input type="checkbox" bind:checked={form.ignoreLevel} data-testid="wsim-ignore-level" />
							Ignore level
						</label>
						<label class={check} title="Keep weapons the character's alignment cannot wield">
							<input type="checkbox" bind:checked={form.ignoreAlignment} data-testid="wsim-ignore-alignment" />
							Ignore alignment
						</label>
						<label class={check} title="Limited items exist in small numbers in the realm">
							<input type="checkbox" bind:checked={form.includeLimited} data-testid="wsim-limited" />
							Limited items
						</label>
						<label class={check} title="Skip weapons that no room, shop, monster, chest or script in the data gives: leftovers such as test items">
							<input type="checkbox" bind:checked={form.onlyObtainable} data-testid="wsim-obtainable" />
							Obtainable only
						</label>
					</div>
				</div>
				<div class="flex flex-col gap-1">
					<span class={labelCls}>Target</span>
					<div class="flex flex-wrap items-center gap-2">
						<div class="w-52">
							<WeaponPicker
								choices={monsterList}
								bind:value={form.monster}
								label=""
								noun="monster"
								testid="wsim-monster"
								clearable
								onchoose={pickMonster}
							/>
						</div>
						{#each TARGET_FIELDS as f (f.key)}
							<label class="flex items-center gap-1 text-sm" title={f.hint}>
								<span class="text-muted-foreground text-xs">{f.label}</span>
								<input
									type="text"
									inputmode="numeric"
									autocomplete="off"
									bind:value={form.target[f.key]}
									data-testid="wsim-target-{f.key}"
									class="{inputCls} !w-16"
								/>
							</label>
						{/each}
					</div>
				</div>
			</div>
			<div class="flex flex-wrap items-end gap-x-5 gap-y-2">
				<label class="flex flex-col gap-1" title="How many of the best weapons by each attack are simulated; the rest are ruled out by MME's own damage figure">
					<span class={labelCls}>Shortlist</span>
					<select bind:value={form.candidates} class="{inputCls} !w-28" data-testid="wsim-candidates">
						{#each CANDIDATE_CHOICES as n (n)}<option value={n}>Top {n}</option>{/each}
					</select>
				</label>
				<label class="flex flex-col gap-1" title="Rounds rolled for each attack of each weapon; more is steadier and slower">
					<span class={labelCls}>Rounds</span>
					<select bind:value={form.rounds} class="{inputCls} !w-28" data-testid="wsim-rounds">
						{#each ROUND_CHOICES as n (n)}<option value={n}>{n.toLocaleString('en-US')}</option>{/each}
					</select>
				</label>
				<button
					type="button"
					class="border-input hover:bg-accent h-8 rounded-md border px-3 text-sm"
					title="Roll again with new dice (seed {form.seed}); a small change between rolls is the simulation's noise"
					data-testid="wsim-reroll"
					onclick={() => (form.seed = Math.floor(Math.random() * 0xffffff) + 1)}
				>
					Roll again
				</button>
				<div class="flex flex-col gap-1">
					<span class={labelCls}>Always include</span>
					<div class="flex flex-wrap items-center gap-2">
						<div class="w-56">
							<WeaponPicker choices={weapons} bind:value={pin} label="" testid="wsim-pin" onchoose={addPin} />
						</div>
						{#each pinnedNames as p (p.number)}
							<button
								type="button"
								class="bg-muted hover:bg-accent rounded-md border px-2 py-0.5 text-xs"
								title="Stop including {p.name}"
								onclick={() => (form.pinned = form.pinned.filter((n) => n !== p.number))}
							>
								{p.name} ×
							</button>
						{/each}
					</div>
				</div>
			</div>
		</section>

		<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
			<div class="flex" role="tablist" aria-label="Rounds to rank by">
				<button type="button" role="tab" aria-selected={view === 'regular'} class={tab(view === 'regular')} data-testid="wsim-view-regular" onclick={() => show('regular')}>
					Regular rounds
				</button>
				<button
					type="button"
					role="tab"
					aria-selected={view === 'backstab'}
					class={tab(view === 'backstab')}
					disabled={result !== undefined && !result.can_backstab}
					title={result && !result.can_backstab ? "This character's class and race cannot backstab" : undefined}
					data-testid="wsim-view-backstab"
					onclick={() => show('backstab')}
				>
					Backstab rounds
				</button>
			</div>
			<input
				type="search"
				placeholder="Search..."
				aria-label="Search weapons"
				bind:value={search}
				class="bg-background border-input placeholder:text-muted-foreground focus-visible:ring-ring h-8 w-48 rounded-md border px-3 text-sm outline-none focus-visible:ring-2"
			/>
			<span class="text-muted-foreground text-sm tabular-nums" data-testid="wsim-summary">
				{#if result}
					{shown} shown: simulated {result.simulated} of {result.usable} usable weapons, {result.rounds.toLocaleString('en-US')} rounds each, in {result.elapsed_ms < 10 ? result.elapsed_ms.toFixed(1) : Math.round(result.elapsed_ms)} ms
				{:else if !calc.error}
					Simulating...
				{/if}
			</span>
			<button
				type="button"
				class="bg-background border-input hover:bg-accent ml-auto h-8 rounded-md border px-3 text-sm"
				aria-pressed={!collapsed}
				onclick={() => (collapsed = !collapsed)}
			>
				{collapsed ? 'Show details' : 'Hide details'}
			</button>
		</div>

		<div class="relative min-h-0 flex-1">
			{#if calc.error || loadError}
				<p class="text-destructive text-sm" role="alert" data-testid="wsim-error">{calc.error ?? loadError}</p>
			{:else if !result}
				<p class="text-muted-foreground text-sm">Simulating...</p>
			{:else if !rows.length}
				<p class="text-muted-foreground text-sm" data-testid="wsim-empty">
					{view === 'backstab' ? 'No weapon this character can wield has backstab accuracy.' : 'No weapon this character can wield deals damage to this target.'}
				</p>
			{:else}
				<SplitPane storageKey="touchstone.split.weapon-sim" autoWidth={gridWidth === undefined ? undefined : Math.min(gridWidth, 900)} bind:collapsed>
					{#snippet left()}
						{#key view}
							<DataGrid
								{rows}
								columnDefs={columns}
								getRowId={(r: WeaponSimRow) => String(r.number)}
								selectedId={picked === undefined ? undefined : String(picked)}
								quickFilter={search}
								storageKey="weapon-sim-{view}"
								rowState={(r: WeaponSimRow) => (r.needs_level || r.wrong_alignment || r.unobtainable ? 'muted' : undefined)}
								onSelect={(r: WeaponSimRow) => (picked = r.number)}
								onRowDoubleClick={(r: WeaponSimRow) => openInWeapons(r.number)}
								onCount={(n) => (shown = n)}
								onWidth={(w) => (gridWidth = w)}
							/>
						{/key}
					{/snippet}
					{#snippet right()}
						{#if selected}
							{@const r = selected}
							{@const scale = scaleOf(r)}
							{@const versus = versusEquipped(r, result, view)}
							<DetailPane groups={[{ title: `${r.name} (${r.number})`, rows: [] }]} pane={livePane} error={paneError} onLink={(l) => follow(l.target)}>
								{#snippet before()}
									<section class="mb-4" data-testid="wsim-detail">
										<p class="mb-2">
											{r.min}-{r.max} damage, speed {r.speed}, {r.two_handed ? 'two-handed' : 'one-handed'}{r.str_req ? `, ${r.str_req} strength` : ''}{r.level ? `, level ${r.level}` : ''}{rowNote(r, result.level) ? ` (${rowNote(r, result.level)})` : ''}.
											{#if versus !== undefined && !r.equipped}
												<span class={Math.round(versus) >= 0 ? 'text-positive' : 'text-destructive'}>{signedPct(versus)}</span> against the weapon in hand in {view === 'backstab' ? 'backstab' : 'regular'} rounds.
											{/if}
											<button type="button" class="text-primary underline" data-testid="wsim-open-weapon" onclick={() => openInWeapons(r.number)}>Open in Weapons</button>
										</p>
										<h3 class={headingCls}>Simulated damage a round</h3>
										<table class="w-full border-collapse text-[13px]">
											<thead>
												<tr class="border-b">
													<th class="{th} text-left"></th>
													<th class={th} title="Mean of the simulated rounds">Avg</th>
													<th class={th} title="MME's damage a round for the same attack">MME</th>
													<th class={th} title="One round in ten deals this or less">Low</th>
													<th class={th}>Median</th>
													<th class={th} title="One round in ten deals this or more">High</th>
													<th class={th}>Max</th>
													<th class={th} title="Rounds that dealt nothing">None</th>
													{#if result.target.hp > 0}<th class={th} title="Mean rounds to bring the target down; ~ marks a fight too long to roll, estimated from the average">Kill</th>{/if}
												</tr>
											</thead>
											<tbody>
												{#each modesOf(r) as m (m.key)}
													{@const s = m.mode.sim}
													<tr class="border-b last:border-0 {m.key === (view === 'backstab' ? 'backstab' : bestRegular(r)?.key) ? 'font-medium' : ''}" data-testid="wsim-mode-{m.key}">
														<td class="px-2 py-1 text-left">{MODE_LABEL[m.key]}</td>
														<td class={td}>{s.mean.toFixed(1)}</td>
														<td class="{td} text-muted-foreground">{m.mode.expected}</td>
														<td class={td}>{s.p10}</td>
														<td class={td}>{s.median}</td>
														<td class={td}>{s.p90}</td>
														<td class={td}>{s.max}</td>
														<td class={td}>{s.zero_pct < 0.05 ? '-' : `${s.zero_pct.toFixed(s.zero_pct < 10 ? 1 : 0)}%`}</td>
														{#if result.target.hp > 0}<td class={td}>{killText(m.mode.kill) || '-'}</td>{/if}
													</tr>
													<tr class="border-b last:border-0">
														<td></td>
														<td colspan={result.target.hp > 0 ? 8 : 7} class="px-2 pb-1.5">
															<!-- Where a round lands: the band is the middle 80% of rounds, the mark the median. -->
															<div class="bg-muted relative h-2 rounded-sm" role="img" aria-label="{MODE_LABEL[m.key]}: most rounds deal {s.p10} to {s.p90}">
																<div class="bg-primary/40 absolute inset-y-0 rounded-sm" style:left={pct(s.p10, scale)} style:width={pct(Math.max(s.p90 - s.p10, scale / 200), scale)}></div>
																<div class="bg-primary absolute inset-y-0 w-0.5" style:left={pct(s.median, scale)}></div>
															</div>
															<div class="text-muted-foreground mt-0.5 text-xs">{m.mode.detail}</div>
														</td>
													</tr>
												{/each}
											</tbody>
										</table>
										{#if r.backstab && view === 'regular'}
											<p class="text-muted-foreground mt-1 text-xs">Backstab is the opening round only; see the Backstab rounds view to rank by it.</p>
										{/if}
									</section>
								{/snippet}
							</DetailPane>
						{:else}
							<div class="text-muted-foreground bg-card h-full rounded-md border p-3 text-[13px]">Select a weapon to see its numbers.</div>
						{/if}
					{/snippet}
				</SplitPane>
			{/if}
		</div>
	{/if}
</div>
