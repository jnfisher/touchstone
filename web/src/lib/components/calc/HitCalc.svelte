<script lang="ts">
	// frmHitCalc: the chance to hit and be dodged for a normal attack or a backstab, between the
	// current character, a monster, a remembered player or typed numbers.
	import { untrack } from 'svelte';
	import { api, type ClassRec } from '$lib/api';
	import { calcPrefill } from '$lib/handoff.svelte';
	import {
		applyHit,
		hitFromCharacter,
		calcClient,
		captionLines,
		HIT_BOXES,
		HIT_DEFAULTS,
		HIT_STRIKES,
		hitRequest,
		monsterChoices,
		weaponChoices,
		type HitAttacker,
		type HitDefender,
		type HitEvent,
		type HitForm,
		type HitResult,
		type HitType,
		type MonsterChoice,
		type WeaponChoice
	} from '$lib/calc';
	import CalcFrame from './CalcFrame.svelte';
	import { useCurrentCharacter } from './current.svelte';
	import UseCharacter from './UseCharacter.svelte';
	import Field from './Field.svelte';
	import { headingCls, inputCls, labelCls } from './styles';
	import WeaponPicker from './WeaponPicker.svelte';

	let form = $state<HitForm>(structuredClone(HIT_DEFAULTS));
	let result = $state.raw<HitResult>();
	let error = $state<string>();
	let monsters = $state.raw<MonsterChoice[]>([]);
	let bsWeapons = $state.raw<WeaponChoice[]>([]);
	let classes = $state.raw<ClassRec[]>([]);
	let loadError = $state<string>();

	// Every event goes through here: only the newest request's answer is used, and a box edit
	// never replaces an option click or character edit that is still waiting.
	let ticket = 0;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let pending: HitEvent | undefined;

	function send(ev: HitEvent, delay = 0) {
		if (ev.event === 'calc' && pending && pending.event !== 'calc') ev = pending;
		pending = ev;
		const mine = ++ticket;
		clearTimeout(timer);
		timer = setTimeout(() => {
			pending = undefined;
			const sent = $state.snapshot(form) as HitForm;
			calcClient
				.hit(hitRequest(sent, ev))
				.then((r) => {
					if (mine !== ticket) return;
					form = applyHit($state.snapshot(form) as HitForm, r, ev);
					result = r;
					error = undefined;
				})
				.catch((e: unknown) => {
					if (mine === ticket) error = e instanceof Error ? e.message : String(e);
				});
		}, delay);
	}
	$effect(() => () => clearTimeout(timer));

	$effect(() => {
		let alive = true;
		Promise.all([api.table('monsters'), api.table('items'), api.table('classes')])
			.then(([m, items, c]) => {
				if (!alive) return;
				monsters = monsterChoices(m);
				bsWeapons = weaponChoices(items, { backstabOnly: true });
				classes = c.filter((x) => x.name !== '');
				// Form_Load: the first monster is selected, then the window is filled in.
				form.monster = monsters[0]?.number ?? null;
				// a monster menu's "Hit Calc vs Mob": the monster, as attacker or as defender
				const want = calcPrefill.take('hit');
				if (want?.monster && monsters.some((m) => m.number === want.monster)) {
					form.monster = want.monster;
					if (want.side === 'attacker') form.attacker = 'monster';
					else form.defender = 'monster';
				}
				if (!current.active) form.character.classNumber = classes[0]?.number ?? null;
				send({ event: 'refresh' });
			})
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});

	// SetHitCalcVals reads the character from the main form: with the switch on its numbers fill the
	// section below and the window refreshes, as it does when the character changes in MME.
	const current = useCurrentCharacter();
	const locked = $derived(current.active);
	let appliedKey = '';
	$effect(() => {
		const c = current.active ? current.character : undefined;
		if (!c) {
			appliedKey = '';
			return;
		}
		const key = JSON.stringify(c);
		if (key === appliedKey) return;
		appliedKey = key;
		untrack(() => {
			form.character = hitFromCharacter(c, $state.snapshot(form.character) as HitForm['character']);
			if (form.enabled !== null) send({ event: 'refresh' });
		});
	});

	const box = (ev: HitEvent) => () => send(ev, 120);
	const typed = box({ event: 'calc' });
	const charEdited = box({ event: 'refresh' });

	function setType(t: HitType) {
		form.calcType = t;
		// optHitCalcType_Click: a backstab has no monster attacker.
		if (t === 'backstab' && form.attacker === 'monster') form.attacker = 'manual';
		send({ event: 'refresh' });
	}
	function setAttacker(a: HitAttacker) {
		form.attacker = a;
		send({ event: 'refresh' });
	}
	function setDefender(d: HitDefender) {
		form.defender = d;
		send({ event: 'refresh' });
	}

	const en = $derived(form.enabled);
	const monsterOff = $derived(en !== null && !en.monster_list);
	const attackerOptions: { value: HitAttacker; label: string }[] = [
		{ value: 'character', label: 'Current Char.' },
		{ value: 'monster', label: 'Monster' },
		{ value: 'manual', label: 'Manual' }
	];
	const defenderOptions: { value: HitDefender; label: string }[] = [
		{ value: 'character', label: 'vs Char.' },
		{ value: 'monster', label: 'vs Mob' },
		{ value: 'player', label: 'vs Player' },
		{ value: 'manual', label: 'Manual' }
	];
	const typeOptions: { value: HitType; label: string }[] = [
		{ value: 'normal', label: 'Normal' },
		{ value: 'backstab', label: 'Backstab' }
	];

	/** Whether a box takes part in the calculation, as SetHitCalcVals enables them. */
	function boxEnabled(i: number): boolean {
		if (en === null) return true;
		if (i === 3) return en.prot_evil;
		if (i === 5) return en.percep;
		return true;
	}
	function boxLabel(i: number, base: string): string {
		return i === 5 && result?.bs_label ? result.bs_label : base;
	}

	const tile = 'bg-background rounded-md border p-3';
	const tileLabel = 'text-muted-foreground text-[11px] font-medium uppercase tracking-wide';
	const arrowCls =
		'bg-background hover:bg-accent h-8 w-7 shrink-0 rounded-md border text-sm leading-none select-none disabled:opacity-40';
