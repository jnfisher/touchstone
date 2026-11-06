<script lang="ts">
	import type { CharacterStore } from '$lib/character.svelte';
	import { headingCls, inputCls, labelCls } from '../calc/styles';

	let { store }: { store: CharacterStore } = $props();
	const ev = $derived(store.evaluated);
	const doc = $derived(store.doc);
	const tile = 'bg-muted/50 rounded-md border px-3 py-2';
	const tileLabel = 'text-muted-foreground text-[11px] font-medium';
</script>

<div>
	<h3 class={headingCls}>Vitals</h3>
	{#if ev}
		<div class="grid grid-cols-2 gap-2 text-sm" data-testid="char-vitals">
			<div class={tile}>
				<div class={tileLabel}>Hit points</div>
				<div data-testid="char-hp">{ev.hp.caption ?? ''}</div>
				{#if ev.hp.min !== null && ev.hp.max !== null}
					<div class="text-muted-foreground text-xs tabular-nums">{ev.hp.min} to {ev.hp.max}, avg {ev.hp.average}</div>
				{/if}
				<div class="text-muted-foreground text-xs whitespace-pre-line">{(ev.hp.rest_caption ?? '').replace(/\r\n/g, '\n')}</div>
				{#if ev.hp.error_text}<div class="text-destructive text-xs">{ev.hp.error_text}</div>{/if}
			</div>
			<div class={tile}>
				<div class={tileLabel}>Mana</div>
				<div data-testid="char-mana">{ev.mana.max_caption ?? 'None'}</div>
				<div class="text-muted-foreground text-xs whitespace-pre-line">{(ev.mana.regen_caption ?? '').replace(/\r\n/g, '\n')}</div>
				<div class="text-muted-foreground text-xs">{ev.mana.spellcasting_caption ?? ''}</div>
				{#if ev.mana.error_text}<div class="text-destructive text-xs">{ev.mana.error_text}</div>{/if}
			</div>
			<div class={tile}>
				<div class={tileLabel}>Magic resistance</div>
				<div class="tabular-nums" data-testid="char-mr">{ev.magic_res ?? ''}</div>
			</div>
			<div class={tile}>
				<div class={tileLabel}>Dodge</div>
				<div class="tabular-nums" data-testid="char-dodge">{ev.dodge}</div>
				<div class="text-muted-foreground text-xs">{ev.dodge_caption}</div>
			</div>
			<div class={tile}>
				<div class={tileLabel}>Picklocks</div>
				<div data-testid="char-picks">{ev.picklocks_caption ?? 'None'}</div>
			</div>
			<div class={tile}>
				<div class={tileLabel}>Encumbrance</div>
				<div class="tabular-nums" data-testid="char-encum">{ev.encum.current} / {ev.encum.max}</div>
				<div class="text-muted-foreground text-xs">{ev.encum.tag ?? ''}</div>
			</div>
		</div>
		{#if ev.stats_error_text}<p class="text-destructive mt-2 text-xs">{ev.stats_error_text}</p>{/if}
	{:else}
		<p class="text-muted-foreground text-sm">Nothing to show until a class and race are picked.</p>
	{/if}
	<div class="mt-3 grid grid-cols-2 gap-3">
		<label class="flex flex-col gap-1">
			<span class={labelCls}>Magic resist (text)</span>
			<input
				type="text"
				value={doc.mr_text}
				oninput={(e) => store.edit((d) => (d.mr_text = e.currentTarget.value))}
				data-testid="char-mr-text"
				class={inputCls}
			/>
		</label>
		<label class="flex items-end gap-2 pb-1 text-sm">
			<input
				type="checkbox"
				checked={doc.anti_magic}
				onchange={(e) => store.edit((d) => (d.anti_magic = e.currentTarget.checked))}
				data-testid="char-anti-magic"
			/>
			Anti-magic
		</label>
	</div>
</div>
