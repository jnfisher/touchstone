<script lang="ts">
	// MME's Monsters filter frame: By Mob / By Lair, the regen, HP, exp, damage and magic boxes, the
	// lair/party boxes and the "Monster Filters" window. The boxes only hold the inputs; the list refetches
	// and the server runs FilterMonsters.
	import { Button } from '$lib/components/ui/button';
	import { getMonsterFilterStore } from '$lib/monsters.svelte';
	import { isFiltering, NO_LIMIT, type ExtraFilters, type PartyBoxes } from '$lib/monsters';

	const store = getMonsterFilterStore();
	const c = $derived(store.controls);
	const x = $derived(c.extras);
	const active = $derived(isFiltering(c));
	let more = $state(false);

	const field =
		'bg-background border-input focus-visible:ring-ring/50 h-7 rounded-md border px-2 text-sm tabular-nums outline-none focus-visible:ring-2 disabled:opacity-60';
	const lab = 'text-muted-foreground flex items-center gap-1.5 text-xs font-medium';

	const PARTY: { key: keyof PartyBoxes; label: string; tip: string }[] = [
		{ key: 'size', label: '# Party', tip: 'Party size; above one the list is made for a party' },
		{ key: 'ac', label: 'AC', tip: 'Average armour class of characters in the party' },
		{ key: 'dr', label: 'DR', tip: 'Average damage resistance of characters in the party' },
		{ key: 'mr', label: 'MR', tip: 'Average magic resistance of characters in the party' },
		{ key: 'dodge', label: 'Dodge', tip: 'Average dodge % of characters in the party' },
		{ key: 'hp', label: 'HP', tip: 'Average max health of characters in the party' },
		{ key: 'anti_magic', label: 'Anti-M', tip: 'Number of anti-magic players in the party' },
		{ key: 'rest_hp', label: 'RestHP', tip: 'Average resting HP per click of characters in the party' },
		{ key: 'accuracy', label: 'Acc', tip: 'Average accuracy of melee characters' },
		{ key: 'swings', label: 'Swing', tip: 'Average number of swings from melee characters' },
		{ key: 'damage_out_physical', label: 'Phys out', tip: 'Party physical damage output per round at 50/0 AC/DR' },
		{ key: 'damage_out_magic', label: 'Magic out', tip: 'Party magic damage output per round at 50 MR' }
	];

	type NumKey = {
		[K in keyof ExtraFilters]: ExtraFilters[K] extends number ? K : never;
	}[keyof ExtraFilters];
	const LIMITS: { key: NumKey; label: string; tip: string }[] = [
		{ key: 'armour_class', label: 'AC at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'damage_resist', label: 'DR at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'magic_res', label: 'MR at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'bs_def', label: 'BS def at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'dodge', label: 'Dodge at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'game_limit', label: 'Game limit at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'atk_accuracy_maj', label: 'Major acc at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'atk_accuracy_max', label: 'Max acc at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'avg_lair_exp', label: 'Lair exp at least', tip: 'Average exp of the monster\'s lairs' },
		{ key: 'num_lairs', label: 'Lairs at least', tip: 'Number of lairs the monster spawns in' },
		{ key: 'num_mobs_gte', label: 'Mobs/spawn at least', tip: 'Average mobs per spawn' },
		{ key: 'num_mobs_lte', label: 'Mobs/spawn at most', tip: `${NO_LIMIT} is no limit` },
		{ key: 'follow_gte', label: 'Follow % at least', tip: 'Chance to follow' },
		{ key: 'follow_lte', label: 'Follow % at most', tip: `${NO_LIMIT} is no limit` }
	];

	type FlagKey = {
		[K in keyof ExtraFilters]: ExtraFilters[K] extends boolean ? K : never;
	}[keyof ExtraFilters];
	const FLAGS: { key: FlagKey; label: string; tip?: string }[] = [
		{ key: 'is_undead', label: 'Undead only' },
		{ key: 'non_hostile_vs_evil', label: 'Non-hostile vs evil' },
		{ key: 'non_hostile_vs_ng', label: 'Non-hostile vs neutral/good' },
		{ key: 'drops_cash', label: 'Drops cash' },
		{ key: 'drops_r', label: 'Drops R' },
		{ key: 'drops_p', label: 'Drops P' },
		{ key: 'drops_g', label: 'Drops G' },
		{ key: 'drops_s', label: 'Drops S' },
		{ key: 'atk_no_poison', label: 'No poison attacks' },
		{ key: 'atk_no_confusion', label: 'No confusion attacks' },
		{ key: 'atk_no_fear', label: 'No fear attacks' },
		{
			key: 'reachable_only',
			label: 'Reachable only',
			tip: 'Needs the reachability module, which is not ported yet; the list is unchanged'
		},
		{ key: 'show_all', label: 'Show all (grey out misses)' }
	];

	function setAbility(i: number, col: 0 | 1 | 2, v: number) {
		const rows = x.abilities.map((r) => [...r]) as ExtraFilters['abilities'];
		rows[i][col] = Number.isFinite(v) ? Math.trunc(v) : 0;
		store.setExtras({ abilities: rows });
	}
