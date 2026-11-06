<script lang="ts">
	// The "use current character" switch every character-aware calculator carries.
	import type { CurrentCharacter } from './current.svelte';

	let { current }: { current: CurrentCharacter } = $props();
</script>

<div class="bg-muted/40 flex flex-col gap-0.5 rounded-md border px-3 py-2" data-testid="use-character">
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
			No character is loaded. Pick a class and race on the Character tab (F10) to fill these in.
		{:else if current.active}
			{current.name || 'Unnamed'}, level {current.level}: the fields show its values.
		{:else}
			Off: type the numbers in yourself.
		{/if}
	</p>
</div>
