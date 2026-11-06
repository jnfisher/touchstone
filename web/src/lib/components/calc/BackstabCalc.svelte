<script lang="ts">
	// frmBSCalc: the damage range of a backstab with a weapon that has BS accuracy.
	import { api } from '$lib/api';
	import {
		BACKSTAB_DEFAULTS,
		backstabRequest,
		calcClient,
		weaponChoices,
		type BackstabForm,
		type WeaponChoice
	} from '$lib/calc';
	import CalcFrame from './CalcFrame.svelte';
	import Field from './Field.svelte';
	import { live } from './live.svelte';
	import { headingCls } from './styles';
	import WeaponPicker from './WeaponPicker.svelte';

	let choices = $state.raw<WeaponChoice[]>([]);
	let form = $state<BackstabForm>({ ...BACKSTAB_DEFAULTS });
	let loadError = $state<string>();

	$effect(() => {
		let alive = true;
		api
			.table('items')
			.then((items) => {
				if (!alive) return;
				choices = weaponChoices(items, { backstabOnly: true });
				if (form.weapon === null) form.weapon = choices[0]?.number ?? null;
			})
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});

	const calc = live(
		() => (form.weapon === null ? undefined : backstabRequest(form)),
		calcClient.backstab
	);
	const result = $derived(calc.result);
	const noBs = $derived(result?.damage === 'No BS');

	// MME limits these boxes to 0..1000 and writes the limit back; say so rather than rewrite the
	// box under the cursor.
	const clamped = $derived(
		result
			? ([
					['BS Min DMG', form.bsMin, result.bs_min],
					['BS Max DMG', form.bsMax, result.bs_max],
					['Level', form.level, result.level],
					['Stealth', form.stealth, result.stealth],
					['Max Damage', form.maxDmg, result.max_dmg_box]
				] as const).filter(([, typed, used]) => typed.trim() !== used && Number.isFinite(Number(typed)))
			: []
	);
</script>

<CalcFrame title="Backstab Calculator" testid="calc-backstab">
	{#snippet inputs()}
		<div class="flex flex-col gap-3">
			<WeaponPicker {choices} bind:value={form.weapon} />
			<div class="grid grid-cols-3 gap-3">
				<Field label="Level" bind:value={form.level} testid="bs-level" />
				<Field label="Strength" bind:value={form.strength} testid="bs-strength" />
				<Field
					label="Stealth"
					bind:value={form.stealth}
					note={result?.stealth_adj}
					testid="bs-stealth"
				/>
			</div>
			<div class="grid grid-cols-3 gap-3">
				<Field label="BS Min DMG" bind:value={form.bsMin} note={result?.bs_min_adj} testid="bs-min" />
				<Field label="BS Max DMG" bind:value={form.bsMax} note={result?.bs_max_adj} testid="bs-max" />
				<Field
					label="Max Damage"
					bind:value={form.maxDmg}
					note={result?.max_adj}
					testid="bs-max-dmg"
					hint="All max damage bonus, strength's included"
				/>
			</div>
			<label class="flex items-center gap-1.5 text-sm">
				<input type="checkbox" bind:checked={form.classStealth} data-testid="bs-class-stealth" />
				Class Stealth
			</label>
			{#if loadError}<p class="text-destructive text-sm" role="alert">{loadError}</p>{/if}
		</div>
	{/snippet}
	{#snippet results()}
		<h3 class={headingCls}>Backstab damage</h3>
		{#if calc.error}
			<p class="text-destructive text-sm" role="alert">{calc.error}</p>
		{:else if result}
			{#if result.error_text}
				<p class="text-destructive mb-2 text-sm" role="alert">{result.error_text}</p>
			{/if}
			<div class="text-3xl font-semibold tabular-nums" data-testid="bs-damage">{result.damage}</div>
			{#if noBs}
				<p class="text-muted-foreground mt-1 text-sm">This weapon has no BS accuracy, so it cannot backstab.</p>
			{/if}
			{#if clamped.length}
				<p class="text-muted-foreground mt-3 text-xs" data-testid="bs-clamped">
					MME limits each box to 0..1000:
					{clamped.map(([name, , used]) => `${name} counts as ${used}`).join(', ')}.
				</p>
			{/if}
			<p class="text-muted-foreground mt-6 max-w-prose text-xs">
				Max damage bonus from strength is not added for you: enter all of it, strength's included, in
				Max Damage. Minimum damage from strength is added. The numbers beside Stealth, BS Min DMG, BS
				Max DMG and Max Damage are what the chosen weapon adds. Level and stats are typed in here until
				the Character tab can supply them.
			</p>
		{:else}
			<p class="text-muted-foreground text-sm">Pick a weapon.</p>
		{/if}
	{/snippet}
</CalcFrame>