</script>

{#snippet radios<T extends string>(
	name: string,
	options: { value: T; label: string }[],
	current: T,
	pick: (v: T) => void,
	disabled: (v: T) => boolean = () => false
)}
	<div class="flex flex-wrap gap-x-4 gap-y-1 text-sm">
		{#each options as o (o.value)}
			<label class="flex items-center gap-1.5 {disabled(o.value) ? 'opacity-50' : ''}">
				<input
					type="radio"
					{name}
					checked={current === o.value}
					disabled={disabled(o.value)}
					onchange={() => pick(o.value)}
					data-testid="hit-{name}-{o.value}"
				/>
				<span class={current === o.value ? 'font-semibold' : ''}>{o.label}</span>
			</label>
		{/each}
	</div>
{/snippet}

<CalcFrame title="Hit Calculator" testid="calc-hit">
	{#snippet inputs()}
		<div class="flex flex-col gap-4">
			<UseCharacter {current} />
			<WeaponPicker
				choices={monsters}
				bind:value={form.monster}
				label="Monster"
				noun="monster"
				testid="hit-monster"
				onchoose={() => send({ event: 'monster' })}
			/>
			<fieldset class="flex flex-col gap-1.5">
				<legend class={labelCls}>Calculate</legend>
				{@render radios('type', typeOptions, form.calcType, setType)}
			</fieldset>
			<fieldset class="flex flex-col gap-1.5">
				<legend class={labelCls}>Attacker</legend>
				{@render radios(
					'attacker',
					attackerOptions,
					form.attacker,
					setAttacker,
					(v) => v === 'monster' && form.calcType === 'backstab'
				)}
			</fieldset>
			<fieldset class="flex flex-col gap-1.5">
				<legend class={labelCls}>Defender</legend>
				{@render radios('defender', defenderOptions, form.defender, setDefender)}
			</fieldset>

			<div class="flex flex-col gap-2">
				{#each HIT_BOXES as b (b.index)}
					<div class="flex flex-col gap-1" class:opacity-50={!boxEnabled(b.index)}>
						<span class={labelCls}>{boxLabel(b.index, b.label)}</span>
						<div class="flex items-center gap-1">
							<button
								type="button"
								class={arrowCls}
								aria-label="Decrease {b.label}"
								disabled={!boxEnabled(b.index)}
								onclick={() => send({ event: 'modify', index: b.index * 2 })}
								data-testid="hit-box-{b.index}-minus">-</button
							>
							<input
								type="text"
								inputmode="decimal"
								autocomplete="off"
								bind:value={form.text[b.index]}
								oninput={typed}
								disabled={!boxEnabled(b.index)}
								data-testid="hit-box-{b.index}"
								class="{inputCls} disabled:opacity-60"
							/>
							<button
								type="button"
								class={arrowCls}
								aria-label="Increase {b.label}"
								disabled={!boxEnabled(b.index)}
								onclick={() => send({ event: 'modify', index: b.index * 2 + 1 })}
								data-testid="hit-box-{b.index}-plus">+</button
							>
						</div>
					</div>
				{/each}
			</div>
			<div class="flex gap-5 text-sm">
				<label class="flex items-center gap-1.5 {en && !en.shadow ? 'opacity-50' : ''}">
					<input
						type="checkbox"
						bind:checked={form.shadow}
						disabled={en !== null && !en.shadow}
						onchange={typed}
						data-testid="hit-shadow"
					/>
					Shadow
				</label>
				<label class="flex items-center gap-1.5 {en && !en.see_hidden ? 'opacity-50' : ''}">
					<input
						type="checkbox"
						bind:checked={form.seeHidden}
						disabled={en !== null && !en.see_hidden}
						onchange={typed}
						data-testid="hit-see-hidden"
					/>
					See Hidden
				</label>
			</div>

			<details open class="border-t pt-3" data-testid="hit-character">
				<summary class="{labelCls} cursor-pointer">Current character</summary>
				<div class="mt-3 flex flex-col gap-3">
					<div class="grid grid-cols-3 gap-3">
						<Field label="Accuracy" bind:value={form.character.accuracy} oninput={charEdited} disabled={locked} testid="hit-char-accuracy" />
						<Field label="AC" bind:value={form.character.ac} oninput={charEdited} disabled={locked} testid="hit-char-ac" />
						<Field label="Dodge" bind:value={form.character.dodge} oninput={charEdited} disabled={locked} testid="hit-char-dodge" />
						<Field label="Prot. Evil" bind:value={form.character.protEvil} oninput={charEdited} disabled={locked} testid="hit-char-prot-evil" />
						<Field label="Stealth" bind:value={form.character.stealth} oninput={charEdited} disabled={locked} testid="hit-char-stealth" />
						<Field label="BS Accuracy" bind:value={form.character.bsAccuracy} oninput={charEdited} disabled={locked} testid="hit-char-bs-accuracy" />
						<Field label="Agility" bind:value={form.character.agility} oninput={charEdited} disabled={locked} testid="hit-char-agility" />
						<Field label="Shadow AC" bind:value={form.character.shadowAc} oninput={charEdited} disabled={locked} testid="hit-char-shadow-ac" hint="The AC a shadow form adds; taken off AC vs Char." />
					</div>
					<label class="flex flex-col gap-1">
						<span class={labelCls}>Class</span>
						<select
							bind:value={form.character.classNumber}
							onchange={charEdited}
							disabled={locked}
							data-testid="hit-char-class"
							class="{inputCls} disabled:opacity-60"
						>
							{#each classes as c (c.number)}<option value={c.number}>{c.name}</option>{/each}
						</select>
					</label>
					<div class="grid grid-cols-[1fr_5rem] gap-3">
						<label class="flex flex-col gap-1">
							<span class={labelCls}>Attack</span>
							<select
								bind:value={form.character.strike}
								onchange={charEdited}
								disabled={locked}
								data-testid="hit-char-strike"
								class="{inputCls} disabled:opacity-60"
							>
								{#each HIT_STRIKES as s (s.value)}<option value={s.value}>{s.label}</option>{/each}
							</select>
						</label>
						<Field label="Accy adj." bind:value={form.character.accyAdj} oninput={charEdited} disabled={locked} testid="hit-char-accy-adj" />
					</div>
					<WeaponPicker
						choices={bsWeapons}
						bind:value={form.character.bsWeapon}
						label="Backstab weapon"
						testid="hit-bs-weapon"
						clearable
						disabled={locked}
						onchoose={charEdited}
					/>
				</div>
			</details>
			{#if loadError}<p class="text-destructive text-sm" role="alert">{loadError}</p>{/if}
		</div>
	{/snippet}
	{#snippet results()}
		<h3 class={headingCls}>Chance to hit</h3>
		{#if error}
			<p class="text-destructive text-sm" role="alert">{error}</p>
		{:else if result}
			{#if result.errors.length}
				<p class="text-destructive mb-3 text-sm" role="alert" data-testid="hit-errors">
					{result.errors.map((e) => `${e.text} in ${e.source}`).join('; ')}
				</p>
			{/if}
			{#if result.hit !== null}
				<div class="grid grid-cols-3 gap-3" data-testid="hit-stats">
					<div class={tile}>
						<div class={tileLabel}>Hit</div>
						<div class="text-2xl font-semibold tabular-nums" data-testid="hit-pct">{result.hit}%</div>
					</div>
					<div class={tile}>
						<div class={tileLabel}>Dodge</div>
						<div class="text-2xl font-semibold tabular-nums" data-testid="hit-dodge">{result.dodge}%</div>
					</div>
					<div class={tile}>
						<div class={tileLabel}>Overall Hit</div>
						<div class="text-2xl font-semibold tabular-nums" data-testid="hit-overall">{result.overall_hit}%</div>
					</div>
				</div>
			{/if}
			<div class="mt-5 grid gap-5 sm:grid-cols-2">
				<div class="text-muted-foreground text-sm" data-testid="hit-left">
					{#each captionLines(result.sub_left) as line, i (i)}<div>{line}</div>{/each}
				</div>
				<div class="text-sm tabular-nums {result.sub_right_bold ? 'font-semibold' : ''}" data-testid="hit-right">
					{#each captionLines(result.sub_right) as line, i (i)}<div class={line.endsWith(':') ? 'text-muted-foreground' : ''}>{line}</div>{/each}
				</div>
			</div>
			<p class="text-muted-foreground mt-6 max-w-prose text-xs">
				Each box and option works as in MME: picking Current Char., a monster or a player fills the boxes
				in; Manual leaves them for you. A starred minimum means the defender's class changes it. The
				character's numbers come from the Character tab while "Use current character" is on, and are typed in otherwise. Vile ward and
				the evilness list are GreaterMUD-only and not shown.
			</p>
		{:else}
			<p class="text-muted-foreground text-sm">Loading.</p>
		{/if}
	{/snippet}
</CalcFrame>
