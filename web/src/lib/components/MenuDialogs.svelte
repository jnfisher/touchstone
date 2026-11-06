<script lang="ts">
	// The dialogs a context-menu action can raise: the text to copy by hand when the browser would not
	// copy for us, and a Yes/No question (MME's MsgBox).
	import { dialogs } from '$lib/dialogs.svelte';
	import ResultsDialog from './ResultsDialog.svelte';

	let area = $state<HTMLTextAreaElement>();
	$effect(() => {
		if (dialogs.fallback && area) {
			area.focus();
			area.select();
		}
	});
	const btn = 'border-input h-8 rounded-md border px-3 text-sm';
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key !== 'Escape') return;
		if (dialogs.fallback) dialogs.closeText();
		if (dialogs.question) dialogs.question.resolve(undefined);
		if (dialogs.prompt) dialogs.prompt.resolve(undefined);
		if (dialogs.results) dialogs.closeResults();
	}}
/>

{#if dialogs.fallback}
	<div class="bg-background/80 fixed inset-0 z-[60] flex items-center justify-center p-4" role="presentation">
		<div class="bg-card flex w-full max-w-2xl flex-col gap-2 rounded-lg border p-4 shadow-xl" role="dialog" aria-modal="true" aria-label="Copy text" data-testid="clipboard-fallback">
			<p class="text-muted-foreground text-sm">{dialogs.fallback.note}</p>
			<textarea bind:this={area} readonly rows="12" class="bg-background border-input w-full rounded-md border p-2 font-mono text-xs" value={dialogs.fallback.text} data-testid="clipboard-fallback-text"></textarea>
			<div class="flex justify-end"><button type="button" class={btn} onclick={() => dialogs.closeText()}>Close</button></div>
		</div>
	</div>
{/if}

{#if dialogs.question}
	{@const q = dialogs.question}
	<div class="bg-background/80 fixed inset-0 z-[60] flex items-center justify-center p-4" role="presentation">
		<div class="bg-card flex w-full max-w-sm flex-col gap-3 rounded-lg border p-4 shadow-xl" role="dialog" aria-modal="true" aria-label={q.title} data-testid="menu-question">
			<h2 class="text-base font-semibold">{q.title}</h2>
			<p class="text-sm">{q.text}</p>
			<div class="flex justify-end gap-2">
				{#each q.choices as c (c.value)}
					<button type="button" class={btn} onclick={() => q.resolve(c.value)} data-testid="menu-question-{c.value}">{c.label}</button>
				{/each}
				<button type="button" class={btn} onclick={() => q.resolve(undefined)}>Cancel</button>
			</div>
		</div>
	</div>
{/if}

<ResultsDialog />

{#if dialogs.prompt}
	{@const q = dialogs.prompt}
	<div class="bg-background/80 fixed inset-0 z-[60] flex items-center justify-center p-4" role="presentation">
		<div class="bg-card w-full max-w-sm rounded-lg border p-4 shadow-xl" role="dialog" aria-modal="true" aria-label={q.title} data-testid="menu-prompt">
		<form
			class="flex flex-col gap-3"
			onsubmit={(e) => {
				e.preventDefault();
				q.resolve(q.value);
			}}
		>
			<h2 class="text-base font-semibold">{q.title}</h2>
			<label class="flex flex-col gap-1 text-sm">
				<span class="whitespace-pre-line">{q.text}</span>
				<input class="bg-background border-input h-8 rounded-md border px-2" bind:value={q.value} data-testid="menu-prompt-input" />
			</label>
			<div class="flex justify-end gap-2">
				<button type="submit" class={btn} data-testid="menu-prompt-ok">OK</button>
				<button type="button" class={btn} onclick={() => q.resolve(undefined)}>Cancel</button>
			</div>
		</form>
		</div>
	</div>
{/if}
