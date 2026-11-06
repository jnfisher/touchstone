<script lang="ts">
	// `FilterMonsters`' "Calculate Monster Damage vs Character (or Party) Defenses first?" box: Yes calculates and lists,
	// No lists with the damage as it stands and does not ask again until the defenses change, Cancel leaves the list.
	import type { DamagePrompt } from '$lib/monsterdamage';

	let {
		prompt,
		onanswer
	}: { prompt: DamagePrompt; onanswer: (a: 'yes' | 'no' | 'cancel') => void } = $props();

	const btn = 'h-8 rounded-md border px-3 text-sm';
</script>

<div class="bg-background/70 absolute inset-0 z-10 flex items-center justify-center p-4" role="presentation">
	<div
		class="bg-card text-card-foreground w-full max-w-lg rounded-lg border p-4 shadow-lg"
		role="alertdialog"
		aria-modal="true"
		aria-label={prompt.title}
		data-testid="damage-question"
	>
		<p class="text-muted-foreground mb-2 text-[11px] font-medium tracking-wide uppercase">{prompt.title}</p>
		<p class="mb-3 text-sm" data-testid="damage-question-text">{prompt.message}</p>
		<div class="flex justify-end gap-2">
			<button
				type="button"
				class="{btn} bg-primary text-primary-foreground border-transparent"
				onclick={() => onanswer('yes')}
				data-testid="damage-q-yes">Yes</button
			>
			<button type="button" class="{btn} border-input" onclick={() => onanswer('no')} data-testid="damage-q-no"
				>No</button
			>
			<button type="button" class="{btn} border-input" onclick={() => onanswer('cancel')} data-testid="damage-q-cancel"
				>Cancel</button
			>
		</div>
	</div>
</div>
