<script lang="ts">
	// One of MME's message or input boxes, asked by the server in the middle of a paste.
	import { VB_CANCEL, VB_NO, VB_YES, type Answer, type Prompt } from '$lib/paste';

	let {
		prompt,
		title = 'Paste',
		onanswer
	}: { prompt: Prompt; title?: string; onanswer: (a: Answer | undefined) => void } = $props();

	let text = $state('');
	$effect(() => {
		text = typeof prompt.default === 'string' ? prompt.default : '';
	});

	const btn = 'h-8 rounded-md border px-3 text-sm';
</script>

<div
	class="bg-background/70 absolute inset-0 z-10 flex items-center justify-center p-4"
	role="presentation"
>
	<div
		class="bg-card text-card-foreground max-h-full w-full max-w-lg overflow-auto rounded-lg border p-4 shadow-lg"
		role="alertdialog"
		aria-modal="true"
		aria-label={title}
		data-testid="paste-question"
	>
		<p class="text-muted-foreground mb-2 text-[11px] font-medium tracking-wide uppercase">{title}</p>
		<p class="mb-3 text-sm whitespace-pre-wrap" data-testid="paste-question-text">{prompt.text}</p>
		{#if prompt.kind === 'input'}
			<input
				bind:value={text}
				class="bg-background border-input mb-3 h-8 w-full rounded-md border px-2 text-sm"
				aria-label="Answer"
				data-testid="paste-question-input"
			/>
			<div class="flex justify-end gap-2">
				<button type="button" class="{btn} border-input" onclick={() => onanswer('')} data-testid="paste-q-cancel"
					>Cancel</button
				>
				<button
					type="button"
					class="{btn} bg-primary text-primary-foreground border-transparent"
					onclick={() => onanswer(text)}
					data-testid="paste-q-ok">OK</button
				>
			</div>
		{:else}
			<div class="flex justify-end gap-2">
				<button
					type="button"
					class="{btn} {prompt.default === VB_YES ? 'bg-primary text-primary-foreground border-transparent' : 'border-input'}"
					onclick={() => onanswer(VB_YES)}
					data-testid="paste-q-yes">Yes</button
				>
				<button
					type="button"
					class="{btn} {prompt.default === VB_NO ? 'bg-primary text-primary-foreground border-transparent' : 'border-input'}"
					onclick={() => onanswer(VB_NO)}
					data-testid="paste-q-no">No</button
				>
				{#if prompt.kind === 'yes_no_cancel'}
					<button
						type="button"
						class="{btn} {prompt.default === VB_CANCEL ? 'bg-primary text-primary-foreground border-transparent' : 'border-input'}"
						onclick={() => onanswer(VB_CANCEL)}
						data-testid="paste-q-cancel">Cancel</button
					>
				{/if}
			</div>
		{/if}
	</div>
</div>
