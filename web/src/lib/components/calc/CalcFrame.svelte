<script lang="ts">
	// Inputs on the left, results on the right; stacks on a narrow screen.
	import type { Snippet } from 'svelte';
	import { cardCls } from './styles';

	let {
		title,
		inputs,
		results,
		testid
	}: { title: string; inputs: Snippet; results: Snippet; testid: string } = $props();
</script>

<div class="h-full overflow-auto" data-testid={testid}>
	<h2 class="mb-3 text-base font-semibold">{title}</h2>
	<div class="grid items-start gap-4 lg:grid-cols-[23rem_minmax(0,1fr)]">
		<section class={cardCls} aria-label="Inputs">{@render inputs()}</section>
		<section class={cardCls} aria-label="Results">{@render results()}</section>
	</div>
</div>
