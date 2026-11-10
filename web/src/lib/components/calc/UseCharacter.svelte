<script lang="ts">
	// The "use current character" switch every character-aware calculator carries: which character it is using,
	// a link to its sheet, and a "use another" dropdown that borrows a realm character for this calculator only.
	import { goto } from '$app/navigation';
	import type { CurrentCharacter } from './current.svelte';
	import CharacterDropdown from './CharacterDropdown.svelte';

	let { current }: { current: CurrentCharacter } = $props();
	const src = $derived(current.source);
</script>

<div class="bg-muted/40 flex flex-col gap-1 rounded-md border px-3 py-2" data-testid="use-character">
	<label class="flex items-center gap-2 text-sm font-medium">
		<input
			type="checkbox"
			checked={current.use}
			disabled={current.character === undefined}
			onchange={(e) => (current.use = e.currentTarget.checked)}
			data-testid="use-character-toggle"
		/>
		Use current character
	</label>
	<p class="text-muted-foreground text-xs" data-testid="use-character-note">
		{#if current.character === undefined}
			{#if current.override !== undefined}
				Loading {src.name}...
			{:else}
				No character is loaded. Pick one in the header, or a class and race on the Character tab (F10).
			{/if}
		{:else if current.active}
			<span data-testid="use-character-who">{src.name}{src.kind ? `, ${src.kind}` : ''}, level {src.level}</span>{src.via === 'override' ? ' (chosen for this calculator)' : ''}: the fields show its values.
			<button type="button" class="text-primary underline" data-testid="use-character-sheet" onclick={() => current.openSheet((p) => void goto(p))}>Open sheet</button>
		{:else}
			Off: type the numbers in yourself.
		{/if}
	</p>
	{#if current.realmAvailable}
		<div class="flex flex-wrap items-center gap-2">
			<CharacterDropdown
				characters={current.choices}
				selected={current.override === undefined ? [] : [current.override]}
				label="Use another character"
				testid="use-another"
				onpick={([id]) => void current.useAnother(id)}
			/>
			{#if current.override !== undefined}
				<button type="button" class="border-input hover:bg-accent h-7 rounded-md border px-2 text-xs" data-testid="use-another-reset" onclick={() => current.clearOverride()}>
					Back to the active character
				</button>
			{/if}
			{#if current.overrideError}<span class="text-destructive text-xs" role="alert">{current.overrideError}</span>{/if}
		</div>
	{/if}
</div>
