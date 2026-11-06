<script lang="ts">
	// frmMonsterAttackSim: one monster's attacks against a defender over many rounds. The defender is
	// the current character (read from the Character tab, like cmdResetUserDefs_Click) or typed boxes.
	import { api, type ClassRec } from '$lib/api';
	import {
		calcClient,
		monsterChoices,
		pctText,
		SIM_BOXES,
		SIM_DEFAULT_SEED,
		SIM_DEFAULTS,
		SIM_MAX_ROUNDS,
		simRequest,
		type MonsterChoice,
		type SimBoxes,
		type SimForm,
		type SimResult
	} from '$lib/calc';
	import { evalRequest } from '$lib/character';
	import { getCharacterStore } from '$lib/character.svelte';
	import CalcFrame from './CalcFrame.svelte';
	import { useCurrentCharacter } from './current.svelte';
	import Field from './Field.svelte';
	import { headingCls, inputCls, labelCls } from './styles';
	import UseCharacter from './UseCharacter.svelte';
	import WeaponPicker from './WeaponPicker.svelte';

	let form = $state<SimForm>(structuredClone(SIM_DEFAULTS));
	let monsters = $state.raw<MonsterChoice[]>([]);
	let classes = $state.raw<ClassRec[]>([]);
	let result = $state.raw<SimResult>();
	let error = $state<string>();
	let loadError = $state<string>();
	let running = $state(false);

	const store = getCharacterStore();
	const current = useCurrentCharacter();

	// The evaluate body the server turns into the defense boxes; undefined without a usable character.
	const charBody = $derived(
		current.active ? evalRequest($state.snapshot(store.doc), $state.snapshot(store.options)) : undefined
	);
	let charBoxes = $state.raw<SimBoxes>();
	let charError = $state<string>();
	$effect(() => {
		const body = charBody;
		if (!body) {
			charBoxes = undefined;
			charError = undefined;
			return;
		}
		let alive = true;
		calcClient
			.monsterSimDefender(body)
			.then((r) => {
				if (!alive) return;
				charBoxes = r.defender;
				charError = undefined;
			})
			.catch((e: unknown) => alive && (charError = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});

	$effect(() => {
		let alive = true;
		Promise.all([api.table('monsters'), api.table('classes')])
			.then(([m, c]) => {
				if (!alive) return;
				monsters = monsterChoices(m);
				classes = c.filter((x) => x.name !== '');
				// Form_Load selects the first monster.
				form.monster = monsters[0]?.number ?? null;
			})
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});

	const locked = $derived(charBody !== undefined);
	const shown = $derived(locked && charBoxes ? charBoxes : form.boxes);

	async function run() {
		if (form.monster === null || running) return;
		running = true;
		try {
			result = await calcClient.monsterSim(simRequest($state.snapshot(form) as SimForm, charBody));
			error = undefined;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			running = false;
		}
	}

	/** cmdResetUserDefs_Click(0). */
	function resetDefenses() {
		form.boxes = { ...SIM_BOXES };
		form.alwaysDodge = false;
	}
	function newSeed() {
		form.seed = String(Math.floor(Math.random() * 0x1000000));
	}

	const tile = 'bg-background rounded-md border p-3';
	const tileLabel = 'text-muted-foreground text-[11px] font-medium uppercase tracking-wide';
	const th = 'text-muted-foreground px-2 py-1 text-right text-[11px] font-medium uppercase tracking-wide';
	const td = 'px-2 py-1 text-right tabular-nums';
	const resists: { key: keyof SimBoxes; label: string }[] = [
		{ key: 'resist_cold', label: 'Cold' },
		{ key: 'resist_fire', label: 'Fire' },
		{ key: 'resist_stone', label: 'Stone' },
		{ key: 'resist_lightning', label: 'Lightning' },
		{ key: 'resist_water', label: 'Water' }
	];
</script>

<CalcFrame title="Monster Attack Simulator" testid="calc-monster-sim">
	{#snippet inputs()}
		<div class="flex flex-col gap-4">
			<WeaponPicker
				choices={monsters}
				bind:value={form.monster}
				label="Monster"
				noun="monster"
				testid="sim-monster"
			/>

			<details open class="flex flex-col gap-3" data-testid="sim-defender">
				<summary class="{labelCls} cursor-pointer">Defender</summary>
				<div class="mt-3 flex flex-col gap-3">
					<UseCharacter {current} />
					{#if charError}<p class="text-destructive text-xs" role="alert">{charError}</p>{/if}
					<div class="grid grid-cols-3 gap-3">
						<Field label="AC" bind:value={() => shown.ac, (v) => (form.boxes.ac = v)} disabled={locked} testid="sim-ac" />
						<Field label="DR" bind:value={() => shown.dr, (v) => (form.boxes.dr = v)} disabled={locked} testid="sim-dr" />
						<Field label={form.alwaysDodge ? 'Dodge %' : 'Dodge'} bind:value={() => shown.dodge, (v) => (form.boxes.dodge = v)} disabled={locked} testid="sim-dodge" />
						<Field label="MR" bind:value={() => shown.mr, (v) => (form.boxes.mr = v)} disabled={locked} testid="sim-mr" />
						<Field label="Prot. Evil" bind:value={() => shown.prot_evil, (v) => (form.boxes.prot_evil = v)} disabled={locked} testid="sim-prot-evil" />
						<label class="flex items-end gap-1.5 pb-1.5 text-sm">
							<input type="checkbox" checked={shown.anti_magic} disabled={locked} onchange={(e) => (form.boxes.anti_magic = e.currentTarget.checked)} data-testid="sim-anti-magic" />
							Anti-Magic
						</label>
					</div>
					<div class="grid grid-cols-5 gap-2">
						{#each resists as r (r.key)}
							<Field label={r.label} bind:value={() => String(shown[r.key]), (v) => (form.boxes[r.key] = v as never)} disabled={locked} testid="sim-{r.key.replace('resist_', 'res-')}" />
						{/each}
					</div>
					{#if !locked}
						<label class="flex flex-col gap-1">
							<span class={labelCls}>Class (hit minimum and dodge caps)</span>
							<select bind:value={form.classNumber} class={inputCls} data-testid="sim-class">
								<option value={null}>(none)</option>
								{#each classes as c (c.number)}<option value={c.number}>{c.name}</option>{/each}
							</select>
						</label>
						<button type="button" class="bg-background hover:bg-accent h-8 self-start rounded-md border px-3 text-sm" onclick={resetDefenses} data-testid="sim-reset">
							Reset defenses
						</button>
					{/if}
					<label class="flex items-center gap-2 text-sm" title="Dodge is rolled before armour class">
						<input type="checkbox" bind:checked={form.alwaysDodge} data-testid="sim-always-dodge" />
						Always dodge first
					</label>
				</div>
			</details>

			<div class="flex flex-col gap-3 border-t pt-3">
				<div class="grid grid-cols-2 gap-3">
					<Field label="Rounds (max {SIM_MAX_ROUNDS.toLocaleString('en-US')})" bind:value={form.rounds} disabled={form.dynamic} testid="sim-rounds" />
					<Field label="Log rounds" bind:value={form.logRounds} testid="sim-log-rounds" hint="MME keeps 100" />
				</div>
				<div class="flex items-end gap-2">
					<Field label="Seed (blank is {SIM_DEFAULT_SEED})" bind:value={form.seed} testid="sim-seed" hint="The same seed gives the same run; MME reseeds every time" />
					<button type="button" class="bg-background hover:bg-accent h-8 shrink-0 rounded-md border px-3 text-sm" onclick={newSeed} data-testid="sim-new-seed">New</button>
				</div>
				<div class="flex flex-wrap gap-x-4 gap-y-1 text-sm">
					<label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={form.dynamic} data-testid="sim-dynamic" />Dynamic rounds</label>
					<label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={form.logMaxRoundOnly} data-testid="sim-max-round-only" />Log worst round only</label>
					<label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={form.hideEnergy} data-testid="sim-hide-energy" />Hide energy</label>
				</div>
				<button
					type="button"
					class="bg-primary text-primary-foreground h-9 rounded-md px-4 text-sm font-medium disabled:opacity-50"
					disabled={form.monster === null || running}
					onclick={run}
					data-testid="sim-run"
				>
					{running ? 'Running...' : 'Run simulation'}
				</button>
			</div>
			{#if loadError}<p class="text-destructive text-sm" role="alert">{loadError}</p>{/if}
		</div>
	{/snippet}
	{#snippet results()}
		<h3 class={headingCls}>Results</h3>
		{#if error}
			<p class="text-destructive text-sm" role="alert" data-testid="sim-error">{error}</p>
		{:else if !result}
			<p class="text-muted-foreground text-sm">Pick a monster and a defender, then run the simulation.</p>
		{:else}
			<p class="mb-3 text-sm font-medium" data-testid="sim-monster-name">
				{result.monster.name} ({result.monster.number}), {result.rounds.toLocaleString('en-US')} rounds, seed {result.seed}
			</p>
			{#if result.error}
				<p class="text-destructive mb-3 text-sm" role="alert">{result.error.text}: the run stopped, partial results shown.</p>
			{/if}
			{#if result.summary}
				<div class="grid grid-cols-3 gap-3" data-testid="sim-summary">
					<div class={tile}>
						<div class={tileLabel}>Avg dmg / round</div>
						<div class="text-2xl font-semibold tabular-nums" data-testid="sim-avg">{result.summary.avg_damage}</div>
					</div>
					<div class={tile}>
						<div class={tileLabel}>Max / seen</div>
						<div class="text-2xl font-semibold tabular-nums" data-testid="sim-max">{result.summary.max_damage}/{result.summary.max_seen}</div>
					</div>
					<div class={tile}>
						<div class={tileLabel}>Physical / spell</div>
						<div class="text-2xl font-semibold tabular-nums" data-testid="sim-split">{result.summary.avg_phys}/{result.summary.avg_spell}</div>
					</div>
				</div>
				<div class="mt-5 overflow-x-auto">
					<table class="w-full text-sm" data-testid="sim-table">
						<thead>
							<tr class="border-b">
								<th class="{th} text-left">Attack</th>
								<th class={th}>Cast %</th>
								<th class={th}>Avg / hit</th>
								<th class={th}>Success %</th>
								<th class={th}>Resisted %</th>
								<th class={th}>Dodged / resisted %</th>
							</tr>
						</thead>
						<tbody>
							{#each result.attacks as a (a.index)}
								<tr class="border-b last:border-0" data-testid="sim-row-{a.index}">
									<td class="px-2 py-1">{a.index + 1}. {a.name}</td>
									{#if a.stats}
										<td class={td}>{pctText(a.stats.true_cast_pct)}</td>
										<td class={td}>{a.stats.avg_per_hit}</td>
										<td class={td}>{pctText(a.stats.success_pct)}</td>
										<td class={td}>{pctText(a.stats.dmg_resisted_pct)}</td>
										<td class={td}>{pctText(a.stats.resist_dodge_pct)}</td>
									{:else}
										<td class="text-muted-foreground px-2 py-1 text-right" colspan="5">-</td>
									{/if}
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else if result.ran}
				<p class="text-muted-foreground text-sm">This monster made no attacks against that defender.</p>
			{:else}
				<p class="text-muted-foreground text-sm">Rounds is 0: nothing was run.</p>
			{/if}
			{#if result.combat_log}
				<h3 class="{headingCls} mt-6">Combat log</h3>
				<!-- svelte-ignore a11y_no_noninteractive_tabindex (a scrollable region must take focus to scroll by keyboard) -->
				<div
					class="bg-background max-h-80 overflow-auto rounded-md border p-3"
					role="region"
					aria-label="Combat log"
					tabindex="0"
				>
					<pre class="font-mono text-xs leading-relaxed" data-testid="sim-log">{result.combat_log}</pre>
				</div>
			{/if}
		{/if}
	{/snippet}
</CalcFrame>
