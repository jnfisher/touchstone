<script lang="ts">
	// frmExpCalc: the exp table number for a class and race, and the exp needed at each level.
	import { api, type ClassRec, type Race } from '$lib/api';
	import { calcClient, EXP_DEFAULTS, expRequest, type ExpForm } from '$lib/calc';
	import CalcFrame from './CalcFrame.svelte';
	import Field from './Field.svelte';
	import { live } from './live.svelte';
	import { headingCls, inputCls, labelCls } from './styles';

	let classes = $state.raw<ClassRec[]>([]);
	let races = $state.raw<Race[]>([]);
	let form = $state<ExpForm>({ ...EXP_DEFAULTS });
	let loadError = $state<string>();

	$effect(() => {
		let alive = true;
		Promise.all([api.table('classes'), api.table('races')])
			.then(([c, r]) => {
				if (!alive) return;
				classes = c.filter((x) => x.name !== '');
				races = r.filter((x) => x.name !== '');
			})
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});

	const calc = live(() => expRequest(form), calcClient.exp);
	const result = $derived(calc.result);

	// The table box shows what class and race give until it is typed over; picking a class or race
	// hands it back (cmbClass_Click / cmbRace_Click rewrite it).
	const tableText = $derived(form.table ?? (result?.table === null ? '' : String(result?.table ?? '')));
	const th = 'text-muted-foreground bg-card sticky top-0 px-3 py-1 text-[11px] font-medium';
</script>

<CalcFrame title="Exp Calculator" testid="calc-exp">
	{#snippet inputs()}
		<div class="flex flex-col gap-3">
			<label class="flex flex-col gap-1">
				<span class={labelCls}>Class</span>
				<select
					bind:value={form.classNumber}
					onchange={() => (form.table = null)}
					data-testid="exp-class"
					class={inputCls}
				>
					<option value={0}>custom</option>
					{#each classes as c (c.number)}<option value={c.number}>{c.name}</option>{/each}
				</select>
			</label>
			<label class="flex flex-col gap-1">
				<span class={labelCls}>Race</span>
				<select
					bind:value={form.raceNumber}
					onchange={() => (form.table = null)}
					data-testid="exp-race"
					class={inputCls}
				>
					<option value={0}>custom</option>
					{#each races as r (r.number)}<option value={r.number}>{r.name}</option>{/each}
				</select>
			</label>
			<label
				class="flex flex-col gap-1"
				title="Class table + 100 + race table; type over it for a custom table"
			>
				<span class={labelCls}>Exp Table %</span>
				<input
					type="text"
					inputmode="numeric"
					autocomplete="off"
					value={tableText}
					oninput={(e) => (form.table = e.currentTarget.value)}
					data-testid="exp-table-number"
					class={inputCls}
				/>
			</label>
			<div class="grid grid-cols-2 gap-3">
				<Field label="Level range: from" bind:value={form.from} testid="exp-from" />
				<Field label="to" bind:value={form.to} testid="exp-to" />
			</div>
			{#if loadError}<p class="text-destructive text-sm" role="alert">{loadError}</p>{/if}
		</div>
	{/snippet}
	{#snippet results()}
		<h3 class={headingCls}>Experience by level</h3>
		{#if calc.error}
			<p class="text-destructive text-sm" role="alert">{calc.error}</p>
		{:else if result}
			{#if result.error_text}
				<p class="text-destructive mb-2 text-sm" role="alert">{result.error_text}</p>
			{/if}
			<div class="max-h-[34rem] overflow-auto rounded-md border">
				<table class="w-full" data-testid="exp-rows">
					<thead>
						<tr>
							<th class="{th} text-right">LVL</th>
							<th class="{th} text-right">Experience</th>
							<th class="{th} text-right">Needed</th>
						</tr>
					</thead>
					<tbody>
						{#each result.rows as r (r.level)}
							<tr class="border-t">
								<td class="px-3 py-0.5 text-right tabular-nums">{r.level}</td>
								<td class="px-3 py-0.5 text-right tabular-nums">{r.exp_text}</td>
								<td class="text-muted-foreground px-3 py-0.5 text-right tabular-nums">{r.needed_text}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="text-muted-foreground text-sm">Working...</p>
		{/if}
	{/snippet}
</CalcFrame>
