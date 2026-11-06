<script lang="ts">
	// MME's Spells filter frame: magery, magery level, learnable only, attack type, target and "contains
	// ability". The boxes only hold the inputs; the list refetches and the server runs FilterSpells.
	import { api, type AbilityName } from '$lib/api';
	import { spellFilters } from '$lib/explorer-state.svelte';
	import { getSettingsStore } from '$lib/settings.svelte';
	import {
		DEFAULT_SPELL_FILTERS,
		MAGERY_LEVELS,
		MAGERY_NAMES,
		SPELL_ATTACK_TYPES,
		SPELL_TARGETS,
		spellFiltersActive,
		type SpellFilterBoxes
	} from '$lib/spelltools';

	const active = $derived(spellFiltersActive(spellFilters));
	const settings = getSettingsStore();
	let abilities = $state.raw<AbilityName[]>([]);
	// Names carry their record numbers unless the Settings window hides them, so ask again after a save.
	$effect(() => {
		void settings.current.hide_record_numbers;
		api.abilityList().then(
			(a) => (abilities = a),
			() => {}
		);
	});

	const field =
		'bg-background border-input focus-visible:ring-ring/50 h-7 rounded-md border px-2 text-sm outline-none focus-visible:ring-2';
	const lab = 'text-muted-foreground flex items-center gap-1.5 text-xs font-medium';

	function set(patch: Partial<SpellFilterBoxes>) {
		Object.assign(spellFilters, patch);
	}
	const num = (e: Event & { currentTarget: HTMLSelectElement }) => Number(e.currentTarget.value);
</script>

<div class="border-b pb-2" role="group" aria-label="Spell filters" data-testid="spell-filters" data-active={active}>
	<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
		<span class="text-xs font-semibold tracking-wide uppercase {active ? 'text-primary' : 'text-muted-foreground'}">
			Filter
		</span>
		<label class={lab}>
			Magery
			<select class={field} value={spellFilters.magery} onchange={(e) => set({ magery: num(e) })} data-testid="sf-magery">
				{#each MAGERY_NAMES as name, i (i)}<option value={i}>{name}</option>{/each}
			</select>
		</label>
		<label class={lab} title="At least this magery level">
			Magery level
			<select
				class={field}
				value={spellFilters.magery_level}
				onchange={(e) => set({ magery_level: num(e) })}
				data-testid="sf-magery-level"
			>
				{#each MAGERY_LEVELS as name, i (i)}<option value={i}>{name}</option>{/each}
			</select>
		</label>
		<label class={lab} title="Only spells a character can learn">
			<input
				type="checkbox"
				checked={spellFilters.learnable_only}
				onchange={(e) => set({ learnable_only: e.currentTarget.checked })}
				data-testid="sf-learnable"
			/>
			Learnable only
		</label>
		<label class={lab}>
			Attack type
			<select
				class={field}
				value={spellFilters.attack_type}
				onchange={(e) => set({ attack_type: num(e) })}
				data-testid="sf-attack-type"
			>
				{#each SPELL_ATTACK_TYPES as name, i (i)}<option value={i}>{name}</option>{/each}
			</select>
		</label>
		<label class={lab}>
			Target
			<select class={field} value={spellFilters.target} onchange={(e) => set({ target: num(e) })} data-testid="sf-target">
				{#each SPELL_TARGETS as name, i (i)}<option value={i}>{name}</option>{/each}
			</select>
		</label>
		<label class={lab}>
			Contains ability
			<select
				class="{field} max-w-56"
				value={spellFilters.contains_ability}
				onchange={(e) => set({ contains_ability: num(e) })}
				data-testid="sf-ability"
			>
				<option value={0}>Any</option>
				{#each abilities as a (a.number)}<option value={a.number}>{a.name}</option>{/each}
			</select>
		</label>
		<button
			type="button"
			class="bg-background border-input hover:bg-accent h-7 rounded-md border px-2 text-xs disabled:opacity-50"
			disabled={!active}
			onclick={() => set({ ...DEFAULT_SPELL_FILTERS })}
			data-testid="sf-reset"
		>
			Remove filters
		</button>
	</div>
</div>
