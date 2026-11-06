<script lang="ts">
	// frmSwingCalc: energy per swing, swings in each of ten rounds and the average round damage.
	import { api } from '$lib/api';
	import {
		afterLabel,
		calcClient,
		COMBAT_LEVELS,
		SPEED_OPTIONS,
		SWING_DEFAULTS,
		swingRequest,
		weaponChoices,
		withSpeedOption,
		type SwingForm,
		type WeaponChoice
	} from '$lib/calc';
	import CalcFrame from './CalcFrame.svelte';
	import Field from './Field.svelte';
	import { live } from './live.svelte';
	import { headingCls, inputCls, labelCls } from './styles';
	import WeaponPicker from './WeaponPicker.svelte';

	let choices = $state.raw<WeaponChoice[]>([]);
	let form = $state<SwingForm>({ ...SWING_DEFAULTS });
	let loadError = $state<string>();

	$effect(() => {
		let alive = true;
		api
			.table('items')
			.then((items) => {
				if (!alive) return;
				choices = weaponChoices(items);
				// frmSwingCalc selects the first weapon in the list on load.
				if (form.weapon === null) form.weapon = choices[0]?.number ?? null;
			})
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});

	// No weapon yet: nothing to ask the server.
	const calc = live(
		() => (form.weapon === null ? undefined : swingRequest(form)),
		calcClient.swing
	);
	const result = $derived(calc.result);

	// Max Enc. mirrors what the server worked out from strength while it follows strength.
	const maxEncumShown = $derived(form.maxEncumAuto ? (result?.max_encum ?? form.maxEncum) : form.maxEncum);

	const tile = 'bg-muted/50 rounded-md border px-3 py-2';
	const tileLabel = 'text-muted-foreground text-[11px] font-medium';
</script>

