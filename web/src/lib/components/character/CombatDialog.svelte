<script lang="ts">
	// MME's combat setup window (frmPopUpOptions): the attack on one page, the healing on the next
	// (NMR 1.83+), and the live numbers under the controls. Nothing is computed here; the server
	// previews the draft and says what Save stores.
	import type { CharacterStore } from '$lib/character.svelte';
	import {
		ATTACK_RADIOS,
		HEAL_RADIOS,
		MARTIAL_ARTS,
		boxInt,
		buildDraft,
		combatClient,
		formFromAttack,
		recoveryOnly,
		setBash,
		setSmash,
		stepRounds,
		type Choice,
		type CombatClient,
		type CombatForm,
		type CombatOptions,
		type CombatPreview
	} from '$lib/combat';
	import { getSettingsStore } from '$lib/settings.svelte';
	import { inputCls, labelCls } from '../calc/styles';

	let {
		store,
		onclose,
		client = combatClient
	}: { store: CharacterStore; onclose: () => void; client?: CombatClient } = $props();

	const settings = getSettingsStore();
	let page = $state<'attack' | 'heal'>('attack');
	let opts = $state.raw<CombatOptions>();
	let form = $state<CombatForm>();
	let preview = $state.raw<CombatPreview>();
	let error = $state<string>();
	let saving = $state(false);

	const twoPages = $derived((opts?.nmr_ver ?? 0) >= 1.83);

	$effect(() => {
		let alive = true;
		void settings.load();
		const doc = $state.snapshot(store.doc);
		client
			.options(doc, recoveryOnly(doc, settings.current))
			.then((o) => {
				if (!alive) return;
				opts = o;
				form = formFromAttack(doc.attack, o, doc.global_filter && doc.level > 0 ? doc.level : undefined);
			})
			.catch((e: unknown) => (error = e instanceof Error ? e.message : String(e)));
		return () => (alive = false);
	});

	// MME refreshes from a timer a moment after each change; this is that timer.
	let ticket = 0;
	$effect(() => {
		if (!form) return;
		const draft = buildDraft($state.snapshot(form));
		const doc = $state.snapshot(store.doc);
		const mine = ++ticket;
		const t = setTimeout(() => {
			client
				.preview(doc, draft, recoveryOnly(doc, settings.current))
				.then((p) => {
					if (mine !== ticket) return;
					if (!p.error_text) preview = p;
					error = p.error_text ?? undefined;
					// the dialog corrects the rounds box itself (a timed spell needs its duration)
					if (form && !p.error_text && form.healRadio >= 2 && p.heal.rounds !== form.healRounds && form.healRounds < p.heal.rounds) {
						form.healRounds = p.heal.rounds;
					}
				})
				.catch((e: unknown) => {
					if (mine === ticket) error = e instanceof Error ? e.message : String(e);
				});
		}, 120);
		return () => clearTimeout(t);
	});

	async function save() {
		if (!form) return;
		saving = true;
		try {
			const doc = $state.snapshot(store.doc);
			const p = await client.preview(doc, buildDraft($state.snapshot(form)), recoveryOnly(doc, settings.current));
			store.edit((d) => (d.attack = p.applied));
			onclose();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			saving = false;
		}
	}

	const btn = 'border-input h-8 rounded-md border px-3 text-sm disabled:opacity-50';
	const primary = 'bg-primary text-primary-foreground h-8 rounded-md px-3 text-sm disabled:opacity-50';
	const tabBtn = (on: boolean) =>
		`h-8 rounded-md px-3 text-sm ${on ? 'bg-secondary text-secondary-foreground font-medium' : 'text-muted-foreground'}`;
	const sel = `${inputCls} min-w-0`;
	const group = 'flex flex-col gap-2 rounded-md border p-3';
</script>

