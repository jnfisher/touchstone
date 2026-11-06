<script lang="ts">
	import { ContextMenu as ContextMenuPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: ContextMenuPrimitive.ContentProps = $props();
</script>

<ContextMenuPrimitive.Portal>
	<ContextMenuPrimitive.Content
		bind:ref
		data-slot="context-menu-content"
		class={cn(
			"ring-foreground/10 bg-popover text-popover-foreground z-50 min-w-48 overflow-x-hidden overflow-y-auto rounded-lg p-1 shadow-md ring-1",
			className
		)}
		{...restProps}
	/>
</ContextMenuPrimitive.Portal>
