<script lang="ts">
	// The header badge: which rules the loaded database follows (`/api/info` `profile`).
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { api, type Info } from '$lib/api';
	import { isQuicksilver, profileBadge, profileBadgeTitle } from '$lib/info';
	import { onMount } from 'svelte';

	let info = $state<Info>();
	onMount(() => {
		let alive = true;
		api
			.info()
			.then((i) => alive && (info = i))
			.catch(() => {});
		return () => (alive = false);
	});
</script>

{#if info}
	{@const i = info}
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<span
					{...props}
					class="inline-flex h-7 items-center rounded-md border px-2 text-xs font-medium whitespace-nowrap {isQuicksilver(i)
						? 'border-primary bg-primary/10 text-foreground'
						: 'border-input text-muted-foreground'}"
					data-testid="profile-badge"
					data-profile={i.profile?.name ?? 'stock'}
				>
					{profileBadge(i)}
				</span>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content data-testid="profile-badge-tip">{profileBadgeTitle(i)}</Tooltip.Content>
	</Tooltip.Root>
{/if}