</script>

<div class="border-b pb-2" role="group" aria-label="Monster filters" data-testid="monster-filters" data-active={active}>
	<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
		<span class="text-xs font-semibold tracking-wide uppercase {active ? 'text-primary' : 'text-muted-foreground'}">
			Filter
		</span>
		<span class="flex items-center gap-3 text-sm" role="radiogroup" aria-label="Filter by">
			<label class="flex items-center gap-1" title="Filter by individual mob stats">
				<input type="radio" name="monster-mode" checked={!c.lair} onchange={() => store.set({ lair: false })} data-testid="mf-mob" />
				By mob
			</label>
			<label class="flex items-center gap-1" title="Filter by overall lair values">
				<input type="radio" name="monster-mode" checked={c.lair} onchange={() => store.set({ lair: true })} data-testid="mf-lair" />
				By lair
			</label>
		</span>
		<label class={lab}>
			Regen
			<select
				class={field}
				value={c.regen_op}
				onchange={(e) => store.set({ regen_op: Number(e.currentTarget.value) === 1 ? 1 : 0 })}
				data-testid="mf-regen-op"
			>
				<option value={0}>&lt;=</option>
				<option value={1}>&gt;=</option>
			</select>
			<input
				class="{field} w-14 text-center"
				maxlength="4"
				value={c.regen}
				oninput={(e) => store.set({ regen: e.currentTarget.value })}
				data-testid="mf-regen"
			/>
		</label>
		<label class={lab}>
			HP
			<input
				class="{field} w-20 text-center"
				maxlength="14"
				value={c.hp}
				oninput={(e) => store.set({ hp: e.currentTarget.value })}
				data-testid="mf-hp"
			/>
		</label>
		<label class={lab} title="Exp (or exp/hr by lair); values ending in K or M are recognized">
			Exp
			<input
				class="{field} w-20 text-center"
				maxlength="14"
				value={c.exp}
				oninput={(e) => store.set({ exp: e.currentTarget.value })}
				data-testid="mf-exp"
			/>
		</label>
		<label class={lab}>
			Damage
			<input
				class="{field} w-20 text-center"
				maxlength="14"
				value={c.damage}
				oninput={(e) => store.set({ damage: e.currentTarget.value })}
				data-testid="mf-damage"
			/>
		</label>
		<label class={lab}>
			<input type="checkbox" checked={c.magic} onchange={(e) => store.set({ magic: e.currentTarget.checked })} data-testid="mf-magic" />
			Magic at most
			<input
				class="{field} w-14 text-center"
				maxlength="6"
				disabled={!c.magic}
				value={c.magic_value}
				oninput={(e) => store.set({ magic_value: e.currentTarget.value })}
				data-testid="mf-magic-value"
			/>
		</label>
		<Button type="button" variant="outline" size="sm" onclick={() => store.reset()} data-testid="mf-reset" title="Put the boxes back to their defaults and turn the extra filters off">
			Reset
		</Button>
		<Button
			type="button"
			variant="outline"
			size="sm"
			aria-expanded={more}
			onclick={() => (more = !more)}
			data-testid="mf-more"
		>
			{more ? 'Fewer filters' : 'More filters'}{x.enabled ? ' (on)' : ''}
		</Button>
	</div>

	{#if c.lair}
		<div class="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1" data-testid="mf-party">
			<span class="text-muted-foreground text-xs font-semibold tracking-wide uppercase">Party</span>
			{#each PARTY as p (p.key)}
				<label class={lab} title={p.tip}>
					{p.label}
					<input
						class="{field} w-14 text-center"
						maxlength="6"
						value={c.party[p.key]}
						oninput={(e) => store.setParty({ [p.key]: e.currentTarget.value })}
						data-testid="mf-party-{p.key}"
					/>
				</label>
			{/each}
		</div>
	{/if}

	{#if more}
		<div class="bg-card mt-2 rounded-md border p-2" data-testid="mf-extras">
			<label class="mb-2 flex items-center gap-1.5 text-sm font-medium">
				<input type="checkbox" checked={x.enabled} onchange={(e) => store.setExtras({ enabled: e.currentTarget.checked })} data-testid="mf-extras-enabled" />
				Use these extra filters
			</label>
			<div class="grid grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-x-4 gap-y-1.5 {x.enabled ? '' : 'opacity-60'}">
				{#each LIMITS as l (l.key)}
					<label class="{lab} justify-between" title={l.tip}>
						{l.label}
						<input
							type="number"
							class="{field} w-20 text-right"
							value={x[l.key]}
							oninput={(e) => store.setExtras({ [l.key]: Number.isFinite(e.currentTarget.valueAsNumber) ? e.currentTarget.valueAsNumber : 0 })}
							data-testid="mf-x-{l.key}"
						/>
					</label>
				{/each}
				<label class="{lab} justify-between" title="Only monsters that spawn in at least this many lairs">
					<span class="flex items-center gap-1.5">
						<input type="checkbox" checked={x.at_least_lairs_on} onchange={(e) => store.setExtras({ at_least_lairs_on: e.currentTarget.checked })} />
						At least lairs
					</span>
					<input
						type="number"
						class="{field} w-20 text-right"
						value={x.at_least_lairs}
						oninput={(e) => store.setExtras({ at_least_lairs: Number.isFinite(e.currentTarget.valueAsNumber) ? e.currentTarget.valueAsNumber : 1 })}
					/>
				</label>
			</div>
			<div class="mt-2 grid grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-x-4 gap-y-1 text-sm {x.enabled ? '' : 'opacity-60'}">
				{#each FLAGS as f (f.key)}
					<label class="flex items-center gap-1.5" title={f.tip}>
						<input type="checkbox" checked={x[f.key]} onchange={(e) => store.setExtras({ [f.key]: e.currentTarget.checked })} data-testid="mf-x-{f.key}" />
						{f.label}
					</label>
				{/each}
			</div>
			<div class="mt-2 flex flex-col gap-1 {x.enabled ? '' : 'opacity-60'}">
				<span class="text-muted-foreground text-xs font-medium">Abilities (ability number 0 is off)</span>
				{#each x.abilities as row, i (i)}
					<div class="flex items-center gap-2">
						<input
							type="number"
							class="{field} w-20 text-right"
							aria-label="Ability {i + 1} number"
							value={row[0]}
							oninput={(e) => setAbility(i, 0, e.currentTarget.valueAsNumber)}
						/>
						<select class={field} aria-label="Ability {i + 1} operator" value={row[1]} onchange={(e) => setAbility(i, 1, Number(e.currentTarget.value))}>
							<option value={0}>&lt;=</option>
							<option value={1}>&gt;=</option>
						</select>
						<input
							type="number"
							class="{field} w-20 text-right"
							aria-label="Ability {i + 1} value"
							value={row[2]}
							oninput={(e) => setAbility(i, 2, e.currentTarget.valueAsNumber)}
						/>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</div>
