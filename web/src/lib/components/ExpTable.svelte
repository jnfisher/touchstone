<script lang="ts">
	// Exp needed per level and the cost to train it, for one race with a chosen class or one class
	// with a chosen race (CalcExpNeededByRaceClass, CalcMoneyRequiredToTrain).
	import { untrack } from 'svelte';
	import { api, type ClassRec, type ExpRow, type Race } from '$lib/api';

	let { kind, number }: { kind: 'race' | 'class'; number: number } = $props();

	let others = $state.raw<(Race | ClassRec)[]>([]);
	let otherNumber = $state<number>();
	let markup = $state(0);
	let rows = $state.raw<ExpRow[]>([]);
	let error = $state<string>();
	let ticket = 0;

	const otherLabel = $derived(kind === 'race' ? 'Class' : 'Race');

	$effect(() => {
		const which = kind === 'race' ? 'classes' : 'races';
		let live = true;
		api.table(which).then((t) => {
			if (!live) return;
			const named = t.filter((r) => r.name !== '');
			others = named;
			const cur = untrack(() => otherNumber);
			if (cur === undefined || !named.some((r) => r.number === cur)) otherNumber = named[0]?.number;
		});
		return () => (live = false);
	});

	$effect(() => {
		const other = otherNumber;
		const m = markup;
		if (other === undefined) return;
		const mine = ++ticket;
		api
			.expTable({
				race: kind === 'race' ? number : other,
				class: kind === 'race' ? other : number,
				from: 1,
				to: 255,
				markup: m
			})
			.then((d) => {
				if (mine !== ticket) return;
				rows = d.rows;
				error = undefined;
			})
			.catch((e) => {
				if (mine !== ticket) return;
				error = e instanceof Error ? e.message : String(e);
			});
	});

	const th = 'text-muted-foreground bg-card sticky top-0 px-2 py-1 text-xs font-medium';
</script>

<section class="mt-5" data-testid="exp-table">
	<h3 class="text-muted-foreground mb-2 text-xs font-medium tracking-wide uppercase">Exp table</h3>
	<div class="mb-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
		<label class="flex items-center gap-2">
			<span>{otherLabel}</span>
			<select
				bind:value={otherNumber}
				data-testid="exp-other"
				class="bg-background border-input h-7 rounded-md border px-2"
			>
				{#each others as o (o.number)}
					<option value={o.number}>{o.name}</option>
				{/each}
			</select>
		</label>
		<label class="flex items-center gap-2" title="Trainer markup percent for the training cost">
			<span>Trainer markup %</span>
			<input
				type="number"
				min="0"
				max="999"
				bind:value={markup}
				class="bg-background border-input h-7 w-16 rounded-md border px-2 text-center tabular-nums"
			/>
		</label>
	</div>
	{#if error}
		<p class="text-destructive text-sm" role="alert">Could not load the exp table: {error}</p>
	{:else}
		<div class="max-h-96 overflow-y-auto rounded-md border">
			<table class="w-full text-sm">
				<thead>
					<tr>
						<th class={th + ' text-right'}>Level</th>
						<th class={th + ' text-right'}>Exp needed</th>
						<th class={th + ' text-left'}>Cost to train</th>
					</tr>
				</thead>
				<tbody>
					{#each rows as r (r.level)}
						<tr class="border-t">
							<td class="px-2 py-0.5 text-right tabular-nums">{r.level}</td>
							<td class="px-2 py-0.5 text-right tabular-nums">{r.exp_text}</td>
							<td class="px-2 py-0.5">{r.train_cost}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</section>
