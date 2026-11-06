<script lang="ts">
	// MME's Settings window. Results-affecting options are saved on the server; "View" options stay
	// in this browser. Numbers are typed as text and checked live against MME's save rules.
	import { untrack } from 'svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { getCharacterStore } from '$lib/character.svelte';
	import { getSettingsStore } from '$lib/settings.svelte';
	import {
		defaultBlock,
		filterNumberText,
		noModelTicked,
		predictKnob,
		predictSimRounds,
		predictWaypoint,
		resetExpFields,
		toDraft,
		type Draft,
		type KnobKey,
		type ModelKey
	} from '$lib/settings';
	import type { ThemeStore } from '$lib/theme.svelte';
	import type { ViewStore } from '$lib/view.svelte';
	import { inputCls } from '$lib/components/calc/styles';
	import { KNOB_HELP, MODEL_HELP, MODEL_NOTES, CLUSTER_NOTE } from './help';

	let {
		open = $bindable(false),
		theme,
		view
	}: { open?: boolean; theme: ThemeStore; view: ViewStore } = $props();

	const settings = getSettingsStore();
	const character = getCharacterStore();

	let draft = $state<Draft>(toDraft(settings.current));
	let help = $state<string>();
	let problem = $state<string>();

	/** A saved character is open, so "by character" has somewhere to write. */
	const characterName = $derived(character.storedName);

	function fill() {
		const s = settings.current;
		const own = s.exp_per_hour.by_character && characterName ? (character.doc.exp_hr ?? defaultBlock()) : undefined;
		draft = toDraft(s, own);
	}

	// Set by the first edit, so the values arriving from the server never overwrite typing.
	let dirty = false;

	$effect(() => {
		if (!open) return;
		let alive = true;
		untrack(() => {
			dirty = false;
			problem = undefined;
			help = undefined;
			fill();
			void settings.reload().then(() => alive && !dirty && fill());
		});
		return () => (alive = false);
	});

	/** MME offers to reload the boxes from the character's or the global values when this flips. */
	function byCharacterChanged() {
		const s = settings.current;
		const block = draft.byCharacter && characterName ? (character.doc.exp_hr ?? defaultBlock()) : s.exp_per_hour.global;
		const keep = { simRounds: draft.simRounds, waypoint: draft.waypoint, autoLoad: draft.autoLoad, by: draft.byCharacter };
		draft = toDraft(s, block);
		draft.simRounds = keep.simRounds;
		draft.waypoint = keep.waypoint;
		draft.autoLoad = keep.autoLoad;
		draft.byCharacter = keep.by;
	}

	const knobNote = (k: KnobKey) => predictKnob(draft.knobs[k]).note;
	const roundsNote = $derived(predictSimRounds(draft.simRounds).note);
	const waypointNote = $derived(predictWaypoint(draft.waypoint).note);

	async function save() {
		problem = undefined;
		const r = await settings.save(draft, !!characterName);
		if (!r.ok) {
			problem = settings.error ?? 'Could not save';
			return;
		}
		if (r.characterBlock) {
			const block = r.characterBlock;
			character.edit((d) => (d.exp_hr = block));
		}
		open = false;
	}

	function reset() {
		dirty = true;
		resetExpFields(draft);
	}

	const MODEL_KEYS: ModelKey[] = ['model_a', 'model_b', 'model_c', 'model_d'];
	const legend = 'text-muted-foreground mb-2 text-[11px] font-medium tracking-wide uppercase';
	const noteCls = 'text-muted-foreground text-xs';
	const helpBtn =
		'text-muted-foreground hover:text-foreground border-input inline-flex size-5 shrink-0 items-center justify-center rounded-full border text-[11px] leading-none';
</script>