{#snippet picker(list: Choice[], value: number, onpick: (n: number) => void, disabled: boolean, id: string)}
	<select
		class={sel}
		{disabled}
		value={value}
		onchange={(e) => onpick(Number(e.currentTarget.value))}
		data-testid={id}
		aria-label={id}
	>
		<option value={0}>Select...</option>
		{#each list as c (c.number)}<option value={c.number}>{c.label}</option>{/each}
	</select>
{/snippet}

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="bg-background/80 fixed inset-0 z-50 flex items-center justify-center p-4" role="presentation">
	<div
		class="bg-card text-card-foreground flex max-h-full w-full max-w-3xl flex-col overflow-hidden rounded-lg border shadow-xl"
		role="dialog"
		aria-modal="true"
		aria-label="Combat setup"
		data-testid="combat-dialog"
	>
		<header class="flex items-center gap-2 border-b p-3">
			<h2 class="mr-3 text-base font-semibold">Combat setup</h2>
			<button type="button" class={tabBtn(page === 'attack')} onclick={() => (page = 'attack')} data-testid="combat-tab-attack">Attack</button>
			{#if twoPages}
				<button type="button" class={tabBtn(page === 'heal')} onclick={() => (page = 'heal')} data-testid="combat-tab-heal">Healing</button>
			{/if}
		</header>

		{#if error}<p class="text-destructive px-3 pt-2 text-sm" data-testid="combat-error">{error}</p>{/if}

		{#if form && opts}
			<div class="min-h-0 flex-1 overflow-auto p-3">
				{#if page === 'attack'}
					<div class="flex flex-col gap-3" data-testid="combat-attack">
						<p class="text-muted-foreground text-xs">
							Weapon: <span data-testid="combat-weapon">{opts.current_weapon}</span>
						</p>
						<div class={group}>
							{#each ATTACK_RADIOS as label, k (k)}
								<div class="flex flex-wrap items-center gap-2">
									<label class="flex w-36 items-center gap-2 text-sm">
										<input type="radio" name="attack" checked={form.radio === k} onchange={() => (form!.radio = k)} data-testid="combat-radio-{k}" />
										{label}
									</label>
									{#if k === 1}
										<label class="flex items-center gap-1 text-sm"><input type="checkbox" disabled={form.radio !== 1} checked={form.bash} onchange={(e) => setBash(form!, e.currentTarget.checked)} data-testid="combat-bash" />Bash</label>
										<label class="flex items-center gap-1 text-sm"><input type="checkbox" disabled={form.radio !== 1} checked={form.smash} onchange={(e) => setSmash(form!, e.currentTarget.checked)} data-testid="combat-smash" />Smash</label>
									{:else if k === 2}
										<div class="min-w-0 flex-1">{@render picker(opts.attack_learned, form.learnedSpell, (n) => (form!.learnedSpell = n), form.radio !== 2, 'combat-learned-spell')}</div>
									{:else if k === 3}
										<div class="min-w-0 flex-1">{@render picker(opts.attack_any, form.anySpell, (n) => (form!.anySpell = n), form.radio !== 3, 'combat-any-spell')}</div>
										<label class="flex items-center gap-1 text-sm">
											<span class={labelCls}>Level</span>
											<input class="{inputCls} w-20" inputmode="numeric" disabled={form.radio !== 3} value={form.spellLvl} oninput={(e) => (form!.spellLvl = boxInt(e.currentTarget.value, 0, 9999))} data-testid="combat-spell-lvl" />
										</label>
									{:else if k === 4}
										<select class="{sel} w-40" disabled={form.radio !== 4} value={form.martialArt} onchange={(e) => (form!.martialArt = Number(e.currentTarget.value))} data-testid="combat-ma" aria-label="Martial art">
											{#each MARTIAL_ARTS as m, j (j)}<option value={j}>{m}</option>{/each}
										</select>
									{:else if k === 5}
										<label class="flex items-center gap-1 text-sm"><span class={labelCls}>Physical</span>
											<input class="{inputCls} w-24" inputmode="numeric" disabled={form.radio !== 5} value={form.manualP} oninput={(e) => (form!.manualP = boxInt(e.currentTarget.value, 0, 999999))} data-testid="combat-manual-p" />
										</label>
										<label class="flex items-center gap-1 text-sm"><span class={labelCls}>Magic</span>
											<input class="{inputCls} w-24" inputmode="numeric" disabled={form.radio !== 5} value={form.manualM} oninput={(e) => (form!.manualM = boxInt(e.currentTarget.value, 0, 99999))} data-testid="combat-manual-m" />
										</label>
									{/if}
								</div>
							{/each}
						</div>
						<div class={group}>
							<div class="flex flex-wrap items-center gap-2">
								<label class="flex w-36 items-center gap-2 text-sm">
									<input type="checkbox" disabled={form.radio === 0} bind:checked={form.backstab} data-testid="combat-backstab" />
									Backstab first
								</label>
								<select class="{sel} flex-1" disabled={form.radio === 0 || !form.backstab} value={form.backstabWeapon} onchange={(e) => (form!.backstabWeapon = Number(e.currentTarget.value))} data-testid="combat-bs-weapon" aria-label="Backstab weapon">
									<option value={0}>[Equipped Weapon]</option>
									<option value={-1}>[No Weapon (Punch)]</option>
									{#each opts.backstab_weapons as c (c.number)}<option value={c.number}>{c.label}</option>{/each}
								</select>
							</div>
							{#if twoPages}
								<label class="flex items-center gap-2 text-sm">
									<input type="checkbox" disabled={form.radio !== 2 && form.radio !== 3} bind:checked={form.meditate} data-testid="combat-meditate" />
									Meditate for mana
								</label>
							{/if}
						</div>
					</div>
				{:else}
					<div class="flex flex-col gap-3" data-testid="combat-heal">
						{#if opts.recovery_only}<p class="text-muted-foreground text-sm" data-testid="combat-recovery-note">NOTE: Calculating recovery is disabled in settings.</p>{/if}
						<div class={group}>
							{#each HEAL_RADIOS as label, k (k)}
								<div class="flex flex-wrap items-center gap-2">
									<label class="flex w-36 items-center gap-2 text-sm">
										<input type="radio" name="heal" checked={form.healRadio === k} onchange={() => (form!.healRadio = k)} data-testid="combat-heal-radio-{k}" />
										{label}
									</label>
									{#if k === 2}
										<div class="min-w-0 flex-1">{@render picker(opts.heal_learned, form.healLearned, (n) => (form!.healLearned = n), form.healRadio !== 2, 'combat-heal-learned')}</div>
									{:else if k === 3}
										<div class="min-w-0 flex-1">{@render picker(opts.heal_any, form.healAny, (n) => (form!.healAny = n), form.healRadio !== 3, 'combat-heal-any')}</div>
										<label class="flex items-center gap-1 text-sm"><span class={labelCls}>Level</span>
											<input class="{inputCls} w-20" inputmode="numeric" disabled={form.healRadio !== 3} value={form.healLvl} oninput={(e) => (form!.healLvl = boxInt(e.currentTarget.value, 0, 9999))} data-testid="combat-heal-lvl" />
										</label>
									{:else if k === 4}
										<input class="{inputCls} w-28" inputmode="numeric" disabled={form.healRadio !== 4} value={form.healManual} oninput={(e) => (form!.healManual = boxInt(e.currentTarget.value, 0, 999999))} data-testid="combat-heal-manual" aria-label="Manual heal" />
									{/if}
								</div>
							{/each}
							<div class="flex items-center gap-2 text-sm">
								<span class={labelCls}>Rounds to cast over</span>
								<button type="button" class={btn} disabled={form.healRadio < 2 || form.healRadio > 3} onclick={() => (form!.healRounds = stepRounds(form!.healRounds, -1))} aria-label="Fewer rounds">-</button>
								<input class="{inputCls} w-16" inputmode="numeric" disabled={form.healRadio < 2 || form.healRadio > 3} value={form.healRounds} oninput={(e) => (form!.healRounds = boxInt(e.currentTarget.value, 1, 99))} data-testid="combat-heal-rounds" />
								<button type="button" class={btn} disabled={form.healRadio < 2 || form.healRadio > 3} onclick={() => (form!.healRounds = stepRounds(form!.healRounds, 1))} aria-label="More rounds">+</button>
							</div>
						</div>
					</div>
				{/if}

				<section class="mt-3 grid gap-3 rounded-md border p-3 sm:grid-cols-2" aria-label="Preview" data-testid="combat-preview">
					<div>
						<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">Attack</h3>
						{#if preview}
							<p class="text-sm font-medium whitespace-pre-line tabular-nums" data-testid="combat-dmg">{preview.attack.caption}</p>
							{#if preview.attack.oom_caption}<p class="text-muted-foreground text-xs" data-testid="combat-attack-oom">{preview.attack.oom_caption}</p>{/if}
							{#if preview.attack.hit_chance !== null}<p class="text-muted-foreground text-xs">{preview.attack.hit_chance}% to hit, {preview.attack.swings} swings/rnd</p>{/if}
							{#if preview.attack.cast_chance !== null}<p class="text-muted-foreground text-xs">{preview.attack.cast_chance}% cast, {preview.attack.avg_cast} per cast, {preview.attack.mana_cost} mana</p>{/if}
							<p class="text-muted-foreground mt-1 text-xs" data-testid="combat-name">Button reads: {preview.attack_name}</p>
						{:else}<p class="text-muted-foreground text-sm">Calculating...</p>{/if}
					</div>
					<div>
						<h3 class="text-muted-foreground mb-1 text-[11px] font-medium tracking-wide uppercase">Healing</h3>
						{#if preview}
							<p class="text-sm font-medium tabular-nums" data-testid="combat-heal-value">{preview.heal.caption}</p>
							<p class="text-sm tabular-nums" data-testid="combat-heal-mana">{preview.heal.mana_caption}</p>
							{#if preview.heal.oom_caption}<p class="text-muted-foreground text-xs" data-testid="combat-heal-oom">{preview.heal.oom_caption}</p>{/if}
						{/if}
					</div>
				</section>
			</div>
		{:else if !error}
			<p class="text-muted-foreground p-3 text-sm">Loading...</p>
		{/if}

		<footer class="flex justify-end gap-2 border-t p-3">
			<button type="button" class={btn} onclick={onclose} data-testid="combat-cancel">Cancel</button>
			<button type="button" class={primary} disabled={!form || saving} onclick={save} data-testid="combat-save">Save</button>
		</footer>
	</div>
</div>
