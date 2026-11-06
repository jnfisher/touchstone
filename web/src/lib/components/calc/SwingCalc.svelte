<script lang="ts">
	// frmSwingCalc: energy per swing, swings in each of ten rounds and the average round damage.
	import { untrack } from 'svelte';
	import { api } from '$lib/api';
	import { calcPrefill } from '$lib/handoff.svelte';
	import {
		afterLabel,
		calcClient,
		COMBAT_LEVELS,
		megaFromFields,
		swingFromCharacter,
		SPEED_OPTIONS,
		SWING_DEFAULTS,
		swingRequest,
		weaponChoices,
		withSpeedOption,
		type SwingForm,
		type WeaponChoice
	} from '$lib/calc';
	import CalcFrame from './CalcFrame.svelte';
	import { useCurrentCharacter } from './current.svelte';
	import UseCharacter from './UseCharacter.svelte';
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
				// a weapon menu's "Calculate Swings": GotoWeapon, then the bashing box
				const want = calcPrefill.take('swing');
				if (want?.weapon && choices.some((c) => c.number === want.weapon)) {
					form.weapon = want.weapon;
					form.bashing = !!want.bashing;
				}
			})
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});

	// Form_Load: the character's numbers go into the boxes; switched off, they stay as they were.
	const current = useCurrentCharacter();
	$effect(() => {
		const c = current.active ? current.character : undefined;
		if (!c) return;
		const next = untrack(() => swingFromCharacter(c, $state.snapshot(form) as SwingForm));
		Object.assign(form, next);
	});
	const locked = $derived(current.active);

	let pasteText = $state('');
	let pasteMessage = $state<string>();
	let pasteOk = $state(false);

	// cmdPasteMega_Click: the server parses the text; a rejected paste changes nothing.
	async function paste(text: string) {
		try {
			const r = await calcClient.megaStat(text);
			pasteOk = r.ok;
			pasteMessage = r.ok ? 'Filled in from the MegaMUD lines.' : (r.message ?? undefined);
			if (r.ok && r.fields) form.mega = megaFromFields(r.fields);
		} catch (e) {
			pasteOk = false;
			pasteMessage = e instanceof Error ? e.message : String(e);
		}
	}
	async function pasteClipboard() {
		try {
			pasteText = await navigator.clipboard.readText();
		} catch {
			// no clipboard permission: the box below takes a normal paste
		}
		if (pasteText.trim() !== '') await paste(pasteText);
	}

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
			<UseCharacter {current} />
			<WeaponPicker {choices} bind:value={form.weapon} disabled={locked} />
			<label class="flex flex-col gap-1">
				<span class={labelCls}>Combat</span>
				<select bind:value={form.combat} disabled={locked} data-testid="swing-combat" class="{inputCls} disabled:opacity-60">
					{#each COMBAT_LEVELS as c (c.value)}<option value={c.value}>{c.label}</option>{/each}
				</select>
			</label>
			<div class="grid grid-cols-3 gap-3">
				<Field label="Level" bind:value={form.level} disabled={locked} testid="swing-level" />
				<Field label="Strength" bind:value={form.strength} disabled={locked} testid="swing-strength" />
				<Field label="Agility" bind:value={form.agility} disabled={locked} testid="swing-agility" />
			</div>
			<div class="grid grid-cols-2 gap-3">
				<Field label="Encum." bind:value={form.encum} disabled={locked} testid="swing-encum" />
				<div class="flex flex-col gap-1">
					<span class="{labelCls} flex h-4 items-center justify-between">
						<span>Max Enc.</span>
						<label class="flex items-center gap-1 font-normal" title="Follow strength, as MME does">
							<input type="checkbox" bind:checked={form.maxEncumAuto} disabled={locked} data-testid="swing-max-auto" />
							auto
						</label>
					</span>
					<input
						type="text"
						inputmode="decimal"
						autocomplete="off"
						value={maxEncumShown}
						disabled={form.maxEncumAuto || locked}
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
			<fieldset class="flex flex-col gap-2 border-t pt-3" data-testid="swing-mega">
				<legend class={labelCls}>MegaMUD stat line</legend>
				<textarea
					rows="3"
					placeholder="Paste the Hit:, Extra: and Crit: lines from MegaMUD"
					bind:value={pasteText}
					onpaste={(e) => {
						const t = e.clipboardData?.getData('text');
						if (t) void paste(t);
					}}
					data-testid="swing-mega-text"
					class="{inputCls} h-auto py-1.5 font-mono text-xs"
				></textarea>
				<div class="flex items-center gap-2">
					<button
						type="button"
						class="bg-secondary text-secondary-foreground h-8 rounded-md px-3 text-sm"
						onclick={() => void paste(pasteText)}
						data-testid="swing-mega-apply">Paste MegaMUD stat line</button
					>
					<button
						type="button"
						class="border-input h-8 rounded-md border px-3 text-sm"
						onclick={() => void pasteClipboard()}
						data-testid="swing-mega-clipboard">From clipboard</button
					>
				</div>
				{#if pasteMessage}
					<p
						class="text-sm {pasteOk ? 'text-muted-foreground' : 'text-destructive'}"
						role={pasteOk ? 'status' : 'alert'}
						data-testid="swing-mega-message"
					>
						{pasteMessage}
					</p>
				{/if}
				<div class="grid grid-cols-3 gap-3">
					<Field label="Hit %" bind:value={form.mega.hitPct} testid="swing-hit-pct" />
					<Field label="Extra %" bind:value={form.mega.extraPct} testid="swing-extra-pct" />
					<Field label="Crit %" bind:value={form.mega.critPct} testid="swing-crit-pct" />
					<Field label="Hit avg" bind:value={form.mega.hitAvg} testid="swing-hit-avg" />
					<Field label="Extra avg" bind:value={form.mega.extraAvg} testid="swing-extra-avg" />
					<Field label="Crit avg" bind:value={form.mega.critAvg} testid="swing-crit-avg" />
				</div>
			</fieldset>
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
					<div>
						<h3 class={headingCls}>Average per round from MegaMUD</h3>
						{#if result.true_average_error}
							<p class="text-destructive text-sm" role="alert">{result.true_average_error}</p>
						{:else}
							<div class="text-2xl font-semibold tabular-nums" data-testid="swing-true-average">
								{result.true_average ?? '-'}
							</div>
							<div class="text-muted-foreground text-xs">
								True average: swings x (Hit, Crit and Extra from the MegaMUD boxes)
							</div>
						{/if}
					</div>
				</div>
			</div>
		{:else}
			<p class="text-muted-foreground text-sm">Pick a weapon.</p>
		{/if}
	{/snippet}
</CalcFrame>
