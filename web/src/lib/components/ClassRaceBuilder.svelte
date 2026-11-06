<script lang="ts">
	// MME's Classes/Races "Build" view: the selected row paired with a class or race picked here,
	// the exp chart, HP range and swings at four level steps, and the Equip and Spellbook buttons
	// (cmdClassRace_Click). The text is the server's, exactly as MME's text box holds it.
	import { goto } from '$app/navigation';
	import { untrack } from 'svelte';
	import { api, type ClassRec, type ClassRaceBuild, type Race } from '$lib/api';
	import { boxText, pairFor, spellbookNote } from '$lib/builder';
	import { spellBook } from '$lib/spellbook.svelte';
	import { bookSeed } from '$lib/spelltools';
	import { getCharacterStore } from '$lib/character.svelte';
	import { Button } from '$lib/components/ui/button';

	let { kind, number }: { kind: 'race' | 'class'; number: number } = $props();

	const store = getCharacterStore();
	let others = $state.raw<(Race | ClassRec)[]>([]);
	let mine = $state.raw<ClassRec | Race>();
	let otherNumber = $state<number>();
	let build = $state.raw<ClassRaceBuild>();
	let error = $state<string>();
	let note = $state('');
	let ticket = 0;

	const otherLabel = $derived(kind === 'race' ? 'Class' : 'Race');
	const pair = $derived(pairFor(kind, number, otherNumber));
	// MME keeps the old text when its handler ran; the box starts empty.
	let shown = $state('');

	$effect(() => {
		const which = kind === 'race' ? 'classes' : 'races';
		const own = kind === 'race' ? 'races' : 'classes';
		let live = true;
		api.table(which).then((t) => {
			if (!live) return;
			const named = t.filter((r) => r.name !== '');
			others = named;
			const cur = untrack(() => otherNumber);
			if (cur === undefined || !named.some((r) => r.number === cur)) otherNumber = named[0]?.number;
		});
		api.table(own).then((t) => {
			if (live) mine = t.find((r) => r.number === number);
		});
		return () => (live = false);
	});

	$effect(() => {
		const { class: c, race: r } = pair;
		if (otherNumber === undefined) return;
		const id = ++ticket;
		note = '';
		api
			.classRaceBuild(c, r)
			.then((b) => {
				if (id !== ticket) return;
				build = b;
				if (b.text !== null) shown = boxText(b.text);
				error = b.error ?? undefined;
			})
			.catch((e) => {
				if (id === ticket) error = e instanceof Error ? e.message : String(e);
			});
	});

	const className = $derived(
		(kind === 'class' ? mine : others.find((o) => o.number === otherNumber))?.name ?? ''
	);

	async function equip() {
		const out = await api.builderEquip(pair.class, pair.race);
		store.edit((d) => {
			if (out.class !== null) d.class = out.class;
			if (out.race !== null) d.race = out.race;
		});
		// MME then switches to the EQ tab; the equipment panel is on the Character tab.
		void goto('/character');
	}

	async function spellbook() {
		try {
			const sb = await api.builderSpellbook(pair.class);
			note = spellbookNote(sb, className);
			// cmdClassRace_Click opens the Spell Book at level 999 with the class and its magery
			const seed = bookSeed(sb);
			if (seed) spellBook.show(seed);
		} catch (e) {
			note = e instanceof Error ? e.message : String(e);
		}
	}
</script>

<section class="mt-4" data-testid="class-race-builder">
	<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">Builder</h3>
	<div class="mb-2 flex flex-wrap items-center gap-x-3 gap-y-1.5">
		<label class="flex items-center gap-2">
			<span>{otherLabel}</span>
			<select
				bind:value={otherNumber}
				data-testid="builder-other"
				class="bg-background border-input h-7 rounded-md border px-2"
			>
				{#each others as o (o.number)}
					<option value={o.number}>{o.name}</option>
				{/each}
			</select>
		</label>
		<Button size="sm" variant="outline" data-testid="builder-equip" onclick={equip}>Equip</Button>
		<Button size="sm" variant="outline" data-testid="builder-spellbook" disabled={pair.class === 0} onclick={spellbook}>
			Spellbook
		</Button>
		{#if note}<span class="text-muted-foreground" data-testid="builder-note" role="status">{note}</span>{/if}
	</div>
	{#if error}
		<p class="text-destructive mb-1 text-sm" role="alert">{build?.error ?? `Could not build: ${error}`}</p>
	{/if}
	<!-- A read-only box like MME's txtClassRaceDetail: focusing it selects everything. -->
	<textarea
		readonly
		rows="6"
		wrap="off"
		aria-label="Builder output"
		data-testid="builder-text"
		value={shown}
		onfocus={(e) => e.currentTarget.select()}
		class="bg-background border-input w-full resize-y overflow-auto rounded-md border p-2 font-mono text-xs leading-5"
	></textarea>
</section>
