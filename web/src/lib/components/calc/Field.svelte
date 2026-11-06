<script lang="ts">
	// A labelled text box. MME reads these with Val(), so they stay text: "abc" is a valid entry.
	import { inputCls, labelCls } from './styles';

	let {
		label,
		value = $bindable(),
		testid,
		note = '',
		disabled = false,
		hint = ''
	}: {
		label: string;
		value: string;
		testid: string;
		/** What the weapon adds or removes, shown beside the box (MME's *Adj labels). */
		note?: string;
		disabled?: boolean;
		hint?: string;
	} = $props();
</script>

<label class="flex flex-col gap-1" title={hint || undefined}>
	<span class="{labelCls} flex h-4 items-center justify-between">
		<span>{label}</span>
		{#if note}<span class="text-foreground tabular-nums" data-testid="{testid}-note">{note}</span>{/if}
	</span>
	<input
		type="text"
		inputmode="decimal"
		autocomplete="off"
		bind:value
		{disabled}
		data-testid={testid}
		class="{inputCls} disabled:opacity-50"
	/>
</label>