<Dialog.Root bind:open>
	<Dialog.Content data-testid="settings-dialog" class="max-w-3xl">
		<div class="border-b px-5 py-3">
			<Dialog.Title>Settings</Dialog.Title>
			<Dialog.Description>
				Exp/hr, simulation and startup options are saved on the server. View options stay in this browser.
			</Dialog.Description>
		</div>

		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="grid min-h-0 flex-1 gap-5 overflow-y-auto px-5 py-4 md:grid-cols-2"
			oninput={() => (dirty = true)}
			onchange={() => (dirty = true)}
		>
			<section data-testid="settings-models">
				<h3 class={legend}>Exp/hr models</h3>
				<div class="flex flex-col gap-1.5">
					{#each MODEL_KEYS as key, i (key)}
						<div class="flex items-center gap-2">
							<label class="flex flex-1 items-center gap-2 text-sm">
								<input type="checkbox" class="accent-primary size-4" bind:checked={draft.block[key]} data-testid="settings-{key}" />
								{MODEL_HELP[i].title}
							</label>
							<button
								type="button"
								class={helpBtn}
								aria-label="About {MODEL_HELP[i].title}"
								aria-expanded={help === key}
								onclick={() => (help = help === key ? undefined : key)}>?</button
							>
						</div>
						{#if help === key}
							<p class="{noteCls} bg-muted rounded-md px-2 py-1.5">{MODEL_HELP[i].text}</p>
						{/if}
					{/each}
					<label class="mt-1 flex items-center gap-2 text-sm">
						<input type="checkbox" class="accent-primary size-4" bind:checked={draft.block.recovery_only} data-testid="settings-recovery-only" />
						No recovery
					</label>
					<label class="flex items-center gap-2 text-sm">
						<input type="checkbox" class="accent-primary size-4" bind:checked={draft.block.show_all} data-testid="settings-show-all" />
						Show in detail
					</label>
				</div>
				{#if noModelTicked(draft.block)}
					<p class="text-foreground font-medium mt-2 text-xs" data-testid="settings-none-ticked">
						No model ticked: Save turns on A, B and C, as MME does.
					</p>
				{/if}
				<ul class="{noteCls} mt-3 list-disc space-y-1 pl-4">
					{#each MODEL_NOTES as n (n)}<li>{n}</li>{/each}
				</ul>
			</section>

			<section data-testid="settings-knobs">
				<h3 class={legend}>Knobs (0.01 to 10, 1 is neutral)</h3>
				<div class="grid grid-cols-2 gap-x-3 gap-y-2">
					{#each KNOB_HELP as k (k.key)}
						<div class="flex flex-col gap-1">
							<div class="flex items-center justify-between">
								<label for="knob-{k.key}" class="text-muted-foreground text-xs font-medium">{k.label}</label>
								<button
									type="button"
									class={helpBtn}
									aria-label="About the {k.label} knob"
									aria-expanded={help === k.key}
									onclick={() => (help = help === k.key ? undefined : k.key)}>?</button
								>
							</div>
							<input
								id="knob-{k.key}"
								class={inputCls}
								inputmode="decimal"
								autocomplete="off"
								value={draft.knobs[k.key]}
								oninput={(e) => {
									const f = filterNumberText(e.currentTarget.value, true);
									draft.knobs[k.key] = f;
									e.currentTarget.value = f;
								}}
								data-testid="settings-{k.key}"
							/>
							{#if knobNote(k.key)}
								<span class="text-foreground font-medium text-xs" data-testid="settings-{k.key}-note">{knobNote(k.key)}</span>
							{/if}
						</div>
					{/each}
				</div>
				{#if help && KNOB_HELP.some((k) => k.key === help)}
					<p class="{noteCls} bg-muted mt-2 rounded-md px-2 py-1.5">
						{KNOB_HELP.find((k) => k.key === help)?.text}
						{#if help === 'knob_move'}{CLUSTER_NOTE}{/if}
					</p>
				{/if}
				<label class="mt-3 flex items-start gap-2 text-sm">
					<input type="checkbox" class="accent-primary mt-0.5 size-4" bind:checked={draft.byCharacter} onchange={byCharacterChanged} data-testid="settings-by-character" />
					<span>
						Keep models and knobs per character
						<span class="{noteCls} block" data-testid="settings-by-character-note">
							{#if !draft.byCharacter}Models and knobs are shared by every character.
							{:else if characterName}Saved with {characterName}; the shared values stay as they are.
							{:else}No saved character is open, so Save writes the shared values.{/if}
						</span>
					</span>
				</label>
				<button type="button" class="text-muted-foreground hover:text-foreground mt-2 text-xs underline underline-offset-2" onclick={reset} data-testid="settings-reset">
					Reset models and knobs to defaults
				</button>
			</section>

			<section data-testid="settings-sim">
				<h3 class={legend}>Simulation and routes</h3>
				<div class="grid grid-cols-2 gap-3">
					<div class="flex flex-col gap-1">
						<label for="sim-rounds" class="text-muted-foreground text-xs font-medium">Monster sim rounds</label>
						<input
							id="sim-rounds"
							class={inputCls}
							inputmode="numeric"
							autocomplete="off"
							value={draft.simRounds}
							oninput={(e) => {
								const f = filterNumberText(e.currentTarget.value, false);
								draft.simRounds = f;
								e.currentTarget.value = f;
							}}
							data-testid="settings-sim-rounds"
						/>
						<span class={noteCls}>100 to 10000, more is steadier but slower.</span>
						{#if roundsNote}<span class="text-foreground font-medium text-xs" data-testid="settings-sim-rounds-note">{roundsNote}</span>{/if}
					</div>
					<div class="flex flex-col gap-1">
						<label for="waypoint" class="text-muted-foreground text-xs font-medium">Route waypoint interval</label>
						<input
							id="waypoint"
							class={inputCls}
							inputmode="numeric"
							autocomplete="off"
							value={draft.waypoint}
							oninput={(e) => {
								const f = filterNumberText(e.currentTarget.value, false);
								draft.waypoint = f;
								e.currentTarget.value = f;
							}}
							data-testid="settings-waypoint"
						/>
						<span class={noteCls}>Rooms between waypoints in a MegaMUD route (1 to 1000).</span>
						{#if waypointNote}<span class="text-foreground font-medium text-xs" data-testid="settings-waypoint-note">{waypointNote}</span>{/if}
					</div>
				</div>
			</section>

			<section data-testid="settings-general">
				<h3 class={legend}>Character</h3>
				<label class="flex items-center gap-2 text-sm">
					<input type="checkbox" class="accent-primary size-4" bind:checked={draft.autoLoad} data-testid="settings-auto-load" />
					Open the last character at start
				</label>
				<h3 class="{legend} mt-5">View (this browser)</h3>
				<div class="flex flex-col gap-1.5">
					<label class="flex items-center gap-2 text-sm">
						<input
							type="checkbox"
							class="accent-primary size-4"
							checked={view.options.name_in_title}
							onchange={(e) => view.set({ name_in_title: e.currentTarget.checked })}
							data-testid="settings-name-in-title"
						/>
						Show the character's name in the page title
					</label>
					<label class="flex items-center gap-2 text-sm">
						<input
							type="checkbox"
							class="accent-primary size-4"
							checked={theme.current === 'dark'}
							onchange={(e) => theme.set(e.currentTarget.checked ? 'dark' : 'light')}
							data-testid="settings-dark"
						/>
						Dark mode
					</label>
				</div>
			</section>
		</div>

		<div class="flex items-center justify-between gap-3 border-t px-5 py-3">
			<p class="text-destructive min-w-0 truncate text-xs" role="alert" data-testid="settings-problem">
				{problem ?? (settings.error && !settings.loaded ? `Could not read the saved settings: ${settings.error}` : '')}
			</p>
			<div class="flex shrink-0 gap-2">
				<Button variant="outline" size="sm" onclick={() => (open = false)} data-testid="settings-cancel">Cancel</Button>
				<Button size="sm" onclick={() => void save()} disabled={settings.saving} data-testid="settings-save">Save</Button>
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>