<CalcFrame title="Swing Calculator" testid="calc-swing">
	{#snippet inputs()}
		<div class="flex flex-col gap-3">
			<WeaponPicker {choices} bind:value={form.weapon} />
			<label class="flex flex-col gap-1">
				<span class={labelCls}>Combat</span>
				<select bind:value={form.combat} data-testid="swing-combat" class={inputCls}>
					{#each COMBAT_LEVELS as c (c.value)}<option value={c.value}>{c.label}</option>{/each}
				</select>
			</label>
			<div class="grid grid-cols-3 gap-3">
				<Field label="Level" bind:value={form.level} testid="swing-level" />
				<Field label="Strength" bind:value={form.strength} testid="swing-strength" />
				<Field label="Agility" bind:value={form.agility} testid="swing-agility" />
			</div>
			<div class="grid grid-cols-2 gap-3">
				<Field label="Encum." bind:value={form.encum} testid="swing-encum" />
				<div class="flex flex-col gap-1">
					<span class="{labelCls} flex h-4 items-center justify-between">
						<span>Max Enc.</span>
						<label class="flex items-center gap-1 font-normal" title="Follow strength, as MME does">
							<input type="checkbox" bind:checked={form.maxEncumAuto} data-testid="swing-max-auto" />
							auto
						</label>
					</span>
					<input
						type="text"
						inputmode="decimal"
						autocomplete="off"
						value={maxEncumShown}
						disabled={form.maxEncumAuto}
						oninput={(e) => (form.maxEncum = e.currentTarget.value)}
						data-testid="swing-max-encum"
						class="{inputCls} disabled:opacity-60"
					/>
				</div>
			</div>
			<fieldset class="flex flex-col gap-1.5">
				<legend class={labelCls}>Speed</legend>
				<div class="flex flex-wrap gap-x-4 gap-y-1 text-sm">
					{#each SPEED_OPTIONS as o (o.value)}
						<label class="flex items-center gap-1.5">
							<input
								type="radio"
								name="swing-speed"
								checked={form.speedOption === o.value}
								onchange={() => (form = withSpeedOption(form, o.value))}
								data-testid="swing-speed-{o.value}"
							/>
							{o.label}
						</label>
					{/each}
				</div>
				<Field
					label="Custom speed"
					bind:value={form.speed}
					disabled={form.speedOption !== 3}
					testid="swing-speed-custom"
				/>
			</fieldset>
			<div class="flex gap-5 text-sm">
				<label class="flex items-center gap-1.5">
					<input type="checkbox" bind:checked={form.slowness} data-testid="swing-slowness" />
					Slowness Ability
				</label>
				<label class="flex items-center gap-1.5">
					<input type="checkbox" bind:checked={form.bashing} data-testid="swing-bashing" />
					Bashing
				</label>
			</div>
			{#if loadError}<p class="text-destructive text-sm" role="alert">{loadError}</p>{/if}
		</div>
	{/snippet}
	{#snippet results()}
		{#if calc.error}
			<p class="text-destructive text-sm" role="alert">{calc.error}</p>
		{:else if result}
			{#if result.error_text}
				<p class="text-destructive mb-3 text-sm" role="alert">
					{result.error_text}: showing what was worked out before it.
				</p>
			{/if}
			<div class="grid grid-cols-2 gap-3 sm:grid-cols-4" data-testid="swing-stats">
				<div class={tile}>
					<div class={tileLabel}>Energy per swing</div>
					<div class="text-lg font-semibold tabular-nums" data-testid="swing-energy">
						{afterLabel(result.energy)}
					</div>
				</div>
				<div class={tile}>
					<div class={tileLabel}>Raw swing</div>
					<div class="text-lg font-semibold tabular-nums" data-testid="swing-raw">
						{afterLabel(result.raw_swing)}
					</div>
				</div>
				<div class={tile}>
					<div class={tileLabel}>QnD crits</div>
					<div class="text-lg font-semibold tabular-nums" data-testid="swing-qnd">
						{afterLabel(result.qnd)}
					</div>
				</div>
				<div class={tile}>
					<div class={tileLabel}>Encum %</div>
					<div class="text-lg font-semibold tabular-nums" data-testid="swing-encum-pct">
						{afterLabel(result.encum)}
					</div>
					<div class="text-muted-foreground text-[11px]">Must be &lt; 67% for QnD</div>
				</div>
			</div>

			<div class="mt-4 grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
				<div>
					<h3 class={headingCls}>Swings / EU remaining</h3>
					<div class="overflow-hidden rounded-md border">
						<table class="w-full" data-testid="swing-rounds">
							<thead>
								<tr class="text-muted-foreground bg-muted/50 text-[11px]">
									<th class="px-3 py-1 text-left font-medium">Round</th>
									<th class="px-3 py-1 text-right font-medium">Swings</th>
									<th class="px-3 py-1 text-right font-medium">EU</th>
								</tr>
							</thead>
							<tbody>
								{#each result.swings as s, i (i)}
									<tr class="border-t">
										<td class="px-3 py-0.5 tabular-nums">{i + 1}</td>
										<td class="px-3 py-0.5 text-right tabular-nums">{s}</td>
										<td class="text-muted-foreground px-3 py-0.5 text-right tabular-nums">
											{result.eu[i]}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</div>
				<div class="flex flex-col gap-3">
					<div>
						<h3 class={headingCls}>True average</h3>
						<div class="text-2xl font-semibold tabular-nums" data-testid="swing-true-avg">
							{result.true_avg}
						</div>
						<div class="text-muted-foreground text-xs">swings per round</div>
					</div>
					<div>
						<h3 class={headingCls}>Average round</h3>
						<div class="text-2xl font-semibold tabular-nums" data-testid="swing-damage">
							{result.damage || '-'}
						</div>
						<div class="text-muted-foreground text-xs whitespace-pre-line">
							{result.damage_caption.replace(/\r\n/g, '\n')}
						</div>
					</div>
				</div>
			</div>
		{:else}
			<p class="text-muted-foreground text-sm">Pick a weapon.</p>
		{/if}
	{/snippet}
</CalcFrame>
